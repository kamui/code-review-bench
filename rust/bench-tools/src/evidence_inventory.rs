//! The storage inventory, as `evidence_inventory.py` takes it, and its reconciliation with published evidence
//! identities, as `reconcile_evidence.py` did. Neither ever authorizes retirement.
//!
//! Paths are bytes. A name that is not UTF-8 is written as Python writes its `surrogateescape` text, and names
//! sort as those texts sort, by code point.

pub mod reconcile;

use crate::evidence::{self, Disk};
use crate::pyjson::{self, Value};
use crate::pytext::{self, PyPath};
use crate::sha256::{self, Sha256};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::Read;
use std::os::unix::fs::MetadataExt;
use std::process::{Command, Stdio};

const CREDENTIALS: [&[u8]; 2] = [b"home/.codex/auth.json", b"home/.claude/.credentials.json"];
const GENERATED: [&[u8]; 6] = [
    b"node_modules",
    b"dist",
    b".output",
    b".vite",
    b"__pycache__",
    b"clone-cache",
];

/// How `evidence_inventory.main` reports a failure.
#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    /// A `ValueError`, printed to stdout with exit 1. Python prints a path that is not UTF-8 as its bytes.
    Violation(Vec<u8>),
    /// An `OSError` or `SubprocessError`, printed to stderr with exit 2.
    Io(String),
}

type Result<T> = std::result::Result<T, Error>;

/// Git's index by path, each path's blob SHA-1.
type Index = HashMap<Vec<u8>, String>;

/// A directory's names: those that are directories, following links, and the rest.
type Names = (Vec<Vec<u8>>, Vec<Vec<u8>>);

fn violation(message: impl Into<Vec<u8>>) -> Error {
    Error::Violation(message.into())
}

fn os_failure(error: &std::io::Error, path: &[u8]) -> Error {
    Error::Io(pytext::os_error(error, path))
}

/// `Path(raw).absolute()`: joined to the current directory, without resolving links or `..`.
pub fn absolute(raw: &[u8]) -> Result<PyPath> {
    let path = PyPath::new(raw);
    if path.as_bytes().starts_with(b"/") {
        return Ok(path);
    }
    let cwd = std::env::current_dir().map_err(|error| os_failure(&error, b"."))?;
    Ok(PyPath::new(pytext::os_bytes(cwd.as_os_str())).join(path.as_bytes()))
}

fn parts(path: &[u8]) -> Vec<&[u8]> {
    path.split(|byte| *byte == b'/')
        .filter(|part| !part.is_empty())
        .collect()
}

/// `PurePath.is_relative_to`, which compares parts without touching the filesystem.
fn is_relative_to(path: &PyPath, base: &PyPath) -> bool {
    let (inner, outer) = (parts(path.as_bytes()), parts(base.as_bytes()));
    path.as_bytes().starts_with(b"/") == base.as_bytes().starts_with(b"/")
        && inner.len() >= outer.len()
        && inner[..outer.len()] == outer[..]
}

fn is_symlink(path: &[u8]) -> bool {
    std::fs::symlink_metadata(PyPath::new(path).to_path_buf())
        .is_ok_and(|metadata| metadata.file_type().is_symlink())
}

/// What `git` printed, decoded as `subprocess` decodes text output, or `None` when it exited non-zero.
fn git_text(root: &PyPath, arguments: &[&str]) -> Result<Option<String>> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root.to_path_buf())
        .args(arguments)
        .stdin(Stdio::inherit())
        .output()
        .map_err(|error| os_failure(&error, b"git"))?;
    let stdout = pytext::decode_utf8(&output.stdout).map_err(violation)?;
    pytext::decode_utf8(&output.stderr).map_err(violation)?;
    Ok(output
        .status
        .success()
        .then(|| pytext::strip(&pytext::universal_newlines(&stdout)).to_string()))
}

