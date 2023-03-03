macro_rules! build_hex_wrapper {
	($vis:vis $typ:ident[$len:literal]) => {
		$vis struct $typ([u8; $len]);

		impl AsRef<[u8; $len]> for $typ {
			fn as_ref(&self) -> &[u8; $len] {
				&self.0
			}
		}

		impl AsRef<[u8]> for $typ {
			fn as_ref(&self) -> &[u8] {
				&self.0
			}
		}

		impl AsRef<str> for $typ {
			fn as_ref(&self) -> &str {
				std::str::from_utf8(&self.0).expect("hex digits")
			}
		}

		impl std::ops::Deref for $typ {
			type Target = str;

			fn deref(&self) -> &Self::Target {
				self.as_ref()
			}
		}

		impl std::fmt::Display for $typ {
			fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
				f.write_str(self.as_ref())
			}
		}
	};
}
