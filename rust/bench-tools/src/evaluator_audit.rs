//! The evaluator audit's whole-cohort checks, seeded draw, claim lists, comparison and conclusion, ported from
//! `bench/tools/evaluator_audit.py`, without model calls.
//!
//! Every record of a round is written once under `<current>/audit/<seed>/`, except a second assessment, which is
//! numbered under `second/` like a current assessment; each operation holds the record lock.

use crate::calibration::{Map, at, each, is, object, sorted, text};
use crate::claim_grading;
use crate::claims;
use crate::current_grading::{self, Documents, Failure, R, Root};
use crate::grade;
use crate::grading_validation;
use crate::pre_existing;
use crate::pyfs;
use crate::pyjson::{Object, Value};
use crate::pytext::{self, PyPath};
use crate::pyunicode;
use crate::pyvalue;
use std::cmp::Ordering;
use std::fmt::Write as _;

pub const PROGRAM: &str = "bench-tools evaluator-audit";
const CREDITS: [(&str, &str); 3] = [
    ("yes", "caught"),
    ("cannot-tell", "unresolved"),
    ("no", "missed"),
];
const CREDIT_STRATA: [(&str, &str); 3] = [
    ("caught", "caught"),
    ("missed", "missed"),
    ("unresolved", "unresolved-credit"),
];
const IMPACT_STRATA: [&str; 2] = ["serious", "other-material"];
const CLAIM_STRATA: [(&str, &str); 7] = [
    ("problem", "problem"),
    ("refuted", "refuted"),
    ("unproven", "unproven"),
    ("suggestion", "suggestion"),
    ("minor-defect", "minor-defect"),
    ("outside-this-change", "outside-this-change"),
    ("unresolved", "unresolved-claim"),
];
pub const PRE_EXISTING_STRATA: [(&str, &str); 3] = [
    ("raised", "pre-existing-raised"),
    ("not-raised", "pre-existing-not-raised"),
    ("unresolved", "pre-existing-unresolved"),
];
/// Each stratum and the kind of unit it draws, in `UNITS`' order.
pub const UNITS: [(&str, &str); 16] = [
    ("caught", "family-review"),
    ("missed", "family-review"),
    ("unresolved-credit", "family-review"),
    ("serious", "family-review"),
    ("other-material", "family-review"),
    ("problem", "claim"),
    ("refuted", "claim"),
    ("unproven", "claim"),
    ("suggestion", "claim"),
    ("minor-defect", "claim"),
    ("outside-this-change", "claim"),
    ("unresolved-claim", "claim"),
    ("control-silence", "control-review"),
    ("pre-existing-raised", "pre-existing-review"),
    ("pre-existing-not-raised", "pre-existing-review"),
    ("pre-existing-unresolved", "pre-existing-review"),
];
const ANSWERS: [&str; 4] = ["true", "this_change", "promised", "delivered"];
const FINDINGS: [&str; 3] = ["first-error", "first-correct", "undetermined"];
const CHECKS: [&str; 4] = [
    "saved-credit",
    "canonical-claims",
    "current-and-covered",
    "pre-existing-results",
];
const MISSING_FAULT: &str = "Missing pre-existing fault:";

/// What one operation ends with: a line it printed, or the problems it found.
pub enum Done {
    Printed(String),
    Problems(Vec<String>),
}

/// The arguments the operations read.
pub struct Arguments<'a> {
    pub root: &'a [u8],
    pub current: &'a [u8],
    pub run: Option<&'a str>,
    pub target: Option<&'a str>,
    pub out: Option<&'a [u8]>,
    pub reconciliation: Option<&'a [u8]>,
    /// The checkout whose prepared inputs a second grader's workspace is checked against.
    pub checkout: &'a [u8],
    pub work: Option<&'a [u8]>,
    pub key: Option<&'a [u8]>,
    pub assessor: Option<&'a [u8]>,
}

fn lookup<'a>(table: &'a [(&str, &'a str)], key: &Value) -> R<&'a str> {
    table
        .iter()
        .find(|(known, _)| is(key, known))
        .map(|(_, value)| *value)
        .ok_or_else(|| pyvalue::key_error(key).into())
}

fn missing(key: &Value) -> Failure {
    pyvalue::key_error(key).into()
}

fn int(number: usize) -> Value {
    Value::Int(number.to_string())
}

/// `FileExistsError` as `main` reports it: an audit record is written once.
fn written_once(path: &PyPath) -> Failure {
    Failure::Inconsistent(
        [
            path.as_bytes(),
            b" is already saved; an audit record is written once",
        ]
        .concat(),
    )
}

fn os_failure(error: &std::io::Error, path: &PyPath) -> Failure {
    if error.kind() == std::io::ErrorKind::AlreadyExists {
        return written_once(path);
    }
    Failure::Input(pytext::os_error(error, path.as_bytes()))
}

/// `Path.mkdir(parents=True, exist_ok=True)`, whose `FileExistsError` names the path that is not a directory.
fn make_dirs(path: &PyPath) -> R<()> {
    let target = path.to_path_buf();
    match std::fs::create_dir(&target) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let parent = current_grading::parent_of(path);
            if parent == *path {
                return Err(os_failure(&error, path));
            }
            make_dirs(&parent)?;
            match std::fs::create_dir(&target) {
                Err(error) if !target.is_dir() => Err(os_failure(&error, path)),
                _ => Ok(()),
            }
        }
        Err(_) if target.is_dir() => Ok(()),
        Err(error) => Err(os_failure(&error, path)),
    }
}

/// `current_grading.record_lock(root)`, with the errors Python's `mkdir` and `open` raise.
pub fn lock(root: &[u8]) -> R<std::fs::File> {
    let path = PyPath::new(&pyfs::resolve(root)).join(b".cache/current-record.lock");
    make_dirs(&current_grading::parent_of(&path))?;
    current_grading::record_lock(root).map_err(|error| os_failure(&error, &path))
}

fn save_new(path: &PyPath, value: &Value) -> R<()> {
    make_dirs(&current_grading::parent_of(path))?;
    write_new(
        path,
        format!("{}\n", current_grading::dumps(value)).as_bytes(),
    )
}

fn write_new(path: &PyPath, bytes: &[u8]) -> R<()> {
    use std::io::Write;
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path.to_path_buf())
        .and_then(|mut handle| handle.write_all(bytes))
        .map_err(|error| os_failure(&error, path))
}

/// `grade.replace_json(path, value)`.
fn replace_json(path: &PyPath, value: &Value) -> R<()> {
    let mut temporary = path.as_bytes().to_vec();
    temporary.extend_from_slice(b".tmp");
    let temporary = PyPath::new(&temporary);
    std::fs::write(
        temporary.to_path_buf(),
        format!("{}\n", current_grading::dumps(value)),
    )
    .map_err(|error| os_failure(&error, &temporary))?;
    std::fs::rename(temporary.to_path_buf(), path.to_path_buf()).map_err(|error| {
        let message = pytext::os_error(&error, b"");
        Failure::Input(format!(
            "{}: {} -> {}",
            message.strip_suffix(": ''").unwrap_or(&message),
            temporary.repr(),
            path.repr()
        ))
    })
}

fn bytes_of(value: &Value) -> R<&[u8]> {
    match value {
        Value::Str(text) => Ok(text.as_bytes()),
        other => Err(Failure::Crash(format!(
            "TypeError: expected str, bytes or os.PathLike object, not {}",
            other.type_name()
        ))),
    }
}

/// `Path(value).name`.
fn name_of(value: &Value) -> R<String> {
    let path = PyPath::new(bytes_of(value)?);
    let bytes = path.as_bytes();
    let name = match bytes.iter().rposition(|byte| *byte == b'/') {
        Some(at) => &bytes[at + 1..],
        None if bytes == b"." => b"",
        None => bytes,
    };
    Ok(String::from_utf8_lossy(name).into_owned())
}

/// `root / value`, for a path a record holds.
fn under(root: &Root, value: &Value) -> R<PyPath> {
    Ok(root.path.join(bytes_of(value)?))
}

fn now() -> Value {
    text(&crate::normalize_review::utc_now())
}

/// `{key: row for row in rows}` by one field, a repeated key keeping its first place and its last row.
fn keyed(rows: &Value, field: &str) -> R<Map<Value>> {
    let mut found = Map::default();
    for row in each(rows)?.iter() {
        let key = at(row, field)?;
        pyvalue::hashable(key)?;
        found.insert(key.clone(), row.clone());
    }
    Ok(found)
}

fn pair(left: &Value, right: &Value) -> Value {
    Value::Array(vec![left.clone(), right.clone()])
}

/// A Python tuple used as a key, which hashes when each of its parts does.
fn tuple_key(parts: &[&Value]) -> R<Value> {
    for part in parts {
        pyvalue::hashable(part)?;
    }
    Ok(Value::Array(
        parts.iter().map(|&part| part.clone()).collect(),
    ))
}

/// `Counter(keys)` of keys already hashed, in first-seen order.
fn tally(keys: impl IntoIterator<Item = R<Value>>) -> R<Map<usize>> {
    let mut found = Map::default();
    for key in keys {
        *found.slot(key?, || 0) += 1;
    }
    Ok(found)
}

/// `Counter(values)`, in first-seen order.
fn counter(values: Vec<Value>) -> R<Map<usize>> {
    tally(values.into_iter().map(|value| {
        pyvalue::hashable(&value)?;
        Ok(value)
    }))
}

/// `sorted(mapping.items())` of keys that are unique, so only the keys are compared.
fn sorted_items<T>(mut items: Vec<(Value, T)>) -> R<Vec<(Value, T)>> {
    let mut failed = None;
    items.sort_by(|left, right| {
        pyvalue::compare(&left.0, &right.0).unwrap_or_else(|error| {
            failed.get_or_insert(error);
            Ordering::Equal
        })
    });
    match failed {
        Some(error) => Err(error.into()),
        None => Ok(items),
    }
}

/// The text a JSON object key takes from a Python dict key.
fn json_key(key: &Value) -> R<String> {
    match key {
        Value::Str(text) => Ok(text.clone()),
        Value::Null => Ok("null".to_string()),
        Value::Bool(flag) => Ok(if *flag { "true" } else { "false" }.to_string()),
        Value::Int(digits) => Ok(digits.clone()),
        Value::Float(_) => Ok(key.py_repr()),
        other => Err(Failure::Crash(format!(
            "TypeError: unhashable type: '{}'",
            other.type_name()
        ))),
    }
}

fn mapping<T>(map: Map<T>, value: impl Fn(T) -> R<Value>) -> R<Value> {
    let mut found = Object::default();
    for (key, item) in map.0 {
        found.insert(json_key(&key)?, value(item)?);
    }
    Ok(Value::Object(found))
}

fn audit_dir(root: &Root, current: &[u8]) -> R<PyPath> {
    let directory = root.local(current)?;
    let audits = current_grading::read_json(&directory.join(b"audits.json"))?;
    let seed = at(at(&audits, "plan")?, "seed")?;
    let Value::Str(name) = seed else {
        return Err(Failure::Crash(
            "TypeError: expected string or bytes-like object, got '".to_string()
                + seed.type_name()
                + "'",
        ));
    };
    let named = !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
    if !named {
        return Err(Failure::Inconsistent(
            format!(
                "the audit seed {} must also name its directory: letters, digits, '.', '_' and '-'",
                seed.py_repr()
            )
            .into_bytes(),
        ));
    }
    Ok(directory.join(b"audit").join(name.as_bytes()))
}

/// `values[:size]` for a Python integer.
fn head(values: &[Value], size: &Value) -> R<Vec<Value>> {
    let Value::Int(digits) = size else {
        return Err(Failure::Crash(format!(
            "TypeError: slice indices must be integers or None or have an __index__ method, not {}",
            size.type_name()
        )));
    };
    let length = values.len() as i128;
    let wanted: i128 = digits.parse().unwrap_or(if digits.starts_with('-') {
        i128::MIN
    } else {
        i128::MAX
    });
    let end = if wanted < 0 {
        (length + wanted).max(0)
    } else {
        wanted.min(length)
    };
    Ok(values[..end as usize].to_vec())
}

