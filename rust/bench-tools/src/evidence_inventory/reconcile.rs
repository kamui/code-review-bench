//! `reconcile_evidence.py`: a saved host inventory, the record of one root, reconciled with the member identities
//! of the published evidence manifests. It never authorizes retirement.
//!
//! Every failure is one message, which `main` prints after `reconcile_evidence.py: ` with exit 1; Python raised a
//! traceback. The inventory is read with [`pyjson`], so one holding a lone surrogate escape, which Python reads
//! into a `str`, is refused.

use crate::archive;
use crate::evidence::{self, Disk};
use crate::pyjson::{self, Duplicates, Object, Value};
use crate::pytext::{self, PyPath};
use crate::schema::Validator;
use std::collections::HashMap;
use std::io::{Read, Write};

/// Python's `bench/schema/evidence-reconciliation.schema.json`, which a cargo test keeps byte-identical.
pub const SCHEMA: &str = include_str!("evidence-reconciliation.schema.json");

type Result<T> = std::result::Result<T, String>;

/// A member's `(sha256, bytes, mode)` and the first logical path a manifest indexes it under.
pub type Shared = HashMap<(String, String, String), String>;

fn evidence_message(error: evidence::Error) -> String {
    error.message().to_string()
}

/// `reconcile_evidence.member_index`: every published member by its exact identity.
pub fn member_index(root: &PyPath) -> Result<Shared> {
    let mut index = Shared::new();
    for (_, document) in evidence::manifests(root).map_err(evidence_message)? {
        let manifest = evidence::validate(&document, true).map_err(evidence_message)?;
        for package in manifest.packages {
            for member in package.members {
                index
                    .entry((
                        member.sha256,
                        member.bytes.to_string(),
                        member.mode.to_string(),
                    ))
                    .or_insert(member.path);
            }
        }
    }
    Ok(index)
}

/// The integer a JSON number equals, as a Python `dict` lookup compares `1`, `1.0` and `True`.
fn exact_integer(value: &Value) -> Option<String> {
    match value {
        Value::Bool(flag) => Some(if *flag { "1" } else { "0" }.to_string()),
        Value::Int(digits) => Some(digits.clone()),
        Value::Float(number) if number.is_finite() && number.fract() == 0.0 => {
            Some(if *number == 0.0 {
                "0".to_string()
            } else {
                format!("{number:.0}")
            })
        }
        _ => None,
    }
}

fn key_error(key: &str) -> String {
    pytext::repr_str(key)
}

fn subscript<'a>(value: &'a Value, key: &str) -> Result<&'a Value> {
    match value {
        Value::Object(object) => object.get(key).ok_or_else(|| key_error(key)),
        Value::Array(_) => Err("list indices must be integers or slices, not str".to_string()),
        Value::Str(_) => Err("string indices must be integers, not 'str'".to_string()),
        other => Err(format!(
            "'{}' object is not subscriptable",
            other.type_name()
        )),
    }
}

/// `PurePosixPath(text).parts`.
fn path_parts(value: &Value) -> Result<Vec<String>> {
    let Value::Str(text) = value else {
        return Err(format!(
            "argument should be a str or an os.PathLike object where __fspath__ returns a str, not '{}'",
            value.type_name()
        ));
    };
    let root = if text.starts_with("//") && !text.starts_with("///") {
        Some("//")
    } else if text.starts_with('/') {
        Some("/")
    } else {
        None
    };
    Ok(root
        .map(str::to_string)
        .into_iter()
        .chain(
            text.split('/')
                .filter(|part| !part.is_empty() && *part != ".")
                .map(str::to_string),
        )
        .collect())
}

fn text(value: &str) -> Value {
    Value::Str(value.to_string())
}

fn is(value: &Value, expected: &str) -> bool {
    value.as_str() == Some(expected)
}

