//! The runner identity of a Rust-prepared key: the running `bench-tools` executable as an installed entry of a
//! runtime store, `<store>/bench-tools/<sha256>/<file>` beside its `receipt.json`, and the checks
//! [`crate::runtime_store`] runs on every pinned entry.

use crate::current_grading::{Failure, R};
use crate::pyjson::Value;
use crate::pytext::{self, PyPath};
use crate::{pyvalue, sha256};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

pub const NAME: &str = "bench-tools";
use crate::runtime_store::INSTALL;

fn bytes(path: &Path) -> &[u8] {
    pytext::os_bytes(path.as_os_str())
}

fn shown(path: &Path) -> String {
    pytext::stderr_text(bytes(path))
}

/// The file's SHA-256; the running executable is hashed once per process, since a debug build hashes slowly.
fn digest(path: &Path) -> Option<String> {
    static RUNNING: OnceLock<Option<String>> = OnceLock::new();
    let hashed = |path: &Path| std::fs::read(path).ok().map(|raw| sha256::hex_digest(&raw));
    let running = std::env::current_exe().and_then(|own| own.canonicalize());
    match (path.canonicalize(), running) {
        (Ok(found), Ok(own)) if found == own => RUNNING.get_or_init(|| hashed(&own)).clone(),
        _ => hashed(path),
    }
}

pub const RECEIPT: &str = "rust-runtime-receipt/1";

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

/// Why the receipt is not complete and portable. A missing or mistyped field is named by its path.
pub fn receipt_problem(receipt: &Value) -> Option<String> {
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

/// The pin of the running executable's store entry, `{"sha256", "receipt_sha256"}`.
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

/// The pinned executable once it and its receipt hash to the pin and, with `here`, it suits this host.
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

/// The supported hosts of docs/agents/scripts.md; .github/workflows/rust-runtime.yml exercises each one.
pub const SUPPORTED_TARGETS: [&str; 3] = [
    "aarch64-apple-darwin",
    "aarch64-unknown-linux-gnu",
    "x86_64-unknown-linux-gnu",
];

/// The target of this build's platform, when it is a supported host.
pub fn host_target() -> Option<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => Some("x86_64-unknown-linux-gnu"),
        ("linux", "aarch64") => Some("aarch64-unknown-linux-gnu"),
        ("macos", "aarch64") => Some("aarch64-apple-darwin"),
        _ => None,
    }
}

/// The C library this host runs, as `platform.libc_ver` names glibc; none elsewhere.
pub fn host_libc() -> (Option<String>, Option<String>) {
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

pub const DEVIATION: &str = "rust-runtime-deviation/1";

/// A run's Rust runtime pins: its `runtimes.frozen.json` (Null when it has none), with each pin moved by the
/// `rust-runtime-deviation/1` records in its `deviations/`, in `recorded_at` order. A record moves its `runtime` from
/// its `old` pin, which must be the pin at that point, to its `new` one, so a started run takes another runtime only
/// through a recorded change, and a later record can move it back. `moved` names the runtimes whose pin differs
/// from the frozen one: their crate comes from their own receipt's commit rather than the run's freeze commit.
pub struct RunPins {
    pub pins: Value,
    pub moved: Vec<String>,
}

pub fn run_pins(run: &Path) -> Result<RunPins, String> {
    let read = |path: &Path| {
        let raw = std::fs::read(path).map_err(|error| format!("{}: {error}", shown(path)))?;
        crate::pyjson::loads(
            &String::from_utf8_lossy(&raw),
            crate::pyjson::Duplicates::KeepLast,
        )
        .map_err(|error| format!("{}: {error}", shown(path)))
    };
    let frozen_path = run.join("runtimes.frozen.json");
    if !frozen_path.is_file() {
        return Ok(RunPins {
            pins: Value::Null,
            moved: Vec::new(),
        });
    }
    let frozen = read(&frozen_path)?;
    let mut records = Vec::new();
    let paths = std::fs::read_dir(run.join("deviations"))
        .map(|entries| entries.flatten().map(|entry| entry.path()).collect())
        .unwrap_or_else(|_| Vec::new());
    for path in paths {
        if path
            .extension()
            .is_some_and(|extension| extension == "json")
            && let Ok(row) = read(&path)
            && text_at(&row, &["schema"]) == Some(DEVIATION)
        {
            let at = text_at(&row, &["recorded_at"]).unwrap_or("").to_string();
            records.push((at, path, row));
        }
    }
    records.sort_by(|left, right| (&left.0, &left.1).cmp(&(&right.0, &right.1)));
    let mut pins = frozen.clone();
    if let Value::Object(object) = &mut pins {
        while !records.is_empty() {
            // `recorded_at` has whole seconds, so of the records made in one second, the one that starts from the
            // pin then held goes first.
            let first = records[0].0.clone();
            let follows = records.iter().position(|(at, _, row)| {
                *at == first
                    && text_at(row, &["runtime"])
                        .and_then(|name| object.get(name))
                        .zip(value_at(row, &["old", "pin"]))
                        .is_some_and(|(pin, old)| old.py_eq(pin))
            });
            let (_, path, row) = records.remove(follows.unwrap_or(0));
            let name = text_at(&row, &["runtime"]).unwrap_or("");
            let (Some(pin), Some(old), Some(new)) = (
                object.get_mut(name),
                value_at(&row, &["old", "pin"]),
                value_at(&row, &["new", "pin"]),
            ) else {
                return Err(format!(
                    "{} moves no runtime the run pins from an old pin to a new one",
                    shown(&path)
                ));
            };
            if !old.py_eq(pin) {
                return Err(format!(
                    "{} moves {name} from a pin other than the one the run held then",
                    shown(&path)
                ));
            }
            *pin = new.clone();
        }
    }
    let moved = match (&pins, &frozen) {
        (Value::Object(now), Value::Object(then)) => now
            .iter()
            .filter(|(name, pin)| !then.get(name).is_some_and(|was| was.py_eq(pin)))
            .map(|(name, _)| name.clone())
            .collect(),
        _ => Vec::new(),
    };
    Ok(RunPins { pins, moved })
}

/// The commit a pinned runtime's crate must come from: the run's freeze commit, or for a pin a runtime deviation
/// moved, the revision its own receipt was built from.
pub fn source_commit<'a>(receipt: &'a Value, moved: bool, freeze_commit: &'a str) -> &'a str {
    if moved {
        text_at(receipt, &["source", "revision"]).unwrap_or("")
    } else {
        freeze_commit
    }
}

fn value_at<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    path.iter()
        .try_fold(value, |found, key| found.as_object()?.get(key))
}