fn strata_of(plan: &Value, populations: &Map<Vec<Value>>) -> R<Vec<Value>> {
    let mut sizes = Map::default();
    for entry in each(at(at(plan, "sample")?, "sizes")?)?.iter() {
        let stratum = at(entry, "stratum")?;
        pyvalue::hashable(stratum)?;
        sizes.insert(stratum.clone(), at(entry, "size")?.clone());
    }
    let mut strata = Vec::new();
    for stratum in each(at(plan, "strata")?)?.iter() {
        let name = at(stratum, "id")?;
        let mut ordered: Vec<(String, Value)> = Vec::new();
        for identifier in populations.get(name).map_or(&[][..], Vec::as_slice) {
            let seed = at(plan, "seed")?.py_str();
            let key = format!("{seed}:{}:{}", name.py_str(), identifier.py_str());
            ordered.push((
                crate::sha256::hex_digest(key.as_bytes()),
                identifier.clone(),
            ));
        }
        let mut failed = None;
        ordered.sort_by(|left, right| {
            left.0.cmp(&right.0).then_with(|| {
                pyvalue::compare(&left.1, &right.1).unwrap_or_else(|error| {
                    failed.get_or_insert(error);
                    Ordering::Equal
                })
            })
        });
        if let Some(error) = failed {
            return Err(error.into());
        }
        let ordered: Vec<Value> = ordered.into_iter().map(|(_, value)| value).collect();
        let size = sizes.get(name).ok_or_else(|| missing(name))?;
        let count = if is(size, "all") {
            int(ordered.len())
        } else {
            size.clone()
        };
        let unit = at(stratum, "unit")?.clone();
        let state = if ordered.is_empty() {
            "empty"
        } else if pyvalue::compare(&int(ordered.len()), &count)? == Ordering::Greater {
            "sampled"
        } else {
            "whole"
        };
        let population = sorted(ordered.clone())?;
        let drawn = if is(size, "all") {
            ordered
        } else {
            head(&ordered, &count)?
        };
        strata.push(object(vec![
            ("id", name.clone()),
            ("unit", unit),
            ("size", size.clone()),
            ("state", text(state)),
            ("population", Value::Array(population)),
            ("drawn", Value::Array(drawn)),
        ]));
    }
    Ok(strata)
}

fn drawn_units(strata: &[Value]) -> R<Map<Vec<Value>>> {
    let mut drawn: Map<Vec<Value>> = Map::default();
    for stratum in strata {
        for identifier in each(at(stratum, "drawn")?)?.iter() {
            drawn
                .slot(identifier.clone(), Vec::new)
                .push(at(stratum, "id")?.clone());
        }
    }
    Ok(drawn)
}

/// `dict(sorted(Counter(unit[field] for unit in units).items()))`.
fn coverage(units: &[Value], field: &str) -> R<Value> {
    let counted = sorted_items(
        tally(units.iter().map(|unit| {
            let value = at(unit, field)?;
            pyvalue::hashable(value)?;
            Ok(value.clone())
        }))?
        .0,
    )?;
    mapping(Map(counted), |count| Ok(int(count)))
}

fn sample_of(plan: &Value, units: &Map<Value>) -> R<Object> {
    let mut populations: Map<Vec<Value>> = Map::default();
    for stratum in each(at(plan, "strata")?)?.iter() {
        let name = at(stratum, "id")?;
        let mut members = Vec::new();
        for (identifier, unit) in &units.0 {
            if pyvalue::contains(at(unit, "strata")?, name)? {
                members.push(identifier.clone());
            }
        }
        pyvalue::hashable(name)?;
        populations.insert(name.clone(), members);
    }
    let strata = strata_of(plan, &populations)?;
    let mut sampled = Vec::new();
    for (identifier, names) in sorted_items(drawn_units(&strata)?.0)? {
        let unit = units.get(&identifier).ok_or_else(|| missing(&identifier))?;
        let mut row = Object::default();
        row.insert("id".to_string(), identifier.clone());
        for (key, value) in unit.as_object().into_iter().flat_map(Object::iter) {
            row.insert(key.clone(), value.clone());
        }
        row.insert("strata".to_string(), Value::Array(names));
        sampled.push(Value::Object(row));
    }
    let batches = tally(
        sampled
            .iter()
            .map(|unit| tuple_key(&[at(unit, "run")?, at(unit, "target")?])),
    )?;
    let tasks = coverage(&sampled, "target")?;
    let configurations = coverage(&sampled, "configuration")?;
    let mut listed = Vec::new();
    for (key, count) in sorted_items(batches.0)? {
        let Value::Array(parts) = key else {
            unreachable!("each batch key is a run and a target");
        };
        listed.push(object(vec![
            ("run", parts[0].clone()),
            ("target", parts[1].clone()),
            ("units", int(count)),
        ]));
    }
    let mut found = Object::default();
    found.insert("strata".to_string(), Value::Array(strata));
    found.insert("units".to_string(), Value::Array(sampled));
    found.insert(
        "coverage".to_string(),
        object(vec![("tasks", tasks), ("configurations", configurations)]),
    );
    found.insert("batches".to_string(), Value::Array(listed));
    Ok(found)
}

fn grades_path(root: &Root, sample: &Value) -> R<PyPath> {
    under(root, at(at(sample, "grades")?, "path")?)
}

fn drawn_sample(root: &Root, current: &[u8]) -> R<(PyPath, Value)> {
    let directory = audit_dir(root, current)?;
    let sample = current_grading::read_json(&directory.join(b"sample.json"))?;
    let grades = grades_path(root, &sample)?;
    if !Value::Str(current_grading::file_hash(&grades)?)
        .py_eq(at(at(&sample, "grades")?, "sha256")?)
    {
        return Err(Failure::Inconsistent(
            b"grades.json changed since the sample was drawn, so its first assessments are no longer the grades \
              on record; correct grades after the conclusion, or draw a fresh sample under a new recorded seed"
                .to_vec(),
        ));
    }
    let audits = current_grading::read_json(&root.local(current)?.join(b"audits.json"))?;
    let plan = at(&audits, "plan")?;
    let units = units_of(
        &current_grading::inventory(root)?,
        at(&current_grading::read_json(&grades)?, "batches")?,
        at(&sample, "tasks")?,
    )?;
    for (field, value) in sample_of(plan, &units)?.iter() {
        if !pyvalue::get(&sample, field)?.py_eq(value) {
            return Err(Failure::Inconsistent(
                b"sample.json is not the draw that the pinned grades, the task facts it pins, the declared sizes and \
                  the seed give"
                    .to_vec(),
            ));
        }
    }
    Ok((directory, sample))
}

fn same_batch(batch: &Value, run: &str, target: &str) -> R<bool> {
    let (left, right) = (at(batch, "run")?, at(batch, "target")?);
    Ok(is(left, run) && is(right, target))
}

fn sampled_grade(sample: &Value, root: &Root, run: &str, target: &str) -> R<Value> {
    let mut held = false;
    for batch in each(at(sample, "batches")?)?.iter() {
        if same_batch(batch, run, target)? {
            held = true;
            break;
        }
    }
    if !held {
        return Err(Failure::Inconsistent(
            format!("{run}/{target} holds no sampled unit").into_bytes(),
        ));
    }
    let grades = current_grading::read_json(&grades_path(root, sample)?)?;
    for batch in each(at(&grades, "batches")?)?.iter() {
        if same_batch(batch, run, target)? {
            return Ok(batch.clone());
        }
    }
    Err(Failure::Crash("StopIteration".to_string()))
}

fn says_what(claim: &Value, family: &Value) -> R<Value> {
    for entry in each(at(claim, "known_problems")?)?.iter() {
        if at(entry, "family_id")?.py_eq(family) {
            return Ok(at(entry, "says_what")?.clone());
        }
    }
    Ok(text("no"))
}

fn credit(claims: &Value, family: &Value) -> R<(&'static str, Vec<Value>)> {
    let mut answers: Map<Value> = Map::default();
    for claim in each(claims)?.iter() {
        let identifier = at(claim, "id")?;
        pyvalue::hashable(identifier)?;
        answers.insert(identifier.clone(), says_what(claim, family)?);
    }
    let given: Vec<&Value> = answers.0.iter().map(|(_, answer)| answer).collect();
    let fact = claim_grading::item_fact(&given);
    let result = CREDITS
        .iter()
        .find(|(known, _)| *known == fact)
        .map(|(_, credit)| *credit)
        .expect("item_fact answers yes, cannot-tell or no");
    let ids = answers
        .0
        .iter()
        .filter(|(_, answer)| fact != "no" && is(answer, fact))
        .map(|(identifier, _)| identifier.clone())
        .collect();
    Ok((result, ids))
}

fn control_result(claims: &Value) -> R<(&'static str, Vec<Value>)> {
    for (result, outcomes) in [
        ("alarmed", &["refuted", "unproven"][..]),
        ("unresolved", &["unresolved"][..]),
    ] {
        let mut found = Vec::new();
        for claim in each(claims)?.iter() {
            let outcome = at(claim, "outcome")?;
            if outcomes.iter().any(|known| is(outcome, known)) {
                found.push(at(claim, "id")?.clone());
            }
        }
        if !found.is_empty() {
            return Ok((result, found));
        }
    }
    Ok(("silent", Vec::new()))
}

fn merged(base: Value, extra: Value) -> Value {
    let (Value::Object(mut base), Value::Object(extra)) = (base, extra) else {
        unreachable!("both parts are objects");
    };
    for (key, value) in extra.iter() {
        base.insert(key.clone(), value.clone());
    }
    Value::Object(base)
}

fn pre_existing_result(claims: &Value, problem: &Value) -> R<(Value, Vec<Value>)> {
    let mut saved = Vec::new();
    for claim in each(claims)?.iter() {
        saved.push(merged(
            object(vec![("id", at(claim, "id")?.clone())]),
            pre_existing::saved_claim(claim)?,
        ));
    }
    let found = pre_existing::assessment(&object(vec![("id", problem.clone())]), &saved, true)?;
    Ok((
        at(&found, "outcome")?.clone(),
        each(at(&found, "claim_ids")?)?.into_owned(),
    ))
}

fn pre_existing_failures(
    place: &str,
    review: &Value,
    admitted: bool,
    catalog: &[Value],
) -> R<Vec<String>> {
    if !pyvalue::has(review, "pre_existing_problems")? {
        return Ok(catalog
            .iter()
            .map(|problem| {
                format!(
                    "{place}#pre-existing:{}: the review has no saved result for this pre-existing problem",
                    problem.py_str()
                )
            })
            .collect());
    }
    let mut claims = Vec::new();
    for claim in each(at(review, "claims")?)?.iter() {
        claims.push(merged(
            current_grading::verdict_claim(claim)?,
            pre_existing::saved_claim(claim)?,
        ));
    }
    let rows_value = at(review, "pre_existing_problems")?;
    let mut saved: Map<Value> = Map::default();
    let mut identifiers = Vec::new();
    for row in each(rows_value)?.iter() {
        let identifier = at(row, "problem_id")?;
        pyvalue::hashable(identifier)?;
        saved.insert(identifier.clone(), at(row, "outcome")?.clone());
        identifiers.push(identifier.clone());
    }
    let rows = counter(identifiers)?;
    let mut found = Vec::new();
    for problem in catalog {
        let derived = at(
            &pre_existing::assessment(&object(vec![("id", problem.clone())]), &claims, admitted)?,
            "outcome",
        )?
        .clone();
        let count = rows.get(problem).copied().unwrap_or(0);
        if count != 1 {
            found.push(format!(
                "{place}#pre-existing:{}: the review has {count} saved results for this pre-existing problem",
                problem.py_str()
            ));
        } else {
            let result = saved.get(problem).ok_or_else(|| missing(problem))?;
            if !result.py_eq(&derived) {
                found.push(format!(
                    "{place}#pre-existing:{}: the saved result is {} and its claims give {}",
                    problem.py_str(),
                    result.py_str(),
                    derived.py_str()
                ));
            }
        }
    }
    let extra: Vec<Value> = rows
        .0
        .iter()
        .map(|(problem, _)| problem)
        .filter(|problem| !catalog.iter().any(|known| known.py_eq(problem)))
        .cloned()
        .collect();
    for problem in sorted(extra)? {
        found.push(format!(
            "{place}#pre-existing:{}: a result is saved for a problem that is not an admitted pre-existing problem of \
             its task",
            problem.py_str()
        ));
    }
    let listed = Value::Array(catalog.to_vec());
    for claim in &claims {
        for problem in pre_existing::raising_problems(claim, &listed)? {
            found.push(format!("{place}#{}: {problem}", at(claim, "id")?.py_str()));
        }
    }
    Ok(found)
}

