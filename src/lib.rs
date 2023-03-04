#![warn(missing_docs)]
//! # HIPB Index
//!
//! Lots of code to build (indexed) lists of password hashes for quick lookup.
//!
//! Tries to use data from <https://haveibeenpwned.com/> and might offer similar APIs one day.

pub mod buf_read;
pub mod data;
pub mod errors;
pub mod index;
