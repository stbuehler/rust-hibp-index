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

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Prefix {
	raw: [u8; KEY_BYTES],
	depth: Depth,
}

impl Prefix {
	pub(super) fn new(depth: Depth, key: &[u8]) -> Self {
		let mut raw = [0u8; KEY_BYTES];
		if depth.as_u8() == 0 {
			return Prefix { raw, depth };
		}
		let mask: BucketIndexInner = (!0) << (KEY_BITS_U8 - depth.as_u8()); // zero depth would overflow shift
		let raw_len = std::cmp::min(key.len(), raw.len());
		// copy data
		// don't care if key was too short for depth... it just gets zero-padded.
		raw[..raw_len].copy_from_slice(&key[..raw_len]);
		// truncate
		let raw = (BucketIndexInner::from_be_bytes(raw) & mask).to_be_bytes();
		Prefix { raw, depth }
	}

	/// Length of prefix
	pub fn depth(self) -> Depth {
		self.depth
	}

	/// Show (significant) nibbles of prefix
	pub fn as_hex(
		&self,
	) -> impl std::ops::Deref<Target = str> + std::fmt::Debug + std::fmt::Display {
		let mut storage = [0u8; 2 * KEY_BYTES];
		hex::encode_to_slice(self.raw, &mut storage).unwrap();
		let len = (self.depth.as_u8() as usize + 3) / 4;
		PrefixHex { storage, len }
	}

	pub(super) fn index(self) -> BucketIndex {
		if self.depth.as_u8() == 0 {
			return BucketIndex(0);
		}
		BucketIndex(BucketIndexInner::from_be_bytes(self.raw) >> (32 - self.depth.as_u8()))
	}

	/// Set prefix bits in key to this prefix
	///
	/// Useful when recombining prefix and suffix.
	///
	/// Panics if key is too short to contain prefix.
	pub fn set_key_prefix(self, key: &mut [u8]) {
		let full_prefix_bytes = (self.depth.as_u8() / 8) as usize;
		key[..full_prefix_bytes].copy_from_slice(&self.raw[..full_prefix_bytes]);
		let partial_bits = self.depth.as_u8() & 0x7;
		if partial_bits != 0 {
			let mask_bits = 0xff >> partial_bits;
			key[full_prefix_bytes] =
				(key[full_prefix_bytes] & mask_bits) | (self.raw[full_prefix_bytes] & !mask_bits);
		}
	}
}

impl std::fmt::Debug for Prefix {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}/{}", self.as_hex(), self.depth.as_u8())
	}
}

struct PrefixHex {
	storage: [u8; 2 * KEY_BYTES],
	len: usize,
}

impl PrefixHex {
	fn str(&self) -> &str {
		std::str::from_utf8(&self.storage[..self.len]).unwrap()
	}
}

impl std::ops::Deref for PrefixHex {
	type Target = str;

	fn deref(&self) -> &Self::Target {
		self.str()
	}
}

impl std::fmt::Debug for PrefixHex {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{:?}", self.str())
	}
}

impl std::fmt::Display for PrefixHex {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_str(self.str())
	}
}

/// When looking for keys with a certain prefix, we might need
/// to iterate over multiple prefixes in the table
#[derive(Clone, Copy, Debug)]
pub struct PrefixRange {
	// although we use BucketIndexInner as type here, it uses the
	// **unshifted** value, which is why we need to increment by step
	// instead of (constant) 1.
	first: Option<BucketIndexInner>,
	last: BucketIndexInner,
	step: BucketIndexInner,
	depth: Depth,
}

impl PrefixRange {
	pub(super) fn new(depth: Depth, key: &[u8], key_bits: u32) -> PrefixRange {
		if depth.as_u8() == 0 {
			// step could also be "0" - really doesn't matter.
			return PrefixRange { first: Some(0), last: 0, step: 1, depth };
		}
		let mask: BucketIndexInner = (!0) << (KEY_BITS_U8 - depth.as_u8()); // zero depth would overflow shift
		let step: BucketIndexInner = 1 << (KEY_BITS_U8 - depth.as_u8());
		if key_bits == 0 {
			return PrefixRange { first: Some(0), last: mask, step, depth };
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
			PrefixRange { first: Some(ndx), last, step, depth }
		} else {
			PrefixRange { first: Some(ndx), last: ndx, step, depth }
		}
	}
}

impl Iterator for PrefixRange {
	type Item = Prefix;

	fn next(&mut self) -> Option<Self::Item> {
		let current = self.first?;
		if current <= self.last {
			// automatically stop on overflow
			self.first = current.checked_add(self.step);
			Some(Prefix { raw: current.to_be_bytes(), depth: self.depth })
		} else {
			None
		}
	}

	fn size_hint(&self) -> (usize, Option<usize>) {
		let l = self.len();
		(l, Some(l))
	}
}

impl DoubleEndedIterator for PrefixRange {
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
			Some(Prefix { raw: current.to_be_bytes(), depth: self.depth })
		} else {
			None
		}
	}
}

impl ExactSizeIterator for PrefixRange {
	fn len(&self) -> usize {
		if let Some(first) = self.first {
			1 + ((self.last - first) / self.step) as usize
		} else {
			0
		}
	}
}
