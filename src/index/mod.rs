mod builder;
mod content_type;
mod depth;
mod key_suffix;
mod payload_type;
mod prefix;
mod reader;
mod table;
mod table_helper;

use self::{depth::BucketIndexInner, prefix::BucketIndex};

pub use self::{
	builder::{Builder, BuilderCreateError, TypedBuilder},
	content_type::{ContentType, ContentTypeData, ContentTypeParseError, KnownContentType},
	depth::Depth,
	key_suffix::KeySuffix,
	payload_type::{PayloadData, NoPayload, PayloadDataExt},
	prefix::{Prefix, PrefixRange},
	reader::{Index, IndexOpenError, LookupError},
};
