use crate::{
	data::{KeyData, KeyType, PayloadData, Prefix, Suffix},
	errors::{HashListCreateError, HashListOpenError},
};
use byteorder::{ReadBytesExt, WriteBytesExt};
use chrono::TimeZone;
use std::{
	cmp::Ordering,
	io::{BufRead, ErrorKind},
};

const HASH_LIST_V0_MAGIC: &str = "hash-list-v0";
const HASH_LIST_V0_HEADER_LIMIT: u64 = 4096;

struct Header {
	key_type: KeyType,
	description: String,
	mtime: chrono::DateTime<chrono::Utc>,
	key_size: u8,
	payload_size: u8,
}

impl Header {
	fn open<R>(reader: R) -> Result<Self, HashListOpenError>
	where
		R: BufRead,
	{
		let mut reader = std::io::Read::take(reader, HASH_LIST_V0_HEADER_LIMIT);
		let mut magic = String::new();
		let mut key_type = String::new();
		let mut description = String::new();
		reader.read_line(&mut magic)?;
		reader.read_line(&mut key_type)?;
		reader.read_line(&mut description)?;
		if !magic.ends_with('\n') || !key_type.ends_with('\n') || !description.ends_with('\n') {
			return Err(HashListOpenError::InvalidHeader);
		}
		magic.pop();
		key_type.pop();
		description.pop();
		if magic != HASH_LIST_V0_MAGIC {
			return Err(HashListOpenError::InvalidHeader);
		}
		let key_type = KeyType::try_from(key_type)?;
		let mtime_epoch = reader.read_i64::<byteorder::BE>()?;
		let mtime = chrono::Utc
			.timestamp_opt(mtime_epoch, 0)
			.single()
			.ok_or(HashListOpenError::InvalidMtime)?;
		let key_size = reader.read_u8()?;
		let payload_size = reader.read_u8()?;
		Ok(Header { key_type, description, mtime, key_size, payload_size })
	}

	fn create<W>(
		mut writer: W,
		description: &str,
		key_type: KeyType,
		key_size: u8,
		payload_size: u8,
		mtime: chrono::DateTime<chrono::Utc>,
	) -> Result<(), HashListCreateError>
	where
		W: std::io::Write,
	{
		writer.write_all(HASH_LIST_V0_MAGIC.as_bytes())?;
		writer.write_all(b"\n")?;
		if description.lines().take(2).count() > 1 {
			return Err(HashListCreateError::InvalidDescription);
		}
		writer.write_all(key_type.name().as_bytes())?;
		writer.write_all(b"\n")?;
		writer.write_all(description.as_bytes())?;
		writer.write_all(b"\n")?;
		writer.write_i64::<byteorder::BE>(mtime.timestamp())?;
		writer.write_u8(key_size)?;
		writer.write_u8(payload_size)?;
		Ok(())
	}
}

/// Typed list reader of keys (hashes) and payload entries with fixed prefix
pub struct TypedListReader<K, P, R> {
	reader: R,
	header: Header,
	prefix: Prefix<K>,
	payload_buf: Vec<u8>,
	_marker: std::marker::PhantomData<P>,
}

