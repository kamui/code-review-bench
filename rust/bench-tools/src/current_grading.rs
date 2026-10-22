//! Inventory and validate the sole-current grading record without model calls, ported from
//! `bench/tools/current_grading.py`.
//!
//! The record's files are read from the repository a command names, the current directory unless `--root` names
//! another. The schemas are the tool's own, as `current_grading.py` read them from its checkout: they are embedded
//! from `bench/schema`, and a test keeps the copies equal. Every fingerprint, digest and serialization is
//! Python's byte for byte. A failure keeps its Python class: [`Failure::Inconsistent`] for inputs that disagree,
//! [`Failure::Input`] for an input that cannot be read, and [`Failure::Crash`] for an exception the Python command
//! line did not catch.

use crate::claim_grading;
use crate::delegation;
use crate::grading_validation;
use crate::packet_selection;
use crate::pre_existing;
use crate::pyfs;
use crate::pyjson::{self, Dumps, Object, Value};
use crate::pyobject;
use crate::pytext::{self, PyPath};
use crate::pyvalue::{self, Raised};
use crate::ruling;
use crate::schema::Validator;
use crate::sha256;
use std::borrow::Cow;
use std::collections::HashMap;

pub const CURRENT: &str = "bench/grading/current";
pub const PROGRAM: &str = "current_grading.py";
const KINDS: [&str; 6] = [
    "reference",
    "adjudication",
    "claim",
    "grade",
    "candidate",
    "credit",
];
const KEPT_STOP_CAUSES: [&str; 2] = ["skill-timeout", "reviewer-stop"];
const V1_CLAIM: [&str; 9] = [
    "id",
    "anchor",
    "canonical_id",
    "outcome",
    "assessment",
    "family_id",
    "duplicate_group",
    "reason",
    "evidence",
];
const V2_CLAIM: [&str; 12] = [
    "id",
    "anchor",
    "canonical_id",
    "outcome",
    "kind",
    "answers",
    "known_problems",
    "open",
    "candidate_id",
    "duplicate_group",
    "reason",
    "evidence",
];
const V2_REVIEW: [&str; 9] = [
    "attempt_id",
    "state",
    "reason",
    "claims",
    "not_findings",
    "families",
    "recommendations",
    "remedy_inventory",
    "advice",
];
const V2_FAMILY: [&str; 6] = [
    "family_id",
    "outcome",
    "claim_ids",
    "sufficiency",
    "reason",
    "cause_only",
];
const V2_RECOMMENDATION: [&str; 5] = ["id", "anchors", "addressed_claims", "safety", "sufficiency"];
/// One ruling admits a pre-existing problem and labels it, so a grade depends only on the facts validation ties
/// to the catalog entry.
const ADMISSION_FACTS: [&str; 7] = [
    "id",
    "target",
    "revision",
    "subject",
    "dimension",
    "status",
    "outcome",
];

const SCHEMAS: [(&str, &str); 10] = [
    (
        "current-cohort-input",
        include_str!("current_grading/current-cohort-input.schema.json"),
    ),
    (
        "current-reference",
        include_str!("current_grading/current-reference.schema.json"),
    ),
    (
        "current-adjudication",
        include_str!("current_grading/current-adjudication.schema.json"),
    ),
    (
        "current-claim",
        include_str!("current_grading/current-claim.schema.json"),
    ),
    (
        "current-grade",
        include_str!("current_grading/current-grade.schema.json"),
    ),
    (
        "current-candidate",
        include_str!("current_grading/current-candidate.schema.json"),
    ),
    (
        "current-credit",
        include_str!("current_grading/current-credit.schema.json"),
    ),
    (
        "attempt",
        include_str!("current_grading/attempt.schema.json"),
    ),
    (
        "current-impact-card",
        include_str!("current_grading/current-impact-card.schema.json"),
    ),
    (
        "current-audit",
        include_str!("current_grading/current-audit.schema.json"),
    ),
];

/// How a command fails, by the Python exception it raised.
#[derive(Clone, Debug, PartialEq)]
pub enum Failure {
    /// `Inconsistent`: the message on stdout, exit 1. Bytes, because a path in it is printed as its bytes.
    Inconsistent(Vec<u8>),
    /// `InputError` or an `OSError`: `current_grading.py: <message>` on stderr, exit 2.
    Input(String),
    /// Any other exception, which the Python command line did not catch: a traceback with exit 1 there. Rust
    /// prints `current_grading.py: <class>: <message>` and exits 2, as `current-grading-decisions.v1.json`
    /// records.
    Crash(String),
}

pub type R<T> = Result<T, Failure>;

impl From<Raised> for Failure {
    fn from(raised: Raised) -> Self {
        match raised {
            Raised::Os(message) => Failure::Input(message),
            other => Failure::Crash(other.describe()),
        }
    }
}

impl From<pyobject::Raised> for Failure {
    /// The slice-1 rules raise without a message; the class is what the decided line names.
    fn from(raised: pyobject::Raised) -> Self {
        Failure::Crash(
            match raised {
                pyobject::Raised::KeyError => "KeyError",
                pyobject::Raised::TypeError => "TypeError",
                pyobject::Raised::IndexError => "IndexError",
                pyobject::Raised::AttributeError => "AttributeError",
                pyobject::Raised::ValueError => "ValueError",
                pyobject::Raised::Refused => {
                    "unsupported input: NaN where Python orders a set by hash"
                }
            }
            .to_string(),
        )
    }
}

fn inconsistent<T>(reason: impl Into<Vec<u8>>) -> R<T> {
    Err(Failure::Inconsistent(reason.into()))
}

fn require(condition: bool, reason: impl Into<Vec<u8>>) -> R<()> {
    if condition {
        Ok(())
    } else {
        inconsistent(reason)
    }
}

fn type_error<T>(message: String) -> R<T> {
    Err(Failure::Crash(format!("TypeError: {message}")))
}

fn s(text: &str) -> Value {
    Value::Str(text.to_string())
}

fn obj(members: Vec<(&str, Value)>) -> Value {
    Value::Object(
        members
            .into_iter()
            .map(|(key, value)| (key.to_string(), value))
            .collect(),
    )
}

fn item<'a>(value: &'a Value, key: &str) -> R<&'a Value> {
    Ok(pyvalue::item(value, key)?)
}

fn get<'a>(value: &'a Value, key: &str) -> R<&'a Value> {
    Ok(pyvalue::get(value, key)?)
}

fn get_or<'a>(value: &'a Value, key: &str, default: &'a Value) -> R<&'a Value> {
    Ok(pyvalue::get_or(value, key, default)?)
}

/// What `for x in value` visits, borrowed where the value is a list.
fn each(value: &Value) -> R<Cow<'_, [Value]>> {
    match value {
        Value::Array(values) => Ok(Cow::Borrowed(values)),
        other => Ok(Cow::Owned(pyvalue::iterate(other)?)),
    }
}

fn eq(left: &Value, right: &Value) -> bool {
    pyobject::eq(left, right)
}

fn is(value: &Value, text: &str) -> bool {
    value.as_str() == Some(text)
}

fn none(value: &Value) -> bool {
    matches!(value, Value::Null)
}

fn length(value: &Value) -> R<usize> {
    Ok(pyvalue::length(value)?)
}

/// `str(value)` for each member of a tuple, as an f-string formats the tuple.
fn tuple(members: &[&Value]) -> String {
    let parts: Vec<String> = members.iter().map(|member| member.py_repr()).collect();
    format!("({})", parts.join(", "))
}

fn unhashable(value: &Value, role: &str) -> Option<String> {
    (!value.hashable()).then(|| {
        format!(
            "cannot use '{0}' as a {role} (unhashable type: '{0}')",
            value.type_name()
        )
    })
}

/// A hashable value, as a set member or dict key must be.
fn hashable(value: &Value, role: &str) -> R<()> {
    match unhashable(value, role) {
        Some(message) => type_error(message),
        None => Ok(()),
    }
}

/// A tuple of values used as a set member or dict key.
fn hashable_tuple(members: &[&Value], role: &str) -> R<()> {
    match members.iter().find(|member| !member.hashable()) {
        Some(member) => type_error(format!(
            "cannot use 'tuple' as a {role} (unhashable type: '{}')",
            member.type_name()
        )),
        None => Ok(()),
    }
}

/// A `dict` or `set` keyed by JSON values: `1`, `True` and `1.0` are one key, and the one JSON `NaN` is itself.
#[derive(Clone, Default)]
struct Dict {
    entries: Vec<(Value, Value)>,
    strings: HashMap<String, usize>,
}

impl Dict {
    fn position(&self, key: &Value) -> Option<usize> {
        match key {
            Value::Str(text) => self.strings.get(text).copied(),
            _ => self.entries.iter().position(|(known, _)| {
                !matches!(known, Value::Str(_)) && pyobject::same(known, key)
            }),
        }
    }

    fn contains(&self, key: &Value) -> R<bool> {
        hashable(key, "dict key")?;
        Ok(self.position(key).is_some())
    }

    fn get(&self, key: &Value) -> R<Option<&Value>> {
        hashable(key, "dict key")?;
        Ok(self.position(key).map(|at| &self.entries[at].1))
    }

    fn insert(&mut self, key: Value, value: Value) -> R<()> {
        hashable(&key, "dict key")?;
        match self.position(&key) {
            Some(at) => self.entries[at].1 = value,
            None => {
                if let Value::Str(text) = &key {
                    self.strings.insert(text.clone(), self.entries.len());
                }
                self.entries.push((key, value));
            }
        }
        Ok(())
    }

    fn keys(&self) -> impl Iterator<Item = &Value> {
        self.entries.iter().map(|(key, _)| key)
    }

    fn values(&self) -> impl Iterator<Item = &Value> {
        self.entries.iter().map(|(_, value)| value)
    }

    fn len(&self) -> usize {
        self.entries.len()
    }
}

/// `set(values)`.
fn set_of(values: &Value) -> R<Dict> {
    let mut set = Dict::default();
    for value in each(values)?.iter() {
        hashable(value, "set element")?;
        set.insert(value.clone(), Value::Null)?;
    }
    Ok(set)
}

/// `unique(rows, field, where)`: the rows by their field, refusing a repeated key.
fn unique(rows: &Value, field: &str, place: &str) -> R<Dict> {
    let mut result = Dict::default();
    for row in each(rows)?.iter() {
        let key = item(row, field)?;
        require(
            !result.contains(key)?,
            format!("{place}: duplicate {field} {}", key.py_str()),
        )?;
        result.insert(key.clone(), row.clone())?;
    }
    Ok(result)
}

/// `json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False)`.
pub fn canonical(value: &Value) -> String {
    pyjson::dumps(
        value,
        &Dumps {
            ensure_ascii: false,
            separators: Some((",".into(), ":".into())),
            sort_keys: true,
            ..Dumps::default()
        },
    )
}

/// The SHA-256 of [`canonical`].
pub fn digest(value: &Value) -> String {
    sha256::hex_digest(canonical(value).as_bytes())
}

/// `json.dumps(value, indent=2)`, the record serialization without its trailing newline.
pub fn dumps(value: &Value) -> String {
    pyjson::dumps(
        value,
        &Dumps {
            indent: Some(2),
            ..Dumps::default()
        },
    )
}

fn cannot_read(path: &[u8], error: &str) -> Failure {
    Failure::Input(format!(
        "cannot read {}: {error}",
        pytext::stderr_text(path)
    ))
}

/// `read_json(path)`: an unreadable file or invalid JSON is an input error.
pub fn read_json(path: &PyPath) -> R<Value> {
    pyvalue::load(path).map_err(|raised| match raised {
        Raised::Os(message) | Raised::Value(message) => cannot_read(path.as_bytes(), &message),
        other => Failure::from(other),
    })
}

/// `file_hash(path)`.
pub fn file_hash(path: &PyPath) -> R<String> {
    match pyvalue::read_bytes(path) {
        Ok(bytes) => Ok(sha256::hex_digest(&bytes)),
        Err(Raised::Os(message)) => Err(cannot_read(path.as_bytes(), &message)),
        Err(other) => Err(other.into()),
    }
}

/// The repository a command reads, resolved as `Path(root).resolve()`.
pub struct Root {
    pub path: PyPath,
    repo: ruling::Repo,
    validator: std::cell::RefCell<Validator>,
    schemas: std::cell::RefCell<HashMap<&'static str, Value>>,
}

impl Root {
    pub fn new(root: &[u8]) -> R<Self> {
        let path = packet_selection::resolve(&PyPath::new(root))?;
        Ok(Self {
            repo: ruling::Repo::new(path.as_bytes()),
            path,
            validator: std::cell::RefCell::default(),
            schemas: std::cell::RefCell::default(),
        })
    }

    /// `local_path(name, root)`: a path strictly inside the repository.
    pub fn local(&self, name: &[u8]) -> R<PyPath> {
        let path = packet_selection::resolve(&self.path.join(name))?;
        let inside = pyfs::relative_parts(path.as_bytes(), self.path.as_bytes())
            .is_some_and(|parts| !parts.is_empty());
        if name.starts_with(b"/") || !inside {
            return inconsistent([b"source escapes repository: ", name].concat());
        }
        Ok(path)
    }

    /// `local_path(value, root)` for a value from a record, which must be text.
    fn local_value(&self, name: &Value) -> R<PyPath> {
        match name {
            Value::Str(text) => self.local(text.as_bytes()),
            other => type_error(format!(
                "unsupported operand type(s) for /: 'PosixPath' and '{}'",
                other.type_name()
            )),
        }
    }

    /// `pin_file(path, root)`.
    pub fn pin_file(&self, path: &PyPath) -> R<Value> {
        let path = packet_selection::resolve(path)?;
        let relative = pyfs::relative_to(path.as_bytes(), self.path.as_bytes())
            .map_err(|message| Failure::Crash(format!("ValueError: {message}")))?;
        let relative = text_of(&relative)?;
        Ok(obj(vec![
            ("path", Value::Str(relative)),
            ("sha256", Value::Str(file_hash(&path)?)),
        ]))
    }

    /// `resolve_pin(pin, root)`: the pinned file, refused when its bytes changed.
    pub fn resolve_pin(&self, pin: &Value) -> R<PyPath> {
        let path = self.local_value(item(pin, "path")?)?;
        if !eq(&Value::Str(file_hash(&path)?), item(pin, "sha256")?) {
            return inconsistent(format!(
                "source hash changed: {}",
                item(pin, "path")?.py_str()
            ));
        }
        Ok(path)
    }

    /// `check_manifest.validate` against one of the embedded schemas.
    fn problems(&self, name: &'static str, document: &Value) -> R<Vec<String>> {
        let mut schemas = self.schemas.borrow_mut();
        if !schemas.contains_key(name) {
            let text = SCHEMAS
                .iter()
                .find(|(known, _)| *known == name)
                .map(|(_, text)| *text)
                .expect("every schema the module reads is embedded");
            schemas.insert(name, pyvalue::loads(text)?);
        }
        self.validator
            .borrow_mut()
            .validate(&schemas[name], document)
            .map_err(|error| Failure::Crash(format!("SchemaError: {}", error.0)))
    }

    /// `validate_schema(kind, document)`.
    pub(crate) fn validate_schema(&self, kind: &'static str, document: &Value) -> R<()> {
        let name = SCHEMAS
            .iter()
            .map(|(known, _)| *known)
            .find(|known| known.strip_prefix("current-") == Some(kind))
            .expect("every current kind has a schema");
        let problems = self.problems(name, document)?;
        require(
            problems.is_empty(),
            format!("current {kind}: {}", problems.join("; ")),
        )
    }
}

/// A path name that is not UTF-8 cannot be a JSON string here, where Python would hold it with surrogates.
fn not_utf8(bytes: &[u8]) -> Failure {
    Failure::Crash(format!(
        "unsupported path: {} is not UTF-8",
        pytext::stderr_text(bytes)
    ))
}

/// `path.name`.
fn name_of(path: &[u8]) -> &[u8] {
    match path.iter().rposition(|byte| *byte == b'/') {
        Some(at) => &path[at + 1..],
        None if path == b"." => b"",
        None => path,
    }
}

pub(crate) fn parent_of(path: &PyPath) -> PyPath {
    let bytes = path.as_bytes();
    match bytes.iter().rposition(|byte| *byte == b'/') {
        Some(0) => PyPath::new(b"/"),
        Some(at) => PyPath::new(&bytes[..at]),
        None => PyPath::new(b"."),
    }
}

/// Path bytes as text for a JSON value.
fn text_of(bytes: &[u8]) -> R<String> {
    String::from_utf8(bytes.to_vec()).map_err(|_| not_utf8(bytes))
}

/// `left + right` for two values Python adds as text.
fn concat(left: &Value, right: &str) -> R<String> {
    match left {
        Value::Str(text) => Ok(format!("{text}{right}")),
        other => type_error(format!(
            "unsupported operand type(s) for +: '{}' and 'str'",
            other.type_name()
        )),
    }
}

fn concat_text(left: &str, right: &Value) -> R<String> {
    match right {
        Value::Str(text) => Ok(format!("{left}{text}")),
        other => type_error(format!(
            "can only concatenate str (not \"{}\") to str",
            other.type_name()
        )),
    }
}

