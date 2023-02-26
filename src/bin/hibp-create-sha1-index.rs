extern crate hibp_index;

use hibp_index::index::{Depth, NoPayload, TypedBuilder};
use hibp_index::sha1::SHA1;

use std::fs;
use std::io::{BufRead, BufReader, BufWriter};

fn main() -> anyhow::Result<()> {
	let input = BufReader::new(fs::File::open("pwned-passwords-sha1-ordered-by-hash-v7.txt")?);
	let output = BufWriter::new(
		fs::OpenOptions::new().write(true).create_new(true).open("hibp-sha1.index")?,
	);
	let mut builder =
		TypedBuilder::<SHA1, NoPayload, _>::create(output, "pwned-passwords v7", Depth::DEPTH20)?;
	for line in input.lines() {
		builder.add_entry_from_hibp_line(&line?)?;
	}
	builder.finish()?;
	Ok(())
}