fn text_at<'a>(value: &'a Value, path: &[&str]) -> Option<&'a str> {
    value_at(value, path).and_then(Value::as_str)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Run(PathBuf);

    impl Drop for Run {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    impl Run {
        fn new(frozen: Option<&str>) -> Self {
            let run = Self(std::env::temp_dir().join(crate::evidence::random_name("run-pins-")));
            std::fs::create_dir_all(run.0.join("deviations")).unwrap();
            if let Some(frozen) = frozen {
                std::fs::write(run.0.join("runtimes.frozen.json"), frozen).unwrap();
            }
            run
        }

        fn deviation(&self, file: &str, at: &str, old: &str, new: &str) {
            std::fs::write(
                self.0.join("deviations").join(file),
                format!(
                    r#"{{"schema": "{DEVIATION}", "runtime": "bench-tools", "recorded_at": "{at}",
                    "old": {{"pin": {{"sha256": "{old}", "receipt_sha256": "r{old}"}}}},
                    "new": {{"pin": {{"sha256": "{new}", "receipt_sha256": "r{new}"}}}}}}"#
                ),
            )
            .unwrap();
        }

        fn pinned(&self) -> Result<(String, Vec<String>), String> {
            let found = run_pins(&self.0)?;
            Ok((
                text_at(&found.pins, &[NAME, "sha256"])
                    .unwrap_or("none")
                    .to_string(),
                found.moved,
            ))
        }
    }

    const FROZEN: &str = r#"{"bench-tools": {"sha256": "a", "receipt_sha256": "ra"}}"#;

    #[test]
    fn a_moved_pin_comes_from_its_receipts_revision_and_a_frozen_one_from_the_freeze_commit() {
        let receipt = crate::pyjson::loads(
            r#"{"source": {"revision": "abc", "tree": "def"}}"#,
            crate::pyjson::Duplicates::KeepLast,
        )
        .unwrap();
        assert_eq!(source_commit(&receipt, true, "f00"), "abc");
        assert_eq!(source_commit(&receipt, false, "f00"), "f00");
    }

    #[test]
    fn a_run_takes_its_frozen_pin_until_a_recorded_deviation_moves_it_in_order() {
        assert!(matches!(
            run_pins(&Run::new(None).0).unwrap().pins,
            Value::Null
        ));
        let run = Run::new(Some(FROZEN));
        std::fs::write(run.0.join("deviations/notes.json"), "not json").unwrap();
        std::fs::write(
            run.0.join("deviations/clone-write.att-001.v1.json"),
            r#"{"predecessor": "att-001", "cause": "harness-stop"}"#,
        )
        .unwrap();
        assert_eq!(run.pinned().unwrap(), ("a".to_string(), vec![]));
        run.deviation("z.json", "2026-01-01T00:00:00Z", "a", "b");
        assert_eq!(
            run.pinned().unwrap(),
            ("b".to_string(), vec!["bench-tools".to_string()])
        );
        // Applied by recorded_at, not file name; moving back to the frozen pin is not a move.
        run.deviation("a.json", "2026-01-02T00:00:00Z", "b", "a");
        assert_eq!(run.pinned().unwrap(), ("a".to_string(), vec![]));
        // Two records made in the same second chain whatever their file names.
        run.deviation("m.json", "2026-01-03T00:00:00Z", "c", "d");
        run.deviation("n.json", "2026-01-03T00:00:00Z", "a", "c");
        assert_eq!(
            run.pinned().unwrap(),
            ("d".to_string(), vec!["bench-tools".to_string()])
        );
    }

    #[test]
    fn a_deviation_from_a_pin_the_run_no_longer_holds_is_refused() {
        let run = Run::new(Some(FROZEN));
        run.deviation("one.json", "2026-01-01T00:00:00Z", "a", "b");
        run.deviation("two.json", "2026-01-02T00:00:00Z", "a", "c");
        let refused = run.pinned().unwrap_err();
        assert!(
            refused.contains(
                "two.json moves bench-tools from a pin other than the one the run held then"
            ),
            "{refused}"
        );
        std::fs::remove_file(run.0.join("deviations/two.json")).unwrap();
        std::fs::write(
            run.0.join("deviations/other.json"),
            format!(r#"{{"schema": "{DEVIATION}", "runtime": "other"}}"#),
        )
        .unwrap();
        let refused = run.pinned().unwrap_err();
        assert!(
            refused.contains("other.json moves no runtime the run pins"),
            "{refused}"
        );
    }
}
