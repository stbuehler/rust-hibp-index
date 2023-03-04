//! Indexed database
//!
//! Contains keys (with fixed length) and payload (again fixed length) per key.
//!
//! Entries are ordered by keys; a (compressed) table at the end of the
//! database stores a file offset for each (bitstring) prefix (with
//! parameter "depth" of index), where entries with that prefix start;
//! keys end at the start of the next prefix (table includes a final offset
//! for end of all keys).

mod builder;
mod depth;
mod hashlist;
mod key_suffix;
mod prefix;
mod reader;
mod table;
mod table_helper;
mod typed_reader;

use self::{depth::BucketIndexInner, prefix::BucketIndex};

pub use self::{
	builder::{Builder, TypedBuilder},
	depth::Depth,
	hashlist::{TypedListReader, TypedListWriter},
	key_suffix::KeySuffix,
	prefix::{Prefix, PrefixRange},
	reader::Index,
	typed_reader::TypedIndex,
};
