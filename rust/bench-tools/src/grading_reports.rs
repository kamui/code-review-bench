//! The read-only grading reports: the queue status `regrade_queues.sh status` prints, the attempt archive
//! `regrade.archive_attempt` writes, and the time and cost audit log of `audit_log.py`.
//!
//! Values keep the Python semantics the outputs depend on, through `pysemantics`. Where Python raised an exception
//! its caller did not catch, the failure is the last line of that traceback.

use crate::archive::{self, ArchiveError, Limits, PackageWriter};
use crate::pyjson::{self, Object, Value};
use crate::pysemantics::{
    Failure, Number, Result, at, contains, file_failure, fixed, fromisoformat, get, get_or,
    iterate, join, join_text, length, loads, lookup, maximum, mean, median, number, number_value,
    object, optional, python_sum, read, read_bytes, read_text, text, to_float, type_error,
};
use crate::pytext::{self, PyPath};
use crate::sha256;
use std::io::{self, Read, Write};
use std::os::unix::fs::PermissionsExt;

/// Python 3.14's message for a key that cannot be hashed, such as
/// `cannot use 'tuple' as a dict key (unhashable type: 'list')`.
fn unhashable(container: &str, key_type: &str, element: &Value) -> Failure {
    type_error(format!(
        "cannot use '{key_type}' as a {container} (unhashable type: '{}')",
        element.type_name()
    ))
}

/// Groups in the order their keys first appear, keyed by tuples that compare as Python compares them.
struct Groups<T> {
    entries: Vec<(Vec<Value>, Vec<T>)>,
}

impl<T> Groups<T> {
    fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// `groups.setdefault(key, []).append(item)`.
    fn push(&mut self, key: Vec<Value>, item: T) -> Result<()> {
        if let Some(element) = key.iter().find(|value| !value.hashable()) {
            return Err(unhashable("dict key", "tuple", element));
        }
        let found = self.entries.iter().position(|(existing, _)| {
            existing.len() == key.len() && existing.iter().zip(&key).all(|(a, b)| a.py_eq(b))
        });
        match found {
            Some(at) => self.entries[at].1.push(item),
            None => self.entries.push((key, vec![item])),
        }
        Ok(())
    }

    /// `sorted(groups.items(), key=lambda item: tuple(map(str, item[0])))`.
    fn sorted(self) -> Vec<(Vec<Value>, Vec<T>)> {
        let mut entries = self.entries;
        entries.sort_by_cached_key(|(key, _)| key.iter().map(Value::py_str).collect::<Vec<_>>());
        entries
    }
}

/// `len(set(values))` of hashable values.
fn distinct(values: &[&Value]) -> usize {
    let mut seen: Vec<&Value> = Vec::new();
    for value in values {
        if !seen.iter().any(|known| known.py_eq(value)) {
            seen.push(value);
        }
    }
    seen.len()
}

/// `Path.mkdir(parents=True, exist_ok=True)`, naming the path whose creation failed.
fn make_directories(path: &PyPath) -> Result<()> {
    match std::fs::create_dir(path.to_path_buf()) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let parent = parent(path);
            if parent == *path {
                return Err(file_failure(&error, path.as_bytes()));
            }
            make_directories(&parent)?;
            match std::fs::create_dir(path.to_path_buf()) {
                Ok(()) => Ok(()),
                Err(_) if path.to_path_buf().is_dir() => Ok(()),
                Err(error) => Err(file_failure(&error, path.as_bytes())),
            }
        }
        Err(_) if path.to_path_buf().is_dir() => Ok(()),
        Err(error) => Err(file_failure(&error, path.as_bytes())),
    }
}

fn parent(path: &PyPath) -> PyPath {
    let bytes = path.as_bytes();
    match bytes.iter().rposition(|byte| *byte == b'/') {
        Some(0) => PyPath::new(b"/"),
        Some(cut) => PyPath::new(&bytes[..cut]),
        None => PyPath::new(b"."),
    }
}

/// The components after the root, as `PurePath.parts` lists them.
fn parts(path: &PyPath) -> Vec<&[u8]> {
    path.as_bytes()
        .split(|byte| *byte == b'/')
        .filter(|part| !part.is_empty() && *part != b".")
        .collect()
}

fn name_bytes(path: &PyPath) -> &[u8] {
    parts(path).last().copied().unwrap_or_default()
}

/// `path.name` as text. A name that is not UTF-8 is refused: Python carried it as escaped surrogates, which
/// these outputs cannot hold.
fn name_text(path: &PyPath) -> Result<Value> {
    utf8(name_bytes(path), path)
}

fn utf8(bytes: &[u8], path: &PyPath) -> Result<Value> {
    std::str::from_utf8(bytes)
        .map(text)
        .map_err(|_| Failure::new("ValueError", format!("path is not UTF-8: {}", path.repr())))
}

fn exists(path: &PyPath) -> bool {
    std::fs::metadata(path.to_path_buf()).is_ok()
}

fn is_dir(path: &PyPath) -> bool {
    path.to_path_buf().is_dir()
}

fn lexists(path: &PyPath) -> bool {
    std::fs::symlink_metadata(path.to_path_buf()).is_ok()
}

/// The names in a directory, or none when it cannot be listed, as a `pathlib` glob ignores the error.
fn names(directory: &PyPath) -> Vec<Vec<u8>> {
    crate::pyfs::names(directory.as_bytes())
}

/// `fnmatch` of a name against `*` followed by a literal suffix, or a literal prefix followed by `*`.
fn matches_suffix(name: &[u8], suffix: &[u8]) -> bool {
    name.ends_with(suffix)
}

/// `directory.glob(pattern)` for one wildcard component that ends in `suffix`.
fn glob_suffix(directory: &PyPath, suffix: &[u8]) -> Vec<PyPath> {
    names(directory)
        .into_iter()
        .filter(|name| matches_suffix(name, suffix))
        .map(|name| directory.join(&name))
        .collect()
}

/// `directory.rglob("*" + suffix)`: every entry below `directory` whose name ends in `suffix`, without
/// descending into symbolic links to directories.
fn rglob_suffix(directory: &PyPath, suffix: &[u8], found: &mut Vec<PyPath>) {
    for name in names(directory) {
        let path = directory.join(&name);
        if matches_suffix(&name, suffix) {
            found.push(path.clone());
        }
        let real_directory =
            std::fs::symlink_metadata(path.to_path_buf()).is_ok_and(|metadata| metadata.is_dir());
        if real_directory {
            rglob_suffix(&path, suffix, found);
        }
    }
}

