extern crate hibp_index;

use hibp_index::index::{Depth, NoPayload, TypedBuilder};
use hibp_index::ntlm::NTLM;

use std::fs;
use std::io::{BufRead, BufReader, BufWriter};

fn main() -> anyhow::Result<()> {
	let input = BufReader::new(fs::File::open("pwned-passwords-ntlm-ordered-by-hash-v7.txt")?);
	let output = BufWriter::new(
		fs::OpenOptions::new().write(true).create_new(true).open("hibp-ntlm.index")?,
	);
	let mut builder =
		TypedBuilder::<NTLM, NoPayload, _>::create(output, "pwned-passwords v7", Depth::DEPTH20)?;
	for line in input.lines() {
		builder.add_entry_from_hibp_line(&line?)?;
	}
	builder.finish()?;
	Ok(())
}
