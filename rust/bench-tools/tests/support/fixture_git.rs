//! `fixture-git [--directory DIR] [FILE ...]`, the corpus setup program that commits DIR, by default the
//! current directory, with a fixed identity, clock and no compression. It removes the config and message, whose
//! bytes depend on the host, rewrites the index from the commit without file timestamps, and writes the commit into
//! each named file in place of [`FREEZE_PLACEHOLDER`]. The objects, refs and index are then the same bytes on every
//! host until a later command refreshes the index.

use std::ffi::OsString;
use std::path::Path;
use std::process::Command;

pub const FREEZE_PLACEHOLDER: &str = "<freeze commit>";

const FIXED: [(&str, &str); 7] = [
    ("GIT_AUTHOR_NAME", "Fixture"),
    ("GIT_AUTHOR_EMAIL", "fixture@example.invalid"),
    ("GIT_CONFIG_NOSYSTEM", "1"),
    ("GIT_COMMITTER_NAME", "Fixture"),
    ("GIT_COMMITTER_EMAIL", "fixture@example.invalid"),
    ("GIT_AUTHOR_DATE", "2026-01-01T00:00:00Z"),
    ("GIT_COMMITTER_DATE", "2026-01-01T00:00:00Z"),
];

/// Runs `fixture-git` in `cwd`. With `environment`, git sees only those variables and the fixed ones.
pub fn run(
    cwd: &Path,
    args: &[OsString],
    environment: Option<&[(String, String)]>,
) -> Result<(), String> {
    let (directory, names) = match args {
        [flag, directory, names @ ..] if flag == "--directory" => (cwd.join(directory), names),
        names => (cwd.to_path_buf(), names),
    };
    let git = |arguments: &[&str]| -> Result<Vec<u8>, String> {
        let mut command = Command::new("git");
        if let Some(environment) = environment {
            command.env_clear().envs(environment.iter().cloned());
        }
        let output = command
            .envs(FIXED)
            .arg("-C")
            .arg(&directory)
            .args([
                "-c",
                "core.compression=0",
                "-c",
                "core.logAllRefUpdates=false",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(arguments)
            .current_dir(cwd)
            .output()
            .map_err(|error| format!("git: {error}"))?;
        if !output.status.success() {
            return Err(format!(
                "git {arguments:?}: {}\n{}",
                output.status,
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        Ok(output.stdout)
    };
    git(&["init", "-q", "--template=", "-b", "main"])?;
    git(&["add", "-A"])?;
    git(&["commit", "-q", "-m", "Freeze the fixture run"])?;
    let commit = git(&["rev-parse", "HEAD"])?.trim_ascii().to_vec();
    for name in ["index", "COMMIT_EDITMSG", "config"] {
        match std::fs::remove_file(directory.join(".git").join(name)) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
                return Err(format!("{name}: {error}"));
            }
            _ => {}
        }
    }
    git(&["read-tree", "HEAD"])?;
    for name in names {
        let path = cwd.join(name);
        let bytes = std::fs::read(&path).map_err(|error| format!("{name:?}: {error}"))?;
        std::fs::write(
            &path,
            replace(&bytes, FREEZE_PLACEHOLDER.as_bytes(), &commit),
        )
        .map_err(|error| format!("{name:?}: {error}"))?;
    }
    Ok(())
}

fn replace(data: &[u8], from: &[u8], to: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len());
    let mut at = 0;
    while at < data.len() {
        if data[at..].starts_with(from) {
            out.extend_from_slice(to);
            at += from.len();
        } else {
            out.push(data[at]);
            at += 1;
        }
    }
    out
}
