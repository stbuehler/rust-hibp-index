extern crate hibp_index;

use hibp_index::data::{KeyData, NoPayload, NTLM, SHA1};
use hibp_index::index::{Index, TypedIndex};

use std::fs;
use std::io::{self, BufRead};
use std::path::Path;

struct AppConfig {
	auto_load: bool,
	load_sha1: bool,
	sha1_index: &'static Path,
	load_ntlm: bool,
	ntlm_index: &'static Path,
	one_shot: bool,
	plaintext: bool,
	no_plaintext: bool,
}

fn app() -> anyhow::Result<AppConfig> {
	#[derive(clap::Parser)]
	#[command(author, version)]
	#[command(help_template(
		"\
{before-help}{name} {version}
{author-with-newline}{about-with-newline}
{usage-heading} {usage}

{all-args}{after-help}
"
	))]
	/// Tool to lookup SHA-1/NTLM hashes in index database
	struct Cli {
		#[arg(long)]
		/// Load SHA-1 index; disable auto-loading NTLM index
		sha1: bool,

		#[arg(long)]
		/// Load NTLM index; disable auto-loading SHA-1 index
		ntlm: bool,

		#[arg(long)]
		/// Only test single password; exit code 0 signals password is fine (not included in index)
		oneshot: bool,

		#[arg(long)]
		/// Treat every input line as plaintext password
		plaintext: bool,

		#[arg(long = "no-plaintext", conflicts_with("plaintext"))]
		/// Every input line must be a hash (either SHA-1 or NTLM)
		no_plaintext: bool,
	}

	let cli = <Cli as clap::Parser>::parse();

	let mut cfg = AppConfig {
		auto_load: true,
		load_sha1: false,
		sha1_index: Path::new("hibp-sha1.index"),
		load_ntlm: false,
		ntlm_index: Path::new("hibp-ntlm.index"),
		one_shot: cli.oneshot,
		plaintext: cli.plaintext,
		no_plaintext: cli.no_plaintext,
	};
	if cli.sha1 {
		cfg.auto_load = false;
		cfg.load_sha1 = true;
	}
	if cli.ntlm {
		cfg.auto_load = false;
		cfg.load_ntlm = true;
	}
	// TODO: make filenames configurable
	if cfg.auto_load {
		if !cfg.load_sha1 && cfg.sha1_index.is_file() {
			cfg.load_sha1 = true;
		}
		if !cfg.load_ntlm && cfg.ntlm_index.is_file() {
			cfg.load_ntlm = true;
		}
		if !cfg.load_sha1 && !cfg.load_ntlm {
			anyhow::bail!("Couldn't find either {:?} nor {:?}", cfg.sha1_index, cfg.ntlm_index);
		}
	}
	Ok(cfg)
}

fn open_index<D>(path: &Path) -> anyhow::Result<TypedIndex<D, NoPayload, fs::File>>
where
	D: KeyData,
{
	let index = Index::open(fs::File::open(path)?)?;
	if index.key_type() != &*D::KEY_TYPE {
		anyhow::bail!(
			"{:?} uses invalid key type: {:?}, expected {:?}",
			path,
			index.key_type(),
			D::KEY_TYPE
		);
	}
	if index.key_size() != D::KEY_TYPE.key_bytes_length() {
		anyhow::bail!(
			"{:?} uses invalid key size: {:?}, expected {:?}",
			path,
			index.key_size(),
			D::KEY_TYPE.key_bytes_length()
		);
	}
	Ok(TypedIndex::<D, NoPayload, _>::new(index)?)
}

fn check<D>(
	cfg: &AppConfig,
	index: &TypedIndex<D, NoPayload, fs::File>,
	hash: &D,
) -> anyhow::Result<()>
where
	D: KeyData + std::fmt::Display,
{
	let is_present = index.lookup(hash)?.is_some();
	if cfg.one_shot {
		std::process::exit(if is_present { 1 } else { 0 });
	}
	if is_present {
		println!("Found {}: {}", D::KEY_TYPE.name(), hash);
	} else {
		println!("Not found {}: {}", D::KEY_TYPE.name(), hash);
	}
	Ok(())
}

#[allow(clippy::upper_case_acronyms)]
enum Input {
	SHA1(SHA1),
	NTLM(NTLM),
}

impl Input {
	fn new(cfg: &AppConfig, line: String) -> anyhow::Result<Self> {
		if !cfg.plaintext {
			if cfg.load_sha1 {
				if let Ok(sha1) = line.parse::<SHA1>() {
					return Ok(Self::SHA1(sha1));
				}
			}
			if cfg.load_ntlm {
				if let Ok(ntlm) = line.parse::<NTLM>() {
					return Ok(Self::NTLM(ntlm));
				}
			}
		}
		if !cfg.no_plaintext {
			// fallback: treat as plaintext
			if cfg.load_sha1 {
				let sha1 = SHA1::hash(line.as_bytes());
				return Ok(Self::SHA1(sha1));
			}
			if cfg.load_ntlm {
				let ntlm = NTLM::hash(&line);
				return Ok(Self::NTLM(ntlm));
			}
			anyhow::bail!("Can't handle input - no index available");
		}
		anyhow::bail!("Can't handle input - plaintext input not allowed");
	}
}

fn main() -> anyhow::Result<()> {
	let cfg = app()?;
	let sha1_index = if cfg.load_sha1 { Some(open_index::<SHA1>(cfg.sha1_index)?) } else { None };
	let ntlm_index = if cfg.load_ntlm { Some(open_index::<NTLM>(cfg.ntlm_index)?) } else { None };
	for line in io::stdin().lock().lines() {
		match Input::new(&cfg, line?)? {
			Input::SHA1(sha1) => {
				check(&cfg, sha1_index.as_ref().expect("SHA1 index required"), &sha1)?;
			},
			Input::NTLM(ntlm) => {
				check(&cfg, ntlm_index.as_ref().expect("NTLM index required"), &ntlm)?;
			},
		}
	}
	Ok(())
}
