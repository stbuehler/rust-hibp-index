use crate::data::{FixedByteArray, HexRange, KeyData};

use super::{BucketIndexInner, Depth};

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub(super) struct BucketIndex(BucketIndexInner);

impl BucketIndex {
	pub(super) fn entry(self) -> usize {
		self.0 as usize
	}
}

const KEY_BYTES: usize = std::mem::size_of::<BucketIndexInner>();
const KEY_BITS_U8: u8 = 8 * (KEY_BYTES as u8);

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
struct LimPrefixBytes([u8; KEY_BYTES]);

impl AsRef<[u8; KEY_BYTES]> for LimPrefixBytes {
	fn as_ref(&self) -> &[u8; KEY_BYTES] {
		&self.0
	}
}

impl AsMut<[u8; KEY_BYTES]> for LimPrefixBytes {
	fn as_mut(&mut self) -> &mut [u8; KEY_BYTES] {
		&mut self.0
	}
}

impl crate::data::FixedByteArrayImpl for LimPrefixBytes {
	type ByteArray = [u8; KEY_BYTES];
	type HexArray = [u8; 2 * KEY_BYTES];
}

/// Prefix of key (limited internal hardcoded maximum length [`Depth`])
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LimPrefix {
	raw: LimPrefixBytes,
	depth: Depth,
}

impl LimPrefix {
	pub(super) fn new(depth: Depth, key: &[u8]) -> Self {
		let mut raw = LimPrefixBytes([0u8; KEY_BYTES]);
		if depth.as_u8() == 0 {
			return Self { raw, depth };
		}
		let mask: BucketIndexInner = (!0) << (KEY_BITS_U8 - depth.as_u8()); // zero depth would overflow shift
		let raw_len = std::cmp::min(key.len(), raw.0.len());
		// copy data
		// don't care if key was too short for depth... it just gets zero-padded.
		raw.0[..raw_len].copy_from_slice(&key[..raw_len]);
		// truncate
		let raw = LimPrefixBytes((BucketIndexInner::from_be_bytes(raw.0) & mask).to_be_bytes());
		Self { raw, depth }
	}

	/// Length of prefix
	pub fn depth(self) -> Depth {
		self.depth
	}

	/// Show (significant) nibbles of prefix
	pub fn hex(&self) -> HexRange<[u8; KEY_BYTES * 2]> {
		self.raw.hex_bit_range(0, self.depth.as_u8() as u32)
	}

	pub(super) fn index(self) -> BucketIndex {
		if self.depth.as_u8() == 0 {
			return BucketIndex(0);
		}
		BucketIndex(BucketIndexInner::from_be_bytes(self.raw.0) >> (32 - self.depth.as_u8()))
	}

	/// Set prefix bits in key to this prefix
	///
	/// Useful when recombining prefix and suffix.
	///
	/// Panics if key is too short to contain prefix.
	pub fn set_key_prefix(self, key: &mut [u8]) {
		let full_prefix_bytes = (self.depth.as_u8() / 8) as usize;
		key[..full_prefix_bytes].copy_from_slice(&self.raw.0[..full_prefix_bytes]);
		let partial_bits = self.depth.as_u8() & 0x7;
		if partial_bits != 0 {
			let mask_bits = 0xff >> partial_bits;
			key[full_prefix_bytes] =
				(key[full_prefix_bytes] & mask_bits) | (self.raw.0[full_prefix_bytes] & !mask_bits);
		}
	}

	/// Complete prefix to key from hex encoded suffix
	///
	/// Ignores bits in first suffix nibble that are part of the prefix.
	pub fn read_suffix_from_hex(
		self,
		hex_suffix: &[u8],
		key_data: &mut [u8],
	) -> Result<(), hex::FromHexError> {
		assert!((self.depth.as_u8() as usize).div_ceil(8) < key_data.len(), "prefix too long for key");
		let suffix_start = (self.depth.as_u8() as usize) / 8;
		if self.depth.as_u8() & 0x7 >= 4 {
			// suffix starts with the low nibble of a byte
			if hex_suffix.is_empty() {
				return Err(hex::FromHexError::InvalidStringLength);
			}
			let padded_nibble = [b'0', hex_suffix[0]];
			hex::decode_to_slice(&padded_nibble[..], &mut key_data[suffix_start..][..1])?;
			hex::decode_to_slice(&hex_suffix[1..], &mut key_data[suffix_start + 1..])?;
		} else {
			hex::decode_to_slice(hex_suffix, &mut key_data[suffix_start..])?;
		}
		self.set_key_prefix(key_data);
		Ok(())
	}

