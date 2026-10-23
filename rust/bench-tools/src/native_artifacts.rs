//! `native_artifacts.py`'s verification, restoration and filing of a skill attempt's report folder, with the
//! same refusals and messages.
//!
//! Unlike the Python, a new archived filing is staged and synced, then placed with its storage record linked last,
//! so an interrupted filing never leaves a storage record that names missing or partial files. A retry that finds the
//! staged names an interruption left completes the filing when they hold exactly what it would write, and refuses
//! otherwise, as Python refuses every existing name. Restoration syncs every restored file and directory before it
//! renames the staged tree into place.

use crate::archive::{self, ArchiveError, Limits, Refusal, ScratchWriter};
use crate::evidence::{self, Disk, TempDir};
use crate::pyfs;
use crate::pyjson::{self, Duplicates, Key, Object, Value};
use crate::pytext::{self, PyPath};
use crate::sha256::{self, Sha256};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs::{File, FileTimes, Metadata};
use std::io::{self, Read};
use std::os::unix::fs::{MetadataExt, PermissionsExt};

pub const STORAGE: &str = "native-artifact-storage.v1.json";
pub const ARCHIVE: &str = "native-scratch.v1.zip";
pub const INDEX: &str = "native-artifacts.json";

/// A failure `native_artifacts.main` reports as `bench-tools native-artifacts: <message>`, exit 1.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error(pub String);

type Result<T> = std::result::Result<T, Error>;

fn fail(message: impl Into<String>) -> Error {
    Error(message.into())
}

fn os_failure(error: &io::Error, path: &PyPath) -> Error {
    fail(pytext::os_error(error, path.as_bytes()))
}

impl From<evidence::Error> for Error {
    fn from(error: evidence::Error) -> Self {
        fail(error.message())
    }
}

/// An indexed file: the bytes its path names on disk, `os.fsencode(path)`, and its row of the index.
#[derive(Clone, Debug)]
pub struct Row {
    pub path: Vec<u8>,
    pub row: Value,
}

/// What `verify` read: the storage record and the index's rows, in index order.
#[derive(Clone, Debug)]
pub struct Verified {
    pub storage: Value,
    pub files: Vec<Row>,
}

impl Verified {
    fn row(&self, name: &[u8]) -> Option<&Row> {
        self.files.iter().find(|row| row.path == name)
    }
}

fn lstat(path: &PyPath) -> Option<Metadata> {
    std::fs::symlink_metadata(path.to_path_buf()).ok()
}

fn is_symlink(path: &PyPath) -> bool {
    lstat(path).is_some_and(|metadata| metadata.file_type().is_symlink())
}

fn is_dir(path: &PyPath) -> bool {
    path.to_path_buf().is_dir()
}

fn exists(path: &PyPath) -> bool {
    path.to_path_buf().exists()
}

/// `Path(path).name`.
fn name_of(path: &PyPath) -> Vec<u8> {
    let bytes = path.as_bytes();
    if bytes == b"." || bytes.iter().all(|byte| *byte == b'/') {
        return Vec::new();
    }
    bytes
        .rsplit(|byte| *byte == b'/')
        .next()
        .unwrap_or_default()
        .to_vec()
}

fn hash_stream(reader: &mut dyn Read) -> io::Result<String> {
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 1024 * 1024];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            return Ok(sha256::hex(&hasher.finalize()));
        }
        hasher.update(&buffer[..read]);
    }
}

/// `native_artifacts.digest`.
fn digest(path: &PyPath) -> Result<String> {
    let mut file = File::open(path.to_path_buf()).map_err(|error| os_failure(&error, path))?;
    hash_stream(&mut file).map_err(|error| os_failure(&error, path))
}

/// `json.loads(path.read_text())`, keeping the undecodable filename bytes `json.dumps` wrote as lone surrogates.
fn read_json(path: &PyPath) -> Result<Value> {
    let text = pytext::read_text(path).map_err(|error| fail(error.message()))?;
    pyjson::loads_os_text(&text, Duplicates::KeepLast).map_err(|error| fail(error.to_string()))
}

/// How `json.dumps(..., indent=2)` writes a record.
fn record_dumps() -> pyjson::Dumps {
    pyjson::Dumps {
        indent: Some(2),
        ..pyjson::Dumps::default()
    }
}

/// `str(name)` as Python writes it to stderr.
fn shown(name: &[u8]) -> String {
    pytext::stderr_text(name)
}

/// `value[key]` for a JSON value, with the message of the `KeyError` or `TypeError` Python raises.
fn item<'a>(value: &'a Value, key: &str) -> Result<&'a Value> {
    match value {
        Value::Object(object) => object.get(key).ok_or_else(|| fail(pytext::repr_str(key))),
        Value::Array(_) => Err(fail("list indices must be integers or slices, not str")),
        Value::Str(_) | Value::OsText(_) => Err(fail("string indices must be integers, not 'str'")),
        other => Err(fail(format!(
            "'{}' object is not subscriptable",
            other.type_name()
        ))),
    }
}

