mod read_at;

pub use self::read_at::{FileLen, ReadAt};

use cached::{Cached, SizedCache};
use std::io;

const PAGE_SIZE_BITS: u32 = 13;
const PAGE_SIZE: usize = 1 << PAGE_SIZE_BITS;

pub struct BufReader<'a, R> {
	cache: SizedCache<u64, Vec<u8>>,
	position: u64,
	reader: &'a R,
}

impl<'a, R: ReadAt> BufReader<'a, R> {
	pub fn new(reader: &'a R, cache_capacity: usize) -> Self {
		let cache = SizedCache::with_size(cache_capacity);
		Self { cache, position: 0, reader }
	}

	fn load_page(&mut self) -> io::Result<&[u8]> {
		let page = self.position >> PAGE_SIZE_BITS;
		let page_offset = page << PAGE_SIZE_BITS;
		let offset = (self.position - page_offset) as usize;
		if self.cache.cache_get(&page).is_none() {
			let mut buf = Vec::new();

			buf.resize(PAGE_SIZE, 0);
			let got = self.reader.read_at_till_eof(&mut buf, page_offset)?;
			buf.truncate(got);

			self.cache.cache_set(page, buf);
		}
		// lookup twice to avoid borrowing issues
		Ok(&self.cache.cache_get(&page).expect("just inserted")[offset..])
	}
}

impl<R: ReadAt> io::Read for BufReader<'_, R> {
	fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
		let source = self.load_page()?;
		let amount = std::cmp::min(source.len(), buf.len());
		buf[..amount].copy_from_slice(&source[..amount]);
		self.position += amount as u64;
		Ok(amount)
	}
}

fn checked_opt<T>(value: Option<T>, msg: &'static str) -> io::Result<T> {
	match value {
		Some(v) => Ok(v),
		None => Err(io::Error::new(io::ErrorKind::Other, msg)),
	}
}

impl<R: ReadAt + FileLen> io::Seek for BufReader<'_, R> {
	fn seek(&mut self, pos: io::SeekFrom) -> io::Result<u64> {
		match pos {
			io::SeekFrom::Start(pos) => {
				self.position = pos;
			},
			io::SeekFrom::Current(offset) => {
				if offset >= 0 {
					self.position =
						checked_opt(self.position.checked_add(offset as u64), "position overflow")?;
				} else {
					let offset = offset.checked_abs().expect("negative offset overflow");
					self.position = checked_opt(
						self.position.checked_sub(offset as u64),
						"position (negative) overflow",
					)?;
				}
			},
			io::SeekFrom::End(offset) => {
				let orig_pos = self.position;
				self.position = self.reader.file_len()?;
				if let Err(e) = self.seek(io::SeekFrom::Current(offset)) {
					self.position = orig_pos;
					return Err(e);
				}
			},
		}
		Ok(self.position)
	}
}