/// `reconcile_evidence.classify`.
fn classify(entry: &Value, shared: &Shared) -> Result<Value> {
    let Value::Object(source) = entry else {
        return Err(format!("'{}' object is not a mapping", entry.type_name()));
    };
    let class = subscript(entry, "class")?.clone();
    let mut row: Object = source.clone();
    row.insert("inventory_class".to_string(), class);
    row.insert("storage".to_string(), Value::Null);
    let parts = path_parts(subscript(entry, "path")?)?;
    let has = |name: &str| parts.iter().any(|part| part == name);
    let set = |row: &mut Object, key: &str, value: Value| row.insert(key.to_string(), value);
    if !is(subscript(entry, "type")?, "file") {
        set(&mut row, "status", text("retained"));
        set(
            &mut row,
            "reason",
            text("Link or special file; inspection and retirement remain blocked."),
        );
    } else if is(subscript(entry, "class")?, "account-state") {
        set(&mut row, "status", text("excluded"));
        set(
            &mut row,
            "reason",
            text("Authentication stays local and is excluded from publication."),
        );
    } else if let Some(storage) = identity(entry)?.and_then(|key| shared.get(&key)) {
        set(&mut row, "status", text("shared"));
        set(&mut row, "storage", text(storage));
        if row.get("class").is_some_and(|class| is(class, "unknown")) {
            set(&mut row, "class", text("local-evidence"));
        }
        set(
            &mut row,
            "reason",
            text("Exact bytes, size and permissions are indexed by a published evidence manifest."),
        );
    } else {
        set(&mut row, "status", text("retained"));
        let unknown = row.get("class").is_some_and(|class| is(class, "unknown"));
        let path = subscript(entry, "path")?;
        let (class, reason) = if has(".git")
            || parts.iter().any(|part| part.ends_with(".git"))
            || has("mirrors")
        {
            (
                "reproducibility",
                "Source/Git metadata retained; independent reachability and retirement are not established.",
            )
        } else if matches!(parts.get(..2), Some([first, second]) if first == "public" && (second == "evidence" || second == "data"))
            || is(path, "src/routeTree.gen.ts")
        {
            (
                "rebuildable",
                "Generated explorer/router output; this inventory does not verify safe deletion.",
            )
        } else if ["clone", "clone-work", "base", "head"]
            .iter()
            .any(|name| has(name))
            && unknown
        {
            (
                "development",
                "Source or investigation workspace; unique changes have not been ruled out.",
            )
        } else if has("caches") && unknown {
            (
                "reproducibility",
                "Preparation cache material; retained until pinned dependencies and layout are independently verified.",
            )
        } else {
            return Ok(Value::Object(row));
        };
        set(&mut row, "class", text(class));
        set(&mut row, "reason", text(reason));
    }
    Ok(Value::Object(row))
}

/// The `(sha256, bytes, mode)` key of a file entry, or `None` when no published member can equal it.
fn identity(entry: &Value) -> Result<Option<(String, String, String)>> {
    let values = [
        subscript(entry, "sha256")?,
        subscript(entry, "bytes")?,
        subscript(entry, "mode")?,
    ];
    if let Some(unhashable) = values.iter().find(|value| !value.hashable()) {
        return Err(format!("unhashable type: '{}'", unhashable.type_name()));
    }
    Ok(match values {
        [Value::Str(sha), bytes, mode] => exact_integer(bytes)
            .zip(exact_integer(mode))
            .map(|(bytes, mode)| (sha.clone(), bytes, mode)),
        _ => None,
    })
}

/// A `Counter` total: Python's `int`, as signed decimal digits, or `float` sum.
#[derive(Clone, Debug)]
enum Total {
    Int(String),
    Float(f64),
}

/// `|a| + |b|` or `|a| - |b|` (with `|a| >= |b|`) on decimal digit strings.
fn magnitude(a: &[u8], b: &[u8], subtract: bool) -> String {
    let mut digits = Vec::new();
    let mut carry = 0i32;
    for at in 0..a.len().max(b.len()) {
        let digit = |number: &[u8]| {
            number
                .len()
                .checked_sub(at + 1)
                .map_or(0, |index| i32::from(number[index] - b'0'))
        };
        let mut value = digit(a) + carry + if subtract { -digit(b) } else { digit(b) };
        carry = 0;
        if value < 0 {
            value += 10;
            carry = -1;
        } else if value > 9 {
            value -= 10;
            carry = 1;
        }
        digits.push(b'0' + value as u8);
    }
    if carry > 0 {
        digits.push(b'1');
    }
    while digits.len() > 1 && digits.last() == Some(&b'0') {
        digits.pop();
    }
    digits.reverse();
    String::from_utf8(digits).unwrap_or_default()
}

