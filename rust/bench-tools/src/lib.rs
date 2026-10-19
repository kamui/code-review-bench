#![forbid(unsafe_code)]

//! Deterministic benchmark tools, one module per tool, behind the `bench-tools` binary.

pub mod derive_packet;
pub mod diff_identity;
pub mod pyjson;
pub mod pytext;
pub mod roster;
pub mod sha256;
