use std::io;

pub trait ReadAt {
	/// Read from file at given offset into buffer.
	///
	/// Reading beyond EOF returns `Ok(0)`, otherwise short reads are allowed (i.e. not filling
	/// buffer even if EOF isn't reached).
	///
	/// Might update file position depending on operating system
	fn read_at(&self, buf: &mut [u8], offset: u64) -> io::Result<usize>;

	/// Read until buffer is filled or EOF is reached.
	///
	/// Might update file position depending on operating system
	fn read_at_till_eof(&self, mut buf: &mut [u8], mut offset: u64) -> io::Result<usize> {
		let mut total = 0;
		while !buf.is_empty() {
			match self.read_at(buf, offset) {
				Ok(0) => return Ok(total),
				Ok(n) => {
					let tmp = buf;
					buf = &mut tmp[n..];
					offset += n as u64;
					total += n;
				},
				Err(ref e) if e.kind() == io::ErrorKind::Interrupted => {},
				Err(e) => return Err(e),
			}
		}
		Ok(total)
	}
}

/// Need file size to support `SeekFrom::End`
pub trait FileLen {
	fn file_len(&self) -> io::Result<u64>;
}

impl FileLen for std::fs::File {
	fn file_len(&self) -> io::Result<u64> {
		Ok(self.metadata()?.len())
	}
}

#[cfg(unix)]
mod unix_impl {
	use std::os::unix::fs::FileExt;

	impl<F: FileExt> super::ReadAt for F {
		fn read_at(&self, buf: &mut [u8], offset: u64) -> std::io::Result<usize> {
			FileExt::read_at(self, buf, offset)
		}
	}
}

#[cfg(windows)]
mod windows_impl {
	use std::os::windows::fs::FileExt;

	impl<F: FileExt> super::ReadAt for F {
		fn read_at(&self, buf: &mut [u8], offset: u64) -> std::io::Result<usize> {
			FileExt::seek_read(self, buf, offset)
		}
	}
}
