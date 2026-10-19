#![forbid(unsafe_code)]

//! Deterministic benchmark tools, one module per tool, behind the `bench-tools` binary.

pub mod claims;
pub mod diff_identity;
pub mod pyfs;
pub mod pyjson;
pub mod pyregex;
pub mod pytext;
pub mod pyunicode;
pub mod rates;
pub mod roster;
pub mod schema;
pub mod sha256;
pub mod upstream;