fn sort_paths(paths: &mut [PyPath]) {
    paths.sort_by_cached_key(|path| pytext::path_order(path.as_bytes()));
}

fn digest(path: &PyPath) -> Result<String> {
    Ok(sha256::hex_digest(&read_bytes(path)?))
}

fn reference(path: &PyPath, root: &PyPath) -> Result<Value> {
    let relative = crate::pyfs::relative_to(path.as_bytes(), root.as_bytes())
        .map_err(|message| Failure::new("ValueError", message))?;
    let relative = utf8(&relative, path)?;
    Ok(object(vec![
        ("path", relative),
        ("sha256", text(&digest(path)?)),
    ]))
}

/// `regrade_queues.sh status`: one line per queue from its saved status, client pin and runs, then the free
/// space of the queue's filesystem. Lines written before a failure stay written.
pub fn status(queues: &[Vec<u8>], out: &mut dyn Write) -> Result<()> {
    for argument in queues {
        let queue = PyPath::new(argument);
        let mut line = queue.as_bytes().to_vec();
        if !exists(&queue.join(b"status.json")) {
            line.extend_from_slice(b": not started\n");
            emit(out, &line)?;
            continue;
        }
        let saved = read(&queue.join(b"status.json"))?;
        let mut mapped = 0usize;
        for row in iterate(at(&saved, "batches")?)?.iter() {
            if at(row, "state")?.py_eq(&text("mapped")) {
                mapped += 1;
            }
        }
        let mut states: Vec<Value> = Vec::new();
        for row in iterate(at(&saved, "batches")?)?.iter() {
            let state = at(row, "state")?;
            if !state.hashable() {
                return Err(unhashable("set element", state.type_name(), state));
            }
            if !states.iter().any(|known| known.py_eq(state)) {
                states.push(state.clone());
            }
        }
        states.retain(|state| !state.py_eq(&text("mapped")) && !state.py_eq(&text("pending")));
        let other = sort_states(states)?;
        let runs = at(&saved, "invocations")?;
        let client = if exists(&queue.join(b"client.json")) {
            at(&read(&queue.join(b"client.json"))?, "version")?.py_str()
        } else {
            "not pinned".to_string()
        };
        let spent = match at(&saved, "spentUpperUsd")? {
            Value::Null => "unknown".to_string(),
            value => format!("${}", format_value(value, 2)?),
        };
        let planned = at(&saved, "plannedBatches")?.py_str();
        let state = at(&saved, "state")?.py_str();
        let reason = if at(&saved, "reason")?.truthy() {
            format!(" ({})", at(&saved, "reason")?.py_str())
        } else {
            String::new()
        };
        let also = if other.is_empty() {
            String::new()
        } else {
            format!(" | also {}", join_text(&other)?)
        };
        let budget = at(&saved, "budgetCapUsd")?.py_str();
        let outstanding = at(&saved, "outstandingReservations")?.py_str();
        let count = length(runs)?.py_str();
        let listed = iterate(runs)?;
        let total = sum_lazy(
            listed
                .iter()
                .map(|run| get_or(run, "wallSeconds", Value::Int("0".to_string()))),
        )?;
        let hours = fixed(&Number::Float(total.divide(3600)?), 2)?;
        let mut workers = Vec::new();
        for run in iterate(runs)?.iter() {
            workers.push(at(run, "workers")?.py_str());
        }
        line.extend_from_slice(
            format!(
                ": mapped {mapped} of {planned} | {state}{reason}{also} | spent {spent} of ${budget} at list price | in flight {outstanding} | client {client} | {count} run(s), {hours} h, workers {}\n",
                workers.join("/")
            )
            .as_bytes(),
        );
        emit(out, &line)?;
        let free = free_bytes(&queue)?;
        let gib = fixed(&Number::Float(free as f64 / f64::from(1u32 << 30)), 0)?;
        emit(out, format!("    free space: {gib} GiB\n").as_bytes())?;
    }
    Ok(())
}

fn emit(out: &mut dyn Write, bytes: &[u8]) -> Result<()> {
    out.write_all(bytes)
        .and_then(|()| out.flush())
        .map_err(|error| Failure::new("OSError", error.to_string()))
}

/// `shutil.disk_usage(path).free`: the blocks available to an unprivileged user.
fn free_bytes(path: &PyPath) -> Result<u64> {
    let stats = nix::sys::statvfs::statvfs(&path.to_path_buf()).map_err(|errno| {
        file_failure(&io::Error::from_raw_os_error(errno as i32), path.as_bytes())
    })?;
    Ok((stats.blocks_available() as u64).saturating_mul(stats.fragment_size() as u64))
}

/// `sorted(states)`: text sorts by code point and numbers by value. Python's message for a mixture names the
/// pair its sort compared first, which follows the set's hash order, so this names the text and the other type.
fn sort_states(mut states: Vec<Value>) -> Result<Vec<Value>> {
    if states.len() < 2 || states.iter().all(|state| state.as_str().is_some()) {
        states.sort_by(|a, b| a.as_str().cmp(&b.as_str()));
        return Ok(states);
    }
    if states.iter().all(|state| number(state).is_some()) {
        let mut numbers: Vec<(Number, Value)> = states
            .into_iter()
            .filter_map(|state| number(&state).map(|value| (value, state)))
            .collect();
        numbers.sort_by(|(a, _), (b, _)| {
            if a.less(b) {
                std::cmp::Ordering::Less
            } else if b.less(a) {
                std::cmp::Ordering::Greater
            } else {
                std::cmp::Ordering::Equal
            }
        });
        return Ok(numbers.into_iter().map(|(_, state)| state).collect());
    }
    let first = states
        .iter()
        .find(|state| state.as_str().is_some())
        .unwrap_or(&states[0]);
    let class = |state: &Value| (state.as_str().is_some(), number(state).is_some());
    let other = states
        .iter()
        .find(|state| class(state) != class(first))
        .unwrap_or(&states[1]);
    Err(type_error(format!(
        "'<' not supported between instances of '{}' and '{}'",
        first.type_name(),
        other.type_name()
    )))
}