/// The attempts of a run that one diagnosis in `deviations/` keeps as their cell's result, each with its cause.
///
/// A diagnosis is a JSON object with `predecessor`, `cause` and `reason`, read as `bench-tools queue` reads it: a
/// file that is not such an object is passed over. An attempt with two diagnoses, with a cause the queue replaces
/// or with no reason is not kept, and neither is one whose cause or reason is not text.
pub fn kept_stops(run_dir: &PyPath) -> R<Vec<(String, String)>> {
    let directory = run_dir.join(b"deviations");
    let mut names: Vec<Vec<u8>> = pyfs::names(directory.as_bytes())
        .into_iter()
        .filter(|name| name.ends_with(b".json"))
        .collect();
    names.sort_by_cached_key(|name| pytext::surrogateescape(name));
    let mut found: Vec<(String, Vec<(Value, Value)>)> = Vec::new();
    for name in names {
        let row = match pyvalue::load(&directory.join(&name)) {
            Ok(row) => row,
            Err(Raised::Os(_) | Raised::Value(_)) => continue,
            Err(other) => return Err(other.into()),
        };
        let Some(object) = row.as_object() else {
            continue;
        };
        let (Some(Value::Str(predecessor)), Some(cause)) =
            (object.get("predecessor"), object.get("cause"))
        else {
            continue;
        };
        let reason = object.get("reason").cloned().unwrap_or(Value::Null);
        match found.iter_mut().find(|(known, _)| known == predecessor) {
            Some((_, rows)) => rows.push((cause.clone(), reason)),
            None => found.push((predecessor.clone(), vec![(cause.clone(), reason)])),
        }
    }
    Ok(found
        .into_iter()
        .filter_map(|(attempt, rows)| match &rows[..] {
            [(Value::Str(cause), Value::Str(reason))]
                if KEPT_STOP_CAUSES.contains(&cause.as_str())
                    && !pytext::strip(reason).is_empty() =>
            {
                Some((attempt, cause.clone()))
            }
            _ => None,
        })
        .collect())
}

/// Resolve a linear replacement chain independently of directory and id order. `kept` gives the cause of each
/// stopped attempt kept as its cell's result: as a terminal it resolves the trial without admission.
pub fn trial(records: &[Value], kept: &[(String, String)]) -> R<Object> {
    let mut result = Object::default();
    if records.is_empty() {
        result.insert("state".into(), s("pending"));
        result.insert("reason".into(), s("No attempt has been dispatched."));
        result.insert("attempts".into(), Value::Array(Vec::new()));
        result.insert("terminal".into(), Value::Null);
        return Ok(result);
    }
    let by_id = unique(
        &Value::Array(records.to_vec()),
        "attempt_id",
        "replacement chain",
    )?;
    let mut successors = Dict::default();
    let mut roots: Vec<Value> = Vec::new();
    for record in records {
        let parent = item(record, "predecessor")?;
        if none(parent) {
            require(
                none(item(record, "retry_reason")?),
                "root attempt cannot have a retry reason",
            )?;
            roots.push(item(record, "attempt_id")?.clone());
        } else {
            require(
                by_id.contains(parent)?,
                format!(
                    "replacement predecessor is not in selected cell: {}",
                    parent.py_str()
                ),
            )?;
            require(
                !successors.contains(parent)?,
                format!("replacement chain forks at {}", parent.py_str()),
            )?;
            let parent_record = by_id.get(parent)?.expect("the predecessor was just found");
            require(
                eq(item(record, "cell")?, item(parent_record, "cell")?),
                "replacement changes selected cell",
            )?;
            let retry = item(record, "retry_reason")?;
            let given = retry.truthy()
                && match retry {
                    Value::Str(text) => !pytext::strip(text).is_empty(),
                    other => {
                        return Err(Failure::Crash(format!(
                            "AttributeError: '{}' object has no attribute 'strip'",
                            other.type_name()
                        )));
                    }
                };
            require(given, "replacement needs a retry reason")?;
            successors.insert(parent.clone(), item(record, "attempt_id")?.clone())?;
        }
    }
    require(roots.len() == 1, "replacement chain needs exactly one root")?;
    let mut order: Vec<Value> = Vec::new();
    let mut identifier = Some(roots[0].clone());
    while let Some(current) = identifier {
        require(
            !order.iter().any(|seen| pyobject::same(seen, &current)),
            "replacement chain contains a cycle",
        )?;
        identifier = successors.get(&current)?.cloned();
        order.push(current);
    }
    require(
        order.len() == records.len(),
        "replacement chain contains disconnected attempts or a cycle",
    )?;
    let terminal = by_id
        .get(order.last().expect("one root"))?
        .expect("every attempt in the chain is a record");
    let disposition = item(terminal, "disposition")?;
    let stopped = match disposition {
        Value::Str(text) => text.starts_with("stopped:"),
        other => {
            return Err(Failure::Crash(format!(
                "AttributeError: '{}' object has no attribute 'startswith'",
                other.type_name()
            )));
        }
    };
    let terminal_id = item(terminal, "attempt_id")?;
    let cause = if stopped {
        kept.iter()
            .find(|(attempt, _)| is(terminal_id, attempt))
            .map(|(_, cause)| cause.clone())
    } else {
        None
    };
    let reason = match &cause {
        Some(cause) => s(&format!(
            "Stopped attempt kept as the cell's result: {cause}."
        )),
        None if stopped => s("Stopped attempt awaits replacement."),
        None => disposition.clone(),
    };
    result.insert(
        "state".into(),
        s(if stopped && cause.is_none() {
            "pending"
        } else {
            "resolved"
        }),
    );
    result.insert("reason".into(), reason);
    result.insert("attempts".into(), Value::Array(order));
    result.insert("terminal".into(), terminal_id.clone());
    Ok(result)
}

/// `path.parent / name`, relative to the root, as text for `local_path`.
fn beside(root: &Root, path: &PyPath, name: &Value) -> R<PyPath> {
    let joined = match name {
        Value::Str(text) => parent_of(path).join(text.as_bytes()),
        other => {
            return type_error(format!(
                "unsupported operand type(s) for /: 'PosixPath' and '{}'",
                other.type_name()
            ));
        }
    };
    let relative = pyfs::relative_to(joined.as_bytes(), root.path.as_bytes())
        .map_err(|message| Failure::Crash(format!("ValueError: {message}")))?;
    Ok(PyPath::new(&relative))
}

/// `f"{path}: {reason}"`, with the path's bytes as Python prints them.
fn about(path: &PyPath, reason: &str) -> Vec<u8> {
    [path.as_bytes(), b": ", reason.as_bytes()].concat()
}

/// `attempt_facts(path, record, root)`: an attempt's pins, admission and completion.
fn attempt_facts(root: &Root, path: &PyPath, record: &Value) -> R<Object> {
    let normalized = item(record, "normalized")?;
    let mut review = Value::Null;
    if !none(item(normalized, "path")?) {
        let relative = beside(root, path, item(normalized, "path")?)?;
        let review_path = root.local(relative.as_bytes())?;
        review = root.pin_file(&review_path)?;
        let document = read_json(&review_path)?;
        require(
            eq(
                get(&document, "parse_status")?,
                item(normalized, "parse_status")?,
            ),
            about(path, "normalized parse status differs"),
        )?;
    }
    let native = item(record, "native_payload")?;
    if !none(native) {
        let relative = beside(root, path, item(native, "path")?)?;
        let pin = obj(vec![
            ("path", Value::Str(text_of(relative.as_bytes())?)),
            ("sha256", item(native, "sha256")?.clone()),
        ]);
        root.resolve_pin(&pin)?;
    }
    let usage = beside(root, path, item(item(record, "usage")?, "requests")?)?;
    let usage_path = root.local(usage.as_bytes())?;
    let disposition = item(record, "disposition")?;
    let admitted = is(disposition, "valid completed");
    if admitted {
        let ready = !none(&review)
            && pyvalue::one_of(item(normalized, "parse_status")?, &["parsed", "empty"])
            && !none(native)
            && is(item(record, "phase_reached")?, "result");
        require(
            ready,
            about(path, "admitted execution lacks validated output"),
        )?;
    }
    let prefix = concat(item(record, "run_id")?, "/")?;
    let identifier = concat_text(&prefix, item(record, "attempt_id")?)?;
    let mut facts = Object::default();
    facts.insert("id".into(), Value::Str(identifier));
    facts.insert("record".into(), root.pin_file(path)?);
    facts.insert("review".into(), review);
    facts.insert(
        "usage".into(),
        if pyvalue::is_file(&usage_path) {
            root.pin_file(&usage_path)?
        } else {
            Value::Null
        },
    );
    facts.insert(
        "admission".into(),
        obj(vec![
            ("state", s(if admitted { "admitted" } else { "excluded" })),
            ("reason", disposition.clone()),
        ]),
    );
    let complete =
        admitted && !matches!(item(record, "arm_reported_complete")?, Value::Bool(false));
    facts.insert(
        "completion".into(),
        obj(vec![
            ("state", s(if complete { "complete" } else { "incomplete" })),
            (
                "reason",
                if admitted {
                    s("Filed output and reported coverage.")
                } else {
                    disposition.clone()
                },
            ),
        ]),
    );
    Ok(facts)
}

/// The pin of the packet a frozen run gave its reviewers, after `packet_selection::select` verified it.
fn selected_packet(
    root: &Root,
    run_dir: &PyPath,
    manifest: &Value,
    directory: &PyPath,
    place: &str,
) -> R<Value> {
    let caught = |raised: Raised| match raised {
        Raised::Os(message) => Failure::Input(format!("{place}: packet selection: {message}")),
        Raised::Value(message) | Raised::Key(message) | Raised::Type(message) => {
            Failure::Inconsistent(format!("{place}: packet selection: {message}").into_bytes())
        }
        other => other.into(),
    };
    let selected =
        packet_selection::select(run_dir, manifest, directory, &root.path).map_err(caught)?;
    match root.pin_file(&selected) {
        Err(Failure::Crash(message)) if message.starts_with("ValueError: ") => {
            Err(Failure::Inconsistent(
                format!(
                    "{place}: packet selection: {}",
                    &message["ValueError: ".len()..]
                )
                .into_bytes(),
            ))
        }
        other => other,
    }
}

/// `value[:12]`, as an f-string formats it.
fn first_twelve(value: &Value) -> R<String> {
    match value {
        Value::Str(text) => Ok(text.chars().take(12).collect()),
        Value::Array(values) => {
            Ok(Value::Array(values.iter().take(12).cloned().collect()).py_repr())
        }
        Value::Object(_) => Err(Failure::Crash("KeyError: slice(None, 12, None)".into())),
        other => type_error(format!(
            "'{}' object is not subscriptable",
            other.type_name()
        )),
    }
}

/// The run's `attempts/*/attempt.json`, sorted as `sorted(Path.glob(...))` sorts them.
fn attempt_paths(run_dir: &PyPath) -> Vec<PyPath> {
    let attempts = run_dir.join(b"attempts");
    let mut found: Vec<PyPath> = pyfs::names(attempts.as_bytes())
        .into_iter()
        .map(|name| attempts.join(&name))
        .filter(|directory| pyfs::is_dir(directory.as_bytes()))
        .map(|directory| directory.join(b"attempt.json"))
        .filter(|path| pyfs::lexists(path.as_bytes()))
        .collect();
    found.sort_by_cached_key(|path| pytext::path_order(path.as_bytes()));
    found
}

struct Run {
    manifest: Value,
    records: Vec<(PyPath, Value)>,
    kept: Vec<(String, String)>,
}

