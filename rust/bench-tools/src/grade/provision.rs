//! The parts of `bench/tools/provision.py` that preparation calls in process: a cache replacement, the archive
//! check of the offline preflight and the removal of a failed provisioner's directories. The provisioner itself
//! and `provision.py check` still run as Python until #157 ports them.

use crate::current_grading::{Failure, R};
use crate::pyjson::Value;
use crate::pytext::{self, PyPath};
use crate::pyvalue::{self, Py, Raised};
use crate::schema::Validator;
use crate::{pyfs, pyobject, sha256};

pub const EMPTY_CACHE_KIND: &str = "none";

fn text(value: &str) -> Value {
    Value::Str(value.to_string())
}

fn object(entries: Vec<(&str, Value)>) -> Value {
    Value::Object(
        entries
            .into_iter()
            .map(|(key, value)| (key.to_string(), value))
            .collect(),
    )
}

fn value_error<T>(message: &str) -> Py<T> {
    Err(Raised::Value(message.to_string()))
}

fn sha256_file(path: &PyPath) -> Py<String> {
    Ok(sha256::hex_digest(&pyvalue::read_bytes(path)?))
}

fn loads_bytes(raw: &[u8]) -> Py<Value> {
    pyvalue::loads(&pytext::decode_utf8(raw).map_err(Raised::Value)?)
}

/// `cache_config(target)["kind"]`.
pub fn cache_kind(target: &Value) -> Py<Value> {
    let provisioning = match pyvalue::get(target, "provisioning")? {
        Value::Null => Value::Object(Default::default()),
        found => found.clone(),
    };
    let cache = pyvalue::get(&provisioning, "cache")?;
    if cache.truthy() {
        if let Some(kind) = cache.as_object().and_then(|cache| cache.get("kind")) {
            return Ok(kind.clone());
        }
        if cache.as_object().is_none() {
            return Err(Raised::Type(format!(
                "'{}' object is not iterable",
                cache.type_name()
            )));
        }
    }
    Ok(text(EMPTY_CACHE_KIND))
}

/// `provision.apply_replacement(target, target_path, manifest_path)`, with the schema `checkout` holds. The
/// error is the `ProvisionError` message.
pub fn apply_replacement(
    checkout: &PyPath,
    target: &mut Value,
    target_path: &PyPath,
    manifest_path: &PyPath,
) -> R<Result<(), String>> {
    match replace(checkout, target, target_path, manifest_path) {
        Ok(Ok(())) => Ok(Ok(())),
        Ok(Err(schema)) => Err(Failure::Crash(format!("SchemaError: {schema}"))),
        Err(
            Raised::Os(message)
            | Raised::Value(message)
            | Raised::Key(message)
            | Raised::Type(message),
        ) => Ok(Err(format!("cache replacement: {message}"))),
        Err(other) => Err(Failure::Crash(other.describe())),
    }
}