/// `format(value, f".{digits}f")` of a value `json.loads` returned.
fn format_value(value: &Value, digits: usize) -> Result<String> {
    match value {
        Value::Str(_) => Err(Failure::new(
            "ValueError",
            "Unknown format code 'f' for object of type 'str'",
        )),
        Value::Array(_) | Value::Object(_) | Value::Null => Err(type_error(format!(
            "unsupported format string passed to {}.__format__",
            value.type_name()
        ))),
        other => fixed(&number(other).unwrap_or(Number::Float(0.0)), digits),
    }
}

/// `sum(items)`, reading each item only after adding the one before, and raising the `TypeError` Python
/// raises for an item that is not a number.
fn sum_lazy(items: impl IntoIterator<Item = Result<Value>>) -> Result<Number> {
    let mut numbers = Vec::new();
    for item in items {
        let value = item?;
        match number(&value) {
            Some(item) => numbers.push(item),
            None => {
                let left = match python_sum(&numbers)? {
                    Number::Float(_) => "float",
                    Number::Int(_) => "int",
                };
                return Err(type_error(format!(
                    "unsupported operand type(s) for +: '{left}' and '{}'",
                    value.type_name()
                )));
            }
        }
    }
    python_sum(&numbers)
}

fn sum(values: &[Value]) -> Result<Number> {
    sum_lazy(values.iter().cloned().map(Ok))
}

fn number_or_error(value: &Value, left: &Value, operator: &str) -> Result<Number> {
    number(value).ok_or_else(|| {
        type_error(format!(
            "unsupported operand type(s) for {operator}: '{}' and '{}'",
            left.type_name(),
            value.type_name()
        ))
    })
}

/// `left - right`.
fn subtract(left: &Value, right: &Value) -> Result<Value> {
    match (number(left), number(right)) {
        (Some(a), Some(b)) => number_value(a.subtract(&b)?),
        _ => Err(type_error(format!(
            "unsupported operand type(s) for -: '{}' and '{}'",
            left.type_name(),
            right.type_name()
        ))),
    }
}

/// `total += value`.
fn accumulate(total: &mut Value, value: &Value) -> Result<()> {
    let left = number(total).unwrap_or(Number::Float(0.0));
    let right = number_or_error(value, total, "+=")?;
    *total = number_value(left.add(&right)?)?;
    Ok(())
}

fn round_value(value: Number, digits: usize) -> Result<Value> {
    number_value(value.round_digits(digits))
}

fn as_number(value: &Value) -> Number {
    number(value).unwrap_or(Number::Float(f64::NAN))
}

const EXCLUDED: [&str; 2] = [
    "rebuildable clone and cache",
    "home account configuration and credentials",
];

/// `regrade.archive_attempt(attempt, archives)`: archives the attempt's records, logs, workspace files, reviews,
/// validator and evidence trees, native sessions and Codex configuration into
/// `ARCHIVES/<run>/<target>/<attempt>/evidence.tar.gz`, beside an `evidence.json` receipt, and returns the
/// receipt's path relative to `root` and its SHA-256. A saved receipt is checked and returned; an existing
/// archive without one is never rewritten.
pub fn archive_attempt(root: &PyPath, attempt: &PyPath, archives: &PyPath) -> Result<Value> {
    let files = attempt_files(attempt)?;
    let components = parts(attempt);
    let count = components.len();
    if count < 2 {
        return Err(Failure::new("IndexError", "1"));
    }
    let run = if count >= 3 {
        components[count - 3]
    } else {
        b""
    };
    let output = archives
        .join(run)
        .join(components[count - 2])
        .join(components[count - 1]);
    make_directories(&output)?;
    let receipt = output.join(b"evidence.json");
    if exists(&receipt) {
        let saved = read(&receipt)?;
        let pinned = at(&saved, "archive")?;
        let path = join(root, at(pinned, "path")?)?;
        let actual = digest(&path)?;
        if !at(pinned, "sha256")?.py_eq(&text(&actual)) {
            return Err(Failure::new(
                "ValueError",
                format!("pinned input changed: {}", at(pinned, "path")?.py_str()),
            ));
        }
        return reference(&receipt, root);
    }
    let archive = output.join(b"evidence.tar.gz");
    let handle = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(archive.to_path_buf())
        .map_err(|error| file_failure(&error, archive.as_bytes()))?;
    let members = match members(attempt, &files) {
        Ok(members) => members,
        Err(failure) => {
            drop(handle);
            let _ = std::fs::remove_file(archive.to_path_buf());
            return Err(failure);
        }
    };
    let mut writer = PackageWriter::new(io::BufWriter::new(handle));
    let mut listed = Vec::with_capacity(members.len());
    for (path, name) in &members {
        let file = std::fs::File::open(path.to_path_buf())
            .map_err(|error| file_failure(&error, path.as_bytes()))?;
        let metadata = file
            .metadata()
            .map_err(|error| file_failure(&error, path.as_bytes()))?;
        let mut hashing = Hashing {
            inner: file,
            digest: sha256::Sha256::new(),
        };
        writer
            .append(
                name.as_bytes(),
                metadata.len(),
                metadata.permissions().mode() & 0o7777,
                &mut hashing,
            )
            .map_err(|error| archive_failure(&error, &archive))?;
        listed.push(object(vec![
            ("path", text(name)),
            ("sha256", text(&sha256::hex(&hashing.digest.finalize()))),
        ]));
    }
    writer
        .finish()
        .and_then(|mut out| out.flush().map_err(ArchiveError::Io))
        .map_err(|error| archive_failure(&error, &archive))?;
    let saved = object(vec![
        ("archive", reference(&archive, root)?),
        ("files", Value::Array(listed)),
        (
            "excluded",
            Value::Array(EXCLUDED.iter().map(|reason| text(reason)).collect()),
        ),
    ]);
    let options = pyjson::Dumps {
        indent: Some(2),
        ..pyjson::Dumps::default()
    };
    let mut handle = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(receipt.to_path_buf())
        .map_err(|error| file_failure(&error, receipt.as_bytes()))?;
    handle
        .write_all(format!("{}\n", pyjson::dumps(&saved, &options)).as_bytes())
        .map_err(|error| file_failure(&error, receipt.as_bytes()))?;
    reference(&receipt, root)
}

fn archive_failure(error: &ArchiveError, archive: &PyPath) -> Failure {
    match error {
        ArchiveError::Io(error) => file_failure(error, archive.as_bytes()),
        ArchiveError::Refused(refusal) => Failure::new("ValueError", refusal.to_string()),
    }
}

