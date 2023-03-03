use std::io;

use crate::{
	buf_read::{FileLen, ReadAt},
	data::{KeyData, PayloadData},
	errors::{IndexOpenError, LookupError},
};

use super::{
	reader::{IndexLookup, IndexWalk},
	Index,
};

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
	pub fn new(index: Index<R>) -> Result<Self, IndexOpenError> {
		if index.key_type() != &*D::KEY_TYPE {
			return Err(IndexOpenError::InvalidKeyLength);
		}
		if index.key_size() != D::KEY_TYPE.key_bytes_length() {
			return Err(IndexOpenError::InvalidKeyLength);
		}
		if (index.payload_size() as usize) < P::SIZE {
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
		self.index.description()
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
