use std::fmt;
use std::ops::{Deref, DerefMut};
use std::str::FromStr;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SHA1(pub [u8; 20]);

impl SHA1 {
	pub fn hash(data: &[u8]) -> Self {
		let mut this = Self([0u8; 20]);
		let hash = ring::digest::digest(&ring::digest::SHA1_FOR_LEGACY_USE_ONLY, data);
		this.0.copy_from_slice(hash.as_ref());
		this
	}

	pub fn hex(&self) -> impl Deref<Target = str> {
		let mut hex = SHA1Hex([0u8; 40]);
		hex::encode_to_slice(&self.0, &mut hex.0).unwrap();
		hex
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

struct SHA1Hex([u8; 40]);

impl Deref for SHA1Hex {
	type Target = str;

	fn deref(&self) -> &Self::Target {
		std::str::from_utf8(&self.0).unwrap()
	}
}