/// Feeds a member's bytes to the archive and hashes them on the way.
struct Hashing<R> {
    inner: R,
    digest: sha256::Sha256,
}

impl<R: Read> Read for Hashing<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let read = self.inner.read(buf)?;
        self.digest.update(&buf[..read]);
        Ok(read)
    }
}

/// The files `regrade.archive_attempt` collects, sorted as `pathlib` sorts them.
fn attempt_files(attempt: &PyPath) -> Result<Vec<PyPath>> {
    let work = attempt.join(b"work");
    let mut files = glob_suffix(attempt, b".json");
    files.extend(glob_suffix(attempt, b".log"));
    let entries = std::fs::read_dir(work.to_path_buf())
        .map_err(|error| file_failure(&error, work.as_bytes()))?;
    for entry in entries {
        let entry = entry.map_err(|error| file_failure(&error, work.as_bytes()))?;
        let path = work.join(entry.file_name().as_encoded_bytes());
        if path.is_file() {
            files.push(path);
        }
    }
    files.extend(glob_suffix(&work.join(b"reviews"), b".md"));
    for tree in [b"validator".as_slice(), b"evidence"] {
        let mut found = Vec::new();
        rglob_suffix(&work.join(tree), b"", &mut found);
        files.extend(found.into_iter().filter(PyPath::is_file));
    }
    for project in names(&work.join(b"home/.claude/projects")) {
        let directory = work.join(b"home/.claude/projects").join(&project);
        if is_dir(&directory) {
            files.extend(glob_suffix(&directory, b".jsonl"));
        }
    }
    rglob_suffix(&work.join(b"home/.codex/sessions"), b".jsonl", &mut files);
    let config = work.join(b"home/.codex/config.toml");
    if config.is_file() {
        files.push(config);
    }
    sort_paths(&mut files);
    Ok(files)
}

/// Each file with its member name, checked before anything is written: a symbolic link is refused as
/// `archive_attempt` refuses it, and so is any other entry that is not a regular file, or a file whose mode
/// carries setuid, setgid or sticky bits.
fn members(attempt: &PyPath, files: &[PyPath]) -> Result<Vec<(PyPath, String)>> {
    let mut members = Vec::with_capacity(files.len());
    for path in files {
        let metadata = std::fs::symlink_metadata(path.to_path_buf())
            .map_err(|error| file_failure(&error, path.as_bytes()))?;
        if metadata.file_type().is_symlink() {
            return Err(Failure::new(
                "ValueError",
                format!("evidence is a symlink: {}", path.display()),
            ));
        }
        if !metadata.is_file() {
            return Err(Failure::new(
                "ValueError",
                format!("evidence is not a regular file: {}", path.display()),
            ));
        }
        let mode = metadata.permissions().mode() & 0o7777;
        if mode & !0o777 != 0 {
            return Err(Failure::new(
                "ValueError",
                format!(
                    "evidence has special mode bits {mode:o}: {}",
                    path.display()
                ),
            ));
        }
        let name = crate::pyfs::relative_to(path.as_bytes(), attempt.as_bytes())
            .map_err(|message| Failure::new("ValueError", message))?;
        let Value::Str(name) = utf8(&name, path)? else {
            unreachable!("utf8 returns text");
        };
        members.push((path.clone(), name));
    }
    Ok(members)
}

/// The arguments of `audit_log.py`, with the repository whose `bench/arms` it reads.
pub struct AuditLog {
    pub root: Vec<u8>,
    pub out: Vec<u8>,
    pub grading: Vec<Vec<u8>>,
    pub runs: Vec<Vec<u8>>,
}

/// Whether `audit_log.py` caught a failure and exited 2 with its message, rather than raising.
pub fn caught(failure: &Failure) -> bool {
    matches!(
        failure.kind,
        "OSError"
            | "FileNotFoundError"
            | "FileExistsError"
            | "PermissionError"
            | "IsADirectoryError"
            | "NotADirectoryError"
            | "ValueError"
            | "UnicodeDecodeError"
            | "json.decoder.JSONDecodeError"
            | "statistics.StatisticsError"
            | "KeyError"
    )
}

/// `audit_log.build`: writes `reviews.jsonl`, `charges.jsonl`, `grading.jsonl`, `summary.json` and `summary.md`
/// under `out` and returns the line it prints.
pub fn audit_log(arguments: &AuditLog) -> Result<String> {
    let resolve = |path: &[u8]| {
        pytext::realpath(PyPath::new(path).as_bytes(), true)
            .map(|resolved| PyPath::new(&resolved))
            .map_err(|message| Failure::new("OSError", message))
    };
    let mut runs = Vec::with_capacity(arguments.runs.len());
    for run in &arguments.runs {
        runs.push(resolve(run)?);
    }
    let mut roots = Vec::with_capacity(arguments.grading.len());
    for root in &arguments.grading {
        roots.push(resolve(root)?);
    }
    let arms = PyPath::new(
        &pytext::realpath(PyPath::new(&arguments.root).as_bytes(), false)
            .map_err(|message| Failure::new("OSError", message))?,
    )
    .join(b"bench/arms");
    let mut rows = Vec::new();
    for run in &runs {
        rows.extend(review_rows(run, &arms)?);
    }
    let mut charges = Vec::new();
    for run in &runs {
        charges.extend(charge_rows(run)?);
    }
    let mut grading = Vec::new();
    for root in &roots {
        grading.extend(grading_rows(root)?);
    }
    let summary = summarize(&rows, &charges, &grading)?;
    let out = PyPath::new(&arguments.out);
    make_directories(&out)?;
    let lines = |values: &[Value]| -> String {
        values
            .iter()
            .map(|value| format!("{}\n", pyjson::dumps(value, &pyjson::Dumps::default())))
            .collect()
    };
    write_text(&out.join(b"reviews.jsonl"), &lines(&rows))?;
    write_text(&out.join(b"charges.jsonl"), &lines(&charges))?;
    write_text(&out.join(b"grading.jsonl"), &lines(&grading))?;
    let options = pyjson::Dumps {
        indent: Some(2),
        ..pyjson::Dumps::default()
    };
    write_text(
        &out.join(b"summary.json"),
        &format!("{}\n", pyjson::dumps(&summary, &options)),
    )?;
    write_text(&out.join(b"summary.md"), &markdown(&summary)?)?;
    let setups = at(&summary, "setups")?.as_array().unwrap_or_default();
    let mut attempts = Vec::new();
    for setup in setups {
        attempts.push(at(setup, "attempts")?.clone());
    }
    Ok(format!(
        "{} attempts in {} setups, {} setup-task combinations",
        number_value(sum(&attempts)?)?.py_str(),
        setups.len(),
        at(&summary, "combinations")?
            .as_array()
            .unwrap_or_default()
            .len()
    ))
}

