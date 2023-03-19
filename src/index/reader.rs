use byteorder::ReadBytesExt;
use std::convert::TryFrom;
use std::io::{self, BufRead, Read, Seek};

use crate::{
	buf_read::{BufReader, FileLen, ReadAt},
	data::{KeyData, KeyType, PayloadData},
	errors::{IndexOpenError, LookupError},
};

use super::{
	table::Table,
	table_helper::{ForwardRangeSearch, ForwardSearch, ForwardSearchResult},
	LimPrefix, LimPrefixRange,
};

pub const INDEX_V0_MAGIC: &str = "hash-index-v0";
pub const INDEX_V0_HEADER_LIMIT: u64 = 4096;

/// Reader for indexed database
struct Index<R> {
	key_type: KeyType,
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
	/// Open index from reader
	fn open(mut database: R) -> Result<Self, IndexOpenError> {
		let mut reader = io::BufReader::new(&mut database);
		reader.rewind()?;
		let mut header = reader.by_ref().take(INDEX_V0_HEADER_LIMIT);
		let mut magic = String::new();
		let mut key_type = String::new();
		let mut description = String::new();
		header.read_line(&mut magic)?;
		header.read_line(&mut key_type)?;
		header.read_line(&mut description)?;
		if !magic.ends_with('\n') || !key_type.ends_with('\n') || !description.ends_with('\n') {
			return Err(IndexOpenError::InvalidHeader);
		}
		magic.pop();
		key_type.pop();
		description.pop();
		if magic != INDEX_V0_MAGIC {
			return Err(IndexOpenError::InvalidHeader);
		}
		let key_type = KeyType::try_from(key_type)?;
		let key_size = header.read_u8()?;
		let payload_size = header.read_u8()?;
		#[allow(clippy::drop_non_drop)]
		drop(header); // must be done with header parsing here
		let table = Table::open(reader.by_ref())?;
		if !table.depth().valid_key_size(key_size) {
			return Err(IndexOpenError::InvalidKeyLength);
		}
		drop(reader);
		Ok(Self { key_type, description, key_size, payload_size, table, database })
	}
}

/// Typed index reader
///
/// Uses generics to read index with specific key and payload data.
pub struct TypedIndex<D, P, R> {
	index: Index<R>,
	_marker: std::marker::PhantomData<(D, P)>,
}

impl<D, P, R> TypedIndex<D, P, R>
where
	D: KeyData,
	P: PayloadData,
	R: io::Read + io::Seek + ReadAt + FileLen,
{
	/// Try use the passed index with the specified types
	fn new(index: Index<R>) -> Result<Self, IndexOpenError> {
		if index.key_type != *D::KEY_TYPE {
			return Err(IndexOpenError::InvalidKeyLength);
		}
		if index.key_size != D::KEY_TYPE.key_bytes_length() {
			return Err(IndexOpenError::InvalidKeyLength);
		}
		if (index.payload_size as usize) < P::SIZE {
			// TODO: new enum?
			return Err(IndexOpenError::InvalidKeyLength);
		}
		Ok(Self { index, _marker: std::marker::PhantomData })
	}

	/// Open an index database
	pub fn open(database: R) -> Result<Self, IndexOpenError> {
		Self::new(Index::open(database)?)
	}

	/// Description of database
	pub fn description(&self) -> &str {
		&self.index.description
	}

	/// Length (in bytes) of payload data of each entry
	///
	/// Might be larger than supplied PayloadData `P` type.
	pub fn payload_size(&self) -> u8 {
		self.index.payload_size
	}

	/// Lookup entry with given key in index
	///
	/// Return payload of entry if found.
	pub fn lookup(&self, key: &D) -> Result<Option<P>, LookupError> {
		let mut payload = P::default();
		if IndexLookup::new(&self.index, key.data()).sync_lookup(payload.data_mut())?.is_none() {
			return Ok(None);
		}
		Ok(Some(payload))
	}

	/// Loop over all entries with given key prefix.
	///
	/// Iterator returns key and payload for each entry.
	pub fn lookup_range<'a>(
		&'a self,
		key: &'a [u8],
		key_bits: u32,
	) -> impl 'a + Iterator<Item = Result<(D, P), LookupError>> {
		let mut walk = IndexWalk::new(&self.index, key, key_bits);
		let mut key = D::default();
		std::iter::from_fn(move || match walk.sync_walk(key.data_mut()) {
			Ok(None) => None,
			Ok(Some(full_payload)) => {
				let mut payload = P::default();
				payload.data_mut().copy_from_slice(&full_payload[..P::SIZE]);
				Some(Ok((key.clone(), payload)))
			},
			Err(e) => Some(Err(e)),
		})
	}
}

struct IndexLookup<'r, 'key, R> {
	database: BufReader<'r, R>,
	entry_buf: Vec<u8>,
	forward_search: ForwardSearch<'key>,
	num_entries: u64,
	err: Option<LookupError>,
}

