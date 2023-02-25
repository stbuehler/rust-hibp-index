extern crate hibp_index;

use hibp_index::index::TypedBuilder;
use hibp_index::ntlm::NTLM;

use std::fs;
use std::io::{BufRead, BufReader, BufWriter};

fn main() -> anyhow::Result<()> {
	let input = BufReader::new(fs::File::open("pwned-passwords-ntlm-ordered-by-hash-v7.txt")?);
	let output = BufWriter::new(
		fs::OpenOptions::new().write(true).create_new(true).open("hibp-ntlm.index")?,
	);
	let mut builder = TypedBuilder::<NTLM, _, 0>::create(output, "pwned-passwords v7", 20)?;
	for line in input.lines() {
		let line = line?;
		if let Some(colon) = line.find(':') {
			let ntlm = line[..colon].parse::<NTLM>()?;
			builder.add_entry(&ntlm, b"")?;
		} else if !line.is_empty() {
			panic!("Invalid input line: {:?}", line);
		}
	}
	builder.finish()?;
	Ok(())
}
