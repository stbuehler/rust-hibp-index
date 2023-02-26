use std::io;

use crate::buf_read::{FileLen, ReadAt};

use super::{
	reader::{IndexLookup, IndexWalk},
	ContentTypeData, Index, IndexOpenError, LookupError, PayloadData, PayloadDataExt,
};

pub struct TypedIndex<D, P, R> {
	index: Index<R>,
	_marker: std::marker::PhantomData<(D, P)>,
}

impl<D, P, R> TypedIndex<D, P, R>
where
	D: ContentTypeData,
	P: PayloadData,
	R: io::Read + io::Seek + ReadAt + FileLen,
{
	pub fn new(index: Index<R>) -> Result<Self, IndexOpenError> {
		if index.content_type() != &*D::CONTENT_TYPE {
			return Err(IndexOpenError::InvalidKeyLength);
		}
		if index.key_size() != D::CONTENT_TYPE.key_bytes_length() {
			return Err(IndexOpenError::InvalidKeyLength);
		}
		if (index.payload_size() as usize) < P::SIZE {
			// TODO: new enum?
			return Err(IndexOpenError::InvalidKeyLength);
		}
		Ok(Self { index, _marker: std::marker::PhantomData })
	}

	pub fn open(database: R) -> Result<Self, IndexOpenError> {
		Self::new(Index::open(database)?)
	}

	pub fn description(&self) -> &str {
		self.index.description()
	}

	pub fn lookup(&self, key: &D) -> Result<Option<P>, LookupError> {
		let mut payload = P::default();
		if IndexLookup::new(&self.index, key.as_ref()).sync_lookup(payload.data_mut())?.is_none() {
			return Ok(None);
		}
		Ok(Some(payload))
	}

	pub fn lookup_range<'a>(
		&'a self,
		key: &'a [u8],
		key_bits: u32,
	) -> impl 'a + Iterator<Item = Result<(D, P), LookupError>> {
		let mut walk = IndexWalk::new(&self.index, key, key_bits);
		let mut key = D::default();
		std::iter::from_fn(move || match walk.sync_walk(key.as_mut()) {
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
