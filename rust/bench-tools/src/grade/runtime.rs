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

/// The problem, if any, with a v2 key's pinned executable: `rust_runtime.check` on the store that holds the running
/// executable, without its host checks, and then that the running executable is the pinned
/// one.
pub fn problem(pinned: &Value, executable: &Path) -> Option<String> {
    let text = |name: &str| {
        pinned
            .as_object()
            .and_then(|pin| pin.get(name))
            .and_then(Value::as_str)
            .unwrap_or("")
    };
    let (sha, receipt_sha) = (text("sha256"), text("receipt_sha256"));
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
    let directory: PathBuf = store.join(NAME).join(sha);
    let receipt_path = directory.join("receipt.json");
    let shown_receipt = shown(&receipt_path);
    let raw = match std::fs::read(&receipt_path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Some(format!("{shown_receipt} is missing"));
        }
        Err(error) => {
            return Some(format!(
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
        return Some(format!("{shown_receipt} is not a JSON object"));
    };
    if sha256::hex_digest(&raw) != receipt_sha {
        return Some(format!("{shown_receipt} no longer hashes to its pin"));
    }
    if let Some(problem) = receipt_problem(&receipt) {
        return Some(format!("{shown_receipt}: {problem}"));
    }
    let (crate_name, artifact, file) = receipt_names(&receipt);
    if crate_name != Some(NAME) || artifact != Some(sha) {
        return Some(format!(
            "{shown_receipt} describes {} {}, not the pinned {NAME}",
            crate_name.unwrap_or("None"),
            artifact.unwrap_or("None")
        ));
    }
    let pinned_executable = directory.join(file.unwrap_or(""));
    let shown_executable = shown(&pinned_executable);
    if std::fs::symlink_metadata(&pinned_executable).map_or(true, |found| !found.is_file()) {
        return Some(format!(
            "the pinned {NAME} executable {shown_executable} is missing"
        ));
    }
    if digest(&pinned_executable).as_deref() != Some(sha) {
        return Some(format!(
            "the pinned {NAME} executable {shown_executable} no longer hashes to its pin"
        ));
    }
    if !PyPath::new(bytes(&pinned_executable)).is_file() || !executable_bit(&pinned_executable) {
        return Some(format!(
            "the pinned {NAME} executable {shown_executable} is not executable"
        ));
    }
    if pinned_executable != executable {
        return Some(format!(
            "the running {NAME} {} is not the pinned executable {shown_executable}",
            shown(executable)
        ));
    }
    None
}

fn executable_bit(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path).is_ok_and(|found| found.permissions().mode() & 0o111 != 0)
}
