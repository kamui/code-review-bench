//! `tools/collect_run.py`: map each attempt and probe record of a run to its transcript archive's repository path
//! in `transcripts.json`, verifying every archive it maps. A record named missing keeps its frozen hash with status
//! `missing` until its archive verifies.
//!
//! Unlike the Python, `transcripts.json` is replaced atomically and a copied archive is linked into place from a
//! synced temporary file, so neither is ever written through a link or left partial. The bytes written to that
//! temporary file are hashed as they are copied, so an archive that changes after it verified is refused as a
//! checksum mismatch rather than linked.

use super::{
    Store, create_new, logical_path as safe_logical_path, make_dirs, manifests, parents, replace,
    resolve, validate,
};
use crate::pyfs;
use crate::pyjson::{self, Duplicates, Value};
use crate::pytext::{self, PyPath};
use crate::sha256::{self, Sha256};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;

/// A failure, as the exception Python raised.
#[derive(Debug, PartialEq, Eq)]
pub enum Failure {
    /// A `ValueError`, such as an unavailable or changed archive, which a record named missing records instead.
    Value(Vec<u8>),
    /// Any other exception.
    Other(Vec<u8>),
}

impl Failure {
    pub fn message(&self) -> &[u8] {
        match self {
            Self::Value(message) | Self::Other(message) => message,
        }
    }
}

type Result<T> = std::result::Result<T, Failure>;

fn other(message: impl Into<Vec<u8>>) -> Failure {
    Failure::Other(message.into())
}

fn from_evidence(error: super::Error) -> Failure {
    match error {
        super::Error::Violation(message) => Failure::Value(message.into_bytes()),
        super::Error::Io(message) => Failure::Other(message.into_bytes()),
    }
}

/// `PurePosixPath(text).parts`.
fn parts(text: &str) -> Vec<&str> {
    let root = if text.starts_with("//") && !text.starts_with("///") {
        Some("//")
    } else if text.starts_with('/') {
        Some("/")
    } else {
        None
    };
    root.into_iter()
        .chain(
            text.split('/')
                .filter(|part| !part.is_empty() && *part != "."),
        )
        .collect()
}

fn name(path: &PyPath) -> &[u8] {
    path.as_bytes()
        .rsplit(|byte| *byte == b'/')
        .next()
        .unwrap_or_default()
}

fn subscript<'a>(value: &'a Value, key: &str) -> Result<&'a Value> {
    match value {
        Value::Object(object) => object.get(key).ok_or_else(|| other(pytext::repr_str(key))),
        Value::Array(_) => Err(other("list indices must be integers or slices, not str")),
        Value::Str(_) => Err(other("string indices must be integers, not 'str'")),
        value => Err(other(format!(
            "'{}' object is not subscriptable",
            value.type_name()
        ))),
    }
}

fn recorded_text(recorded: &Value) -> Result<&str> {
    recorded.as_str().ok_or_else(|| {
        other(format!(
            "argument should be a str or an os.PathLike object where __fspath__ returns a str, not '{}'",
            recorded.type_name()
        ))
    })
}

/// `collect_run.logical_path`: the archive's repository path, the one its record holds or names inside any
/// checkout, else the collector's own under `artifacts/transcripts/<run>/`.
fn logical_path(run: &PyPath, record: &PyPath, recorded: &Value) -> Result<Vec<u8>> {
    let recorded = recorded_text(recorded)?;
    let parts = parts(recorded);
    let first = parts
        .first()
        .ok_or_else(|| other("tuple index out of range"))?;
    if !recorded.starts_with('/') && *first != "~" && safe_logical_path(recorded).is_ok() {
        return Ok(recorded.as_bytes().to_vec());
    }
    for start in (0..parts.len().saturating_sub(1)).rev() {
        if parts[start..start + 2] == ["artifacts", "transcripts"] {
            let path = parts[start..].join("/");
            safe_logical_path(&path).map_err(from_evidence)?;
            return Ok(path.into_bytes());
        }
    }
    let attempt = parents(record)
        .into_iter()
        .next()
        .unwrap_or_else(|| PyPath::new(b"."));
    let layer = parents(&attempt)
        .into_iter()
        .next()
        .unwrap_or_else(|| PyPath::new(b"."));
    let mut path = PyPath::new(b"artifacts/transcripts").join(name(run));
    if name(&layer) != b"attempts" {
        path = path.join(name(&layer));
    }
    Ok(path
        .join(&[name(&attempt), b".tar.gz"].concat())
        .as_bytes()
        .to_vec())
}

