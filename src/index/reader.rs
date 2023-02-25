use byteorder::ReadBytesExt;
use std::convert::TryFrom;
use std::io::{self, BufRead, Read, Seek};

use crate::buf_read::{BufReader, FileLen, ReadAt};

use super::{
	table::{Table, TableReadError},
	ContentType, ContentTypeParseError,
};

pub const INDEX_V0_MAGIC: &str = "hash-index-v0";
pub const INDEX_V0_HEADER_LIMIT: u64 = 4096;

pub struct Index<R> {
	content_type: ContentType,
	description: String,
	key_size: u8,
	payload_size: u8,
	table: Table,
	database: R,
}

impl<R> Index<R>
where
	R: io::Read + io::Seek + ReadAt + FileLen,
{
	pub fn content_type(&self) -> &ContentType {
		&self.content_type
	}

	pub fn description(&self) -> &str {
		&self.description
	}

	pub fn key_size(&self) -> u8 {
		self.key_size
	}

	pub fn payload_size(&self) -> u8 {
		self.payload_size
	}

	pub fn open(mut database: R) -> Result<Self, IndexOpenError> {
		let mut reader = io::BufReader::new(&mut database);
		reader.rewind()?;
		let mut header = reader.by_ref().take(INDEX_V0_HEADER_LIMIT);
		let mut magic = String::new();
		let mut content_type = String::new();
		let mut description = String::new();
		header.read_line(&mut magic)?;
		header.read_line(&mut content_type)?;
		header.read_line(&mut description)?;
		if !magic.ends_with('\n') || !content_type.ends_with('\n') || !description.ends_with('\n') {
			return Err(IndexOpenError::InvalidHeader);
		}
		magic.pop();
		content_type.pop();
		description.pop();
		if magic != INDEX_V0_MAGIC {
			return Err(IndexOpenError::InvalidHeader);
		}
		let content_type = ContentType::try_from(content_type)?;
		let key_size = header.read_u8()?;
		let payload_size = header.read_u8()?;
		#[allow(clippy::drop_non_drop)]
		drop(header); // must be done with header parsing here
		let table = Table::read(reader.by_ref())?;
		if key_size == 0 {
			return Err(IndexOpenError::InvalidKeyLength);
		}
		if (table.depth() as u32 + 7) / 8 > key_size as u32 {
			return Err(IndexOpenError::InvalidKeyLength);
		}
		drop(reader);
		Ok(Self { content_type, description, key_size, payload_size, table, database })
	}

	pub fn lookup<'a>(
		&self,
		key: &[u8],
		payload: &'a mut [u8],
	) -> Result<Option<&'a mut [u8]>, LookupError> {
		assert_ne!(self.key_size, 0);
		assert_eq!(key.len(), self.key_size as usize);
		let needle = self.table.mask(key);
		let std::ops::Range { start, end } = self.table.lookup(key);
		let length = end - start;
		let entry_size = needle.len() + self.payload_size as usize;
		if length % entry_size as u64 != 0 {
			return Err(LookupError::InvalidSegmentLength);
		}
		let num_entries = length / entry_size as u64;
		let mut database = BufReader::new(&self.database, 16);
		database.seek(io::SeekFrom::Start(start))?;
		let mut entry_buf = Vec::new();
		entry_buf.resize(needle.len() + self.payload_size as usize, 0u8);
		for _ in 0..num_entries {
			// read (partial) key with payload in one operation
			database.read_exact(&mut entry_buf)?;
			let db_key = &entry_buf[..needle.len()];
			if db_key == needle {
				let p_len = std::cmp::min(payload.len(), self.payload_size as usize);
				let payload = &mut payload[..p_len];
				payload.copy_from_slice(&entry_buf[needle.len()..][..p_len]);
				return Ok(Some(payload));
			} else if db_key > needle.as_slice() {
				break;
			}
		}
		Ok(None)
	}
}

#[derive(thiserror::Error, Debug)]
pub enum IndexOpenError {
	#[error("IO error: {0}")]
	IOError(#[from] io::Error),
	#[error("content-type error: {0}")]
	ContentTypeError(#[from] ContentTypeParseError),
	#[error("table read error: {0}")]
	TableReadError(#[from] TableReadError),
	#[error("invalid key / table depth length")]
	InvalidKeyLength,
	#[error("invalid/unknown header format")]
	InvalidHeader,
}

#[derive(thiserror::Error, Debug)]
pub enum LookupError {
	#[error("IO error: {0}")]
	IOError(#[from] io::Error),
	#[error("Invalid length of segment containing key (not a multiple of entry size)")]
	InvalidSegmentLength,
}
