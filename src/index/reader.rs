use byteorder::ReadBytesExt;
use std::convert::TryFrom;
use std::io::{self, BufRead, Read, Seek};

use crate::buf_read::{BufReader, FileLen, ReadAt};

use super::table::{ForwardRangeSearch, Prefix, PrefixRange};
use super::{
	table::{ForwardSearch, ForwardSearchResult, Table, TableReadError},
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
		let table = Table::open(reader.by_ref())?;
		if !table.depth().valid_key_size(key_size) {
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
		IndexLookup::new(self, key).sync_lookup(payload)
	}

	pub fn lookup_range<'a>(
		&'a self,
		key: &'a [u8],
		key_bits: u32,
	) -> impl 'a + Iterator<Item = Result<Vec<u8>, LookupError>> {
		let mut walk = IndexWalk::new(self, key, key_bits);
		let mut key_buf = Vec::new();
		key_buf.resize(self.key_size as usize, 0);
		std::iter::from_fn(move || match walk.sync_walk(&mut key_buf) {
			Ok(None) => None,
			Ok(Some(_payload)) => Some(Ok(key_buf.clone())),
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

		let forward_search = index.table.depth().start_forward_search(key);

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
	fn sync_lookup<'a>(
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
	prefixes: PrefixRange,
	payload_buf: Vec<u8>,
	entry_size: usize,
	current_prefix_num_entries: Option<(Prefix, u64)>,
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
	fn sync_walk<'a>(&'a mut self, key: &mut [u8]) -> Result<Option<&'a mut [u8]>, LookupError> {
		assert_eq!(key.len(), self.index.key_size as usize);

		loop {
			if let Some((prefix, mut num_entries)) = self.current_prefix_num_entries.take() {
				while num_entries > 0 {
					self.database.read_exact(key)?;
					self.database.read_exact(&mut self.payload_buf)?;
					num_entries -= 1;
					prefix.fix_entry(key);
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
			// all entries in current prefix done
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
