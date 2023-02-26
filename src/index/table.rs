use byteorder::{ReadBytesExt, WriteBytesExt, BE};
use std::io::{self, Read, Write};
use std::ops::Range;
use std::cmp::Ordering;

pub struct Suffix<'key> {
	mask_bits: u8,  // bits to use in first octect of suffix
	key_suffix: &'key [u8],
}

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

	fn prefix(self, key: &[u8]) -> Prefix {
		let mut raw = [0u8; Self::KEY_BYTES];
		if self.0 == 0 {
			return Prefix { raw, depth: self };
		}
		let mask: BucketIndexInner = (!0) << (Self::KEY_BITS_U8 - self.0); // self.0 == 0 would overflow shift
		let raw_len = std::cmp::min(key.len(), raw.len());
		// copy data
		// don't care if key was too short for depth... it just gets zero-padded.
		raw[..raw_len].copy_from_slice(&key[..raw_len]);
		// truncate
		let raw = (BucketIndexInner::from_be_bytes(raw) & mask).to_be_bytes();
		Prefix { raw, depth: self }
	}

	fn index(self, key: &[u8]) -> BucketIndex {
		self.prefix(key).index()
	}

	fn prepare_key(self, key: &[u8]) -> Suffix {
		let strip_key_prefix = self.0 as usize / 8;
		// don't store unncessary bits; i.e. strip of "depth" bits of key
		// don't shift bits though, so skip full bytes and zero partial bits.
		// returns first (possibly masked byte) and remaining slice of key bytes.
		let suffix = &key[strip_key_prefix..];
		let partial_bits = self.0 & 0x7;
		let mask_bits = 0xff >> partial_bits;
		Suffix { mask_bits, key_suffix: suffix }
	}

	fn prefix_range(self, key: &[u8], key_bits: u32) -> PrefixRange {
		if self.0 == 0 {
			// step could also be "0" - really doesn't matter.
			return PrefixRange { first: Some(0), last: 0, step: 1, depth: self };
		}
		let mask: BucketIndexInner = (!0) << (Self::KEY_BITS_U8 - self.0); // self.0 == 0 would overflow shift
		let step: BucketIndexInner = 1 << (Self::KEY_BITS_U8 - self.0);
		if key_bits == 0 {
			return PrefixRange { first: Some(0), last: mask, step, depth: self };
		}
		let mut raw = [0u8; Self::KEY_BYTES];
		let raw_len = std::cmp::min(key.len(), raw.len());
		// copy data
		raw[..raw_len].copy_from_slice(&key[..raw_len]);
		let ndx = u32::from_be_bytes(raw) & mask;
		if key_bits < self.0 as u32 {
			debug_assert!(key_bits > 0 && key_bits <= Self::TABLE_MAX_DEPTH as u32);
			// find first key_bits bits
			let key_mask: BucketIndexInner = (!0) << (Self::KEY_BITS_U8 - key_bits as u8);
			// truncate to key_bits
			let ndx = ndx & key_mask;
			// set the bits in the prefix that are allowed to be used but are not part
			// of the key prefix to get the last prefix covered by the key
			let last = ndx | (mask & !key_mask);
			PrefixRange { first: Some(ndx), last, step, depth: self }
		} else {
			PrefixRange { first: Some(ndx), last: ndx, step, depth: self }
		}
	}

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
	suffix: Suffix<'key>,
}

impl<'key> ForwardSearch<'key> {
	fn new(depth: Depth, key: &'key [u8]) -> Self {
		let suffix = depth.prepare_key(key);
		Self { suffix }
	}

	pub(super) fn test_entry<'data>(&self, entry: &'data [u8]) -> ForwardSearchResult<'data> {
		let Suffix { mask_bits, key_suffix } = self.suffix;
		let (entry_key, entry_payload) = entry.split_at(key_suffix.len());
		match (key_suffix[0] & mask_bits).cmp(&(entry[0] & mask_bits)).then_with(|| key_suffix[1..].cmp(&entry_key[1..])) {
			Ordering::Equal => ForwardSearchResult::Match(entry_payload),
			// key we're looking for still greater than entry from file
			Ordering::Greater => ForwardSearchResult::Continue,
			// now entry in file greater than our search key - only getting worse now.
			Ordering::Less => ForwardSearchResult::Break,
		}
	}
}

pub(super) struct ForwardRangeSearch<'key> {
	prefix: &'key [u8],
	prefix_end: u8,
	mask_end_bits: u8,
}

