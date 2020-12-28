use super::{
	index::{INDEX_V0_HEADER_LIMIT, INDEX_V0_MAGIC},
	table::{TableBuilder, TABLE_MAX_DEPTH},
	ContentType,
};
use byteorder::WriteBytesExt;
use std::io;

pub struct Builder<W> {
	key_size: u8,
	payload_size: u8,
	table: TableBuilder,
	database: W,
}

impl<W> Builder<W>
where
	W: io::Write + io::Seek,
{
	pub fn create(
		mut database: W,
		content_type: ContentType,
		description: &str,
		key_size: u8,
		payload_size: u8,
		depth: u8,
	) -> Result<Self, BuilderCreateError> {
		let start = database.seek(io::SeekFrom::Current(0))?;
		if key_size == 0 {
			return Err(BuilderCreateError::InvalidKeyLength);
		}
		if depth > TABLE_MAX_DEPTH {
			return Err(BuilderCreateError::InvalidKeyLength);
		}
		if (depth as u32 + 7) / 8 > key_size as u32 {
			return Err(BuilderCreateError::InvalidKeyLength);
		}
		if description.contains('\n') {
			return Err(BuilderCreateError::InvalidDescription {
				description: description.to_string(),
			});
		}
		database.write_all(INDEX_V0_MAGIC.as_bytes())?;
		database.write_all(b"\n")?;
		database.write_all(content_type.as_bytes())?;
		database.write_all(b"\n")?;
		database.write_all(description.as_bytes())?;
		database.write_all(b"\n")?;
		database.write_u8(key_size)?;
		database.write_u8(payload_size)?;
		let header_end = database.seek(io::SeekFrom::Current(0))?;
		let header_size = header_end - start;
		if header_size > INDEX_V0_HEADER_LIMIT {
			return Err(BuilderCreateError::HeaderTooBig);
		}
		let table = TableBuilder::new(depth);
		Ok(Self { key_size, payload_size, table, database })
	}

	pub fn add_entry(&mut self, key: &[u8], payload: &[u8]) -> io::Result<()> {
		assert_eq!(key.len(), self.key_size as usize);
		assert_eq!(payload.len(), self.payload_size as usize);
		self.table.write_key(&mut self.database, key)?;
		self.database.write_all(payload)?;
		Ok(())
	}

	pub fn finish(mut self) -> io::Result<()> {
		self.table.close(&mut self.database)?;
		Ok(())
	}
}

#[derive(thiserror::Error, Debug)]
pub enum BuilderCreateError {
	#[error("IO error: {0}")]
	IOError(#[from] io::Error),
	#[error("Invalid description: {description:?}")]
	InvalidDescription { description: String },
	#[error("invalid key / table depth length")]
	InvalidKeyLength,
	#[error("Header too big")]
	HeaderTooBig,
}
