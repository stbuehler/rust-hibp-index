//! Various types representing "data" (keys, payload, related)
#[macro_use]
mod macros;

mod hex;
mod key_type;
mod ntlm;
mod prefix;
mod sha1;

pub use self::{
	key_type::{KeyType, KnownKeyType},
	ntlm::NTLM,
	prefix::{Prefix, Suffix},
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
///
/// While those type could easily be `Copy` too we might want to be careful copying
/// them around implicitly - so only ask for `Clone`.
pub trait FixedByteArrayImpl:
	Default + Clone + AsRef<Self::ByteArray> + AsMut<Self::ByteArray>
{
	/// underlying [u8; N] type
	type ByteArray: seal_trait::U8Array;
}

/// Types that wrap a byte array of fixed length
///
/// Must be readable and writable as raw u8 array.
// Would be nicer if we could have:
// pub trait FixedByteArray: Default + Clone + AsRef<[u8; Self::SIZE]> + AsMut<[u8; Self::SIZE]> { const SIZE: usize; }
pub trait FixedByteArray: FixedByteArrayImpl {
	/// Length of underlying byte arry
	const SIZE: usize = <Self::ByteArray as seal_trait::U8Array>::SIZE;

	/// Data of `Self::SIZE` length, but type system can't handle it yet
	fn data(&self) -> &[u8] {
		self.as_ref().as_ref()
	}

	/// Mutable data of `Self::SIZE` length, but type system can't handle it yet
	fn data_mut(&mut self) -> &mut [u8] {
		self.as_mut().as_mut()
	}

	/// Returns an `impl std::fmt::Display` showing the hex digits of the data
	fn hex(&self) -> hex::DisplayHex<'_> {
		hex::DisplayHex::new(self.data(), 0, Self::SIZE as u32 * 8)
	}

	/// Returns an `impl std::fmt::Display` showing the hex digits of the data in the given bit range
	///
	/// Shows all hex digits that contain at least one bit to be shown (but doesn't mask the other bits;
	/// that might change though).
	fn hex_bit_range(&self, start: u32, end: u32) -> hex::DisplayHex<'_> {
		assert!(start <= end);
		assert!(end <= Self::SIZE as u32 * 8);
		hex::DisplayHex::new(self.data(), start, end)
	}
}

impl<T: FixedByteArrayImpl> FixedByteArray for T {}

/// Explicitly mark `FixedByteArray` to be used as key (hash).
pub trait KeyData: FixedByteArray {
	/// Key type
	///
	/// For types that store hash data for a certain hash we really should know the key type.
	const KEY_TYPE: KnownKeyType;

	/// Build prefix with given number of bits
	fn prefix(&self, bits: u32) -> Prefix<Self> {
		Prefix::<Self>::new_from_key(self, bits)
	}

	/// Build suffix after stripping prefix with given number of bits
	fn suffix(&self, bits: u32) -> Suffix<Self> {
		Suffix::<Self>::new_from_key(self, bits)
	}

	/// Split data into prefix (with given number of bits) and suffix
	fn split(&self, bits: u32) -> (Prefix<Self>, Suffix<Self>) {
		(self.prefix(bits), self.suffix(bits))
	}
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
