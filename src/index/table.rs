use byteorder::{ReadBytesExt, WriteBytesExt, BE};
use std::io::{self, Read, Write};
use std::ops::Range;
use std::cmp::Ordering;

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Depth(u8);

impl Depth {
	const KEY_BYTES: usize = std::mem::size_of::<BucketIndexInner>();
	const KEY_BITS: usize = 8 * Self::KEY_BYTES;
	const KEY_BITS_U8: u8 = Self::KEY_BITS as u8;

	// must be less than bit width of BucketIndex!
	// also limits table size (stored in memory; u64 per entry):
	// * 20 bits -> 8MB MB table
	// * 24 bits -> 128 MB table
	// and uncompressed table size must fit in a 32-bit var!
	const TABLE_MAX_DEPTH: u8 = 24;

	pub(super) fn new(depth: u8) -> Option<Self> {
		// static assert?
		// need to store index past the end of depth bits, i.e. at least one bit more:
		assert!((Self::TABLE_MAX_DEPTH as usize) < Self::KEY_BITS);

		if depth > Self::TABLE_MAX_DEPTH {
			return None;
		}
		Some(Self(depth))
	}

	pub(super) fn valid_key_size(&self, key_bytes: u8) -> bool {
		if key_bytes == 0 {
			return false;
		}
		if (self.0 / 8) + 1 > key_bytes {
			// expect at least one byte after prefix
			// implementation depends on this (design does not, but we expect long keys anyway)
			return false;
		}
		true
	}

	fn table_entries(self) -> usize {
		(1 << self.0) + 1
	}

	pub(super) fn entry_size(self, key_size: u8, payload_size: u8) -> usize {
		let strip_key_prefix = self.0 as usize / 8;
		(key_size as usize) - strip_key_prefix + (payload_size as usize)
	}

	fn index(self, key: &[u8]) -> BucketIndex {
		if self.0 == 0 {
			return BucketIndex(0);
		}
		let mut raw = [0u8; Self::KEY_BYTES];
		let raw_len = std::cmp::min(key.len(), raw.len());
		// copy data
		raw[..raw_len].copy_from_slice(&key[..raw_len]);
		// don't care if key was too short for depth... it just gets zero-padded.
		let ndx = u32::from_be_bytes(raw);
		let start = ndx >> (Self::KEY_BITS_U8 - self.0);
		BucketIndex(start)
	}

	fn prepare_key(self, key: &[u8]) -> (u8, &[u8]) {
		let strip_key_prefix = self.0 as usize / 8;
		// don't store unncessary bits; i.e. strip of "depth" bits of key
		// don't shift bits though, so skip full bytes and zero partial bits.
		// returns first (possibly masked byte) and qwremaining slice of key bytes.
		let suffix = &key[strip_key_prefix..];
		let partial_bits = self.0 & 0x7;
		let mask_bits = 0xff >> partial_bits;
		(suffix[0] & mask_bits, &suffix[1..])
	}

	/*
	fn index_range(self, key: &[u8], key_bits: u32) -> Range<BucketIndex> {
		if self.0 == 0 {
			return BucketIndex(0)..BucketIndex(1);
		}
		if key_bits == 0 {
			return BucketIndex(0)..BucketIndex(1 << self.0);
		}
		let mut raw = [0u8; Self::KEY_BYTES];
		let raw_len = std::cmp::min(key.len(), raw.len());
		// copy data
		raw[..raw_len].copy_from_slice(&key[..raw_len]);
		let mut ndx = u32::from_be_bytes(raw);
		if key_bits < self.0 as u32 {
			debug_assert!(key_bits > 0 && key_bits <= Self::TABLE_MAX_DEPTH as u32);
			// truncate to key_bits
			let trunc_shift = Self::KEY_BITS_U8 - (key_bits as u8);
			ndx = (ndx >> trunc_shift) << trunc_shift;
			// find window
			let start = ndx >> (Self::KEY_BITS_U8 - self.0);
			let window = self.0 - (key_bits as u8);
			BucketIndex(start)..BucketIndex(start + (1 << window))
		} else {
			let start = ndx >> (Self::KEY_BITS_U8 - self.0);
			BucketIndex(start)..BucketIndex(start + 1)
		}
	}
	*/

	pub(super) fn start_forward_search(self, key: &[u8]) -> ForwardSearch {
		ForwardSearch::new(self, key)
	}

}

pub(super) enum ForwardSearchResult<'data> {
	Match(&'data [u8]),
	Continue,
	Break,
}

pub(super) struct ForwardSearch<'key> {
	mask_bits: u8,
	suffix: &'key [u8],
}

impl<'key> ForwardSearch<'key> {
	fn new(depth: Depth, key: &'key [u8]) -> Self {
		let strip_key_prefix = depth.0 as usize / 8;
		let suffix = &key[strip_key_prefix..];
		let partial_bits = depth.0 & 0x7;
		let mask_bits = 0xff >> partial_bits;
		Self { mask_bits, suffix }
	}

	pub(super) fn test_entry<'data>(&self, entry: &'data [u8]) -> ForwardSearchResult<'data> {
		let (entry_key, entry_payload) = entry.split_at(self.suffix.len());
		match (self.suffix[0] & self.mask_bits).cmp(&(entry[0] & self.mask_bits)).then_with(|| self.suffix[1..].cmp(&entry_key[1..])) {
			Ordering::Equal => ForwardSearchResult::Match(entry_payload),
			// key we're looking for still greater than entry from file
			Ordering::Greater => ForwardSearchResult::Continue,
			// now entry in file greater than our search key - only getting worse now.
			Ordering::Less => ForwardSearchResult::Break,
		}
	}
}