/// Git's index of `root` by path, and its `HEAD`, when `root` is the top of a work tree with a commit.
fn tracked(root: &PyPath) -> Result<(Index, Option<String>)> {
    let Some(top) = git_text(root, &["rev-parse", "--show-toplevel"])? else {
        return Ok((HashMap::new(), None));
    };
    if crate::pyfs::resolve(PyPath::new(top.as_bytes()).as_bytes()) != root.as_bytes() {
        return Ok((HashMap::new(), None));
    }
    let listed = Command::new("git")
        .arg("-C")
        .arg(root.to_path_buf())
        .args(["ls-files", "--stage", "-z"])
        .stdin(Stdio::inherit())
        .stderr(Stdio::inherit())
        .output()
        .map_err(|error| os_failure(&error, b"git"))?;
    if !listed.status.success() {
        return Ok((HashMap::new(), None));
    }
    let mut index = HashMap::new();
    for entry in listed
        .stdout
        .split(|byte| *byte == 0)
        .filter(|entry| !entry.is_empty())
    {
        let Some(tab) = entry.iter().position(|byte| *byte == b'\t') else {
            return Err(violation("not enough values to unpack (expected 2, got 1)"));
        };
        let metadata = pytext::decode_utf8(&entry[..tab]).map_err(violation)?;
        let fields: Vec<&str> = metadata
            .split(pytext::is_space)
            .filter(|field| !field.is_empty())
            .collect();
        let [_, sha, stage] = fields[..] else {
            return Err(violation(if fields.len() > 3 {
                format!(
                    "too many values to unpack (expected 3, got {})",
                    fields.len()
                )
            } else {
                format!(
                    "not enough values to unpack (expected 3, got {})",
                    fields.len()
                )
            }));
        };
        if stage != "0" {
            return Err(violation("unmerged index"));
        }
        index.insert(entry[tab + 1..].to_vec(), sha.to_string());
    }
    match git_text(root, &["rev-parse", "HEAD"])? {
        Some(head) => Ok((index, Some(head))),
        None => Ok((HashMap::new(), None)),
    }
}

/// The SHA-256 of a file and its Git blob SHA-1, with `size` as the blob header's length.
fn identities(path: &[u8], size: u64) -> Result<(String, String)> {
    let mut file = std::fs::File::open(PyPath::new(path).to_path_buf())
        .map_err(|error| os_failure(&error, path))?;
    let mut sha = Sha256::new();
    let mut blob = sha1_smol::Sha1::new();
    blob.update(format!("blob {size}\0").as_bytes());
    let mut buffer = vec![0u8; 1024 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| os_failure(&error, path))?;
        if read == 0 {
            break;
        }
        sha.update(&buffer[..read]);
        blob.update(&buffer[..read]);
    }
    Ok((sha256::hex(&sha.finalize()), blob.digest().to_string()))
}

/// `evidence_inventory.classification`, on a relative path's bytes, which match its ASCII rules exactly as its
/// `surrogateescape` text does.
fn classification(
    name: &[u8],
    indexed: &HashMap<Vec<u8>, String>,
    blob: Option<&str>,
) -> (&'static str, &'static str) {
    let names: Vec<&[u8]> = name.split(|byte| *byte == b'/').collect();
    if let Some(sha) = indexed.get(name) {
        return if blob == Some(sha.as_str()) {
            ("tracked-identical", "matches Git index")
        } else {
            ("development", "differs from Git index")
        };
    }
    if CREDENTIALS
        .iter()
        .any(|entry| name == *entry || name.ends_with(&[b"/", *entry].concat()))
    {
        return ("account-state", "authentication; exclude from publication");
    }
    if names.contains(&&b".git"[..]) {
        return (
            "rebuildable",
            "Git execution metadata; verify reachability before removal",
        );
    }
    if names.iter().any(|part| GENERATED.contains(part)) {
        return (
            "rebuildable",
            "generated storage; not evidence of safe deletion",
        );
    }
    if names.contains(&&b"archives"[..])
        && [&b".tar.gz"[..], b".tgz", b".tar.zst"]
            .iter()
            .any(|suffix| name.ends_with(suffix))
    {
        return ("reproducibility", "dependency archive may be hash-pinned");
    }
    if names.iter().any(|part| {
        [
            &b"clone-work"[..],
            b"home",
            b"attempts",
            b"grading",
            b"regrading",
        ]
        .contains(part)
    }) {
        return (
            "local-evidence",
            "execution or grading material requires capture",
        );
    }
    let last = names.last().copied().unwrap_or_default();
    let suffix = match last.iter().rposition(|byte| *byte == b'.') {
        Some(at) if at > 0 => &last[at..],
        _ => b"",
    };
    if [&b".json"[..], b".jsonl", b".log"].contains(&suffix) {
        return (
            "local-evidence",
            "possible records or diagnostics require inspection",
        );
    }
    (
        "unknown",
        "no evidence or rebuildability contract establishes its role",
    )
}

