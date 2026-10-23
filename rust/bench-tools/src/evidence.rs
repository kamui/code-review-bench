//! Restoration and verification of published evidence, as `evidence_store.py` restores it: logical paths,
//! confinement, manifest validation, unpacking, remote verification, materialization, resolution and receipts.
//!
//! Unlike the Python, every restored file and cached archive is synced before it is linked into place, and every
//! directory that gained a link is synced before `materialize` returns, so a fetch that exits 0 survives a host
//! restart on a filesystem that honors `fsync`. A destination that exists but differs from the manifest is never
//! trusted or overwritten: `materialize` lists every such file with a command that moves them aside.

pub mod collect;
#[cfg(test)]
pub mod fake;
pub mod publication;
pub mod recovery;
pub mod remote;
pub mod transcripts;

use crate::archive::{self, ArchiveError, Limits, Refusal};
use crate::pyjson::{self, Duplicates, Object, Value};
use crate::pytext::{self, PyPath};
use crate::schema::Validator;
use crate::sha256::{self, Sha256};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::fmt::Write as _;
use std::fs::File;
use std::hash::{BuildHasher, Hasher};
use std::io::{self, Read, Write};
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::path::Path;

pub use remote::{GitHub, Remote, Upload};

/// Python's `bench/schema/evidence-storage.schema.json`, which a cargo test keeps byte-identical.
pub const SCHEMA: &str = include_str!("evidence/evidence-storage.schema.json");
pub const ASSET_LIMIT: u64 = 2 * 1024 * 1024 * 1024;
const RECEIPT_METHOD: &str =
    "empty temporary directory; remote identity, archive hash, member bytes and permissions";

/// How `evidence_store.main` reports a failure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// An evidence violation (`EvidenceError`, `ValueError`, `TarError`): stdout, exit 1.
    Violation(String),
    /// An I/O or subprocess failure (`OSError`, `SubprocessError`): stderr, exit 2.
    Io(String),
}

impl Error {
    pub fn message(&self) -> &str {
        match self {
            Self::Violation(message) | Self::Io(message) => message,
        }
    }
}

type Result<T> = std::result::Result<T, Error>;

fn violation(message: impl Into<String>) -> Error {
    Error::Violation(message.into())
}

fn os_failure(error: &io::Error, path: &[u8]) -> Error {
    Error::Io(pytext::os_error(error, path))
}

/// The filesystem effects whose order makes restoration durable. Cargo tests record and interrupt them.
pub trait Disk {
    /// Create a new file, as `open(path, 'xb')` does.
    fn create(&mut self, path: &Path) -> io::Result<File>;
    /// Flush a file's data and metadata, as `os.fsync` does.
    fn sync(&mut self, file: &File, path: &Path) -> io::Result<()>;
    /// Hard-link `from` at `to`, failing when `to` exists.
    fn link(&mut self, from: &Path, to: &Path) -> io::Result<()>;
    /// Flush a directory's entries.
    fn sync_dir(&mut self, path: &Path) -> io::Result<()>;
    /// Rename `from` over `to`, as `os.replace` does.
    fn rename(&mut self, from: &Path, to: &Path) -> io::Result<()> {
        std::fs::rename(from, to)
    }
}

/// The host filesystem.
pub struct System;

impl Disk for System {
    fn create(&mut self, path: &Path) -> io::Result<File> {
        File::options().write(true).create_new(true).open(path)
    }

    fn sync(&mut self, file: &File, _path: &Path) -> io::Result<()> {
        file.sync_all()
    }

    fn link(&mut self, from: &Path, to: &Path) -> io::Result<()> {
        std::fs::hard_link(from, to)
    }

    fn sync_dir(&mut self, path: &Path) -> io::Result<()> {
        match File::open(path)?.sync_all() {
            // Some filesystems cannot sync a directory; their entries are as durable as they allow.
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::InvalidInput | io::ErrorKind::Unsupported
                ) || error.raw_os_error() == Some(25) =>
            {
                Ok(())
            }
            other => other,
        }
    }
}

/// `datetime.now(timezone.utc).isoformat()`.
pub fn now_isoformat() -> String {
    let elapsed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    isoformat(elapsed.as_secs(), elapsed.subsec_micros())
}

/// `datetime.isoformat()` of a UTC instant: the fraction only when the microseconds are not zero.
pub fn isoformat(seconds: u64, micros: u32) -> String {
    let days = i64::try_from(seconds / 86_400).unwrap_or(i64::MAX);
    let rest = seconds % 86_400;
    // Howard Hinnant's civil_from_days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    let time = format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}",
        rest / 3_600,
        rest % 3_600 / 60,
        rest % 60
    );
    if micros == 0 {
        format!("{time}+00:00")
    } else {
        format!("{time}.{micros:06}+00:00")
    }
}

/// What restoration acts through.
pub struct Store<'a> {
    pub remote: &'a mut dyn Remote,
    pub disk: &'a mut dyn Disk,
    pub now: &'a dyn Fn() -> String,
    /// Where `verify` makes its empty directories: [`system_temp`], as `tempfile.gettempdir()` chooses.
    pub temporary: PyPath,
}

/// `evidence_store.read`: `json.loads(Path(path).read_text(encoding='utf-8'))`.
pub fn read(path: &PyPath) -> Result<Value> {
    let text = pytext::read_text(path).map_err(|error| match error {
        pytext::ReadError::Os(message) => Error::Io(message),
        pytext::ReadError::Decode(message) => Error::Violation(message),
    })?;
    pyjson::loads(&text, Duplicates::KeepLast).map_err(|error| violation(error.to_string()))
}

fn open(path: &PyPath) -> Result<File> {
    File::open(path.to_path_buf()).map_err(|error| os_failure(&error, path.as_bytes()))
}