type BucketIndexInner = u32;

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct BucketIndex(BucketIndexInner);

impl BucketIndex {
	fn entry(self) -> usize {
		self.0 as usize
	}
}

pub(super) struct Table {
	depth: Depth,
	file_offsets: Vec<u64>,
}

impl Table {
	fn new(depth: Depth, file_offsets: Vec<u64>) -> Self {
		Self {
			depth,
			file_offsets,
		}
	}

	pub(super) fn depth(&self) -> Depth {
		self.depth
	}

	pub(super) fn lookup(&self, key: &[u8]) -> Range<u64> {
		let start = self.depth.index(key);
		self.file_offsets[start.entry()]..self.file_offsets[start.entry() + 1]
	}

	/*
	pub(super) fn lookup_range(&self, key: &[u8], key_bits: u32) -> Range<u64> {
		let Range { start, end } = self.depth.index_range(key, key_bits);
		self.file_offsets[start.entry()]..self.file_offsets[end.entry()]
	}
	*/

	pub(super) fn open<R>(mut input: R) -> Result<Self, TableReadError>
	where
		R: io::Read + io::Seek,
	{
		input.seek(io::SeekFrom::End(-4))?;
		let table_size = input.read_u32::<BE>()? as u64;
		input.seek(io::SeekFrom::End(-4 - table_size as i64))?;
		let mut tbl_reader = flate2::read::DeflateDecoder::new(input.take(table_size));
		let depth = tbl_reader.read_u8()?;
		let depth = Depth::new(depth).ok_or(TableReadError::InvalidDepth { depth })?;
		let entries = depth.table_entries();
		let mut file_offsets: Vec<u64> = Vec::new();
		file_offsets.resize(entries, 0);
		tbl_reader.read_u64_into::<BE>(&mut file_offsets)?;
		if 1 == tbl_reader.read(&mut [0])? {
			return Err(TableReadError::TooMuchTableData);
		}
		for i in 0..(entries - 1) {
			if file_offsets[i] > file_offsets[i + 1] {
				return Err(TableReadError::InvalidTableOffsets);
			}
		}
		Ok(Table::new(depth, file_offsets))
	}
}

#[derive(thiserror::Error, Debug)]
pub enum TableReadError {
	#[error("IO error: {0}")]
	IOError(#[from] io::Error),
	#[error("Invalid depth {depth}")]
	InvalidDepth { depth: u8 },
	#[error("Table data too large")]
	TooMuchTableData,
	#[error("Table offsets not increasing")]
	InvalidTableOffsets,
}

pub(super) struct TableBuilder {
	table: Table,
	current_index: Option<BucketIndex>,
	previous_entry: Vec<u8>,
}

impl TableBuilder {
	pub(super) fn new(depth: Depth) -> Self {
		Self {
			table: Table::new(depth, Vec::new()),
			current_index: None,
			previous_entry: Vec::new(),
		}
	}

	fn fill_index<W: io::Seek>(&mut self, database: &mut W, index: BucketIndex) -> io::Result<()> {
		if let Some(cur_ndx) = self.current_index {
			debug_assert!(self.table.file_offsets.len() == (cur_ndx.0 as usize) + 1);
			assert!(index >= cur_ndx);
		}
		let target_size = index.0 as usize + 1;
		if target_size != self.table.file_offsets.len() {
			let pos = database.stream_position()?;
			self.table.file_offsets.resize(target_size, pos);
		}
		self.current_index = Some(index);
		Ok(())
	}

	pub(super) fn write_key<W: io::Write + io::Seek>(
		&mut self,
		database: &mut W,
		key: &[u8],
	) -> io::Result<()> {
		if self.previous_entry.is_empty() {
			debug_assert!(self.current_index.is_none());
			self.previous_entry = key.to_vec();
		} else {
			debug_assert!(self.current_index.is_some());
			debug_assert!(self.previous_entry.len() == key.len());
			assert!(self.previous_entry.as_slice() < key);
			self.previous_entry.copy_from_slice(key);
		}
		let ndx = self.table.depth.index(key);
		self.fill_index(database, ndx)?;
		let (k1, k_suffix) = self.table.depth.prepare_key(key);
		database.write_all(&[k1])?;
		database.write_all(k_suffix)?;
		Ok(())
	}

	pub(super) fn close<W: io::Write + io::Seek>(&mut self, database: &mut W) -> io::Result<()> {
		let table_start = database.stream_position()?;
		let entries = self.table.depth.table_entries();
		self.table.file_offsets.resize(entries, table_start);
		let mut tbl_writer =
			flate2::write::DeflateEncoder::new(database.by_ref(), flate2::Compression::default());
		tbl_writer.write_u8(self.table.depth.0)?;
		for &p in &self.table.file_offsets {
			tbl_writer.write_u64::<BE>(p)?;
		}
		tbl_writer.flush()?;
		drop(tbl_writer);
		let table_size = database.stream_position()? - table_start;
		// due to limited table depth this shouldn't exceed u32 ever.
		assert!(table_size < (u32::MAX as u64));
		database.write_u32::<BE>(table_size as u32)?;
		database.flush()?;
		Ok(())
	}
}
