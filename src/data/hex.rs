pub struct DisplayHex<'a> {
	data: &'a [u8],
	start: u32,
	end: u32,
}

impl<'a> DisplayHex<'a> {
	pub(super) fn new(data: &'a [u8], start: u32, end: u32) -> Self {
		Self { data, start, end }
	}
}

struct Nibble(u8);

impl Nibble {
	fn new_high(raw: u8) -> Self {
		let mut buf = [0u8; 2];
		hex::encode_to_slice([raw], &mut buf).expect("length");
		Self(buf[0])
	}

	fn new_low(raw: u8) -> Self {
		let mut buf = [0u8; 2];
		hex::encode_to_slice([raw], &mut buf).expect("length");
		Self(buf[1])
	}
}

impl AsRef<str> for Nibble {
	fn as_ref(&self) -> &str {
		std::str::from_utf8(std::slice::from_ref(&self.0)).expect("ascii")
	}
}

impl std::fmt::Display for DisplayHex<'_> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		let mut s = self.start / 8;
		if (self.start & 0x7) >= 4 {
			// skip first 4 bits - i.e. first (high) nibble
			f.write_str(Nibble::new_low(self.data[s as usize]).as_ref())?;
			s += 1;
		}
		let mut e = (self.end + 7) / 8;
		let final_nibble = if (self.end & 0x7) > 0 && (self.end & 0x7) <= 4 {
			// skip last 4 bits of final octet - i.e. last low nibble
			e -= 1;
			Some(Nibble::new_high(self.data[e as usize]))
		} else {
			None
		};
		for c in &self.data[s as usize..e as usize] {
			let mut buf = [0u8; 2];
			hex::encode_to_slice([*c], &mut buf).expect("length");
			f.write_str(std::str::from_utf8(&buf).expect("ascii"))?;
		}
		if let Some(n) = final_nibble {
			f.write_str(n.as_ref())?;
		}
		Ok(())
	}
}