fn hash_reader(reader: &mut dyn Read, path: &PyPath) -> Result<String> {
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 1024 * 1024];
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|error| os_failure(&error, path.as_bytes()))?;
        if read == 0 {
            return Ok(sha256::hex(&hasher.finalize()));
        }
        hasher.update(&buffer[..read]);
    }
}

/// `evidence_store.digest`.
pub fn digest(path: &PyPath) -> Result<String> {
    hash_reader(&mut open(path)?, path)
}

/// `evidence_store.logical_path`: a relative POSIX path under `bench`, `artifacts` or `docs`, already normalized.
pub fn logical_path(value: &str) -> Result<()> {
    let parts: Vec<&str> = value.split('/').collect();
    if value.is_empty()
        || value.starts_with('/')
        || value.contains('\\')
        || parts
            .iter()
            .any(|part| matches!(*part, "" | "." | ".." | ".git"))
        || !matches!(parts[0], "bench" | "artifacts" | "docs")
    {
        return Err(violation(format!("unsafe evidence path: {value}")));
    }
    Ok(())
}

fn logical_value(value: &Value) -> Result<&str> {
    let text = value
        .as_str()
        .ok_or_else(|| violation("evidence paths must be strings"))?;
    logical_path(text)?;
    Ok(text)
}

/// `PurePath.parents`, nearest first.
pub fn parents(path: &PyPath) -> Vec<PyPath> {
    let mut found = Vec::new();
    let mut current = path.as_bytes().to_vec();
    loop {
        let parent = match current.iter().rposition(|byte| *byte == b'/') {
            Some(0) if current.len() > 1 => b"/".to_vec(),
            Some(0) => return found,
            Some(at) if current[..at].iter().all(|byte| *byte == b'/') => current[..=at].to_vec(),
            Some(at) => current[..at].to_vec(),
            None if current == b"." => return found,
            None => b".".to_vec(),
        };
        if parent == current {
            return found;
        }
        found.push(PyPath::new(&parent));
        current = parent;
    }
}

fn is_symlink(path: &PyPath) -> bool {
    std::fs::symlink_metadata(path.to_path_buf())
        .is_ok_and(|metadata| metadata.file_type().is_symlink())
}

fn exists(path: &PyPath) -> bool {
    path.to_path_buf().exists()
}

/// `evidence_store.confined`: the logical path under `root`, refused when it or any ancestor is a symlink.
pub fn confined(root: &PyPath, name: &str) -> Result<PyPath> {
    logical_path(name)?;
    let path = root.join(name.as_bytes());
    for current in std::iter::once(path.clone()).chain(parents(&path)) {
        if is_symlink(&current) {
            return Err(violation(format!(
                "symlink in evidence path: {}",
                current.display()
            )));
        }
    }
    Ok(path)
}

/// A member as a validated manifest indexes it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Member {
    pub path: String,
    pub sha256: String,
    pub bytes: u64,
    pub mode: u32,
}

/// The release and asset a package was published as.
#[derive(Clone, Debug)]
pub struct Publication {
    pub release_id: Value,
    pub asset_id: Value,
}

#[derive(Clone, Debug)]
pub struct Package {
    pub asset: String,
    pub sha256: String,
    pub bytes: u64,
    pub members: Vec<Member>,
    pub publication: Option<Publication>,
}

#[derive(Clone, Debug)]
pub struct Manifest {
    pub repository: String,
    pub tag: String,
    pub subjects: Vec<String>,
    pub packages: Vec<Package>,
}

fn field<'a>(object: &'a Object, key: &str) -> Result<&'a Value> {
    object
        .get(key)
        .ok_or_else(|| violation(format!("missing key: {}", pytext::repr_str(key))))
}

fn text(object: &Object, key: &str) -> Result<String> {
    Ok(field(object, key)?.py_str())
}

/// A count the schema already checked is a nonnegative integer; one beyond `u64` saturates.
fn count(value: &Value) -> u64 {
    match value {
        Value::Int(digits) => digits.parse().unwrap_or(u64::MAX),
        _ => u64::MAX,
    }
}

fn object(value: &Value) -> Result<&Object> {
    value
        .as_object()
        .ok_or_else(|| violation(format!("type error: {} is not a dict", value.type_name())))
}

fn array(value: &Value) -> Result<&[Value]> {
    value
        .as_array()
        .ok_or_else(|| violation(format!("type error: {} is not a list", value.type_name())))
}

