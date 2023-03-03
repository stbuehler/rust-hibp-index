use super::{
	reader::{INDEX_V0_HEADER_LIMIT, INDEX_V0_MAGIC},
	table::TableBuilder,
	Depth,
};
use crate::{
	data::{KeyData, KnownKeyType, NoPayload, PayloadData},
	errors::BuilderCreateError,
};
use anyhow::Context;
use byteorder::WriteBytesExt;
use std::io;

/// Build index in database file
pub struct Builder<W> {
	key_bytes: u8,
	payload_size: u8,
	table: TableBuilder,
	database: W,
}

impl<W> Builder<W>
where
	W: io::Write + io::Seek,
{
	/// Create new builder to write database
	pub fn create(
		mut database: W,
		key_type: KnownKeyType,
		description: &str,
		payload_size: u8,
		depth: Depth,
	) -> Result<Self, BuilderCreateError> {
		let key_bytes = key_type.key_bytes_length();
		let start = database.stream_position()?;
		if !depth.valid_key_size(key_bytes) {
			return Err(BuilderCreateError::InvalidKeyLength);
		}
		if description.contains('\n') {
			return Err(BuilderCreateError::InvalidDescription {
				description: description.to_string(),
			});
		}
		database.write_all(INDEX_V0_MAGIC.as_bytes())?;
		database.write_all(b"\n")?;
		database.write_all(key_type.name().as_bytes())?;
		database.write_all(b"\n")?;
		database.write_all(description.as_bytes())?;
		database.write_all(b"\n")?;
		database.write_u8(key_bytes)?;
		database.write_u8(payload_size)?;
		let header_end = database.stream_position()?;
		let header_size = header_end - start;
		if header_size > INDEX_V0_HEADER_LIMIT {
			return Err(BuilderCreateError::HeaderTooBig);
		}
		let table = TableBuilder::new(depth);
		Ok(Self { key_bytes, payload_size, table, database })
	}

	/// Add entry to database (must be added in order)
	pub fn add_entry(&mut self, key: &[u8], payload: &[u8]) -> io::Result<()> {
		assert_eq!(key.len(), self.key_bytes as usize);
		assert_eq!(payload.len(), self.payload_size as usize);
		self.table.write_key(&mut self.database, key)?;
		self.database.write_all(payload)?;
		Ok(())
	}

	/// Write index table for database
	pub fn finish(mut self) -> io::Result<()> {
		self.table.close(&mut self.database)?;
		Ok(())
	}
}

/// Builder with generic types for fixed-size key and data
pub struct TypedBuilder<D, P, W> {
	builder: Builder<W>,
	_marker: std::marker::PhantomData<(D, P)>,
}

impl<D, P, W> TypedBuilder<D, P, W>
where
	D: KeyData,
	P: PayloadData,
	W: io::Write + io::Seek,
{
	/// Create a new builder
	pub fn create(
		database: W,
		description: &str,
		depth: Depth,
	) -> Result<Self, BuilderCreateError> {
		assert!(P::SIZE < 0x100);
		Ok(Self {
			builder: Builder::create(database, D::KEY_TYPE, description, P::SIZE as u8, depth)?,
			_marker: std::marker::PhantomData,
		})
	}

	/// Add entry to database (must be added in order)
	pub fn add_entry(&mut self, key: &D, payload: &P) -> io::Result<()> {
		self.builder.add_entry(key.data(), payload.data())
	}

	/// Write index table for database
	pub fn finish(self) -> io::Result<()> {
		self.builder.finish()
	}
}

impl<D, W> TypedBuilder<D, NoPayload, W>
where
	D: KeyData + std::str::FromStr,
	<D as std::str::FromStr>::Err: std::error::Error + Sync + Send + 'static,
	W: io::Write + io::Seek,
{
	/// Add entry from HIBP file line
	///
	/// https://haveibeenpwned.com/API/v3#PwnedPasswords
	/// > The downloadable source data delimits the hash and the password count with a colon (:) and each line with a CRLF.
	/// we ignore the password count (empty payload to builder)
	pub fn add_entry_from_hibp_line(&mut self, line: &str) -> anyhow::Result<()> {
		if let Some(colon) = line.find(':') {
			let hash =
				line[..colon].parse::<D>().context("Failed to parse hash from HIBP source line")?;
			self.add_entry(&hash, &NoPayload).context("Failed to add hash to index")?;
		} else if !line.is_empty() {
			anyhow::bail!("Invalid HIBP source line: {:?}", line);
		}
		Ok(())
	}
}