/// The selected sources, cells and attempts the current record grades, from the cohort registry and the frozen
/// runs it names.
pub fn inventory(root: &Root) -> R<Value> {
    let registry = read_json(&root.path.join(b"bench/scoreboard.current.json"))?;
    root.validate_schema("cohort-input", &registry)?;
    let tasks = unique(item(&registry, "tasks")?, "id", "registry tasks")?;
    let configurations = unique(
        item(&registry, "configurations")?,
        "id",
        "registry configurations",
    )?;
    unique(item(&registry, "suites")?, "id", "registry suites")?;
    for suite in each(item(&registry, "suites")?)?.iter() {
        let suite_id = item(suite, "id")?.py_str();
        let tasks_named = set_of(item(suite, "tasks")?)?;
        require(
            tasks_named
                .keys()
                .all(|task| tasks.position(task).is_some()),
            format!("{suite_id}: unknown suite task"),
        )?;
        let named = set_of(item(suite, "configurations")?)?;
        require(
            named
                .keys()
                .all(|key| configurations.position(key).is_some()),
            format!("{suite_id}: unknown suite configuration"),
        )?;
    }
    let mut directories: HashMap<String, PyPath> = HashMap::new();
    let mut originals: HashMap<String, Value> = HashMap::new();
    for task in tasks.values() {
        let identifier = item(task, "id")?;
        let task_id = identifier.py_str();
        let directory = root.local(concat_text("bench/targets/", identifier)?.as_bytes())?;
        let target = read_json(&directory.join(b"target.json"))?;
        require(
            eq(item(&target, "id")?, identifier),
            format!("{task_id}: target identity differs"),
        )?;
        let revision = item(task, "revision")?;
        let mut same = true;
        for (key, value) in revision
            .as_object()
            .map(|object| object.iter().collect::<Vec<_>>())
            .unwrap_or_default()
        {
            if key != "packet_sha256" && !eq(item(&target, key)?, value) {
                same = false;
                break;
            }
        }
        require(same, format!("{task_id}: target revision differs"))?;
        let original = root.pin_file(&directory.join(b"packet.md"))?;
        let matches = eq(item(&original, "sha256")?, item(&target, "packet_sha256")?);
        originals.insert(task_id.clone(), original);
        directories.insert(task_id.clone(), directory);
        require(matches, format!("{task_id}: packet bytes changed"))?;
    }
    let mut runs: Vec<(String, Run)> = Vec::new();
    let mut source_inputs: Vec<((String, String), Value)> = Vec::new();
    let mut cells = Dict::default();
    let mut attempts = Dict::default();
    let mut placements: Vec<((String, String), (String, String))> = Vec::new();
    let mut batches: Vec<(String, String)> = Vec::new();
    let mut packets: HashMap<String, (Value, (String, String))> = HashMap::new();
    for source in each(item(&registry, "sources")?)?.iter() {
        let run_value = item(source, "run")?;
        let arm_value = item(source, "arm")?;
        let configuration = item(source, "configuration")?;
        let run = run_value.py_str();
        let arm = arm_value.py_str();
        let key_text = tuple(&[run_value, arm_value]);
        let key = (run.clone(), arm.clone());
        require(
            configurations.contains(configuration)?,
            format!("{key_text}: unknown configuration"),
        )?;
        let source_tasks = item(source, "tasks")?;
        let named = set_of(source_tasks)?;
        require(
            named.keys().all(|task| tasks.position(task).is_some()) && source_tasks.truthy(),
            format!("{key_text}: unknown or empty source tasks"),
        )?;
        require(
            length(source_tasks)? == named.len(),
            format!("{key_text}: duplicate source tasks"),
        )?;
        let run_dir = root.local(concat_text("bench/", run_value)?.as_bytes())?;
        let run_name = name_of(run_dir.as_bytes()).to_vec();
        if !runs.iter().any(|(known, _)| *known == run) {
            let manifest = read_json(&run_dir.join(b"manifest.json"))?;
            let named_run = std::str::from_utf8(&run_name).ok();
            let run_id = item(&manifest, "run_id")?;
            require(
                named_run.is_some_and(|name| eq(run_id, &s(name))),
                format!("{run}: manifest run identity differs"),
            )?;
            let mut records = Vec::new();
            for path in attempt_paths(&run_dir) {
                let record = read_json(&path)?;
                let problems = root.problems("attempt", &record)?;
                require(problems.is_empty(), about(&path, &problems.join("; ")))?;
                let directory = name_of(parent_of(&path).as_bytes()).to_vec();
                let attempt_named = std::str::from_utf8(&directory).ok().is_some_and(|name| {
                    item(&record, "attempt_id").is_ok_and(|id| eq(id, &s(name)))
                });
                item(&record, "attempt_id")?;
                let identity = attempt_named && {
                    let run_id = item(&record, "run_id")?;
                    named_run.is_some_and(|name| eq(run_id, &s(name)))
                };
                require(identity, about(&path, "attempt identity differs"))?;
                records.push((path, record));
            }
            let kept = kept_stops(&run_dir)?;
            runs.push((
                run.clone(),
                Run {
                    manifest,
                    records,
                    kept,
                },
            ));
        }
        let run_entry = &runs
            .iter()
            .find(|(known, _)| *known == run)
            .expect("the run was just read")
            .1;
        let manifest = &run_entry.manifest;
        let arms = unique(item(manifest, "arms")?, "id", &run)?;
        require(
            arms.contains(arm_value)?,
            format!("{run}: selected arm is absent"),
        )?;
        let arm_path = root.local(format!("bench/arms/{arm}.json").as_bytes())?;
        let arm_pin = root.pin_file(&arm_path)?;
        let frozen_arm = arms.get(arm_value)?.expect("the arm was just found");
        require(
            eq(
                item(&arm_pin, "sha256")?,
                item(frozen_arm, "arm_file_sha256")?,
            ),
            format!("{run}/{arm}: frozen arm bytes changed"),
        )?;
        let mut normalized_source = source
            .as_object()
            .cloned()
            .expect("a schema-valid source is an object");
        normalized_source.insert(
            "manifest".into(),
            root.pin_file(&run_dir.join(b"manifest.json"))?,
        );
        normalized_source.insert("arm_file".into(), arm_pin);
        let normalized_source = Value::Object(normalized_source);
        if pyvalue::has(source, "billing_correction")? {
            let correction = item(source, "billing_correction")?;
            let mut pin = correction.as_object().cloned().unwrap_or_default();
            pin.insert(
                "path".into(),
                Value::Str(concat_text("bench/", item(correction, "path")?)?),
            );
            let receipt = root.resolve_pin(&Value::Object(pin))?;
            let billing = read_json(&receipt)?;
            let wanted = obj(vec![("run", run_value.clone()), ("arm", arm_value.clone())]);
            require(
                pyvalue::contains(item(&billing, "sources")?, &wanted)?,
                format!("{key_text}: billing receipt does not apply"),
            )?;
        }
        if let Some((_, known)) = source_inputs.iter().find(|(known, _)| *known == key) {
            require(
                eq(known, &normalized_source),
                format!("{key_text}: conflicting duplicate source"),
            )?;
            continue;
        }
        source_inputs.push((key.clone(), normalized_source));
        let cohort = unique(item(manifest, "cohort")?, "target", &run)?;
        let mut scheduled: Vec<(Value, Value, Value)> = Vec::new();
        let prefix = format!("{}/", text_of(&run_name)?);
        for planned in each(item(manifest, "planned_cells")?)?.iter() {
            if !eq(item(planned, "arm")?, arm_value)
                || !pyvalue::contains(source_tasks, item(planned, "target")?)?
            {
                continue;
            }
            let target_value = item(planned, "target")?;
            let target = target_value.py_str();
            require(
                cohort.contains(target_value)?,
                format!("{key_text}/{target}: scheduled target not in cohort"),
            )?;
            let revision = item(
                tasks.get(target_value)?.expect("a source task is a task"),
                "revision",
            )?;
            let row = cohort
                .get(target_value)?
                .expect("the target was just found");
            require(
                eq(
                    item(row, "diff_manifest_sha256")?,
                    item(revision, "diff_manifest_sha256")?,
                ),
                format!("{key_text}/{target}: diff differs"),
            )?;
            let packet_matches = eq(
                item(row, "packet_sha256")?,
                item(revision, "packet_sha256")?,
            );
            let message = format!(
                "{key_text}/{target}: packet differs: the run's reviews read {} and the task revision pins {}; \
                 reviews of differently cut packets cannot share a task revision",
                first_twelve(item(row, "packet_sha256")?)?,
                first_twelve(item(revision, "packet_sha256")?)?
            );
            require(packet_matches, message)?;
            if !scheduled
                .iter()
                .any(|(known, _, _)| pyobject::same(known, target_value))
            {
                let place = format!("{key_text}/{target}");
                let packet =
                    selected_packet(root, &run_dir, manifest, &directories[&target], &place)?;
                let (first, earlier) = packets
                    .entry(target.clone())
                    .or_insert_with(|| (packet.clone(), key.clone()))
                    .clone();
                require(
                    eq(&packet, &first),
                    format!(
                        "{key_text}/{target}: packet differs: selects {} while {} selects {}",
                        item(&packet, "path")?.py_str(),
                        tuple(&[&s(&earlier.0), &s(&earlier.1)]),
                        item(&first, "path")?.py_str()
                    ),
                )?;
            }
            let replicate = item(planned, "replicate")?;
            hashable_tuple(&[target_value, arm_value, replicate], "set element")?;
            let cell_text = tuple(&[target_value, arm_value, replicate]);
            require(
                !scheduled.iter().any(|(t, a, r)| {
                    pyobject::same(t, target_value)
                        && pyobject::same(a, arm_value)
                        && pyobject::same(r, replicate)
                }),
                format!("{key_text}: repeated scheduled cell {cell_text}"),
            )?;
            scheduled.push((target_value.clone(), arm_value.clone(), replicate.clone()));
            let configuration_name = configuration.py_str();
            let placement = (configuration_name.clone(), target.clone());
            if let Some((_, owner)) = placements.iter().find(|(known, _)| *known == placement) {
                require(
                    *owner == key,
                    format!(
                        "{}: conflicting source placement",
                        tuple(&[configuration, target_value])
                    ),
                )?;
            }
            match placements.iter_mut().find(|(known, _)| *known == placement) {
                Some((_, owner)) => *owner = key.clone(),
                None => placements.push((placement, key.clone())),
            }
            let mut selected: Vec<(PyPath, Value)> = Vec::new();
            for (path, record) in &run_entry.records {
                let cell = item(record, "cell")?;
                if pyobject::same(item(cell, "target")?, target_value)
                    && pyobject::same(item(cell, "arm")?, arm_value)
                    && pyobject::same(item(cell, "replicate")?, replicate)
                {
                    selected.push((path.clone(), record.clone()));
                }
            }
            let records: Vec<Value> = selected.iter().map(|(_, record)| record.clone()).collect();
            let mut chain = trial(&records, &run_entry.kept)?;
            let mut prefixed = Vec::new();
            for identifier in
                each(chain.get("attempts").expect("a trial lists its attempts"))?.iter()
            {
                prefixed.push(Value::Str(concat_text(&prefix, identifier)?));
            }
            chain.insert("attempts".into(), Value::Array(prefixed));
            if let Some(terminal) = chain
                .get("terminal")
                .filter(|terminal| !none(terminal))
                .cloned()
            {
                chain.insert(
                    "terminal".into(),
                    Value::Str(concat_text(&prefix, &terminal)?),
                );
            }
            let identifier = format!("{prefix}{target}/{arm}/{}", replicate.py_str());
            let mut cell = Object::default();
            cell.insert("id".into(), s(&identifier));
            cell.insert("run".into(), run_value.clone());
            cell.insert("configuration".into(), configuration.clone());
            for (field, value) in planned
                .as_object()
                .map(|object| object.iter().collect::<Vec<_>>())
                .unwrap_or_default()
            {
                cell.insert(field.clone(), value.clone());
            }
            for (field, value) in chain.iter() {
                cell.insert(field.clone(), value.clone());
            }
            cells.insert(s(&identifier), Value::Object(cell))?;
            if !batches.iter().any(|(r, t)| *r == run && *t == target) {
                batches.push((run.clone(), target.clone()));
            }
            for (path, record) in &selected {
                let mut facts = attempt_facts(root, path, record)?;
                let fact_id = facts.get("id").cloned().expect("facts name their attempt");
                facts.insert("cell".into(), s(&identifier));
                attempts.insert(fact_id, Value::Object(facts))?;
            }
        }
        let scheduled_targets: Vec<&Value> =
            scheduled.iter().map(|(target, _, _)| target).collect();
        let covered = named.keys().all(|task| {
            scheduled_targets
                .iter()
                .any(|known| pyobject::same(known, task))
        }) && scheduled_targets
            .iter()
            .all(|known| named.position(known).is_some());
        require(
            covered,
            format!("{key_text}: source task has no scheduled cell"),
        )?;
        for (_, record) in &run_entry.records {
            let cell = item(record, "cell")?;
            if eq(item(cell, "arm")?, arm_value)
                && pyvalue::contains(source_tasks, item(cell, "target")?)?
            {
                let replicate = item(cell, "replicate")?;
                let target = item(cell, "target")?;
                hashable_tuple(&[target, arm_value, replicate], "set element")?;
                require(
                    scheduled.iter().any(|(t, a, r)| {
                        pyobject::same(t, target)
                            && pyobject::same(a, arm_value)
                            && pyobject::same(r, replicate)
                    }),
                    format!("{key_text}: attempt outside scheduled cells"),
                )?;
            }
        }
    }
    let suites = each(item(&registry, "suites")?)?.into_owned();
    let mut chosen: Vec<(String, String)> = Vec::new();
    for suite in &suites {
        for configuration in each(item(suite, "configurations")?)?.iter() {
            for task in each(item(suite, "tasks")?)?.iter() {
                let pair = (configuration.py_str(), task.py_str());
                if placements.iter().any(|(known, _)| *known == pair) && !chosen.contains(&pair) {
                    chosen.push(pair);
                }
            }
        }
    }
    require(
        placements.len() == chosen.len(),
        "source placement is outside suite selections",
    )?;
    let mut task_inputs = Vec::new();
    for task in tasks.values() {
        let task_id = item(task, "id")?.py_str();
        let packet = packets
            .get(&task_id)
            .map(|(packet, _)| packet.clone())
            .unwrap_or_else(|| originals[&task_id].clone());
        require(
            eq(
                item(&packet, "sha256")?,
                item(item(task, "revision")?, "packet_sha256")?,
            ),
            format!("{task_id}: no selected source verifies the packet of the task revision"),
        )?;
        let mut input = task
            .as_object()
            .cloned()
            .expect("a schema-valid task is an object");
        input.insert(
            "target".into(),
            root.pin_file(&directories[&task_id].join(b"target.json"))?,
        );
        input.insert("packet".into(), packet);
        task_inputs.push(Value::Object(input));
    }
    let records: usize = runs.iter().map(|(_, run)| run.records.len()).sum();
    let counts = obj(vec![
        ("tasks", int(tasks.len())),
        ("configurations", int(configurations.len())),
        ("source_pairs", int(source_inputs.len())),
        ("runs", int(runs.len())),
        ("selected_cells", int(cells.len())),
        ("selected_attempts", int(attempts.len())),
        ("batches", int(batches.len())),
        (
            "excluded_attempts",
            Value::Int((records as i64 - attempts.len() as i64).to_string()),
        ),
    ]);
    task_inputs.sort_by_key(|value| text_key(value, "id"));
    source_inputs.sort_by(|left, right| left.0.cmp(&right.0));
    let mut cell_list: Vec<Value> = cells.values().cloned().collect();
    cell_list.sort_by_key(|value| text_key(value, "id"));
    let mut attempt_list: Vec<Value> = attempts.values().cloned().collect();
    attempt_list.sort_by_key(|value| text_key(value, "id"));
    batches.sort();
    Ok(obj(vec![
        ("schema_version", Value::Int("1".into())),
        (
            "registry",
            root.pin_file(&root.path.join(b"bench/scoreboard.current.json"))?,
        ),
        ("counts", counts),
        ("tasks", Value::Array(task_inputs)),
        (
            "sources",
            Value::Array(
                source_inputs
                    .into_iter()
                    .map(|(_, source)| source)
                    .collect(),
            ),
        ),
        ("cells", Value::Array(cell_list)),
        ("attempts", Value::Array(attempt_list)),
        (
            "batches",
            Value::Array(
                batches
                    .into_iter()
                    .map(|(run, target)| obj(vec![("run", s(&run)), ("target", s(&target))]))
                    .collect(),
            ),
        ),
    ]))
}

fn int(number: usize) -> Value {
    Value::Int(number.to_string())
}

fn text_key(value: &Value, field: &str) -> String {
    value
        .as_object()
        .and_then(|object| object.get(field))
        .map(Value::py_str)
        .unwrap_or_default()
}

/// `verify_pins(value, root)`: every `{path, sha256}` object names its bytes, and none is historical grading input.
pub fn verify_pins(root: &Root, value: &Value) -> R<()> {
    match value {
        Value::Object(object) => {
            if object.len() == 2 && object.get("path").is_some() && object.get("sha256").is_some() {
                let path = object.get("path").expect("checked");
                let name = match path {
                    Value::Str(text) => {
                        String::from_utf8_lossy(name_of(PyPath::new(text.as_bytes()).as_bytes()))
                            .into_owned()
                    }
                    other => {
                        return type_error(format!(
                            "argument should be a str or an os.PathLike object where __fspath__ returns a str, not '{}'",
                            other.type_name()
                        ));
                    }
                };
                let text = name;
                require(
                    !((text.starts_with("mapping.v") || text.starts_with("results.v"))
                        && text.ends_with(".json")),
                    format!(
                        "historical grading input is not current evidence: {}",
                        path.py_str()
                    ),
                )?;
                root.resolve_pin(value)?;
            } else {
                for (_, child) in object.iter() {
                    verify_pins(root, child)?;
                }
            }
        }
        Value::Array(values) => {
            for child in values {
                verify_pins(root, child)?;
            }
        }
        _ => {}
    }
    Ok(())
}

/// `int(text)` for an item number. The schemas admit only `item-` and ASCII digits, which `$` lets a newline end.
pub(crate) fn item_number(text: &str) -> R<(bool, String)> {
    let stripped = pytext::strip(text);
    let (negative, digits) = match stripped.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, stripped.strip_prefix('+').unwrap_or(stripped)),
    };
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(Failure::Crash(format!(
            "ValueError: invalid literal for int() with base 10: {}",
            pytext::repr_str(text)
        )));
    }
    if let Some(message) = pyjson::int_digit_limit(digits.len()) {
        return Err(Failure::Crash(format!("ValueError: {message}")));
    }
    let trimmed = digits.trim_start_matches('0');
    let trimmed = if trimmed.is_empty() { "0" } else { trimmed };
    Ok((negative && trimmed != "0", trimmed.to_string()))
}

/// `source_item(link, target, root)`: the cohort entry and the original item a link names.
pub fn source_item(root: &Root, link: &Value, target: &Value) -> R<(Value, Value)> {
    let review = item(link, "review")?;
    let path = root.resolve_pin(review)?;
    let relative = pyfs::relative_to(path.as_bytes(), root.path.as_bytes())
        .map_err(|message| Failure::Crash(format!("ValueError: {message}")))?;
    let relative_text = text_of(&relative)?;
    require(
        eq(item(review, "path")?, &Value::Str(relative_text.clone())),
        "claim review path is not canonical",
    )?;
    let parts: Vec<&str> = relative_text.split('/').collect();
    let exact = if parts.len() == 6 && parts[..2] == ["bench", "runs"] {
        let attempt_id = item(link, "attempt_id")?;
        parts[3] == "attempts" && eq(&s(parts[4]), attempt_id) && parts[5] == "normalized.json"
    } else {
        false
    };
    require(exact, "claim link is not an exact source item")?;
    let record = read_json(&parent_of(&path).join(b"attempt.json"))?;
    let identity = (item(&record, "run_id")?, item(&record, "attempt_id")?);
    require(
        eq(identity.0, &s(parts[2])) && eq(identity.1, &s(parts[4])),
        "claim source path differs from its attempt identity",
    )?;
    require(
        eq(item(item(&record, "cell")?, "target")?, target),
        "claim source belongs to another target",
    )?;
    let manifest = read_json(
        &root
            .path
            .join(b"bench/runs")
            .join(parts[2].as_bytes())
            .join(b"manifest.json"),
    )?;
    let cohort = unique(item(&manifest, "cohort")?, "target", parts[2])?;
    let item_id = item(link, "item_id")?;
    let Value::Str(item_text) = item_id else {
        return Err(Failure::Crash(format!(
            "AttributeError: '{}' object has no attribute 'removeprefix'",
            item_id.type_name()
        )));
    };
    let (negative, digits) = item_number(item_text.strip_prefix("item-").unwrap_or(item_text))?;
    let spelled = format!("item-{}{digits}", if negative { "-" } else { "" });
    require(
        *item_text == spelled,
        "claim source item id is not canonical",
    )?;
    let document = read_json(&path)?;
    let items = item(&document, "items")?;
    let count = length(items)?;
    let below = negative
        || digits.len() < count.to_string().len()
        || (digits.len() == count.to_string().len() && digits < count.to_string());
    require(below, "claim source item is missing")?;
    let entry = cohort
        .get(target)?
        .cloned()
        .ok_or_else(|| Failure::Crash(format!("KeyError: {}", target.py_repr())))?;
    let index = if negative {
        let magnitude: usize = digits.parse().unwrap_or(usize::MAX);
        count.checked_sub(magnitude)
    } else {
        digits.parse::<usize>().ok()
    };
    let found = match (items, index) {
        (Value::Array(values), Some(at)) => values[at].clone(),
        (Value::Array(_), None) => {
            return Err(Failure::Crash("IndexError: list index out of range".into()));
        }
        (Value::Str(text), Some(at)) => {
            s(&text.chars().nth(at).map(String::from).unwrap_or_default())
        }
        (Value::Str(_), None) => {
            return Err(Failure::Crash(
                "IndexError: string index out of range".into(),
            ));
        }
        (Value::Object(_), _) => {
            let key = format!("{}{digits}", if negative { "-" } else { "" });
            return Err(Failure::Crash(format!("KeyError: {key}")));
        }
        _ => unreachable!("len() accepted only a list, a dict or a string"),
    };
    Ok((entry, found))
}

/// `applicable_decision(identifier, decisions, target, revision, subject, dimension)`.
fn applicable_decision<'a>(
    identifier: &Value,
    decisions: &'a Dict,
    target: &Value,
    revision: &Value,
    subject: &Value,
    dimension: &str,
) -> R<&'a Value> {
    require(
        decisions.contains(identifier)?,
        format!("unknown adjudication {}", identifier.py_str()),
    )?;
    let decision = decisions.get(identifier)?.expect("just found");
    let found = [
        item(decision, "target")?,
        item(decision, "revision")?,
        item(decision, "subject")?,
        item(decision, "dimension")?,
    ];
    let wanted = [target, revision, subject, &s(dimension)];
    require(
        found
            .iter()
            .zip(wanted)
            .all(|(left, right)| pyobject::same(left, right)),
        format!("{}: adjudication does not apply", identifier.py_str()),
    )?;
    Ok(decision)
}

