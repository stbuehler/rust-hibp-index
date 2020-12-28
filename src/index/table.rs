use byteorder::{ReadBytesExt, WriteBytesExt, BE};
use std::io::{self, Read, Write};
use std::ops::Range;

pub const TABLE_MAX_DEPTH: u8 = 24;

pub struct Table {
	depth: u8,
	file_offsets: Vec<u64>,
}

fn index(mut depth: u8, key: &[u8]) -> usize {
	assert!(depth <= TABLE_MAX_DEPTH);
	let mut index = 0;
	let mut key_pos = 0;
	while depth >= 8 {
		depth -= 8;
		index |= (key[key_pos] as usize) << depth;
		key_pos += 1;
	}
	if depth > 0 {
		index |= (key[key_pos] >> (8 - depth)) as usize;
	}
	index
}

impl Table {
	pub fn mask(&self, key: &[u8]) -> Vec<u8> {
		let mut masked_key = key[(self.depth / 8) as usize..].to_vec();
		let partial_bits = self.depth & 0x7;
		if partial_bits > 0 {
			let mask_bits = 0xff >> partial_bits;
			masked_key[0] &= mask_bits;
		}
		masked_key
	}

	pub fn depth(&self) -> u8 {
		self.depth
	}

	pub fn lookup(&self, key: &[u8]) -> Range<u64> {
		let ndx = index(self.depth, key);
		self.file_offsets[ndx]..self.file_offsets[ndx + 1]
	}

	pub fn read<R>(mut input: R) -> Result<Self, TableReadError>
	where
		R: io::Read + io::Seek,
	{
		input.seek(io::SeekFrom::End(-4))?;
		let table_size = input.read_u32::<BE>()? as u64;
		input.seek(io::SeekFrom::End(-4 - table_size as i64))?;
		let mut tbl_reader = flate2::read::DeflateDecoder::new(input.take(table_size));
		let depth = tbl_reader.read_u8()?;
		if depth > TABLE_MAX_DEPTH {
			return Err(TableReadError::InvalidDepth { depth });
		}
		let entries: u32 = (1 << depth) + 1;
		let mut file_offsets: Vec<u64> = Vec::new();
		file_offsets.resize(entries as usize, 0);
		tbl_reader.read_u64_into::<BE>(&mut file_offsets)?;
		if 1 == tbl_reader.read(&mut [0])? {
			return Err(TableReadError::TooMuchTableData);
		}
		for i in 0..(entries - 1) as usize {
			if file_offsets[i] > file_offsets[i + 1] {
				return Err(TableReadError::InvalidTableOffsets);
			}
		}
		Ok(Self { depth, file_offsets })
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

pub struct TableBuilder {
	table: Table,
	current_index: Option<usize>,
	previous_entry: Vec<u8>,
}

impl TableBuilder {
	pub fn new(depth: u8) -> Self {
		assert!(depth <= TABLE_MAX_DEPTH);
		Self {
			table: Table { depth, file_offsets: Vec::new() },
			current_index: None,
			previous_entry: Vec::new(),
		}
	}

	fn fill_index<W: io::Seek>(&mut self, database: &mut W, index: usize) -> io::Result<()> {
		if let Some(cur_ndx) = self.current_index {
			debug_assert!(self.table.file_offsets.len() == cur_ndx + 1);
			assert!(index >= cur_ndx);
		}
		let target_size = index + 1;
		if target_size != self.table.file_offsets.len() {
			let pos = database.seek(io::SeekFrom::Current(0))?;
			self.table.file_offsets.resize(index + 1, pos);
		}
		self.current_index = Some(index);
		Ok(())
	}

	pub fn write_key<W: io::Write + io::Seek>(
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
		let ndx = index(self.table.depth, key);
		self.fill_index(database, ndx)?;
		let store_key = &key[(self.table.depth / 8) as usize..];
		let partial_bits = self.table.depth & 0x7;
		if partial_bits > 0 {
			let mask_bits = 0xff >> partial_bits;
			let first_byte = store_key[0] & mask_bits;
			database.write_all(&[first_byte])?;
			database.write_all(&store_key[1..])?;
		} else {
			database.write_all(store_key)?;
		}
		Ok(())
	}

	pub fn close<W: io::Write + io::Seek>(&mut self, database: &mut W) -> io::Result<()> {
		let table_start = database.seek(io::SeekFrom::Current(0))?;
		let entries = (1 << self.table.depth) + 1;
		self.table.file_offsets.resize(entries, table_start);
		let mut tbl_writer =
			flate2::write::DeflateEncoder::new(database.by_ref(), flate2::Compression::default());
		tbl_writer.write_u8(self.table.depth)?;
		for &p in &self.table.file_offsets {
			tbl_writer.write_u64::<BE>(p)?;
		}
		tbl_writer.flush()?;
		drop(tbl_writer);
		let table_size = database.seek(io::SeekFrom::Current(0))? - table_start;
		// due to limited table depth this shouldn't exceed u32 ever.
		assert!(table_size < (u32::MAX as u64));
		database.write_u32::<BE>(table_size as u32)?;
		database.flush()?;
		Ok(())
	}
}
