//! Reading a `behavior-contract/1` corpus and the decision history that records each case's contract digest.

use crate::b64;
use crate::json::{self, text};
use bench_tools::pyjson::{Object, Value};
use bench_tools::{pytext, sha256};
use std::collections::HashSet;
use std::path::Path;

pub const GRADING_OUTCOMES: [&str; 6] = [
    "family-outcome",
    "credit-fact",
    "unresolved-state",
    "candidate-state",
    "prepared-review",
    "prepared-family",
];
const STRUCTURED: [&str; 7] = [
    "pins", "tokens", "clocks", "identity", "sections", "lines", "members",
];
const UNSTRUCTURED: [&str; 5] = ["archives", "digests", "manifests", "references", "volatile"];

pub fn field<'a>(value: &'a Value, key: &str) -> Result<&'a Value, String> {
    value
        .as_object()
        .ok_or_else(|| format!("{} has no field {key:?}", value.type_name()))?
        .get(key)
        .ok_or_else(|| format!("missing field {key:?}"))
}

pub fn has(value: &Value, key: &str) -> bool {
    value
        .as_object()
        .is_some_and(|object| object.get(key).is_some())
}

pub fn string<'a>(value: &'a Value, key: &str) -> Result<&'a str, String> {
    field(value, key)?
        .as_str()
        .ok_or_else(|| format!("{key} is not a string"))
}

pub fn list<'a>(value: &'a Value, key: &str) -> Result<&'a [Value], String> {
    match value.as_object().and_then(|object| object.get(key)) {
        None => Ok(&[]),
        Some(found) => found
            .as_array()
            .ok_or_else(|| format!("{key} is not a list")),
    }
}

pub fn required_list<'a>(value: &'a Value, key: &str) -> Result<&'a [Value], String> {
    field(value, key)?
        .as_array()
        .ok_or_else(|| format!("{key} is not a list"))
}

pub fn keys(value: &Value) -> HashSet<&str> {
    value
        .as_object()
        .map(|object| object.iter().map(|(key, _)| key.as_str()).collect())
        .unwrap_or_default()
}

/// The steps of a case: its own steps, or the case itself as its one step.
pub fn steps(case: &Value) -> Vec<&Value> {
    match case
        .as_object()
        .and_then(|object| object.get("steps"))
        .and_then(Value::as_array)
    {
        Some(steps) => steps.iter().collect(),
        None => vec![case],
    }
}

/// The bytes of a base64 path that names an entry inside the case root.
pub fn relative(value: &Value) -> Result<Vec<u8>, String> {
    let raw = b64::decode(value.as_str().ok_or("a path_b64 is a string")?)?;
    if raw.is_empty()
        || raw.starts_with(b"/")
        || raw
            .split(|byte| *byte == b'/')
            .any(|part| matches!(part, b"" | b"." | b".."))
        || raw.contains(&0)
    {
        return Err(
            "filesystem paths must be nonempty relative byte paths without dot segments"
                .to_string(),
        );
    }
    Ok(raw)
}

pub struct Corpus {
    pub options: Value,
    pub cases: Vec<Value>,
}

impl Corpus {
    pub fn snapshot_modes(&self) -> bool {
        self.options
            .as_object()
            .and_then(|object| object.get("snapshot_modes"))
            .is_some_and(Value::truthy)
    }
}

fn read(path: &Path) -> Result<Value, String> {
    let raw = std::fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
    json::value(&raw).map_err(|error| format!("{}: {error}", path.display()))
}

