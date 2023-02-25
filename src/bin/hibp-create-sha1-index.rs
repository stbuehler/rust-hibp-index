extern crate hibp_index;

use hibp_index::index::TypedBuilder;
use hibp_index::sha1::SHA1;

use std::fs;
use std::io::{BufRead, BufReader, BufWriter};

fn main() -> anyhow::Result<()> {
	let input = BufReader::new(fs::File::open("pwned-passwords-sha1-ordered-by-hash-v7.txt")?);
	let output = BufWriter::new(
		fs::OpenOptions::new().write(true).create_new(true).open("hibp-sha1.index")?,
	);
	let mut builder = TypedBuilder::<SHA1, _, 0>::create(output, "pwned-passwords v7", 20)?;
	for line in input.lines() {
		let line = line?;
		if let Some(colon) = line.find(':') {
			let sha1 = line[..colon].parse::<SHA1>()?;
			builder.add_entry(&sha1, b"")?;
		} else if !line.is_empty() {
			panic!("Invalid input line: {:?}", line);
		}
	}
	builder.finish()?;
	Ok(())
}
