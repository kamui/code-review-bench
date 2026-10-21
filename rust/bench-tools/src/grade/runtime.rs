//! The runner identity of a Rust-prepared key: the running `bench-tools` executable as an installed entry of a
//! `bench/tools/rust_runtime.py` store, `<store>/bench-tools/<sha256>/<file>` beside its `receipt.json`.

use crate::current_grading::{Failure, R};
use crate::pyjson::Value;
use crate::pytext::{self, PyPath};
use crate::{pyvalue, sha256};
use std::path::{Path, PathBuf};

pub const NAME: &str = "bench-tools";
const INSTALL: &str =
    "python3 bench/tools/rust_runtime.py install --crate rust/bench-tools --select";

fn bytes(path: &Path) -> &[u8] {
    pytext::os_bytes(path.as_os_str())
}

fn shown(path: &Path) -> String {
    pytext::stderr_text(bytes(path))
}

fn digest(path: &Path) -> Option<String> {
    std::fs::read(path).ok().map(|raw| sha256::hex_digest(&raw))
}

const RECEIPT: &str = "rust-runtime-receipt/1";

fn is_name(text: &str) -> bool {
    let mut chars = text.chars();
    chars
        .next()
        .is_some_and(|first| first.is_ascii_lowercase() || first.is_ascii_digit())
        && chars.all(|rest| rest.is_ascii_lowercase() || rest.is_ascii_digit() || rest == '-')
}

