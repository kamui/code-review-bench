//! A frozen run's packet, and its original and replacement pins, ported from `bench/tools/packet_selection.py`.
//!
//! [`select`] fails with the exception the Python raises: `ValueError` for an invalid pin, `OSError` for an
//! unreadable input, and `KeyError` or `TypeError` for a record of the wrong shape, each with Python's message,
//! because `current_grading` prints the message of each one it catches.

use crate::pyfs;
use crate::pyjson::Value;
use crate::pyobject;
use crate::pytext::{self, PyPath};
use crate::pyvalue::{self, Py, Raised};
use crate::sha256;
use std::collections::BTreeMap;
use std::sync::Mutex;

fn value_error<T>(message: impl Into<String>) -> Py<T> {
    Err(Raised::Value(message.into()))
}

/// `hashlib.sha256(path.read_bytes()).hexdigest()`.
pub fn digest(path: &PyPath) -> Py<String> {
    pyvalue::read_bytes(path).map(|bytes| sha256::hex_digest(&bytes))
}

/// `Path(value)` for a value `json.loads` returned.
pub fn path_of(value: &Value) -> Py<PyPath> {
    match value {
        Value::Str(text) => Ok(PyPath::new(text.as_bytes())),
        other => Err(Raised::Type(format!(
            "argument should be a str or an os.PathLike object where __fspath__ returns a str, not '{}'",
            other.type_name()
        ))),
    }
}

/// `path.resolve()`.
pub fn resolve(path: &PyPath) -> Py<PyPath> {
    if path.as_bytes().contains(&0) {
        return value_error("lstat: embedded null character in path");
    }
    Ok(PyPath::new(&pyfs::resolve(path.as_bytes())))
}

/// `json.loads(raw)` for bytes, which `json` decodes by the encoding it detects. Rust holds only UTF-8 text, so the
/// UTF-16 and UTF-32 encodings Python detects are refused, as `current-grading-decisions.v1.json` records.
pub fn loads_bytes(raw: &[u8]) -> Py<Value> {
    let utf16_or_32 = raw.starts_with(&[0xFF, 0xFE])
        || raw.starts_with(&[0xFE, 0xFF])
        || raw.starts_with(&[0, 0, 0xFE, 0xFF])
        || (raw.len() >= 4 && (raw[0] == 0 || raw[1] == 0))
        || (raw.len() == 2 && (raw[0] == 0 || raw[1] == 0));
    if utf16_or_32 {
        return Err(Raised::Unsupported(
            "unsupported JSON: only UTF-8 bytes are read".into(),
        ));
    }
    let body = raw.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(raw);
    let text = pytext::decode_utf8(body).map_err(|message| {
        let start = std::str::from_utf8(body)
            .err()
            .map_or(0, |error| error.valid_up_to());
        // `json` decodes with `surrogatepass`, which reads an encoded surrogate that a `str` here cannot hold.
        if body.get(start) == Some(&0xED) && body.get(start + 1).is_some_and(|b| *b >= 0xA0) {
            Raised::Unsupported("unsupported JSON: a lone surrogate".into())
        } else {
            Raised::Value(message)
        }
    })?;
    pyvalue::loads(&text)
}

/// `repository_path(root, name)`: the resolved path, refused when it leaves the repository.
pub fn repository_path(root: &PyPath, name: &Value) -> Py<PyPath> {
    let relative = path_of(name)?;
    let path = resolve(&root.join(relative.as_bytes()))?;
    let raw = relative.as_bytes();
    let inside = pyfs::relative_parts(path.as_bytes(), resolve(root)?.as_bytes()).is_some();
    if raw.starts_with(b"/") || raw.split(|byte| *byte == b'/').any(|part| part == b"..") || !inside
    {
        return value_error(format!(
            "packet path must stay inside the repository: {}",
            name.py_str()
        ));
    }
    Ok(path)
}

/// `text=True` output: strict UTF-8, then universal newlines.
fn decoded(bytes: &[u8]) -> Py<String> {
    let text = pytext::decode_utf8(bytes).map_err(Raised::Value)?;
    Ok(pytext::universal_newlines(&text))
}