/// `evidence_store.validate`, returning the manifest's typed view.
pub fn validate(manifest: &Value, published: bool) -> Result<Manifest> {
    let schema = pyjson::loads(SCHEMA, Duplicates::KeepLast)
        .map_err(|error| Error::Io(format!("evidence schema: {error}")))?;
    let errors = Validator::default()
        .validate(&schema, manifest)
        .map_err(|error| violation(error.0))?;
    if !errors.is_empty() {
        return Err(violation(errors.join("; ")));
    }
    let document = object(manifest)?;
    let subjects: Vec<String> = array(field(document, "subjects")?)?
        .iter()
        .map(Value::py_str)
        .collect();
    if subjects.is_empty() || subjects.iter().collect::<HashSet<_>>().len() != subjects.len() {
        return Err(violation("subjects must be nonempty and distinct"));
    }
    let raw_packages = array(field(document, "packages")?)?;
    if published && raw_packages.is_empty() {
        return Err(violation("published manifest contains no packages"));
    }
    let mut paths = HashSet::new();
    let mut hashes = HashSet::new();
    let mut packages = Vec::new();
    for raw in raw_packages {
        let package = object(raw)?;
        let sha = text(package, "sha256")?;
        let bytes = count(field(package, "bytes")?);
        if hashes.contains(&sha) || bytes >= ASSET_LIMIT {
            return Err(violation(
                "duplicate package or package exceeds release asset limit",
            ));
        }
        hashes.insert(sha.clone());
        let asset = text(package, "asset")?;
        if asset != format!("{sha}.tar.gz") {
            return Err(violation("asset name must identify the archive bytes"));
        }
        let publication = field(package, "publication")?;
        if published && matches!(publication, Value::Null) {
            return Err(violation("package has no publication receipt"));
        }
        let publication = match publication {
            Value::Object(receipt) if !receipt.is_empty() => {
                if text(receipt, "sha256")? != sha {
                    return Err(violation("publication receipt names different bytes"));
                }
                Some(Publication {
                    release_id: field(receipt, "release_id")?.clone(),
                    asset_id: field(receipt, "asset_id")?.clone(),
                })
            }
            _ => None,
        };
        let raw_members = array(field(package, "members")?)?;
        if raw_members.is_empty() {
            return Err(violation("empty evidence package"));
        }
        let total = raw_members.iter().try_fold(0u128, |sum, member| {
            Ok::<_, Error>(sum + u128::from(count(field(object(member)?, "bytes")?)))
        })?;
        if total >= u128::from(ASSET_LIMIT) {
            return Err(violation("unpacked package exceeds size limit"));
        }
        let mut members = Vec::new();
        for raw_member in raw_members {
            let member = object(raw_member)?;
            let path = logical_value(field(member, "path")?)?.to_string();
            let mode = count(field(member, "mode")?);
            if paths.contains(&path) || mode > 0o777 {
                return Err(violation("duplicate member or unsafe permission bits"));
            }
            paths.insert(path.clone());
            members.push(Member {
                path,
                sha256: text(member, "sha256")?,
                bytes: count(field(member, "bytes")?),
                mode: u32::try_from(mode).unwrap_or(u32::MAX),
            });
        }
        packages.push(Package {
            asset,
            sha256: sha,
            bytes,
            members,
            publication,
        });
    }
    Ok(Manifest {
        repository: text(document, "repository")?,
        tag: text(document, "tag")?,
        subjects,
        packages,
    })
}

/// What is at a path that should hold a member.
enum Found {
    Missing,
    NotRegular,
    Regular {
        bytes: u64,
        sha256: String,
        mode: u32,
    },
}

fn inspect(path: &PyPath) -> Result<Found> {
    if !exists(path) && !is_symlink(path) {
        return Ok(Found::Missing);
    }
    let regular = std::fs::symlink_metadata(path.to_path_buf())
        .is_ok_and(|metadata| metadata.file_type().is_file());
    if !regular {
        return Ok(Found::NotRegular);
    }
    let file = open(path)?;
    let metadata = file
        .metadata()
        .map_err(|error| os_failure(&error, path.as_bytes()))?;
    let mut file = file;
    Ok(Found::Regular {
        bytes: metadata.len(),
        sha256: hash_reader(&mut file, path)?,
        mode: metadata.permissions().mode() & 0o777,
    })
}

/// `evidence_store.checked_file`: a regular unlinked file of the indexed size and hash, and mode when given.
pub fn checked_file(path: &PyPath, sha: &str, bytes: u64, mode: Option<u32>) -> Result<()> {
    let changed = || violation(format!("missing or changed evidence: {}", path.display()));
    if is_symlink(path) || !path.is_file() {
        return Err(changed());
    }
    let metadata = std::fs::metadata(path.to_path_buf())
        .map_err(|error| os_failure(&error, path.as_bytes()))?;
    if metadata.len() != bytes || digest(path)? != sha {
        return Err(changed());
    }
    if let Some(mode) = mode
        && metadata.permissions().mode() & 0o777 != mode
    {
        return Err(violation(format!(
            "changed evidence permissions: {}",
            path.display()
        )));
    }
    Ok(())
}

/// `Path.mkdir(parents=True, exist_ok=True)`, returning the directories it created, outermost first.
pub fn make_dirs(path: &PyPath) -> Result<Vec<PyPath>> {
    let mut missing = Vec::new();
    for current in std::iter::once(path.clone()).chain(parents(path)) {
        if current.to_path_buf().is_dir() {
            break;
        }
        missing.push(current);
    }
    let mut created = Vec::new();
    for directory in missing.into_iter().rev() {
        match std::fs::create_dir(directory.to_path_buf()) {
            Ok(()) => created.push(directory),
            Err(error)
                if error.kind() == io::ErrorKind::AlreadyExists
                    && directory.to_path_buf().is_dir() => {}
            Err(error) => return Err(os_failure(&error, directory.as_bytes())),
        }
    }
    Ok(created)
}

pub(crate) fn parent(path: &PyPath) -> PyPath {
    parents(path)
        .into_iter()
        .next()
        .unwrap_or_else(|| PyPath::new(b"."))
}

/// The directories from `path`'s parent up, each of which an interrupted earlier write may have given an entry it
/// never synced.
fn reaching(path: &PyPath) -> BTreeSet<Vec<u8>> {
    let directory = parent(path);
    std::iter::once(directory.clone())
        .chain(parents(&directory))
        .map(|ancestor| ancestor.as_bytes().to_vec())
        .collect()
}

/// Sync the directories that reach an existing record, so a record an interrupted call linked is durable before a
/// retry reports it.
fn sync_reaching(disk: &mut dyn Disk, path: &PyPath) -> Result<()> {
    sync_dirs(disk, &reaching(path))
}

