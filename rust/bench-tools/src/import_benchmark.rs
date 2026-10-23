//! `tools/import_benchmark.py`'s `--check` and `--preserve`: verify the imported originals that
//! `bench/import-manifest.json` pins, and preserve an original before its current copy is edited.
//!
//! Unlike the Python, a preserved copy is written to a synced temporary file and linked to its name, never
//! replacing a file, and the manifest is replaced atomically, only after the new manifest verifies.

use crate::evidence::{self, Disk, Store};
use crate::pyjson::{self, Duplicates, Object, Value};
use crate::pytext::{self, PyPath};
use std::collections::{BTreeSet, HashMap};
use std::fmt::Write as _;
use std::io::{self, Write};
use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Stdio};

pub const MANIFEST: &str = "bench/import-manifest.json";
const PRESERVED: &[u8] = b"artifacts/import-source";
pub const PRESERVE_COMMAND: &str = "bun run import:preserve";

/// A refusal, printed after `bench-tools import-benchmark: ` on stderr with exit 1, the status Python's traceback had.
#[derive(Debug, PartialEq, Eq)]
pub struct Refusal(pub String);

type Result<T> = std::result::Result<T, Refusal>;

fn from_evidence(error: evidence::Error) -> Refusal {
    Refusal(error.message().to_string())
}

fn malformed(detail: &str) -> Refusal {
    Refusal(format!("invalid import manifest: {detail}"))
}

fn read_manifest(root: &PyPath) -> Result<Value> {
    let path = root.join(MANIFEST.as_bytes());
    let text = pytext::read_text(&path).map_err(|error| match error {
        pytext::ReadError::Os(message) => Refusal(message),
        pytext::ReadError::Decode(message) => malformed(&message),
    })?;
    pyjson::loads(&text, Duplicates::KeepLast).map_err(|error| malformed(&error.to_string()))
}

fn lists(manifest: &Value) -> Result<(&[Value], &[Value])> {
    let lists = manifest.as_object().and_then(|object| {
        Some((
            object.get("files")?.as_array()?,
            object.get("transcripts")?.as_array()?,
        ))
    });
    lists.ok_or_else(|| malformed("it must hold files and transcripts lists"))
}

fn entry_object(entry: &Value) -> Result<&Object> {
    entry
        .as_object()
        .ok_or_else(|| malformed("each entry must be an object"))
}

fn required<'a>(entry: &'a Object, key: &str) -> Result<&'a Value> {
    entry
        .get(key)
        .ok_or_else(|| malformed(&format!("an entry has no {key}")))
}

fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value
        .as_str()
        .ok_or_else(|| malformed(&format!("an entry's {key} must be a string")))
}

fn names_digest(sha256: &Value, actual: &str) -> bool {
    sha256.as_str() == Some(actual)
}

/// `import_benchmark.verify`: every entry not marked missing resolves, through the evidence manifests when it is
/// absent, to a file with its recorded hash. Returns the summary line.
pub fn verify(store: &mut Store<'_>, root: &PyPath, manifest: &Value) -> Result<String> {
    let (files, transcripts) = lists(manifest)?;
    let mut errors: Vec<String> = Vec::new();
    for entry in files.iter().chain(transcripts) {
        let entry = entry_object(entry)?;
        if entry.get("status").and_then(Value::as_str) == Some("missing") {
            continue;
        }
        let path = required(entry, "path")?;
        let logical = entry.get("preserved_path").unwrap_or(path);
        let sha256 = required(entry, "sha256")?;
        let logical = text(logical, "path")?;
        // Python passes a non-string hash on, so it matches no file; `None` skips resolve's own check.
        let wanted = match sha256 {
            Value::Str(sha256) => Some(sha256.as_str()),
            Value::Null => None,
            _ => Some(""),
        };
        match evidence::resolve(store, root, logical, wanted, false) {
            Err(evidence::Error::Violation(_)) => errors.push(logical.to_string()),
            Err(error) => return Err(from_evidence(error)),
            Ok(found) => {
                if !found.is_file()
                    || !names_digest(sha256, &evidence::digest(&found).map_err(from_evidence)?)
                {
                    errors.push(text(path, "path")?.to_string());
                }
            }
        }
    }
    let mut editable = Vec::new();
    for entry in files {
        let entry = entry_object(entry)?;
        let path = required(entry, "path")?;
        if let Some(path) = path.as_str()
            && errors.iter().any(|error| error == path)
            && entry.get("preserved_path").is_none()
            && !path.starts_with("bench/runs/")
        {
            editable.push(path);
        }
    }
    if !errors.is_empty() {
        let mut message = format!(
            "Missing or changed imported evidence: {}",
            errors.join(", ")
        );
        if !editable.is_empty() {
            let _ = write!(
                message,
                "\nTo keep an intended edit, preserve the imported bytes: {PRESERVE_COMMAND} {}",
                editable.join(" ")
            );
        }
        return Err(Refusal(message));
    }
    let mut missing = 0;
    let mut mismatches = 0;
    for item in transcripts {
        let status = entry_object(item)?
            .get("status")
            .ok_or_else(|| malformed("a transcript has no status"))?;
        missing += usize::from(status.as_str() == Some("missing"));
        mismatches += usize::from(status.as_str() == Some("mismatch"));
    }
    Ok(format!(
        "Verified copied bytes for {} source files and {} archives; {mismatches} superseded source-hash mismatches, {missing} unavailable",
        files.len(),
        transcripts.len() - missing
    ))
}