/// The sum of two integers written as `pyjson` writes them, of any length.
fn add_integers(left: &str, right: &str) -> String {
    let split = |number: &str| -> (bool, String) {
        match number.strip_prefix('-') {
            Some(rest) => (true, rest.to_string()),
            None => (false, number.to_string()),
        }
    };
    let ((left_negative, left), (right_negative, right)) = (split(left), split(right));
    let (negative, digits) = if left_negative == right_negative {
        (
            left_negative,
            magnitude(left.as_bytes(), right.as_bytes(), false),
        )
    } else if (left.len(), left.as_str()) >= (right.len(), right.as_str()) {
        (
            left_negative,
            magnitude(left.as_bytes(), right.as_bytes(), true),
        )
    } else {
        (
            right_negative,
            magnitude(right.as_bytes(), left.as_bytes(), true),
        )
    };
    if negative && digits != "0" {
        format!("-{digits}")
    } else {
        digits
    }
}

/// `float(int)`, which Python refuses past the largest float.
fn to_float(digits: &str) -> Result<f64> {
    digits
        .parse::<f64>()
        .ok()
        .filter(|number| number.is_finite())
        .ok_or_else(|| "int too large to convert to float".to_string())
}

fn add(total: Total, value: &Value) -> Result<Total> {
    Ok(match (total, value) {
        (Total::Int(sum), Value::Bool(flag)) => {
            Total::Int(add_integers(&sum, if *flag { "1" } else { "0" }))
        }
        (Total::Int(sum), Value::Int(digits)) => Total::Int(add_integers(&sum, digits)),
        (Total::Int(sum), Value::Float(number)) => Total::Float(to_float(&sum)? + number),
        (Total::Float(sum), Value::Bool(flag)) => Total::Float(sum + f64::from(u8::from(*flag))),
        (Total::Float(sum), Value::Int(digits)) => Total::Float(sum + to_float(digits)?),
        (Total::Float(sum), Value::Float(number)) => Total::Float(sum + number),
        (total, value) => {
            let left = match total {
                Total::Int(_) => "int",
                Total::Float(_) => "float",
            };
            return Err(format!(
                "unsupported operand type(s) for +=: '{left}' and '{}'",
                value.type_name()
            ));
        }
    })
}

/// The JSON object key `json.dumps` writes for a `dict` key.
fn json_key(key: &Value) -> String {
    match key {
        Value::Str(text) => text.clone(),
        Value::Null => "null".to_string(),
        Value::Bool(flag) => flag.to_string(),
        Value::Float(number) => pyjson::float_repr(*number),
        other => other.py_str(),
    }
}

/// `reconcile_evidence.reconcile`: classify every entry, total allocated bytes by class and validate the record.
pub fn reconcile(inventory: &Value, shared: &Shared) -> Result<Value> {
    let entries = subscript(inventory, "entries")?;
    let Value::Array(entries) = entries else {
        return Err(format!("'{}' object is not iterable", entries.type_name()));
    };
    let rows = entries
        .iter()
        .map(|entry| classify(entry, shared))
        .collect::<Result<Vec<_>>>()?;
    let mut totals: Vec<(Value, Total)> = Vec::new();
    for row in &rows {
        let class = subscript(row, "class")?;
        if !class.hashable() {
            return Err(format!("unhashable type: '{}'", class.type_name()));
        }
        let allocated = subscript(row, "allocated_bytes")?;
        match totals.iter_mut().find(|(key, _)| key.py_eq(class)) {
            Some((_, total)) => *total = add(total.clone(), allocated)?,
            None => totals.push((class.clone(), add(Total::Int("0".to_string()), allocated)?)),
        }
    }
    let Value::Object(mut result) = inventory.clone() else {
        unreachable!("subscript accepted only an object")
    };
    result.insert("entries".to_string(), Value::Array(rows));
    result.insert(
        "allocated_bytes_by_class".to_string(),
        Value::Object(
            totals
                .into_iter()
                .map(|(class, total)| {
                    let value = match total {
                        Total::Int(sum) => Value::Int(sum),
                        Total::Float(sum) => Value::Float(sum),
                    };
                    (json_key(&class), value)
                })
                .collect(),
        ),
    );
    result.insert("retirement_authorized".to_string(), Value::Bool(false));
    let result = Value::Object(result);
    validate(&result)?;
    Ok(result)
}

