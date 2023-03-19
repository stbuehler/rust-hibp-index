use byteorder::{WriteBytesExt, LE};
use std::fmt;
use std::ops::{Deref, DerefMut};
use std::str::FromStr;

use super::FixedByteArray;

fn utf16le(data: &str) -> Vec<u8> {
	let mut result = Vec::new();
	for c in data.encode_utf16() {
		result.write_u16::<LE>(c).unwrap();
	}
	result
}

/// Storing NT hash
///
/// NT hashes are sometimes called NTLM hashes (not by Microsoft though).
/// The NT hash is the MD4-checksum of the UTF-16LE encoded plaintext.
#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NT(pub [u8; 16]);

impl NT {
	/// Calculate hash of plaintext
	pub fn hash(password: &str) -> Self {
		use md4::Digest;
		let buf = utf16le(password);
		let dig = md4::Md4::digest(buf);
		let mut this = Self([0u8; 16]);
		this.0.copy_from_slice(&dig);
		this
	}
}

impl FromStr for NT {
	type Err = hex::FromHexError;

	fn from_str(hex: &str) -> Result<Self, Self::Err> {
		let mut this = Self([0u8; 16]);
		hex::decode_to_slice(hex, &mut this.0)?;
		Ok(this)
	}
}

impl Deref for NT {
	type Target = [u8];

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl DerefMut for NT {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.0
	}
}

impl AsRef<[u8; 16]> for NT {
	fn as_ref(&self) -> &[u8; 16] {
		&self.0
	}
}

impl AsMut<[u8; 16]> for NT {
	fn as_mut(&mut self) -> &mut [u8; 16] {
		&mut self.0
	}
}

impl fmt::Debug for NT {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.write_str(&self.hex())
	}
}

impl fmt::Display for NT {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.write_str(&self.hex())
	}
}

impl crate::data::FixedByteArrayImpl for NT {
	type ByteArray = [u8; 16];
	type HexArray = [u8; 32];
}

impl crate::data::KeyData for NT {
	const KEY_TYPE: crate::data::KnownKeyType = crate::data::KnownKeyType::NT;
}