fn write_text(path: &PyPath, content: &str) -> Result<()> {
    std::fs::write(path.to_path_buf(), content)
        .map_err(|error| file_failure(&error, path.as_bytes()))
}

const METHODS: [&str; 3] = ["claude-builtin", "codex", "review-code"];

/// `method(run, arm)`: the built-in method, or the run's pinned skill.
fn method(run: &PyPath, arm: &Value) -> Result<Value> {
    let kind = at(arm, "kind")?;
    if !kind.hashable() {
        return Err(unhashable("dict key", kind.type_name(), kind));
    }
    if let Some(known) = METHODS.iter().find(|name| kind.py_eq(&text(name))) {
        return Ok(text(known));
    }
    let pin = read(&run.join(b"inputs/skill-pin.json"))?;
    match get(&pin, "skill_id")? {
        Some(value) if value.truthy() => Ok(value.clone()),
        _ => Ok(at(&pin, "skill")?.clone()),
    }
}

/// `",".join(value)`.
fn comma_join(value: &Value) -> Result<String> {
    if !matches!(value, Value::Array(_) | Value::Object(_) | Value::Str(_)) {
        return Err(type_error("can only join an iterable"));
    }
    let items = iterate(value)?;
    let mut parts = Vec::with_capacity(items.len());
    for (position, item) in items.iter().enumerate() {
        match item {
            Value::Str(part) => parts.push(part.clone()),
            other => {
                return Err(type_error(format!(
                    "sequence item {position}: expected str instance, {} found",
                    other.type_name()
                )));
            }
        }
    }
    Ok(parts.join(","))
}

/// `seconds(start, end)`: whole seconds between two ISO instants, ties to even, or null when either is empty.
fn seconds(start: &Value, end: &Value) -> Result<Value> {
    if !start.truthy() || !end.truthy() {
        return Ok(Value::Null);
    }
    let parse = |value: &Value| -> Result<(i64, bool)> {
        let Value::Str(raw) = value else {
            return Err(Failure::new(
                "AttributeError",
                format!("'{}' object has no attribute 'replace'", value.type_name()),
            ));
        };
        let replaced = raw.replace('Z', "+00:00");
        fromisoformat(&replaced).ok_or_else(|| {
            Failure::new(
                "ValueError",
                format!("Invalid isoformat string: {}", pytext::repr_str(&replaced)),
            )
        })
    };
    let (finish, finish_aware) = parse(end)?;
    let (begin, begin_aware) = parse(start)?;
    if finish_aware != begin_aware {
        return Err(type_error(
            "can't subtract offset-naive and offset-aware datetimes",
        ));
    }
    let micros = finish - begin;
    let sign = if micros < 0 { "-" } else { "" };
    let magnitude = micros.unsigned_abs();
    let elapsed: f64 = format!(
        "{sign}{}.{:06}",
        magnitude / 1_000_000,
        magnitude % 1_000_000
    )
    .parse()
    .unwrap_or(f64::NAN);
    number_value(Number::Float(elapsed).round_integer()?)
}

/// `key in container`, with Python 3.14's message for a container that is not one.
fn member(container: &Value, key: &str) -> Result<bool> {
    match container {
        Value::Object(_) | Value::Array(_) | Value::Str(_) => contains(container, key),
        other => Err(type_error(format!(
            "argument of type '{}' is not a container or iterable",
            other.type_name()
        ))),
    }
}

fn zero() -> Value {
    Value::Int("0".to_string())
}

/// `tokens(path)`: summed request counts from `usage-requests.jsonl`, or nulls when it is not a file.
fn tokens(path: &PyPath) -> Result<Vec<(&'static str, Value)>> {
    const FIELDS: [&str; 5] = [
        "requests",
        "fresh_input",
        "cache_read",
        "cache_write",
        "output",
    ];
    if !path.is_file() {
        return Ok(FIELDS.iter().map(|field| (*field, Value::Null)).collect());
    }
    let mut totals: Vec<Value> = vec![zero(); 5];
    let content = read_text(path)?;
    for line in pytext::splitlines(&content) {
        if pytext::strip(line).is_empty() {
            continue;
        }
        let row = loads(line)?;
        accumulate(&mut totals[0], &Value::Int("1".to_string()))?;
        if member(&row, "cached_input_tokens")? {
            let fresh = subtract(at(&row, "input_tokens")?, at(&row, "cached_input_tokens")?)?;
            accumulate(&mut totals[1], &fresh)?;
            accumulate(&mut totals[2], at(&row, "cached_input_tokens")?)?;
            accumulate(
                &mut totals[3],
                &get_or(&row, "cache_write_input_tokens", zero())?,
            )?;
        } else {
            accumulate(&mut totals[1], &get_or(&row, "input_tokens", zero())?)?;
            accumulate(
                &mut totals[2],
                &get_or(&row, "cache_read_input_tokens", zero())?,
            )?;
            accumulate(
                &mut totals[3],
                &get_or(&row, "cache_creation_input_tokens", zero())?,
            )?;
        }
        accumulate(&mut totals[4], &get_or(&row, "output_tokens", zero())?)?;
    }
    Ok(FIELDS.into_iter().zip(totals).collect())
}

/// `run.glob("attempts/*/attempt.json")`, sorted.
fn attempt_records(run: &PyPath) -> Vec<PyPath> {
    let attempts = run.join(b"attempts");
    if !is_dir(&attempts) {
        return Vec::new();
    }
    let mut found: Vec<PyPath> = names(&attempts)
        .into_iter()
        .map(|name| attempts.join(&name))
        .filter(is_dir)
        .map(|directory| directory.join(b"attempt.json"))
        .filter(lexists)
        .collect();
    sort_paths(&mut found);
    found
}

