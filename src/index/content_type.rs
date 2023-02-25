use std::borrow::Cow;

#[allow(clippy::upper_case_acronyms)]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[non_exhaustive]
/// Known content type this crate uses to build stuff
pub enum KnownContentType {
	SHA1,
	NTLM,
}

impl KnownContentType {
	// for deref to ContentType
	const CT_SHA1: ContentType = ContentType(InnerContentType::Known(KnownContentType::SHA1));
	const CT_NTLM: ContentType = ContentType(InnerContentType::Known(KnownContentType::NTLM));

	pub fn key_bytes_length(self) -> u8 {
		match self {
			Self::SHA1 => 20,
			Self::NTLM => 16,
		}
	}

	pub fn as_content_type(self) -> ContentType {
		ContentType(InnerContentType::Known(self))
	}

	pub fn name(self) -> &'static str {
		match self {
			Self::SHA1 => "SHA-1",
			Self::NTLM => "NTLM",
		}
	}
}

impl std::ops::Deref for KnownContentType {
	type Target = ContentType;

	fn deref(&self) -> &Self::Target {
		match self {
			Self::SHA1 => &Self::CT_SHA1,
			Self::NTLM => &Self::CT_NTLM,
		}
	}
}

pub trait ContentTypeData:
	AsRef<[u8]> + AsMut<[u8]> + std::fmt::Display + std::str::FromStr
{
	const CONTENT_TYPE: KnownContentType;
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
enum InnerContentType {
	Known(KnownContentType),
	Unknown(Cow<'static, str>),
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct ContentType(InnerContentType);

impl ContentType {
	// easier access to known content types
	pub const SHA1: KnownContentType = KnownContentType::SHA1;
	pub const NTLM: KnownContentType = KnownContentType::NTLM;

	fn from_known(input: &str) -> Option<KnownContentType> {
		match input {
			"SHA-1" => Some(Self::SHA1),
			"NTLM" => Some(Self::NTLM),
			_ => None,
		}
	}

	pub fn as_known(&self) -> Option<KnownContentType> {
		if let InnerContentType::Known(k) = self.0 {
			Some(k)
		} else {
			None
		}
	}

	fn check(input: &str) -> Result<(), ContentTypeParseError> {
		if input.as_bytes().iter().all(u8::is_ascii_graphic) {
			Ok(())
		} else {
			Err(ContentTypeParseError::Invalid(input.to_string()))
		}
	}

	pub fn from_static(content_type: &'static str) -> Result<Self, ContentTypeParseError> {
		if let Some(ct) = Self::from_known(content_type) {
			return Ok(ct.as_content_type());
		}
		Self::check(content_type)?;
		Ok(Self(InnerContentType::Unknown(Cow::Borrowed(content_type))))
	}

	pub fn name(&self) -> &str {
		match &self.0 {
			InnerContentType::Known(k) => k.name(),
			InnerContentType::Unknown(name) => name,
		}
	}
}

#[derive(thiserror::Error, Debug)]
pub enum ContentTypeParseError {
	#[error("Invalid content type {0:?}")]
	Invalid(String),
}

impl std::str::FromStr for ContentType {
	type Err = ContentTypeParseError;

	fn from_str(content_type: &str) -> Result<Self, Self::Err> {
		if let Some(ct) = Self::from_known(content_type) {
			return Ok(ct.as_content_type());
		}
		Self::check(content_type)?;
		Ok(Self(InnerContentType::Unknown(Cow::Owned(content_type.to_string()))))
	}
}

impl std::convert::TryFrom<&'static str> for ContentType {
	type Error = ContentTypeParseError;

	fn try_from(content_type: &'static str) -> Result<Self, Self::Error> {
		Self::from_static(content_type)
	}
}

impl std::convert::TryFrom<String> for ContentType {
	type Error = ContentTypeParseError;

	fn try_from(content_type: String) -> Result<Self, Self::Error> {
		if let Some(ct) = Self::from_known(&content_type) {
			return Ok(ct.as_content_type());
		}
		Self::check(&content_type)?;
		Ok(Self(InnerContentType::Unknown(Cow::Owned(content_type))))
	}
}