/// `reconcile_evidence.validate`.
pub fn validate(record: &Value) -> Result<()> {
    let schema = pyjson::loads(SCHEMA, Duplicates::KeepLast)
        .map_err(|error| format!("evidence reconciliation schema: {error}"))?;
    let problems = Validator::default()
        .validate(&schema, record)
        .map_err(|error| error.0)?;
    if !problems.is_empty() {
        return Err(problems[..problems.len().min(10)].join("; "));
    }
    let mut names = std::collections::HashSet::new();
    for entry in subscript(record, "entries")?.as_array().unwrap_or_default() {
        let path = subscript(entry, "path")?.as_str().unwrap_or_default();
        let parts = path_parts(&text(path))?;
        let normalized = match parts.first().map(String::as_str) {
            Some("/" | "//") => format!("{}{}", parts[0], parts[1..].join("/")),
            _ => parts.join("/"),
        };
        if path.starts_with('/')
            || parts.is_empty()
            || parts.iter().any(|part| part == "..")
            || normalized != path
            || !names.insert(path.to_string())
        {
            return Err("duplicate or unsafe inventory path".to_string());
        }
        let storage = subscript(entry, "storage")?;
        if is(subscript(entry, "status")?, "shared") {
            if !is(subscript(entry, "type")?, "file")
                || !subscript(entry, "sha256")?.truthy()
                || !storage.truthy()
                || is(subscript(entry, "class")?, "account-state")
            {
                return Err(
                    "shared entry must identify a regular noncredential payload".to_string()
                );
            }
            evidence::logical_path(storage.as_str().unwrap_or_default())
                .map_err(evidence_message)?;
        } else if !matches!(storage, Value::Null) {
            return Err("retained/excluded entry cannot claim shared storage".to_string());
        }
    }
    Ok(())
}

/// The inventory as `json.load` reads it from `open(path, 'rt')`, or from `gzip.open(path, 'rt')` when the name's
/// suffix is `.gz`.
fn read_inventory(path: &PyPath) -> Result<Value> {
    let file = std::fs::File::open(path.to_path_buf())
        .map_err(|error| pytext::os_error(&error, path.as_bytes()))?;
    let name = path
        .as_bytes()
        .rsplit(|byte| *byte == b'/')
        .next()
        .unwrap_or_default();
    let gzipped = matches!(name.iter().rposition(|byte| *byte == b'.'), Some(at) if at > 0 && &name[at..] == b".gz");
    let mut raw = Vec::new();
    if gzipped {
        archive::gzip_reader(file, u64::MAX)
            .read_to_end(&mut raw)
            .map_err(|error| error.to_string())?;
    } else {
        std::io::BufReader::new(file)
            .read_to_end(&mut raw)
            .map_err(|error| pytext::os_error(&error, path.as_bytes()))?;
    }
    let text = pytext::universal_newlines(&pytext::decode_utf8(&raw)?);
    pyjson::loads(&text, Duplicates::KeepLast).map_err(|error| error.to_string())
}