/// Sync each directory that gained an entry: the parents of new directories and of new links.
pub(crate) fn sync_dirs(disk: &mut dyn Disk, directories: &BTreeSet<Vec<u8>>) -> Result<()> {
    for directory in directories {
        let path = PyPath::new(directory);
        disk.sync_dir(&path.to_path_buf())
            .map_err(|error| os_failure(&error, path.as_bytes()))?;
    }
    Ok(())
}

const TEMPORARY_CHARACTERS: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789_";

pub(crate) fn random_name(prefix: &str) -> String {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let mut hasher = std::hash::RandomState::new().build_hasher();
    hasher.write_u64(COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed));
    hasher.write_u32(std::process::id());
    let mut value = hasher.finish();
    let mut name = prefix.to_string();
    for _ in 0..8 {
        name.push(char::from(TEMPORARY_CHARACTERS[(value % 37) as usize]));
        value /= 37;
    }
    name
}

/// A new name in `directory` for a temporary file. It does not include the destination's name, so a destination
/// whose name is near the filesystem's limit still has one.
fn temporary_beside(directory: &PyPath) -> PyPath {
    directory.join(format!(".{}.tmp", random_name("")).as_bytes())
}

/// `tempfile.TemporaryDirectory(prefix=..., dir=...)`: a new directory, mode 0700, removed with its contents.
pub(crate) struct TempDir {
    pub(crate) path: PyPath,
}

impl TempDir {
    pub(crate) fn new(parent: &PyPath, prefix: &str) -> Result<Self> {
        loop {
            let path = parent.join(random_name(prefix).as_bytes());
            match std::fs::DirBuilder::new()
                .mode(0o700)
                .create(path.to_path_buf())
            {
                Ok(()) => return Ok(Self { path }),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(os_failure(&error, path.as_bytes())),
            }
        }
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(self.path.to_path_buf());
    }
}

/// `TMPDIR`, else `/tmp`, with its links resolved: `verify` refuses a temporary directory below a symlink, and
/// macOS's is below `/var`, a link to `/private/var`.
pub fn system_temp() -> PyPath {
    use std::os::unix::ffi::OsStrExt;
    let temp = std::env::temp_dir();
    let temp = temp.as_os_str().as_bytes();
    PyPath::new(&pytext::realpath(temp, false).unwrap_or_else(|_| temp.to_vec()))
}

/// `tarfile.open(archive, 'r:gz')` reports a bad gzip magic number or compression method, which Python's `gzip`
/// raises as `BadGzipFile`, as "not a gzip file".
fn check_gzip_header(path: &PyPath) -> Result<()> {
    let mut header = Vec::with_capacity(3);
    open(path)?
        .take(3)
        .read_to_end(&mut header)
        .map_err(|error| os_failure(&error, path.as_bytes()))?;
    if header.len() >= 2
        && (header[..2] != [0x1f, 0x8b] || header.get(2).is_some_and(|method| *method != 8))
    {
        return Err(violation("not a gzip file"));
    }
    Ok(())
}

fn archive_failure(error: ArchiveError, path: &PyPath) -> Error {
    match error {
        ArchiveError::Io(error) if error.raw_os_error().is_some() => {
            os_failure(&error, path.as_bytes())
        }
        other => violation(other.to_string()),
    }
}

/// `evidence_store.unpack`: restore every indexed member of a checked archive into `destination`, a new
/// directory, checking each. Members in `durable` are synced, because they are about to be linked into place.
fn unpack(
    disk: &mut dyn Disk,
    archive_path: &PyPath,
    package: &Package,
    destination: &PyPath,
    durable: &HashSet<&str>,
) -> Result<()> {
    checked_file(archive_path, &package.sha256, package.bytes, None)?;
    check_gzip_header(archive_path)?;
    let expected: HashMap<&[u8], &Member> = package
        .members
        .iter()
        .map(|member| (member.path.as_bytes(), member))
        .collect();
    // One member past the index reaches the check below, so an extra or repeated member is named as Python names
    // it rather than counted.
    let limits = Limits {
        members: package.members.len() as u64 + 1,
        payload: package.members.iter().map(|member| member.bytes).sum(),
    };
    let mut seen: HashSet<Vec<u8>> = HashSet::new();
    let mut failure: Option<Error> = None;
    let stop = || ArchiveError::Refused(Refusal::Malformed("restoration stopped".to_string()));
    let unexpected = |name: &[u8]| {
        violation(format!(
            "unexpected archive member: {}",
            String::from_utf8_lossy(name)
        ))
    };
    let result = archive::read_tar_gz(open(archive_path)?, &limits, |item, data| {
        let outcome = (|| -> Result<()> {
            let member = match expected.get(item.name.as_slice()) {
                Some(member) if !seen.contains(&item.name) => *member,
                _ => return Err(unexpected(&item.name)),
            };
            if item.size != member.bytes || item.mode != member.mode {
                return Err(violation(format!(
                    "archive metadata differs: {}",
                    member.path
                )));
            }
            let path = confined(destination, &member.path)?;
            make_dirs(&parent(&path))?;
            let os_path = path.to_path_buf();
            let mut file = disk
                .create(&os_path)
                .map_err(|error| os_failure(&error, path.as_bytes()))?;
            let mut buffer = vec![0u8; 1024 * 1024];
            loop {
                let read = data
                    .read(&mut buffer)
                    .map_err(|error| archive_failure(ArchiveError::from(error), archive_path))?;
                if read == 0 {
                    break;
                }
                file.write_all(&buffer[..read])
                    .map_err(|error| os_failure(&error, path.as_bytes()))?;
            }
            std::fs::set_permissions(&os_path, std::fs::Permissions::from_mode(member.mode))
                .map_err(|error| os_failure(&error, path.as_bytes()))?;
            if durable.contains(member.path.as_str()) {
                disk.sync(&file, &os_path)
                    .map_err(|error| os_failure(&error, path.as_bytes()))?;
            }
            drop(file);
            checked_file(&path, &member.sha256, member.bytes, Some(member.mode))?;
            seen.insert(item.name.clone());
            Ok(())
        })();
        outcome.map_err(|error| {
            failure = Some(error);
            stop()
        })
    });
    if let Some(error) = failure {
        return Err(error);
    }
    match result {
        Ok(()) if seen.len() != package.members.len() => {
            Err(violation("archive is missing indexed members"))
        }
        Ok(()) => Ok(()),
        Err(ArchiveError::Refused(
            Refusal::Duplicate { name } | Refusal::NotRegular { name, .. },
        )) => {
            let name = name.strip_suffix(b"/").map_or(name.clone(), <[u8]>::to_vec);
            Err(unexpected(&name))
        }
        Err(ArchiveError::Refused(Refusal::SpecialMode { name, .. })) => {
            match expected.get(name.as_slice()) {
                Some(member) if !seen.contains(&name) => Err(violation(format!(
                    "archive metadata differs: {}",
                    member.path
                ))),
                _ => Err(unexpected(&name)),
            }
        }
        Err(error) => Err(archive_failure(error, archive_path)),
    }
}

