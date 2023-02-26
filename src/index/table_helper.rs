use std::cmp::Ordering;

use super::Depth;

pub(super) enum ForwardSearchResult<'data> {
	Match(&'data [u8]),
	Continue,
	Break,
}

pub(super) struct ForwardSearch<'key> {
	suffix: super::KeySuffix<'key>,
	key_len: usize,
}

impl<'key> ForwardSearch<'key> {
	pub(super) fn new(depth: Depth, key: &'key [u8]) -> Self {
		let suffix = depth.prepare_key(key);
		Self { suffix, key_len: key.len() }
	}

	pub(super) fn test_entry<'data>(&self, entry: &'data [u8]) -> ForwardSearchResult<'data> {
		let (entry_key, entry_payload) = entry.split_at(self.key_len);
		match self.suffix.compare_entry(entry_key) {
			Ordering::Equal => ForwardSearchResult::Match(entry_payload),
			// key we're looking for still greater than entry from file
			Ordering::Greater => ForwardSearchResult::Continue,
			// now entry in file greater than our search key - only getting worse now.
			Ordering::Less => ForwardSearchResult::Break,
		}
	}
}

pub(super) struct ForwardRangeSearch<'key> {
	prefix: &'key [u8],
	prefix_end: u8,
	mask_end_bits: u8,
}

impl<'key> ForwardRangeSearch<'key> {
	pub(super) fn new(key: &'key [u8], key_bits: u32) -> Self {
		if key_bits == 0 {
			return Self { prefix: b"", prefix_end: 0, mask_end_bits: 0 };
		}
		let len_clipped = (key_bits / 8) as usize;
		let partial_bits = key_bits & 0x7;
		if partial_bits > 0 {
			let mask_end_bits = !(0xff >> partial_bits);
			Self {
				prefix: &key[..len_clipped],
				prefix_end: key[len_clipped] & mask_end_bits,
				mask_end_bits,
			}
		} else {
			// fully match last byte with mask
			Self {
				prefix: &key[..len_clipped - 1],
				prefix_end: key[len_clipped - 1],
				mask_end_bits: 0xff,
			}
			// this should work too:
			// Self { prefix: &key[..len_clipped], prefix_end: 0, mask_end_bits: 0 }
		}
	}

	pub(super) fn test_key<'data>(&self, key: &'data [u8]) -> ForwardSearchResult<'data> {
		match self
			.prefix
			.cmp(&key[..self.prefix.len()])
			.then_with(|| self.prefix_end.cmp(&(key[self.prefix.len()] & self.mask_end_bits)))
		{
			Ordering::Equal => ForwardSearchResult::Match(key),
			// key we're looking for still greater than entry from file
			Ordering::Greater => ForwardSearchResult::Continue,
			// now entry in file greater than our search key - only getting worse now.
			Ordering::Less => ForwardSearchResult::Break,
		}
	}
}