fn text(value: &str) -> Value {
    Value::Str(value.to_string())
}

fn integer(value: u64) -> Value {
    Value::Int(value.to_string())
}

fn object(pairs: Vec<(&str, Value)>) -> Value {
    Value::Object(
        pairs
            .into_iter()
            .map(|(key, value)| (key.to_string(), value))
            .collect(),
    )
}

/// The names of `directory` split as `os.walk` splits them, into directories, following links, and the rest;
/// `None` when it cannot be listed, which `os.walk` skips silently.
fn scan(directory: &PyPath) -> Option<Names> {
    let (mut dirs, mut files) = (Vec::new(), Vec::new());
    for entry in std::fs::read_dir(directory.to_path_buf()).ok()? {
        let name = pytext::os_bytes(&entry.ok()?.file_name()).to_vec();
        let is_dir = std::fs::metadata(directory.join(&name).to_path_buf())
            .is_ok_and(|metadata| metadata.is_dir());
        if is_dir { &mut dirs } else { &mut files }.push(name);
    }
    Some((dirs, files))
}

fn sorted(mut names: Vec<Vec<u8>>) -> Vec<Vec<u8>> {
    names.sort_by_cached_key(|name| pytext::surrogateescape(name));
    names
}

/// `evidence_inventory.inventory`: every entry below `root` that is not a directory, without following links.
pub fn inventory(root: &[u8], hash_all: bool) -> Result<Value> {
    let root = absolute(root)?;
    if crate::pyfs::resolve(root.as_bytes()) != root.as_bytes() {
        return Err(violation(
            [
                &b"root must be canonical, without aliases or parent traversals: "[..],
                root.as_bytes(),
            ]
            .concat(),
        ));
    }
    if std::iter::once(root.clone())
        .chain(evidence::parents(&root))
        .any(|path| is_symlink(path.as_bytes()))
        || !crate::pyfs::is_dir(root.as_bytes())
    {
        return Err(violation(
            [
                &b"root must be an existing directory without symlink ancestors: "[..],
                root.as_bytes(),
            ]
            .concat(),
        ));
    }
    let (indexed, head) = tracked(&root)?;
    let none = HashMap::new();
    let mut entries = Vec::new();
    let mut present = HashSet::new();
    let mut totals: BTreeMap<&str, u64> = BTreeMap::new();
    let mut pending = vec![root.clone()];
    while let Some(current) = pending.pop() {
        let Some((dirs, files)) = scan(&current) else {
            continue;
        };
        let (links, dirs): (Vec<Vec<u8>>, Vec<Vec<u8>>) = dirs
            .into_iter()
            .partition(|name| is_symlink(current.join(name).as_bytes()));
        for name in sorted([files, links.clone()].concat()) {
            let path = current.join(&name);
            let relative = path.as_bytes()[root.as_bytes().len()..]
                .strip_prefix(b"/")
                .unwrap_or(&path.as_bytes()[root.as_bytes().len()..])
                .to_vec();
            let info = std::fs::symlink_metadata(path.to_path_buf())
                .map_err(|error| os_failure(&error, path.as_bytes()))?;
            let kind = if info.file_type().is_file() {
                "file"
            } else if info.file_type().is_symlink() {
                "symlink"
            } else {
                "special"
            };
            let (first, _) = classification(&relative, &none, None);
            let hashed = kind == "file"
                && (hash_all
                    || indexed.contains_key(&relative)
                    || !matches!(first, "rebuildable" | "account-state"));
            let (sha, blob) = if hashed {
                let (sha, blob) = identities(path.as_bytes(), info.len())?;
                (Some(sha), Some(blob))
            } else {
                (None, None)
            };
            let reason_text;
            let (class, reason) = if kind == "file" {
                classification(&relative, &indexed, blob.as_deref())
            } else {
                reason_text = format!("{kind}: must be inspected; never followed");
                ("unknown", reason_text.as_str())
            };
            let allocated = info.blocks() * 512;
            let mut fields = vec![
                ("path", Value::os_text(&relative)),
                ("type", text(kind)),
                ("bytes", integer(info.len())),
                ("allocated_bytes", integer(allocated)),
                ("mode", integer(u64::from(info.mode() & 0o7777))),
                ("sha256", sha.map_or(Value::Null, Value::Str)),
                ("class", text(class)),
                ("reason", text(reason)),
            ];
            if kind == "symlink" {
                let target = std::fs::read_link(path.to_path_buf())
                    .map_err(|error| os_failure(&error, path.as_bytes()))?;
                fields.push((
                    "target",
                    Value::os_text(pytext::os_bytes(target.as_os_str())),
                ));
            }
            entries.push(object(fields));
            present.insert(relative);
            *totals.entry(class).or_default() += allocated;
        }
        for name in sorted(dirs).into_iter().rev() {
            let next = current.join(&name);
            if !is_symlink(next.as_bytes()) {
                pending.push(next);
            }
        }
    }
    let missing = sorted(
        indexed
            .keys()
            .filter(|name| !present.contains(*name))
            .cloned()
            .collect(),
    );
    Ok(object(vec![
        ("schema_version", integer(1)),
        ("root", Value::os_text(root.as_bytes())),
        ("head", head.map_or(Value::Null, Value::Str)),
        ("entries", Value::Array(entries)),
        (
            "missing_tracked",
            Value::Array(missing.iter().map(|name| Value::os_text(name)).collect()),
        ),
        (
            "allocated_bytes_by_class",
            object(
                totals
                    .into_iter()
                    .map(|(class, bytes)| (class, integer(bytes)))
                    .collect(),
            ),
        ),
        ("retirement_authorized", Value::Bool(false)),
    ]))
}

