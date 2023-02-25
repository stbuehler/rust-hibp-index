use super::{
	reader::{INDEX_V0_HEADER_LIMIT, INDEX_V0_MAGIC},
	table::{Depth, TableBuilder},
	ContentTypeData, KnownContentType,
};
use anyhow::Context;
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
		content_type: KnownContentType,
		description: &str,
		payload_size: u8,
		depth: u8,
	) -> Result<Self, BuilderCreateError> {
		let key_size = content_type.key_bytes_length();
		let start = database.stream_position()?;
		if key_size == 0 {
			return Err(BuilderCreateError::InvalidKeyLength);
		}
		if (depth / 8) + 1 > key_size {
			// builder code always splits key into (prefix, possibly_partial_byte, suffix)
			// suffix can be empty, but always need at least one byte after the prefix
			// (implementation detail, not needed by design; but expect keys to be long anyway)
			return Err(BuilderCreateError::InvalidKeyLength);
		}
		let depth = Depth::new(depth).ok_or(BuilderCreateError::InvalidKeyLength)?;
		if description.contains('\n') {
			return Err(BuilderCreateError::InvalidDescription {
				description: description.to_string(),
			});
		}
		database.write_all(INDEX_V0_MAGIC.as_bytes())?;
		database.write_all(b"\n")?;
		database.write_all(content_type.name().as_bytes())?;
		database.write_all(b"\n")?;
		database.write_all(description.as_bytes())?;
		database.write_all(b"\n")?;
		database.write_u8(key_size)?;
		database.write_u8(payload_size)?;
		let header_end = database.stream_position()?;
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

pub struct TypedBuilder<D, W, const PAYLOAD_SIZE: usize> {
	builder: Builder<W>,
	_marker: std::marker::PhantomData<D>,
}

impl<D, W, const PAYLOAD_SIZE: usize> TypedBuilder<D, W, PAYLOAD_SIZE>
where
	D: ContentTypeData,
	W: io::Write + io::Seek,
{
	pub fn create(database: W, description: &str, depth: u8) -> Result<Self, BuilderCreateError> {
		assert!(PAYLOAD_SIZE < 0x100);
		Ok(Self {
			builder: Builder::create(
				database,
				D::CONTENT_TYPE,
				description,
				PAYLOAD_SIZE as u8,
				depth,
			)?,
			_marker: std::marker::PhantomData,
		})
	}

	pub fn add_entry(&mut self, key: &D, payload: &[u8; PAYLOAD_SIZE]) -> io::Result<()> {
		self.builder.add_entry(key.as_ref(), payload)
	}

	pub fn finish(self) -> io::Result<()> {
		self.builder.finish()
	}
}

impl<D, W> TypedBuilder<D, W, 0>
where
	D: ContentTypeData,
	<D as std::str::FromStr>::Err: std::error::Error + Sync + Send + 'static,
	W: io::Write + io::Seek,
{
	// https://haveibeenpwned.com/API/v3#PwnedPasswords
	// > The downloadable source data delimits the hash and the password count with a colon (:) and each line with a CRLF.
	// we ignore the password count (empty payload to builder)
	pub fn add_entry_from_hibp_line(&mut self, line: &str) -> anyhow::Result<()> {
		if let Some(colon) = line.find(':') {
			let hash =
				line[..colon].parse::<D>().context("Failed to parse hash from HIBP source line")?;
			self.add_entry(&hash, b"").context("Failed to add hash to index")?;
		} else if !line.is_empty() {
			anyhow::bail!("Invalid HIBP source line: {:?}", line);
		}
		Ok(())
	}
}