/// `list(value)`: a list's elements, a dict's keys or a string's characters.
fn iterate(value: &Value) -> Result<Vec<Value>> {
    match value {
        Value::Array(values) => Ok(values.clone()),
        Value::Object(object) => Ok(object
            .members()
            .map(|(key, _)| Value::from(key.clone()))
            .collect()),
        Value::Str(text) => Ok(text.chars().map(|c| Value::Str(c.to_string())).collect()),
        Value::OsText(text) => Ok(pytext::surrogateescape(text.as_bytes())
            .into_iter()
            .map(|point| match char::from_u32(point) {
                Some(c) => Value::Str(c.to_string()),
                None => Value::os_text(&[(point - 0xDC00) as u8]),
            })
            .collect()),
        other => Err(fail(format!(
            "'{}' object is not iterable",
            other.type_name()
        ))),
    }
}

fn unhashable(value: &Value) -> Error {
    fail(format!("unhashable type: '{}'", value.type_name()))
}

/// `native_artifacts.relative_path`: a relative POSIX path that `pathlib` leaves as written, without `..`, as the
/// bytes it names on disk.
fn relative_path(value: &Value) -> Result<&[u8]> {
    let name = value.os_bytes().ok_or_else(|| {
        fail(format!(
            "argument should be a str or an os.PathLike object where __fspath__ returns a str, not '{}'",
            value.type_name()
        ))
    })?;
    if name.is_empty()
        || name.starts_with(b"/")
        || name.split(|byte| *byte == b'/').any(|part| part == b"..")
        || PyPath::new(name).as_bytes() != name
    {
        return Err(fail(format!(
            "unsafe artifact path: {}",
            pytext::repr(&pytext::surrogateescape(name))
        )));
    }
    Ok(name)
}

/// `native_artifacts.indexed_files`: the index's rows by path, in order, each path safe and listed once.
pub fn indexed_files(index: &PyPath) -> Result<Vec<Row>> {
    let document = read_json(index)?;
    let rows = item(&document, "files")?;
    let rows = match rows {
        Value::Array(values) => values.clone(),
        Value::Object(object) if object.is_empty() => Vec::new(),
        Value::Str(text) if text.is_empty() => Vec::new(),
        Value::Object(_) | Value::Str(_) | Value::OsText(_) => {
            return Err(fail("string indices must be integers, not 'str'"));
        }
        other => {
            return Err(fail(format!(
                "'{}' object is not iterable",
                other.type_name()
            )));
        }
    };
    let mut files = Vec::with_capacity(rows.len());
    let mut seen = HashSet::new();
    for row in rows {
        let name = relative_path(item(&row, "path")?)?.to_vec();
        if !seen.insert(name.clone()) {
            return Err(fail(format!(
                "duplicate indexed artifact: {}",
                shown(&name)
            )));
        }
        files.push(Row { path: name, row });
    }
    Ok(files)
}

/// `native_artifacts.check_file`: a regular, unlinked file of the indexed size and SHA-256.
fn check_file(path: &PyPath, row: &Row) -> Result<()> {
    let changed = || fail(format!("artifact differs from index: {}", shown(&row.path)));
    let Some(metadata) = lstat(path) else {
        return Err(changed());
    };
    if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
        return Err(changed());
    }
    if !Value::Int(metadata.len().to_string()).py_eq(item(&row.row, "bytes")?) {
        return Err(changed());
    }
    let expected = item(&row.row, "sha256")?;
    if !Value::Str(digest(path)?).py_eq(expected) {
        return Err(changed());
    }
    Ok(())
}

fn inside(path: &PyPath, base: &PyPath) -> bool {
    pyfs::relative_parts(
        &pyfs::resolve(path.as_bytes()),
        &pyfs::resolve(base.as_bytes()),
    )
    .is_some()
}

/// `native_artifacts.check_reference`: the referenced file below `directory`, hashing to its pin.
fn check_reference(directory: &PyPath, reference: &Value) -> Result<PyPath> {
    let name = item(reference, "path")?;
    let path = directory.join(relative_path(name)?);
    let mismatch = || fail(format!("artifact checksum mismatch: {}", name.py_str()));
    if !inside(&path, directory) {
        return Err(mismatch());
    }
    let actual = digest(&path)?;
    if !Value::Str(actual).py_eq(item(reference, "sha256")?) {
        return Err(mismatch());
    }
    Ok(path)
}

/// `{"path": path.name, "sha256": digest(path)}`.
fn reference(path: &PyPath) -> Result<Value> {
    let name = name_of(path);
    Ok(Value::Object(
        [
            (
                "path".to_string(),
                Value::Str(String::from_utf8_lossy(&name).into_owned()),
            ),
            ("sha256".to_string(), Value::Str(digest(path)?)),
        ]
        .into_iter()
        .collect(),
    ))
}

/// A directory entry `Path.rglob('*')` lists, without following a symbolic link to a directory.
struct Entry {
    path: PyPath,
    relative: Vec<u8>,
    metadata: Metadata,
}

/// `list(root.rglob('*'))`: every entry below `root`, nothing when it is not a readable directory.
fn rglob(root: &PyPath) -> Vec<Entry> {
    fn walk(directory: &PyPath, prefix: &[u8], found: &mut Vec<Entry>) {
        for name in pyfs::names(directory.as_bytes()) {
            let path = directory.join(&name);
            let relative = if prefix.is_empty() {
                name.clone()
            } else {
                [prefix, b"/", &name].concat()
            };
            let Some(metadata) = lstat(&path) else {
                continue;
            };
            let recurse = metadata.file_type().is_dir();
            found.push(Entry {
                path: path.clone(),
                relative: relative.clone(),
                metadata,
            });
            if recurse {
                walk(&path, &relative, found);
            }
        }
    }
    let mut found = Vec::new();
    if is_dir(root) {
        walk(root, b"", &mut found);
    }
    found
}

