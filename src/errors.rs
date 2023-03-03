//! Error types

/// Error when parsing key types
#[derive(thiserror::Error, Debug)]
pub enum KeyTypeParseError {
	/// Parsed key type is invalid (probably not ASCII printable)
	#[error("Invalid key type {0:?}")]
	Invalid(String),
}

/// Error when creating a new index
#[derive(thiserror::Error, Debug)]
pub enum BuilderCreateError {
	/// IO write error
	#[error("IO error: {0}")]
	IOError(#[from] std::io::Error),
	/// Invalid description
	#[error("Invalid description: {description:?}")]
	InvalidDescription {
		/// the invalid description
		description: String,
	},
	/// Invalid parameters
	#[error("invalid key / table depth length")]
	InvalidKeyLength,
	/// Parameters too long for header
	#[error("Header too big")]
	HeaderTooBig,
}

/// Error when opening index
#[derive(thiserror::Error, Debug)]
pub enum IndexOpenError {
	/// IO read error
	#[error("IO error: {0}")]
	IOError(#[from] std::io::Error),
	/// Invalid key type
	#[error("key-type error: {0}")]
	KeyTypeError(#[from] KeyTypeParseError),
	/// Invalid error when reading table
	#[error("table read error: {0}")]
	TableReadError(#[from] TableReadError),
	/// Invalid key length
	#[error("invalid key / table depth length")]
	InvalidKeyLength,
	/// Invalid header
	#[error("invalid/unknown header format")]
	InvalidHeader,
}

/// Error when looking up entry in index
#[derive(thiserror::Error, Debug)]
pub enum LookupError {
	/// IO read error
	#[error("IO error: {0}")]
	IOError(#[from] std::io::Error),
	/// Invalid segment length
	#[error("Invalid length of segment containing key (not a multiple of entry size)")]
	InvalidSegmentLength,
}

/// Table read error
///
/// The table is the part of the index that tells us where keys with a given
/// prefix are stored.
#[derive(thiserror::Error, Debug)]
pub enum TableReadError {
	/// IO read error
	#[error("IO error: {0}")]
	IOError(#[from] std::io::Error),
	/// Invalid depth
	#[error("Invalid depth {depth}")]
	InvalidDepth {
		/// the invalid depth value
		depth: u8,
	},
	/// Table larger than it should be
	#[error("Table data too large")]
	TooMuchTableData,
	/// Table offsets decreasing
	#[error("Table offsets not increasing")]
	InvalidTableOffsets,
}