/// `review_rows(run, arms)`: one row per filed attempt.
fn review_rows(run: &PyPath, arms: &PyPath) -> Result<Vec<Value>> {
    let mut rows = Vec::new();
    for path in attempt_records(run) {
        let record = read(&path)?;
        let cell = at(&record, "cell")?;
        let arm_name = format!("{}.json", at(cell, "arm")?.py_str());
        let arm = read(&arms.join(arm_name.as_bytes()))?;
        let timing = at(&record, "timing")?;
        let usage = at(&record, "usage")?;
        let disposition = at(&record, "disposition")?;
        let mut row: Vec<(&str, Value)> = vec![
            ("run", name_text(run)?),
            ("attempt", at(&record, "attempt_id")?.clone()),
            ("task", at(at(&record, "cell")?, "target")?.clone()),
            ("replicate", at(at(&record, "cell")?, "replicate")?.clone()),
            ("arm", at(&arm, "id")?.clone()),
            ("method", method(run, &arm)?),
        ];
        let observed = at(&record, "observed")?;
        let client = format!(
            "{} {}",
            at(observed, "harness")?.py_str(),
            at(at(&record, "observed")?, "cli_version")?.py_str()
        );
        row.push(("client", text(&client)));
        let model = match get(&arm, "model")? {
            Some(value) if value.truthy() => value.clone(),
            _ => {
                let models = optional(get(at(&record, "observed")?, "models")?);
                let models = if models.truthy() {
                    models
                } else {
                    Value::Array(Vec::new())
                };
                text(&comma_join(&models)?)
            }
        };
        row.push(("model", model));
        row.push(("effort", optional(get(&arm, "effort")?)));
        let valid = disposition.py_eq(&text("valid completed"));
        let Value::Str(written) = disposition else {
            return Err(Failure::new(
                "AttributeError",
                format!(
                    "'{}' object has no attribute 'split'",
                    disposition.type_name()
                ),
            ));
        };
        let outcome = if valid {
            "valid"
        } else {
            written.split(':').next().unwrap_or_default()
        };
        row.push(("outcome", text(outcome)));
        row.push((
            "disposition",
            text(&written.chars().take(200).collect::<String>()),
        ));
        row.push(("replaces", optional(get(&record, "predecessor")?)));
        let dispatched = optional(lookup(timing, &text("dispatched_at"))?);
        let ended = match get(timing, "completed_at")? {
            Some(value) if value.truthy() => value.clone(),
            _ => optional(get(timing, "stopped_at")?),
        };
        row.push(("dispatched_at", dispatched.clone()));
        row.push(("ended_at", ended.clone()));
        row.push(("seconds", seconds(&dispatched, &ended)?));
        row.push(("cost_usd", optional(get(usage, "priced_total_usd")?)));
        row.push(("billing", optional(get(usage, "billing")?)));
        row.push(("metering", optional(get(usage, "metering_status")?)));
        let usage_path = parent(&path).join(b"usage-requests.jsonl");
        row.extend(tokens(&usage_path)?);
        rows.push(object(row));
    }
    Ok(rows)
}

/// `charge_rows(run)`: each recorded non-review charge, after the run's name.
fn charge_rows(run: &PyPath) -> Result<Vec<Value>> {
    let path = run.join(b"charges.jsonl");
    if !path.is_file() {
        return Ok(Vec::new());
    }
    let content = read_text(&path)?;
    let mut rows = Vec::new();
    for line in pytext::splitlines(&content) {
        if pytext::strip(line).is_empty() {
            continue;
        }
        let charge = loads(line)?;
        let Value::Object(entries) = &charge else {
            return Err(type_error(format!(
                "'{}' object is not a mapping",
                charge.type_name()
            )));
        };
        let mut row = Object::default();
        row.insert("run".to_string(), name_text(run)?);
        for (key, value) in entries.iter() {
            row.insert(key.clone(), value.clone());
        }
        rows.push(Value::Object(row));
    }
    Ok(rows)
}

/// `root.glob("*/*/attempt-*/evidence.tar.gz")`, sorted.
fn session_archives(root: &PyPath) -> Vec<PyPath> {
    let mut found = Vec::new();
    for run in names(root) {
        let run = root.join(&run);
        if !is_dir(&run) {
            continue;
        }
        for task in names(&run) {
            let task = run.join(&task);
            if !is_dir(&task) {
                continue;
            }
            for attempt in names(&task) {
                if !attempt.starts_with(b"attempt-") {
                    continue;
                }
                let attempt = task.join(&attempt);
                let archive = attempt.join(b"evidence.tar.gz");
                if is_dir(&attempt) && lexists(&archive) {
                    found.push(archive);
                }
            }
        }
    }
    sort_paths(&mut found);
    found
}

const EISDIR: i32 = 21;

/// The deflate format cannot expand a stream more than 1032 times, so these limits refuse no well-formed
/// gzip archive; they only bound what a malformed one could make the reader allocate.
fn archive_limits(compressed: u64) -> Limits {
    let payload = compressed.saturating_mul(1032).saturating_add(1 << 20);
    Limits {
        members: payload / 512 + 1,
        payload,
    }
}

/// A session archive's member names, and its `work/dispatch.json`: absent, a directory, or a file's bytes.
struct Session {
    names: Vec<Vec<u8>>,
    dispatch: Option<Option<Vec<u8>>>,
}

fn session(archive: &PyPath) -> Result<Session> {
    let file = std::fs::File::open(archive.to_path_buf())
        .map_err(|error| file_failure(&error, archive.as_bytes()))?;
    let metadata = file
        .metadata()
        .map_err(|error| file_failure(&error, archive.as_bytes()))?;
    if metadata.is_dir() {
        return Err(file_failure(
            &io::Error::from_raw_os_error(EISDIR),
            archive.as_bytes(),
        ));
    }
    let size = metadata.len();
    let mut names: Vec<Vec<u8>> = Vec::new();
    let mut directories: Vec<Vec<u8>> = Vec::new();
    let mut dispatch: Option<Option<Vec<u8>>> = None;
    let mut listed = |name: &[u8]| directories.push(name.to_vec());
    let outcome = archive::read_tar_gz_listing(
        io::BufReader::new(file),
        &archive_limits(size),
        &mut listed,
        |member, data| {
            if member.name == b"work/dispatch.json" {
                let mut bytes = Vec::new();
                data.read_to_end(&mut bytes)?;
                dispatch = Some(Some(bytes));
            }
            names.push(member.name.clone());
            Ok(())
        },
    );
    outcome.map_err(|error| {
        Failure::new(
            "tarfile.ReadError",
            format!("{}: {error}", archive.display()),
        )
    })?;
    // Directory names arrive through their own callback; tarfile lists them among the members, which
    // only matters for whether `work/dispatch.json` names one.
    if dispatch.is_none() && directories.iter().any(|name| name == b"work/dispatch.json") {
        dispatch = Some(None);
    }
    names.extend(directories);
    Ok(Session { names, dispatch })
}