/// Whether the entry is a regular file or directory, as `path.is_dir() or path.is_file()` holds for a non-link.
fn regular_or_directory(entry: &Entry) -> bool {
    let kind = entry.metadata.file_type();
    !kind.is_symlink() && (kind.is_dir() || kind.is_file())
}

/// Python's set of `storage["archived_paths"]`, checked to be distinct indexed file paths.
fn archived_paths(storage: &Value, files: &[Row]) -> Result<Vec<Vec<u8>>> {
    let archived = iterate(item(storage, "archived_paths")?)?;
    if let Some(value) = archived.iter().find(|value| !value.hashable()) {
        return Err(unhashable(value));
    }
    let names: Option<Vec<Vec<u8>>> = archived
        .iter()
        .map(|value| value.os_bytes().map(<[u8]>::to_vec))
        .collect();
    let indexed: HashSet<&[u8]> = files.iter().map(|row| row.path.as_slice()).collect();
    match names {
        Some(names)
            if names.iter().collect::<HashSet<_>>().len() == names.len()
                && names.iter().all(|name| indexed.contains(name.as_slice())) =>
        {
            Ok(names)
        }
        _ => Err(fail("archive paths must be distinct indexed files")),
    }
}

/// `storage["modes"]` by archived path, each an `int` from 0 to 0o777.
fn archived_modes(storage: &Value, archived: &[Vec<u8>]) -> Result<HashMap<Vec<u8>, u32>> {
    let modes = match item(storage, "modes")? {
        Value::Object(object) => object,
        other => {
            return Err(fail(format!(
                "'{}' object has no attribute 'keys'",
                other.type_name()
            )));
        }
    };
    let archived: HashSet<&[u8]> = archived.iter().map(Vec::as_slice).collect();
    if !modes
        .members()
        .all(|(name, _)| archived.contains(name.os_bytes()))
    {
        return Err(fail("archive modes must refer to archived files"));
    }
    let mut found = HashMap::new();
    for (name, mode) in modes.members() {
        let mode = match mode {
            Value::Int(digits) => digits.parse::<u32>().ok().filter(|mode| *mode <= 0o777),
            _ => None,
        }
        .ok_or_else(|| fail("invalid archived file permissions"))?;
        found.insert(name.os_bytes().to_vec(), mode);
    }
    Ok(found)
}

/// A content hash and its indexed size, as the dict `{files[name]["sha256"]: files[name]["bytes"]}` keeps them.
struct Blob {
    sha: Value,
    size: Value,
}

fn blobs(verified_files: &[Row], archived: &[Vec<u8>]) -> Result<Vec<Blob>> {
    let rows: HashMap<&[u8], &Row> = verified_files
        .iter()
        .map(|row| (row.path.as_slice(), row))
        .collect();
    let mut blobs: Vec<Blob> = Vec::new();
    for name in archived {
        let row = rows[name.as_slice()];
        let sha = item(&row.row, "sha256")?;
        let size = item(&row.row, "bytes")?;
        if !sha.hashable() {
            return Err(unhashable(sha));
        }
        match blobs.iter_mut().find(|blob| blob.sha.py_eq(sha)) {
            Some(blob) => blob.size = size.clone(),
            None => blobs.push(Blob {
                sha: sha.clone(),
                size: size.clone(),
            }),
        }
    }
    for name in archived {
        let row = rows[name.as_slice()];
        let sha = item(&row.row, "sha256")?;
        let blob = blobs.iter().find(|blob| blob.sha.py_eq(sha));
        if !blob.is_some_and(|blob| {
            blob.size
                .py_eq(item(&row.row, "bytes").unwrap_or(&Value::Null))
        }) {
            return Err(fail("the same content hash has conflicting indexed sizes"));
        }
    }
    Ok(blobs)
}

/// The archive limits an index allows: one entry per blob, of their indexed sizes.
fn blob_limits(blobs: &[Blob]) -> Limits {
    let payload = blobs
        .iter()
        .filter_map(|blob| match &blob.size {
            Value::Int(digits) => digits.parse::<u64>().ok(),
            _ => None,
        })
        .fold(0u64, u64::saturating_add);
    Limits {
        members: blobs.len() as u64,
        payload,
    }
}

const CONTENTS_DIFFER: &str = "archive contents differ from indexed scratch files";

/// The message `zipfile` gives for a member it cannot read, where Python names one.
fn member_failure(error: &ArchiveError, name: &[u8]) -> String {
    match error {
        ArchiveError::Io(inner) if inner.to_string().contains("Invalid checksum") => format!(
            "Bad CRC-32 for file {}",
            pytext::repr(&pytext::surrogateescape(name))
        ),
        other => other.to_string(),
    }
}

