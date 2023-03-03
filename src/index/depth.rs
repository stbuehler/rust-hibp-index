use super::{BucketIndex, Prefix, PrefixRange};

/// The depth of a index determines how long the prefix is long (in bits)
///
/// A key in the index is split into a prefix and a suffix for storage; the
/// prefix is used to determine the part of the index entries with the prefix
/// are located in; the entries then don't store the prefix anymore, only
/// the suffix.
///
/// Internally the depth is an `u8`, but the actual range is much more limited,
/// as it influences how big the index table is: for each prefix it stores a
/// file offset (`u64`); this results in the following example table sizes:
/// * 16 bit depth: 512 KB table
/// * 20 bit depth:   8 MB table
/// * 24 bit depth: 128 MB table.
///
/// The current (hardcoded) maximum depth is 24.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Depth(u8);

/// must contain `TABLE_MAX_DEPTH` bits
pub(super) type BucketIndexInner = u32;

impl Depth {
	const KEY_BYTES: usize = std::mem::size_of::<BucketIndexInner>();
	const KEY_BITS: usize = 8 * Self::KEY_BYTES;

	// * must be less than bit width of BucketIndex!
	// * the table size should fit in an u32 (usize on 32-bit platforms),
	//   which is (1 << TABLE_MAX_DEPTH) + 1, i.e. must be LESS THAN 32.
	//   (also important for PrefixRange::len)
	// * also should be a sane limit for memory ussage (see above).
	const TABLE_MAX_DEPTH: u8 = 24;

	// these obviously should obey the above limit (unwrap/expect not const yet).
	/// Depth of 20 bits (always valid)
	pub const DEPTH20: Self = Self(20);
	/// Depth of 16 bits (always valid)
	pub const DEPTH16: Self = Self(16);

	/// Create a new depth; returns `None` if depth is too large.
	pub fn new(depth: u8) -> Option<Self> {
		// static assert?
		// need to store index past the end of depth bits, i.e. at least one bit more:
		assert!((Self::TABLE_MAX_DEPTH as usize) < Self::KEY_BITS);

		if depth > Self::TABLE_MAX_DEPTH {
			return None;
		}
		Some(Self(depth))
	}

	/// Depth in bits
	pub fn as_u8(self) -> u8 {
		self.0
	}

	pub(super) fn valid_key_size(&self, key_bytes: u8) -> bool {
		if key_bytes == 0 {
			return false;
		}
		if (self.0 / 8) + 1 > key_bytes {
			// expect at least one byte after prefix
			// implementation depends on this (design does not, but we expect long keys anyway)
			return false;
		}
		true
	}

	pub(super) fn table_entries(self) -> usize {
		(1 << self.0) + 1
	}

	pub(super) fn entry_size(self, key_size: u8, payload_size: u8) -> usize {
		let strip_key_prefix = self.0 as usize / 8;
		(key_size as usize) - strip_key_prefix + (payload_size as usize)
	}

	/// Extract prefix from a key
	pub fn prefix(self, key: &[u8]) -> Prefix {
		Prefix::new(self, key)
	}

	pub(super) fn index(self, key: &[u8]) -> BucketIndex {
		self.prefix(key).index()
	}

	pub(super) fn prepare_key(self, key: &[u8]) -> super::KeySuffix {
		super::KeySuffix::new(self, key)
	}

	/// Extract prefix range from key prefix to look for
	pub fn prefix_range(self, key: &[u8], key_bits: u32) -> PrefixRange {
		PrefixRange::new(self, key, key_bits)
	}
}
