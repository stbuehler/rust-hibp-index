use byteorder::{LE, WriteBytesExt};
use std::fmt;
use std::ops::{Deref, DerefMut};
use std::str::FromStr;

fn utf16le(data: &str) -> Vec<u8> {
	let mut result = Vec::new();
	for c in data.encode_utf16() {
		result.write_u16::<LE>(c).unwrap();
	}
	result
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct NTLM(pub [u8; 16]);

impl NTLM {
	// NTHash: MD4(UTF-16-LE(password))
	pub fn hash(password: &str) -> Self {
		use md4::Digest;
		let buf = utf16le(password);
		let dig = md4::Md4::digest(&buf);
		let mut this = Self([0u8; 16]);
		this.0.copy_from_slice(&dig);
		this
	}

	pub fn hex(&self) -> impl Deref<Target = str> {
		let mut hex = NTLMHex([0u8; 32]);
		hex::encode_to_slice(&self.0, &mut hex.0).unwrap();
		hex
	}
}

impl FromStr for NTLM {
	type Err = hex::FromHexError;

	fn from_str(hex: &str) -> Result<Self, Self::Err> {
		let mut this = Self([0u8; 16]);
		hex::decode_to_slice(hex, &mut this.0)?;
		Ok(this)
	}
}

impl Deref for NTLM {
	type Target = [u8];

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl DerefMut for NTLM {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.0
	}
}

impl fmt::Debug for NTLM {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.write_str(&self.hex())
	}
}

impl fmt::Display for NTLM {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.write_str(&self.hex())
	}
}

struct NTLMHex([u8; 32]);

impl Deref for NTLMHex {
	type Target = str;

	fn deref(&self) -> &Self::Target {
		std::str::from_utf8(&self.0).unwrap()
	}
}