fn field<'a>(value: &'a Value, key: &str) -> &'a Value {
    value
        .as_object()
        .and_then(|object| object.get(key))
        .unwrap_or(&Value::Null)
}

/// The `evidence_inventory.py` command: inventory each root, write the result as a new
/// record and return the summary it prints.
pub fn run(disk: &mut dyn Disk, roots: &[Vec<u8>], out: &[u8], hash_all: bool) -> Result<String> {
    let roots = roots
        .iter()
        .map(|root| absolute(root))
        .collect::<Result<Vec<_>>>()?;
    let out_absolute = absolute(out)?;
    if roots.iter().any(|root| is_relative_to(&out_absolute, root)) {
        return Err(violation("write the inventory outside the inspected roots"));
    }
    let inventories = roots
        .iter()
        .map(|root| inventory(root.as_bytes(), hash_all))
        .collect::<Result<Vec<_>>>()?;
    let result = object(vec![
        ("schema_version", integer(1)),
        ("roots", Value::Array(inventories)),
    ]);
    let out = PyPath::new(out);
    evidence::write_new(disk, &out, &result).map_err(|error| match error {
        evidence::Error::Violation(message)
            if message == format!("refusing to replace {}", out.display()) =>
        {
            violation([&b"refusing to replace "[..], out.as_bytes()].concat())
        }
        evidence::Error::Violation(message) => violation(message),
        evidence::Error::Io(message) => Error::Io(message),
    })?;
    let summary = Value::Array(
        result
            .as_object()
            .and_then(|object| object.get("roots"))
            .and_then(Value::as_array)
            .unwrap_or_default()
            .iter()
            .map(|item| {
                let count =
                    |key| integer(field(item, key).as_array().map_or(0, <[Value]>::len) as u64);
                object(vec![
                    ("root", field(item, "root").clone()),
                    ("files", count("entries")),
                    (
                        "allocated_bytes_by_class",
                        field(item, "allocated_bytes_by_class").clone(),
                    ),
                    ("missing_tracked", count("missing_tracked")),
                ])
            })
            .collect(),
    );
    Ok(pyjson::dumps(
        &summary,
        &pyjson::Dumps {
            indent: Some(2),
            ..pyjson::Dumps::default()
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn none() -> HashMap<Vec<u8>, String> {
        HashMap::new()
    }

    #[test]
    fn classification_matches_python() {
        let cases: [(&[u8], &str); 12] = [
            (b"home/.codex/auth.json", "account-state"),
            (b"work/home/.claude/.credentials.json", "account-state"),
            (b".git/HEAD", "rebuildable"),
            (b"a/node_modules/x.js", "rebuildable"),
            (b"archives/dep.tgz", "reproducibility"),
            (b"archives/dep.zip", "unknown"),
            (b"clone-work/out", "local-evidence"),
            (b"notes.log", "local-evidence"),
            (b".json", "unknown"),
            (b"x.json.", "unknown"),
            (b"\xff/notes.jsonl", "local-evidence"),
            (b"blob.bin", "unknown"),
        ];
        for (name, class) in cases {
            assert_eq!(classification(name, &none(), None).0, class, "{name:?}");
        }
        let indexed: HashMap<Vec<u8>, String> = [(b"src/a".to_vec(), "abc".to_string())]
            .into_iter()
            .collect();
        assert_eq!(
            classification(b"src/a", &indexed, Some("abc")).0,
            "tracked-identical"
        );
        assert_eq!(classification(b"src/a", &indexed, None).0, "development");
    }

    #[test]
    fn identities_are_sha256_and_the_git_blob_sha1() {
        let directory =
            std::env::temp_dir().join(format!("inventory-identity-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("file");
        std::fs::write(&path, b"tracked").unwrap();
        let (sha, blob) = identities(pytext::os_bytes(path.as_os_str()), 7).unwrap();
        assert_eq!(sha, sha256::hex_digest(b"tracked"));
        assert_eq!(
            blob, "3880c85b1e5623ae9955c2c4b1e14bb58249ed09",
            "git hash-object"
        );
        std::fs::remove_dir_all(&directory).unwrap();
    }

    #[test]
    fn names_sort_by_their_surrogateescape_code_points() {
        let names = sorted(vec![
            b"a\xee\x80\x80".to_vec(),
            b"a\xff".to_vec(),
            b"a\xc3\xa9".to_vec(),
        ]);
        assert_eq!(
            names,
            [
                b"a\xc3\xa9".to_vec(),
                b"a\xff".to_vec(),
                b"a\xee\x80\x80".to_vec()
            ]
        );
    }

    #[test]
    fn relative_checks_compare_parts() {
        let path = |raw: &[u8]| PyPath::new(raw);
        assert!(is_relative_to(&path(b"/a/b/c"), &path(b"/a/b")));
        assert!(is_relative_to(&path(b"/a/b"), &path(b"/a/b")));
        assert!(!is_relative_to(&path(b"/a/bc"), &path(b"/a/b")));
        assert!(!is_relative_to(&path(b"/a/x"), &path(b"/a/../a")));
    }
}