fn is_hex(text: &str, lengths: std::ops::RangeInclusive<usize>) -> bool {
    lengths.contains(&text.len())
        && text
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn relative(path: &str) -> bool {
    let path = Path::new(path);
    !path.is_absolute()
        && path
            .components()
            .all(|part| part != std::path::Component::ParentDir)
}

/// `rust_runtime.receipt_problem`: why the receipt is not complete and portable. A missing or mistyped field is
/// named by its path rather than by Python's exception repr.
fn receipt_problem(receipt: &Value) -> Option<String> {
    let malformed = |path: &[&str]| format!("a field is missing or malformed: {}", path.join("."));
    let field = |path: &[&str]| -> Result<&Value, String> {
        path.iter().try_fold(receipt, |value, key| {
            value
                .as_object()
                .and_then(|object| object.get(key))
                .ok_or_else(|| malformed(path))
        })
    };
    let text = |path: &[&str]| -> Result<&str, String> {
        field(path)?.as_str().ok_or_else(|| malformed(path))
    };
    let complete = || -> Result<(), String> {
        let schema = field(&["schema"])?;
        if schema.as_str() != Some(RECEIPT) {
            return Err(format!("schema is {}, not {RECEIPT}", schema.py_repr()));
        }
        for part in ["crate", "source", "artifact"] {
            field(&[part])?;
        }
        let crate_complete = is_name(text(&["crate", "name"])?)
            && field(&["crate", "bin"])?.truthy()
            && relative(text(&["crate", "path"])?);
        if !crate_complete {
            return Err("the crate needs a name, a binary and a repository-relative path".into());
        }
        if !is_hex(text(&["source", "revision"])?, 40..=64)
            || !is_hex(text(&["source", "tree"])?, 40..=64)
        {
            return Err("the source needs a commit revision and a crate tree".into());
        }
        for key in [
            "cargo_toml_sha256",
            "cargo_lock_sha256",
            "toolchain_file_sha256",
        ] {
            if !is_hex(text(&["source", key])?, 64..=64) {
                return Err("the source needs the SHA-256 of Cargo.toml, Cargo.lock and rust-toolchain.toml".into());
            }
        }
        let release = field(&["toolchain", "rustc"])?
            .as_object()
            .ok_or_else(|| malformed(&["toolchain", "rustc"]))?
            .get("release")
            .is_some_and(Value::truthy);
        if !release || !field(&["target"])?.truthy() || !field(&["build", "command"])?.truthy() {
            return Err(
                "the receipt needs the rustc release, the target and the build command".into(),
            );
        }
        let artifact_complete = is_hex(text(&["artifact", "sha256"])?, 64..=64)
            && {
                let file = text(&["artifact", "file"])?;
                !["", ".", ".."].contains(&file) && !file.contains('/')
            }
            && match field(&["artifact", "size"])? {
                Value::Int(digits) => !digits.starts_with('-'),
                Value::Float(number) => *number >= 0.0,
                Value::Bool(_) => true,
                _ => return Err(malformed(&["artifact", "size"])),
            };
        if !artifact_complete {
            return Err(
                "the artifact needs a SHA-256, a size and a file name within its entry".into(),
            );
        }
        Ok(())
    };
    complete().err()
}

fn receipt_names(receipt: &Value) -> (Option<&str>, Option<&str>, Option<&str>) {
    let field = |outer: &str, inner: &str| {
        receipt
            .as_object()
            .and_then(|receipt| receipt.get(outer))
            .and_then(Value::as_object)
            .and_then(|found| found.get(inner))
            .and_then(Value::as_str)
    };
    (
        field("crate", "name"),
        field("artifact", "sha256"),
        field("artifact", "file"),
    )
}

/// The pin `rust_runtime.pin_of` gives the running executable's store entry, `{"sha256", "receipt_sha256"}`.
pub fn pin(executable: &Path) -> R<Value> {
    let refuse = |why: &str| {
        Failure::Input(format!(
            "the running {NAME} {} is not an installed runtime: {why}; install it with: {INSTALL}",
            shown(executable)
        ))
    };
    let sha = digest(executable).ok_or_else(|| refuse("it cannot be read"))?;
    let directory = executable
        .parent()
        .ok_or_else(|| refuse("it has no entry directory"))?;
    let in_store = directory.file_name().map(|name| bytes(Path::new(name))) == Some(sha.as_bytes())
        && directory
            .parent()
            .and_then(Path::file_name)
            .is_some_and(|name| name == NAME);
    if !in_store {
        return Err(refuse("it is not in a store entry named by its SHA-256"));
    }
    let receipt_path = directory.join("receipt.json");
    let raw =
        std::fs::read(&receipt_path).map_err(|_| refuse("its receipt.json cannot be read"))?;
    let receipt = pytext::decode_utf8(&raw)
        .ok()
        .and_then(|text| pyvalue::loads(&text).ok())
        .ok_or_else(|| refuse("its receipt.json is not JSON"))?;
    if let Some(problem) = receipt_problem(&receipt) {
        return Err(refuse(&format!("its receipt.json: {problem}")));
    }
    let file = executable.file_name().map(|name| bytes(Path::new(name)));
    let (crate_name, artifact, artifact_file) = receipt_names(&receipt);
    if crate_name != Some(NAME)
        || artifact != Some(sha.as_str())
        || artifact_file.map(str::as_bytes) != file
    {
        return Err(refuse("its receipt describes another executable"));
    }
    Ok(Value::Object(
        [
            ("sha256".to_string(), Value::Str(sha)),
            (
                "receipt_sha256".to_string(),
                Value::Str(sha256::hex_digest(&raw)),
            ),
        ]
        .into_iter()
        .collect(),
    ))
}

/// The problem, if any, with a v2 key's pinned executable: `check` on the store that holds the running executable,
/// without its host checks, and then that the running executable is the pinned one. A store failure starts with
/// `unavailable`.
pub fn problem(pinned: &Value, executable: &Path, unavailable: &str) -> Option<String> {
    let text = |name: &str| {
        pinned
            .as_object()
            .and_then(|pin| pin.get(name))
            .and_then(Value::as_str)
            .unwrap_or("")
    };
    let Some(store) = executable
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
    else {
        return Some(format!(
            "the running {NAME} {} is not in a runtime store",
            shown(executable)
        ));
    };
    match check(store, NAME, text("sha256"), text("receipt_sha256"), false) {
        Err(Checked::Refused(why) | Checked::Input(why)) => Some(format!("{unavailable}{why}")),
        Ok(found) if found.executable != executable => Some(format!(
            "the running {NAME} {} is not the pinned executable {}",
            shown(executable),
            shown(&found.executable)
        )),
        Ok(_) => None,
    }
}

/// Why `check` turned a pinned runtime down.
#[derive(Debug)]
pub enum Checked {
    /// The pinned entry is missing, changed or does not suit this host.
    Refused(String),
    /// The pin itself is not a runtime name and a SHA-256.
    Input(String),
}

/// A pinned executable that hashes to its pin, with its receipt.
pub struct Runtime {
    pub executable: PathBuf,
    pub receipt: Value,
}

/// `rust_runtime.check`: the pinned executable once it and its receipt hash to the pin and, with `here`, it suits
/// this host.
pub fn check(
    store: &Path,
    name: &str,
    sha: &str,
    receipt_sha: &str,
    here: bool,
) -> Result<Runtime, Checked> {
    let refuse = |why: String| Err(Checked::Refused(why));
    if !is_name(name) || !is_hex(sha, 64..=64) {
        return Err(Checked::Input(format!(
            "{} is not a runtime name or {} is not a SHA-256",
            pytext::repr_str(name),
            pytext::repr_str(sha)
        )));
    }
    let directory: PathBuf = store.join(name).join(sha);
    let receipt_path = directory.join("receipt.json");
    let shown_receipt = shown(&receipt_path);
    let raw = match std::fs::read(&receipt_path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return refuse(format!("{shown_receipt} is missing"));
        }
        Err(error) => {
            return refuse(format!(
                "{shown_receipt} cannot be read: {}",
                pytext::os_error(&error, bytes(&receipt_path))
            ));
        }
    };
    let Some(receipt) = pytext::decode_utf8(&raw)
        .ok()
        .and_then(|text| pyvalue::loads(&text).ok())
        .filter(|value| value.as_object().is_some())
    else {
        return refuse(format!("{shown_receipt} is not a JSON object"));
    };
    if sha256::hex_digest(&raw) != receipt_sha {
        return refuse(format!("{shown_receipt} no longer hashes to its pin"));
    }
    if let Some(problem) = receipt_problem(&receipt) {
        return refuse(format!("{shown_receipt}: {problem}"));
    }
    let (crate_name, artifact, file) = receipt_names(&receipt);
    if crate_name != Some(name) || artifact != Some(sha) {
        return refuse(format!(
            "{shown_receipt} describes {} {}, not the pinned {name}",
            crate_name.unwrap_or("None"),
            artifact.unwrap_or("None")
        ));
    }
    if here && let Some(problem) = host_problem(name, &receipt) {
        return refuse(problem);
    }
    let executable = directory.join(file.unwrap_or(""));
    let shown_executable = shown(&executable);
    if std::fs::symlink_metadata(&executable).map_or(true, |found| !found.is_file()) {
        return refuse(format!(
            "the pinned {name} executable {shown_executable} is missing"
        ));
    }
    if digest(&executable).as_deref() != Some(sha) {
        return refuse(format!(
            "the pinned {name} executable {shown_executable} no longer hashes to its pin"
        ));
    }
    if !PyPath::new(bytes(&executable)).is_file() || !executable_bit(&executable) {
        return refuse(format!(
            "the pinned {name} executable {shown_executable} is not executable"
        ));
    }
    Ok(Runtime {
        executable,
        receipt,
    })
}

