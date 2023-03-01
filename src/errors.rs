#[derive(thiserror::Error, Debug)]
pub enum KeyTypeParseError {
	#[error("Invalid key type {0:?}")]
	Invalid(String),
}
