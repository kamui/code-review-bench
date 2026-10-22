//! An installed `bench-tools`: this build and the receipt a runtime store keeps beside it.

use bench_tools::sha256::hex_digest;
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

/// This build's `bench-tools` and its SHA-256, hashed once per test process.
pub fn built() -> &'static (PathBuf, String) {
    static BUILT: OnceLock<(PathBuf, String)> = OnceLock::new();
    BUILT.get_or_init(|| {
        let path = PathBuf::from(env!("CARGO_BIN_EXE_bench-tools"));
        let sha = hex_digest(&fs::read(&path).unwrap());
        (path, sha)
    })
}

pub fn receipt(sha: &str, size: u64) -> String {
    let commit = "0".repeat(40);
    let zero = "0".repeat(64);
    format!(
        r#"{{"schema": "rust-runtime-receipt/1",
 "crate": {{"name": "bench-tools", "bin": "bench-tools", "path": "rust/bench-tools"}},
 "source": {{"revision": "{commit}", "tree": "{commit}", "cargo_toml_sha256": "{zero}",
  "cargo_lock_sha256": "{zero}", "toolchain_file_sha256": "{zero}"}},
 "toolchain": {{"rustc": {{"release": "1.99.0"}}}}, "target": "{}",
 "build": {{"command": ["cargo", "build"]}},
 "artifact": {{"sha256": "{sha}", "file": "bench-tools", "size": {size}}}}}"#,
        host_target()
    )
}

/// The target a receipt names for this host, as `bench-tools runtime install` builds it.
fn host_target() -> &'static str {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => "x86_64-unknown-linux-gnu",
        ("linux", "aarch64") => "aarch64-unknown-linux-gnu",
        ("macos", "aarch64") => "aarch64-apple-darwin",
        (os, arch) => panic!("no runtime target for {os} {arch}"),
    }
}
