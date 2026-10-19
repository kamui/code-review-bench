//! Recovery copies and captured original paths: `recover_evidence.py` and `local_evidence_resolution.py`, and the
//! research record `restore-local.py` that runs them.

use super::{Error, Store, confined, digest, logical_path, materialize, read, resolve, validate};
use crate::pyjson::{self, Duplicates, Value};
use crate::pytext::{self, PyPath};
use crate::sha256;
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;

pub const RECOVERY_INDEX: &str = "bench/evidence/recovery/2026-10-08-v1.json";
const CAPTURE_SUMMARY: &str = "docs/research/evidence-migration-2026-10-08/capture-summary.json";
const WITHHELD_SUMMARY: &str = "docs/research/evidence-completion-2026-10-08/withheld-summary.json";

type Result<T> = std::result::Result<T, Error>;

fn violation(message: impl Into<String>) -> Error {
    Error::Violation(message.into())
}

fn get<'a>(value: &'a Value, key: &str) -> Result<&'a Value> {
    match value {
        Value::Object(object) => object
            .get(key)
            .ok_or_else(|| violation(format!("KeyError: {}", pytext::repr_str(key)))),
        other => Err(violation(format!(
            "TypeError: {} indices must be integers or slices, not str",
            other.type_name()
        ))),
    }
}

/// `dict.get(key)`.
fn optional<'a>(value: &'a Value, key: &str) -> Result<Option<&'a Value>> {
    match value {
        Value::Object(object) => Ok(object.get(key)),
        other => Err(violation(format!(
            "AttributeError: '{}' object has no attribute 'get'",
            other.type_name()
        ))),
    }
}

fn items(value: &Value) -> Result<&[Value]> {
    value.as_array().ok_or_else(|| {
        violation(format!(
            "TypeError: '{}' object is not iterable",
            value.type_name()
        ))
    })
}

fn text(value: &Value) -> Result<&str> {
    value.as_str().ok_or_else(|| {
        violation(format!(
            "TypeError: expected str, not {}",
            value.type_name()
        ))
    })
}

/// `recover_evidence.manifest`: the original manifest, retargeted at the recovery release's copies.
fn recovery_manifest(root: &PyPath, name: &str) -> Result<Value> {
    let index = read(&root.join(RECOVERY_INDEX.as_bytes()))?;
    if !optional(&index, "schema_version")?
        .is_some_and(|version| version.py_eq(&Value::Int("1".to_string())))
    {
        return Err(violation("unsupported recovery index"));
    }
    let mut records = Vec::new();
    for entry in items(get(&index, "sources")?)? {
        if get(entry, "name")?.py_eq(&Value::Str(name.to_string())) {
            records.push(entry);
        }
    }
    let [record] = records.as_slice() else {
        return Err(violation(
            "choose exactly one source name from the recovery index",
        ));
    };
    let source = confined(root, text(get(record, "manifest")?)?)?;
    if !Value::Str(digest(&source)?).py_eq(get(record, "manifest_sha256")?) {
        return Err(violation("original discovery manifest changed"));
    }
    let mut result = read(&source)?;
    let original = validate(&result, true)?;
    if !get(&result, "repository")?.py_eq(get(&index, "repository")?) {
        return Err(violation("recovery storage repository differs"));
    }
    let publications = match get(record, "publications")? {
        Value::Object(publications) => publications.clone(),
        other => {
            return Err(violation(format!(
                "TypeError: {} is not a dict",
                other.type_name()
            )));
        }
    };
    let named: std::collections::BTreeSet<&str> =
        publications.iter().map(|(key, _)| key.as_str()).collect();
    let packaged: std::collections::BTreeSet<&str> = original
        .packages
        .iter()
        .map(|package| package.sha256.as_str())
        .collect();
    if named != packaged {
        return Err(violation(
            "recovery index does not cover exactly the original packages",
        ));
    }
    let tag = get(&index, "tag")?.clone();
    let Value::Object(document) = &mut result else {
        return Err(violation("TypeError: the manifest is not a dict"));
    };
    document.insert("tag".to_string(), tag);
    if let Some(Value::Array(packages)) = document.get_mut("packages") {
        for package in packages {
            if let Value::Object(package) = package {
                let sha = package.get("sha256").map(Value::py_str).unwrap_or_default();
                let publication = publications.get(&sha).cloned().unwrap_or(Value::Null);
                package.insert("publication".to_string(), publication);
            }
        }
    }
    validate(&result, true)?;
    Ok(result)
}

/// `recover_evidence.main`: restore an exact package copy from the recorded recovery release.
pub fn recover(
    store: &mut Store<'_>,
    root: &PyPath,
    name: &str,
    paths: Option<&[String]>,
    offline: bool,
) -> Result<()> {
    let manifest = recovery_manifest(root, name)?;
    materialize(store, &manifest, root, paths, offline)?;
    Ok(())
}