/// `Path(text).expanduser()`. Only `~` itself is expanded, from `HOME`; another user's home is refused as Python
/// refuses an unknown user's.
fn expanduser(text: &str) -> Result<PyPath> {
    let path = PyPath::new(text.as_bytes());
    if text.starts_with('/') {
        return Ok(path);
    }
    let parts = parts(text);
    let Some(first) = parts.first().filter(|first| first.starts_with('~')) else {
        return Ok(path);
    };
    let unknown = || other("Could not determine home directory.");
    if *first != "~" {
        return Err(unknown());
    }
    let home = std::env::var_os("HOME").ok_or_else(unknown)?;
    let home = pytext::os_bytes(&home);
    let trimmed = match home.iter().rposition(|byte| *byte != b'/') {
        Some(end) => &home[..=end],
        None => b"/",
    };
    if trimmed.starts_with(b"~") {
        return Err(unknown());
    }
    Ok(PyPath::new(trimmed).join(parts[1..].join("/").as_bytes()))
}

fn sha256_of(path: &PyPath) -> Result<String> {
    let mut file = std::fs::File::open(path.to_path_buf())
        .map_err(|error| other(pytext::os_error(&error, path.as_bytes())))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 1024 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| other(pytext::os_error(&error, path.as_bytes())))?;
        if read == 0 {
            return Ok(sha256::hex(&hasher.finalize()));
        }
        hasher.update(&buffer[..read]);
    }
}

/// Release storage: the logical paths each member hash is stored under.
struct Stored {
    by_sha: HashMap<String, BTreeSet<String>>,
    paths: HashSet<String>,
}

impl Stored {
    fn holds(&self, logical: &[u8]) -> bool {
        std::str::from_utf8(logical).is_ok_and(|logical| self.paths.contains(logical))
    }
}

/// `collect_run.verified_path`: verify the record's archive, place it at its repository path and return that path.
fn verified_path(
    store: &mut Store<'_>,
    root: &PyPath,
    run: &PyPath,
    record: &PyPath,
    archive: &Value,
    stored: &Stored,
) -> Result<Vec<u8>> {
    let recorded = subscript(archive, "path")?;
    let mut logical = logical_path(run, record, recorded)?;
    let sha = subscript(archive, "sha256")?;
    if !sha.hashable() {
        return Err(other(format!("unhashable type: '{}'", sha.type_name())));
    }
    let same_bytes = sha
        .as_str()
        .and_then(|sha| stored.by_sha.get(sha))
        .cloned()
        .unwrap_or_default();
    if !root.join(&logical).is_file() && !stored.holds(&logical) && same_bytes.len() == 1 {
        logical = same_bytes
            .into_iter()
            .next()
            .unwrap_or_default()
            .into_bytes();
    }
    let destination = root.join(&logical);
    let source = if destination.is_file() {
        destination.clone()
    } else if stored.holds(&logical) {
        let name = String::from_utf8_lossy(&logical).into_owned();
        // A frozen hash that is not text never equals a digest; Python compared it and found it changed.
        let expected = match sha {
            Value::Str(sha) => Some(sha.as_str()),
            Value::Null => None,
            _ => Some(""),
        };
        resolve(store, root, &name, expected, false).map_err(from_evidence)?
    } else {
        root.join(expanduser(recorded_text(recorded)?)?.as_bytes())
    };
    if !source.is_file() {
        return Err(Failure::Value(
            [
                &b"Transcript archive unavailable: "[..],
                &logical,
                b" for ",
                record.as_bytes(),
            ]
            .concat(),
        ));
    }
    let mismatch =
        || Failure::Value([&b"Transcript checksum mismatch: "[..], record.as_bytes()].concat());
    if sha.as_str() != Some(sha256_of(&source)?.as_str()) {
        return Err(mismatch());
    }
    let directory = parents(&destination)
        .into_iter()
        .next()
        .unwrap_or_else(|| PyPath::new(b"."));
    make_dirs(&directory).map_err(from_evidence)?;
    if pyfs::resolve(source.as_bytes()) != pyfs::resolve(destination.as_bytes()) {
        let mut file = std::fs::File::open(source.to_path_buf())
            .map_err(|error| other(pytext::os_error(&error, source.as_bytes())))?;
        let mut changed = false;
        let copied = create_new(store.disk, &destination, &mut |target| {
            let mut hasher = Sha256::new();
            let mut buffer = vec![0u8; 1024 * 1024];
            loop {
                let read = match file.read(&mut buffer) {
                    Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                    read => read?,
                };
                if read == 0 {
                    break;
                }
                hasher.update(&buffer[..read]);
                target.write_all(&buffer[..read])?;
            }
            changed = sha.as_str() != Some(sha256::hex(&hasher.finalize()).as_str());
            if changed {
                return Err(std::io::Error::other(
                    "the archive changed while it was copied",
                ));
            }
            Ok(())
        });
        if changed {
            return Err(mismatch());
        }
        copied.map_err(from_evidence)?;
    }
    Ok(logical)
}