impl<'r, 'key, R> IndexLookup<'r, 'key, R>
where
	R: io::Read + io::Seek + ReadAt + FileLen,
{
	fn new(index: &'r Index<R>, key: &'key [u8]) -> Self {
		assert_ne!(index.key_size, 0);
		assert_eq!(key.len(), index.key_size as usize);
		let mut database = BufReader::new(&index.database, 16);

		let forward_search = ForwardSearch::new(index.table.depth(), key);

		let entry_size = index.table.depth().entry_size(index.key_size, index.payload_size);
		let mut entry_buf = Vec::new();
		entry_buf.resize(entry_size, 0u8);

		let std::ops::Range { start, end } = index.table.lookup(key);
		database.seek_from_start(start);

		let length = end - start;
		let num_entries: u64;
		let err: Option<LookupError>;
		if length % entry_size as u64 != 0 {
			num_entries = 0;
			err = Some(LookupError::InvalidSegmentLength);
		} else {
			num_entries = length / entry_size as u64;
			err = None
		}

		Self { database, entry_buf, forward_search, num_entries, err }
	}
}

impl<R> IndexLookup<'_, '_, R>
where
	R: io::Read + io::Seek + ReadAt + FileLen,
{
	pub(super) fn sync_lookup<'a>(
		&mut self,
		payload: &'a mut [u8],
	) -> Result<Option<&'a mut [u8]>, LookupError> {
		if let Some(err) = self.err.take() {
			return Err(err);
		}
		for _ in 0..self.num_entries {
			// read (partial) key with payload in one operation
			self.database.read_exact(&mut self.entry_buf)?;
			match self.forward_search.test_entry(&self.entry_buf) {
				ForwardSearchResult::Match(data) => {
					let p_len = std::cmp::min(payload.len(), data.len());
					let payload = &mut payload[..p_len];
					payload.copy_from_slice(&data[..p_len]);
					return Ok(Some(payload));
				},
				ForwardSearchResult::Continue => (),
				ForwardSearchResult::Break => break,
			}
		}
		Ok(None)
	}
}

struct IndexWalk<'r, 'key, R> {
	index: &'r Index<R>,
	database: BufReader<'r, R>,
	forward_search: ForwardRangeSearch<'key>,
	prefixes: LimPrefixRange,
	payload_buf: Vec<u8>,
	entry_size: usize,
	current_prefix_num_entries: Option<(LimPrefix, u64)>,
}

impl<'r, 'key, R> IndexWalk<'r, 'key, R>
where
	R: io::Read + io::Seek + ReadAt + FileLen,
{
	fn new(index: &'r Index<R>, key: &'key [u8], key_bits: u32) -> Self {
		assert_ne!(index.key_size, 0);

		let database = BufReader::new(&index.database, 16);

		let forward_search = ForwardRangeSearch::new(key, key_bits);
		let prefixes = index.table.prefix_range(key, key_bits);

		let mut payload_buf = Vec::new();
		payload_buf.resize(index.payload_size as usize, 0u8);

		let entry_size = index.table.depth().entry_size(index.key_size, index.payload_size);

		Self {
			index,
			database,
			forward_search,
			prefixes,
			payload_buf,
			entry_size,
			current_prefix_num_entries: None,
		}
	}
}

impl<R> IndexWalk<'_, '_, R>
where
	R: io::Read + io::Seek + ReadAt + FileLen,
{
	pub(super) fn sync_walk<'a>(
		&'a mut self,
		key: &mut [u8],
	) -> Result<Option<&'a mut [u8]>, LookupError> {
		assert_eq!(key.len(), self.index.key_size as usize);

		loop {
			if let Some((prefix, mut num_entries)) = self.current_prefix_num_entries.take() {
				// if all entires are done (num_entries == 0) we just don't write state back;
				// next (outer) loop iteration will load next prefix.
				while num_entries > 0 {
					self.database.read_exact(key)?;
					self.database.read_exact(&mut self.payload_buf)?;
					num_entries -= 1;
					prefix.set_key_prefix(key);
					match self.forward_search.test_key(key) {
						ForwardSearchResult::Match(_) => {
							// remember state
							self.current_prefix_num_entries = Some((prefix, num_entries));
							return Ok(Some(&mut self.payload_buf));
						},
						ForwardSearchResult::Continue => (),
						ForwardSearchResult::Break => return Ok(None),
					}
				}
			} else {
				// currently no prefix active, load next one
				let prefix = match self.prefixes.next() {
					None => return Ok(None),
					Some(prefix) => prefix,
				};
				let std::ops::Range { start, end } = self.index.table.lookup_prefix(prefix);
				self.database.seek_from_start(start);

				let length = end - start;
				if length % self.entry_size as u64 != 0 {
					return Err(LookupError::InvalidSegmentLength);
				}
				let num_entries = length / self.entry_size as u64;
				self.current_prefix_num_entries = Some((prefix, num_entries));
			}
		}
	}
}