/// `json.load(gzip.open(path, 'rt'))`.
fn read_gzip_json(path: &PyPath) -> Result<Value> {
    let file = std::fs::File::open(path.to_path_buf())
        .map_err(|error| Error::Io(pytext::os_error(&error, path.as_bytes())))?;
    let mut bytes = Vec::new();
    crate::archive::gzip_reader(file, u64::MAX)
        .read_to_end(&mut bytes)
        .map_err(|error| match error.raw_os_error() {
            Some(_) => Error::Io(pytext::os_error(&error, path.as_bytes())),
            None => violation(error.to_string()),
        })?;
    let text = pytext::decode_utf8(&bytes).map_err(Error::Violation)?;
    pyjson::loads(&pytext::universal_newlines(&text), Duplicates::KeepLast)
        .map_err(|error| violation(error.to_string()))
}

/// `local_evidence_resolution.resolve`: the published member a withheld original path now resolves to.
fn withheld_reference(store: &mut Store<'_>, root: &PyPath, row: &Value) -> Result<Value> {
    if !root.join(WITHHELD_SUMMARY.as_bytes()).is_file() {
        return Err(violation(format!(
            "Not published: {}",
            text(get(row, "blocked")?)?
        )));
    }
    let summary = read(&root.join(WITHHELD_SUMMARY.as_bytes()))?;
    let reference = get(&summary, "mapping")?;
    let sha256 = match get(reference, "sha256")? {
        Value::Null => None,
        other => Some(text(other)?.to_string()),
    };
    let mapping = resolve(
        store,
        root,
        text(get(reference, "path")?)?,
        sha256.as_deref(),
        false,
    )?;
    let document = read_gzip_json(&mapping)?;
    let supported = optional(&document, "schema_version")?
        .is_some_and(|version| version.py_eq(&Value::Int("1".to_string())))
        && matches!(
            optional(&document, "retirement_authorized")?,
            Some(Value::Bool(false))
        );
    if !supported {
        return Err(violation("unsupported local-evidence resolution mapping"));
    }
    let mut matches = Vec::new();
    for entry in items(get(&document, "entries")?)? {
        if get(entry, "root")?.py_eq(get(row, "root")?)
            && get(entry, "path")?.py_eq(get(row, "path")?)
        {
            matches.push(entry);
        }
    }
    let exact = match matches.as_slice() {
        [entry] => {
            let mut same = true;
            for key in ["sha256", "bytes", "mode"] {
                same = same && get(entry, key)?.py_eq(get(row, key)?);
            }
            same.then_some(*entry)
        }
        _ => None,
    };
    let Some(entry) = exact else {
        return Err(violation(
            "resolution must match exactly one original path, hash, size and mode",
        ));
    };
    let reference = get(entry, "storage")?;
    let keys: Vec<&str> = match reference {
        Value::Object(object) => object.iter().map(|(key, _)| key.as_str()).collect(),
        _ => Vec::new(),
    };
    if keys != ["path"] {
        return Err(violation("resolution must name one published member"));
    }
    match get(reference, "path")? {
        Value::Str(path) => logical_path(path)?,
        _ => return Err(violation("evidence paths must be strings")),
    }
    Ok(reference.clone())
}

/// `PurePosixPath(text).name`.
fn path_name(text: &str) -> String {
    let path = PyPath::new(text.as_bytes());
    let bytes = path.as_bytes();
    if bytes == b"." || bytes.iter().all(|byte| *byte == b'/') {
        return String::new();
    }
    String::from_utf8_lossy(
        bytes
            .rsplit(|byte| *byte == b'/')
            .next()
            .unwrap_or_default(),
    )
    .into_owned()
}

/// A Git reference path the research script accepts: relative, normalized and without `..` or `.git`.
fn safe_git_path(text: &str) -> bool {
    let normalized = PyPath::new(text.as_bytes());
    !text.starts_with('/')
        && normalized.as_bytes() == text.as_bytes()
        && !text.split('/').any(|part| part == ".." || part == ".git")
}

fn require(condition: bool, message: &str) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(violation(message))
    }
}

fn read_bytes(path: &PyPath) -> Result<Vec<u8>> {
    std::fs::read(path.to_path_buf())
        .map_err(|error| Error::Io(pytext::os_error(&error, path.as_bytes())))
}