fn release_matches(release: &Value, package: &Package, publication: &Publication) -> Result<bool> {
    let release = object(release)?;
    if !field(release, "id")?.py_eq(&publication.release_id) {
        return Ok(false);
    }
    let asset_name = Value::Str(package.asset.clone());
    let asset_size = Value::Int(package.bytes.to_string());
    for asset in array(field(release, "assets")?)? {
        let asset = object(asset)?;
        if field(asset, "id")?.py_eq(&publication.asset_id)
            && field(asset, "name")?.py_eq(&asset_name)
            && field(asset, "size")?.py_eq(&asset_size)
        {
            return Ok(true);
        }
    }
    Ok(false)
}

/// `evidence_store.download_package`: check the release still names the published asset, then download it into
/// the new file `archive_path`, synced when `durable`, and check its bytes.
fn download_package(
    store: &mut Store<'_>,
    manifest: &Manifest,
    package: &Package,
    archive_path: &PyPath,
    durable: bool,
) -> Result<()> {
    let release = store.remote.release(&manifest.repository, &manifest.tag)?;
    if object(&release)?.get("draft").is_some_and(Value::truthy) {
        return Err(violation("published release became a draft"));
    }
    let publication = package
        .publication
        .as_ref()
        .ok_or_else(|| violation("package has no publication receipt"))?;
    if !release_matches(&release, package, publication)? {
        return Err(violation("published release or asset identity changed"));
    }
    let os_path = archive_path.to_path_buf();
    let mut file = store
        .disk
        .create(&os_path)
        .map_err(|error| os_failure(&error, archive_path.as_bytes()))?;
    store.remote.download(
        &manifest.repository,
        &publication.asset_id.py_str(),
        &mut file,
        package.bytes,
    )?;
    if durable {
        store
            .disk
            .sync(&file, &os_path)
            .map_err(|error| os_failure(&error, archive_path.as_bytes()))?;
    }
    drop(file);
    checked_file(archive_path, &package.sha256, package.bytes, None)
}

/// `evidence_store.verify_remote`: download every package into an empty temporary directory and restore it there.
pub fn verify_remote(store: &mut Store<'_>, manifest: &Value) -> Result<()> {
    let manifest = validate(manifest, true)?;
    for package in &manifest.packages {
        let temp = TempDir::new(&store.temporary, "evidence-verify-")?;
        let archive_path = temp.path.join(package.asset.as_bytes());
        download_package(store, &manifest, package, &archive_path, false)?;
        unpack(
            store.disk,
            &archive_path,
            package,
            &temp.path.join(b"restored"),
            &HashSet::new(),
        )?;
    }
    Ok(())
}

/// `json.dumps(value, indent=2) + '\n'`, written as a new immutable record: to a synced temporary file in the same
/// directory, then linked to its name, never replacing it, and the directory and each of its ancestors synced. An
/// existing record with the same text is accepted, as `evidence_store.write_new` accepts it.
pub fn write_new(disk: &mut dyn Disk, path: &PyPath, value: &Value) -> Result<()> {
    let options = pyjson::Dumps {
        indent: Some(2),
        ..pyjson::Dumps::default()
    };
    write_new_as(disk, path, value, &options)
}

/// `write_new` with `json.dumps` options other than `indent=2`.
pub fn write_new_as(
    disk: &mut dyn Disk,
    path: &PyPath,
    value: &Value,
    options: &pyjson::Dumps,
) -> Result<()> {
    let directory = parent(path);
    make_dirs(&directory)?;
    let reaching = reaching(path);
    let encoded = format!("{}\n", pyjson::dumps(value, options));
    let same = |path: &PyPath| -> Result<()> {
        match pytext::read_text(path) {
            Ok(existing) if existing == encoded => Ok(()),
            Ok(_) => Err(violation(format!("refusing to replace {}", path.display()))),
            Err(pytext::ReadError::Os(message)) => Err(Error::Io(message)),
            Err(pytext::ReadError::Decode(message)) => Err(Error::Violation(message)),
        }
    };
    if exists(path) {
        sync_dirs(disk, &reaching)?;
        return same(path);
    }
    let temporary = temporary_beside(&directory);
    let os_temporary = temporary.to_path_buf();
    let written = (|| -> Result<()> {
        // Named as `open(path, 'x')` names the record when its directory refuses a new file.
        let mut file = disk
            .create(&os_temporary)
            .map_err(|error| os_failure(&error, path.as_bytes()))?;
        file.write_all(encoded.as_bytes())
            .map_err(|error| os_failure(&error, temporary.as_bytes()))?;
        disk.sync(&file, &os_temporary)
            .map_err(|error| os_failure(&error, temporary.as_bytes()))?;
        match disk.link(&os_temporary, &path.to_path_buf()) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => same(path),
            Err(error) => Err(os_failure(&error, path.as_bytes())),
        }
    })();
    let _ = std::fs::remove_file(&os_temporary);
    written?;
    sync_dirs(disk, &reaching)
}