fn replace(
    checkout: &PyPath,
    target: &mut Value,
    target_path: &PyPath,
    manifest_path: &PyPath,
) -> Py<Result<(), String>> {
    let raw = pyvalue::read_bytes(manifest_path)?;
    let manifest = loads_bytes(&raw)?;
    let schema_raw =
        pyvalue::read_bytes(&checkout.join(b"bench/schema/cache-replacements.schema.json"))?;
    let schema = loads_bytes(&schema_raw)?;
    let problems = match Validator::default().validate(&schema, &manifest) {
        Ok(problems) => problems,
        Err(error) => return Ok(Err(error.0)),
    };
    if !problems.is_empty() {
        return Err(Raised::Value(problems.join("; ")));
    }
    let entries = pyvalue::iterate(pyvalue::item(&manifest, "targets")?)?;
    let mut ids = Vec::new();
    for entry in &entries {
        ids.push(pyvalue::item(entry, "target")?.clone());
    }
    if pyvalue::distinct(&ids)?.len() != ids.len() {
        return value_error("duplicate replacement target");
    }
    let id = pyvalue::item(target, "id")?.clone();
    let Some(entry) = entries
        .iter()
        .find(|entry| pyvalue::item(entry, "target").is_ok_and(|found| pyobject::eq(found, &id)))
        .cloned()
    else {
        return value_error(&format!("no replacement for {}", id.py_str()));
    };
    let target_sha = text(&sha256_file(target_path)?);
    if !pyobject::eq(pyvalue::item(&entry, "target_sha256")?, &target_sha) {
        return value_error("frozen target changed");
    }
    let kind = cache_kind(target)?;
    let wanted = text(&format!("cache archive ({})", kind.py_str()));
    let identities = pyvalue::iterate(pyvalue::item(
        pyvalue::item(target, "provisioning")?,
        "dependency_identity",
    )?)?;
    let mut archives = Vec::new();
    for (position, value) in identities.iter().enumerate() {
        if pyobject::eq(pyvalue::item(value, "name")?, &wanted) {
            archives.push(position);
        }
    }
    if archives.len() != 1
        || !pyobject::eq(
            pyvalue::item(&identities[archives[0]], "sha256")?,
            pyvalue::item(&entry, "original_sha256")?,
        )
    {
        return value_error("original cache identity differs");
    }
    let reference = pyvalue::item(&entry, "build_receipt")?;
    let relative = pyvalue::item(reference, "path")?
        .as_str()
        .ok_or_else(|| Raised::Type("expected str, bytes or os.PathLike object".into()))?;
    if relative.starts_with('/') || relative.split('/').any(|part| part == "..") {
        return value_error("build receipt must be relative to the replacement manifest");
    }
    let receipt_path = crate::current_grading::parent_of(manifest_path).join(relative.as_bytes());
    let receipt_raw = pyvalue::read_bytes(&receipt_path)?;
    if !pyobject::eq(
        &text(&sha256::hex_digest(&receipt_raw)),
        pyvalue::item(reference, "sha256")?,
    ) {
        return value_error("build receipt changed");
    }
    let receipt = loads_bytes(&receipt_raw)?;
    if !receipt_proves(target, &receipt, &entry, &kind, &id)? {
        return value_error(
            "build receipt does not prove the frozen recipe and replacement archive",
        );
    }
    let manifest_text = pytext::decode_utf8(&raw).map_err(Raised::Value)?;
    let receipt_text = pytext::decode_utf8(&receipt_raw).map_err(Raised::Value)?;
    let resolved = |path: &PyPath| {
        String::from_utf8(pyfs::resolve(path.as_bytes()))
            .map_err(|_| Raised::Unsupported("unsupported path: not UTF-8".into()))
    };
    let replacement = object(vec![
        ("contract", text("cache-replacements-v1")),
        ("schema_sha256", text(&sha256::hex_digest(&schema_raw))),
        (
            "manifest",
            object(vec![
                ("path", Value::Str(resolved(manifest_path)?)),
                ("sha256", text(&sha256::hex_digest(&raw))),
            ]),
        ),
        ("manifest_text", Value::Str(manifest_text)),
        ("build_receipt_text", Value::Str(receipt_text)),
        ("target_path", Value::Str(resolved(target_path)?)),
        ("selection", entry.clone()),
    ]);
    let sha = pyvalue::item(&entry, "sha256")?.clone();
    let Value::Object(fields) = target else {
        return Err(Raised::Type("'target' is not a dict".into()));
    };
    fields.insert("_cache_replacement".to_string(), replacement);
    if let Some(Value::Object(provisioning)) = fields.get_mut("provisioning")
        && let Some(Value::Array(identities)) = provisioning.get_mut("dependency_identity")
        && let Some(Value::Object(archive)) = identities.get_mut(archives[0])
    {
        archive.insert("sha256".to_string(), sha);
    }
    Ok(Ok(()))
}

fn receipt_proves(
    target: &Value,
    receipt: &Value,
    entry: &Value,
    kind: &Value,
    id: &Value,
) -> Py<bool> {
    let field = pyvalue::item;
    if !pyobject::eq(field(receipt, "target")?, id)
        || !pyobject::eq(field(receipt, "kind")?, kind)
        || !pyobject::eq(
            field(receipt, "target_sha256")?,
            field(entry, "target_sha256")?,
        )
        || !pyobject::eq(
            field(field(receipt, "archive")?, "sha256")?,
            field(entry, "sha256")?,
        )
    {
        return Ok(false);
    }
    let steps = pyvalue::iterate(field(receipt, "build")?)?;
    let mut commands = Vec::new();
    for step in &steps {
        commands.push(field(step, "command")?.clone());
    }
    let provisioning = pyvalue::get(target, "provisioning")?;
    let cache = match provisioning {
        Value::Null => &pyvalue::NONE,
        found => pyvalue::get(found, "cache")?,
    };
    let build = cache
        .as_object()
        .and_then(|cache| cache.get("build"))
        .cloned()
        .unwrap_or(Value::Array(Vec::new()));
    if !pyobject::eq(&Value::Array(commands), &build) {
        return Ok(false);
    }
    for step in &steps {
        if !pyobject::eq(field(step, "exit_code")?, &Value::Int("0".into())) {
            return Ok(false);
        }
    }
    Ok(true)
}

/// `provision.archive_path(cache_root, target)`.
pub fn archive_path(cache_root: &[u8], id: &str) -> Vec<u8> {
    let mut path = cache_root.to_vec();
    if !path.is_empty() && !path.ends_with(b"/") {
        path.push(b'/');
    }
    path.extend_from_slice(format!("archives/{id}.tar.gz").as_bytes());
    path
}