fn independent(check: &Value, confirmed: bool) -> R<bool> {
    Ok(!eq(item(check, "checker")?, item(check, "independent_of")?)
        && (!confirmed || is(item(check, "result")?, "confirmed")))
}

/// `any(c["result"] == "confirmed" and c["checker"] != c["independent_of"] for c in checks)`, evaluated in order.
fn confirmed_independently(checks: &Value) -> R<bool> {
    for check in each(checks)?.iter() {
        if is(item(check, "result")?, "confirmed") && independent(check, false)? {
            return Ok(true);
        }
    }
    Ok(false)
}

/// A family or a pre-existing problem names the approved decision behind its impact label, and none while the label
/// is unknown. `inspected` requires an independent inspection whatever the label, as a pre-existing problem's does.
fn require_impact(
    entry: &Value,
    decisions: &Dict,
    target: &Value,
    revision: &Value,
    inspected: bool,
) -> R<()> {
    let identifier = item(entry, "id")?;
    let impact = item(entry, "impact")?;
    if !is(item(impact, "band")?, "unknown") {
        let decision = applicable_decision(
            item(impact, "adjudication")?,
            decisions,
            target,
            revision,
            identifier,
            "impact",
        )?;
        require(
            is(item(decision, "status")?, "approved")
                && eq(item(decision, "outcome")?, item(impact, "band")?),
            format!("{}: unapproved impact label", identifier.py_str()),
        )?;
        let mut checked = !inspected;
        let message = format!(
            "{}: the label of a pre-existing problem needs an independent inspection",
            item(decision, "id")?.py_str()
        );
        if !checked {
            for check in each(item(decision, "independent_checks")?)?.iter() {
                if independent(check, false)? {
                    checked = true;
                    break;
                }
            }
        }
        require(checked, message)?;
    } else {
        require(
            none(item(impact, "adjudication")?),
            format!(
                "{}: unknown impact cannot claim approval",
                identifier.py_str()
            ),
        )?;
    }
    Ok(())
}

fn faults_message(identifier: &Value, faults: &[String]) -> String {
    format!("{}: {}", identifier.py_str(), faults.join("; "))
}