/// Check the scratch archive as `native_artifacts.verify` reads it: the same names as the blobs, then each blob's
/// content hash and size, in the blobs' order.
fn check_archive(path: &PyPath, blobs: &[Blob]) -> Result<()> {
    let limits = blob_limits(blobs);
    let open = || File::open(path.to_path_buf()).map_err(|error| os_failure(&error, path));
    let names = match archive::zip_names(open()?, &limits) {
        Ok(names) => names,
        Err(ArchiveError::Io(error)) => return Err(os_failure(&error, path)),
        Err(ArchiveError::Refused(
            Refusal::TooManyEntries { .. } | Refusal::ZipDirectory { .. },
        )) => {
            return Err(fail(CONTENTS_DIFFER));
        }
        Err(ArchiveError::Refused(_)) => return Err(fail("File is not a zip file")),
    };
    let named: HashSet<&[u8]> = names.iter().map(Vec::as_slice).collect();
    let expected: Option<HashSet<&[u8]>> = blobs
        .iter()
        .map(|blob| blob.sha.as_str().map(str::as_bytes))
        .collect();
    if names.len() != blobs.len() || expected.as_ref() != Some(&named) {
        return Err(fail(CONTENTS_DIFFER));
    }
    let mut read: HashMap<Vec<u8>, (String, u64)> = HashMap::new();
    let mut failed: Option<(Vec<u8>, ArchiveError)> = None;
    let mut current = Vec::new();
    let result = archive::read_zip(open()?, &limits, |member, data| {
        current = member.name.clone();
        let sha = hash_stream(data)?;
        read.insert(member.name.clone(), (sha, member.size));
        Ok(())
    });
    match result {
        Ok(()) => {}
        // A read error belongs to the member being read, whose CRC is checked once its data ends; a refusal
        // belongs to the next member, before it is read, and names it.
        Err(error @ ArchiveError::Io(_)) => {
            read.remove(&current);
            failed = Some((current, error));
        }
        Err(error) => failed = Some((Vec::new(), error)),
    }
    for blob in blobs {
        let sha = blob.sha.as_str().unwrap_or_default();
        match read.get(sha.as_bytes()) {
            Some((actual, size)) => {
                if actual != sha || !Value::Int(size.to_string()).py_eq(&blob.size) {
                    return Err(fail(format!("archived content differs from index: {sha}")));
                }
            }
            None => {
                return Err(match &failed {
                    Some((name, error)) => fail(member_failure(error, name)),
                    None => fail(format!("archived content differs from index: {sha}")),
                });
            }
        }
    }
    match failed {
        Some((name, error)) => Err(fail(member_failure(&error, &name))),
        None => Ok(()),
    }
}

/// `native_artifacts.verify`: the storage record `pin` names, or `attempt.json`'s, and everything it pins.
pub fn verify(directory: &PyPath, pin: Option<&Value>) -> Result<Verified> {
    let owned;
    let pin = match pin {
        Some(pin) => pin,
        None => {
            owned = read_json(&directory.join(b"attempt.json"))?;
            item(&owned, "native_artifact_storage")?
        }
    };
    let storage = read_json(&check_reference(directory, pin)?)?;
    if !item(&storage, "schema_version")?.py_eq(&Value::Int("1".to_string())) {
        return Err(fail("unsupported native artifact storage version"));
    }
    let files = indexed_files(&check_reference(directory, item(&storage, "index")?)?)?;
    let archived = archived_paths(&storage, &files)?;
    archived_modes(&storage, &archived)?;
    let blobs = blobs(&files, &archived)?;
    check_archive(
        &check_reference(directory, item(&storage, "archive")?)?,
        &blobs,
    )?;
    let artifact_root = directory.join(relative_path(item(&storage, "artifact_root")?)?);
    if is_symlink(&artifact_root) || !inside(&artifact_root, directory) {
        return Err(fail("native artifact root escapes filed directory"));
    }
    let entries = rglob(&artifact_root);
    if !entries.iter().all(regular_or_directory) {
        return Err(fail(
            "filed artifacts must be regular files and directories",
        ));
    }
    let archived_set: HashSet<&[u8]> = archived.iter().map(Vec::as_slice).collect();
    let loose: HashSet<Vec<u8>> = files
        .iter()
        .filter(|row| !archived_set.contains(row.path.as_slice()))
        .map(|row| row.path.clone())
        .collect();
    let found: HashSet<Vec<u8>> = entries
        .iter()
        .filter(|entry| entry.metadata.file_type().is_file())
        .map(|entry| entry.relative.clone())
        .collect();
    if found != loose {
        return Err(fail("loose artifacts differ from indexed file inventory"));
    }
    for row in &files {
        if archived_set.contains(row.path.as_slice()) {
            continue;
        }
        let path = artifact_root.join(&row.path);
        if !inside(&path, &artifact_root) {
            return Err(fail(format!("artifact escapes root: {}", shown(&row.path))));
        }
        check_file(&path, row)?;
    }
    Ok(Verified { storage, files })
}

/// The access and modification times `shutil.copystat` copies.
fn times(metadata: &Metadata) -> FileTimes {
    let mut times = FileTimes::new();
    if let Ok(accessed) = metadata.accessed() {
        times = times.set_accessed(accessed);
    }
    if let Ok(modified) = metadata.modified() {
        times = times.set_modified(modified);
    }
    times
}