	/// Complete prefix to key from hex encoded suffix
	///
	/// Ignores bits in first suffix nibble that are part of the prefix.
	pub fn read_key_from_suffix_hex<D>(self, hex_suffix: &[u8]) -> Result<D, hex::FromHexError>
	where
		D: KeyData,
	{
		let mut key = D::default();
		self.read_suffix_from_hex(hex_suffix, key.data_mut())?;
		Ok(key)
	}
}

impl std::fmt::Debug for LimPrefix {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}/{}", self.hex(), self.depth.as_u8())
	}
}

/// When looking for keys with a certain (limited) prefix, we might need
/// to iterate over multiple prefixes in the table
#[derive(Clone, Copy, Debug)]
pub struct LimPrefixRange {
	// although we use BucketIndexInner as type here, it uses the
	// **unshifted** value, which is why we need to increment by step
	// instead of (constant) 1.
	first: Option<BucketIndexInner>,
	last: BucketIndexInner,
	step: BucketIndexInner,
	depth: Depth,
}

impl LimPrefixRange {
	pub(super) fn new(depth: Depth, key: &[u8], key_bits: u32) -> Self {
		if depth.as_u8() == 0 {
			// step could also be "0" - really doesn't matter.
			return Self { first: Some(0), last: 0, step: 1, depth };
		}
		let mask: BucketIndexInner = (!0) << (KEY_BITS_U8 - depth.as_u8()); // zero depth would overflow shift
		let step: BucketIndexInner = 1 << (KEY_BITS_U8 - depth.as_u8());
		if key_bits == 0 {
			return Self { first: Some(0), last: mask, step, depth };
		}
		let mut raw = [0u8; KEY_BYTES];
		let raw_len = std::cmp::min(key.len(), raw.len());
		// copy data
		raw[..raw_len].copy_from_slice(&key[..raw_len]);
		let ndx = u32::from_be_bytes(raw) & mask;
		if key_bits < depth.as_u8() as u32 {
			// key_bits == KEY_BITS_U8 would overflow shift below, but depth already must be <= KEY_BITS_U8
			debug_assert!(key_bits < KEY_BITS_U8 as u32);
			// find first key_bits bits
			let key_mask: BucketIndexInner = (!0) << (KEY_BITS_U8 - key_bits as u8);
			// truncate to key_bits
			let ndx = ndx & key_mask;
			// set the bits in the prefix that are allowed to be used but are not part
			// of the key prefix to get the last prefix covered by the key
			let last = ndx | (mask & !key_mask);
			LimPrefixRange { first: Some(ndx), last, step, depth }
		} else {
			LimPrefixRange { first: Some(ndx), last: ndx, step, depth }
		}
	}
}

impl Iterator for LimPrefixRange {
	type Item = LimPrefix;

	fn next(&mut self) -> Option<Self::Item> {
		let current = self.first?;
		if current <= self.last {
			// automatically stop on overflow
			self.first = current.checked_add(self.step);
			Some(LimPrefix { raw: LimPrefixBytes(current.to_be_bytes()), depth: self.depth })
		} else {
			None
		}
	}

	fn size_hint(&self) -> (usize, Option<usize>) {
		let l = self.len();
		(l, Some(l))
	}
}

impl DoubleEndedIterator for LimPrefixRange {
	fn next_back(&mut self) -> Option<Self::Item> {
		let first = self.first?;
		if first <= self.last {
			let current = self.last;
			match self.last.checked_sub(self.step) {
				None => {
					self.first = None;
				},
				Some(last) => {
					self.last = last;
				},
			}
			Some(LimPrefix { raw: LimPrefixBytes(current.to_be_bytes()), depth: self.depth })
		} else {
			None
		}
	}
}

impl ExactSizeIterator for LimPrefixRange {
	fn len(&self) -> usize {
		if let Some(first) = self.first {
			1 + ((self.last - first) / self.step) as usize
		} else {
			0
		}
	}
}