/// A synced temporary file beside `path`, filled by `fill`, which `settle` then moves to `path`. The temporary
/// file is removed whatever happens, and `path`'s directory and each of its ancestors are synced afterwards.
fn through_temporary(
    disk: &mut dyn Disk,
    path: &PyPath,
    mode: Option<u32>,
    fill: &mut dyn FnMut(&mut File) -> io::Result<()>,
    settle: &mut dyn FnMut(&mut dyn Disk, &Path) -> Result<()>,
) -> Result<()> {
    let directory = parent(path);
    let temporary = temporary_beside(&directory);
    let os_temporary = temporary.to_path_buf();
    let written = (|| -> Result<()> {
        let mut file = disk
            .create(&os_temporary)
            .map_err(|error| os_failure(&error, temporary.as_bytes()))?;
        if let Some(mode) = mode {
            file.set_permissions(std::fs::Permissions::from_mode(mode))
                .map_err(|error| os_failure(&error, temporary.as_bytes()))?;
        }
        fill(&mut file).map_err(|error| os_failure(&error, temporary.as_bytes()))?;
        disk.sync(&file, &os_temporary)
            .map_err(|error| os_failure(&error, temporary.as_bytes()))?;
        settle(disk, &os_temporary)
    })();
    let _ = std::fs::remove_file(&os_temporary);
    written?;
    let reaching: BTreeSet<Vec<u8>> = std::iter::once(directory.clone())
        .chain(parents(&directory))
        .map(|ancestor| ancestor.as_bytes().to_vec())
        .collect();
    sync_dirs(disk, &reaching)
}

/// `open(path, 'xb')` and a write, made durable: the bytes go to a synced temporary file that is linked to `path`,
/// so an interruption never leaves a partial file there. An existing entry at `path`, even a dangling link, is the
/// `FileExistsError` Python raises.
pub fn create_new(
    disk: &mut dyn Disk,
    path: &PyPath,
    fill: &mut dyn FnMut(&mut File) -> io::Result<()>,
) -> Result<()> {
    through_temporary(disk, path, None, fill, &mut |disk, temporary| {
        disk.link(temporary, &path.to_path_buf())
            .map_err(|error| os_failure(&error, path.as_bytes()))
    })
}

/// Replace `path` with `data` atomically: a synced temporary file, given `mode` when one is named, renamed over
/// it, then its directory synced. A link at `path` is replaced, never written through.
pub fn replace(disk: &mut dyn Disk, path: &PyPath, data: &[u8], mode: Option<u32>) -> Result<()> {
    through_temporary(
        disk,
        path,
        mode,
        &mut |file| file.write_all(data),
        &mut |disk, temporary| {
            disk.rename(temporary, &path.to_path_buf())
                .map_err(|error| os_failure(&error, path.as_bytes()))
        },
    )
}

/// `evidence_store.restoration_receipt`: verify every package from the remote, then write a new receipt.
pub fn restoration_receipt(
    store: &mut Store<'_>,
    manifest_path: &PyPath,
    destination: &PyPath,
) -> Result<Value> {
    let document = read(manifest_path)?;
    let manifest = validate(&document, true)?;
    verify_remote(store, &document)?;
    let receipt: Object = [
        ("schema_version", Value::Int("1".to_string())),
        ("status", Value::Str("verified".to_string())),
        ("manifest_sha256", Value::Str(digest(manifest_path)?)),
        ("at", Value::Str((store.now)())),
        (
            "subjects",
            Value::Array(manifest.subjects.iter().cloned().map(Value::Str).collect()),
        ),
        (
            "packages",
            Value::Array(
                manifest
                    .packages
                    .iter()
                    .map(|package| Value::Str(package.sha256.clone()))
                    .collect(),
            ),
        ),
        ("method", Value::Str(RECEIPT_METHOD.to_string())),
    ]
    .into_iter()
    .map(|(key, value)| (key.to_string(), value))
    .collect();
    let receipt = Value::Object(receipt);
    write_new(store.disk, destination, &receipt)?;
    Ok(receipt)
}

/// A destination that exists but differs from its member.
struct Differs {
    path: PyPath,
    member: Member,
    bytes: u64,
    sha256: String,
    mode: u32,
}

fn differs_refusal(root: &PyPath, found: &[Differs]) -> Error {
    let aside = root.join(b".cache/evidence/moved-aside");
    let one = found.len() == 1;
    let mut message = format!(
        "{} existing file{} differ{} from the manifest and {} not overwritten:",
        found.len(),
        if one { "" } else { "s" },
        if one { "s" } else { "" },
        if one { "was" } else { "were" }
    );
    let mut directories = Vec::new();
    let mut moves = Vec::new();
    for item in found {
        let _ = write!(
            message,
            "\n  {}: expected {} bytes, sha256 {}, mode {:04o}; found {} bytes, sha256 {}, mode {:04o}",
            item.path.display(),
            item.member.bytes,
            item.member.sha256,
            item.member.mode,
            item.bytes,
            item.sha256,
            item.mode
        );
        let target = parent(
            &aside
                .join(item.sha256.as_bytes())
                .join(item.member.path.as_bytes()),
        );
        let target = crate::shlex::quote(&target.display());
        if !directories.contains(&target) {
            directories.push(target.clone());
        }
        moves.push(format!(
            "mv -- {} {target}/",
            crate::shlex::quote(&item.path.display())
        ));
    }
    message.push_str(if one {
        "\nMove it aside under its SHA-256, then fetch again:\n  "
    } else {
        "\nMove them aside under their SHA-256, then fetch again:\n  "
    });
    let _ = write!(message, "mkdir -p -- {}", directories.join(" "));
    for command in moves {
        message.push_str(" &&\n  ");
        message.push_str(&command);
    }
    violation(message)
}