impl<'key> ForwardRangeSearch<'key> {
	pub(super) fn new(key: &'key [u8], key_bits: u32) -> Self {
		if key_bits == 0 {
			return Self { prefix: b"", prefix_end: 0, mask_end_bits: 0 };
		}
		let len_clipped = (key_bits / 8) as usize;
		let partial_bits = key_bits & 0x7;
		if partial_bits > 0 {
			let mask_end_bits = !(0xff >> partial_bits);
			Self { prefix: &key[..len_clipped], prefix_end: key[len_clipped] & mask_end_bits, mask_end_bits }
		} else {
			// fully match last byte with mask
			Self { prefix: &key[..len_clipped-1], prefix_end: key[len_clipped-1], mask_end_bits: 0xff }
			// this should work too:
			// Self { prefix: &key[..len_clipped], prefix_end: 0, mask_end_bits: 0 }
		}
	}

	pub(super) fn test_key<'data>(&self, key: &'data [u8]) -> ForwardSearchResult<'data> {
		match self.prefix.cmp(&key[..self.prefix.len()]).then_with(|| {
			self.prefix_end.cmp(&(key[self.prefix.len()] & self.mask_end_bits))
		}) {
			Ordering::Equal => ForwardSearchResult::Match(key),
			// key we're looking for still greater than entry from file
			Ordering::Greater => ForwardSearchResult::Continue,
			// now entry in file greater than our search key - only getting worse now.
			Ordering::Less => ForwardSearchResult::Break,
		}
	}
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub(super) struct Prefix {
	raw: [u8; Depth::KEY_BYTES],
	depth: Depth,
}

impl Prefix {
	pub(super) fn index(self) -> BucketIndex {
		if self.depth.0 == 0 { return BucketIndex(0); }
		BucketIndex(BucketIndexInner::from_be_bytes(self.raw) >> (32 - self.depth.0))
	}

	pub(super) fn fix_entry(self, entry: &mut [u8]) {
		let full_prefix_bytes = (self.depth.0 / 8) as usize;
		entry[..full_prefix_bytes].copy_from_slice(&self.raw[..full_prefix_bytes]);
		let partial_bits = self.depth.0 & 0x7;
		if partial_bits != 0 {
			let mask_bits = 0xff >> partial_bits;
			entry[full_prefix_bytes] = (entry[full_prefix_bytes] & mask_bits) | (self.raw[full_prefix_bytes] & !mask_bits);
		}
	}
}

pub(super) struct PrefixRange {
	// although we use BucketIndexInner as type here, it uses the
	// **unshifted** value, which is why we need to increment by step
	// instead of (constant) 1.
	first: Option<BucketIndexInner>,
	last: BucketIndexInner,
	step: BucketIndexInner,
	depth: Depth,
}

impl Iterator for PrefixRange {
	type Item = Prefix;

	fn next(&mut self) -> Option<Self::Item> {
		let current = self.first?;
		if current <= self.last {
			// automatically stop on overflow
			self.first = current.checked_add(self.step);
			Some(Prefix { raw: current.to_be_bytes(), depth: self.depth })
		} else {
			None
		}
	}

	fn size_hint(&self) -> (usize, Option<usize>) {
		let l = self.len();
		(l, Some(l))
	}
}

impl DoubleEndedIterator for PrefixRange {
	fn next_back(&mut self) -> Option<Self::Item> {
		let first = self.first?;
		if first <= self.last {
			let current = self.last;
			match self.last.checked_sub(self.step) {
				None => { self.first = None; },
				Some(last) => { self.last = last; },
			}
			Some(Prefix { raw: current.to_be_bytes(), depth: self.depth })
		} else {
			None
		}
	}
}

impl ExactSizeIterator for PrefixRange {
	fn len(&self) -> usize {
		if let Some(first) = self.first {
			1 + ((self.last - first) / self.step) as usize
		} else {
			0
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

	pub(super) fn prefix_range(&self, key: &[u8], key_bits: u32) -> PrefixRange {
		self.depth.prefix_range(key, key_bits)
	}

	pub(super) fn lookup_prefix(&self, prefix: Prefix) -> Range<u64> {
		assert_eq!(prefix.depth, self.depth);
		let start = prefix.index();
		self.file_offsets[start.entry()]..self.file_offsets[start.entry() + 1]
	}

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
		let k_suffix = self.table.depth.prepare_key(key);
		database.write_all(&[k_suffix.key_suffix[0] & k_suffix.mask_bits])?;
		database.write_all(&k_suffix.key_suffix[1..])?;
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