fn known_problems(reference: &Value) -> R<Vec<Value>> {
    let mut found = Vec::new();
    for family in each(at(reference, "families")?)?.iter() {
        if is(at(at(family, "eligibility")?, "state")?, "approved") {
            found.push(family.clone());
        }
    }
    Ok(found)
}

/// `grade.attempts_on(selected, run, target)`: the batch's selected attempts by the attempt id after the run.
fn attempts_on(selected: &Value, run: &Value, target: &Value) -> R<Map<Value>> {
    let mut cells = Vec::new();
    for cell in each(at(selected, "cells")?)?.iter() {
        let (left, right) = (at(cell, "run")?, at(cell, "target")?);
        if left.py_eq(run) && right.py_eq(target) {
            cells.push(at(cell, "id")?.clone());
        }
    }
    let mut found = Map::default();
    for attempt in each(at(selected, "attempts")?)?.iter() {
        if pyvalue::contains(&Value::Array(cells.clone()), at(attempt, "cell")?)? {
            let identifier = at(attempt, "id")?;
            let Value::Str(identifier) = identifier else {
                return Err(Failure::Crash(format!(
                    "AttributeError: '{}' object has no attribute 'split'",
                    identifier.type_name()
                )));
            };
            let Some((_, name)) = identifier.split_once('/') else {
                return Err(Failure::Crash(
                    "IndexError: list index out of range".to_string(),
                ));
            };
            found.insert(text(name), attempt.clone());
        }
    }
    Ok(found)
}

fn pre_existing_catalog(reference: &Value) -> R<Vec<Value>> {
    let none = Value::Array(Vec::new());
    let mut found = Vec::new();
    for problem in each(pyvalue::get_or(reference, "pre_existing_problems", &none)?)?.iter() {
        found.push(at(problem, "id")?.clone());
    }
    Ok(found)
}

fn cohort_problems(selected: &Value, documents: &Documents, root: &Root) -> R<Vec<String>> {
    let references = keyed(at(documents.get("reference"), "targets")?, "target")?;
    let cases = keyed(at(documents.get("claim"), "claims")?, "id")?;
    let decisions = claims::Decisions::new(at(documents.get("adjudication"), "decisions")?)
        .map_err(|claims::Failure(message)| Failure::Input(message))?;
    let mut graded: Map<Value> = Map::default();
    for batch in each(at(documents.get("grade"), "batches")?)?.iter() {
        graded.insert(
            tuple_key(&[at(batch, "run")?, at(batch, "target")?])?,
            batch.clone(),
        );
    }
    let (mut problems, mut ungraded) = (Vec::new(), Vec::new());
    for identity in each(at(selected, "batches")?)?.iter() {
        let (run, target) = (at(identity, "run")?, at(identity, "target")?);
        let state = current_grading::batch_state(root, identity, selected, documents)?.1;
        if state == "missing" {
            ungraded.push(format!("{}/{}", run.py_str(), target.py_str()));
            continue;
        }
        if state == "stale" {
            problems.push(format!(
                "{}/{}: the grade is stale",
                run.py_str(),
                target.py_str()
            ));
            continue;
        }
        let key = tuple_key(&[run, target])?;
        let batch = graded.get(&key).ok_or_else(|| missing(&key))?;
        let reviews = keyed(at(batch, "reviews")?, "attempt_id")?;
        let reference = references.get(target).ok_or_else(|| missing(target))?;
        let approved: Vec<Value> = known_problems(reference)?
            .iter()
            .map(|family| at(family, "id").cloned())
            .collect::<R<_>>()?;
        let catalog = pre_existing_catalog(reference)?;
        for (attempt, facts) in &attempts_on(selected, run, target)?.0 {
            let place = format!("{}/{}", name_of(run)?, attempt.py_str());
            let Some(review) = reviews.get(attempt) else {
                if at(facts, "review")?.truthy() {
                    problems.push(format!("{place}: the saved review has no grade"));
                }
                continue;
            };
            let admitted = is(at(at(facts, "admission")?, "state")?, "admitted");
            let mut rows: Map<Value> = Map::default();
            for row in each(at(review, "families")?)?.iter() {
                let family = at(row, "family_id")?;
                pyvalue::hashable(family)?;
                rows.insert(family.clone(), at(row, "outcome")?.clone());
            }
            for family in each(at(reference, "families")?)?.iter() {
                let family = at(family, "id")?;
                let derived = credit(at(review, "claims")?, family)?.0;
                let is_approved = approved.iter().any(|known| known.py_eq(family));
                match rows.get(family) {
                    None => problems.push(format!(
                        "{place}#{}: the review has no row for this known problem",
                        family.py_str()
                    )),
                    Some(saved) if !is_approved && !is(saved, "unresolved") => {
                        problems.push(format!(
                            "{place}#{}: the saved credit is {}, and a problem awaiting approval is unresolved",
                            family.py_str(),
                            saved.py_str()
                        ));
                    }
                    Some(saved) if is_approved && admitted && !is(saved, derived) => {
                        problems.push(format!(
                            "{place}#{}: the saved credit is {} and its claims give {derived}",
                            family.py_str(),
                            saved.py_str()
                        ));
                    }
                    Some(_) => {}
                }
            }
            problems.extend(pre_existing_failures(&place, review, admitted, &catalog)?);
            for claim in each(at(review, "claims")?)?.iter() {
                let canonical = at(claim, "canonical_id")?;
                if matches!(canonical, Value::Null) {
                    continue;
                }
                let unit = format!("{place}#{}", at(claim, "id")?.py_str());
                pyvalue::hashable(canonical)?;
                let case = match cases.get(canonical) {
                    Some(case) if at(case, "target")?.py_eq(target) => case,
                    _ => {
                        problems.push(format!(
                            "{unit}: {} is not a saved claim of its task",
                            canonical.py_str()
                        ));
                        continue;
                    }
                };
                let pinned = claims::pinned(case, &decisions)
                    .map_err(|claims::Failure(message)| Failure::Input(message))?;
                let family = at(&pinned, "family")?;
                let mut named = matches!(family, Value::Null);
                if !named {
                    for entry in each(at(claim, "known_problems")?)?.iter() {
                        if at(entry, "family_id")?.py_eq(family) {
                            named = true;
                            break;
                        }
                    }
                }
                let matches = named
                    && grading_validation::pinned_matches_v2(
                        &current_grading::verdict_claim(claim)?,
                        &pinned,
                    )?;
                if !matches {
                    let mut line = format!("{unit}: holds {}", at(claim, "outcome")?.py_str());
                    if !named {
                        let _ = write!(line, " with no entry for {}", family.py_str());
                    }
                    let _ = write!(
                        line,
                        ", and {} is saved as {}",
                        at(case, "id")?.py_str(),
                        at(&pinned, "outcome")?.py_str()
                    );
                    if family.truthy() {
                        let _ = write!(line, " for {}", family.py_str());
                    }
                    problems.push(line);
                }
            }
        }
    }
    if !ungraded.is_empty() {
        let mut line = format!(
            "{} selected batch(es) have no grade: {}",
            ungraded.len(),
            ungraded[..ungraded.len().min(5)].join(", ")
        );
        if ungraded.len() > 5 {
            line.push_str(" and others");
        }
        problems.push(line);
    }
    Ok(problems)
}

fn checked(root: &Root, current: &[u8]) -> R<(Value, Documents, Vec<String>)> {
    let (selected, documents) = current_grading::load_current(root, current, false)?;
    let mut other = Vec::new();
    for batch in each(at(documents.get("grade"), "batches")?)?.iter() {
        let verdicts = pyvalue::get(batch, "verdicts")?;
        if !(is(verdicts, grading_validation::CONTRACT_V2) || is(verdicts, pre_existing::CONTRACT))
        {
            other.push(format!(
                "{}/{}",
                at(batch, "run")?.py_str(),
                at(batch, "target")?.py_str()
            ));
        }
    }
    if !other.is_empty() {
        return Err(Failure::Inconsistent(
            format!(
                "{} batch(es) were not graded under {}: {}",
                other.len(),
                grading_validation::CONTRACT_V2,
                other[..other.len().min(5)].join(", ")
            )
            .into_bytes(),
        ));
    }
    let problems = cohort_problems(&selected, &documents, root)?;
    if problems.is_empty() {
        current_grading::validate_documents(root, &documents, &selected, true)?;
    }
    Ok((selected, documents, problems))
}

fn check(root: &Root, current: &[u8]) -> R<Done> {
    let (selected, _documents, problems) = checked(root, current)?;
    if !problems.is_empty() {
        return Ok(Done::Problems(problems));
    }
    Ok(Done::Printed(format!(
        "the {} whole-cohort checks passed for {} selected batch(es): {}",
        CHECKS.len(),
        pyvalue::length(at(&selected, "batches")?)?,
        CHECKS.join(", ")
    )))
}

fn labels(entries: &[Value]) -> R<Value> {
    let mut found = Object::default();
    for entry in entries {
        let key = json_key(at(entry, "id")?)?;
        let band = at(at(entry, "impact")?, "band")?;
        let label = if IMPACT_STRATA.iter().any(|known| is(band, known)) {
            band.clone()
        } else {
            Value::Null
        };
        found.insert(key, label);
    }
    Ok(Value::Object(found))
}

fn task_facts(documents: &Documents) -> R<Value> {
    let none = Value::Null;
    let mut found = Object::default();
    for reference in each(at(documents.get("reference"), "targets")?)?.iter() {
        let target = json_key(at(reference, "target")?)?;
        let mut facts = Object::default();
        facts.insert(
            "control".to_string(),
            at(at(reference, "control")?, "status")?.clone(),
        );
        facts.insert(
            "known_problems".to_string(),
            labels(&known_problems(reference)?)?,
        );
        if pyvalue::get_or(reference, "pre_existing_problems", &none)?.truthy() {
            facts.insert(
                "pre_existing_problems".to_string(),
                labels(&each(at(reference, "pre_existing_problems")?)?)?,
            );
        }
        found.insert(target, Value::Object(facts));
    }
    Ok(Value::Object(found))
}

/// The keys of a dict a record holds, as a `for` loop visits them.
fn keys(value: &Value) -> R<Vec<(Value, Value)>> {
    match value {
        Value::Object(object) => Ok(object
            .iter()
            .map(|(key, value)| (text(key), value.clone()))
            .collect()),
        other => Err(Failure::Crash(format!(
            "AttributeError: '{}' object has no attribute 'items'",
            other.type_name()
        ))),
    }
}

fn task_of<'a>(tasks: &'a Value, target: &Value) -> R<&'a Value> {
    match (tasks, target) {
        (Value::Object(object), Value::Str(name)) => {
            object.get(name).ok_or_else(|| missing(target))
        }
        (Value::Object(_), _) => Err(missing(target)),
        (other, _) => Err(Failure::Crash(format!(
            "TypeError: '{}' object is not subscriptable",
            other.type_name()
        ))),
    }
}