/// The bytes, permission bits and times of `source` in the new file `target`, synced; `shutil.copy2` into a new
/// file.
fn copy_new(disk: &mut dyn Disk, source: &PyPath, target: &PyPath) -> Result<()> {
    let mut input = File::open(source.to_path_buf()).map_err(|error| os_failure(&error, source))?;
    let metadata = input
        .metadata()
        .map_err(|error| os_failure(&error, source))?;
    let os_target = target.to_path_buf();
    let mut output = disk
        .create(&os_target)
        .map_err(|error| os_failure(&error, target))?;
    io::copy(&mut input, &mut output).map_err(|error| os_failure(&error, target))?;
    finish_copy(disk, &output, &metadata, target)
}

fn finish_copy(
    disk: &mut dyn Disk,
    output: &File,
    metadata: &Metadata,
    target: &PyPath,
) -> Result<()> {
    output
        .set_permissions(std::fs::Permissions::from_mode(metadata.mode() & 0o7777))
        .map_err(|error| os_failure(&error, target))?;
    output
        .set_times(times(metadata))
        .map_err(|error| os_failure(&error, target))?;
    disk.sync(output, &target.to_path_buf())
        .map_err(|error| os_failure(&error, target))
}

/// `shutil.copy2(source, target)`, replacing a file or writing through a link as it does, then synced.
fn copy2(source: &PyPath, target: &PyPath) -> Result<()> {
    let target = if is_dir(target) {
        target.join(&name_of(source))
    } else {
        target.clone()
    };
    if let (Ok(left), Ok(right)) = (
        std::fs::metadata(source.to_path_buf()),
        std::fs::metadata(target.to_path_buf()),
    ) && left.dev() == right.dev()
        && left.ino() == right.ino()
    {
        return Err(fail(format!(
            "{} and {} are the same file",
            source.repr(),
            target.repr()
        )));
    }
    let mut input = File::open(source.to_path_buf()).map_err(|error| os_failure(&error, source))?;
    let metadata = input
        .metadata()
        .map_err(|error| os_failure(&error, source))?;
    let mut output =
        File::create(target.to_path_buf()).map_err(|error| os_failure(&error, &target))?;
    io::copy(&mut input, &mut output).map_err(|error| os_failure(&error, &target))?;
    finish_copy(&mut evidence::System, &output, &metadata, &target)
}

fn create_dir(path: &PyPath) -> Result<()> {
    std::fs::create_dir(path.to_path_buf()).map_err(|error| os_failure(&error, path))
}

/// Every directory below and including `root`, deepest first, so each is synced after the entries it gained.
fn directories(root: &PyPath) -> Vec<PyPath> {
    let mut found: Vec<PyPath> = rglob(root)
        .into_iter()
        .filter(|entry| entry.metadata.file_type().is_dir())
        .map(|entry| entry.path)
        .collect();
    found.reverse();
    found.push(root.clone());
    found
}

fn sync_tree(disk: &mut dyn Disk, root: &PyPath) -> Result<()> {
    for directory in directories(root) {
        disk.sync_dir(&directory.to_path_buf())
            .map_err(|error| os_failure(&error, &directory))?;
    }
    Ok(())
}

fn sync_dir(disk: &mut dyn Disk, directory: &PyPath) -> Result<()> {
    disk.sync_dir(&directory.to_path_buf())
        .map_err(|error| os_failure(&error, directory))
}

/// `name in record` for the JSON value `attempt.json` holds.
fn holds(record: &Value, key: &str) -> Result<bool> {
    match record {
        Value::Object(object) => Ok(object.get(key).is_some()),
        Value::Array(values) => Ok(values.iter().any(|value| value.as_str() == Some(key))),
        Value::Str(text) => Ok(text.contains(key)),
        Value::OsText(text) => Ok(text
            .as_bytes()
            .windows(key.len())
            .any(|window| window == key.as_bytes())),
        other => Err(fail(format!(
            "argument of type '{}' is not iterable",
            other.type_name()
        ))),
    }
}

/// Whether `found` holds exactly the regular files and directories of `staged`, with the same bytes and permission
/// bits: what an interrupted filing of the same tree placed.
fn same_entry(staged: &PyPath, found: &PyPath) -> bool {
    let (Some(left), Some(right)) = (lstat(staged), lstat(found)) else {
        return false;
    };
    if left.file_type().is_dir() {
        if !right.file_type().is_dir() {
            return false;
        }
        let inner = |root: &PyPath| -> Option<Vec<(Vec<u8>, bool, u32)>> {
            let mut entries: Vec<(Vec<u8>, bool, u32)> = Vec::new();
            for entry in rglob(root) {
                let kind = entry.metadata.file_type();
                if !kind.is_dir() && !kind.is_file() {
                    return None;
                }
                entries.push((
                    entry.relative,
                    kind.is_dir(),
                    entry.metadata.mode() & 0o7777,
                ));
            }
            entries.sort();
            Some(entries)
        };
        let (Some(expected), Some(actual)) = (inner(staged), inner(found)) else {
            return false;
        };
        return expected
            .iter()
            .map(|(name, directory, _)| (name, directory))
            .eq(actual.iter().map(|(name, directory, _)| (name, directory)))
            && expected.iter().zip(&actual).all(|(left, right)| {
                left.1
                    || left.2 == right.2 && same_bytes(&staged.join(&left.0), &found.join(&right.0))
            });
    }
    right.file_type().is_file()
        && left.mode() & 0o7777 == right.mode() & 0o7777
        && same_bytes(staged, found)
}