/// `evidence_store.materialize`, made durable: restore the requested members of a published manifest under
/// `root`, hard-linking them out of `.cache/evidence`.
///
/// Every requested destination is checked before anything is restored. An existing destination that is not a
/// regular file is refused as Python refused it; regular files that differ in size, hash or mode are all listed
/// in one refusal with a command that moves them aside, and nothing is restored.
///
/// `paths` are the raw argument bytes, which argparse decodes with `surrogateescape`, so one that is not UTF-8
/// names no member.
pub fn materialize(
    store: &mut Store<'_>,
    document: &Value,
    root: &PyPath,
    paths: Option<&[Vec<u8>]>,
    offline: bool,
) -> Result<Vec<PyPath>> {
    let manifest = validate(document, true)?;
    let indexed: BTreeSet<&str> = manifest
        .packages
        .iter()
        .flat_map(|package| package.members.iter().map(|member| member.path.as_str()))
        .collect();
    let requested: BTreeSet<&str> = match paths {
        Some(paths) => {
            let mut absent: Vec<Vec<u32>> = paths
                .iter()
                .filter(|path| {
                    std::str::from_utf8(path).map_or(true, |path| !indexed.contains(path))
                })
                .map(|path| pytext::surrogateescape(path))
                .collect();
            absent.sort();
            absent.dedup();
            if !absent.is_empty() {
                let listed: Vec<String> = absent.iter().map(|path| pytext::repr(path)).collect();
                return Err(violation(format!(
                    "paths absent from manifest: [{}]",
                    listed.join(", ")
                )));
            }
            indexed
                .iter()
                .copied()
                .filter(|member| paths.iter().any(|path| path == member.as_bytes()))
                .collect()
        }
        None => indexed.clone(),
    };
    let mut plan: Vec<(&Package, Vec<&Member>)> = Vec::new();
    let mut differs = Vec::new();
    for package in &manifest.packages {
        let mut missing = Vec::new();
        for member in package
            .members
            .iter()
            .filter(|member| requested.contains(member.path.as_str()))
        {
            let path = confined(root, &member.path)?;
            match inspect(&path)? {
                Found::Missing => missing.push(member),
                Found::NotRegular => {
                    return Err(violation(format!(
                        "missing or changed evidence: {}",
                        path.display()
                    )));
                }
                Found::Regular {
                    bytes,
                    sha256,
                    mode,
                } => {
                    if bytes != member.bytes || sha256 != member.sha256 || mode != member.mode {
                        differs.push(Differs {
                            path,
                            member: member.clone(),
                            bytes,
                            sha256,
                            mode,
                        });
                    }
                }
            }
        }
        if !missing.is_empty() {
            plan.push((package, missing));
        }
    }
    if !differs.is_empty() {
        return Err(differs_refusal(root, &differs));
    }
    for (package, missing) in plan {
        restore_package(store, &manifest, package, &missing, root, offline)?;
    }
    let destinations: Vec<PyPath> = requested
        .iter()
        .map(|path| confined(root, path))
        .collect::<Result<_>>()?;
    sync_dirs(store.disk, &settled_directories(root, &destinations))?;
    Ok(destinations)
}

/// Every directory from `root` down to each destination, and the cache, so success also settles links an
/// interrupted earlier fetch made but never synced.
fn settled_directories(root: &PyPath, destinations: &[PyPath]) -> BTreeSet<Vec<u8>> {
    let below_root = |path: &PyPath| path.as_bytes().len() >= root.as_bytes().len();
    let mut directories: BTreeSet<Vec<u8>> = destinations
        .iter()
        .flat_map(parents)
        .filter(below_root)
        .map(|directory| directory.as_bytes().to_vec())
        .collect();
    let cache = root.join(b".cache/evidence");
    directories.extend(
        std::iter::once(cache.clone())
            .chain(parents(&cache))
            .filter(|directory| below_root(directory) && directory.to_path_buf().is_dir())
            .map(|directory| directory.as_bytes().to_vec()),
    );
    directories
}