fn units_of(selected: &Value, batches: &Value, tasks: &Value) -> R<Map<Value>> {
    let cells = keyed(at(selected, "cells")?, "id")?;
    let attempts = keyed(at(selected, "attempts")?, "id")?;
    let empty = Value::Object(Object::default());
    let mut found: Vec<(Value, Value)> = Vec::new();
    for batch in each(batches)?.iter() {
        let task = task_of(tasks, at(batch, "target")?)?;
        for review in each(at(batch, "reviews")?)?.iter() {
            let attempt_id = at(review, "attempt_id")?;
            let attempt = text(&format!(
                "{}/{}",
                name_of(at(batch, "run")?)?,
                attempt_id.py_str()
            ));
            let facts = attempts.get(&attempt).ok_or_else(|| missing(&attempt))?;
            let cell = at(facts, "cell")?;
            pyvalue::hashable(cell)?;
            let configuration = at(
                cells.get(cell).ok_or_else(|| missing(cell))?,
                "configuration",
            )?;
            let common = |unit: &str, subject: Value, strata: Vec<Value>, first: Value| {
                object(vec![
                    ("unit", text(unit)),
                    ("run", at(batch, "run").cloned().unwrap_or(Value::Null)),
                    (
                        "target",
                        at(batch, "target").cloned().unwrap_or(Value::Null),
                    ),
                    ("attempt", attempt_id.clone()),
                    ("configuration", configuration.clone()),
                    ("subject", subject),
                    ("strata", Value::Array(strata)),
                    ("first", first),
                ])
            };
            let claims = at(review, "claims")?;
            let base = attempt.py_str();
            if is(at(at(facts, "admission")?, "state")?, "admitted") {
                for (family, label) in keys(at(task, "known_problems")?)? {
                    let (result, ids) = credit(claims, &family)?;
                    let mut strata = vec![text(lookup(&CREDIT_STRATA, &text(result))?)];
                    if label.truthy() {
                        strata.push(label.clone());
                    }
                    found.push((
                        text(&format!("{base}#{}", family.py_str())),
                        common(
                            "family-review",
                            family,
                            strata,
                            object(vec![
                                ("result", text(result)),
                                ("claim_ids", Value::Array(ids)),
                            ]),
                        ),
                    ));
                }
                for (problem, _label) in
                    keys(pyvalue::get_or(task, "pre_existing_problems", &empty)?)?
                {
                    let (result, ids) = pre_existing_result(claims, &problem)?;
                    let stratum = lookup(&PRE_EXISTING_STRATA, &result)?;
                    found.push((
                        text(&format!("{base}#pre-existing:{}", problem.py_str())),
                        common(
                            "pre-existing-review",
                            problem,
                            vec![text(stratum)],
                            object(vec![("result", result), ("claim_ids", Value::Array(ids))]),
                        ),
                    ));
                }
                if is(at(task, "control")?, "audited-clean") {
                    let (result, ids) = control_result(claims)?;
                    found.push((
                        attempt.clone(),
                        common(
                            "control-review",
                            Value::Null,
                            vec![text("control-silence")],
                            object(vec![
                                ("result", text(result)),
                                ("claim_ids", Value::Array(ids)),
                            ]),
                        ),
                    ));
                }
            }
            for claim in each(claims)?.iter() {
                if matches!(at(claim, "canonical_id")?, Value::Null) {
                    let outcome = at(claim, "outcome")?;
                    let anchor = at(claim, "anchor")?;
                    found.push((
                        text(&format!("{base}#{}", at(claim, "id")?.py_str())),
                        common(
                            "claim",
                            at(claim, "id")?.clone(),
                            vec![text(lookup(&CLAIM_STRATA, outcome)?)],
                            object(vec![
                                ("result", outcome.clone()),
                                ("item_id", at(anchor, "item_id")?.clone()),
                                ("quote", at(anchor, "quote")?.clone()),
                            ]),
                        ),
                    ));
                }
            }
        }
    }
    let identifiers = found
        .iter()
        .map(|(identifier, _)| identifier.clone())
        .collect();
    let repeated: Vec<Value> = counter(identifiers)?
        .0
        .into_iter()
        .filter(|(_, count)| *count > 1)
        .map(|(identifier, _)| identifier)
        .collect();
    let repeated = sorted(repeated)?;
    if !repeated.is_empty() {
        return Err(Failure::Inconsistent(
            format!(
                "two units share an id, so no sample is drawn: {}",
                pyvalue::join(", ", &repeated[..repeated.len().min(5)])?
            )
            .into_bytes(),
        ));
    }
    Ok(Map(found))
}

fn draw(root: &Root, current: &[u8]) -> R<Done> {
    let (selected, documents, problems) = checked(root, current)?;
    let plan = at(documents.get("audit"), "plan")?;
    if !is(at(at(plan, "sample")?, "state")?, "selected") {
        return Err(Failure::Inconsistent(
            b"the audit sample sizes await the human selection".to_vec(),
        ));
    }
    let mut unknown = Vec::new();
    let mut declared = Vec::new();
    for stratum in each(at(plan, "strata")?)?.iter() {
        let (name, unit) = (at(stratum, "id")?, at(stratum, "unit")?);
        pyvalue::hashable(name)?;
        declared.push(name.clone());
        let known = UNITS
            .iter()
            .find(|(known, _)| is(name, known))
            .is_some_and(|(_, kind)| is(unit, kind));
        if !known {
            unknown.push(text(&format!("{} ({})", name.py_str(), unit.py_str())));
        }
    }
    let unknown = sorted(unknown)?;
    if !unknown.is_empty() {
        return Err(Failure::Inconsistent(
            format!(
                "this tool does not know these declared strata, so no sample is drawn: {}",
                pyvalue::join(", ", &unknown)?
            )
            .into_bytes(),
        ));
    }
    let none = Value::Null;
    let mut any_pre_existing = false;
    for reference in each(at(documents.get("reference"), "targets")?)?.iter() {
        if pyvalue::get_or(reference, "pre_existing_problems", &none)?.truthy() {
            any_pre_existing = true;
            break;
        }
    }
    if any_pre_existing
        && !PRE_EXISTING_STRATA
            .iter()
            .all(|(_, stratum)| declared.iter().any(|name| is(name, stratum)))
    {
        return Err(Failure::Inconsistent(
            b"pre-existing-problem audit strata and acceptance criteria need owner approval before drawing".to_vec(),
        ));
    }
    let mut refusals = problems;
    if !refusals.is_empty() {
        refusals.push("the whole-cohort checks failed, so no sample is drawn".to_string());
    }
    let mut pending = Vec::new();
    for candidate in each(at(documents.get("candidate"), "candidates")?)?.iter() {
        if is(pyvalue::get(candidate, "kind")?, "pre-existing-problem")
            && matches!(at(candidate, "decision")?, Value::Null)
        {
            pending.push(text(&format!(
                "{} on {}",
                at(candidate, "id")?.py_str(),
                at(candidate, "target")?.py_str()
            )));
        }
    }
    let pending = sorted(pending)?;
    if !pending.is_empty() {
        refusals.push(format!(
            "{} pre-existing-problem candidate(s) await a ruling, so no sample is drawn: {}",
            pending.len(),
            pyvalue::join(", ", &pending)?
        ));
    }
    if !refusals.is_empty() {
        return Ok(Done::Problems(refusals));
    }
    let directory = root.local(current)?;
    let out = audit_dir(root, current)?;
    let tasks = task_facts(&documents)?;
    let sample = sample_of(
        plan,
        &units_of(&selected, at(documents.get("grade"), "batches")?, &tasks)?,
    )?;
    let mut record = Object::default();
    record.insert("contract".to_string(), text("evaluator-audit-sample/v2"));
    record.insert("drawn_at".to_string(), now());
    record.insert("seed".to_string(), at(plan, "seed")?.clone());
    record.insert(
        "dataset_hash".to_string(),
        at(
            &current_grading::coverage_status(&selected, &documents)?,
            "dataset_hash",
        )?
        .clone(),
    );
    record.insert(
        "grades".to_string(),
        root.pin_file(&directory.join(b"grades.json"))?,
    );
    let mut checks = Object::default();
    for name in CHECKS {
        checks.insert(name.to_string(), text("passed"));
    }
    record.insert("checks".to_string(), Value::Object(checks));
    record.insert("tasks".to_string(), tasks);
    for (key, value) in sample.iter() {
        record.insert(key.clone(), value.clone());
    }
    let record = Value::Object(record);
    save_new(&out.join(b"sample.json"), &record)?;
    let mut parts = Vec::new();
    for stratum in each(at(&record, "strata")?)?.iter() {
        parts.push(format!(
            "{} {}/{} {}",
            at(stratum, "id")?.py_str(),
            pyvalue::length(at(stratum, "drawn")?)?,
            pyvalue::length(at(stratum, "population")?)?,
            at(stratum, "state")?.py_str()
        ));
    }
    Ok(Done::Printed(format!(
        "drew {} unit(s) in {} batch(es): {}",
        pyvalue::length(at(&record, "units")?)?,
        pyvalue::length(at(&record, "batches")?)?,
        parts.join(", ")
    )))
}

/// `int(value.removeprefix("item-")) + 1`.
fn item_number(item_id: &Value) -> R<String> {
    let Value::Str(identifier) = item_id else {
        return Err(Failure::Crash(format!(
            "AttributeError: '{}' object has no attribute 'removeprefix'",
            item_id.type_name()
        )));
    };
    Ok(step(
        &python_int(identifier.strip_prefix("item-").unwrap_or(identifier))?,
        true,
    ))
}

/// `int(text)`, as digits with an optional `-`.
fn python_int(text: &str) -> R<String> {
    pyunicode::parse_int(text).map_err(|message| Failure::Crash(format!("ValueError: {message}")))
}

/// `number + 1` or `number - 1` of `int` digits, at any length.
fn step(number: &str, up: bool) -> String {
    let (negative, magnitude) = match number.strip_prefix('-') {
        Some(magnitude) => (true, magnitude),
        None => (false, number),
    };
    if magnitude == "0" {
        return if up { "1" } else { "-1" }.to_string();
    }
    let mut digits = magnitude.as_bytes().to_vec();
    if negative != up {
        let carried = digits.iter_mut().rev().all(|digit| {
            let wrapped = *digit == b'9';
            *digit = if wrapped { b'0' } else { *digit + 1 };
            wrapped
        });
        if carried {
            digits.insert(0, b'1');
        }
    } else {
        for digit in digits.iter_mut().rev() {
            let borrowed = *digit == b'0';
            *digit = if borrowed { b'9' } else { *digit - 1 };
            if !borrowed {
                break;
            }
        }
        let leading = digits.iter().take_while(|digit| **digit == b'0').count();
        digits.drain(..leading.min(digits.len() - 1));
    }
    let magnitude = String::from_utf8(digits).unwrap_or_default();
    match (negative, magnitude.as_str()) {
        (_, "0") | (false, _) => magnitude,
        (true, _) => format!("-{magnitude}"),
    }
}

fn claim_list(batch: &Value) -> R<Value> {
    let mut listed = Object::default();
    for review in each(at(batch, "reviews")?)?.iter() {
        let mut items: Vec<(String, &str, Vec<Value>, Vec<Value>)> = Vec::new();
        for entry in each(at(review, "not_findings")?)?.iter() {
            let number = item_number(at(entry, "item_id")?)?;
            match items.iter_mut().find(|item| item.0 == number) {
                Some(item) => *item = (number, "not-a-finding", Vec::new(), Vec::new()),
                None => items.push((number, "not-a-finding", Vec::new(), Vec::new())),
            }
        }
        for claim in each(at(review, "claims")?)?.iter() {
            let anchor = at(claim, "anchor")?;
            let number = item_number(at(anchor, "item_id")?)?;
            let at_item = match items.iter().position(|item| item.0 == number) {
                Some(position) => position,
                None => {
                    items.push((number, "finding", Vec::new(), Vec::new()));
                    items.len() - 1
                }
            };
            items[at_item].2.push(at(anchor, "quote")?.clone());
            items[at_item].3.push(at(claim, "id")?.clone());
        }
        items.sort_by(|left, right| pyvalue::compare_integers(&left.0, &right.0));
        let mut numbered = Object::default();
        for (number, kind, quotes, ids) in items {
            numbered.insert(
                number,
                object(vec![
                    ("kind", text(kind)),
                    ("quotes", Value::Array(quotes)),
                    ("ids", Value::Array(ids)),
                ]),
            );
        }
        listed.insert(
            json_key(at(review, "attempt_id")?)?,
            Value::Object(numbered),
        );
    }
    Ok(Value::Object(listed))
}