pub fn load_decisions(path: &Path) -> Result<Object, String> {
    let decisions = read(path)?;
    let mut previous = Object::default();
    if has(&decisions, "previous") {
        let source = field(&decisions, "previous")?;
        let prior = path.with_file_name(string(source, "path")?);
        let bytes =
            std::fs::read(&prior).map_err(|error| format!("{}: {error}", prior.display()))?;
        if sha256::hex_digest(&bytes) != string(source, "sha256")? {
            return Err("previous decisions changed".to_string());
        }
        previous = if string(source, "format")? == "decisions/2" {
            load_decisions(&prior)?
        } else {
            match json::value(&bytes)? {
                Value::Object(object) => object,
                _ => return Err("previous decisions are not an object".to_string()),
            }
        };
    }
    let records = match field(&decisions, "records")? {
        Value::Object(object) => object.clone(),
        _ => return Err("records is not an object".to_string()),
    };
    if previous.iter().any(|(name, _)| records.get(name).is_none()) {
        return Err("decision history cannot remove cases".to_string());
    }
    for (name, record) in records.iter() {
        let payload: Object = record
            .as_object()
            .ok_or_else(|| format!("{name}: a record is an object"))?
            .iter()
            .filter(|(key, _)| key.as_str() != "decision")
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
        let decision = field(record, "decision")?;
        if pytext::strip(string(record, "reason")?).is_empty()
            || !decision.py_eq(&text(&format!(
                "decision-{}",
                json::digest(&Value::Object(payload))?
            )))
        {
            return Err(format!(
                "{name}: changed contract needs an explicit decision record"
            ));
        }
        if let Some(old) = previous.get(name).filter(|old| old.truthy())
            && !record.py_eq(old)
        {
            let superseded = field(old, "contract_sha256")?;
            let supersedes = record
                .as_object()
                .and_then(|object| object.get("supersedes"));
            if !supersedes.is_some_and(|value| value.py_eq(superseded))
                || decision.py_eq(field(old, "decision")?)
                || field(record, "reason")?.py_eq(field(old, "reason")?)
            {
                return Err(format!(
                    "{name}: new decision must supersede the previous digest with a new reason"
                ));
            }
        }
    }
    Ok(records)
}

pub fn load(path: &Path) -> Result<Corpus, String> {
    let corpus = read(path)?;
    if !field(&corpus, "format")?.py_eq(&text("behavior-contract/1")) {
        return Err("unsupported corpus format".to_string());
    }
    let allowed = [
        "format",
        "decisions",
        "cases",
        "snapshot_modes",
        "gh_stub_b64",
    ];
    if keys(&corpus).iter().any(|key| !allowed.contains(key)) {
        return Err("unknown corpus option".to_string());
    }
    let decisions = load_decisions(&path.with_file_name(string(&corpus, "decisions")?))?;
    let cases = field(&corpus, "cases")?
        .as_array()
        .ok_or("cases is not a list")?
        .to_vec();
    let mut ids = HashSet::new();
    for case in &cases {
        check_case(case)?;
        let id = string(case, "id")?;
        if !ids.insert(id.to_string()) {
            return Err("duplicate case id".to_string());
        }
        let decision = decisions
            .get(id)
            .ok_or_else(|| format!("{id}: no decision records this case"))?;
        if !field(decision, "reason")?.truthy()
            || !field(decision, "contract_sha256")?.py_eq(&text(&json::digest(case)?))
        {
            return Err(format!("{id}: changed contract needs an explicit decision"));
        }
    }
    if ids.len() != decisions.len() || decisions.iter().any(|(name, _)| !ids.contains(name)) {
        return Err(
            "corpus case ids differ from decisions; removal needs an explicit decision".to_string(),
        );
    }
    Ok(Corpus {
        options: corpus,
        cases,
    })
}

