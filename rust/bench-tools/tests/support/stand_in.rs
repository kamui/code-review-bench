//! The stand-in clients and probes of `stand_in/main.rs`, compiled once per source with this toolchain's `rustc`.
//! A second `[[bin]]` would be built and installed with `bench-tools`, whose runtime installer builds the first bin
//! target it finds.

use bench_tools::sha256::hex_digest;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

/// Compiled into each test crate too, so Clippy and rustfmt check it with the tests.
#[allow(dead_code, reason = "the tests run this source as its own executable")]
#[path = "stand_in/main.rs"]
mod program;

const SOURCE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/support/stand_in/main.rs"
);

/// The stand-in executable, built under `CARGO_TARGET_TMPDIR` for this source unless a build is already there.
pub fn built() -> &'static Path {
    static BUILT: OnceLock<PathBuf> = OnceLock::new();
    BUILT.get_or_init(|| {
        let digest = hex_digest(&fs::read(SOURCE).unwrap());
        let directory =
            Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("stand-in-{}", &digest[..16]));
        let executable = directory.join("stand-in");
        if executable.is_file() {
            return executable;
        }
        let staging = directory.with_extension(format!("{}.tmp", std::process::id()));
        let _ = fs::remove_dir_all(&staging);
        fs::create_dir_all(&staging).unwrap();
        let rustc = Path::new(env!("CARGO")).with_file_name("rustc");
        let rustc = if rustc.is_file() {
            rustc
        } else {
            PathBuf::from("rustc")
        };
        let compiled = Command::new(rustc)
            .args([
                "--edition",
                "2024",
                "--crate-name",
                "stand_in",
                "-C",
                "debuginfo=0",
            ])
            .arg("-o")
            .arg(staging.join("stand-in"))
            .arg(SOURCE)
            .output()
            .unwrap();
        assert!(
            compiled.status.success(),
            "{}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        // Another test process may have finished the same build first; either copy serves.
        if fs::rename(&staging, &directory).is_err() {
            fs::remove_dir_all(&staging).unwrap();
        }
        executable
    })
}

/// The stand-in installed as `name` in `directory`, where it acts as that client. An installed copy stays: it may
/// be a hard link to the build, which copying over it would truncate.
#[allow(
    dead_code,
    reason = "network_gate runs the build without installing it"
)]
pub fn install(directory: &Path, name: &str) {
    let target = directory.join(name);
    if target.exists() {
        return;
    }
    if fs::hard_link(built(), &target).is_err() {
        fs::copy(built(), &target).unwrap();
    }
}