/// `directory in (path, *path.parents)`, both resolved.
fn holds(directory: &[u8], path: &[u8]) -> bool {
    if path == directory {
        return true;
    }
    if directory == b"/" {
        return path.starts_with(b"/");
    }
    path.starts_with(directory) && path.get(directory.len()) == Some(&b'/')
}

fn listed_claims(root: &Root, current: &[u8], run: &str, target: &str, out: &[u8]) -> R<Done> {
    let run = format!("runs/{}", name_of(&text(run))?);
    let given = PyPath::new(out);
    let out = crate::packet_selection::resolve(&given)?;
    let (directory, sample) = drawn_sample(root, current)?;
    let first = sampled_grade(&sample, root, &run, target)?;
    if pyvalue::exists(&out) || holds(&pyfs::resolve(directory.as_bytes()), out.as_bytes()) {
        return Err(Failure::Inconsistent(
            [
                given.as_bytes(),
                b" exists or is inside the round's directory; a claim list goes to a new file outside the round's \
                  records",
            ]
            .concat(),
        ));
    }
    save_new(&out, &claim_list(&first)?)?;
    let reviews = each(at(&first, "reviews")?)?;
    let mut claims = 0;
    for review in reviews.iter() {
        claims += pyvalue::length(at(review, "claims")?)?;
    }
    Ok(Done::Printed(format!(
        "wrote the claim list of {run}/{target}: {claims} claims in {} reviews; prepare the second workspace with \
         --inventory {}",
        reviews.len(),
        String::from_utf8_lossy(given.as_bytes())
    )))
}

fn sibling(path: &PyPath, name: &str) -> PyPath {
    current_grading::parent_of(path).join(name.as_bytes())
}

fn missing_fault_notes(assessment: &PyPath) -> R<Map<Value>> {
    let verdicts = current_grading::read_json(&sibling(assessment, "verdicts.json"))?;
    let receipt = current_grading::read_json(&sibling(assessment, "receipt.json"))?;
    let mut found = Map::default();
    for review in each(at(&receipt, "reviews")?)?.iter() {
        let attempt = at(review, "attempt_id")?;
        pyvalue::hashable(attempt)?;
        let token = at(review, "token")?;
        let reviews = at(&verdicts, "reviews")?;
        let entry = match (reviews, token) {
            (Value::Object(object), Value::Str(name)) => {
                object.get(name).ok_or_else(|| missing(token))?
            }
            _ => return Err(missing(token)),
        };
        let mut items: Vec<(String, Value)> = Vec::new();
        for (number, item) in keys(at(entry, "items")?)? {
            items.push((python_int(&number.py_str())?, item));
        }
        items.sort_by(|left, right| pyvalue::compare_integers(&left.0, &right.0));
        let mut notes = Object::default();
        for (number, item) in items {
            let note = at(&item, "note")?;
            if pyvalue::contains(note, &text(MISSING_FAULT))? {
                notes.insert(format!("item-{}", step(&number, false)), note.clone());
            }
        }
        found.insert(attempt.clone(), Value::Object(notes));
    }
    Ok(found)
}

/// `Counter((item id, claim id, quote))` of a review's claims.
fn identity(review: &Value) -> R<Map<usize>> {
    tally(each(at(review, "claims")?)?.iter().map(|claim| {
        let anchor = at(claim, "anchor")?;
        tuple_key(&[
            at(anchor, "item_id")?,
            at(claim, "id")?,
            at(anchor, "quote")?,
        ])
    }))
}

/// `sorted(left - right)` of two counters.
fn surplus(left: &Map<usize>, right: &Map<usize>) -> R<Vec<Value>> {
    let found = left
        .0
        .iter()
        .filter(|(key, count)| *count > right.get(key).copied().unwrap_or(0))
        .map(|(key, _)| key.clone())
        .collect();
    sorted(found)
}

fn find_claim<'a>(review: &'a Value, identifier: &Value) -> R<&'a Value> {
    match at(review, "claims")? {
        Value::Array(claims) => {
            for claim in claims {
                if at(claim, "id")?.py_eq(identifier) {
                    return Ok(claim);
                }
            }
            Err(Failure::Crash("StopIteration".to_string()))
        }
        other => Err(Failure::Crash(format!(
            "TypeError: '{}' object is not iterable",
            other.type_name()
        ))),
    }
}

fn answer(unit: &Value, review: &Value) -> R<Value> {
    let kind = at(unit, "unit")?;
    let subject = at(unit, "subject")?;
    if is(kind, "claim") {
        let claim = find_claim(review, subject)?;
        let mut says = Object::default();
        for entry in each(at(claim, "known_problems")?)?.iter() {
            says.insert(
                json_key(at(entry, "family_id")?)?,
                at(entry, "says_what")?.clone(),
            );
        }
        return Ok(object(vec![
            ("result", at(claim, "outcome")?.clone()),
            ("kind", at(claim, "kind")?.clone()),
            ("open", at(claim, "open")?.clone()),
            ("answers", at(claim, "answers")?.clone()),
            ("says_what", Value::Object(says)),
            ("reason", at(claim, "reason")?.clone()),
        ]));
    }
    let claims = at(review, "claims")?;
    let (result, ids) = if is(kind, "family-review") {
        let (result, ids) = credit(claims, subject)?;
        (text(result), ids)
    } else if is(kind, "pre-existing-review") {
        pre_existing_result(claims, subject)?
    } else {
        let (result, ids) = control_result(claims)?;
        (text(result), ids)
    };
    Ok(object(vec![
        ("result", result),
        ("claim_ids", Value::Array(ids)),
    ]))
}

fn table(pairs: &[(Value, Value)]) -> R<Value> {
    let mut confusion: Map<Map<usize>> = Map::default();
    for (first, second) in pairs {
        pyvalue::hashable(first)?;
        pyvalue::hashable(second)?;
        *confusion
            .slot(first.clone(), Map::default)
            .slot(second.clone(), || 0) += 1;
    }
    let agreements = pairs
        .iter()
        .filter(|(first, second)| first.py_eq(second))
        .count();
    Ok(object(vec![
        ("units", int(pairs.len())),
        ("agreements", int(agreements)),
        (
            "confusion",
            mapping(confusion, |cell| mapping(cell, |count| Ok(int(count))))?,
        ),
    ]))
}

/// Each review of the sampled batches, by `<run name>/<attempt>`: both assessments, its task and the second
/// assessor's missing-fault notes.
struct Paired {
    reviews: Vec<(String, Value, Value)>,
    targets: Map<Value>,
    notes: Map<Value>,
}

fn pre_existing_says(claim: &Value, problem: &Value) -> R<Value> {
    for entry in each(at(claim, "pre_existing_problems")?)?.iter() {
        if at(entry, "problem_id")?.py_eq(problem) {
            return Ok(at(entry, "says_what")?.clone());
        }
    }
    Ok(text("no"))
}

fn side(claim: &Value) -> R<Value> {
    let mut says = Object::default();
    for entry in each(at(claim, "pre_existing_problems")?)?.iter() {
        says.insert(
            json_key(at(entry, "problem_id")?)?,
            at(entry, "says_what")?.clone(),
        );
    }
    Ok(object(vec![
        ("outcome", at(claim, "outcome")?.clone()),
        ("says_what", Value::Object(says)),
        ("candidate", at(claim, "pre_existing_candidate_id")?.clone()),
    ]))
}

fn populations(sample: &Value) -> R<Map<Value>> {
    let mut found = Map::default();
    for stratum in each(at(sample, "strata")?)?.iter() {
        let name = at(stratum, "id")?;
        pyvalue::hashable(name)?;
        found.insert(name.clone(), at(stratum, "population")?.clone());
    }
    Ok(found)
}

fn target_of<'a>(paired: &'a Paired, review: &str) -> R<&'a Value> {
    let key = text(review);
    paired.targets.get(&key).ok_or_else(|| missing(&key))
}

fn grouped(groups: Map<Vec<(Value, Value)>>) -> R<Value> {
    let ordered = sorted_items(groups.0)?;
    mapping(Map(ordered), |pairs| table(&pairs))
}

fn pre_existing_reported(sample: &Value, paired: &Paired) -> R<Value> {
    let empty = Value::Object(Object::default());
    let mut catalogs: Map<Value> = Map::default();
    for (target, task) in keys(at(sample, "tasks")?)? {
        catalogs.insert(
            target,
            pyvalue::get_or(&task, "pre_existing_problems", &empty)?.clone(),
        );
    }
    let catalog_of = |review: &str| -> R<Value> {
        let target = target_of(paired, review)?;
        catalogs.get(target).cloned().ok_or_else(|| missing(target))
    };
    let (mut pairs, mut differing, mut outside) = (Vec::new(), Vec::new(), Vec::new());
    let mut candidates: Map<Vec<Value>> = Map::default();
    for (review, before, after) in &paired.reviews {
        for first in each(at(before, "claims")?)?.iter() {
            let second = find_claim(after, at(first, "id")?)?;
            let unit = text(&format!("{review}#{}", at(first, "id")?.py_str()));
            for (problem, _label) in keys(&catalog_of(review)?)? {
                let (said, again) = (
                    pre_existing_says(first, &problem)?,
                    pre_existing_says(second, &problem)?,
                );
                let row = object(vec![
                    ("claim", unit.clone()),
                    ("problem", problem.clone()),
                    ("first", said.clone()),
                    ("second", again.clone()),
                ]);
                if !said.py_eq(&again) {
                    let results = vec![
                        pre_existing_result(at(before, "claims")?, &problem)?.0,
                        pre_existing_result(at(after, "claims")?, &problem)?.0,
                    ];
                    differing.push(merged(
                        row.clone(),
                        object(vec![("results", Value::Array(results))]),
                    ));
                }
                pairs.push(row);
            }
            let outcomes = [at(first, "outcome")?, at(second, "outcome")?];
            if outcomes
                .iter()
                .any(|outcome| is(outcome, "outside-this-change"))
            {
                outside.push(object(vec![
                    ("claim", unit.clone()),
                    ("first", side(first)?),
                    ("second", side(second)?),
                ]));
            }
            let candidate = at(second, "pre_existing_candidate_id")?;
            if !matches!(candidate, Value::Null) {
                candidates
                    .slot(
                        tuple_key(&[target_of(paired, review)?, candidate])?,
                        Vec::new,
                    )
                    .push(unit.clone());
            }
        }
    }
    let mut grid = Vec::new();
    for row in &pairs {
        grid.push((at(row, "first")?.clone(), at(row, "second")?.clone()));
    }
    let populations = populations(sample)?;
    let mut results: [Map<Vec<(Value, Value)>>; 3] = Default::default();
    for (_, stratum) in PRE_EXISTING_STRATA {
        let empty = Value::Array(Vec::new());
        let members = populations.get(&text(stratum)).unwrap_or(&empty);
        for identifier in each(members)?.iter() {
            let Value::Str(identifier) = identifier else {
                return Err(Failure::Crash(format!(
                    "AttributeError: '{}' object has no attribute 'partition'",
                    identifier.type_name()
                )));
            };
            let (review, problem) = identifier
                .split_once("#pre-existing:")
                .unwrap_or((identifier.as_str(), ""));
            let Some((_, before, after)) =
                paired.reviews.iter().find(|(known, ..)| known == review)
            else {
                continue;
            };
            let problem = text(problem);
            let result = pair(
                &pre_existing_result(at(before, "claims")?, &problem)?.0,
                &pre_existing_result(at(after, "claims")?, &problem)?.0,
            );
            let Value::Array(result) = result else {
                unreachable!("a pair is a list");
            };
            let result = (result[0].clone(), result[1].clone());
            let catalog = catalog_of(review)?;
            let label = match &catalog {
                Value::Object(object) => object
                    .get(problem.as_str().unwrap_or_default())
                    .ok_or_else(|| missing(&problem))?,
                _ => return Err(missing(&problem)),
            };
            let label = if label.truthy() {
                label.clone()
            } else {
                text("unlabelled")
            };
            for (group, name) in
                results
                    .iter_mut()
                    .zip([problem.clone(), target_of(paired, review)?.clone(), label])
            {
                group.slot(name, Vec::new).push(result.clone());
            }
        }
    }
    let raised: Vec<(Value, Value)> = grid
        .iter()
        .filter(|(first, second)| !(is(first, "no") && is(second, "no")))
        .cloned()
        .collect();
    let mut second_candidates = Vec::new();
    for (key, claims) in sorted_items(candidates.0)? {
        let Value::Array(parts) = key else {
            unreachable!("each candidate key is a target and a candidate");
        };
        second_candidates.push(object(vec![
            ("target", parts[0].clone()),
            ("candidate", parts[1].clone()),
            ("claims", Value::Array(claims)),
        ]));
    }
    let mut notes = Vec::new();
    for (review, ..) in &paired.reviews {
        let key = text(review);
        let held = paired.notes.get(&key).ok_or_else(|| missing(&key))?;
        for (item, note) in keys(held)? {
            notes.push(object(vec![
                ("review", key.clone()),
                ("item", item),
                ("note", note),
            ]));
        }
    }
    let [problems, tasks, labels] = results;
    Ok(object(vec![
        (
            "says_what",
            object(vec![
                ("every_pair", table(&grid)?),
                ("raised", table(&raised)?),
                ("pairs", Value::Array(pairs)),
                ("differing", Value::Array(differing)),
            ]),
        ),
        ("outside_this_change", Value::Array(outside)),
        ("second_candidates", Value::Array(second_candidates)),
        ("missing_fault_notes", Value::Array(notes)),
        (
            "results",
            object(vec![
                ("problem", grouped(problems)?),
                ("task", grouped(tasks)?),
                ("label", grouped(labels)?),
            ]),
        ),
    ]))
}