/// The loaded record: the inventory and the current documents by kind, with `policy` and `audit`.
pub struct Documents {
    pub kinds: Vec<(&'static str, Value)>,
}

impl Documents {
    pub fn get(&self, kind: &str) -> &Value {
        &self
            .kinds
            .iter()
            .find(|(known, _)| *known == kind)
            .expect("every kind is loaded")
            .1
    }
}

const OUTCOMES: [(&str, &[&str]); 5] = [
    (
        "eligibility",
        &[
            "eligible",
            "refuted",
            "unsupported",
            "advisory",
            "inconsequential",
            "scope-excluded",
            "unresolved",
        ],
    ),
    ("impact", &["serious", "other-material", "unknown"]),
    (
        "control",
        &[
            "audited-clean",
            "provisional",
            "unaudited",
            "known-problems",
        ],
    ),
    (
        "advice-benefit",
        &["supported-benefit", "unsupported-benefit", "unresolved"],
    ),
    (
        "pre-existing-admission",
        &["admitted", "not-admitted", "unresolved"],
    ),
];

/// `validate_documents(documents, selected, root, grades)`.
pub fn validate_documents(
    root: &Root,
    documents: &Documents,
    selected: &Value,
    grades: bool,
) -> R<()> {
    for kind in KINDS {
        root.validate_schema(kind, documents.get(kind))?;
        verify_pins(root, documents.get(kind))?;
    }
    let mut tasks = Dict::default();
    for task in each(item(selected, "tasks")?)?.iter() {
        tasks.insert(item(task, "id")?.clone(), task.clone())?;
    }
    let references = unique(
        item(documents.get("reference"), "targets")?,
        "target",
        "current references",
    )?;
    require(
        references.len() == tasks.len()
            && references.keys().all(|key| tasks.position(key).is_some()),
        "current references must cover every selected task",
    )?;
    let decisions = unique(
        item(documents.get("adjudication"), "decisions")?,
        "id",
        "current adjudications",
    )?;
    let mut families = Dict::default();
    let mut pre_existing = Dict::default();
    for decision in decisions.values() {
        let dimension = item(decision, "dimension")?;
        let admission = is(dimension, "pre-existing-admission");
        if admission {
            require(
                is(get_or(decision, "settled_by", &s("owner"))?, "owner"),
                "pre-existing-problem admission stays with the owner",
            )?;
        }
        let faults = if admission {
            Vec::new()
        } else {
            delegation::filing_faults(&root.repo, decision, "candidate")?
        };
        require(
            faults.is_empty(),
            faults_message(item(decision, "id")?, &faults),
        )?;
        let outcome = item(decision, "outcome")?;
        hashable(dimension, "dict key")?;
        let Some((_, allowed)) = OUTCOMES.iter().find(|(name, _)| is(dimension, name)) else {
            return Err(Failure::Crash(format!("KeyError: {}", dimension.py_repr())));
        };
        hashable(outcome, "set element")?;
        require(
            allowed.iter().any(|name| is(outcome, name)),
            format!(
                "{}: decision variant contradicts dimension",
                item(decision, "id")?.py_str()
            ),
        )?;
        let decision_target = item(decision, "target")?;
        let applies = tasks.contains(decision_target)?
            && eq(
                item(decision, "revision")?,
                item(tasks.get(decision_target)?.expect("just found"), "revision")?,
            );
        require(
            applies,
            format!(
                "{}: adjudication revision differs",
                item(decision, "id")?.py_str()
            ),
        )?;
        let approved = is(item(decision, "status")?, "approved");
        if approved {
            let saved = is(item(decision, "authority")?, "human")
                && !none(item(decision, "receipt")?)
                && !none(item(decision, "receipt_scope")?);
            require(
                saved,
                format!(
                    "{}: approved decision requires a saved human ruling",
                    item(decision, "id")?.py_str()
                ),
            )?;
            let receipt = pyvalue::read_text(&root.resolve_pin(item(decision, "receipt")?)?)?;
            require(
                pyvalue::contains(&Value::Str(receipt), item(decision, "receipt_scope")?)?,
                format!(
                    "{}: ruling scope is not in saved receipt",
                    item(decision, "id")?.py_str()
                ),
            )?;
        }
        if is(dimension, "impact") && !is(outcome, "unknown") && approved {
            require(
                !none(item(decision, "boundary")?),
                format!(
                    "{}: impact needs a calibrated boundary",
                    item(decision, "id")?.py_str()
                ),
            )?;
            if is(outcome, "serious") {
                require(
                    confirmed_independently(item(decision, "independent_checks")?)?,
                    format!(
                        "{}: serious impact needs an independent check",
                        item(decision, "id")?.py_str()
                    ),
                )?;
            }
        }
        if is(dimension, "control") && is(outcome, "audited-clean") && approved {
            let audited = item(decision, "evidence")?.truthy()
                && confirmed_independently(item(decision, "independent_checks")?)?;
            require(audited, "audited-clean needs an independent audit")?;
        }
    }
    for (target, reference) in references.entries.iter() {
        let target_text = target.py_str();
        let revision = item(reference, "revision")?;
        require(
            eq(
                revision,
                item(
                    tasks.get(target)?.expect("references cover the tasks"),
                    "revision",
                )?,
            ),
            format!("{target_text}: reference revision differs"),
        )?;
        let grouped = unique(
            item(reference, "families")?,
            "id",
            &format!("{target_text} families"),
        )?;
        for (identifier, family) in grouped.entries.iter() {
            require(
                !families.contains(identifier)?,
                format!("duplicate family id {}", identifier.py_str()),
            )?;
            families.insert(identifier.clone(), target.clone())?;
            require(
                item(family, "evidence")?.truthy(),
                format!("{}: causal family needs evidence", identifier.py_str()),
            )?;
            let eligibility = item(family, "eligibility")?;
            if is(item(eligibility, "state")?, "approved") {
                let decision = applicable_decision(
                    item(eligibility, "adjudication")?,
                    &decisions,
                    target,
                    revision,
                    identifier,
                    "eligibility",
                )?;
                require(
                    is(item(decision, "status")?, "approved")
                        && is(item(decision, "outcome")?, "eligible"),
                    format!("{}: unapproved family", identifier.py_str()),
                )?;
                require(
                    !is(get(decision, "settled_by")?, "agents"),
                    format!(
                        "{}: adding a problem stays with the owner",
                        identifier.py_str()
                    ),
                )?;
            } else if !none(item(eligibility, "adjudication")?) {
                let decision = applicable_decision(
                    item(eligibility, "adjudication")?,
                    &decisions,
                    target,
                    revision,
                    identifier,
                    "eligibility",
                )?;
                require(
                    !is(item(decision, "status")?, "approved"),
                    format!(
                        "{}: pending family contradicts approved decision",
                        identifier.py_str()
                    ),
                )?;
            }
            require_impact(family, &decisions, target, revision, false)?;
        }
        let empty = Value::Array(Vec::new());
        let problems = unique(
            get_or(reference, "pre_existing_problems", &empty)?,
            "id",
            &format!("{target_text} pre-existing problems"),
        )?;
        for (identifier, problem) in problems.entries.iter() {
            require(
                !pre_existing.contains(identifier)? && !families.contains(identifier)?,
                format!("duplicate problem id {}", identifier.py_str()),
            )?;
            pre_existing.insert(identifier.clone(), target.clone())?;
            let problem_revision = item(problem, "revision")?;
            require(
                eq(problem_revision, revision),
                format!(
                    "{}: pre-existing-problem revision differs",
                    identifier.py_str()
                ),
            )?;
            require(
                item(problem, "evidence")?.truthy(),
                format!(
                    "{}: pre-existing problem needs evidence",
                    identifier.py_str()
                ),
            )?;
            let decision = applicable_decision(
                item(problem, "admission")?,
                &decisions,
                target,
                problem_revision,
                identifier,
                "pre-existing-admission",
            )?;
            require(
                is(item(decision, "status")?, "approved")
                    && is(item(decision, "outcome")?, "admitted"),
                format!(
                    "{}: unapproved pre-existing-problem admission",
                    identifier.py_str()
                ),
            )?;
            require_impact(problem, &decisions, target, problem_revision, true)?;
        }
        let control = item(reference, "control")?;
        let status = item(control, "status")?;
        require(
            grouped.len() == 0 || !is(status, "audited-clean"),
            format!("{target_text}: clean control contains known families"),
        )?;
        require(
            grouped.len() > 0 || !is(status, "known-problems"),
            format!("{target_text}: known-problems control has no families"),
        )?;
        if is(status, "audited-clean") {
            let decision = applicable_decision(
                item(control, "adjudication")?,
                &decisions,
                target,
                revision,
                target,
                "control",
            )?;
            require(
                is(item(decision, "status")?, "approved")
                    && is(item(decision, "outcome")?, "audited-clean"),
                format!("{target_text}: control audit not approved"),
            )?;
        } else {
            require(
                none(item(control, "adjudication")?),
                format!("{target_text}: provisional control cannot claim audit approval"),
            )?;
        }
    }
    require(
        !families
            .keys()
            .any(|key| pre_existing.position(key).is_some()),
        "ordinary and pre-existing problem IDs overlap",
    )?;
    for decision in decisions.values() {
        if is(get(decision, "settled_by")?, "agents") && is(item(decision, "outcome")?, "eligible")
        {
            let settlement = read_json(&root.resolve_pin(item(decision, "settlement")?)?)?;
            let named = families
                .get(get(&settlement, "same_fault_as")?)?
                .cloned()
                .unwrap_or(Value::Null);
            require(
                eq(&named, item(decision, "target")?),
                format!(
                    "{}: a duplicate must name an existing problem on this target",
                    item(decision, "id")?.py_str()
                ),
            )?;
        }
    }
    let claims = unique(
        item(documents.get("claim"), "claims")?,
        "id",
        "current claims",
    )?;
    let mut equivalent: Vec<((Value, Value), Value)> = Vec::new();
    let mut comments: Vec<LinkedComment> = Vec::new();
    for claim in claims.values() {
        let target = item(claim, "target")?;
        let claim_id = item(claim, "id")?.py_str();
        let revision = item(claim, "revision")?;
        let applies = tasks.contains(target)?
            && eq(
                revision,
                item(tasks.get(target)?.expect("just found"), "revision")?,
            );
        require(applies, format!("{claim_id}: claim revision differs"))?;
        require(
            item(claim, "links")?.truthy() && item(claim, "evidence")?.truthy(),
            format!("{claim_id}: claim needs source links and evidence"),
        )?;
        let family_id = item(claim, "family_id")?;
        if !none(family_id) {
            require(
                families
                    .get(family_id)?
                    .is_some_and(|known| eq(known, target)),
                format!("{claim_id}: unknown claim family"),
            )?;
        }
        let problem_id = get(claim, "pre_existing_problem_id")?;
        if !none(problem_id) {
            require(
                none(family_id)
                    && pre_existing
                        .get(problem_id)?
                        .is_some_and(|known| eq(known, target)),
                format!("{claim_id}: contradictory or unknown pre-existing-problem link"),
            )?;
            let decision = applicable_decision(
                item(claim, "adjudication")?,
                &decisions,
                target,
                revision,
                item(claim, "id")?,
                "eligibility",
            )?;
            require(
                is(item(decision, "status")?, "approved")
                    && is(item(decision, "outcome")?, "scope-excluded"),
                "pre-existing-problem link needs approved outside-change claim",
            )?;
        }
        if !none(item(claim, "adjudication")?) {
            let decision = applicable_decision(
                item(claim, "adjudication")?,
                &decisions,
                target,
                revision,
                item(claim, "id")?,
                "eligibility",
            )?;
            require(
                is(item(decision, "outcome")?, "eligible") == !none(family_id),
                format!("{claim_id}: claim decision contradicts family"),
            )?;
            if is(get(decision, "settled_by")?, "agents")
                && is(item(decision, "outcome")?, "eligible")
            {
                let settlement = read_json(&root.resolve_pin(item(decision, "settlement")?)?)?;
                require(
                    eq(family_id, item(&settlement, "same_fault_as")?),
                    format!("{claim_id}: duplicate names another problem"),
                )?;
            }
        }
        let mut seen: Vec<(Value, Value)> = Vec::new();
        for link in each(item(claim, "links")?)?.iter() {
            let (cohort, _) = source_item(root, link, target)?;
            let mut same_packet = true;
            for field in ["packet_sha256", "diff_manifest_sha256"] {
                if !eq(item(&cohort, field)?, item(revision, field)?) {
                    same_packet = false;
                    break;
                }
            }
            require(
                same_packet,
                format!("{claim_id}: source packet or diff changed"),
            )?;
            let key = (
                item(item(link, "review")?, "path")?.clone(),
                item(link, "item_id")?.clone(),
            );
            hashable_tuple(&[&key.0, &key.1], "set element")?;
            require(
                !seen.iter().any(|known| pair_same(known, &key)),
                format!("{claim_id}: duplicate source link"),
            )?;
            seen.push(key.clone());
            let is_equivalent = is(item(link, "relation")?, "equivalent");
            match comments
                .iter_mut()
                .find(|comment| pair_same(&comment.key, &key))
            {
                Some(comment) => {
                    comment.claims.push(claim_id.clone());
                    comment.equivalent |= is_equivalent;
                }
                None => comments.push(LinkedComment {
                    key: key.clone(),
                    claims: vec![claim_id.clone()],
                    equivalent: is_equivalent,
                }),
            }
            if is_equivalent {
                equivalent.push((key, item(claim, "id")?.clone()));
            }
        }
    }
    for comment in &comments {
        require(
            comment.claims.len() < 2 || !comment.equivalent,
            format!(
                "{}: a comment linked to two or more claims ({}) is linked to none of them as equivalent",
                tuple(&[&comment.key.0, &comment.key.1]),
                comment.claims.join(", ")
            ),
        )?;
    }
    for candidate in unique(
        item(documents.get("candidate"), "candidates")?,
        "id",
        "current candidates",
    )?
    .values()
    {
        let target = item(candidate, "target")?;
        let candidate_id = item(candidate, "id")?.py_str();
        let applies = tasks.contains(target)?
            && eq(
                item(candidate, "revision")?,
                item(tasks.get(target)?.expect("just found"), "revision")?,
            );
        require(
            applies,
            format!("{candidate_id}: candidate revision differs"),
        )?;
        require(
            item(candidate, "anchors")?.truthy(),
            format!("{candidate_id}: candidate needs its original anchors"),
        )?;
        for anchor in each(item(candidate, "anchors")?)?.iter() {
            validate_anchor(root, anchor, item(anchor, "review")?, target)?;
        }
        if !none(item(candidate, "decision")?) {
            let dimension = if is(get(candidate, "kind")?, "pre-existing-problem") {
                "pre-existing-admission"
            } else {
                "eligibility"
            };
            let decision = applicable_decision(
                item(candidate, "decision")?,
                &decisions,
                target,
                item(candidate, "revision")?,
                item(candidate, "id")?,
                dimension,
            )?;
            require(
                is(item(decision, "status")?, "approved")
                    && !is(item(decision, "outcome")?, "unresolved"),
                format!("{candidate_id}: only an approved saved human ruling resolves a candidate"),
            )?;
        }
    }
    let mut seen: Vec<(Value, Value, Value)> = Vec::new();
    for ruling in unique(
        item(documents.get("credit"), "rulings")?,
        "id",
        "current credit rulings",
    )?
    .values()
    {
        let has_family = pyvalue::has(ruling, "family_id")?;
        let has_problem = pyvalue::has(ruling, "pre_existing_problem_id")?;
        require(
            has_family != has_problem,
            "credit must name exactly one ordinary or pre-existing problem",
        )?;
        if has_problem {
            require(
                is(get_or(ruling, "settled_by", &s("owner"))?, "owner"),
                "a credit ruling on a pre-existing problem stays with the owner",
            )?;
        }
        let faults = if has_problem {
            Vec::new()
        } else {
            delegation::filing_faults(&root.repo, ruling, "recovery")?
        };
        require(
            faults.is_empty(),
            faults_message(item(ruling, "id")?, &faults),
        )?;
        let ruling_id = item(ruling, "id")?.py_str();
        let target = item(ruling, "target")?;
        let applies = tasks.contains(target)?
            && eq(
                item(ruling, "revision")?,
                item(tasks.get(target)?.expect("just found"), "revision")?,
            );
        require(
            applies,
            format!("{ruling_id}: credit ruling revision differs"),
        )?;
        let subject = pre_existing::credit_subject(ruling)?;
        let known = if has_problem {
            &pre_existing
        } else {
            &families
        };
        require(
            known.get(&subject)?.is_some_and(|known| eq(known, target)),
            format!("{ruling_id}: unknown known problem"),
        )?;
        source_item(root, ruling, target)?;
        let key = (
            item(item(ruling, "review")?, "path")?.clone(),
            item(ruling, "item_id")?.clone(),
            subject.clone(),
        );
        hashable_tuple(&[&key.0, &key.1, &key.2], "set element")?;
        require(
            !seen.iter().any(|(a, b, c)| {
                pyobject::same(a, &key.0) && pyobject::same(b, &key.1) && pyobject::same(c, &key.2)
            }),
            format!("{ruling_id}: a comment has one credit ruling per known problem"),
        )?;
        seen.push(key.clone());
        let scope = item(ruling, "receipt_scope")?;
        let receipt = pyvalue::read_text(&root.resolve_pin(item(ruling, "receipt")?)?)?;
        require(
            pyvalue::contains(&Value::Str(receipt), scope)?,
            format!("{ruling_id}: ruling scope is not in saved receipt"),
        )?;
        let pair = (key.0.clone(), key.1.clone());
        let linked = match equivalent.iter().find(|(known, _)| pair_same(known, &pair)) {
            Some((_, claim_id)) => claims.get(claim_id)?.cloned(),
            None => claims.get(&Value::Null)?.cloned(),
        };
        let field = if has_problem {
            "pre_existing_problem_id"
        } else {
            "family_id"
        };
        let allowed = is(item(ruling, "says_what")?, "yes")
            || match &linked {
                None => true,
                Some(claim) => !eq(get(claim, field)?, &subject),
            };
        require(
            allowed,
            format!(
                "{ruling_id}: the comment is ruled no credit and linked as equivalent to a claim of that known problem"
            ),
        )?;
    }
    if grades {
        validate_grades(root, documents, selected, &references, &claims)?;
    }
    Ok(())
}

/// A review comment and the claims that link it, for the owner's 2026-10-11 rule (#256).
struct LinkedComment {
    key: (Value, Value),
    claims: Vec<String>,
    equivalent: bool,
}

fn pair_same(left: &(Value, Value), right: &(Value, Value)) -> bool {
    pyobject::same(&left.0, &right.0) && pyobject::same(&left.1, &right.1)
}

fn batch_value(run: &Value, target: &Value) -> Value {
    obj(vec![("run", run.clone()), ("target", target.clone())])
}

/// `grading_inputs(batch, selected, documents, policy, root)`: everything a batch's grade depends on.
pub fn grading_inputs(
    root: &Root,
    batch: &Value,
    selected: &Value,
    documents: &Documents,
    policy: &Value,
) -> R<Value> {
    let target = item(batch, "target")?;
    let run = item(batch, "run")?;
    let mut task = None;
    for candidate in each(item(selected, "tasks")?)?.iter() {
        if eq(item(candidate, "id")?, target) {
            task = Some(candidate.clone());
            break;
        }
    }
    let task = task.ok_or_else(|| Failure::Crash("StopIteration".into()))?;
    let mut cell_ids: Vec<Value> = Vec::new();
    for cell in each(item(selected, "cells")?)?.iter() {
        if eq(item(cell, "run")?, run) && eq(item(cell, "target")?, target) {
            cell_ids.push(item(cell, "id")?.clone());
        }
    }
    let mut attempts = Vec::new();
    for attempt in each(item(selected, "attempts")?)?.iter() {
        let cell = item(attempt, "cell")?;
        hashable(cell, "set element")?;
        if cell_ids.iter().any(|known| pyobject::same(known, cell)) {
            attempts.push(attempt.clone());
        }
    }
    let mut paths: Vec<Value> = Vec::new();
    for attempt in &attempts {
        let review = item(attempt, "review")?;
        if review.truthy() {
            let path = item(review, "path")?;
            hashable(path, "set element")?;
            paths.push(path.clone());
        }
    }
    let has_path = |value: &Value| paths.iter().any(|path| pyobject::same(path, value));
    let mut reference = None;
    for candidate in each(item(documents.get("reference"), "targets")?)?.iter() {
        if eq(item(candidate, "target")?, target) {
            reference = Some(candidate.clone());
            break;
        }
    }
    let reference = reference.ok_or_else(|| Failure::Crash("StopIteration".into()))?;
    let without_impact = |entry: &Value| -> R<Value> {
        match entry {
            Value::Object(object) => Ok(Value::Object(
                object
                    .iter()
                    .filter(|(key, _)| key.as_str() != "impact")
                    .map(|(key, value)| (key.clone(), value.clone()))
                    .collect(),
            )),
            other => Err(Failure::Crash(format!(
                "AttributeError: '{}' object has no attribute 'items'",
                other.type_name()
            ))),
        }
    };
    let mut family_inputs = Vec::new();
    for family in each(item(&reference, "families")?)?.iter() {
        family_inputs.push(without_impact(family)?);
    }
    let empty = Value::Array(Vec::new());
    let mut pre_existing_inputs = Vec::new();
    for problem in each(get_or(&reference, "pre_existing_problems", &empty)?)?.iter() {
        pre_existing_inputs.push(without_impact(problem)?);
    }
    let mut applicable_claims = Vec::new();
    for claim in each(item(documents.get("claim"), "claims")?)?.iter() {
        if eq(item(claim, "target")?, target) {
            let mut links = Vec::new();
            for link in each(item(claim, "links")?)?.iter() {
                if has_path(item(item(link, "review")?, "path")?) {
                    links.push(link.clone());
                }
            }
            if !links.is_empty() {
                let mut copied = claim.as_object().cloned().unwrap_or_default();
                copied.insert("links".into(), Value::Array(links));
                applicable_claims.push(Value::Object(copied));
            }
        }
    }
    let mut decision_ids: Vec<Value> = Vec::new();
    for claim in &applicable_claims {
        let identifier = item(claim, "adjudication")?;
        hashable(identifier, "set element")?;
        decision_ids.push(identifier.clone());
    }
    for family in &family_inputs {
        let identifier = item(item(family, "eligibility")?, "adjudication")?;
        hashable(identifier, "set element")?;
        decision_ids.push(identifier.clone());
    }
    let mut decisions = Vec::new();
    for decision in each(item(documents.get("adjudication"), "decisions")?)?.iter() {
        let identifier = item(decision, "id")?;
        hashable(identifier, "set element")?;
        if decision_ids
            .iter()
            .any(|known| pyobject::same(known, identifier))
            && is(item(decision, "dimension")?, "eligibility")
        {
            decisions.push(decision.clone());
        }
    }
    let mut credits = Vec::new();
    for ruling in each(item(documents.get("credit"), "rulings")?)?.iter() {
        if eq(item(ruling, "target")?, target) && has_path(item(item(ruling, "review")?, "path")?) {
            credits.push(ruling.clone());
        }
    }
    sort_by_id(&mut credits)?;
    let mut review_inputs = Vec::new();
    for attempt in &attempts {
        let record = read_json(&root.resolve_pin(item(attempt, "record")?)?)?;
        review_inputs.push(obj(vec![
            ("id", item(attempt, "id")?.clone()),
            ("review", item(attempt, "review")?.clone()),
            ("admission", item(attempt, "admission")?.clone()),
            ("completion", item(attempt, "completion")?.clone()),
            (
                "parse_status",
                item(item(&record, "normalized")?, "parse_status")?.clone(),
            ),
        ]));
    }
    sort_by_id(&mut applicable_claims)?;
    sort_by_id(&mut decisions)?;
    let mut inputs = Object::default();
    inputs.insert("contract".into(), s("current-grading-input/v1"));
    inputs.insert("batch".into(), batch.clone());
    inputs.insert("revision".into(), item(&task, "revision")?.clone());
    inputs.insert("packet".into(), item(&task, "packet")?.clone());
    inputs.insert("reviews".into(), Value::Array(review_inputs));
    inputs.insert("families".into(), Value::Array(family_inputs));
    inputs.insert("claims".into(), Value::Array(applicable_claims));
    inputs.insert("adjudications".into(), Value::Array(decisions));
    inputs.insert("credits".into(), Value::Array(credits));
    inputs.insert("validation_policy".into(), policy.clone());
    if !pre_existing_inputs.is_empty() {
        let mut admission_ids = Vec::new();
        for problem in &pre_existing_inputs {
            let identifier = item(problem, "admission")?;
            hashable(identifier, "set element")?;
            admission_ids.push(identifier.clone());
        }
        inputs.insert(
            "pre_existing_problems".into(),
            Value::Array(pre_existing_inputs),
        );
        let mut admissions = Vec::new();
        for decision in each(item(documents.get("adjudication"), "decisions")?)?.iter() {
            let identifier = item(decision, "id")?;
            hashable(identifier, "set element")?;
            if admission_ids
                .iter()
                .any(|known| pyobject::same(known, identifier))
            {
                let mut facts = Object::default();
                for fact in ADMISSION_FACTS {
                    facts.insert(fact.into(), item(decision, fact)?.clone());
                }
                admissions.push(Value::Object(facts));
            }
        }
        sort_by_id(&mut admissions)?;
        inputs.insert("pre_existing_admissions".into(), Value::Array(admissions));
    }
    let inputs = Value::Object(inputs);
    verify_pins(root, &inputs)?;
    Ok(inputs)
}

/// `sorted(values, key=lambda value: value["id"])`.
fn sort_by_id(values: &mut [Value]) -> R<()> {
    let mut keys = Vec::with_capacity(values.len());
    for value in values.iter() {
        keys.push(item(value, "id")?.clone());
    }
    if let Some(first) = keys.first() {
        for key in &keys[1..] {
            pyvalue::compare(key, first)?;
        }
    }
    let mut order: Vec<usize> = (0..values.len()).collect();
    order.sort_by(|left, right| {
        pyvalue::compare(&keys[*left], &keys[*right]).unwrap_or(std::cmp::Ordering::Equal)
    });
    let sorted: Vec<Value> = order.iter().map(|at| values[*at].clone()).collect();
    values.clone_from_slice(&sorted);
    Ok(())
}

/// `grading_fingerprint(batch, selected, documents, policy, root)`.
pub fn grading_fingerprint(
    root: &Root,
    batch: &Value,
    selected: &Value,
    documents: &Documents,
    policy: &Value,
) -> R<String> {
    Ok(digest(&grading_inputs(
        root, batch, selected, documents, policy,
    )?))
}

/// `(fingerprint, state)`: `current` when a saved grade matches the batch's inputs, `stale` when they changed after
/// it was graded, `missing` when no grade is saved.
pub fn batch_state(
    root: &Root,
    batch: &Value,
    selected: &Value,
    documents: &Documents,
) -> R<(String, &'static str)> {
    let fingerprint =
        grading_fingerprint(root, batch, selected, documents, documents.get("policy"))?;
    let run = item(batch, "run")?;
    let target = item(batch, "target")?;
    let mut saved = Vec::new();
    for grade in each(item(documents.get("grade"), "batches")?)?.iter() {
        let identity = (item(grade, "run")?, item(grade, "target")?);
        if pyobject::same(identity.0, run) && pyobject::same(identity.1, target) {
            saved.push(item(grade, "input_fingerprint")?.clone());
        }
    }
    let state = if saved.is_empty() {
        "missing"
    } else if saved.len() == 1 && pyobject::same(&saved[0], &Value::Str(fingerprint.clone())) {
        "current"
    } else {
        "stale"
    };
    Ok((fingerprint, state))
}

/// The verdict contract a validation policy names, current-verdicts/v1 when it names none.
fn policy_contract(root: &Root, documents: &Documents) -> R<Value> {
    let policy = read_json(&root.resolve_pin(documents.get("policy"))?)?;
    Ok(get_or(&policy, "verdicts", &s(grading_validation::CONTRACT))?.clone())
}

fn validate_grades(
    root: &Root,
    documents: &Documents,
    selected: &Value,
    references: &Dict,
    canonical_claims: &Dict,
) -> R<()> {
    let mut batches: Vec<(Value, Value)> = Vec::new();
    for batch in each(item(selected, "batches")?)?.iter() {
        batches.push((item(batch, "run")?.clone(), item(batch, "target")?.clone()));
    }
    let mut attempts = Dict::default();
    for attempt in each(item(selected, "attempts")?)?.iter() {
        attempts.insert(item(attempt, "id")?.clone(), attempt.clone())?;
    }
    let mut cells = Dict::default();
    for cell in each(item(selected, "cells")?)?.iter() {
        cells.insert(item(cell, "id")?.clone(), cell.clone())?;
    }
    let mut equivalent_items: Vec<((Value, Value), Value)> = Vec::new();
    for claim in canonical_claims.values() {
        for link in each(item(claim, "links")?)?.iter() {
            if is(item(link, "relation")?, "equivalent") {
                let key = (
                    item(item(link, "review")?, "path")?.clone(),
                    item(link, "item_id")?.clone(),
                );
                hashable_tuple(&[&key.0, &key.1], "dict key")?;
                let identifier = item(claim, "id")?.clone();
                match equivalent_items
                    .iter_mut()
                    .find(|(known, _)| pair_same(known, &key))
                {
                    Some((_, value)) => *value = identifier,
                    None => equivalent_items.push((key, identifier)),
                }
            }
        }
    }
    let contract = policy_contract(root, documents)?;
    let mut seen: Vec<(Value, Value)> = Vec::new();
    for batch in each(item(documents.get("grade"), "batches")?)?.iter() {
        let key = (item(batch, "run")?.clone(), item(batch, "target")?.clone());
        let key_text = tuple(&[&key.0, &key.1]);
        hashable_tuple(&[&key.0, &key.1], "set element")?;
        require(
            batches.iter().any(|known| pair_same(known, &key))
                && !seen.iter().any(|known| pair_same(known, &key)),
            format!("{key_text}: missing or duplicate selected batch"),
        )?;
        seen.push(key.clone());
        let expected = grading_fingerprint(
            root,
            &batch_value(&key.0, &key.1),
            selected,
            documents,
            documents.get("policy"),
        )?;
        require(
            eq(&Value::Str(expected), item(batch, "input_fingerprint")?),
            format!("{key_text}: grade fingerprint is stale"),
        )?;
        let verdicts = get_or(batch, "verdicts", &s(grading_validation::CONTRACT))?.clone();
        require(
            eq(&verdicts, &contract),
            format!(
                "{key_text}: batch is not marked with the validation policy's verdict contract {}",
                contract.py_str()
            ),
        )?;
        let assessor = item(batch, "assessor")?;
        let receipt = read_json(&root.resolve_pin(item(assessor, "receipt")?)?)?;
        let saved = [
            item(&receipt, "input_fingerprint")?,
            item(&receipt, "verdicts_sha256")?,
            item(item(&receipt, "provenance")?, "kind")?,
        ];
        let filed = [
            item(batch, "input_fingerprint")?,
            item(item(assessor, "verdicts")?, "sha256")?,
            item(assessor, "kind")?,
        ];
        require(
            saved
                .iter()
                .zip(filed)
                .all(|(left, right)| pyobject::same(left, right)),
            format!("{key_text}: assessor receipt belongs to another assessment"),
        )?;
        let conditional_safety = pyvalue::has(&receipt, "remedy_policy")?;
        if conditional_safety {
            let remedy_policy = item(&receipt, "remedy_policy")?;
            require(
                is(item(remedy_policy, "role")?, "first"),
                "a second assessment cannot supply a first grade",
            )?;
            let rules = read_json(&root.resolve_pin(item(remedy_policy, "source")?)?)?;
            require(
                is(get(&rules, "contract")?, "remedy-assessment/v1"),
                "unsupported remedy policy",
            )?;
        }
        let target = &key.1;
        let reference = references
            .get(target)?
            .ok_or_else(|| Failure::Crash(format!("KeyError: {}", target.py_repr())))?;
        let mut family_ids: Vec<Value> = Vec::new();
        for family in each(item(reference, "families")?)?.iter() {
            let identifier = item(family, "id")?;
            hashable(identifier, "set element")?;
            family_ids.push(identifier.clone());
        }
        unique(item(batch, "reviews")?, "attempt_id", &key_text)?;
        let run_name = match &key.0 {
            Value::Str(text) => {
                String::from_utf8_lossy(name_of(PyPath::new(text.as_bytes()).as_bytes()))
                    .into_owned()
            }
            other => {
                return type_error(format!(
                    "argument should be a str or an os.PathLike object where __fspath__ returns a str, not '{}'",
                    other.type_name()
                ));
            }
        };
        for review in each(item(batch, "reviews")?)?.iter() {
            let attempt_id = concat_text(&format!("{run_name}/"), item(review, "attempt_id")?)?;
            let attempt_key = s(&attempt_id);
            let inside = match attempts.get(&attempt_key)? {
                Some(attempt) => {
                    let cell = cells.get(item(attempt, "cell")?)?.ok_or_else(|| {
                        Failure::Crash(format!(
                            "KeyError: {}",
                            item(attempt, "cell")
                                .map(Value::py_repr)
                                .unwrap_or_default()
                        ))
                    })?;
                    pyobject::same(item(cell, "run")?, &key.0)
                        && pyobject::same(item(cell, "target")?, &key.1)
                }
                None => false,
            };
            require(
                inside,
                format!("{attempt_id}: review is outside selected batch"),
            )?;
            let attempt = attempts.get(&attempt_key)?.expect("just found").clone();
            require(
                !none(item(&attempt, "review")?),
                format!("{attempt_id}: review has no source output"),
            )?;
            let marked = get(batch, "verdicts")?;
            if is(marked, grading_validation::CONTRACT_V2) || is(marked, pre_existing::CONTRACT) {
                validate_review_v2(
                    root,
                    review,
                    &attempt,
                    reference,
                    documents,
                    canonical_claims,
                    &equivalent_items,
                    is(marked, pre_existing::CONTRACT),
                    conditional_safety,
                )?;
                continue;
            }
            validate_review_v1(
                root,
                review,
                &attempt,
                &attempt_id,
                reference,
                target,
                &family_ids,
                documents,
                canonical_claims,
                &equivalent_items,
            )?;
        }
    }
    Ok(())
}

fn has_exactly(value: &Value, fields: &[&str]) -> R<bool> {
    match value {
        Value::Object(object) => {
            Ok(object.len() == fields.len()
                && fields.iter().all(|field| object.get(field).is_some()))
        }
        Value::Array(_) | Value::Str(_) => {
            let members = set_of(value)?;
            Ok(members.len() == fields.len()
                && fields
                    .iter()
                    .all(|field| members.position(&s(field)).is_some()))
        }
        other => type_error(format!("'{}' object is not iterable", other.type_name())),
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "each argument is a separate input the caller already holds; a one-off parameter struct would not simplify the call"
)]
fn validate_review_v1(
    root: &Root,
    review: &Value,
    attempt: &Value,
    attempt_id: &str,
    reference: &Value,
    target: &Value,
    family_ids: &[Value],
    documents: &Documents,
    canonical_claims: &Dict,
    equivalent_items: &[((Value, Value), Value)],
) -> R<()> {
    let review_pin = item(attempt, "review")?;
    let has_family = |value: &Value| family_ids.iter().any(|known| pyobject::same(known, value));
    let claims = unique(item(review, "claims")?, "id", attempt_id)?;
    let mut groups: Vec<(Value, (Value, Value, Value))> = Vec::new();
    for claim in claims.values() {
        require(
            has_exactly(claim, &V1_CLAIM)?,
            format!(
                "{}: not a current-verdicts/v1 claim",
                item(claim, "id")?.py_str()
            ),
        )?;
        let anchor = item(claim, "anchor")?;
        validate_anchor(root, anchor, review_pin, target)?;
        require(
            item(claim, "evidence")?.truthy(),
            "claim assessment needs evidence or an explicit unresolved limitation",
        )?;
        let problems = claim_grading::assessment_problems(
            item(claim, "outcome")?,
            item(claim, "assessment")?,
        )?;
        require(
            problems.is_empty(),
            format!("{}: {}", item(claim, "id")?.py_str(), problems.join("; ")),
        )?;
        let family_id = item(claim, "family_id")?;
        let outcome = item(claim, "outcome")?;
        if !none(family_id) {
            hashable(family_id, "set element")?;
            require(
                has_family(family_id) && (is(outcome, "eligible") || is(outcome, "unresolved")),
                "claim family contradicts outcome",
            )?;
        }
        if is(outcome, "eligible") {
            hashable(family_id, "set element")?;
        }
        require(
            !is(outcome, "eligible") || has_family(family_id),
            "eligible claim needs a family",
        )?;
        let canonical_id = item(claim, "canonical_id")?;
        let anchor_key = (
            item(item(anchor, "review")?, "path")?.clone(),
            item(anchor, "item_id")?.clone(),
        );
        hashable_tuple(&[&anchor_key.0, &anchor_key.1], "dict key")?;
        let equivalent_id = equivalent_items
            .iter()
            .find(|(known, _)| pair_same(known, &anchor_key))
            .map(|(_, identifier)| identifier.clone());
        require(
            equivalent_id.is_none() || !none(canonical_id),
            "equivalent source item must retain its canonical assessment",
        )?;
        if !none(canonical_id) {
            let case = canonical_claims.get(canonical_id)?;
            let known = match case {
                Some(case) => eq(item(case, "target")?, target),
                None => false,
            };
            require(known, "unknown canonical claim")?;
            let case = case.expect("checked");
            let mut linked = false;
            for link in each(item(case, "links")?)?.iter() {
                if eq(item(link, "review")?, item(anchor, "review")?)
                    && eq(item(link, "item_id")?, item(anchor, "item_id")?)
                {
                    linked = true;
                    break;
                }
            }
            require(linked, "canonical claim is not linked to the original item")?;
            let mut decision = None;
            for known in each(item(documents.get("adjudication"), "decisions")?)?.iter() {
                if eq(item(known, "id")?, item(case, "adjudication")?) {
                    decision = Some(known.clone());
                    break;
                }
            }
            if equivalent_id
                .as_ref()
                .is_some_and(|identifier| eq(canonical_id, identifier))
            {
                require(
                    eq(family_id, item(case, "family_id")?),
                    "equivalent claim contradicts its canonical family",
                )?;
                match &decision {
                    Some(decision) if is(item(decision, "status")?, "approved") => require(
                        eq(outcome, item(decision, "outcome")?),
                        "equivalent claim contradicts applicable human ruling",
                    )?,
                    _ => require(
                        is(outcome, "unresolved"),
                        "pending canonical decision must remain unresolved",
                    )?,
                }
            }
        }
        let group = item(claim, "duplicate_group")?;
        let signature = (outcome.clone(), canonical_id.clone(), family_id.clone());
        if !none(group) {
            hashable(group, "dict key")?;
        }
        let conflict = !none(group)
            && groups.iter().any(|(known, value)| {
                pyobject::same(known, group)
                    && !(pyobject::same(&value.0, &signature.0)
                        && pyobject::same(&value.1, &signature.1)
                        && pyobject::same(&value.2, &signature.2))
            });
        require(!conflict, "conflicting duplicate claim group")?;
        if !none(group) {
            match groups
                .iter_mut()
                .find(|(known, _)| pyobject::same(known, group))
            {
                Some((_, value)) => *value = signature,
                None => groups.push((group.clone(), signature)),
            }
        }
    }
    let covered = unique(item(review, "families")?, "family_id", attempt_id)?;
    require(
        covered.keys().all(&has_family),
        "grade names unknown family",
    )?;
    let assessed = is(item(review, "state")?, "assessed");
    if assessed {
        require(
            covered.len() == distinct(family_ids).len() && covered.keys().all(&has_family),
            "assessed review must cover every family",
        )?;
        require(
            is(
                item(item(review, "remedy_inventory")?, "state")?,
                "complete",
            ),
            "assessed review has incomplete remedy inventory",
        )?;
    }
    let mut missed = false;
    if !assessed {
        for family in covered.values() {
            if is(item(family, "outcome")?, "missed") {
                missed = true;
                break;
            }
        }
    }
    if assessed || missed {
        let normalized = read_json(&root.resolve_pin(review_pin)?)?;
        let mut anchored = Dict::default();
        for claim in claims.values() {
            let identifier = item(item(claim, "anchor")?, "item_id")?;
            hashable(identifier, "set element")?;
            anchored.insert(identifier.clone(), Value::Null)?;
        }
        let count = length(item(&normalized, "items")?)?;
        let all = anchored.len() == count
            && (0..count).all(|at| anchored.position(&s(&format!("item-{at}"))).is_some());
        require(
            all,
            "assessed review or missed family must account for every original item",
        )?;
        let review_path = item(review_pin, "path")?;
        let mut present: Vec<(Value, Value)> = Vec::new();
        for claim in claims.values() {
            present.push((
                item(item(claim, "anchor")?, "item_id")?.clone(),
                item(claim, "canonical_id")?.clone(),
            ));
        }
        let complete = equivalent_items
            .iter()
            .filter(|((path, _), _)| eq(path, review_path))
            .all(|((_, item_id), identifier)| {
                present
                    .iter()
                    .any(|known| pair_same(known, &(item_id.clone(), identifier.clone())))
            });
        require(
            complete,
            "assessed review or missed family omits an applicable canonical claim",
        )?;
    }
    let recommendations = unique(item(review, "recommendations")?, "id", attempt_id)?;
    let mut remedy_groups: Vec<Value> = Vec::new();
    for recommendation in recommendations.values() {
        require(
            item(recommendation, "anchors")?.truthy()
                && item(recommendation, "addressed_claims")?.truthy(),
            "recommendation needs anchors and addressed claims",
        )?;
        for anchor in each(item(recommendation, "anchors")?)?.iter() {
            validate_anchor(root, anchor, review_pin, target)?;
        }
        let addressed = set_of(item(recommendation, "addressed_claims")?)?;
        require(
            addressed.keys().all(|key| claims.position(key).is_some()),
            "recommendation addresses unknown claims",
        )?;
        require(
            pyvalue::has(recommendation, "duplicate_group")?,
            "recommendation needs a duplicate group or null",
        )?;
        let group = item(recommendation, "duplicate_group")?;
        if !none(group) {
            hashable(group, "set element")?;
        }
        require(
            none(group)
                || !remedy_groups
                    .iter()
                    .any(|known| pyobject::same(known, group)),
            "duplicate remedy must be one recommendation with all original anchors",
        )?;
        if !none(group) {
            remedy_groups.push(group.clone());
        }
        let safety = item(recommendation, "safety")?;
        if !is(item(safety, "state")?, "unassessed") {
            let mut assessed = false;
            for check in each(item(safety, "independent_checks")?)?.iter() {
                if independent(check, true)? {
                    assessed = true;
                    break;
                }
            }
            require(assessed, "remedy safety needs an independent assessment")?;
        }
        let sufficiency = unique(
            item(recommendation, "sufficiency")?,
            "family_id",
            "recommendation sufficiency",
        )?;
        let mut named: Vec<Value> = Vec::new();
        for identifier in each(item(recommendation, "addressed_claims")?)?.iter() {
            let claim = claims
                .get(identifier)?
                .ok_or_else(|| Failure::Crash(format!("KeyError: {}", identifier.py_repr())))?;
            let family = item(claim, "family_id")?;
            hashable(family, "set element")?;
            if !none(family) && !named.iter().any(|known| pyobject::same(known, family)) {
                named.push(family.clone());
            }
        }
        require(
            sufficiency.len() == named.len()
                && named.iter().all(|key| sufficiency.position(key).is_some()),
            "assess sufficiency separately for every addressed family",
        )?;
    }
    validate_inventory(root, review, &recommendations, review_pin, target)?;
    let remedy_complete = is(
        item(item(review, "remedy_inventory")?, "state")?,
        "complete",
    );
    let admitted = is(item(item(attempt, "admission")?, "state")?, "admitted");
    for (family_id, family) in covered.entries.iter() {
        let cited = set_of(item(family, "claim_ids")?)?;
        require(
            cited.keys().all(|key| claims.position(key).is_some()),
            "family recovery cites unknown claims",
        )?;
        let mut recoveries: Vec<Value> = Vec::new();
        for claim in claims.values() {
            if eq(item(claim, "family_id")?, family_id) && is(item(claim, "outcome")?, "eligible") {
                recoveries.push(claim.clone());
            }
        }
        let reference_family = || -> R<Value> {
            for known in each(item(reference, "families")?)?.iter() {
                if eq(item(known, "id")?, family_id) {
                    return Ok(known.clone());
                }
            }
            Err(Failure::Crash("StopIteration".into()))
        };
        let outcome = item(family, "outcome")?;
        if is(outcome, "caught") {
            let known = reference_family()?;
            require(
                is(item(item(&known, "eligibility")?, "state")?, "approved"),
                "pending family cannot receive detection credit",
            )?;
            require(
                !recoveries.is_empty() && admitted,
                "caught family needs admitted supported claim",
            )?;
            let mut recovered = Dict::default();
            for claim in &recoveries {
                let identifier = item(claim, "id")?;
                hashable(identifier, "set element")?;
                recovered.insert(identifier.clone(), Value::Null)?;
            }
            require(
                cited.len() == recovered.len()
                    && cited.keys().all(|key| recovered.position(key).is_some()),
                "caught family must retain original claim provenance",
            )?;
            let mut remedies = Vec::new();
            for recommendation in recommendations.values() {
                for entry in each(item(recommendation, "sufficiency")?)?.iter() {
                    if eq(item(entry, "family_id")?, family_id) {
                        remedies.push(item(entry, "outcome")?.clone());
                    }
                }
            }
            let has = |name: &str| remedies.iter().any(|remedy| is(remedy, name));
            let expected = if has("sufficient") {
                "sufficient"
            } else if has("unassessed") {
                "unassessed"
            } else if !remedies.is_empty() {
                "partial"
            } else if remedy_complete {
                "absent"
            } else {
                "unassessed"
            };
            require(
                is(item(family, "sufficiency")?, expected),
                "family fix sufficiency contradicts distinct recommendations",
            )?;
        } else if is(outcome, "missed") {
            require(
                recoveries.is_empty(),
                "missed family has an eligible recovery",
            )?;
            let mut unresolved = false;
            for claim in claims.values() {
                if eq(item(claim, "family_id")?, family_id)
                    && is(item(claim, "outcome")?, "unresolved")
                {
                    unresolved = true;
                    break;
                }
            }
            require(!unresolved, "unresolved family recovery cannot be missed")?;
            let known = reference_family()?;
            require(
                is(item(item(&known, "eligibility")?, "state")?, "approved"),
                "pending family recovery must remain unresolved",
            )?;
            require(
                !item(family, "claim_ids")?.truthy()
                    && is(item(family, "sufficiency")?, "unassessed"),
                "missed family has no remedy sufficiency",
            )?;
        } else {
            require(
                is(item(family, "sufficiency")?, "unassessed"),
                "unresolved recovery has unassessed sufficiency",
            )?;
        }
    }
    validate_advice(review, &claims)
}

fn distinct(values: &[Value]) -> Vec<&Value> {
    let mut members: Vec<&Value> = Vec::new();
    for value in values {
        if !members.iter().any(|member| pyobject::same(member, value)) {
            members.push(value);
        }
    }
    members
}

fn validate_inventory(
    root: &Root,
    review: &Value,
    recommendations: &Dict,
    review_pin: &Value,
    target: &Value,
) -> R<()> {
    let inventory = item(review, "remedy_inventory")?;
    for anchor in each(item(inventory, "anchors")?)?.iter() {
        validate_anchor(root, anchor, review_pin, target)?;
    }
    if is(item(inventory, "state")?, "complete") {
        let mut indexed: Vec<String> = each(item(inventory, "anchors")?)?
            .iter()
            .map(digest)
            .collect();
        indexed.sort();
        indexed.dedup();
        let mut recorded: Vec<String> = Vec::new();
        for recommendation in recommendations.values() {
            for anchor in each(item(recommendation, "anchors")?)?.iter() {
                recorded.push(digest(anchor));
            }
        }
        recorded.sort();
        recorded.dedup();
        require(
            indexed == recorded,
            "remedy inventory does not cover every distinct recommendation",
        )?;
        let normalized = read_json(&root.resolve_pin(review_pin)?)?;
        let mut proposed = Vec::new();
        for (at, entry) in each(item(&normalized, "items")?)?.iter().enumerate() {
            if get(entry, "proposed_fix")?.truthy() {
                proposed.push(s(&format!("item-{at}")));
            }
        }
        let mut anchored: Vec<Value> = Vec::new();
        for recommendation in recommendations.values() {
            for anchor in each(item(recommendation, "anchors")?)?.iter() {
                let identifier = item(anchor, "item_id")?;
                hashable(identifier, "set element")?;
                anchored.push(identifier.clone());
            }
        }
        require(
            proposed
                .iter()
                .all(|wanted| anchored.iter().any(|known| pyobject::same(known, wanted))),
            "complete remedy inventory omits an original proposed fix",
        )?;
    }
    Ok(())
}

fn validate_advice(review: &Value, claims: &Dict) -> R<()> {
    for advice in each(item(review, "advice")?)?.iter() {
        let cited = item(advice, "claim_ids")?;
        let members = set_of(cited)?;
        require(
            members.keys().all(|key| claims.position(key).is_some()) && cited.truthy(),
            "advice needs original claims",
        )?;
        require(
            is(item(advice, "kind")?, "sampled") == !none(item(advice, "sample")?),
            "sampled advice needs population, selection and limits",
        )?;
        require(
            is(item(advice, "kind")?, "sampled") || is(item(advice, "benefit")?, "unresolved"),
            "generic advice is not sampled benefit evidence",
        )?;
        if !is(item(advice, "benefit")?, "unresolved") {
            require(
                item(advice, "evidence")?.truthy() && item(advice, "independent_checks")?.truthy(),
                "advice benefit needs independent evidence",
            )?;
        }
    }
    Ok(())
}

/// A saved v2 claim in the shape the verdict rules of `claim_grading` read.
pub(crate) fn verdict_claim(claim: &Value) -> R<Value> {
    let mut verdict = Object::default();
    verdict.insert("id".into(), item(claim, "id")?.clone());
    verdict.insert(
        "quote".into(),
        item(item(claim, "anchor")?, "quote")?.clone(),
    );
    match item(claim, "answers")? {
        Value::Object(answers) => {
            for (key, value) in answers.iter() {
                verdict.insert(key.clone(), value.clone());
            }
        }
        other => {
            return type_error(format!("'{}' object is not a mapping", other.type_name()));
        }
    }
    verdict.insert("outcome".into(), item(claim, "outcome")?.clone());
    verdict.insert("kind".into(), item(claim, "kind")?.clone());
    verdict.insert(
        "canonical_claim_id".into(),
        item(claim, "canonical_id")?.clone(),
    );
    verdict.insert(
        "duplicate_group".into(),
        item(claim, "duplicate_group")?.clone(),
    );
    verdict.insert("candidate".into(), item(claim, "candidate_id")?.clone());
    verdict.insert("open".into(), item(claim, "open")?.clone());
    verdict.insert("notes".into(), item(claim, "reason")?.clone());
    verdict.insert("evidence".into(), Value::Array(vec![s("saved")]));
    let mut known = Vec::new();
    for entry in each(item(claim, "known_problems")?)?.iter() {
        known.push(obj(vec![
            ("family", item(entry, "family_id")?.clone()),
            ("says_what", item(entry, "says_what")?.clone()),
            ("identifies_cause", item(entry, "identifies_cause")?.clone()),
            ("reason", item(entry, "reason")?.clone()),
        ]));
    }
    verdict.insert("known_problems".into(), Value::Array(known));
    Ok(Value::Object(verdict))
}

/// One review graded under verdict contract v2: every claim satisfies the ordered questions, the saved rulings on
/// its canonical claims and on single comments hold, and each known problem's recovery and fix sufficiency are the
/// ones the claims and recommendations give.
#[allow(
    clippy::too_many_arguments,
    reason = "each argument is a separate input the caller already holds; a one-off parameter struct would not simplify the call"
)]
fn validate_review_v2(
    root: &Root,
    review: &Value,
    attempt: &Value,
    reference: &Value,
    documents: &Documents,
    canonical_claims: &Dict,
    equivalent_items: &[((Value, Value), Value)],
    v3: bool,
    conditional_safety: bool,
) -> R<()> {
    let target = item(reference, "target")?;
    let pin = item(attempt, "review")?;
    let place = item(attempt, "id")?.py_str();
    let mut family_ids: Vec<Value> = Vec::new();
    for family in each(item(reference, "families")?)?.iter() {
        family_ids.push(item(family, "id")?.clone());
    }
    let mut review_fields: Vec<&str> = V2_REVIEW.to_vec();
    if v3 {
        review_fields.push("pre_existing_problems");
    }
    require(
        has_exactly(review, &review_fields)?,
        format!("{place}: not a current-verdicts/v2 review"),
    )?;
    let claims = unique(item(review, "claims")?, "id", &place)?;
    let mut verdicts = Dict::default();
    let mut groups: Vec<(Value, Value)> = Vec::new();
    let decisions = {
        let mut decisions = Dict::default();
        for decision in each(item(documents.get("adjudication"), "decisions")?)?.iter() {
            decisions.insert(item(decision, "id")?.clone(), decision.clone())?;
        }
        decisions
    };
    let candidates = {
        let mut candidates = Dict::default();
        for candidate in each(item(documents.get("candidate"), "candidates")?)?.iter() {
            candidates.insert(item(candidate, "id")?.clone(), candidate.clone())?;
        }
        candidates
    };
    let empty = Value::Array(Vec::new());
    for claim in claims.values() {
        let mut claim_fields: Vec<&str> = V2_CLAIM.to_vec();
        if v3 {
            claim_fields.extend(["pre_existing_problems", "pre_existing_candidate_id"]);
        }
        require(
            has_exactly(claim, &claim_fields)?,
            format!(
                "{}: not a current-verdicts/v2 claim",
                item(claim, "id")?.py_str()
            ),
        )?;
        let anchor = item(claim, "anchor")?;
        validate_anchor(root, anchor, pin, target)?;
        let candidate_id = item(claim, "candidate_id")?;
        if !none(candidate_id) {
            let candidate = candidates.get(candidate_id)?;
            let on_record = match candidate {
                Some(candidate) => {
                    eq(item(candidate, "target")?, target)
                        && pyvalue::contains(item(candidate, "anchors")?, anchor)?
                        && is(get_or(candidate, "kind", &s("ordinary"))?, "ordinary")
                        && none(item(candidate, "decision")?)
                }
                None => false,
            };
            require(
                on_record,
                format!(
                    "{}: names a candidate that is not on record for this wording",
                    item(claim, "id")?.py_str()
                ),
            )?;
        }
        require(
            item(claim, "evidence")?.truthy(),
            "claim assessment needs evidence or an explicit unresolved limitation",
        )?;
        let mut verdict = verdict_claim(claim)?;
        let problems = if v3 {
            if let (Value::Object(verdict), Value::Object(saved)) =
                (&mut verdict, pre_existing::saved_claim(claim)?)
            {
                for (key, value) in saved.iter() {
                    verdict.insert(key.clone(), value.clone());
                }
            }
            let mut problem_ids = Vec::new();
            for problem in each(get_or(reference, "pre_existing_problems", &empty)?)?.iter() {
                problem_ids.push(item(problem, "id")?.clone());
            }
            let problems = pre_existing::verdict_problems(
                &verdict,
                &Value::Array(family_ids.clone()),
                &Value::Array(problem_ids),
            )?;
            let pre_candidate_id = item(claim, "pre_existing_candidate_id")?;
            let pending = none(pre_candidate_id)
                || match candidates.get(pre_candidate_id)? {
                    Some(candidate) => {
                        eq(item(candidate, "target")?, target)
                            && is(get(candidate, "kind")?, "pre-existing-problem")
                            && none(item(candidate, "decision")?)
                            && pyvalue::contains(item(candidate, "anchors")?, anchor)?
                    }
                    None => false,
                };
            require(
                pending,
                "pre-existing-problem candidate is not pending for this wording",
            )?;
            problems
        } else {
            claim_grading::verdict_problems_v2(&verdict, &Value::Array(family_ids.clone()))?
        };
        verdicts.insert(item(claim, "id")?.clone(), verdict.clone())?;
        require(
            problems.is_empty(),
            format!("{}: {}", item(claim, "id")?.py_str(), problems.join("; ")),
        )?;
        let canonical_id = item(claim, "canonical_id")?;
        if !none(canonical_id) {
            let case = canonical_claims.get(canonical_id)?;
            let known = match case {
                Some(case) => eq(item(case, "target")?, target),
                None => false,
            };
            require(known, "unknown canonical claim")?;
            let case = case.expect("checked");
            let mut linked = false;
            for link in each(item(case, "links")?)?.iter() {
                if eq(item(link, "review")?, item(anchor, "review")?)
                    && eq(item(link, "item_id")?, item(anchor, "item_id")?)
                {
                    linked = true;
                    break;
                }
            }
            require(linked, "canonical claim is not linked to the original item")?;
            let anchor_key = (
                item(item(anchor, "review")?, "path")?.clone(),
                item(anchor, "item_id")?.clone(),
            );
            hashable_tuple(&[&anchor_key.0, &anchor_key.1], "dict key")?;
            let equivalent = equivalent_items
                .iter()
                .find(|(known, _)| pair_same(known, &anchor_key))
                .map(|(_, identifier)| identifier);
            if equivalent.is_some_and(|identifier| eq(canonical_id, identifier)) {
                let decision = decisions.get(item(case, "adjudication")?)?;
                let approved = match decision {
                    Some(decision) => is(item(decision, "status")?, "approved"),
                    None => false,
                };
                let pinned = obj(vec![
                    (
                        "outcome",
                        if approved {
                            item(decision.expect("approved"), "outcome")?.clone()
                        } else {
                            s("unresolved")
                        },
                    ),
                    ("family", item(case, "family_id")?.clone()),
                ]);
                require(
                    grading_validation::pinned_matches_v2(&verdict, &pinned)?,
                    "equivalent claim contradicts applicable human ruling",
                )?;
                if v3 && !none(get(case, "pre_existing_problem_id")?) {
                    let wanted = item(case, "pre_existing_problem_id")?;
                    let mut found = false;
                    for entry in each(item(&verdict, "pre_existing_problems")?)?.iter() {
                        if eq(item(entry, "problem")?, wanted) {
                            found = true;
                            break;
                        }
                    }
                    require(
                        found,
                        "equivalent claim omits its pre-existing-problem link",
                    )?;
                }
            }
        }
        let group = item(claim, "duplicate_group")?;
        if !none(group) {
            let signature =
                duplicate_signature(&verdict, item(claim, "outcome")?, canonical_id, v3)?;
            hashable(group, "dict key")?;
            match groups
                .iter()
                .find(|(known, _)| pyobject::same(known, group))
            {
                Some((_, known)) => {
                    require(eq(known, &signature), "conflicting duplicate claim group")?
                }
                None => groups.push((group.clone(), signature)),
            }
        }
    }
    let normalized = read_json(&root.resolve_pin(pin)?)?;
    let count = length(item(&normalized, "items")?)?;
    let mut quiet: Vec<Value> = Vec::new();
    for entry in each(item(review, "not_findings")?)?.iter() {
        quiet.push(item(entry, "item_id")?.clone());
    }
    let mut found: Vec<Value> = Vec::new();
    for claim in claims.values() {
        let identifier = item(item(claim, "anchor")?, "item_id")?;
        hashable(identifier, "set element")?;
        if !found.iter().any(|known| pyobject::same(known, identifier)) {
            found.push(identifier.clone());
        }
    }
    for identifier in &quiet {
        hashable(identifier, "set element")?;
    }
    let quiet_distinct = distinct(&quiet);
    let all_items: Vec<Value> = (0..count).map(|at| s(&format!("item-{at}"))).collect();
    let accounted = quiet_distinct.len() == quiet.len()
        && !quiet_distinct
            .iter()
            .any(|known| found.iter().any(|other| pyobject::same(other, known)))
        && {
            let joined: Vec<Value> = quiet.iter().chain(found.iter()).cloned().collect();
            let union = distinct(&joined);
            union.len() == all_items.len()
                && all_items
                    .iter()
                    .all(|wanted| union.iter().any(|known| pyobject::same(known, wanted)))
        };
    require(
        accounted,
        "every original item holds claims or is recorded as not a finding",
    )?;
    let pin_path = item(pin, "path")?;
    let mut present: Vec<(Value, Value)> = Vec::new();
    for claim in claims.values() {
        present.push((
            item(item(claim, "anchor")?, "item_id")?.clone(),
            item(claim, "canonical_id")?.clone(),
        ));
    }
    let complete = equivalent_items
        .iter()
        .filter(|((path, _), _)| eq(path, pin_path))
        .all(|((_, item_id), identifier)| {
            present
                .iter()
                .any(|known| pair_same(known, &(item_id.clone(), identifier.clone())))
        });
    require(
        complete,
        "an equivalent source item must retain its canonical claim",
    )?;
    for ruling in each(item(documents.get("credit"), "rulings")?)?.iter() {
        if eq(item(ruling, "review")?, pin) && !pyvalue::has(ruling, "pre_existing_problem_id")? {
            let entries = known_entries(
                &claims,
                &verdicts,
                item(ruling, "item_id")?,
                "known_problems",
                "family",
                item(ruling, "family_id")?,
            )?;
            let wanted = obj(vec![
                ("family", item(ruling, "family_id")?.clone()),
                ("says_what", item(ruling, "says_what")?.clone()),
                (
                    "identifies_cause",
                    item(ruling, "identifies_cause")?.clone(),
                ),
            ]);
            let refs: Vec<&Value> = entries.iter().collect();
            let problems = claim_grading::credit_problems(&refs, &wanted)?;
            require(
                problems.is_empty(),
                format!(
                    "{place} {}: {}",
                    item(ruling, "item_id")?.py_str(),
                    problems.join("; ")
                ),
            )?;
        }
    }
    let recommendations = unique(item(review, "recommendations")?, "id", &place)?;
    for recommendation in recommendations.values() {
        require(
            has_exactly(recommendation, &V2_RECOMMENDATION)?
                && item(recommendation, "anchors")?.truthy(),
            "recommendation needs its original anchors",
        )?;
        for anchor in each(item(recommendation, "anchors")?)?.iter() {
            validate_anchor(root, anchor, pin, target)?;
        }
        let addressed = item(recommendation, "addressed_claims")?;
        let members = set_of(addressed)?;
        require(
            members.len() == length(addressed)?
                && members.keys().all(|key| claims.position(key).is_some()),
            "recommendation addresses unknown claims",
        )?;
        let safety = item(recommendation, "safety")?;
        if !is(item(safety, "state")?, "unassessed") {
            let mut assessed = false;
            for check in each(item(safety, "independent_checks")?)?.iter() {
                if independent(check, true)? {
                    assessed = true;
                    break;
                }
            }
            require(assessed, "remedy safety needs an independent assessment")?;
        }
        let sufficiency = unique(
            item(recommendation, "sufficiency")?,
            "family_id",
            "recommendation sufficiency",
        )?;
        if conditional_safety {
            let mut fixes = false;
            for entry in sufficiency.values() {
                if pyvalue::one_of(item(entry, "outcome")?, &["sufficient", "partial"]) {
                    fixes = true;
                    break;
                }
            }
            if !fixes {
                require(
                    is(item(safety, "state")?, "unassessed")
                        && !item(safety, "independent_checks")?.truthy(),
                    "skip safety unless a fix is sufficient or partial",
                )?;
            }
        } else {
            let mut effective = true;
            for entry in sufficiency.values() {
                if is(item(entry, "outcome")?, "ineffective") {
                    effective = false;
                    break;
                }
            }
            require(
                effective,
                "ineffective sufficiency requires the conditional safety policy",
            )?;
        }
        let mut named: Vec<Value> = Vec::new();
        for identifier in each(addressed)?.iter() {
            let verdict = verdicts
                .get(identifier)?
                .ok_or_else(|| Failure::Crash(format!("KeyError: {}", identifier.py_repr())))?;
            for entry in each(item(verdict, "known_problems")?)?.iter() {
                let facts = (item(entry, "says_what")?, item(entry, "identifies_cause")?);
                if is(facts.0, "yes") || is(facts.1, "yes") {
                    let family = item(entry, "family")?;
                    hashable(family, "set element")?;
                    if !named.iter().any(|known| pyobject::same(known, family)) {
                        named.push(family.clone());
                    }
                }
            }
        }
        require(
            sufficiency.len() == named.len()
                && named.iter().all(|key| sufficiency.position(key).is_some()),
            "assess sufficiency separately for every addressed known problem",
        )?;
    }
    validate_inventory(root, review, &recommendations, pin, target)?;
    let covered = unique(item(review, "families")?, "family_id", &place)?;
    let has_family = |value: &Value| family_ids.iter().any(|known| pyobject::same(known, value));
    for identifier in &family_ids {
        hashable(identifier, "set element")?;
    }
    require(
        covered.keys().all(&has_family),
        "grade names unknown family",
    )?;
    let complete = is(
        item(item(review, "remedy_inventory")?, "state")?,
        "complete",
    );
    if is(item(review, "state")?, "assessed") {
        require(
            covered.len() == distinct(&family_ids).len() && covered.keys().all(&has_family),
            "assessed review must cover every family",
        )?;
        require(complete, "assessed review has incomplete remedy inventory")?;
    }
    let admitted = is(item(item(attempt, "admission")?, "state")?, "admitted");
    let verdict_list: Vec<Value> = verdicts.values().cloned().collect();
    for family in each(item(reference, "families")?)?.iter() {
        let Some(saved) = covered.get(item(family, "id")?)? else {
            continue;
        };
        require(
            has_exactly(saved, &V2_FAMILY)?,
            format!(
                "{}: not a current-verdicts/v2 recovery",
                item(family, "id")?.py_str()
            ),
        )?;
        let recovery = claim_grading::family_recovery_v2(family, &verdict_list, admitted)?;
        let mut remedies = Vec::new();
        for recommendation in recommendations.values() {
            for entry in each(item(recommendation, "sufficiency")?)?.iter() {
                if eq(item(entry, "family_id")?, item(family, "id")?) {
                    remedies.push(item(entry, "outcome")?.clone());
                }
            }
        }
        let mut saved_ids = each(item(saved, "claim_ids")?)?.into_owned();
        sort_values(&mut saved_ids)?;
        let mut expected_ids = recovery.claim_ids.clone();
        sort_values(&mut expected_ids)?;
        let sufficiency = claim_grading::family_sufficiency(recovery.outcome, &remedies, complete);
        let same = is(item(saved, "outcome")?, recovery.outcome)
            && eq(&Value::Array(saved_ids), &Value::Array(expected_ids))
            && eq(
                item(saved, "cause_only")?,
                &Value::Bool(recovery.cause_only),
            )
            && is(item(saved, "sufficiency")?, sufficiency);
        require(
            same,
            format!(
                "{}: saved recovery differs from the one its claims and recommendations give",
                item(family, "id")?.py_str()
            ),
        )?;
    }
    if v3 {
        let covered = unique(item(review, "pre_existing_problems")?, "problem_id", &place)?;
        let mut expected = Dict::default();
        for problem in each(get_or(reference, "pre_existing_problems", &empty)?)?.iter() {
            expected.insert(
                item(problem, "id")?.clone(),
                pre_existing::assessment(problem, &verdict_list, admitted)?,
            )?;
        }
        let equal = covered.len() == expected.len()
            && covered.entries.iter().all(|(key, value)| {
                expected
                    .get(key)
                    .ok()
                    .flatten()
                    .is_some_and(|other| pyobject::same(value, other))
            });
        require(
            equal,
            "pre-existing-problem assessment differs from its claims or omits a listed problem",
        )?;
        for ruling in each(item(documents.get("credit"), "rulings")?)?.iter() {
            if !eq(item(ruling, "review")?, pin)
                || !pyvalue::has(ruling, "pre_existing_problem_id")?
            {
                continue;
            }
            let entries = known_entries(
                &claims,
                &verdicts,
                item(ruling, "item_id")?,
                "pre_existing_problems",
                "problem",
                item(ruling, "pre_existing_problem_id")?,
            )?;
            let wanted = obj(vec![
                ("family", item(ruling, "pre_existing_problem_id")?.clone()),
                ("says_what", item(ruling, "says_what")?.clone()),
                (
                    "identifies_cause",
                    item(ruling, "identifies_cause")?.clone(),
                ),
            ]);
            let refs: Vec<&Value> = entries.iter().collect();
            let problems = claim_grading::credit_problems(&refs, &wanted)?;
            require(
                problems.is_empty(),
                format!("{place}: {}", problems.join("; ")),
            )?;
        }
    }
    validate_advice(review, &claims)
}