/// `json.load` of bytes: UTF-8 with or without its byte-order mark. Other encodings `json` detects are refused.
fn load_bytes(bytes: &[u8]) -> Result<Value> {
    let utf16_or_32 = bytes.starts_with(&[0xff, 0xfe])
        || bytes.starts_with(&[0xfe, 0xff])
        || bytes.starts_with(&[0, 0, 0xfe, 0xff])
        || (bytes.len() >= 4 && (bytes[0] == 0 || bytes[1] == 0))
        || (bytes.len() == 2 && (bytes[0] == 0 || bytes[1] == 0));
    if utf16_or_32 && !bytes.starts_with(&[0xef, 0xbb, 0xbf]) {
        return Err(Failure::new(
            "ValueError",
            "dispatch record is not UTF-8; UTF-16 and UTF-32 are refused",
        ));
    }
    let body = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes);
    let decoded =
        pytext::decode_utf8(body).map_err(|message| Failure::new("UnicodeDecodeError", message))?;
    loads(&decoded)
}

/// `grading_rows(root)`: one row per archived grading session that holds a dispatch record.
fn grading_rows(root: &PyPath) -> Result<Vec<Value>> {
    let mut rows = Vec::new();
    for archive in session_archives(root) {
        let Session { names, dispatch } = session(&archive)?;
        let Some(dispatch) = dispatch else {
            continue;
        };
        let Some(bytes) = dispatch else {
            return Err(Failure::new(
                "AttributeError",
                "'NoneType' object has no attribute 'read'",
            ));
        };
        let record = load_bytes(&bytes)?;
        let attempt = parent(&archive);
        let task = parent(&attempt);
        let run = parent(&task);
        let mut row = vec![
            ("run", name_text(&run)?),
            ("task", name_text(&task)?),
            ("attempt", name_text(&attempt)?),
            ("model", optional(get(&record, "model")?)),
            ("effort", optional(get(&record, "effort")?)),
            (
                "reviews",
                Value::Int(
                    names
                        .iter()
                        .filter(|name| name.starts_with(b"work/reviews/"))
                        .count()
                        .to_string(),
                ),
            ),
        ];
        let dispatched = optional(get(&record, "dispatched_at")?);
        let completed = optional(get(&record, "completed_at")?);
        row.push(("dispatched_at", dispatched.clone()));
        row.push(("ended_at", completed.clone()));
        row.push(("seconds", seconds(&dispatched, &completed)?));
        let usage = optional(get(&record, "usage")?);
        let cost = if usage.truthy() {
            optional(get(&usage, "priced_total_usd")?)
        } else {
            Value::Null
        };
        row.push(("cost_usd", cost));
        row.push(("exit_code", optional(get(&record, "exit_code")?)));
        rows.push(object(row));
    }
    Ok(rows)
}

/// `aggregate(rows)`: attempts, valid reviews, seconds and cost of one group.
fn aggregate(rows: &[&Value]) -> Result<Vec<(&'static str, Value)>> {
    let mut timed = Vec::new();
    let mut priced = Vec::new();
    let mut valid = Vec::new();
    for row in rows {
        let seconds = at(row, "seconds")?;
        if !matches!(seconds, Value::Null) {
            timed.push(seconds.clone());
        }
        let cost = at(row, "cost_usd")?;
        if !matches!(cost, Value::Null) {
            priced.push(cost.clone());
        }
        if at(row, "outcome")?.py_eq(&text("valid")) {
            valid.push(*row);
        }
    }
    let mut valid_seconds = Vec::new();
    let mut valid_cost = Vec::new();
    for row in &valid {
        let seconds = at(row, "seconds")?;
        if !matches!(seconds, Value::Null) {
            valid_seconds.push(as_number(seconds));
        }
        let cost = at(row, "cost_usd")?;
        if !matches!(cost, Value::Null) {
            valid_cost.push(as_number(cost));
        }
    }
    let count = |items: usize| Value::Int(items.to_string());
    let total_seconds = number_value(sum(&timed)?)?;
    let total_cost = round_value(sum(&priced)?, 6)?;
    let median_seconds = if valid_seconds.is_empty() {
        Value::Null
    } else {
        number_value(median(&valid_seconds)?.round_integer()?)?
    };
    let max_seconds = match maximum(&valid_seconds) {
        Some(value) => number_value(value)?,
        None => Value::Null,
    };
    let mean_cost = if valid_cost.is_empty() {
        Value::Null
    } else {
        round_value(mean(&valid_cost)?, 6)?
    };
    let max_cost = match maximum(&valid_cost) {
        Some(value) => round_value(value, 6)?,
        None => Value::Null,
    };
    Ok(vec![
        ("attempts", count(rows.len())),
        ("valid", count(valid.len())),
        ("unmetered", count(rows.len() - priced.len())),
        ("total_seconds", total_seconds),
        ("total_cost_usd", total_cost),
        ("median_valid_seconds", median_seconds),
        ("max_valid_seconds", max_seconds),
        ("mean_valid_cost_usd", mean_cost),
        ("max_valid_cost_usd", max_cost),
    ])
}

fn label(key: &[Value]) -> Vec<(&'static str, Value)> {
    vec![
        ("method", key[0].clone()),
        ("model", key[1].clone()),
        ("effort", key[2].clone()),
    ]
}