/// `provision.remove_tree(path)`: owner write and search bits first, since Go makes module-cache directories
/// read-only. The error is the `ProvisionError` message.
pub fn remove_tree(path: &[u8]) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let os = PyPath::new(path).to_path_buf();
    let failed = |error: std::io::Error, at: &std::path::Path| {
        format!(
            "cannot remove {}: {}",
            pytext::stderr_text(path),
            pytext::os_error(&error, pytext::os_bytes(at.as_os_str()))
        )
    };
    let Ok(metadata) = std::fs::symlink_metadata(&os) else {
        return Ok(());
    };
    if !metadata.is_dir() {
        return std::fs::remove_file(&os).map_err(|error| failed(error, &os));
    }
    let mut pending = vec![os.clone()];
    while let Some(directory) = pending.pop() {
        let mode = std::fs::symlink_metadata(&directory)
            .map_err(|error| failed(error, &directory))?
            .permissions()
            .mode();
        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(mode | 0o700))
            .map_err(|error| failed(error, &directory))?;
        for child in std::fs::read_dir(&directory).map_err(|error| failed(error, &directory))? {
            let child = child.map_err(|error| failed(error, &directory))?.path();
            if std::fs::symlink_metadata(&child).is_ok_and(|found| found.is_dir()) {
                pending.push(child);
            }
        }
    }
    std::fs::remove_dir_all(&os).map_err(|error| failed(error, &os))
}

/// What a finished subprocess wrote, kept as bytes until a caller decodes them.
pub struct Finished {
    pub code: i32,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

impl Finished {
    /// Stdout then stderr as `subprocess.run(..., text=True)` decodes them, which raises on bytes that are not
    /// UTF-8 whatever the exit.
    pub fn output(&self) -> R<String> {
        let decode = |bytes: &[u8]| {
            pytext::decode_utf8(bytes)
                .map(|text| pytext::universal_newlines(&text))
                .map_err(|message| Failure::Crash(format!("UnicodeDecodeError: {message}")))
        };
        Ok(decode(&self.stdout)? + &decode(&self.stderr)?)
    }

    /// The last `limit` characters of the stripped output decoded with `surrogateescape`, as stderr writes them,
    /// so a failure can be reported whatever bytes the subprocess wrote.
    pub fn tail(&self, limit: usize) -> String {
        let mut points = universal_newlines(pytext::surrogateescape(&self.stdout));
        points.extend(universal_newlines(pytext::surrogateescape(&self.stderr)));
        let space = |point: &u32| char::from_u32(*point).is_some_and(pytext::is_space);
        let start = points
            .iter()
            .position(|point| !space(point))
            .unwrap_or(points.len());
        let end = points
            .iter()
            .rposition(|point| !space(point))
            .map_or(start, |last| last + 1);
        let stripped = &points[start..end];
        pytext::stderr_points(&stripped[stripped.len().saturating_sub(limit)..])
    }
}

fn universal_newlines(points: Vec<u32>) -> Vec<u32> {
    let mut translated = Vec::with_capacity(points.len());
    let mut points = points.into_iter().peekable();
    while let Some(point) = points.next() {
        if point == u32::from('\r') {
            points.next_if_eq(&u32::from('\n'));
            translated.push(u32::from('\n'));
        } else {
            translated.push(point);
        }
    }
    translated
}

/// Run `command` with its output captured, as `subprocess.run(command, capture_output=True)` does.
pub fn run(command: &[Vec<u8>]) -> R<Finished> {
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::process::ExitStatusExt;
    let program = std::ffi::OsStr::from_bytes(&command[0]);
    let done = std::process::Command::new(program)
        .args(
            command[1..]
                .iter()
                .map(|arg| std::ffi::OsStr::from_bytes(arg)),
        )
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|error| Failure::Input(pytext::os_error(&error, &command[0])))?;
    Ok(Finished {
        code: done
            .status
            .code()
            .unwrap_or_else(|| -done.status.signal().unwrap_or(0)),
        stdout: done.stdout,
        stderr: done.stderr,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_failure_tail_renders_undecodable_bytes_as_python_writes_them_to_stderr() {
        let done = Finished {
            code: 7,
            stdout: b"\r\nfetching \xff\r\n".to_vec(),
            stderr: b"failed\r \xfe ".to_vec(),
        };
        assert_eq!(done.tail(2000), "fetching \\udcff\nfailed\n \\udcfe");
        assert_eq!(done.tail(3), "\n \\udcfe");
        assert!(done.output().is_err());
    }
}