/// `run/attempts/*/attempt.json` and `run/probes/*/attempt.json`, as Python 3.14's `glob` lists them, in path order.
fn records(run: &PyPath) -> Vec<PyPath> {
    let mut found: Vec<PyPath> = ["attempts", "probes"]
        .iter()
        .flat_map(|layer| {
            let directory = run.join(layer.as_bytes());
            pyfs::names(directory.as_bytes())
                .into_iter()
                .map(move |name| directory.join(&name))
        })
        .filter(|directory| pyfs::is_dir(directory.as_bytes()))
        .map(|directory| directory.join(b"attempt.json"))
        .filter(|path| pyfs::lexists(path.as_bytes()))
        .collect();
    found.sort_by_cached_key(|path| pytext::path_order(path.as_bytes()));
    found
}

fn read_json(path: &PyPath) -> Result<Value> {
    let text = pytext::read_text(path).map_err(|error| other(error.message().as_bytes()))?;
    pyjson::loads(&text, Duplicates::KeepLast).map_err(|error| other(error.to_string()))
}

fn text(value: &str) -> Value {
    Value::Str(value.to_string())
}

/// `collect_run.collect(Path(run).resolve(), missing)` with `ROOT` the repository at `root`: write
/// `transcripts.json` and return the line it prints.
pub fn collect(
    store: &mut Store<'_>,
    root: &PyPath,
    run: &[u8],
    missing: &[Vec<u8>],
) -> Result<Vec<u8>> {
    let run = PyPath::new(&pyfs::resolve(run));
    let mut stored = Stored {
        by_sha: HashMap::new(),
        paths: HashSet::new(),
    };
    for (_, document) in manifests(root).map_err(from_evidence)? {
        for package in validate(&document, true).map_err(from_evidence)?.packages {
            for member in package.members {
                stored.paths.insert(member.path.clone());
                stored
                    .by_sha
                    .entry(member.sha256)
                    .or_default()
                    .insert(member.path);
            }
        }
    }
    let records = records(&run);
    let mapping = run.join(b"transcripts.json");
    let mut lost: Vec<PyPath> = missing
        .iter()
        .map(|name| run.join(name).join(b"attempt.json"))
        .collect();
    if mapping.is_file() {
        let Value::Array(entries) = read_json(&mapping)? else {
            return Err(other("transcripts.json is not a list"));
        };
        for entry in &entries {
            if subscript(entry, "status")?.as_str() == Some("missing") {
                lost.push(root.join(recorded_text(subscript(entry, "attempt")?)?.as_bytes()));
            }
        }
    }
    let mut absent: Vec<&PyPath> = lost.iter().filter(|path| !records.contains(path)).collect();
    if !absent.is_empty() {
        absent.sort_by_cached_key(|path| pytext::surrogateescape(path.as_bytes()));
        absent.dedup();
        let listed: Vec<String> = absent
            .iter()
            .map(|path| pytext::repr(&pytext::surrogateescape(path.as_bytes())))
            .collect();
        return Err(Failure::Value(
            format!("No such record to mark missing: [{}]", listed.join(", ")).into_bytes(),
        ));
    }
    let mut entries = Vec::new();
    for path in &records {
        let document = read_json(path)?;
        let Value::Object(object) = &document else {
            return Err(other(format!(
                "'{}' object has no attribute 'get'",
                document.type_name()
            )));
        };
        let Some(archive) = object
            .get("transcript_archive")
            .filter(|archive| archive.truthy())
        else {
            continue;
        };
        let attempt = pyfs::relative_to(path.as_bytes(), root.as_bytes()).map_err(other)?;
        let sha = subscript(archive, "sha256")?.clone();
        let (logical, status) = match verified_path(store, root, &run, path, archive, &stored) {
            Ok(logical) => (logical, "verified"),
            Err(Failure::Value(_)) if lost.contains(path) => (
                logical_path(&run, path, subscript(archive, "path")?)?,
                "missing",
            ),
            Err(failure) => return Err(failure),
        };
        entries.push(Value::Object(
            [
                ("attempt", Value::os_text(&attempt)),
                ("path", Value::os_text(&logical)),
                ("sha256", sha),
                ("status", text(status)),
            ]
            .into_iter()
            .map(|(key, value)| (key.to_string(), value))
            .collect(),
        ));
    }
    let encoded = format!(
        "{}\n",
        pyjson::dumps(
            &Value::Array(entries.clone()),
            &pyjson::Dumps {
                indent: Some(2),
                ..pyjson::Dumps::default()
            }
        )
    );
    let mode = std::fs::metadata(mapping.to_path_buf())
        .ok()
        .filter(std::fs::Metadata::is_file)
        .map(|metadata| metadata.permissions().mode() & 0o7777);
    replace(store.disk, &mapping, encoded.as_bytes(), mode).map_err(from_evidence)?;
    let verified = entries
        .iter()
        .filter(|entry| matches!(entry, Value::Object(object) if object.get("status").and_then(Value::as_str) == Some("verified")))
        .count();
    let mut line = format!("Collected {verified} verified transcripts for ").into_bytes();
    line.extend_from_slice(name(&run));
    if verified < entries.len() {
        line.extend_from_slice(
            format!("; {} recorded as missing", entries.len() - verified).as_bytes(),
        );
    }
    Ok(line)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(raw: &str) -> PyPath {
        PyPath::new(raw.as_bytes())
    }

    fn logical(record: &str, recorded: &str) -> Result<String> {
        logical_path(
            &path("/repo/bench/runs/test"),
            &path(record),
            &text(recorded),
        )
        .map(|bytes| String::from_utf8(bytes).unwrap())
    }

    #[test]
    fn logical_paths_match_collect_run() {
        let attempt = "/repo/bench/runs/test/attempts/att-001/attempt.json";
        let probe = "/repo/bench/runs/test/probes/att-002/attempt.json";
        assert_eq!(
            logical(attempt, "artifacts/transcripts/reviews/test/att-001.tar.gz").unwrap(),
            "artifacts/transcripts/reviews/test/att-001.tar.gz"
        );
        assert_eq!(
            logical(attempt, "artifacts/custom/test/att-001.tar.gz").unwrap(),
            "artifacts/custom/test/att-001.tar.gz"
        );
        assert_eq!(
            logical(
                attempt,
                "~/gone/checkout/artifacts/transcripts/test/att-001.tar.gz"
            )
            .unwrap(),
            "artifacts/transcripts/test/att-001.tar.gz"
        );
        assert_eq!(
            logical(
                probe,
                "/gone/artifacts/transcripts/x/artifacts/transcripts/y.tar.gz"
            )
            .unwrap(),
            "artifacts/transcripts/y.tar.gz"
        );
        assert_eq!(
            logical(attempt, "local-archive/test/att-001.tar.gz").unwrap(),
            "artifacts/transcripts/test/att-001.tar.gz"
        );
        assert_eq!(
            logical(probe, "/cache/att-002.tar.gz").unwrap(),
            "artifacts/transcripts/test/probes/att-002.tar.gz"
        );
        assert_eq!(
            logical(attempt, "/gone/artifacts/transcripts/../../escape.tar.gz"),
            Err(Failure::Value(
                b"unsafe evidence path: artifacts/transcripts/../../escape.tar.gz".to_vec()
            ))
        );
        assert_eq!(logical(attempt, ""), Err(other("tuple index out of range")));
    }

    #[test]
    fn only_the_current_users_home_expands() {
        let home = std::env::var_os("HOME").map(|home| home.into_encoded_bytes());
        if let Some(home) = home.filter(|home| home.starts_with(b"/")) {
            let expected = PyPath::new(&home).join(b"a/b");
            assert_eq!(expanduser("~/a/b").unwrap(), expected);
        }
        assert_eq!(expanduser("/x/~/y").unwrap(), path("/x/~/y"));
        assert_eq!(expanduser("a/~").unwrap(), path("a/~"));
        assert_eq!(
            expanduser("~root/a"),
            Err(other("Could not determine home directory."))
        );
    }
}
