mod key_type;
mod ntlm;
mod sha1;

pub use self::{
	key_type::{KeyType, KnownKeyType},
	ntlm::NTLM,
	sha1::SHA1,
};

/// Both keys (hashes) and payload are stored as raw bytestrings with fixed length
///
/// Provide traits to handle those in a generic way.
///
/// Keys also have a string type name that get serialized and deserialized.

mod seal_trait {
	pub trait U8Array: AsRef<[u8]> + AsMut<[u8]> {
		const SIZE: usize;
	}

	impl<const N: usize> U8Array for [u8; N] {
		const SIZE: usize = N;
	}
}

/// Used to implement `FixedByteArray` due to current limitations in type system.
pub trait FixedByteArrayImpl:
	Default + Clone + AsRef<Self::ByteArray> + AsMut<Self::ByteArray>
{
	type ByteArray: seal_trait::U8Array;
}

/// Types that wrap a byte array of fixed length
///
/// Must be readable and writable as raw u8 array.
// Would be nicer if we could have:
// pub trait FixedByteArray: Default + Clone + AsRef<[u8; Self::SIZE]> + AsMut<[u8; Self::SIZE]> { const SIZE: usize; }
pub trait FixedByteArray: FixedByteArrayImpl {
	const SIZE: usize = <Self::ByteArray as seal_trait::U8Array>::SIZE;

	/// Data of `Self::SIZE` length, but type system can't handle it yet
	fn data(&self) -> &[u8] {
		self.as_ref().as_ref()
	}

	/// Mutable data of `Self::SIZE` length, but type system can't handle it yet
	fn data_mut(&mut self) -> &mut [u8] {
		self.as_mut().as_mut()
	}
}

impl<T: FixedByteArrayImpl> FixedByteArray for T {}

/// Explicitly mark `FixedByteArray` to be used as key (hash).
pub trait KeyData: FixedByteArray {
	const KEY_TYPE: KnownKeyType;
}

/// Explicitly mark `FixedByteArray` to be used as payload.
///
/// When reading a file excessive data will be truncated!
pub trait PayloadData: FixedByteArray {}

/// `PayloadData` type with zero length (and no data)
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default)]
pub struct NoPayload;

impl AsRef<[u8; 0]> for NoPayload {
	fn as_ref(&self) -> &[u8; 0] {
		&[]
	}
}

impl AsMut<[u8; 0]> for NoPayload {
	fn as_mut(&mut self) -> &mut [u8; 0] {
		&mut []
	}
}

impl FixedByteArrayImpl for NoPayload {
	type ByteArray = [u8; 0];
}

impl PayloadData for NoPayload {}