/// `rust_runtime.HOST_TARGETS` for this build's platform.
fn host_target() -> Option<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => Some("x86_64-unknown-linux-gnu"),
        ("linux", "aarch64") => Some("aarch64-unknown-linux-gnu"),
        ("macos", "aarch64") => Some("aarch64-apple-darwin"),
        _ => None,
    }
}

/// The C library this host runs, as `platform.libc_ver` names glibc; none elsewhere.
fn host_libc() -> (Option<String>, Option<String>) {
    let reported = std::process::Command::new("getconf")
        .arg("GNU_LIBC_VERSION")
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()
        .filter(|done| done.status.success())
        .map(|done| String::from_utf8_lossy(&done.stdout).trim().to_string());
    match reported.as_deref().and_then(|text| text.split_once(' ')) {
        Some(("glibc", version)) => (Some("glibc".into()), Some(version.to_string())),
        _ => (None, None),
    }
}

fn version_numbers(text: &str) -> Vec<u64> {
    text.split(|c: char| !c.is_ascii_digit())
        .filter(|part| !part.is_empty())
        .map(|part| part.parse().unwrap_or(u64::MAX))
        .collect()
}

/// Why the receipt's executable may not run on this host.
fn host_problem(name: &str, receipt: &Value) -> Option<String> {
    let field = |value: &Value, key: &str| -> Value {
        value
            .as_object()
            .and_then(|object| object.get(key))
            .cloned()
            .unwrap_or(Value::Null)
    };
    let target = field(receipt, "target").py_str();
    let host = host_target();
    if Some(target.as_str()) != host {
        let here = host.map_or_else(
            || format!("{} {}", std::env::consts::OS, std::env::consts::ARCH),
            str::to_string,
        );
        return Some(format!(
            "{name} was built for {target}; this host is {here}"
        ));
    }
    let built = field(receipt, "libc");
    let built_name = field(&built, "name");
    if !built_name.truthy() {
        return None;
    }
    let built_version = field(&built, "version");
    let (libc, version) = host_libc();
    let older = version_numbers(version.as_deref().unwrap_or(""))
        < version_numbers(
            &built_version
                .as_str()
                .map(str::to_string)
                .unwrap_or_default(),
        );
    (libc.as_deref() != built_name.as_str() || older).then(|| {
        format!(
            "{name} links {} {}; this host has {} {}",
            built_name.py_str(),
            built_version.py_str(),
            libc.as_deref().unwrap_or("None"),
            version.as_deref().unwrap_or("None")
        )
    })
}

