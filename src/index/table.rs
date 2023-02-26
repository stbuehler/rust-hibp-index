use byteorder::{ReadBytesExt, WriteBytesExt, BE};
use std::io::{self, Read, Write};
use std::ops::Range;

use super::{BucketIndex, Depth, Prefix, PrefixRange};

pub(super) struct Table {
	depth: Depth,
	file_offsets: Vec<u64>,
}

impl Table {
	fn new(depth: Depth, file_offsets: Vec<u64>) -> Self {
		Self { depth, file_offsets }
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
		assert_eq!(prefix.depth(), self.depth);
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
			debug_assert!(self.table.file_offsets.len() == cur_ndx.entry() + 1);
			assert!(index >= cur_ndx);
		}
		let target_size = index.entry() + 1;
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
		database.write_all(k_suffix.first_byte())?;
		database.write_all(k_suffix.remaining_bytes())?;
		Ok(())
	}

	pub(super) fn close<W: io::Write + io::Seek>(&mut self, database: &mut W) -> io::Result<()> {
		let table_start = database.stream_position()?;
		let entries = self.table.depth.table_entries();
		self.table.file_offsets.resize(entries, table_start);
		let mut tbl_writer =
			flate2::write::DeflateEncoder::new(database.by_ref(), flate2::Compression::default());
		tbl_writer.write_u8(self.table.depth.as_u8())?;
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
