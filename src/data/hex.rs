use super::seal_trait::U8Array;
use super::FixedByteArray;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
/// Hex representation of a byte array
pub struct Hex<D> {
	str_data: D,
}

impl Hex<()> {
	pub(super) fn new<A: FixedByteArray>(arr: &A) -> Hex<A::HexArray> {
		let mut str_data = A::HexArray::zeroed();
		hex::encode_to_slice(arr.data(), str_data.as_mut()).expect("length mismatch");
		Hex { str_data }
	}
}

impl<D: AsRef<[u8]>> Hex<D> {
	fn _raw(&self) -> &[u8] {
		self.str_data.as_ref()
	}

	/// String representation
	pub fn as_str(&self) -> &str {
		std::str::from_utf8(self._raw()).expect("ascii")
	}
}

impl<D: AsRef<[u8]>> std::ops::Deref for Hex<D> {
	type Target = str;

	fn deref(&self) -> &Self::Target {
		self.as_str()
	}
}

impl<D: AsRef<[u8]>> AsRef<str> for Hex<D> {
	fn as_ref(&self) -> &str {
		self.as_str()
	}
}

impl<D: AsRef<[u8]>> AsRef<[u8]> for Hex<D> {
	fn as_ref(&self) -> &[u8] {
		self._raw()
	}
}

impl<D: AsRef<[u8]>> AsRef<D> for Hex<D> {
	fn as_ref(&self) -> &D {
		&self.str_data
	}
}

impl<D: AsRef<[u8]>> std::fmt::Display for Hex<D> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_str(self.as_str())
	}
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
/// Hex representation of (bit-slice) of a byte array
pub struct HexRange<D> {
	len: usize,
	str_data: D,
}

fn nibbles(byte: u8) -> [u8; 2] {
	let mut buf = [0u8; 2];
	hex::encode_to_slice([byte], &mut buf).expect("length");
	buf
}

fn nibble_high(byte: u8) -> u8 {
	nibbles(byte)[0]
}

fn nibble_low(byte: u8) -> u8 {
	nibbles(byte)[1]
}

impl HexRange<()> {
	pub(super) fn new<A: FixedByteArray>(arr: &A, start: u32, end: u32) -> HexRange<A::HexArray> {
		let data = arr.data();
		let mut str_data = A::HexArray::zeroed();
		let target = str_data.as_mut();
		let mut len = 0;

		let mut s = start / 8;
		if (start & 0x7) >= 4 {
			// skip first 4 bits - i.e. first (high) nibble
			target[len] = nibble_low(data[s as usize]);
			len += 1;
			s += 1;
		}
		let mut e = end.div_ceil(8);
		let final_nibble = if (end & 0x7) > 0 && (end & 0x7) <= 4 {
			// skip last 4 bits of final octet - i.e. last low nibble
			e -= 1;
			Some(nibble_high(data[e as usize]))
		} else {
			None
		};
		let main_len = (e - s) as usize;
		hex::encode_to_slice(&data[s as usize..][..main_len], &mut target[len..][..2 * main_len])
			.expect("length");
		len += 2 * main_len;
		if let Some(n) = final_nibble {
			target[len] = n;
			len += 1;
		}

		HexRange { len, str_data }
	}
}

impl<D: AsRef<[u8]>> HexRange<D> {
	fn _raw(&self) -> &[u8] {
		&self.str_data.as_ref()[..self.len]
	}

	/// String representation
	pub fn as_str(&self) -> &str {
		std::str::from_utf8(self._raw()).expect("ascii")
	}
}

impl<D: AsRef<[u8]>> std::ops::Deref for HexRange<D> {
	type Target = str;

	fn deref(&self) -> &Self::Target {
		self.as_str()
	}
}

impl<D: AsRef<[u8]>> AsRef<str> for HexRange<D> {
	fn as_ref(&self) -> &str {
		self.as_str()
	}
}

impl<D: AsRef<[u8]>> AsRef<[u8]> for HexRange<D> {
	fn as_ref(&self) -> &[u8] {
		self._raw()
	}
}

impl<D: AsRef<[u8]>> std::fmt::Display for HexRange<D> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_str(self.as_str())
	}
}