fn same_bytes(left: &PyPath, right: &PyPath) -> bool {
    match (
        std::fs::read(left.to_path_buf()),
        std::fs::read(right.to_path_buf()),
    ) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}

/// `native_artifacts.file_artifacts`: file a native tree into `out` and return the storage pin, or None for a
/// loose filing. An existing filing is verified and never changed.
pub fn file_artifacts(
    disk: &mut dyn Disk,
    root: &PyPath,
    index: &PyPath,
    out: &PyPath,
    native_relative: Option<&[u8]>,
) -> Result<Option<Value>> {
    let files = indexed_files(index)?;
    let archived: Vec<Vec<u8>> = {
        let mut names: Vec<Vec<u8>> = files
            .iter()
            .map(|row| row.path.clone())
            .filter(|name| {
                let parts: Vec<&[u8]> = name.split(|byte| *byte == b'/').collect();
                parts[..parts.len() - 1].contains(&&b"scratch"[..])
                    && Some(name.as_slice()) != native_relative
            })
            .collect();
        names.sort_by_cached_key(|name| pytext::surrogateescape(name));
        names
    };
    let record_path = out.join(b"attempt.json");
    let record = if exists(&record_path) {
        read_json(&record_path)?
    } else {
        Value::Object(Object::default())
    };
    let storage_path = out.join(STORAGE.as_bytes());
    let root_name = name_of(root);
    if holds(&record, "native_artifact_storage")? || exists(&storage_path) {
        let recorded = match &record {
            Value::Object(object) => object.get("native_artifact_storage").cloned(),
            Value::Array(_) => return Err(fail("'list' object has no attribute 'get'")),
            Value::Str(_) | Value::OsText(_) => {
                return Err(fail("'str' object has no attribute 'get'"));
            }
            _ => None,
        };
        let pin = match recorded {
            Some(pin) if pin.truthy() => pin,
            _ => reference(&storage_path)?,
        };
        let verified = verify(out, Some(&pin))?;
        let filed_index = item(item(&verified.storage, "index")?, "sha256")?;
        let filed_root = item(&verified.storage, "artifact_root")?;
        if !filed_index.py_eq(&Value::Str(digest(index)?))
            || !filed_root.py_eq(&Value::Str(
                String::from_utf8_lossy(&root_name).into_owned(),
            ))
            || std::str::from_utf8(&root_name).is_err()
        {
            return Err(fail(
                "refusing to change an existing native artifact filing",
            ));
        }
        // An interrupted filing may have linked its storage record without syncing the entry.
        let reaching: BTreeSet<Vec<u8>> = std::iter::once(out.clone())
            .chain(evidence::parents(out))
            .map(|directory| directory.as_bytes().to_vec())
            .collect();
        evidence::sync_dirs(disk, &reaching)?;
        return Ok(Some(pin));
    }
    if archived.is_empty() || exists(&record_path) {
        for row in &files {
            check_file(&root.join(&row.path), row)?;
            let target = out.join(&root_name).join(&row.path);
            evidence::make_dirs(&evidence::parent(&target))?;
            copy2(&root.join(&row.path), &target)?;
        }
        copy2(index, &out.join(INDEX.as_bytes()))?;
        return Ok(None);
    }
    if is_symlink(root) {
        return Err(fail("scratch packing requires a regular artifact root"));
    }
    let entries = rglob(root);
    if !entries.iter().all(regular_or_directory) {
        return Err(fail(
            "scratch packing requires regular files and directories",
        ));
    }
    let actual: HashSet<Vec<u8>> = entries
        .iter()
        .filter(|entry| entry.metadata.file_type().is_file())
        .map(|entry| entry.relative.clone())
        .collect();
    let listed: HashSet<Vec<u8>> = files.iter().map(|row| row.path.clone()).collect();
    if actual != listed {
        return Err(fail(
            "native artifact tree differs from indexed file inventory",
        ));
    }
    for row in &files {
        check_file(&root.join(&row.path), row)?;
    }
    evidence::make_dirs(out)?;
    let reserved = [ARCHIVE, STORAGE, INDEX];
    if reserved
        .iter()
        .any(|name| name.as_bytes() == root_name.as_slice())
        || exists(&storage_path)
        || is_symlink(&storage_path)
    {
        return Err(fail("scratch packing needs unused artifact output paths"));
    }
    let staging = TempDir::new(out, "native-artifacts-")?;
    let staged = staging.path.clone();
    let storage = stage(disk, root, index, &files, &archived, &staged, &root_name)?;
    let pin = reference(&staged.join(STORAGE.as_bytes()))?;
    verify(&staged, Some(&pin))?;
    // An interrupted filing of this tree left any of these names holding what this one staged; anything else
    // there is refused, as Python refuses every existing name.
    let placed = [
        root_name.clone(),
        INDEX.as_bytes().to_vec(),
        ARCHIVE.as_bytes().to_vec(),
    ];
    for name in &placed {
        let target = out.join(name);
        if lstat(&target).is_some() && !same_entry(&staged.join(name), &target) {
            return Err(fail("scratch packing needs unused artifact output paths"));
        }
    }
    for name in &placed {
        let (source, target) = (staged.join(name), out.join(name));
        if lstat(&target).is_some() {
            continue;
        }
        let moved = if name == &root_name {
            disk.rename(&source.to_path_buf(), &target.to_path_buf())
        } else {
            disk.link(&source.to_path_buf(), &target.to_path_buf())
        };
        moved.map_err(|error| os_failure(&error, &target))?;
    }
    // The placed names are durable before the storage record that pins them; write_new then syncs `out` and
    // each of its ancestors, which covers the directories this call created.
    sync_dir(disk, out)?;
    evidence::write_new_as(disk, &storage_path, &storage, &record_dumps())?;
    drop(staging);
    Ok(Some(reference(&storage_path)?))
}