fn check_case(case: &Value) -> Result<(), String> {
    let id = string(case, "id")?;
    if let Some(steps) = case.as_object().and_then(|object| object.get("steps")) {
        let steps = steps.as_array().ok_or("steps is not a list")?;
        if ["argv", "stdin_b64", "expected", "baseline_defect"]
            .iter()
            .any(|key| has(case, key))
            || !steps.iter().any(|step| has(step, "argv"))
            || steps
                .iter()
                .any(|step| has(step, "baseline_defect") && !has(step, "argv"))
        {
            return Err(format!(
                "{id}: a step case keeps its commands and expectations in its steps"
            ));
        }
    }
    if has(case, "structured") {
        check_structured(case)?;
    }
    let mut all = vec![case];
    all.extend(list(case, "steps")?);
    for step in all {
        if has(step, "file_size_limit")
            && (!has(step, "argv")
                || !matches!(field(step, "file_size_limit")?, Value::Int(digits) if !digits.starts_with('-')))
        {
            return Err(format!("{id}: file_size_limit is a command's byte count"));
        }
        if has(step, "fail_write_b64") {
            let target = field(step, "fail_write_b64")?;
            if !has(step, "argv") || target.as_str().is_none() {
                return Err(format!("{id}: fail_write_b64 is a command's relative path"));
            }
            relative(target)?;
        }
        if has(step, "shell") && names_python(string(step, "shell")?) {
            return Err(format!(
                "{id}: a shell step must not run a Python interpreter"
            ));
        }
        let outcomes = match step.as_object().and_then(|object| object.get("expected")) {
            Some(expected) => list(expected, "outcomes")?,
            None => &[],
        };
        for outcome in outcomes {
            let fields = keys(outcome);
            let path_form: HashSet<&str> = ["kind", "path", "pointer", "value"].into();
            let stream_form: HashSet<&str> = ["kind", "stream", "pointer", "value"].into();
            let kind_known = field(outcome, "kind")
                .ok()
                .and_then(Value::as_str)
                .is_some_and(|kind| GRADING_OUTCOMES.contains(&kind));
            let stream_known = !has(outcome, "stream")
                || ["stdout", "stderr"]
                    .iter()
                    .any(|name| field(outcome, "stream").is_ok_and(|s| s.py_eq(&text(name))));
            if (fields != path_form && fields != stream_form) || !kind_known || !stream_known {
                return Err(format!(
                    "{id}: an outcome states its kind ({}), path or stream, pointer and value",
                    GRADING_OUTCOMES.join(", ")
                ));
            }
        }
    }
    Ok(())
}

/// Whether a word of a shell script names a Python interpreter, by name or path: `python`, `python3`,
/// `/usr/bin/python3.12`.
fn names_python(script: &str) -> bool {
    script
        .split(|c: char| c.is_whitespace() || ";&|()`\"'<>=".contains(c))
        .filter_map(|word| word.rsplit('/').next())
        .any(|name| {
            name.strip_prefix("python")
                .is_some_and(|rest| rest.chars().all(|c| c.is_ascii_digit() || c == '.'))
        })
}

fn nonempty(value: Option<&Value>) -> bool {
    value.is_some_and(Value::truthy)
}

fn fullmatch_lowercase(value: &Value) -> bool {
    let shown = value.py_str();
    !shown.is_empty() && shown.bytes().all(|byte| byte.is_ascii_lowercase())
}

