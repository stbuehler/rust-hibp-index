use std::borrow::Cow;

use crate::errors::KeyTypeParseError;

#[allow(clippy::upper_case_acronyms)]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[non_exhaustive]
/// Known key type this crate uses to build stuff
pub enum KnownKeyType {
	/// SHA-1 hash data
	SHA1,
	/// NT hash data
	///
	/// -> md4 hash of UTF16-LE encoded data
	NTLM,
}

impl KnownKeyType {
	// for deref to KeyType
	const KT_SHA1: KeyType = KeyType(InnerKeyType::Known(KnownKeyType::SHA1));
	const KT_NTLM: KeyType = KeyType(InnerKeyType::Known(KnownKeyType::NTLM));

	/// Fixed length of key values with our type
	pub fn key_bytes_length(self) -> u8 {
		match self {
			Self::SHA1 => 20,
			Self::NTLM => 16,
		}
	}

	/// Name of key type (used for serialization)
	///
	/// Each character of a name is `u8::is_ascii_graphic` - i.e. name is an ASCII string.
	pub fn name(self) -> &'static str {
		match self {
			Self::SHA1 => "SHA-1",
			Self::NTLM => "NTLM",
		}
	}
}

impl From<KnownKeyType> for KeyType {
	fn from(value: KnownKeyType) -> Self {
		Self(InnerKeyType::Known(value))
	}
}

impl std::ops::Deref for KnownKeyType {
	type Target = KeyType;

	fn deref(&self) -> &Self::Target {
		match self {
			Self::SHA1 => &Self::KT_SHA1,
			Self::NTLM => &Self::KT_NTLM,
		}
	}
}

impl std::cmp::PartialEq<KeyType> for KnownKeyType {
	fn eq(&self, other: &KeyType) -> bool {
		KeyType(InnerKeyType::Known(*self)) == *other
	}
}

// hide underlying enum
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
enum InnerKeyType {
	Known(KnownKeyType),
	Unknown(Cow<'static, str>),
}

/// Key type is an ASCII string; known types are stored as enum.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct KeyType(InnerKeyType);

impl KeyType {
	// easier access to known key types
	/// SHA-1 key type
	pub const SHA1: KnownKeyType = KnownKeyType::SHA1;
	/// NT hash key type
	pub const NTLM: KnownKeyType = KnownKeyType::NTLM;

	fn from_known(input: &str) -> Option<KnownKeyType> {
		match input {
			"SHA-1" => Some(Self::SHA1),
			"NTLM" => Some(Self::NTLM),
			_ => None,
		}
	}

	/// Return known key type - if type is known.
	pub fn as_known(&self) -> Option<KnownKeyType> {
		match self.0 {
			InnerKeyType::Known(k) => Some(k),
			InnerKeyType::Unknown(_) => None,
		}
	}

	fn check(input: &str) -> Result<(), KeyTypeParseError> {
		if input.as_bytes().iter().all(u8::is_ascii_graphic) {
			Ok(())
		} else {
			Err(KeyTypeParseError::Invalid(input.to_string()))
		}
	}

	/// Similar to `FromStr` but parses string with static lifetime
	pub fn from_static(key_type: &'static str) -> Result<Self, KeyTypeParseError> {
		if let Some(kt) = Self::from_known(key_type) {
			return Ok(kt.into());
		}
		Self::check(key_type)?;
		Ok(Self(InnerKeyType::Unknown(Cow::Borrowed(key_type))))
	}

	/// ASCII name of key type
	pub fn name(&self) -> &str {
		match &self.0 {
			InnerKeyType::Known(k) => k.name(),
			InnerKeyType::Unknown(name) => name,
		}
	}
}

impl std::str::FromStr for KeyType {
	type Err = KeyTypeParseError;

	fn from_str(key_type: &str) -> Result<Self, Self::Err> {
		if let Some(kt) = Self::from_known(key_type) {
			return Ok(kt.into());
		}
		Self::check(key_type)?;
		Ok(Self(InnerKeyType::Unknown(Cow::Owned(key_type.to_string()))))
	}
}

impl std::convert::TryFrom<&'static str> for KeyType {
	type Error = KeyTypeParseError;

	fn try_from(key_type: &'static str) -> Result<Self, Self::Error> {
		Self::from_static(key_type)
	}
}

impl std::convert::TryFrom<String> for KeyType {
	type Error = KeyTypeParseError;

	fn try_from(key_type: String) -> Result<Self, Self::Error> {
		if let Some(kt) = Self::from_known(&key_type) {
			return Ok(kt.into());
		}
		Self::check(&key_type)?;
		Ok(Self(InnerKeyType::Unknown(Cow::Owned(key_type))))
	}
}