/// `reconcile_evidence.py --inventory INVENTORY [--root ROOT] --out OUT`: write the gzip-compressed reconciliation
/// as a new file and return the summary it prints.
pub fn run(disk: &mut dyn Disk, inventory: &[u8], root: &PyPath, out: &[u8]) -> Result<String> {
    let inventory = read_inventory(&PyPath::new(inventory))?;
    let shared = member_index(root)?;
    let result = reconcile(&inventory, &shared)?;
    let out = PyPath::new(out);
    let directory = evidence::parents(&out)
        .into_iter()
        .next()
        .unwrap_or_else(|| PyPath::new(b"."));
    evidence::make_dirs(&directory).map_err(evidence_message)?;
    let encoded = pyjson::dumps(
        &result,
        &pyjson::Dumps {
            separators: Some((",".to_string(), ":".to_string())),
            ..pyjson::Dumps::default()
        },
    );
    evidence::create_new(disk, &out, &mut |file| {
        let mut writer = archive::gzip_writer(file, 9);
        writer.write_all(encoded.as_bytes())?;
        writer.finish().map(|_| ())
    })
    .map_err(evidence_message)?;
    let rows = subscript(&result, "entries")?
        .as_array()
        .unwrap_or_default();
    let mut statuses: Vec<(String, u64)> = Vec::new();
    for row in rows {
        let status = subscript(row, "status")?.py_str();
        match statuses.iter_mut().find(|(name, _)| *name == status) {
            Some((_, count)) => *count += 1,
            None => statuses.push((status, 1)),
        }
    }
    let summary: Object = [
        ("root".to_string(), subscript(&result, "root")?.clone()),
        ("entries".to_string(), Value::Int(rows.len().to_string())),
        (
            "statuses".to_string(),
            Value::Object(
                statuses
                    .into_iter()
                    .map(|(status, count)| (status, Value::Int(count.to_string())))
                    .collect(),
            ),
        ),
        ("retirement_authorized".to_string(), Value::Bool(false)),
    ]
    .into_iter()
    .collect();
    Ok(pyjson::dumps(
        &Value::Object(summary),
        &pyjson::Dumps::default(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry() -> Value {
        pyjson::loads(
            r#"{"path": "scratch/report.txt", "type": "file", "bytes": 4, "allocated_bytes": 4096, "mode": 384,
                "sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "class": "unknown",
                "reason": "Unclassified scratch."}"#,
            Duplicates::KeepLast,
        )
        .unwrap()
    }

    fn inventory(entries: Vec<Value>) -> Value {
        let mut object: Object = [
            ("schema_version", Value::Int("1".to_string())),
            ("root", text("/host/workspace")),
            ("head", Value::Null),
        ]
        .into_iter()
        .map(|(key, value)| (key.to_string(), value))
        .collect();
        object.insert("entries".to_string(), Value::Array(entries));
        object.insert("missing_tracked".to_string(), Value::Array(Vec::new()));
        object.insert(
            "allocated_bytes_by_class".to_string(),
            pyjson::loads(r#"{"unknown": 4096}"#, Duplicates::KeepLast).unwrap(),
        );
        object.insert("retirement_authorized".to_string(), Value::Bool(false));
        Value::Object(object)
    }

    fn shared() -> Shared {
        [(
            ("a".repeat(64), "4".to_string(), "384".to_string()),
            "artifacts/capture/object".to_string(),
        )]
        .into_iter()
        .collect()
    }

    fn with(key: &str, value: Value) -> Value {
        let Value::Object(mut object) = entry() else {
            unreachable!()
        };
        object.insert(key.to_string(), value);
        Value::Object(object)
    }

    fn row(record: &Value) -> &Value {
        &subscript(record, "entries").unwrap().as_array().unwrap()[0]
    }

    #[test]
    fn the_embedded_schema_is_the_repository_schema() {
        assert_eq!(
            SCHEMA,
            include_str!("../../../../bench/schema/evidence-reconciliation.schema.json")
        );
    }

    #[test]
    fn exact_shared_identity_reconciles_without_authorizing_retirement() {
        let record = reconcile(&inventory(vec![entry()]), &shared()).unwrap();
        let row = row(&record);
        assert!(is(subscript(row, "status").unwrap(), "shared"));
        assert!(is(subscript(row, "class").unwrap(), "local-evidence"));
        assert!(is(subscript(row, "inventory_class").unwrap(), "unknown"));
        assert!(is(
            subscript(row, "storage").unwrap(),
            "artifacts/capture/object"
        ));
        assert_eq!(
            pyjson::dumps(
                subscript(&record, "allocated_bytes_by_class").unwrap(),
                &pyjson::Dumps::default()
            ),
            r#"{"local-evidence": 4096}"#
        );
        assert!(matches!(
            subscript(&record, "retirement_authorized").unwrap(),
            Value::Bool(false)
        ));
    }

    #[test]
    fn changed_bytes_size_or_permissions_remain_local() {
        for (key, value) in [
            ("sha256", text(&"b".repeat(64))),
            ("bytes", Value::Int("5".to_string())),
            ("mode", Value::Int("420".to_string())),
        ] {
            let record = reconcile(&inventory(vec![with(key, value)]), &shared()).unwrap();
            let row = row(&record);
            assert!(is(subscript(row, "status").unwrap(), "retained"), "{key}");
            assert!(matches!(subscript(row, "storage").unwrap(), Value::Null));
            assert!(is(subscript(row, "class").unwrap(), "unknown"));
        }
    }

    #[test]
    fn an_equal_float_or_boolean_matches_as_a_python_dict_lookup_does() {
        let record = reconcile(
            &inventory(vec![with("bytes", Value::Float(4.0))]),
            &shared(),
        );
        assert_eq!(
            record.unwrap_err(),
            "$.entries[0].bytes: expected type integer, got float"
        );
    }

    #[test]
    fn credentials_and_links_do_not_become_shared_by_content_match() {
        for (key, value, status) in [
            ("class", text("account-state"), "excluded"),
            ("type", text("symlink"), "retained"),
        ] {
            let record = reconcile(&inventory(vec![with(key, value)]), &shared()).unwrap();
            let row = row(&record);
            assert!(is(subscript(row, "status").unwrap(), status));
            assert!(matches!(subscript(row, "storage").unwrap(), Value::Null));
        }
    }

    #[test]
    fn unknowns_and_modified_work_remain_protected() {
        for class in ["unknown", "development"] {
            let record =
                reconcile(&inventory(vec![with("class", text(class))]), &Shared::new()).unwrap();
            let row = row(&record);
            assert!(is(subscript(row, "class").unwrap(), class));
            assert!(is(subscript(row, "status").unwrap(), "retained"));
            assert!(subscript(row, "reason").unwrap().truthy());
        }
    }

    #[test]
    fn rejects_omitted_reasons_duplicate_paths_and_forged_authorization() {
        let record = reconcile(&inventory(vec![entry()]), &Shared::new()).unwrap();
        let edit = |change: &dyn Fn(&mut Object)| {
            let Value::Object(mut object) = record.clone() else {
                unreachable!()
            };
            change(&mut object);
            validate(&Value::Object(object))
        };
        let first = |object: &mut Object| -> Object {
            let Some(Value::Array(rows)) = object.get("entries") else {
                unreachable!()
            };
            let Value::Object(row) = rows[0].clone() else {
                unreachable!()
            };
            row
        };
        let omitted = edit(&|object| {
            let row: Object = first(object)
                .iter()
                .filter(|(key, _)| *key != "reason")
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect();
            object.insert(
                "entries".to_string(),
                Value::Array(vec![Value::Object(row)]),
            );
        });
        assert_eq!(
            omitted.unwrap_err(),
            "$.entries[0]: missing required key 'reason'"
        );
        let duplicate = edit(&|object| {
            let row = Value::Object(first(object));
            object.insert("entries".to_string(), Value::Array(vec![row.clone(), row]));
        });
        assert_eq!(duplicate.unwrap_err(), "duplicate or unsafe inventory path");
        let authorized = edit(&|object| {
            object.insert("retirement_authorized".to_string(), Value::Bool(true));
        });
        assert_eq!(
            authorized.unwrap_err(),
            "$.retirement_authorized: expected constant False, got True"
        );
    }

    #[test]
    fn totals_add_integers_of_any_length() {
        assert_eq!(add_integers("999", "1"), "1000");
        assert_eq!(add_integers("-5", "3"), "-2");
        assert_eq!(add_integers("5", "-5"), "0");
        assert_eq!(add_integers("-5", "-7"), "-12");
        let big = "9".repeat(60);
        assert_eq!(add_integers(&big, "1"), format!("1{}", "0".repeat(60)));
    }

    #[test]
    fn paths_are_refused_unless_normalized_relative_and_inside() {
        for path in ["/a", "a/../b", "a//b", "a/./b", "a/", "./a", ".", "//a"] {
            let record = reconcile(&inventory(vec![with("path", text(path))]), &Shared::new());
            assert_eq!(
                record.unwrap_err(),
                "duplicate or unsafe inventory path",
                "{path}"
            );
        }
    }
}
