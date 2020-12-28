extern crate hibp_index;

use hibp_index::index::{ContentType, Index};
use hibp_index::sha1::SHA1;

use std::fs;
use std::io::{self, BufRead, BufReader};

fn main() -> anyhow::Result<()> {
	let input = BufReader::new(fs::File::open("hibp-sha1.index")?);
	let mut index = Index::open(input)?;
	assert_eq!(index.content_type(), &ContentType::SHA1);
	assert_eq!(index.key_size(), 20);
	for line in io::stdin().lock().lines() {
		let line = line?;
		let sha1 = match line.parse::<SHA1>() {
			Ok(sha1) => sha1,
			Err(_) => SHA1::hash(line.as_bytes()),
		};
		if let Some(_) = index.lookup(&sha1, &mut [])? {
			println!("Found: {}", sha1);
		} else {
			println!("Not found: {}", sha1);
		}
	}
	Ok(())
}
