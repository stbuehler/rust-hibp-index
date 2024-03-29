use std::fmt;
use std::ops::{Deref, DerefMut};
use std::str::FromStr;

use super::FixedByteArray;

/// SHA-1 hash data
#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SHA1(pub [u8; 20]);

impl SHA1 {
	/// Calculate SHA-1 hash of plaintext data
	pub fn hash(data: &[u8]) -> Self {
		use sha1::Digest;
		let dig = sha1::Sha1::digest(data);
		let mut this = Self([0u8; 20]);
		this.0.copy_from_slice(&dig);
		this
	}
}

impl FromStr for SHA1 {
	type Err = hex::FromHexError;

	fn from_str(hex: &str) -> Result<Self, Self::Err> {
		let mut this = Self([0u8; 20]);
		hex::decode_to_slice(hex, &mut this.0)?;
		Ok(this)
	}
}

impl Deref for SHA1 {
	type Target = [u8];

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl DerefMut for SHA1 {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.0
	}
}

impl AsRef<[u8; 20]> for SHA1 {
	fn as_ref(&self) -> &[u8; 20] {
		&self.0
	}
}

impl AsMut<[u8; 20]> for SHA1 {
	fn as_mut(&mut self) -> &mut [u8; 20] {
		&mut self.0
	}
}

impl fmt::Debug for SHA1 {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.write_str(&self.hex())
	}
}

impl fmt::Display for SHA1 {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.write_str(&self.hex())
	}
}

impl crate::data::FixedByteArrayImpl for SHA1 {
	type ByteArray = [u8; 20];
	type HexArray = [u8; 40];
}

impl crate::data::KeyData for SHA1 {
	const KEY_TYPE: crate::data::KnownKeyType = crate::data::KnownKeyType::SHA1;
}
