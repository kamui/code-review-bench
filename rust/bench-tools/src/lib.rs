#![forbid(unsafe_code)]

//! Deterministic benchmark tools, one module per tool, behind the `bench-tools` binary.

pub mod pyjson;
pub mod pytext;
pub mod roster;
pub mod sha256;
