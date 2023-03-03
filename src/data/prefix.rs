use super::KeyData;

#[derive(Clone)]
pub struct Prefix<D> {
	key: D,
	bits: u32,
}

impl<D> Prefix<D>
where
	D: KeyData,
{
	/// Extract prefix of given length in bits from key
	pub fn new_from_key(key: &D, bits: u32) -> Self {
		assert!(bits as usize <= D::SIZE * 8);
		Self::new_from_raw(key.data(), bits)
	}

	/// Extract prefix of given length in bits from raw (potentially partial) key data
	///
	/// raw must contain enough octets to contain the bits.
	pub fn new_from_raw(raw: &[u8], bits: u32) -> Self {
		assert!(bits as usize <= D::SIZE * 8);
		assert!(bits as usize <= raw.len() * 8);
		let octets = (bits as usize + 7) / 8;
		let mut key = D::default();
		let key_data = key.data_mut();
		key_data[..octets].copy_from_slice(&raw[..octets]);
		// mask bits we need in last octet
		let need_bits = bits & 7;
		if need_bits != 0 {
			let mask_bits: u8 = !(0xff >> need_bits);
			key_data[octets-1] &= mask_bits;
		}
		Self { key, bits }
	}

	/// Key containing prefix; unused (suffix) bits are zero
	pub fn key(&self) -> &D {
		&self.key
	}

	/// Length of prefix in bits
	pub fn bits(&self) -> u32 {
		self.bits
	}

	/// Show hex digits of prefix
	pub fn hex(&self) -> impl '_ + std::fmt::Display {
		self.key.hex_bit_range(0, self.bits)
	}

	/// Recombine prefix with suffix data from hexadecimal input
	pub fn unsplit_from_hex_suffix(&self, suffix_str: &str) -> Result<D, hex::FromHexError> {
		let suffix_str = suffix_str.as_bytes();
		let mut key = self.key.clone();
		let key_data = key.data_mut();
		let start = self.bits as usize / 8;
		// bits allowed in first octect of suffix
		let truncate_bits = self.bits & 7;
		let mask_bits: u8 = 0xff >> truncate_bits;

		if self.bits & 7 >= 4 {
			// prefix contains full nibble on shared octet; the nibble is not part of the suffix
			if suffix_str.is_empty() {
				return Err(hex::FromHexError::InvalidStringLength);
			}
			let shared_octet_str = [b'0', suffix_str[0]];
			let mut shared_octet: u8 = 0;
			hex::decode_to_slice(shared_octet_str, std::slice::from_mut(&mut shared_octet))?;
			key_data[start] |= mask_bits & shared_octet;
			hex::decode_to_slice(&suffix_str[1..], &mut key_data[start+1..])?;
		} else if start == D::SIZE {
			// prefix already is full key
			if !suffix_str.is_empty() {
				return Err(hex::FromHexError::InvalidStringLength);
			}
		} else {
			let mut shared_octet: u8 = 0;
			hex::decode_to_slice(&suffix_str[..2], std::slice::from_mut(&mut shared_octet))?;
			key_data[start] |= mask_bits & shared_octet;
			hex::decode_to_slice(&suffix_str[2..], &mut key_data[start+1..])?;
		}
		Ok(key)
	}

	/// Combines prefix and suffix to a full key
	///
	/// Panics if prefix length in prefix and suffix don't match.
	pub fn unsplit(&self, suffix: Suffix<D>) -> D {
		assert_eq!(self.bits, suffix.prefix_bits);
		let mut key = self.key.clone();
		let key_data = key.data_mut();
		let start = self.bits as usize / 8;
		key_data[start+1..].copy_from_slice(&suffix.key.data()[start+1..]);
		// As both prefix and suffix are "clean" in unused bits we can
		// combine the (potentially) overlapping byte with bitwise or.
		key_data[start] |= suffix.key.data()[start];
		key
	}
}

impl<D> std::fmt::Debug for Prefix<D>
where
	D: KeyData,
{
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}/{}", self.hex(), self.bits)
	}
}

#[derive(Clone)]
pub struct Suffix<D> {
	key: D,
	prefix_bits: u32,
}

impl<D> Suffix<D>
where
	D: KeyData,
{
	/// Extract suffix from key (removing given number of prefix bits)
	pub fn new_from_key(key: &D, prefix_bits: u32) -> Self {
		assert!(prefix_bits as usize <= D::SIZE * 8);
		let start = prefix_bits as usize / 8;
		Self::new_from_suffix_raw(&key.data()[start..], prefix_bits)
	}

	/// Raw must contain exactly the octects needed to store the suffix of a key,
	/// if the full octects in the prefix with the given length is not included.
	pub fn new_from_suffix_raw(raw: &[u8], prefix_bits: u32) -> Self {
		let start = prefix_bits as usize / 8;
		let mut key = D::default();
		let key_data = key.data_mut();
		key_data[start..].copy_from_slice(raw);
		// mask bits we need in last octet
		let truncate_bits = prefix_bits & 7;
		if truncate_bits != 0 {
			let mask_bits: u8 = 0xff >> truncate_bits;
			key_data[start] &= mask_bits;
		}
		Self { key, prefix_bits }
	}

	/// Key containing suffix; unused (prefix) bits are zero
	pub fn key(&self) -> &D {
		&self.key
	}

	/// Length of prefix in bits (i.e. "unused" bits in this suffix)
	pub fn prefix_bits(&self) -> u32 {
		self.prefix_bits
	}

	/// Show hex digits of suffix
	pub fn hex(&self) -> impl '_ + std::fmt::Display {
		self.key.hex_bit_range(self.prefix_bits, D::SIZE as u32 * 8)
	}
}

impl<D> std::fmt::Debug for Suffix<D>
where
	D: KeyData,
{
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "<0../{}>{}", self.prefix_bits, self.hex())
	}
}