fn reported(sample: &Value, paired: &Paired) -> R<Value> {
    let mut rows: Vec<(Value, Value, Value)> = Vec::new();
    for (review, before, after) in &paired.reviews {
        for claim in each(at(before, "claims")?)?.iter() {
            let other = find_claim(after, at(claim, "id")?)?.clone();
            let known = at(
                task_of(at(sample, "tasks")?, target_of(paired, review)?)?,
                "known_problems",
            )?
            .clone();
            rows.push((claim.clone(), other, known));
        }
    }
    let mut grid = Vec::new();
    for (first, second, known) in &rows {
        for (family, _label) in keys(known)? {
            grid.push((says_what(first, &family)?, says_what(second, &family)?));
        }
    }
    let populations = populations(sample)?;
    let population = |name: &str| -> R<&Value> {
        let key = text(name);
        populations.get(&key).ok_or_else(|| missing(&key))
    };
    let mut labels: Map<Value> = Map::default();
    for label in IMPACT_STRATA {
        for identifier in each(population(label)?)?.iter() {
            pyvalue::hashable(identifier)?;
            labels.insert(identifier.clone(), text(label));
        }
    }
    let mut credits: Map<Vec<(Value, Value)>> = Map::default();
    for (_, stratum) in CREDIT_STRATA {
        for identifier in each(population(stratum)?)?.iter() {
            let Value::Str(name) = identifier else {
                return Err(Failure::Crash(format!(
                    "AttributeError: '{}' object has no attribute 'partition'",
                    identifier.type_name()
                )));
            };
            let (review, family) = name.split_once('#').unwrap_or((name.as_str(), ""));
            let Some((_, before, after)) =
                paired.reviews.iter().find(|(known, ..)| known == review)
            else {
                continue;
            };
            let family = text(family);
            let result = (
                text(credit(at(before, "claims")?, &family)?.0),
                text(credit(at(after, "claims")?, &family)?.0),
            );
            let label = labels
                .get(identifier)
                .cloned()
                .unwrap_or_else(|| text("unlabelled"));
            credits.slot(label, Vec::new).push(result);
        }
    }
    let outcome_pairs = |linked: bool| -> R<Vec<(Value, Value)>> {
        let mut found = Vec::new();
        for (first, second, _) in &rows {
            if matches!(at(first, "canonical_id")?, Value::Null) != linked {
                found.push((
                    at(first, "outcome")?.clone(),
                    at(second, "outcome")?.clone(),
                ));
            }
        }
        Ok(found)
    };
    let both = |first: &Value, second: &Value, outcome: &str| -> R<bool> {
        let left = at(first, "outcome")?;
        Ok(left.py_eq(at(second, "outcome")?) && is(left, outcome))
    };
    let mut kinds = Vec::new();
    let mut reasons = Vec::new();
    for (first, second, _) in &rows {
        if both(first, second, "suggestion")? {
            kinds.push((at(first, "kind")?.clone(), at(second, "kind")?.clone()));
        }
        if both(first, second, "unresolved")? {
            reasons.push((
                at(at(first, "open")?, "kind")?.clone(),
                at(at(second, "open")?, "kind")?.clone(),
            ));
        }
    }
    let mut answers = Object::default();
    for name in ANSWERS {
        let mut found = Vec::new();
        for (first, second, _) in &rows {
            let (left, right) = (
                at(at(first, "answers")?, name)?,
                at(at(second, "answers")?, name)?,
            );
            if !matches!(left, Value::Null) && !matches!(right, Value::Null) {
                found.push((left.clone(), right.clone()));
            }
        }
        answers.insert(name.to_string(), table(&found)?);
    }
    let raised: Vec<(Value, Value)> = grid
        .iter()
        .filter(|(first, second)| !(is(first, "no") && is(second, "no")))
        .cloned()
        .collect();
    let mut found = vec![
        (
            "claims",
            object(vec![
                ("unlinked", table(&outcome_pairs(false)?)?),
                ("linked", table(&outcome_pairs(true)?)?),
            ]),
        ),
        ("suggestion_kinds", table(&kinds)?),
        ("open_reasons", table(&reasons)?),
        ("answers", Value::Object(answers)),
        (
            "says_what",
            object(vec![
                ("every_pair", table(&grid)?),
                ("raised", table(&raised)?),
            ]),
        ),
        ("credit", grouped(credits)?),
    ];
    let mut any_pre_existing = false;
    for (_, before, _) in &paired.reviews {
        if pyvalue::has(before, "pre_existing_problems")? {
            any_pre_existing = true;
            break;
        }
    }
    if any_pre_existing {
        found.push(("pre_existing", pre_existing_reported(sample, paired)?));
    }
    Ok(object(found))
}

/// The `assessment-<n>/assessment.json` files under a batch's directory, by `n`.
fn assessments(batch: &PyPath) -> R<Vec<PyPath>> {
    let mut found = Vec::new();
    for name in pyfs::names(batch.as_bytes()) {
        if !name.starts_with(b"assessment-") {
            continue;
        }
        let path = batch.join(&name).join(b"assessment.json");
        if !pyfs::lexists(path.as_bytes()) {
            continue;
        }
        let text = String::from_utf8_lossy(&name).into_owned();
        let part = text.split('-').nth(1).unwrap_or_default();
        found.push((python_int(part)?, path));
    }
    found.sort_by(|left, right| pyvalue::compare_integers(&left.0, &right.0));
    Ok(found.into_iter().map(|(_, path)| path).collect())
}

fn comparison_of(directory: &PyPath, sample: &Value, root: &Root) -> R<Result<Value, Vec<String>>> {
    let grades = current_grading::read_json(&grades_path(root, sample)?)?;
    let mut graded: Map<Value> = Map::default();
    for batch in each(at(&grades, "batches")?)?.iter() {
        graded.insert(
            tuple_key(&[at(batch, "run")?, at(batch, "target")?])?,
            batch.clone(),
        );
    }
    let mut held: Map<PyPath> = Map::default();
    let mut problems = Vec::new();
    for batch in each(at(sample, "batches")?)?.iter() {
        let (run, target) = (at(batch, "run")?, at(batch, "target")?);
        let saved = assessments(
            &directory
                .join(b"second")
                .join(name_of(run)?.as_bytes())
                .join(bytes_of(target)?),
        )?;
        let place = format!("{}/{}", run.py_str(), target.py_str());
        let Some(latest) = saved.last() else {
            problems.push(format!("{place}: no second assessment"));
            continue;
        };
        let receipt = current_grading::read_json(&sibling(latest, "receipt.json"))?;
        let mut changed = Vec::new();
        for name in ["verdicts", "assessment"] {
            let hash = current_grading::file_hash(&sibling(latest, &format!("{name}.json")))?;
            if !pyvalue::get(&receipt, &format!("{name}_sha256"))?.py_eq(&Value::Str(hash)) {
                changed.push(name);
            }
        }
        let key = tuple_key(&[run, target])?;
        let first = graded.get(&key).ok_or_else(|| missing(&key))?;
        if !at(&receipt, "input_fingerprint")?.py_eq(at(first, "input_fingerprint")?) {
            problems.push(format!(
                "{place}: the second assessment graded other inputs than the sampled grade"
            ));
        }
        for name in changed {
            problems.push(format!(
                "{place}: {name}.json is not the file the second assessment's receipt records"
            ));
        }
        held.insert(key, latest.clone());
    }
    if !problems.is_empty() {
        return Ok(Err(problems));
    }
    let mut paired = Paired {
        reviews: Vec::new(),
        targets: Map::default(),
        notes: Map::default(),
    };
    let mut invalid = Vec::new();
    for (key, path) in &held.0 {
        let Value::Array(parts) = key else {
            unreachable!("each batch key is a run and a target");
        };
        let batch = graded.get(key).ok_or_else(|| missing(key))?;
        let firsts = keyed(at(batch, "reviews")?, "attempt_id")?;
        let assessment = current_grading::read_json(path)?;
        let answered = at(&assessment, "reviews")?;
        let noted = missing_fault_notes(path)?;
        let seconds = keyed(answered, "attempt_id")?;
        let mut ids = Vec::new();
        for review in each(answered)?.iter() {
            ids.push(at(review, "attempt_id")?.clone());
        }
        let counts = counter(ids)?;
        let mut attempts: Vec<Value> = firsts
            .0
            .iter()
            .map(|(attempt, _)| attempt.clone())
            .collect();
        for (attempt, _) in &seconds.0 {
            if !attempts.iter().any(|known| known.py_eq(attempt)) {
                attempts.push(attempt.clone());
            }
        }
        for attempt in sorted(attempts)? {
            let place = format!("{}/{}", name_of(&parts[0])?, attempt.py_str());
            let (Some(first), Some(second)) = (firsts.get(&attempt), seconds.get(&attempt)) else {
                if seconds.get(&attempt).is_none() {
                    invalid.push(format!(
                        "{place}: the second assessment holds no such review"
                    ));
                } else {
                    invalid.push(format!(
                        "{place}: the second assessment holds a review the first grade does not, or holds it twice"
                    ));
                }
                continue;
            };
            if counts.get(&attempt).copied().unwrap_or(0) > 1 {
                invalid.push(format!(
                    "{place}: the second assessment holds a review the first grade does not, or holds it twice"
                ));
                continue;
            }
            let (listed, given) = (identity(first)?, identity(second)?);
            for claim in surplus(&listed, &given)? {
                invalid.push(format!(
                    "{place}#{}: the second assessment has no answer under this claim's item, id and exact quote",
                    each(&claim)?[1].py_str()
                ));
            }
            for claim in surplus(&given, &listed)? {
                invalid.push(format!(
                    "{place}#{}: the second assessment answers a claim the first grader did not list, or answers it \
                     twice",
                    each(&claim)?[1].py_str()
                ));
            }
            let entry = paired
                .reviews
                .iter()
                .position(|(known, ..)| *known == place);
            let row = (place.clone(), first.clone(), second.clone());
            match entry {
                Some(position) => paired.reviews[position] = row,
                None => paired.reviews.push(row),
            }
            paired.targets.insert(text(&place), parts[1].clone());
            paired.notes.insert(
                text(&place),
                noted
                    .get(&attempt)
                    .cloned()
                    .unwrap_or_else(|| Value::Object(Object::default())),
            );
        }
    }
    if !invalid.is_empty() {
        invalid.push(
            "a second assessment that drops, adds, repeats, renames or re-quotes a claim or a review is invalid audit \
             output, so nothing was compared; assess the batch again from the first grader's claim list"
                .to_string(),
        );
        return Ok(Err(invalid));
    }
    let mut rows = Vec::new();
    for unit in each(at(sample, "units")?)?.iter() {
        let place = format!(
            "{}/{}",
            name_of(at(unit, "run")?)?,
            at(unit, "attempt")?.py_str()
        );
        let Some((_, before, after)) = paired.reviews.iter().find(|(known, ..)| *known == place)
        else {
            return Err(missing(&text(&place)));
        };
        let (first, second) = (answer(unit, before)?, answer(unit, after)?);
        let agreement = at(&first, "result")?.py_eq(at(&second, "result")?);
        rows.push(object(vec![
            ("id", at(unit, "id")?.clone()),
            ("unit", at(unit, "unit")?.clone()),
            ("strata", at(unit, "strata")?.clone()),
            ("first", first),
            ("second", second),
            ("agreement", Value::Bool(agreement)),
        ]));
    }
    let mut strata = Vec::new();
    for stratum in each(at(sample, "strata")?)?.iter() {
        let name = at(stratum, "id")?;
        let mut found = Vec::new();
        for row in &rows {
            if pyvalue::contains(at(row, "strata")?, name)? {
                found.push((
                    at(at(row, "first")?, "result")?.clone(),
                    at(at(row, "second")?, "result")?.clone(),
                ));
            }
        }
        strata.push(merged(
            object(vec![
                ("id", name.clone()),
                ("state", at(stratum, "state")?.clone()),
            ]),
            table(&found)?,
        ));
    }
    let sample_pin = root.pin_file(&directory.join(b"sample.json"))?;
    let mut second = Vec::new();
    for (_key, path) in sorted_items(held.0)? {
        for name in ["verdicts.json", "assessment.json", "receipt.json"] {
            second.push(root.pin_file(&sibling(&path, name))?);
        }
    }
    Ok(Ok(object(vec![
        ("contract", text("evaluator-audit-comparison/v2")),
        ("sample", sample_pin),
        ("second", Value::Array(second)),
        ("strata", Value::Array(strata)),
        ("units", Value::Array(rows)),
        ("reported", reported(sample, &paired)?),
    ])))
}