/// The `--check` of `tools/import_benchmark.py`.
pub fn check(store: &mut Store<'_>, root: &PyPath) -> Result<String> {
    let manifest = read_manifest(root)?;
    verify(store, root, &manifest)
}

fn git_failure(command: &[&str], error: &io::Error) -> Refusal {
    Refusal(pytext::os_error(error, command[0].as_bytes()))
}

/// `import_benchmark.imported_bytes`: the current file when it still has the recorded hash, else the first
/// commit, oldest first, whose copy has it.
fn imported_bytes(root: &PyPath, name: &str, sha256: &Value) -> Result<Vec<u8>> {
    let path = root.join(name.as_bytes());
    if path.is_file() {
        let data = std::fs::read(path.to_path_buf())
            .map_err(|error| Refusal(pytext::os_error(&error, path.as_bytes())))?;
        if names_digest(sha256, &crate::sha256::hex_digest(&data)) {
            return Ok(data);
        }
    }
    let shown_root = root.display();
    let log = [
        "git",
        "-C",
        shown_root.as_str(),
        "log",
        "--format=%H",
        "--",
        name,
    ];
    let listed = Command::new(log[0])
        .arg("-C")
        .arg(root.to_path_buf())
        .args(&log[3..])
        .stdin(Stdio::inherit())
        .stderr(Stdio::inherit())
        .output()
        .map_err(|error| git_failure(&log, &error))?;
    if !listed.status.success() {
        return Err(Refusal(evidence::remote::called_process_error(
            &log,
            listed.status,
        )));
    }
    let commits = String::from_utf8_lossy(&listed.stdout).into_owned();
    for commit in commits.split_whitespace().rev() {
        let object = format!("{commit}:{name}");
        let show = ["git", "-C", shown_root.as_str(), "show", object.as_str()];
        let shown = Command::new(show[0])
            .arg("-C")
            .arg(root.to_path_buf())
            .args(&show[3..])
            .stdin(Stdio::inherit())
            .output()
            .map_err(|error| git_failure(&show, &error))?;
        if shown.status.success() && names_digest(sha256, &crate::sha256::hex_digest(&shown.stdout))
        {
            return Ok(shown.stdout);
        }
    }
    Err(Refusal(format!(
        "Cannot recover the imported bytes from Git history: {name}"
    )))
}

fn os_refusal(error: &io::Error, path: &PyPath) -> Refusal {
    Refusal(pytext::os_error(error, path.as_bytes()))
}

fn temporary_beside(path: &PyPath) -> PyPath {
    let name = path
        .as_bytes()
        .rsplit(|byte| *byte == b'/')
        .next()
        .unwrap_or_default();
    evidence::parent(path).join(
        [
            b".",
            name,
            b".",
            evidence::random_name("").as_bytes(),
            b".tmp",
        ]
        .concat()
        .as_slice(),
    )
}

/// The directory that gained an entry and each of its ancestors, which an interrupted earlier call may have
/// created without syncing.
fn reaching(path: &PyPath) -> BTreeSet<Vec<u8>> {
    let directory = evidence::parent(path);
    std::iter::once(directory.clone())
        .chain(evidence::parents(&directory))
        .map(|ancestor| ancestor.as_bytes().to_vec())
        .collect()
}