/// `restore-local.py ROOT_NAME ORIGINAL_RELATIVE_PATH NEW_FILE`, run in the repository `root`: restore one captured
/// original path to a new file, without writing to its original workspace. Returns the line it prints.
pub fn restore_local(
    store: &mut Store<'_>,
    root: &PyPath,
    label: &str,
    original: &str,
    output_name: &[u8],
) -> Result<String> {
    let summary = read(&root.join(CAPTURE_SUMMARY.as_bytes()))?;
    let mut roots = Vec::new();
    for row in items(get(&summary, "roots")?)? {
        if path_name(text(get(row, "root")?)?) == label {
            roots.push(row);
        }
    }
    let [captured] = roots.as_slice() else {
        return Err(violation(
            "Choose exactly one root name from capture-summary.json",
        ));
    };
    let mapping_sha = match get(captured, "sha256")? {
        Value::Null => None,
        other => Some(text(other)?.to_string()),
    };
    let mapping = resolve(
        store,
        root,
        text(get(captured, "mapping")?)?,
        mapping_sha.as_deref(),
        false,
    )?;
    let document = read_gzip_json(&mapping)?;
    let mut rows = Vec::new();
    for row in items(get(&document, "entries")?)? {
        if get(row, "path")?.py_eq(&Value::Str(original.to_string())) {
            rows.push(row);
        }
    }
    let [row] = rows.as_slice() else {
        return Err(violation(
            "Path was not selected; consult the migration inventory",
        ));
    };
    let reference = if optional(row, "blocked")?.is_some_and(Value::truthy) {
        withheld_reference(store, root, row)?
    } else {
        get(row, "storage")?.clone()
    };
    let expected_sha = text(get(row, "sha256")?)?.to_string();
    let data = if optional(&reference, "git_commit")?.is_some_and(Value::truthy) {
        let name = text(get(&reference, "path")?)?;
        require(safe_git_path(name), "Unsafe Git reference path")?;
        let local = root.join(name.as_bytes());
        let symlinked = std::fs::symlink_metadata(local.to_path_buf())
            .is_ok_and(|metadata| metadata.file_type().is_symlink());
        if local.is_file() && !symlinked && digest(&local)? == expected_sha {
            read_bytes(&local)?
        } else {
            let object = format!("artifacts/local-capture/git-objects/{expected_sha}");
            read_bytes(&resolve(store, root, &object, Some(&expected_sha), false)?)?
        }
    } else {
        let path = text(get(&reference, "path")?)?;
        read_bytes(&resolve(store, root, path, Some(&expected_sha), false)?)?
    };
    let size = Value::Int(data.len().to_string());
    require(
        size.py_eq(get(row, "bytes")?) && sha256::hex_digest(&data) == expected_sha,
        "Restored bytes differ from original inventory",
    )?;
    let output = root.join(output_name);
    let linked = std::iter::once(output.clone())
        .chain(super::parents(&output))
        .any(|path| {
            std::fs::symlink_metadata(path.to_path_buf())
                .is_ok_and(|metadata| metadata.file_type().is_symlink())
        });
    require(!linked, "Output must not use symlinks")?;
    let directory = super::parents(&output)
        .into_iter()
        .next()
        .unwrap_or_else(|| PyPath::new(b"."));
    std::fs::create_dir_all(directory.to_path_buf())
        .map_err(|error| Error::Io(pytext::os_error(&error, directory.as_bytes())))?;
    let io_failure = |error: std::io::Error| Error::Io(pytext::os_error(&error, output.as_bytes()));
    let mut file = std::fs::File::options()
        .write(true)
        .create_new(true)
        .open(output.to_path_buf())
        .map_err(io_failure)?;
    file.write_all(&data).map_err(io_failure)?;
    drop(file);
    let mode = match get(row, "mode")? {
        Value::Int(digits) => digits
            .parse::<u32>()
            .map_err(|_| violation(format!("OverflowError: mode {digits} is out of range")))?,
        other => {
            return Err(violation(format!(
                "TypeError: mode must be int, not {}",
                other.type_name()
            )));
        }
    };
    std::fs::set_permissions(output.to_path_buf(), std::fs::Permissions::from_mode(mode))
        .map_err(io_failure)?;
    Ok(format!(
        "Verified {expected_sha}; restored {} bytes to {}",
        get(row, "bytes")?.py_str(),
        output.display()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_name_matches_pathlib() {
        assert_eq!(path_name("/original/ws-a"), "ws-a");
        assert_eq!(path_name("/original/ws-a/"), "ws-a");
        assert_eq!(path_name("/"), "");
        assert_eq!(path_name(""), "");
    }

    #[test]
    fn git_reference_paths_match_the_script() {
        assert!(safe_git_path("bench/a.md"));
        for bad in ["/a", "a/../b", "a/.git/b", "a//b", "a/./b", "a/"] {
            assert!(!safe_git_path(bad), "{bad}");
        }
    }

    #[test]
    fn the_object_type_is_reported_for_a_bad_index() {
        let object = crate::pyjson::Object::default();
        assert!(matches!(
            get(&Value::Object(object), "sources"),
            Err(Error::Violation(message)) if message == "KeyError: 'sources'"
        ));
    }
}
