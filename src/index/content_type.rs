use std::borrow::Cow;

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct ContentType(Cow<'static, str>);

impl ContentType {
	pub const SHA1: Self = Self(Cow::Borrowed("SHA-1"));
	pub const NTLM: Self = Self(Cow::Borrowed("NTLM"));

	fn from_known(input: &str) -> Option<Self> {
		match input {
			"SHA-1" => Some(Self::SHA1),
			"NTLM" => Some(Self::NTLM),
			_ => None,
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
			return Ok(ct);
		}
		Self::check(content_type)?;
		Ok(Self(Cow::Borrowed(content_type)))
	}
}

impl std::ops::Deref for ContentType {
	type Target = str;

	fn deref(&self) -> &Self::Target {
		&self.0
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
			return Ok(ct);
		}
		Self::check(content_type)?;
		Ok(Self(Cow::Owned(content_type.to_string())))
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
			return Ok(ct);
		}
		Self::check(&content_type)?;
		Ok(Self(Cow::Owned(content_type)))
	}
}