fn compare(root: &Root, current: &[u8]) -> R<Done> {
    let (directory, sample) = drawn_sample(root, current)?;
    let comparison = match comparison_of(&directory, &sample, root)? {
        Ok(comparison) => comparison,
        Err(problems) => return Ok(Done::Problems(problems)),
    };
    let saved = merged(comparison, object(vec![("compared_at", now())]));
    save_new(&directory.join(b"comparison.json"), &saved)?;
    let mut parts = Vec::new();
    for stratum in each(at(&saved, "strata")?)?.iter() {
        parts.push(format!(
            "{} {}/{}",
            at(stratum, "id")?.py_str(),
            at(stratum, "agreements")?.py_str(),
            at(stratum, "units")?.py_str()
        ));
    }
    let mut disagreements = 0;
    for row in each(at(&saved, "units")?)?.iter() {
        if !at(row, "agreement")?.truthy() {
            disagreements += 1;
        }
    }
    Ok(Done::Printed(format!(
        "compared {}; {disagreements} disagreement(s) to reconcile",
        parts.join(", ")
    )))
}

fn read_bytes(path: &PyPath) -> R<Vec<u8>> {
    std::fs::read(path.to_path_buf()).map_err(|error| {
        Failure::Input(format!(
            "cannot read {}: {}",
            pytext::stderr_text(path.as_bytes()),
            pytext::os_error(&error, path.as_bytes())
        ))
    })
}

fn filled(value: Option<&Value>) -> R<bool> {
    let text = value.map_or(String::new(), Value::py_str);
    Ok(!pytext::strip(&text).is_empty())
}

fn conclude(root: &Root, current_arg: &[u8], reconciliation_path: &[u8]) -> R<Done> {
    let current = root.local(current_arg)?;
    let (directory, sample) = drawn_sample(root, current_arg)?;
    let comparison = current_grading::read_json(&directory.join(b"comparison.json"))?;
    let mut audit = current_grading::read_json(&current.join(b"audits.json"))?;
    let mut pins = vec![at(&comparison, "sample")?.clone()];
    pins.extend(each(at(&comparison, "second")?)?.iter().cloned());
    let mut changed = Vec::new();
    for pin in &pins {
        let path = at(pin, "path")?;
        if !Value::Str(current_grading::file_hash(&under(root, path)?)?).py_eq(at(pin, "sha256")?) {
            changed.push(path.clone());
        }
    }
    if !changed.is_empty() {
        return Err(Failure::Inconsistent(
            format!(
                "changed since the comparison, so nothing was concluded: {}",
                pyvalue::join(", ", &changed)?
            )
            .into_bytes(),
        ));
    }
    let derived = comparison_of(&directory, &sample, root)?;
    let recorded: Object = comparison
        .as_object()
        .into_iter()
        .flat_map(Object::iter)
        .filter(|(field, _)| field.as_str() != "compared_at")
        .map(|(field, value)| (field.clone(), value.clone()))
        .collect();
    if !derived.is_ok_and(|derived| derived.py_eq(&Value::Object(recorded))) {
        return Err(Failure::Inconsistent(
            b"comparison.json is not the comparison of sample.json with the latest second assessment of each \
              sampled batch, so nothing was concluded"
                .to_vec(),
        ));
    }
    let raw = read_bytes(&PyPath::new(reconciliation_path))?;
    let reconciliation = crate::packet_selection::loads_bytes(&raw)?;
    let listed = at(&reconciliation, "decisions")?;
    let decisions = keyed(listed, "unit")?;
    let mut disputed: Map<Value> = Map::default();
    for row in each(at(&comparison, "units")?)?.iter() {
        if !at(row, "agreement")?.truthy() {
            let identifier = at(row, "id")?;
            pyvalue::hashable(identifier)?;
            disputed.insert(identifier.clone(), row.clone());
        }
    }
    let only = |left: &Map<Value>, right: &Map<Value>| -> R<Vec<Value>> {
        sorted(
            left.0
                .iter()
                .map(|(key, _)| key)
                .filter(|key| right.get(key).is_none())
                .cloned()
                .collect(),
        )
    };
    let mut problems = Vec::new();
    for unit in only(&disputed, &decisions)? {
        problems.push(format!(
            "{}: a disagreement without a reconciliation",
            unit.py_str()
        ));
    }
    for unit in only(&decisions, &disputed)? {
        problems.push(format!(
            "{}: reconciled, but the comparison records no such disagreement",
            unit.py_str()
        ));
    }
    let ordered = sorted_items(decisions.0.clone())?;
    let finding = |decision: &Value| -> R<Value> { Ok(pyvalue::get(decision, "finding")?.clone()) };
    for (unit, decision) in &ordered {
        let found = finding(decision)?;
        let named = FINDINGS.iter().any(|known| is(&found, known));
        if !named
            || !filled(Some(pyvalue::get_or(decision, "reason", &text(""))?))?
            || !pyvalue::get(decision, "evidence")?.truthy()
        {
            problems.push(format!(
                "{}: a reconciliation names one of {} with its reason and evidence",
                unit.py_str(),
                FINDINGS.join(", ")
            ));
        }
    }
    if decisions.0.len() != pyvalue::length(listed)?
        || !filled(Some(pyvalue::get_or(
            &reconciliation,
            "reconciler",
            &text(""),
        )?))?
    {
        problems
            .push("the reconciliation names its reconciler and decides each unit once".to_string());
    }
    let mut undetermined = Vec::new();
    for (unit, decision) in &decisions.0 {
        if is(&finding(decision)?, "undetermined") {
            undetermined.push(unit.clone());
        }
    }
    let undetermined = sorted(undetermined)?;
    if problems.is_empty() && !undetermined.is_empty() {
        problems = vec![format!(
            "{} disagreement(s) are undetermined, so nothing was saved; settle them and conclude again: {}",
            undetermined.len(),
            pyvalue::join(", ", &undetermined)?
        )];
    }
    if !problems.is_empty() {
        return Ok(Done::Problems(problems));
    }
    let mut limits: Map<Value> = Map::default();
    for entry in each(at(at(at(&audit, "plan")?, "tolerances")?, "limits")?)?.iter() {
        let stratum = at(entry, "stratum")?;
        pyvalue::hashable(stratum)?;
        limits.insert(stratum.clone(), at(entry, "max_errors")?.clone());
    }
    let mut strata = Vec::new();
    for stratum in each(at(&comparison, "strata")?)?.iter() {
        let name = at(stratum, "id")?;
        let mut errors = 0;
        for (unit, row) in &disputed.0 {
            if pyvalue::contains(at(row, "strata")?, name)? {
                let decision = decisions.get(unit).ok_or_else(|| missing(unit))?;
                if is(at(decision, "finding")?, "first-error") {
                    errors += 1;
                }
            }
        }
        let limit = if is(at(stratum, "state")?, "sampled") {
            limits.get(name).cloned().ok_or_else(|| missing(name))?
        } else {
            Value::Null
        };
        let within = if matches!(limit, Value::Null) {
            Value::Null
        } else {
            Value::Bool(pyvalue::compare(&int(errors), &limit)? != Ordering::Greater)
        };
        strata.push(merged(
            stratum.clone(),
            object(vec![
                ("confirmed_errors", int(errors)),
                ("max_errors", limit),
                ("within_tolerance", within),
            ]),
        ));
    }
    let saved = directory.join(b"reconciliation.json");
    write_new(&saved, &raw)?;
    save_new(
        &directory.join(b"conclusion.json"),
        &object(vec![
            ("contract", text("evaluator-audit-conclusion/v2")),
            ("concluded_at", now()),
            (
                "comparison",
                root.pin_file(&directory.join(b"comparison.json"))?,
            ),
            ("reconciliation", root.pin_file(&saved)?),
            ("strata", Value::Array(strata.clone())),
        ]),
    )?;
    let mut failed = Vec::new();
    for stratum in &strata {
        if matches!(at(stratum, "within_tolerance")?, Value::Bool(false)) {
            failed.push(at(stratum, "id")?.clone());
        }
    }
    if !failed.is_empty() {
        return Ok(Done::Problems(vec![format!(
            "this round failed and audits.json stays unassessed; outside tolerance: {}. Grade the affected batches \
             again, record the next seed in audits.json and draw every stratum again",
            pyvalue::join(", ", &failed)?
        )]));
    }
    let mut evidence = Vec::new();
    for name in [
        "sample.json",
        "comparison.json",
        "reconciliation.json",
        "conclusion.json",
    ] {
        evidence.push(root.pin_file(&directory.join(name.as_bytes()))?);
    }
    evidence.extend(each(at(&comparison, "second")?)?.iter().cloned());
    let reason = "Every stratum was drawn, assessed a second time and reconciled, and every sampled stratum is \
                  within its tolerance. conclusion.json holds each stratum's agreements and confirmed first-grader \
                  errors.";
    let Value::Object(fields) = &mut audit else {
        return Err(Failure::Crash(format!(
            "AttributeError: '{}' object has no attribute 'update'",
            audit.type_name()
        )));
    };
    fields.insert("state".to_string(), text("assessed"));
    fields.insert("evidence".to_string(), Value::Array(evidence));
    fields.insert("reason".to_string(), text(reason));
    replace_json(&current.join(b"audits.json"), &audit)?;
    Ok(Done::Printed(format!("audits.json is assessed: {reason}")))
}

/// One operation of `evaluator_audit.py` other than `second`, run under the record lock by the caller.
fn string_of(value: &Value) -> R<&str> {
    match value {
        Value::Str(text) => Ok(text),
        other => bytes_of(other).map(|_| ""),
    }
}