/// The known-problem entries of one item's claims for one problem, in claim order.
fn known_entries(
    claims: &Dict,
    verdicts: &Dict,
    item_id: &Value,
    list: &str,
    field: &str,
    subject: &Value,
) -> R<Vec<Value>> {
    let mut entries = Vec::new();
    for claim in claims.values() {
        if eq(item(item(claim, "anchor")?, "item_id")?, item_id) {
            let verdict = verdicts
                .get(item(claim, "id")?)?
                .ok_or_else(|| Failure::Crash("KeyError".into()))?;
            for entry in each(item(verdict, list)?)?.iter() {
                if eq(item(entry, field)?, subject) {
                    entries.push(entry.clone());
                }
            }
        }
    }
    Ok(entries)
}

/// The tuple a duplicate group's claims must share, as a JSON array for comparison.
fn duplicate_signature(
    verdict: &Value,
    outcome: &Value,
    canonical_id: &Value,
    v3: bool,
) -> R<Value> {
    let facts = |list: &str, field: &str| -> R<Value> {
        let mut rows = Vec::new();
        for entry in each(item(verdict, list)?)?.iter() {
            rows.push(Value::Array(vec![
                item(entry, field)?.clone(),
                item(entry, "says_what")?.clone(),
                item(entry, "identifies_cause")?.clone(),
            ]));
        }
        sort_rows(&mut rows)?;
        Ok(Value::Array(rows))
    };
    let mut signature = vec![
        outcome.clone(),
        canonical_id.clone(),
        facts("known_problems", "family")?,
    ];
    if v3 {
        signature.push(facts("pre_existing_problems", "problem")?);
    }
    Ok(Value::Array(signature))
}

