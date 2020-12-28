extern crate hibp_index;

use hibp_index::index::{ContentType, Index};
use hibp_index::sha1::SHA1;
use hibp_index::ntlm::NTLM;

use std::fs;
use std::io::{self, BufRead, BufReader};

fn open_index(path: &str, content_type: &ContentType, key_size: u8) -> anyhow::Result<Option<Index<BufReader<fs::File>>>> {
	let path = std::path::Path::new(path);
	if path.is_file() {
		let index = Index::open(BufReader::new(fs::File::open(path)?))?;
		if index.content_type() != content_type {
			anyhow::bail!("{:?} uses invalid content type: {:?}, expected {:?}", path, index.content_type(), content_type);
		}
		if index.key_size() != key_size {
			anyhow::bail!("{:?} uses invalid key size: {:?}, expected {:?}", path, index.key_size(), key_size);
		}
		Ok(Some(index))
	} else {
		Ok(None)
	}
}

fn check<K, R>(index: &mut Index<R>, hash: &K) -> anyhow::Result<()>
where
	K: std::fmt::Display + std::ops::Deref<Target=[u8]>,
	R: io::Seek + io::BufRead,
{
	if let Some(_) = index.lookup(&hash, &mut [])? {
		println!("Found {}: {}", &**index.content_type(), hash);
	} else {
		println!("Not found {}: {}", &**index.content_type(), hash);
	}
	Ok(())
}

fn main() -> anyhow::Result<()> {
	let mut sha1_index = open_index("hibp-sha1.index", &ContentType::SHA1, 20)?;
	let mut ntlm_index = open_index("hibp-ntlm.index", &ContentType::NTLM, 16)?;
	if sha1_index.is_none() && ntlm_index.is_none() {
		anyhow::bail!("Couldn't find either 'hibp-sha1.index' nor 'hibp-ntlm.index'");
	}
	for line in io::stdin().lock().lines() {
		let line = line?;
		if let Ok(sha1) = line.parse::<SHA1>() {
			if let Some(i) = &mut sha1_index {
				check(i, &sha1)?;
			} else {
				println!("Missing SHA1 index, can't check {}", sha1);
			}
		} else if let Ok(ntlm) = line.parse::<NTLM>() {
			if let Some(i) = &mut ntlm_index {
				check(i, &ntlm)?;
			} else {
				println!("Missing NTLM index, can't check {}", ntlm);
			}
		} else if let Some(i) = &mut sha1_index {
			let sha1 = SHA1::hash(line.as_bytes());
			check(i, &sha1)?;
		} else if let Some(i) = &mut ntlm_index {
			let ntlm = NTLM::hash(&line);
			check(i, &ntlm)?;
		} else {
			unreachable!("At least one index must be loaded");
		}
	}
	Ok(())
}