/// Write the filing into `staged` as `file_artifacts` stages it, every file and directory synced, and return the
/// storage record, which is also written there.
fn stage(
    disk: &mut dyn Disk,
    root: &PyPath,
    index: &PyPath,
    files: &[Row],
    archived: &[Vec<u8>],
    staged: &PyPath,
    root_name: &[u8],
) -> Result<Value> {
    copy_new(disk, index, &staged.join(INDEX.as_bytes()))?;
    let rows: HashMap<&[u8], &Row> = files.iter().map(|row| (row.path.as_slice(), row)).collect();
    let archive_path = staged.join(ARCHIVE.as_bytes());
    let mut modes = Object::default();
    {
        let file = disk
            .create(&archive_path.to_path_buf())
            .map_err(|error| os_failure(&error, &archive_path))?;
        let mut writer = ScratchWriter::new(file);
        let mut written = HashSet::new();
        for name in archived {
            let path = root.join(name);
            let metadata =
                std::fs::metadata(path.to_path_buf()).map_err(|error| os_failure(&error, &path))?;
            let mode = metadata.mode() & 0o777;
            if mode != 0o644 {
                modes.insert_key(Key::os_text(name), Value::Int(mode.to_string()));
            }
            let sha = item(&rows[name.as_slice()].row, "sha256")?.py_str();
            if written.insert(sha.clone()) {
                let mut data =
                    File::open(path.to_path_buf()).map_err(|error| os_failure(&error, &path))?;
                writer
                    .append(&sha, &mut data)
                    .map_err(|error| fail(error.to_string()))?;
            }
        }
        let file = writer.finish().map_err(|error| fail(error.to_string()))?;
        disk.sync(&file, &archive_path.to_path_buf())
            .map_err(|error| os_failure(&error, &archive_path))?;
    }
    let tree = staged.join(root_name);
    create_dir(&tree)?;
    let archived_set: HashSet<&[u8]> = archived.iter().map(Vec::as_slice).collect();
    for row in files {
        if archived_set.contains(row.path.as_slice()) {
            continue;
        }
        let target = tree.join(&row.path);
        evidence::make_dirs(&evidence::parent(&target))?;
        copy_new(disk, &root.join(&row.path), &target)?;
    }
    sync_tree(disk, &tree)?;
    let storage = Value::Object(
        [
            ("schema_version".to_string(), Value::Int("1".to_string())),
            (
                "artifact_root".to_string(),
                Value::Str(String::from_utf8_lossy(root_name).into_owned()),
            ),
            (
                "index".to_string(),
                reference(&staged.join(INDEX.as_bytes()))?,
            ),
            ("archive".to_string(), reference(&archive_path)?),
            (
                "archived_paths".to_string(),
                Value::Array(archived.iter().map(|name| Value::os_text(name)).collect()),
            ),
            ("modes".to_string(), Value::Object(modes)),
        ]
        .into_iter()
        .collect(),
    );
    evidence::write_new_as(
        disk,
        &staged.join(STORAGE.as_bytes()),
        &storage,
        &record_dumps(),
    )?;
    sync_dir(disk, staged)?;
    Ok(storage)
}

