use std::cmp::Ordering;

/// We often split a key into a prefix (of "depth" bits) and the remaining suffix
///
/// As the split might not happen on byte boundary, the first byte of the suffix
/// might be a "partial" byte; we set unused bits to zero.
/// Assuming we can't mutate the original key, this extra byte can't be referenced
/// from the original key, and is stored locally.
///
/// Therefore we can't return a slice to the full suffix.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct KeySuffix<'key> {
	depth: super::Depth,
	key_first_byte: u8,
	key_suffix: &'key [u8],
}

impl<'key> KeySuffix<'key> {
	pub(super) fn new(depth: super::Depth, key: &'key [u8]) -> Self {
		let strip_key_prefix = depth.as_u8() as usize / 8;
		// don't store unncessary bits; i.e. strip of "depth" bits of key
		// don't shift bits though, so skip full bytes and zero partial bits.
		// returns first (possibly masked byte) and remaining slice of key bytes.
		let suffix = &key[strip_key_prefix..];
		let partial_bits = depth.as_u8() & 0x7;
		let mask_bits = 0xff >> partial_bits;
		Self { depth, key_first_byte: suffix[0] & mask_bits, key_suffix: &suffix[1..] }
	}

	pub(super) fn new_from_entry(depth: super::Depth, entry_key: &'key [u8]) -> Self {
		let partial_bits = depth.as_u8() & 0x7;
		let mask_bits = 0xff >> partial_bits;
		Self { depth, key_first_byte: entry_key[0] & mask_bits, key_suffix: &entry_key[1..] }
	}

	/// First byte of suffix (unused bits of original key cleared)
	pub fn first_byte(&self) -> &[u8] {
		std::slice::from_ref(&self.key_first_byte)
	}

	/// Remaining bytes of suffix (without first byte), reference to original key
	pub fn remaining_bytes(&self) -> &[u8] {
		self.key_suffix
	}

	/// Total length of suffix in bytes (including first possibly partial byte)
	#[allow(clippy::len_without_is_empty)] // len is always > 0 -> never empty
	pub fn len(&self) -> usize {
		self.key_suffix.len() + 1
	}

	/// Store suffix in continuous memory
	pub fn to_vec(&self) -> Vec<u8> {
		let mut buf = Vec::with_capacity(self.len());
		buf.push(self.key_first_byte);
		buf.extend_from_slice(self.key_suffix);
		buf
	}

	/// An entry in the index doesn't include the prefix - i.e. it is a suffix itself.
	///
	/// entry_key is the full slice to such suffix
	///
	/// Panics if the entry_key has a diffent length
	pub fn compare_entry(&self, entry_key: &[u8]) -> Ordering {
		self.partial_cmp(&KeySuffix::new_from_entry(self.depth, entry_key))
			.expect("Invalid entry key length")
	}
}

/*
impl std::fmt::Debug for KeySuffix<'_> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		TODO: (if 20 is the prefix length)
		only show the significant nibbles from the first byte!
		"<*20>suffixnibbles"
		Ok(())
	}
}
*/

impl PartialOrd for KeySuffix<'_> {
	fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
		if self.depth != other.depth {
			return None;
		}
		if self.key_suffix.len() != other.key_suffix.len() {
			return None;
		}
		Some(
			self.key_first_byte
				.cmp(&other.key_first_byte)
				.then_with(|| self.key_suffix.cmp(other.key_suffix)),
		)
	}
}