impl<K, P, R> TypedListReader<K, P, R>
where
	K: KeyData,
	P: PayloadData,
	R: BufRead,
{
	/// Prefix of hash list
	pub fn prefix(&self) -> &Prefix<K> {
		&self.prefix
	}

	/// Description of list
	pub fn description(&self) -> &str {
		&self.header.description
	}

	/// Last-Modified timestamp of list (header field, not file metadata)
	pub fn mtime(&self) -> chrono::DateTime<chrono::Utc> {
		self.header.mtime
	}

	/// Open hash list
	pub fn open(mut reader: R) -> Result<Self, HashListOpenError> {
		let header = Header::open(&mut reader)?;
		if header.key_type.as_known() != Some(K::KEY_TYPE) {
			return Err(HashListOpenError::InvalidKeyLength);
		}
		if header.key_size as usize != K::SIZE {
			return Err(HashListOpenError::InvalidKeyLength);
		}
		if (header.payload_size as usize) < P::SIZE {
			return Err(HashListOpenError::InvalidKeyLength);
		}
		// header followed by prefix, then entries
		let prefix_len = reader.read_u8()?;
		let prefix_len_bytes = (prefix_len as usize).div_ceil(8);
		if prefix_len_bytes >= K::SIZE {
			return Err(HashListOpenError::InvalidKeyLength);
		}
		let mut key = K::default();
		reader.read_exact(&mut key.data_mut()[..prefix_len_bytes])?;
		let prefix = key.prefix(prefix_len as u32);
		let payload_buf = vec![0; header.payload_size as usize];
		Ok(Self { reader, header, prefix, payload_buf, _marker: std::marker::PhantomData })
	}

	/// Read next entry from hash list
	pub fn next_entry(&mut self) -> Option<std::io::Result<(K, P)>> {
		let mut key = K::default();
		let key_data = key.data_mut();
		let skip = self.prefix.bits() as usize / 8;
		if let Err(e) = self.reader.read_exact(&mut key_data[skip..]) {
			match e.kind() {
				// TODO: technically we should also return an error if we read a partial key...
				ErrorKind::UnexpectedEof => return None,
				_ => return Some(Err(e)),
			}
		}
		let suffix = Suffix::new_from_key(&key, self.prefix.bits());
		let key = self.prefix.unsplit(suffix);
		if let Err(e) = self.reader.read_exact(&mut self.payload_buf) {
			return Some(Err(e));
		}
		let mut payload = P::default();
		payload.data_mut().copy_from_slice(&self.payload_buf[..P::SIZE]);
		Some(Ok((key, payload)))
	}

	/// Search key in list
	pub fn lookup(&mut self, key: &K) -> std::io::Result<Option<P>> {
		while let Some(entry) = self.next_entry() {
			let (entry_key, payload) = entry?;
			match entry_key.data().cmp(key.data()) {
				Ordering::Less => (),
				Ordering::Equal => return Ok(Some(payload)),
				Ordering::Greater => return Ok(None),
			}
		}
		Ok(None)
	}
}

/// Typed list writer of keys (hashes) and payload entries with fixed prefix
pub struct TypedListWriter<K, P, W> {
	writer: W,
	prefix: Prefix<K>,
	_marker: std::marker::PhantomData<P>,
}

impl<K, P, W> TypedListWriter<K, P, W>
where
	K: KeyData,
	P: PayloadData,
	W: std::io::Write,
{
	/// Create new hash list file
	pub fn create(
		mut writer: W,
		description: &str,
		mtime: chrono::DateTime<chrono::Utc>,
		prefix: Prefix<K>,
	) -> Result<Self, HashListCreateError> {
		assert!(K::SIZE < 256);
		assert!(P::SIZE < 256);
		Header::create(
			&mut writer,
			description,
			K::KEY_TYPE.into(),
			K::SIZE as u8,
			P::SIZE as u8,
			mtime,
		)?;
		writer.write_u8(prefix.bits() as u8)?;
		let prefix_byte_count = (prefix.bits() as usize).div_ceil(8);
		writer.write_all(&prefix.key().data()[..prefix_byte_count])?;
		Ok(Self { writer, prefix, _marker: std::marker::PhantomData })
	}

	/// Add entry (should be ordered)
	pub fn add(&mut self, key: &K, payload: &P) -> std::io::Result<()> {
		let suffix = Suffix::new_from_key(key, self.prefix.bits());
		let suffix_start = self.prefix.bits() as usize / 8;
		self.writer.write_all(&suffix.key().data()[suffix_start..])?;
		self.writer.write_all(payload.data())?;
		Ok(())
	}
}