/// `sorted(tuples)`: element by element, each pair of elements compared as Python compares them.
fn sort_rows(rows: &mut [Value]) -> R<()> {
    let compare = |left: &Value, right: &Value| -> R<std::cmp::Ordering> {
        let (Value::Array(left), Value::Array(right)) = (left, right) else {
            return Ok(std::cmp::Ordering::Equal);
        };
        for (a, b) in left.iter().zip(right) {
            if pyobject::same(a, b) {
                continue;
            }
            return match (a, b) {
                (Value::Null, Value::Null) => Ok(std::cmp::Ordering::Equal),
                _ => Ok(pyvalue::compare(a, b)?),
            };
        }
        Ok(left.len().cmp(&right.len()))
    };
    for at in 1..rows.len() {
        compare(&rows[at], &rows[at - 1])?;
    }
    let mut failure = None;
    rows.sort_by(|left, right| {
        compare(left, right).unwrap_or_else(|error| {
            failure.get_or_insert(error);
            std::cmp::Ordering::Equal
        })
    });
    match failure {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

/// `sorted(values)`.
fn sort_values(values: &mut Vec<Value>) -> R<()> {
    let sorted = pyvalue::sorted(std::mem::take(values))?;
    *values = sorted;
    Ok(())
}

/// `validate_anchor(anchor, review_pin, target, root)`: the anchor names an item of the original review and quotes
/// it verbatim.
fn validate_anchor(root: &Root, anchor: &Value, review_pin: &Value, target: &Value) -> R<()> {
    require(
        eq(item(anchor, "review")?, review_pin),
        "anchor is not in the original review",
    )?;
    let attempt = match item(review_pin, "path")? {
        Value::Str(text) => {
            let path = PyPath::new(text.as_bytes());
            String::from_utf8_lossy(name_of(parent_of(&path).as_bytes())).into_owned()
        }
        other => {
            return type_error(format!(
                "argument should be a str or an os.PathLike object where __fspath__ returns a str, not '{}'",
                other.type_name()
            ));
        }
    };
    let link = obj(vec![
        ("review", item(anchor, "review")?.clone()),
        ("attempt_id", s(&attempt)),
        ("item_id", item(anchor, "item_id")?.clone()),
    ]);
    let (_, found) = source_item(root, &link, target)?;
    let values = match &found {
        Value::Object(object) => object
            .iter()
            .map(|(_, value)| value.clone())
            .collect::<Vec<_>>(),
        other => {
            return Err(Failure::Crash(format!(
                "AttributeError: '{}' object has no attribute 'values'",
                other.type_name()
            )));
        }
    };
    let mut verbatim = false;
    for value in &values {
        if let Value::Str(text) = value
            && pyvalue::contains(&Value::Str(text.clone()), item(anchor, "quote")?)?
        {
            verbatim = true;
            break;
        }
    }
    require(verbatim, "anchor quote is not verbatim in original item")
}

/// `load_current(root, current, grades)`. `grades=False` leaves saved grades unchecked, so a queue can still be
/// read while some are stale.
pub fn load_current(root: &Root, current: &[u8], grades: bool) -> R<(Value, Documents)> {
    let directory = root.local(current)?;
    let selected = inventory(root)?;
    let stored = read_json(&directory.join(b"inventory.json"))?;
    require(
        eq(&stored, &selected),
        "current inventory differs from selected saved sources; regenerate inventory",
    )?;
    let mut kinds: Vec<(&'static str, Value)> = Vec::new();
    for kind in KINDS {
        let name = if kind == "adjudication" {
            "adjudications.json".to_string()
        } else {
            format!("{kind}s.json")
        };
        kinds.push((kind, read_json(&directory.join(name.as_bytes()))?));
    }
    kinds.push((
        "policy",
        root.pin_file(&directory.join(b"validation-policy.json"))?,
    ));
    verify_pins(
        root,
        &read_json(&directory.join(b"validation-policy.json"))?,
    )?;
    let audit = read_json(&directory.join(b"audits.json"))?;
    verify_pins(root, &audit)?;
    kinds.push(("audit", audit));
    let documents = Documents { kinds };
    validate_documents(root, &documents, &selected, grades)?;
    Ok((selected, documents))
}

/// Hold the current record for one writer, with the `flock` `current_grading.record_lock` takes. A grade replaced
/// while an audit concludes would escape the audit's check that the grades are still the ones its sample was drawn
/// from. The lock is released when the returned file is dropped.
pub fn record_lock(root: &[u8]) -> std::io::Result<std::fs::File> {
    let path = PyPath::new(&pyfs::resolve(root))
        .join(b".cache/current-record.lock")
        .to_path_buf();
    std::fs::create_dir_all(path.parent().expect("the lock has a parent directory"))?;
    let file = std::fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(&path)?;
    file.lock()?;
    Ok(file)
}

/// A clean control stays provisional while a potentially eligible candidate on its task awaits a ruling.
pub fn control_state(reference: &Value, candidates: &Value) -> R<Value> {
    let target = item(reference, "target")?;
    let mut pending = false;
    for candidate in each(candidates)?.iter() {
        if eq(item(candidate, "target")?, target) && none(item(candidate, "decision")?) {
            pending = true;
            break;
        }
    }
    let status = item(item(reference, "control")?, "status")?;
    Ok(if pending && is(status, "audited-clean") {
        s("provisional")
    } else {
        status.clone()
    })
}

/// `coverage_status(selected, documents)`: what `status` prints, with the record's `dataset_hash`.
pub fn coverage_status(selected: &Value, documents: &Documents) -> R<Value> {
    let mut required: Vec<Value> = Vec::new();
    for attempt in each(item(selected, "attempts")?)?.iter() {
        if is(item(item(attempt, "admission")?, "state")?, "admitted") {
            let identifier = item(attempt, "id")?;
            hashable(identifier, "set element")?;
            if !required
                .iter()
                .any(|known| pyobject::same(known, identifier))
            {
                required.push(identifier.clone());
            }
        }
    }
    let mut reviews = Dict::default();
    for batch in each(item(documents.get("grade"), "batches")?)?.iter() {
        let run = match item(batch, "run")? {
            Value::Str(text) => {
                String::from_utf8_lossy(name_of(PyPath::new(text.as_bytes()).as_bytes()))
                    .into_owned()
            }
            other => {
                return type_error(format!(
                    "argument should be a str or an os.PathLike object where __fspath__ returns a str, not '{}'",
                    other.type_name()
                ));
            }
        };
        for review in each(item(batch, "reviews")?)?.iter() {
            let identifier = concat_text(&format!("{run}/"), item(review, "attempt_id")?)?;
            reviews.insert(s(&identifier), review.clone())?;
        }
    }
    let mut assessed: Vec<&Value> = Vec::new();
    for (identifier, review) in reviews.entries.iter() {
        if is(item(review, "state")?, "assessed") {
            assessed.push(identifier);
        }
    }
    let is_required = |identifier: &Value| {
        required
            .iter()
            .any(|known| pyobject::same(known, identifier))
    };
    let is_assessed = |identifier: &Value| {
        assessed
            .iter()
            .any(|known| pyobject::same(known, identifier))
    };
    let admitted: Vec<&Value> = reviews
        .entries
        .iter()
        .filter(|(identifier, _)| is_required(identifier))
        .map(|(_, review)| review)
        .collect();
    let mut unresolved = 0;
    for review in &admitted {
        for family in each(item(review, "families")?)?.iter() {
            if is(item(family, "outcome")?, "unresolved") {
                unresolved += 1;
            }
        }
    }
    let mut unresolved_claims = 0;
    for review in &admitted {
        for claim in each(item(review, "claims")?)?.iter() {
            if is(item(claim, "outcome")?, "unresolved") {
                unresolved_claims += 1;
            }
        }
    }
    let mut missing: Vec<Value> = required
        .iter()
        .filter(|identifier| !is_assessed(identifier))
        .cloned()
        .collect();
    sort_values(&mut missing)?;
    let candidates = item(documents.get("candidate"), "candidates")?;
    let mut pending = Vec::new();
    for candidate in each(candidates)?.iter() {
        if none(item(candidate, "decision")?) {
            pending.push(obj(vec![
                ("id", item(candidate, "id")?.clone()),
                ("target", item(candidate, "target")?.clone()),
                ("recorded_at", item(candidate, "recorded_at")?.clone()),
                ("limits", item(candidate, "limits")?.clone()),
                ("relevance", item(candidate, "relevance")?.clone()),
            ]));
        }
    }
    let mut trials = Object::default();
    for cell in each(item(selected, "cells")?)?.iter() {
        let state = item(cell, "state")?;
        let Value::Str(name) = state else {
            return type_error("keys must be str, int, float, bool or None, not list".into());
        };
        let count = match trials.get(name) {
            Some(Value::Int(count)) => count.parse::<usize>().unwrap_or(0) + 1,
            _ => 1,
        };
        trials.insert(name.clone(), int(count));
    }
    let mut controls = Object::default();
    for reference in each(item(documents.get("reference"), "targets")?)?.iter() {
        let target = item(reference, "target")?;
        let state = control_state(reference, candidates)?;
        controls.insert(target.py_str(), state);
    }
    let incomplete = !missing.is_empty() || unresolved > 0 || unresolved_claims > 0;
    let mut dataset = Object::default();
    dataset.insert("inventory".into(), selected.clone());
    for (kind, value) in &documents.kinds {
        dataset.insert((*kind).to_string(), value.clone());
    }
    let assessed_count = required
        .iter()
        .filter(|identifier| is_assessed(identifier))
        .count();
    Ok(obj(vec![
        ("counts", item(selected, "counts")?.clone()),
        ("trials", Value::Object(trials)),
        ("required_reviews", int(required.len())),
        ("assessed_reviews", int(assessed_count)),
        ("ungraded_reviews", Value::Array(missing)),
        ("unresolved_recoveries", int(unresolved)),
        ("unresolved_claims", int(unresolved_claims)),
        ("pending_candidates", Value::Array(pending)),
        ("controls", Value::Object(controls)),
        ("complete", Value::Bool(!incomplete)),
        (
            "reason",
            s(if incomplete {
                "Current judgment coverage is incomplete."
            } else {
                "All admitted reviews have current assessments."
            }),
        ),
        ("dataset_hash", s(&digest(&Value::Object(dataset)))),
    ]))
}

/// Each selected batch with its input fingerprint and state, read without checking saved grades, so a stale batch
/// reports `stale` rather than refusing the record.
pub fn fingerprints(root: &Root, current: &[u8]) -> R<Value> {
    let (selected, documents) = load_current(root, current, false)?;
    let mut rows = Vec::new();
    for batch in each(item(&selected, "batches")?)?.iter() {
        let (fingerprint, state) = batch_state(root, batch, &selected, &documents)?;
        let mut row = batch.as_object().cloned().unwrap_or_default();
        row.insert("input_fingerprint".into(), s(&fingerprint));
        row.insert("state".into(), s(state));
        rows.push(Value::Object(row));
    }
    Ok(Value::Array(rows))
}

/// `Path.mkdir(parents=True, exist_ok=True)`.
pub(crate) fn make_parents(path: &PyPath) -> R<()> {
    let target = path.to_path_buf();
    match std::fs::create_dir(&target) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let parent = parent_of(path);
            if parent == *path {
                return Err(Failure::Input(pytext::os_error(&error, path.as_bytes())));
            }
            make_parents(&parent)?;
            match std::fs::create_dir(&target) {
                Ok(()) => Ok(()),
                Err(_) if target.is_dir() => Ok(()),
                Err(error) => Err(Failure::Input(pytext::os_error(&error, path.as_bytes()))),
            }
        }
        Err(_) if target.is_dir() => Ok(()),
        Err(error) => Err(Failure::Input(pytext::os_error(&error, path.as_bytes()))),
    }
}

/// `out.parent.mkdir(parents=True, exist_ok=True)` and `out.write_text(text)`, for `inventory --out`.
pub fn write_out(out: &[u8], text: &str) -> R<()> {
    let out = PyPath::new(out);
    make_parents(&parent_of(&out))?;
    std::fs::write(out.to_path_buf(), text)
        .map_err(|error| Failure::Input(pytext::os_error(&error, out.as_bytes())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_embedded_schemas_are_the_repository_schemas() {
        let repository =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bench/schema");
        for (name, text) in SCHEMAS {
            let saved =
                std::fs::read_to_string(repository.join(format!("{name}.schema.json"))).unwrap();
            assert_eq!(text, saved, "{name}");
        }
    }

    #[test]
    fn canonical_bytes_match_python() {
        let value = pyvalue::loads(r#"{"b": [1, 2.5, "é"], "a": {"z": null, "y": true}}"#).unwrap();
        assert_eq!(
            canonical(&value),
            "{\"a\":{\"y\":true,\"z\":null},\"b\":[1,2.5,\"é\"]}"
        );
    }

    #[test]
    fn item_numbers_read_as_int_reads_them() {
        assert_eq!(item_number("7").unwrap(), (false, "7".into()));
        assert_eq!(item_number("007").unwrap(), (false, "7".into()));
        assert_eq!(item_number("1\n").unwrap(), (false, "1".into()));
        assert_eq!(item_number("-1").unwrap(), (true, "1".into()));
        assert!(matches!(item_number("x"), Err(Failure::Crash(_))));
    }
}