/// `native_artifacts.restore`: verify the filing, then restore its indexed paths into the new directory `out`.
pub fn restore(disk: &mut dyn Disk, directory: &PyPath, out: &PyPath) -> Result<()> {
    let verified = verify(directory, None)?;
    if exists(out) || is_symlink(out) {
        return Err(fail("restoration requires a new output directory"));
    }
    let parent = evidence::parent(out);
    let created = evidence::make_dirs(&parent)?;
    let staging = TempDir::new(&parent, "native-restore-")?;
    let staged = staging.path.join(b"artifacts");
    let artifact_root = item(&verified.storage, "artifact_root")?.py_str();
    copytree(disk, &directory.join(artifact_root.as_bytes()), &staged)?;
    let archived = archived_paths(&verified.storage, &verified.files)?;
    let modes = archived_modes(&verified.storage, &archived)?;
    let mut by_sha: Vec<(String, Vec<&[u8]>)> = Vec::new();
    for name in &archived {
        let row = verified
            .row(name)
            .ok_or_else(|| fail(pytext::repr(&pytext::surrogateescape(name))))?;
        let sha = item(&row.row, "sha256")?.py_str();
        match by_sha.iter_mut().find(|(known, _)| *known == sha) {
            Some((_, names)) => names.push(name),
            None => by_sha.push((sha, vec![name])),
        }
    }
    let archive_path = directory.join(
        item(item(&verified.storage, "archive")?, "path")?
            .py_str()
            .as_bytes(),
    );
    let blobs = blobs(&verified.files, &archived)?;
    let file = File::open(archive_path.to_path_buf())
        .map_err(|error| os_failure(&error, &archive_path))?;
    let mut failure = None;
    let result = archive::read_zip(file, &blob_limits(&blobs), |member, data| {
        let Some((_, names)) = by_sha.iter().find(|(sha, _)| sha.as_bytes() == member.name) else {
            return Ok(());
        };
        let mut first: Option<PyPath> = None;
        for name in names {
            let target = staged.join(name);
            let written = (|| -> Result<()> {
                evidence::make_dirs(&evidence::parent(&target))?;
                let mut output = disk
                    .create(&target.to_path_buf())
                    .map_err(|error| os_failure(&error, &target))?;
                match &first {
                    None => io::copy(data, &mut output),
                    Some(source) => {
                        let mut input = File::open(source.to_path_buf())
                            .map_err(|error| os_failure(&error, source))?;
                        io::copy(&mut input, &mut output)
                    }
                }
                .map_err(|error| os_failure(&error, &target))?;
                output
                    .set_permissions(std::fs::Permissions::from_mode(
                        modes.get(*name).copied().unwrap_or(0o644),
                    ))
                    .map_err(|error| os_failure(&error, &target))?;
                disk.sync(&output, &target.to_path_buf())
                    .map_err(|error| os_failure(&error, &target))
            })();
            if let Err(error) = written {
                failure = Some(error);
                return Err(Refusal::Malformed("restoration failed".to_string()).into());
            }
            first.get_or_insert(target);
        }
        Ok(())
    });
    if let Some(error) = failure {
        return Err(error);
    }
    result.map_err(|error| fail(member_failure(&error, b"")))?;
    for row in &verified.files {
        check_file(&staged.join(&row.path), row)?;
    }
    sync_tree(disk, &staged)?;
    disk.rename(&staged.to_path_buf(), &out.to_path_buf())
        .map_err(|error| os_failure(&error, out))?;
    let reaching: BTreeSet<Vec<u8>> = std::iter::once(parent.clone())
        .chain(created.iter().map(evidence::parent))
        .map(|directory| directory.as_bytes().to_vec())
        .collect();
    evidence::sync_dirs(disk, &reaching)?;
    Ok(())
}

/// `shutil.copytree(source, target)` for a verified tree of regular files and directories, every file synced.
fn copytree(disk: &mut dyn Disk, source: &PyPath, target: &PyPath) -> Result<()> {
    let metadata =
        std::fs::metadata(source.to_path_buf()).map_err(|error| os_failure(&error, source))?;
    if !metadata.is_dir() {
        return Err(os_failure(&io::Error::from_raw_os_error(20), source));
    }
    let names =
        std::fs::read_dir(source.to_path_buf()).map_err(|error| os_failure(&error, source))?;
    std::fs::create_dir_all(target.to_path_buf()).map_err(|error| os_failure(&error, target))?;
    for entry in names {
        let entry = entry.map_err(|error| os_failure(&error, source))?;
        let name = pytext::os_bytes(entry.file_name().as_os_str()).to_vec();
        let (from, to) = (source.join(&name), target.join(&name));
        if is_dir(&from) {
            copytree(disk, &from, &to)?;
        } else {
            copy_new(disk, &from, &to)?;
        }
    }
    std::fs::set_permissions(
        target.to_path_buf(),
        std::fs::Permissions::from_mode(metadata.mode() & 0o7777),
    )
    .map_err(|error| os_failure(&error, target))
}

/// `native_artifacts.unindexed`: entries below `root` that the index does not list, and every link, sorted as
/// Python sorts their text. No link is followed.
pub fn unindexed(root: &PyPath, index: &PyPath) -> Result<Vec<Vec<u8>>> {
    let files = indexed_files(index)?;
    let listed: HashSet<Vec<u8>> = files.iter().map(|row| row.path.clone()).collect();
    fn walk(
        directory: &PyPath,
        prefix: &[u8],
        listed: &HashSet<Vec<u8>>,
        extra: &mut Vec<Vec<u8>>,
    ) {
        let Ok(entries) = std::fs::read_dir(directory.to_path_buf()) else {
            return;
        };
        for entry in entries.flatten() {
            let name = pytext::os_bytes(entry.file_name().as_os_str()).to_vec();
            let path = directory.join(&name);
            let relative = if prefix.is_empty() {
                name
            } else {
                [prefix, b"/", &name].concat()
            };
            let Some(metadata) = lstat(&path) else {
                continue;
            };
            if metadata.file_type().is_symlink() {
                extra.push(relative);
            } else if metadata.file_type().is_dir() {
                walk(&path, &relative, listed, extra);
            } else if !listed.contains(relative.as_slice()) {
                extra.push(relative);
            }
        }
    }
    let mut extra = Vec::new();
    walk(root, b"", &listed, &mut extra);
    extra.sort_by_key(|name| pytext::surrogateescape(name));
    Ok(extra)
}

/// The `file` command's report: `json.dumps({"pin": pin, "unindexed": [...]})`.
pub fn filing_report(pin: Option<&Value>, unindexed: &[Vec<u8>]) -> String {
    let pin = pin.map_or_else(
        || "null".to_string(),
        |pin| pyjson::dumps(pin, &pyjson::Dumps::default()),
    );
    let names: Vec<String> = unindexed
        .iter()
        .map(|name| pyjson::ascii_string(&pytext::surrogateescape(name)))
        .collect();
    format!("{{\"pin\": {pin}, \"unindexed\": [{}]}}", names.join(", "))
}