pub fn check_structured(case: &Value) -> Result<(), String> {
    let id = string(case, "id")?;
    let spec = field(case, "structured")?;
    if keys(spec).iter().any(|key| !STRUCTURED.contains(key))
        || UNSTRUCTURED.iter().any(|key| has(case, key))
    {
        return Err(format!(
            "{id}: a structured comparison declares only {}",
            STRUCTURED.join(", ")
        ));
    }
    for rule in list(spec, "pins")? {
        let fields = keys(rule);
        if fields
            .iter()
            .any(|key| !["glob", "base", "map", "pointer", "path"].contains(key))
            || fields.contains("pointer") != fields.contains("path")
            || fields.contains("map") && fields.contains("pointer")
        {
            return Err(format!(
                "{id}: a pin rule names a glob and at most one of map, or pointer with path"
            ));
        }
    }
    for rule in list(spec, "identity")? {
        let object = rule.as_object();
        if !nonempty(object.and_then(|o| o.get("label")))
            || !nonempty(object.and_then(|o| o.get("pointers")))
        {
            return Err(format!(
                "{id}: a runner identity rule names its pointers and its decided label"
            ));
        }
    }
    for rule in list(spec, "sections")? {
        let heading = rule
            .as_object()
            .and_then(|object| object.get("heading"))
            .and_then(Value::as_str)
            .unwrap_or("");
        let hashes = heading.trim_end_matches(' ');
        if !(heading.len() == hashes.len() + 1
            && !hashes.is_empty()
            && hashes.bytes().all(|byte| byte == b'#'))
        {
            return Err(format!(
                "{id}: a section rule names a Markdown heading prefix such as '### '"
            ));
        }
    }
    for rule in list(spec, "members")? {
        if keys(rule) != HashSet::from(["glob", "pointers"]) || !field(rule, "pointers")?.truthy() {
            return Err(format!(
                "{id}: a members rule names a glob and the pointers of the objects to order"
            ));
        }
    }
    for rule in list(spec, "lines")? {
        if keys(rule) != HashSet::from(["glob", "start"])
            || !field(rule, "start")?
                .as_str()
                .is_some_and(|start| !start.is_empty())
        {
            return Err(format!(
                "{id}: a lines rule names a glob and the start of the lines to order"
            ));
        }
    }
    for rule in list(spec, "tokens")? {
        if !token_rule(rule) {
            return Err(format!(
                "{id}: a token rule names a lowercase prefix, such as blind, its sites, each a glob with the JSON \
                 pointers that hold generated tokens (and keys: true when the keys of the objects there are the \
                 tokens), and its references, each a glob with pointers (and keys: true for object keys), a glob \
                 with line_starts, a glob with names: true, a stream, or a glob alone for whole files, and at most \
                 ordinals: sites"
            ));
        }
    }
    Ok(())
}

fn is_true(value: Option<&Value>) -> bool {
    matches!(value, None | Some(Value::Bool(true)))
}

fn token_rule(rule: &Value) -> bool {
    let Some(object) = rule.as_object() else {
        return false;
    };
    let mut fields = keys(rule);
    fields.remove("ordinals");
    if fields != HashSet::from(["prefix", "sites", "references"]) {
        return false;
    }
    if object
        .get("ordinals")
        .is_some_and(|ordinals| !ordinals.py_eq(&text("sites")))
        || !object.get("prefix").is_some_and(fullmatch_lowercase)
    {
        return false;
    }
    let Some(sites) = object.get("sites") else {
        return false;
    };
    if !sites.truthy() {
        return false;
    }
    let Ok(sites) = bench_tools::pyvalue::iterate(sites) else {
        return false;
    };
    let sites_valid = sites.iter().all(|site| {
        let mut fields = keys(site);
        fields.remove("keys");
        fields == HashSet::from(["glob", "pointers"])
            && nonempty(site.as_object().and_then(|o| o.get("pointers")))
            && is_true(site.as_object().and_then(|o| o.get("keys")))
    });
    sites_valid
        && object
            .get("references")
            .and_then(Value::as_array)
            .is_some_and(|references| references.iter().all(token_reference))
}

fn token_reference(reference: &Value) -> bool {
    let Some(object) = reference.as_object() else {
        return false;
    };
    let fields = keys(reference);
    let set = |names: &[&'static str]| fields == names.iter().copied().collect::<HashSet<&str>>();
    (set(&["stream"])
        && ["stdout", "stderr"].iter().any(|stream| {
            object
                .get("stream")
                .is_some_and(|value| value.py_eq(&text(stream)))
        }))
        || (set(&["glob", "names"]) && matches!(object.get("names"), Some(Value::Bool(true))))
        || ((set(&["glob", "pointers"]) || set(&["glob", "pointers", "keys"]))
            && nonempty(object.get("pointers"))
            && is_true(object.get("keys")))
        || (set(&["glob", "line_starts"])
            && nonempty(object.get("line_starts"))
            && object
                .get("line_starts")
                .and_then(Value::as_array)
                .is_some_and(|starts| {
                    starts
                        .iter()
                        .all(|start| start.as_str().is_some_and(|start| !start.is_empty()))
                }))
        || set(&["glob"])
}