/// `Path.mkdir(parents=True)`, whose `FileExistsError` `main` reports as a record written once.
fn make_new_dirs(path: &PyPath) -> R<()> {
    make_dirs(&current_grading::parent_of(path))?;
    std::fs::create_dir(path.to_path_buf()).map_err(|error| os_failure(&error, path))
}

const SECOND_CLEANUP: &str =
    "the verdicts passed every check, but workspace cleanup failed, so nothing was saved: ";

/// `second`: save an independent assessment of a sampled batch, graded blind from the first grader's claim list.
fn second(root: &Root, current: &[u8], arguments: &Arguments) -> R<Done> {
    let work = grade::resolve(arguments.work.unwrap_or_default())?;
    let key = current_grading::read_json(&PyPath::new(arguments.key.unwrap_or_default()))?;
    let inputs = work.join(b"validator/inputs.json");
    let legacy = text("legacy");
    let safety_policy = current_grading::read_json(&inputs)?;
    let safety_policy = pyvalue::get_or(&safety_policy, "safety_policy", &legacy)?;
    if !(is(safety_policy, "legacy") || is(safety_policy, grading_validation::NO_SAFETY)) {
        return Err(Failure::Inconsistent(
            b"the second grader requires an assessment prepared with --inventory".to_vec(),
        ));
    }
    let (run, target) = (at(&key, "run")?, at(&key, "target")?);
    let (run_text, target_text) = (string_of(run)?, string_of(target)?);
    let (directory, sample) = drawn_sample(root, current)?;
    let first = sampled_grade(&sample, root, run_text, target_text)?;
    let (selected, documents, fingerprint) =
        grade::batch_inputs(root, current, run_text, target_text, None)?;
    let fingerprint = Value::Str(fingerprint);
    let batch_name = format!("{}/{}", run.py_str(), target.py_str());
    if !fingerprint.py_eq(at(&key, "input_fingerprint")?) {
        return Err(Failure::Inconsistent(
            format!("{batch_name}: the batch's inputs changed since preparation; prepare it again")
                .into_bytes(),
        ));
    }
    if !at(&first, "input_fingerprint")?.py_eq(&fingerprint) {
        return Err(Failure::Inconsistent(
            format!(
                "{batch_name}: the sampled grade was made from other inputs; grade the batch again and draw a \
                 fresh sample"
            )
            .into_bytes(),
        ));
    }
    let first_receipt =
        current_grading::read_json(&root.resolve_pin(at(at(&first, "assessor")?, "receipt")?)?)?;
    let graded_by = at(at(&first_receipt, "provenance")?, "identity")?.clone();
    let (provenance, mut problems) = grade::provenance_of(&work, &key, arguments.assessor)?;
    if let Some(provenance) = &provenance
        && at(provenance, "identity")?.py_eq(&graded_by)
    {
        problems.push(format!(
            "{} produced the grade, so it cannot be the second assessor",
            graded_by.py_str()
        ));
    }
    let checked = grade::check_attempts(
        root,
        &selected,
        run_text,
        target_text,
        &[("the key".to_string(), grade::key_counts(&key)?)],
    )?;
    problems.extend(checked.problems);
    let checkout = PyPath::new(arguments.checkout);
    problems.extend(grade::check_prepared(
        &checkout,
        &work,
        &key,
        false,
        std::path::Path::new(""),
    )?);
    if !problems.is_empty() {
        return Err(Failure::Inconsistent(problems.join("\n").into_bytes()));
    }
    let provenance = provenance.expect("a missing provenance comes with a problem");
    let mut tokens: Vec<(Value, Value)> = Vec::new();
    for review in each(at(&key, "reviews")?)?.iter() {
        let attempt = at(review, "attempt_id")?.clone();
        pyvalue::hashable(&attempt)?;
        let token = at(review, "token")?.clone();
        match tokens.iter_mut().find(|(known, _)| known.py_eq(&attempt)) {
            Some(entry) => entry.1 = token,
            None => tokens.push((attempt, token)),
        }
    }
    let mut listed = Object::default();
    if let Value::Object(claims) = claim_list(&first)? {
        for (attempt, items) in claims.iter() {
            let attempt = text(attempt);
            let token = tokens
                .iter()
                .find(|(known, _)| known.py_eq(&attempt))
                .map(|(_, token)| token)
                .ok_or_else(|| missing(&attempt))?;
            listed.insert(string_of(token)?.to_string(), items.clone());
        }
    }
    if !pyvalue::get(&current_grading::read_json(&inputs)?, "inventory")?
        .py_eq(&Value::Object(listed))
    {
        return Err(Failure::Inconsistent(
            format!(
                "{batch_name}: the workspace was not prepared with the first grader's claim list; write it with \
                 the claims operation and prepare again with --inventory"
            )
            .into_bytes(),
        ));
    }
    let (raw, verdicts, problems) = grade::read_verdicts(&work)?;
    if !problems.is_empty() {
        return Err(Failure::Inconsistent(problems.join("\n").into_bytes()));
    }
    let batch = directory
        .join(b"second")
        .join(&name_of(run)?.into_bytes())
        .join(target_text.as_bytes());
    let out = grade::next_assessment(&batch)?;
    make_new_dirs(&out)?;
    let saved = (|| -> R<usize> {
        let verdicts_path = out.join(b"verdicts.json");
        pytext::write_bytes(&verdicts_path, &raw).map_err(Failure::Input)?;
        let evidence = [root.pin_file(&verdicts_path)?];
        let reference = each(at(documents.get("reference"), "targets")?)?
            .iter()
            .find(|reference| {
                pyvalue::get(reference, "target").is_ok_and(|named| named.py_eq(target))
            })
            .cloned()
            .ok_or_else(|| Failure::Crash("StopIteration".into()))?;
        let families = at(&reference, "families")?;
        let task = each(at(&selected, "tasks")?)?
            .iter()
            .find(|task| pyvalue::get(task, "id").is_ok_and(|id| id.py_eq(target)))
            .cloned()
            .ok_or_else(|| Failure::Crash("StopIteration".into()))?;
        let reviews = at(&key, "reviews")?;
        let raised = grade::raised_candidates(&verdicts, reviews, &checked.attempts, &task)?;
        let none = Value::Array(Vec::new());
        let pre_existing = if pyvalue::get(&first, "verdicts")?.py_eq(&text(pre_existing::CONTRACT))
        {
            Some(pyvalue::get_or(&reference, "pre_existing_problems", &none)?)
        } else {
            None
        };
        let mut graded = Vec::new();
        for entry in grade::sorted_by(reviews, "attempt_id")? {
            let verdict = pyvalue::subscript(at(&verdicts, "reviews")?, at(&entry, "token")?)?;
            let attempt = at(&entry, "attempt_id")?;
            let facts = checked
                .attempts
                .iter()
                .find(|(name, _)| is(attempt, name))
                .map(|(_, facts)| facts)
                .ok_or_else(|| missing(attempt))?;
            graded.push(grade::graded_v2(
                &entry,
                verdict,
                facts,
                families,
                &evidence,
                &Vec::new(),
                &[],
                &raised,
                pre_existing,
            )?);
        }
        let count = graded.len();
        let assessment_path = out.join(b"assessment.json");
        let assessment = object(vec![
            ("run", run.clone()),
            ("target", target.clone()),
            ("reviews", Value::Array(graded)),
        ]);
        pytext::write_bytes(
            &assessment_path,
            format!("{}\n", current_grading::dumps(&assessment)).as_bytes(),
        )
        .map_err(Failure::Input)?;
        let mut receipt = vec![
            ("contract", text("evaluator-audit-second/v2")),
            ("run", run.clone()),
            ("target", target.clone()),
            ("input_fingerprint", fingerprint.clone()),
            ("saved_at", now()),
            ("provenance", provenance.clone()),
            ("independent_of", graded_by.clone()),
            ("verdicts_sha256", text(&crate::sha256::hex_digest(&raw))),
            (
                "assessment_sha256",
                text(&current_grading::file_hash(&assessment_path)?),
            ),
            ("prepared_files", at(&key, "prepared_files")?.clone()),
            ("validator", at(&key, "validator")?.clone()),
            ("link_disputes", at(&verdicts, "link_disputes")?.clone()),
            ("new_candidates", at(&verdicts, "new_candidates")?.clone()),
            ("reviews", grade::key_reviews(&key)?),
        ];
        if let Some(policy) = key.as_object().and_then(|key| key.get("remedy_policy")) {
            receipt.push(("remedy_policy", policy.clone()));
        }
        let receipt_path = out.join(b"receipt.json");
        pytext::write_bytes(
            &receipt_path,
            format!("{}\n", current_grading::dumps(&object(receipt))).as_bytes(),
        )
        .map_err(Failure::Input)?;
        grade::prune_grading(root, target_text, &work, &receipt_path, SECOND_CLEANUP)?;
        Ok(count)
    })();
    let count = match saved {
        Ok(count) => count,
        Err(failure) => {
            let _ = std::fs::remove_dir_all(out.to_path_buf());
            return Err(failure);
        }
    };
    let relative = pyfs::relative_to(out.as_bytes(), root.path.as_bytes())
        .map_err(|message| Failure::Crash(format!("ValueError: {message}")))?;
    Ok(Done::Printed(format!(
        "saved the second assessment of {batch_name}: {count} reviews; evidence {}",
        pytext::stderr_text(&relative)
    )))
}

pub fn run(operation: &str, arguments: &Arguments) -> R<Done> {
    let root = Root::new(arguments.root)?;
    let current = arguments.current;
    match operation {
        "check" => check(&root, current),
        "draw" => draw(&root, current),
        "claims" => listed_claims(
            &root,
            current,
            arguments.run.unwrap_or_default(),
            arguments.target.unwrap_or_default(),
            arguments.out.unwrap_or_default(),
        ),
        "second" => second(&root, current, arguments),
        "compare" => compare(&root, current),
        "conclude" => conclude(&root, current, arguments.reconciliation.unwrap_or_default()),
        other => unreachable!("clap offers no operation {other}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_strata_are_the_python_units() {
        let credit = CREDIT_STRATA.map(|(_, stratum)| stratum);
        let claim = CLAIM_STRATA.map(|(_, stratum)| stratum);
        let pre_existing = PRE_EXISTING_STRATA.map(|(_, stratum)| stratum);
        let expected: Vec<(&str, &str)> = credit
            .iter()
            .chain(&IMPACT_STRATA)
            .map(|name| (*name, "family-review"))
            .chain(claim.iter().map(|name| (*name, "claim")))
            .chain([("control-silence", "control-review")])
            .chain(
                pre_existing
                    .iter()
                    .map(|name| (*name, "pre-existing-review")),
            )
            .collect();
        assert_eq!(UNITS.to_vec(), expected);
    }

    #[test]
    fn an_item_number_steps_at_any_length() {
        let cases = [
            ("0", "1", "-1"),
            ("9", "10", "8"),
            ("10", "11", "9"),
            ("-1", "0", "-2"),
            ("-10", "-9", "-11"),
            (
                "99999999999999999999999999999999999999999",
                "100000000000000000000000000000000000000000",
                "99999999999999999999999999999999999999998",
            ),
        ];
        for (number, up, down) in cases {
            assert_eq!(step(number, true), up, "{number} + 1");
            assert_eq!(step(number, false), down, "{number} - 1");
        }
    }

    #[test]
    fn a_slice_past_either_end_is_clamped() {
        let values: Vec<Value> = (0..3).map(int).collect();
        assert_eq!(head(&values, &int(5)).unwrap().len(), 3);
        assert_eq!(head(&values, &Value::Int("-1".into())).unwrap().len(), 2);
        assert_eq!(head(&values, &Value::Int("-9".into())).unwrap().len(), 0);
    }

    #[test]
    fn a_directory_holds_itself_and_its_descendants() {
        assert!(holds(b"/a/b", b"/a/b"));
        assert!(holds(b"/a/b", b"/a/b/c"));
        assert!(!holds(b"/a/b", b"/a/bc"));
        assert!(holds(b"/", b"/x"));
    }
}