/// The run manifest at the run's `freeze_commit`, through `git show`, or `None` when the run has no freeze commit
/// or Git cannot show it. The current record selects a packet for every attempt of a run, so the process keeps each
/// root and spec's `git show` result, a failure included, and never retries a failed show. An error running Git or
/// decoding its output is raised and not kept.
pub fn frozen_manifest(run_dir: &PyPath, manifest: &Value, root: &PyPath) -> Py<Option<Value>> {
    let freeze = pyvalue::get(manifest, "freeze_commit")?;
    if !freeze.truthy() {
        return Ok(None);
    }
    let Ok(relative) = pyfs::relative_to(resolve(run_dir)?.as_bytes(), resolve(root)?.as_bytes())
    else {
        return Ok(None);
    };
    let relative = PyPath::new(&relative).join(b"manifest.json");
    let mut spec = freeze.py_str().into_bytes();
    spec.push(b':');
    spec.extend_from_slice(relative.as_bytes());
    if spec.contains(&0) || root.as_bytes().contains(&0) {
        return value_error("embedded null byte");
    }
    type Shown = BTreeMap<(Vec<u8>, Vec<u8>), Option<String>>;
    static SHOWN: Mutex<Shown> = Mutex::new(BTreeMap::new());
    let key = (root.as_bytes().to_vec(), spec);
    let cached = SHOWN.lock().unwrap().get(&key).cloned();
    let shown = match cached {
        Some(shown) => shown,
        None => {
            let shown = show(root, &key.1)?;
            SHOWN.lock().unwrap().insert(key, shown.clone());
            shown
        }
    };
    shown.as_deref().map(pyvalue::loads).transpose()
}

/// `git show <spec>` in `root`: its stdout, or `None` when Git fails.
fn show(root: &PyPath, spec: &[u8]) -> Py<Option<String>> {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(root.to_path_buf())
        .arg("show")
        .arg(<std::ffi::OsStr as std::os::unix::ffi::OsStrExt>::from_bytes(spec))
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|error| Raised::Os(pytext::os_error(&error, b"git")))?;
    let stdout = decoded(&output.stdout)?;
    decoded(&output.stderr)?;
    Ok((output.status.code() == Some(0)).then_some(stdout))
}

/// `row[key] == target["id"]` for each row, evaluated in Python's order.
fn rows_for(rows: &Value, target: &Value) -> Py<Vec<Value>> {
    let mut found = Vec::new();
    for row in pyvalue::iterate(rows)? {
        if pyobject::eq(pyvalue::item(&row, "target")?, pyvalue::item(target, "id")?) {
            found.push(row);
        }
    }
    Ok(found)
}

fn differs(left: &str, right: &Value) -> bool {
    !pyobject::eq(&Value::Str(left.to_string()), right)
}