/// `summarize(rows, charges, grading)`.
fn summarize(rows: &[Value], charges: &[Value], grading: &[Value]) -> Result<Value> {
    let mut setups = Groups::new();
    let mut combinations = Groups::new();
    for row in rows {
        let setup = vec![
            at(row, "method")?.clone(),
            at(row, "model")?.clone(),
            at(row, "effort")?.clone(),
        ];
        setups.push(setup.clone(), row)?;
        let mut combination = setup;
        combination.push(at(row, "task")?.clone());
        combinations.push(combination, row)?;
    }
    let mut setup_rows = Vec::new();
    for (key, group) in setups.sorted() {
        let tasks: Vec<&Value> = group
            .iter()
            .map(|row| at(row, "task"))
            .collect::<Result<_>>()?;
        let mut entry = label(&key);
        entry.push(("tasks", Value::Int(distinct(&tasks).to_string())));
        entry.extend(aggregate(&group)?);
        setup_rows.push(object(entry));
    }
    let mut combination_rows = Vec::new();
    for (key, group) in combinations.sorted() {
        let mut entry = label(&key);
        entry.push(("task", key[3].clone()));
        entry.extend(aggregate(&group)?);
        combination_rows.push(object(entry));
    }
    let mut dollars = Vec::with_capacity(charges.len());
    for charge in charges {
        dollars.push(Value::Float(to_float(at(charge, "usd")?)?));
    }
    let charge_summary = object(vec![
        ("rows", Value::Int(charges.len().to_string())),
        ("total_usd", round_value(sum(&dollars)?, 6)?),
    ]);
    let mut sessions = Groups::new();
    for row in grading {
        sessions.push(
            vec![at(row, "model")?.clone(), at(row, "effort")?.clone()],
            row,
        )?;
    }
    let mut grading_rows = Vec::new();
    for (key, group) in sessions.sorted() {
        let mut reviews = Vec::new();
        let mut seconds = Vec::new();
        let mut costs = Vec::new();
        let mut timed = Vec::new();
        for row in &group {
            reviews.push(at(row, "reviews")?.clone());
            let elapsed = at(row, "seconds")?;
            seconds.push(if elapsed.truthy() {
                elapsed.clone()
            } else {
                zero()
            });
            let cost = at(row, "cost_usd")?;
            costs.push(if cost.truthy() { cost.clone() } else { zero() });
            if !matches!(elapsed, Value::Null) {
                timed.push(as_number(elapsed));
            }
        }
        grading_rows.push(object(vec![
            ("model", key[0].clone()),
            ("effort", key[1].clone()),
            ("sessions", Value::Int(group.len().to_string())),
            ("reviews", number_value(sum(&reviews)?)?),
            ("total_seconds", number_value(sum(&seconds)?)?),
            ("total_cost_usd", round_value(sum(&costs)?, 6)?),
            (
                "median_session_seconds",
                number_value(median(&timed)?.round_integer()?)?,
            ),
        ]));
    }
    Ok(object(vec![
        ("setups", Value::Array(setup_rows)),
        ("combinations", Value::Array(combination_rows)),
        ("charges", charge_summary),
        ("grading", Value::Array(grading_rows)),
    ]))
}

/// `minutes(value)`.
fn minutes(value: &Value) -> Result<String> {
    match value {
        Value::Null => Ok(String::new()),
        other => fixed(&Number::Float(as_number(other).divide(60)?), 1),
    }
}

/// `money(value)`.
fn money(value: &Value) -> Result<String> {
    match value {
        Value::Null => Ok(String::new()),
        other => Ok(format!("${}", fixed(&as_number(other), 3)?)),
    }
}

/// `total / reviews` for the cost per review, where `reviews` counts archive members.
fn per_review(total: &Value, reviews: &Value) -> Result<Value> {
    match reviews.py_str().parse::<u64>() {
        Ok(0) | Err(_) => Err(Failure::new("ZeroDivisionError", "division by zero")),
        Ok(count) => Ok(Value::Float(as_number(total).divide(count)?)),
    }
}

/// `markdown(summary)`: the per-setup, per-task and grading tables.
fn markdown(summary: &Value) -> Result<String> {
    let mut lines: Vec<String> = vec![
        "# Review time and cost".into(),
        String::new(),
        "Derived from filed attempt records by `bench/tools/audit_log.py`. Minutes are reviewer wall time. Costs are list-price equivalents for subscription usage. Totals count every attempt, including failed and replaced ones.".into(),
        String::new(),
        "## Per setup".into(),
        String::new(),
        "| Method | Model | Effort | Tasks | Attempts | Valid | Total min | Total cost | Median valid min | Mean valid cost |".into(),
        "| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |".into(),
    ];
    let field = |row: &Value, key: &str| at(row, key).map(Value::py_str);
    for row in at(summary, "setups")?.as_array().unwrap_or_default() {
        lines.push(format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
            field(row, "method")?,
            field(row, "model")?,
            field(row, "effort")?,
            field(row, "tasks")?,
            field(row, "attempts")?,
            field(row, "valid")?,
            minutes(at(row, "total_seconds")?)?,
            money(at(row, "total_cost_usd")?)?,
            minutes(at(row, "median_valid_seconds")?)?,
            money(at(row, "mean_valid_cost_usd")?)?,
        ));
    }
    lines.extend([
        String::new(),
        "## Per setup and task".into(),
        String::new(),
        "| Method | Model | Task | Attempts | Valid | Total min | Total cost | Median valid min | Mean valid cost |".into(),
        "| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |".into(),
    ]);
    for row in at(summary, "combinations")?.as_array().unwrap_or_default() {
        lines.push(format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} |",
            field(row, "method")?,
            field(row, "model")?,
            field(row, "task")?,
            field(row, "attempts")?,
            field(row, "valid")?,
            minutes(at(row, "total_seconds")?)?,
            money(at(row, "total_cost_usd")?)?,
            minutes(at(row, "median_valid_seconds")?)?,
            money(at(row, "mean_valid_cost_usd")?)?,
        ));
    }
    let grading = at(summary, "grading")?.as_array().unwrap_or_default();
    if !grading.is_empty() {
        lines.extend([
            String::new(),
            "## Grading".into(),
            String::new(),
            "| Grader | Effort | Sessions | Reviews | Total min | Total cost | Median session min | Cost per review |".into(),
            "| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |".into(),
        ]);
        for row in grading {
            lines.push(format!(
                "| {} | {} | {} | {} | {} | {} | {} | {} |",
                field(row, "model")?,
                field(row, "effort")?,
                field(row, "sessions")?,
                field(row, "reviews")?,
                minutes(at(row, "total_seconds")?)?,
                money(at(row, "total_cost_usd")?)?,
                minutes(at(row, "median_session_seconds")?)?,
                money(&per_review(
                    at(row, "total_cost_usd")?,
                    at(row, "reviews")?
                )?)?,
            ));
        }
    }
    let charges = at(summary, "charges")?;
    lines.extend([
        String::new(),
        format!(
            "Other charges recorded in the runs (setup probes and grading): {} rows, {}. See `charges.jsonl`.",
            field(charges, "rows")?,
            money(at(charges, "total_usd")?)?
        ),
        String::new(),
    ]);
    Ok(lines.join("\n"))
}