/// The tree at `path` in `commit`, or None when the commit or the path is not there.
pub fn crate_tree(repo: &Path, commit: &str, path: &str) -> Option<String> {
    let done = std::process::Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["rev-parse", "--verify", "--quiet"])
        .arg(format!("{commit}^{{commit}}:{path}"))
        .stdin(std::process::Stdio::null())
        .output()
        .ok()?;
    done.status
        .success()
        .then(|| String::from_utf8_lossy(&done.stdout).trim().to_string())
}

/// Why the executable was not built from the crate tree at `freeze_commit`.
pub fn source_problem(receipt: &Value, freeze_commit: Option<&str>, repo: &Path) -> Option<String> {
    let Some(commit) = freeze_commit.filter(|commit| !commit.is_empty()) else {
        return Some("the manifest has no freeze_commit to compare the crate with".into());
    };
    let field = |outer: &str, inner: &str| {
        receipt
            .as_object()
            .and_then(|receipt| receipt.get(outer))
            .and_then(Value::as_object)
            .and_then(|found| found.get(inner))
            .and_then(Value::as_str)
            .unwrap_or("")
    };
    let path = field("crate", "path");
    let Some(tree) = crate_tree(repo, commit, path) else {
        return Some(format!("freeze commit {commit} has no crate {path}"));
    };
    let built = field("source", "tree");
    (tree != built).then(|| {
        format!(
            "{} was built from crate tree {built}; freeze commit {commit} has {tree}",
            field("crate", "name")
        )
    })
}

fn executable_bit(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path).is_ok_and(|found| found.permissions().mode() & 0o111 != 0)
}