/// `import_benchmark.write_checked`: an existing file must already hold `data`; otherwise `data` is written to a
/// synced temporary file and linked to `path`, never replacing a file that appeared meanwhile.
fn write_checked(disk: &mut dyn Disk, path: &PyPath, shown: &str, data: &[u8]) -> Result<()> {
    let same = |path: &PyPath| -> Result<()> {
        let existing =
            std::fs::read(path.to_path_buf()).map_err(|error| os_refusal(&error, path))?;
        if existing == data {
            Ok(())
        } else {
            Err(Refusal(format!(
                "Refusing to overwrite modified file: {shown}"
            )))
        }
    };
    let directories = reaching(path);
    if path.to_path_buf().exists() {
        evidence::sync_dirs(disk, &directories).map_err(from_evidence)?;
        return same(path);
    }
    evidence::make_dirs(&evidence::parent(path)).map_err(from_evidence)?;
    let temporary = temporary_beside(path);
    let os_temporary = temporary.to_path_buf();
    let written = (|| -> Result<()> {
        let mut file = disk
            .create(&os_temporary)
            .map_err(|error| os_refusal(&error, &temporary))?;
        file.write_all(data)
            .map_err(|error| os_refusal(&error, &temporary))?;
        disk.sync(&file, &os_temporary)
            .map_err(|error| os_refusal(&error, &temporary))?;
        match disk.link(&os_temporary, &path.to_path_buf()) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => same(path),
            Err(error) => Err(os_refusal(&error, path)),
        }
    })();
    let _ = std::fs::remove_file(&os_temporary);
    written?;
    evidence::sync_dirs(disk, &directories).map_err(from_evidence)
}

/// Replace the manifest with `text`, keeping its permission bits: a synced temporary file renamed over it, then
/// the directory synced, so an interruption leaves the old manifest or the new one.
fn replace(disk: &mut dyn Disk, path: &PyPath, text: &str) -> Result<()> {
    let mode = std::fs::metadata(path.to_path_buf())
        .map_err(|error| os_refusal(&error, path))?
        .permissions()
        .mode()
        & 0o7777;
    let temporary = temporary_beside(path);
    let os_temporary = temporary.to_path_buf();
    let written = (|| -> Result<()> {
        let mut file = disk
            .create(&os_temporary)
            .map_err(|error| os_refusal(&error, &temporary))?;
        file.write_all(text.as_bytes())
            .map_err(|error| os_refusal(&error, &temporary))?;
        file.set_permissions(std::fs::Permissions::from_mode(mode))
            .map_err(|error| os_refusal(&error, &temporary))?;
        disk.sync(&file, &os_temporary)
            .map_err(|error| os_refusal(&error, &temporary))?;
        disk.rename(&os_temporary, &path.to_path_buf())
            .map_err(|error| os_refusal(&error, path))
    })();
    if written.is_err() {
        let _ = std::fs::remove_file(&os_temporary);
    }
    written?;
    let directory = evidence::parent(path);
    disk.sync_dir(&directory.to_path_buf())
        .map_err(|error| os_refusal(&error, &directory))
}

/// The `--preserve NAME...` of `tools/import_benchmark.py`: copy each named file's imported bytes, from the current file or Git
/// history, under `artifacts/import-source/` and record that copy as its `preserved_path`. The manifest is
/// replaced only after the new manifest verifies. Returns the summary line.
pub fn preserve(store: &mut Store<'_>, root: &PyPath, names: &[Vec<u8>]) -> Result<String> {
    let mut manifest = read_manifest(root)?;
    let files = match &mut manifest {
        Value::Object(object) => match object.get_mut("files") {
            Some(Value::Array(files)) => files,
            _ => return Err(malformed("it must hold a files list")),
        },
        _ => return Err(malformed("it must hold a files list")),
    };
    let mut positions: HashMap<String, usize> = HashMap::new();
    for (position, entry) in files.iter().enumerate() {
        match required(entry_object(entry)?, "path")? {
            Value::Array(_) | Value::Object(_) => {
                return Err(malformed("an entry's path must be a string"));
            }
            Value::Str(path) => {
                positions.insert(path.clone(), position);
            }
            _ => {}
        }
    }
    for raw in names {
        let shown = pytext::stderr_text(raw);
        let Some(position) = std::str::from_utf8(raw)
            .ok()
            .and_then(|name| positions.get(name))
        else {
            return Err(Refusal(format!("not an imported file: {shown}")));
        };
        let Value::Object(entry) = &mut files[*position] else {
            unreachable!("every files entry was checked to be an object");
        };
        if entry.get("preserved_path").is_some() {
            continue;
        }
        let name = std::str::from_utf8(raw).unwrap_or_default();
        let logical = PyPath::new(PRESERVED).join(raw).display();
        let destination = evidence::confined(root, &logical).map_err(from_evidence)?;
        let data = imported_bytes(root, name, required(entry, "sha256")?)?;
        write_checked(store.disk, &destination, &logical, &data)?;
        entry.insert("preserved_path".to_string(), Value::Str(logical));
    }
    let summary = verify(store, root, &manifest)?;
    let text = format!(
        "{}\n",
        pyjson::dumps(
            &manifest,
            &pyjson::Dumps {
                indent: Some(2),
                ..pyjson::Dumps::default()
            }
        )
    );
    replace(store.disk, &root.join(MANIFEST.as_bytes()), &text)?;
    Ok(summary)
}