fn restore_package(
    store: &mut Store<'_>,
    manifest: &Manifest,
    package: &Package,
    missing: &[&Member],
    root: &PyPath,
    offline: bool,
) -> Result<()> {
    let cache = root.join(b".cache/evidence");
    if std::iter::once(cache.clone())
        .chain(parents(&cache))
        .any(|path| is_symlink(&path))
    {
        return Err(violation("evidence cache is symlinked"));
    }
    let mut changed: BTreeSet<Vec<u8>> = make_dirs(&cache)?
        .iter()
        .map(|created| parent(created).as_bytes().to_vec())
        .collect();
    sync_dirs(store.disk, &changed)?;
    changed.clear();
    let archive_path = cache.join(package.asset.as_bytes());
    let temp = TempDir::new(&cache, "tmp")?;
    let durable: HashSet<&str> = missing.iter().map(|member| member.path.as_str()).collect();
    let restored = if exists(&archive_path) || is_symlink(&archive_path) {
        checked_file(&archive_path, &package.sha256, package.bytes, None)?;
        let restored = temp.path.join(b"restored");
        unpack(store.disk, &archive_path, package, &restored, &durable)?;
        restored
    } else {
        if offline {
            return Err(violation(format!(
                "archive unavailable offline: {}",
                package.asset
            )));
        }
        let downloaded = temp.path.join(package.asset.as_bytes());
        download_package(store, manifest, package, &downloaded, true)?;
        let restored = temp.path.join(b"download-check");
        unpack(store.disk, &downloaded, package, &restored, &durable)?;
        match store
            .disk
            .link(&downloaded.to_path_buf(), &archive_path.to_path_buf())
        {
            Ok(()) => {
                store
                    .disk
                    .sync_dir(&cache.to_path_buf())
                    .map_err(|error| os_failure(&error, cache.as_bytes()))?;
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                checked_file(&archive_path, &package.sha256, package.bytes, None)?;
            }
            Err(error) => return Err(os_failure(&error, archive_path.as_bytes())),
        }
        restored
    };
    for member in missing {
        let path = confined(root, &member.path)?;
        let directory = parent(&path);
        changed.extend(
            make_dirs(&directory)?
                .iter()
                .map(|created| parent(created).as_bytes().to_vec()),
        );
        let source = restored.join(member.path.as_bytes());
        match store.disk.link(&source.to_path_buf(), &path.to_path_buf()) {
            Ok(()) => {
                changed.insert(directory.as_bytes().to_vec());
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                checked_file(&path, &member.sha256, member.bytes, Some(member.mode))?;
            }
            Err(error) => return Err(os_failure(&error, path.as_bytes())),
        }
    }
    sync_dirs(store.disk, &changed)
}

/// `evidence_store.manifests`: every `bench/evidence/manifests/*.json` under `root`, in path order, each a
/// published manifest.
pub fn manifests(root: &PyPath) -> Result<Vec<(PyPath, Value)>> {
    let directory = root.join(b"bench/evidence/manifests");
    let mut names: Vec<Vec<u8>> = crate::pyfs::names(directory.as_bytes())
        .into_iter()
        .filter(|name| name.ends_with(b".json"))
        .collect();
    names.sort_by_key(|name| pytext::surrogateescape(name));
    names
        .into_iter()
        .map(|name| {
            let path = directory.join(&name);
            let document = read(&path)?;
            validate(&document, true)?;
            Ok((path, document))
        })
        .collect()
}

/// `evidence_store.resolve`: the file at a logical path, restored from the manifests when absent, and checked
/// against `sha256` when one is given.
pub fn resolve(
    store: &mut Store<'_>,
    root: &PyPath,
    name: &str,
    sha256: Option<&str>,
    offline: bool,
) -> Result<PyPath> {
    let path = confined(root, name)?;
    if path.is_file() {
        if let Some(sha256) = sha256
            && digest(&path)? != sha256
        {
            return Err(violation(format!("changed evidence: {name}")));
        }
        return Ok(path);
    }
    let mut matches: Vec<(Value, String)> = Vec::new();
    for (_, document) in manifests(root)? {
        let manifest = validate(&document, true)?;
        for package in &manifest.packages {
            for member in &package.members {
                if member.path == name {
                    matches.push((document.clone(), member.sha256.clone()));
                }
            }
        }
    }
    if matches
        .iter()
        .map(|(_, sha)| sha)
        .collect::<HashSet<_>>()
        .len()
        > 1
    {
        return Err(violation(format!(
            "conflicting evidence identities: {name}"
        )));
    }
    if let Some((document, member_sha)) = matches.first() {
        if sha256.is_some_and(|sha256| sha256 != member_sha) {
            return Err(violation(format!(
                "locator conflicts with frozen hash: {name}"
            )));
        }
        materialize(
            store,
            document,
            root,
            Some(&[name.as_bytes().to_vec()]),
            offline,
        )?;
    }
    let changed = match sha256 {
        _ if !path.is_file() => true,
        Some(sha256) => digest(&path)? != sha256,
        None => false,
    };
    if changed {
        return Err(violation(format!("missing or changed evidence: {name}")));
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_embedded_schema_is_the_repository_schema() {
        assert_eq!(
            SCHEMA,
            include_str!("../../../bench/schema/evidence-storage.schema.json")
        );
    }

    #[test]
    fn isoformat_matches_python() {
        assert_eq!(isoformat(0, 0), "1970-01-01T00:00:00+00:00");
        assert_eq!(
            isoformat(1_791_244_800, 5),
            "2026-10-06T00:00:00.000005+00:00"
        );
        assert_eq!(
            isoformat(951_782_400, 123_456),
            "2000-02-29T00:00:00.123456+00:00"
        );
    }

    #[test]
    fn logical_paths_match_python() {
        for good in ["bench/a", "artifacts/x/y.tar.gz", "docs/r.md"] {
            assert!(logical_path(good).is_ok(), "{good}");
        }
        for bad in [
            "",
            ".",
            "/bench/a",
            "bench//a",
            "bench/./a",
            "bench/a/",
            "bench/../a",
            "bench/.git/x",
            "bench\\a",
            "src/a",
            "./bench/a",
        ] {
            assert_eq!(
                logical_path(bad),
                Err(Error::Violation(format!("unsafe evidence path: {bad}"))),
                "{bad}"
            );
        }
    }

    #[test]
    fn parents_match_pathlib() {
        let shown = |path: &str| -> Vec<String> {
            parents(&PyPath::new(path.as_bytes()))
                .iter()
                .map(PyPath::display)
                .collect()
        };
        assert_eq!(shown("/a/b"), ["/a", "/"]);
        assert_eq!(shown("a/b"), ["a", "."]);
        assert_eq!(shown("."), Vec::<String>::new());
        assert_eq!(shown("/"), Vec::<String>::new());
    }
}