/// `packet_selection.select`: the packet the run's reviewers read, after every pin that names it is verified.
pub fn select(
    run_dir: &PyPath,
    manifest: &Value,
    target_dir: &PyPath,
    root: &PyPath,
) -> Py<PyPath> {
    let target_path = target_dir.join(b"target.json");
    let target = loads_bytes(&pyvalue::read_bytes(&target_path)?)?;
    let id = || pyvalue::item(&target, "id").map(Value::py_str);
    let entries = rows_for(pyvalue::item(manifest, "cohort")?, &target)?;
    if entries.len() != 1 {
        return value_error(format!(
            "{}: target must appear exactly once in the cohort",
            id()?
        ));
    }
    let entry = &entries[0];
    let original = target_dir.join(b"packet.md");
    if differs(
        &digest(&original)?,
        pyvalue::item(&target, "packet_sha256")?,
    ) {
        return value_error(format!(
            "{}: original packet.md differs from target.json",
            id()?
        ));
    }
    if !pyobject::eq(
        pyvalue::item(&target, "diff_manifest_sha256")?,
        pyvalue::item(entry, "diff_manifest_sha256")?,
    ) {
        return value_error(format!(
            "{}: target.json and the cohort disagree on the diff identity",
            id()?
        ));
    }
    let mut selected = original.clone();
    let frozen = frozen_manifest(run_dir, manifest, root)?;
    let replaced = pyvalue::has(manifest, "packet_replacements")?
        || match &frozen {
            Some(frozen) => pyvalue::has(frozen, "packet_replacements")?,
            None => false,
        };
    if replaced {
        let pin = pyvalue::get(manifest, "packet_replacements")?;
        if !pyvalue::get(manifest, "freeze_commit")?.truthy()
            || !pyvalue::get(manifest, "frozen_at")?.truthy()
        {
            return value_error("packet replacements require a frozen run");
        }
        let Some(frozen) = frozen else {
            return value_error("cannot read the run manifest at freeze_commit");
        };
        let frozen_entries = rows_for(pyvalue::item(&frozen, "cohort")?, &target)?;
        if !pyobject::eq(pyvalue::get(&frozen, "packet_replacements")?, pin)
            || !pyobject::eq(
                &Value::Array(frozen_entries),
                &Value::Array(entries.clone()),
            )
        {
            return value_error(
                "packet replacement pin or cohort entry differs from freeze_commit",
            );
        }
        let replacement_path =
            repository_path(root, pyvalue::subscript(pin, &pyvalue::text("path"))?)?;
        if differs(
            &digest(&replacement_path)?,
            pyvalue::subscript(pin, &pyvalue::text("sha256"))?,
        ) {
            return value_error("packet replacement manifest differs from the frozen hash");
        }
        let replacements = loads_bytes(&pyvalue::read_bytes(&replacement_path)?)?;
        if !pyobject::eq(
            pyvalue::item(&replacements, "schema_version")?,
            &Value::Int("1".into()),
        ) {
            return value_error("unsupported packet replacement schema_version");
        }
        let rows = pyvalue::iterate(pyvalue::item(&replacements, "targets")?)?;
        let mut ids = Vec::with_capacity(rows.len());
        for row in &rows {
            ids.push(pyvalue::item(row, "target")?.clone());
        }
        if distinct_count(&ids)? != ids.len() {
            return value_error("duplicate packet replacement target");
        }
        let mut replacement = None;
        for row in pyvalue::iterate(pyvalue::item(&replacements, "targets")?)? {
            if pyobject::eq(
                pyvalue::item(&row, "target")?,
                pyvalue::item(&target, "id")?,
            ) {
                replacement = Some(row);
                break;
            }
        }
        if let Some(replacement) = replacement {
            if differs(
                &digest(&target_path)?,
                pyvalue::item(&replacement, "target_sha256")?,
            ) {
                return value_error(format!("{}: frozen target.json changed", id()?));
            }
            let named = pyvalue::item(&replacement, "original")?;
            if repository_path(root, pyvalue::item(named, "path")?)? != resolve(&original)?
                || !pyobject::eq(
                    pyvalue::item(named, "sha256")?,
                    pyvalue::item(&target, "packet_sha256")?,
                )
            {
                return value_error(format!(
                    "{}: original packet identity differs from replacement manifest",
                    id()?
                ));
            }
            let named = pyvalue::item(&replacement, "replacement")?;
            selected = repository_path(root, pyvalue::item(named, "path")?)?;
            if differs(&digest(&selected)?, pyvalue::item(named, "sha256")?) {
                return value_error(format!(
                    "{}: replacement packet differs from replacement manifest",
                    id()?
                ));
            }
        }
    }
    if differs(&digest(&selected)?, pyvalue::item(entry, "packet_sha256")?) {
        return value_error(format!(
            "{}: selected packet differs from the frozen cohort hash",
            id()?
        ));
    }
    Ok(selected)
}

/// `len(set(values))`, with Python 3.14's message for a member it cannot hash.
fn distinct_count(values: &[Value]) -> Py<usize> {
    if let Some(member) = values.iter().find(|value| !value.hashable()) {
        return Err(Raised::Type(format!(
            "cannot use '{0}' as a set element (unhashable type: '{0}')",
            member.type_name()
        )));
    }
    let mut members: Vec<&Value> = Vec::new();
    for value in values {
        if !members.iter().any(|member| pyobject::same(member, value)) {
            members.push(value);
        }
    }
    Ok(members.len())
}
