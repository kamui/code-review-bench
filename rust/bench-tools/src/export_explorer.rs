//! The explorer export that `tools/export_explorer.py` wrote: per-review facts, their detail files and the
//! evidence they link, written from the current facts, checked as one export, then moved into `public/`.
//!
//! Values keep Python's semantics where an output depends on them: `dict` and `set` membership as `==` and
//! hashing decide it, iteration over the container `json.loads` returned, truthiness, `sum` as Python 3.14
//! adds, and `datetime.fromisoformat` as Python 3.14 parses. A failure is the last line of the traceback
//! `export_explorer.py` printed, exiting 1 as it did.

use crate::pyjson::{self, Object, Value};
use crate::pysemantics::{
    Dict, Failure, Result, at, concat, contains, count, file_failure, fromisoformat, get, get_or,
    index, iterate, join, join_text, length, loads, lookup, move_failure, number, number_value,
    object, optional, or, path_of, python_sum, read, read_bytes, read_text, set_of, sorted,
    str_text, text, unhashable,
};
use crate::pytext::{self, PyPath};
use crate::{current_grading, pyfs, sha256};
use std::collections::HashSet;
use std::ffi::OsStr;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

fn inconsistent(message: impl Into<String>) -> Failure {
    Failure::new("current_grading.Inconsistent", message)
}

fn evidence_error(message: impl Into<String>) -> Failure {
    Failure::new("evidence_store.EvidenceError", message)
}

fn os(path: &[u8]) -> &Path {
    Path::new(OsStr::from_bytes(path))
}

fn dump(value: &Value) -> String {
    let options = pyjson::Dumps {
        ensure_ascii: false,
        ..pyjson::Dumps::default()
    };
    format!("{}\n", pyjson::dumps(value, &options))
}

fn write_text(path: &PyPath, text: &str) -> Result<()> {
    std::fs::write(path.to_path_buf(), text).map_err(|error| file_failure(&error, path.as_bytes()))
}

fn parent(path: &PyPath) -> PyPath {
    let bytes = path.as_bytes();
    match bytes.iter().rposition(|byte| *byte == b'/') {
        Some(0) => PyPath::new(b"/"),
        Some(cut) => PyPath::new(&bytes[..cut]),
        None => PyPath::new(b"."),
    }
}

/// `path.name`.
fn name(path: &PyPath) -> Value {
    let bytes = path.as_bytes();
    let last = bytes
        .rsplit(|byte| *byte == b'/')
        .next()
        .unwrap_or_default();
    let last = if last == b"." { b"".as_slice() } else { last };
    Value::Str(String::from_utf8_lossy(last).into_owned())
}

/// `Path.mkdir(parents=True, exist_ok=True)`.
fn make_directories(path: &PyPath) -> Result<()> {
    std::fs::create_dir_all(path.to_path_buf())
        .map_err(|error| file_failure(&error, path.as_bytes()))
}

fn exists(path: &PyPath) -> bool {
    path.to_path_buf().exists()
}

/// `shutil.rmtree(path)`, which refuses a symbolic link.
fn remove_tree(path: &PyPath) -> Result<()> {
    let target = path.to_path_buf();
    if std::fs::symlink_metadata(&target).is_ok_and(|meta| meta.file_type().is_symlink()) {
        return Err(Failure::new(
            "OSError",
            "Cannot call rmtree on a symbolic link",
        ));
    }
    std::fs::remove_dir_all(&target).map_err(|error| file_failure(&error, path.as_bytes()))
}

/// `shutil.rmtree(path, ignore_errors=True)`.
fn remove_tree_quietly(path: &PyPath) {
    let target = path.to_path_buf();
    if std::fs::symlink_metadata(&target).is_ok_and(|meta| meta.is_dir()) {
        let _ = std::fs::remove_dir_all(&target);
    }
}

/// `any(path.iterdir())`.
fn has_entries(path: &PyPath) -> Result<bool> {
    let mut entries = std::fs::read_dir(path.to_path_buf())
        .map_err(|error| file_failure(&error, path.as_bytes()))?;
    Ok(entries.next().is_some())
}

/// `shutil.copyfile(source, destination)`: the bytes only, never the mode.
fn copy_file(source: &PyPath, destination: &PyPath) -> Result<()> {
    let mut input = std::fs::File::open(source.to_path_buf())
        .map_err(|error| file_failure(&error, source.as_bytes()))?;
    let mut output = std::fs::File::create(destination.to_path_buf())
        .map_err(|error| file_failure(&error, destination.as_bytes()))?;
    io::copy(&mut input, &mut output)
        .map_err(|error| file_failure(&error, destination.as_bytes()))?;
    Ok(())
}

/// Copies or lists the evidence each exported link names, as `export_explorer.evidence` did.
struct Exporter {
    root: PyPath,
    bench: PyPath,
    base: String,
    stage: Option<PyPath>,
    listed: Vec<Value>,
    seen: HashSet<String>,
    copied: HashSet<Vec<u8>>,
}

impl Exporter {
    fn new(root: &[u8], base_path: &str, stage: Option<&[u8]>) -> Self {
        let root = PyPath::new(root);
        Self {
            bench: root.join(b"bench"),
            root,
            base: base_path.trim_end_matches('/').to_string(),
            stage: stage.map(PyPath::new),
            listed: Vec::new(),
            seen: HashSet::new(),
            copied: HashSet::new(),
        }
    }

    fn evidence(&mut self, path: &PyPath, sha256: Option<&Value>) -> Result<Value> {
        let relative = pyfs::relative_to(path.as_bytes(), self.root.as_bytes())
            .map_err(|message| Failure::new("ValueError", message))?;
        let name = String::from_utf8(relative.clone()).map_err(|_| {
            Failure::new(
                "UnicodeEncodeError",
                format!(
                    "'utf-8' codec can't encode {}: surrogates not allowed",
                    pytext::stderr_text(&relative)
                ),
            )
        })?;
        let sha256 = sha256.filter(|value| !matches!(value, Value::Null));
        match &self.stage {
            None => {
                let pin = sha256.cloned().unwrap_or(Value::Null);
                if self.seen.insert(format!(
                    "{name}\0{}",
                    pyjson::dumps(&pin, &pyjson::Dumps::default())
                )) {
                    self.listed
                        .push(object(vec![("path", text(&name)), ("sha256", pin)]));
                }
            }
            Some(stage) => {
                if !path.is_file() {
                    return Err(evidence_error(format!(
                        "missing or changed evidence: {name}"
                    )));
                }
                if let Some(expected) = sha256
                    && !Value::Str(sha256::hex_digest(&read_bytes(path)?)).py_eq(expected)
                {
                    return Err(evidence_error(format!("changed evidence: {name}")));
                }
                let destination = stage.join(b"evidence").join(&relative);
                if self.copied.insert(destination.as_bytes().to_vec()) {
                    make_directories(&parent(&destination))?;
                    copy_file(path, &destination)?;
                }
            }
        }
        Ok(Value::Str(format!("{}/evidence/{name}", self.base)))
    }

    /// The saved receipt of the decision a record names; a decision without a receipt links nothing.
    fn ruling(&mut self, decisions: &Dict<Value>, identifier: &Value) -> Result<Value> {
        if !identifier.truthy() {
            return Ok(Value::Null);
        }
        let decision = decisions
            .get(identifier)?
            .ok_or_else(|| Failure::new("KeyError", identifier.py_repr()))?;
        let receipt = at(decision, "receipt")?;
        if !receipt.truthy() {
            return Ok(Value::Null);
        }
        let path = join(&self.root, at(receipt, "path")?)?;
        self.evidence(&path, None)
    }
}

/// `usage_tokens`: the summed output tokens of complete metering, or null when any count is unavailable.
fn usage_tokens(path: &PyPath, record: &Value) -> Result<Value> {
    if !path.is_file() {
        return Ok(Value::Null);
    }
    let usage = get_or(record, "usage", Value::Object(Object::default()))?;
    if !optional(get(&usage, "metering_status")?).py_eq(&text("complete")) {
        return Ok(Value::Null);
    }
    let content = read_text(path)?;
    let mut lines = Vec::new();
    for line in pytext::splitlines(&content) {
        if !pytext::strip(line).is_empty() {
            lines.push(loads(line)?);
        }
    }
    let mut counts = Vec::with_capacity(lines.len());
    for line in &lines {
        match get(line, "output_tokens")?.and_then(number) {
            Some(value) => counts.push(value),
            None => return Ok(Value::Null),
        }
    }
    if lines.is_empty() {
        return Ok(Value::Null);
    }
    number_value(python_sum(&counts)?)
}

/// `duration_seconds`: the reviewer's elapsed seconds between dispatch and its filed end event.
fn duration_seconds(record: &Value) -> Result<Value> {
    let timing = get_or(record, "timing", Value::Object(Object::default()))?;
    let start = optional(get(&timing, "dispatched_at")?);
    let end = if at(record, "disposition")?.py_eq(&text("valid completed")) {
        optional(get(&timing, "completed_at")?)
    } else {
        optional(get(&timing, "stopped_at")?)
    };
    let (Value::Str(start), Value::Str(end)) = (start, end) else {
        return Ok(Value::Null);
    };
    let Some((start, start_aware)) = fromisoformat(&start.replace('Z', "+00:00")) else {
        return Ok(Value::Null);
    };
    let Some((end, end_aware)) = fromisoformat(&end.replace('Z', "+00:00")) else {
        return Ok(Value::Null);
    };
    if !start_aware || !end_aware {
        return Ok(Value::Null);
    }
    let micros = end - start;
    if micros < 0 {
        return Ok(Value::Null);
    }
    let seconds: f64 = format!("{}.{:06}", micros / 1_000_000, micros % 1_000_000)
        .parse()
        .unwrap_or(f64::NAN);
    Ok(Value::Float(seconds))
}

const BUILTIN_METHODS: [&str; 2] = ["codex", "claude-builtin"];

/// `value in {"codex", "claude-builtin"}`.
fn builtin(value: &Value) -> Result<bool> {
    if !value.hashable() {
        return Err(unhashable(value));
    }
    Ok(BUILTIN_METHODS
        .iter()
        .any(|method| value.py_eq(&text(method))))
}

const V2_OUTCOMES: [(&str, &str); 5] = [
    ("problem", "eligible"),
    ("unproven", "unsupported"),
    ("outside-this-change", "scope-excluded"),
    ("suggestion", "advisory"),
    ("minor-defect", "inconsequential"),
];

fn projected_problems(claim: &Value, key: &str, identifier: &str, camel: &str) -> Result<Value> {
    let entries = get_or(claim, key, Value::Array(Vec::new()))?;
    let mut projected = Vec::new();
    for entry in iterate(&entries)?.iter() {
        projected.push(object(vec![
            (camel, at(entry, identifier)?.clone()),
            ("saysWhat", at(entry, "says_what")?.clone()),
            ("identifiesCause", at(entry, "identifies_cause")?.clone()),
            ("reason", at(entry, "reason")?.clone()),
        ]));
    }
    Ok(Value::Array(projected))
}

/// `projected_claim`.
fn projected_claim(claim: &Value) -> Result<Value> {
    let identifier = at(claim, "id")?.clone();
    let item = at(at(claim, "anchor")?, "item_id")?.clone();
    let native = at(claim, "outcome")?;
    if !native.hashable() {
        return Err(unhashable(native));
    }
    let outcome = V2_OUTCOMES
        .iter()
        .find(|(name, _)| native.py_eq(&text(name)))
        .map_or_else(|| native.clone(), |(_, category)| text(category));
    Ok(object(vec![
        ("id", identifier),
        ("itemId", item),
        ("outcome", outcome),
        ("nativeOutcome", native.clone()),
        ("kind", optional(get(claim, "kind")?)),
        ("familyId", optional(get(claim, "family_id")?)),
        ("canonicalId", at(claim, "canonical_id")?.clone()),
        ("duplicateGroup", at(claim, "duplicate_group")?.clone()),
        (
            "knownProblems",
            projected_problems(claim, "known_problems", "family_id", "familyId")?,
        ),
        (
            "preExistingProblems",
            projected_problems(claim, "pre_existing_problems", "problem_id", "problemId")?,
        ),
        (
            "preExistingCandidateId",
            optional(get(claim, "pre_existing_candidate_id")?),
        ),
    ]))
}

fn each<F>(value: &Value, mut project: F) -> Result<Value>
where
    F: FnMut(&Value) -> Result<Value>,
{
    let mut out = Vec::new();
    for item in iterate(value)?.iter() {
        out.push(project(item)?);
    }
    Ok(Value::Array(out))
}

/// `assessment`: one saved grade record, its judgments and eligibility as recorded.
fn assessment(grade: &Value) -> Result<Value> {
    let families = each(at(grade, "families")?, |family| {
        Ok(object(vec![
            ("familyId", at(family, "family_id")?.clone()),
            ("outcome", at(family, "outcome")?.clone()),
            ("sufficiency", at(family, "sufficiency")?.clone()),
            ("claimIds", at(family, "claim_ids")?.clone()),
            (
                "causeOnly",
                get_or(family, "cause_only", Value::Bool(false))?,
            ),
        ]))
    })?;
    let claims = each(at(grade, "claims")?, projected_claim)?;
    let pre_existing = if contains(grade, "pre_existing_problems")? {
        each(at(grade, "pre_existing_problems")?, |entry| {
            Ok(object(vec![
                ("problemId", at(entry, "problem_id")?.clone()),
                ("outcome", at(entry, "outcome")?.clone()),
                ("claimIds", at(entry, "claim_ids")?.clone()),
                ("saysWhat", at(entry, "says_what")?.clone()),
                ("identifiesCause", at(entry, "identifies_cause")?.clone()),
            ]))
        })?
    } else {
        Value::Null
    };
    let recommendations = each(at(grade, "recommendations")?, |recommendation| {
        Ok(object(vec![
            ("id", at(recommendation, "id")?.clone()),
            (
                "addressedClaims",
                at(recommendation, "addressed_claims")?.clone(),
            ),
            (
                "safety",
                at(at(recommendation, "safety")?, "state")?.clone(),
            ),
            (
                "sufficiency",
                each(at(recommendation, "sufficiency")?, |entry| {
                    Ok(object(vec![
                        ("familyId", at(entry, "family_id")?.clone()),
                        ("outcome", at(entry, "outcome")?.clone()),
                    ]))
                })?,
            ),
        ]))
    })?;
    let remedy = at(at(grade, "remedy_inventory")?, "state")?.clone();
    let advice = each(at(grade, "advice")?, |entry| {
        Ok(object(vec![
            ("id", at(entry, "id")?.clone()),
            ("claimIds", at(entry, "claim_ids")?.clone()),
            ("kind", at(entry, "kind")?.clone()),
            ("benefit", at(entry, "benefit")?.clone()),
            ("sample", at(entry, "sample")?.clone()),
        ]))
    })?;
    Ok(object(vec![
        ("state", at(grade, "state")?.clone()),
        ("families", families),
        ("claims", claims),
        ("preExistingProblems", pre_existing),
        ("recommendations", recommendations),
        ("remedyInventory", remedy),
        ("advice", advice),
    ]))
}

/// A billing receipt that replaces the attempts' recorded billing.
struct Correction {
    receipt: PyPath,
    billing: Value,
}

/// A saved grade and the assessor of its batch.
#[derive(Clone)]
struct Graded {
    review: Value,
    assessor: Value,
}

struct Lookups {
    archives: Dict<Value>,
    rulings: Dict<Value>,
    lower_bounds: Option<LowerBounds>,
}

/// The usage deviation that exports some filed costs as lower bounds, with its entries by attempt identifier.
struct LowerBounds {
    receipt: PyPath,
    attempts: Dict<Value>,
}

const LOWER_BOUNDS: &[u8] = b"deviations/usage-lower-bounds.v1.json";

impl LowerBounds {
    fn read(bench: &PyPath) -> Result<Option<Self>> {
        let receipt = bench.join(LOWER_BOUNDS);
        if !exists(&receipt) {
            return Ok(None);
        }
        let mut attempts = Dict::new();
        for entry in iterate(at(&read(&receipt)?, "attempts")?)?.iter() {
            if at(entry, "probe")?.truthy() {
                continue;
            }
            let identifier = concat(
                &concat(at(entry, "run")?, &text("/"))?,
                at(entry, "attempt")?,
            )?;
            attempts.insert(identifier, entry.clone())?;
        }
        Ok(Some(Self { receipt, attempts }))
    }

    /// The entry for an attempt, refused when the filed record no longer holds the cost it names.
    fn entry(&self, identifier: &Value, usage: &Value) -> Result<Option<&Value>> {
        let Some(entry) = self.attempts.get(identifier)? else {
            return Ok(None);
        };
        let filed = optional(get(usage, "priced_total_usd")?);
        if !optional(get(usage, "metering_status")?).py_eq(&text("complete"))
            || !filed.py_eq(at(entry, "filed_cost_usd")?)
        {
            return Err(inconsistent(format!(
                "{}: the usage deviation names another filed cost",
                identifier.py_str()
            )));
        }
        Ok(Some(entry))
    }
}

impl Exporter {
    /// `export_attempt`: one attempt's detail file and its summary in the dataset.
    fn export_attempt(
        &mut self,
        run: &PyPath,
        facts: &Value,
        lookups: &Lookups,
        graded: Option<&Graded>,
        correction: Option<&Correction>,
    ) -> Result<Value> {
        let identifier = at(facts, "id")?;
        let attempt_id = str_text(identifier)?
            .split('/')
            .nth(1)
            .ok_or_else(|| Failure::new("IndexError", "list index out of range"))?
            .to_string();
        let directory = run.join(b"attempts").join(attempt_id.as_bytes());
        let record_path = directory.join(b"attempt.json");
        let record = read(&record_path)?;
        let normalized_path = directory.join(b"normalized.json");
        let normalized = if exists(&normalized_path) {
            read(&normalized_path)?
        } else {
            object(vec![("items", Value::Array(Vec::new()))])
        };
        let grade = graded
            .map(|graded| &graded.review)
            .filter(|grade| grade.truthy());
        let mut sufficiency = Dict::new();
        if let Some(grade) = grade {
            for family in iterate(at(grade, "families")?)?.iter() {
                let key = at(family, "family_id")?.clone();
                let value = at(family, "sufficiency")?.clone();
                sufficiency.insert(key, value)?;
            }
        }
        let unassessed = text("unassessed");
        let fix_sufficiency = |key: &Value| -> Result<Value> {
            Ok(sufficiency
                .get(key)?
                .cloned()
                .unwrap_or_else(|| unassessed.clone()))
        };
        let no_claims = Value::Array(Vec::new());
        let grade_claims = match grade {
            Some(grade) => at(grade, "claims")?,
            None => &no_claims,
        };
        let mut items = Vec::new();
        for (position, item) in iterate(&get_or(&normalized, "items", Value::Array(Vec::new()))?)?
            .iter()
            .enumerate()
        {
            let anchor = text(&format!("item-{position}"));
            let mut claims = Vec::new();
            let mut groups = Dict::new();
            for claim in iterate(grade_claims)?.iter() {
                if !at(at(claim, "anchor")?, "item_id")?.py_eq(&anchor) {
                    continue;
                }
                let canonical = at(claim, "canonical_id")?;
                let mut evidence = Vec::new();
                for entry in iterate(at(claim, "evidence")?)?.iter() {
                    evidence.push(at(entry, "path")?.clone());
                }
                let mut known = Dict::new();
                for entry in
                    iterate(&get_or(claim, "known_problems", Value::Array(Vec::new()))?)?.iter()
                {
                    known.insert(fix_sufficiency(at(entry, "family_id")?)?, ())?;
                }
                let joined = join_text(&sorted(known.keys().cloned().collect())?)?;
                let claim_sufficiency = if joined.is_empty() {
                    fix_sufficiency(&optional(get(claim, "family_id")?))?
                } else {
                    Value::Str(joined)
                };
                let projected = projected_claim(claim)?;
                let group = at(claim, "duplicate_group")?.clone();
                claims.push(object(vec![
                    ("id", at(claim, "id")?.clone()),
                    ("quote", at(at(claim, "anchor")?, "quote")?.clone()),
                    ("assignment", at(claim, "outcome")?.clone()),
                    ("canonical_claim_id", canonical.clone()),
                    ("rulingUrl", optional(lookups.rulings.get(canonical)?)),
                    ("notes", at(claim, "reason")?.clone()),
                    ("evidence", Value::Array(evidence)),
                    ("fix_sufficiency", claim_sufficiency),
                    ("knownProblems", at(&projected, "knownProblems")?.clone()),
                    (
                        "preExistingProblems",
                        at(&projected, "preExistingProblems")?.clone(),
                    ),
                    (
                        "preExistingCandidateId",
                        optional(get(claim, "pre_existing_candidate_id")?),
                    ),
                ]));
                groups.insert(group, ())?;
            }
            let groups: Vec<&Value> = groups
                .keys()
                .filter(|group| !matches!(group, Value::Null))
                .collect();
            let assignments = set_of(
                claims
                    .iter()
                    .map(|claim| at(claim, "assignment"))
                    .collect::<Result<Vec<_>>>()?,
            )?;
            let sufficiencies = set_of(
                claims
                    .iter()
                    .map(|claim| at(claim, "fix_sufficiency"))
                    .collect::<Result<Vec<_>>>()?,
            )?;
            let assignment = join_text(&sorted(assignments.keys().cloned().collect())?)?;
            let duplicate = if groups.len() == 1 {
                groups[0].clone()
            } else {
                Value::Null
            };
            let fixes = join_text(&sorted(sufficiencies.keys().cloned().collect())?)?;
            let notes = match grade {
                Some(grade) if !claims.is_empty() => at(grade, "reason")?.clone(),
                _ => text("Current judgment is unavailable."),
            };
            items.push(object(vec![
                ("id", anchor),
                ("claim", or(get(item, "claim")?, text("Untitled finding"))),
                ("consequence", or(get(item, "consequence")?, text(""))),
                ("file", or(get(item, "file")?, text(""))),
                ("line", optional(get(item, "line_start")?)),
                ("proposedFix", optional(get(item, "proposed_fix")?)),
                (
                    "assignment",
                    if assignment.is_empty() {
                        unassessed.clone()
                    } else {
                        Value::Str(assignment)
                    },
                ),
                ("duplicateGroup", duplicate),
                (
                    "fixSufficiency",
                    if fixes.is_empty() {
                        unassessed.clone()
                    } else {
                        Value::Str(fixes)
                    },
                ),
                ("notes", notes),
                ("claims", Value::Array(claims)),
            ]));
        }
        let record_name = pyfs::relative_to(record_path.as_bytes(), self.root.as_bytes())
            .map_err(|message| Failure::new("ValueError", message))?;
        let archive = lookups
            .archives
            .get(&Value::Str(
                String::from_utf8_lossy(&record_name).into_owned(),
            ))?
            .filter(|archive| archive.truthy());
        let archive_url = match archive {
            Some(archive) if at(archive, "status")?.py_eq(&text("verified")) => {
                let path = join(&self.root, at(archive, "path")?)?;
                self.evidence(&path, Some(at(archive, "sha256")?))?
            }
            _ => Value::Null,
        };
        let stop_path = directory.join(b"stop.json");
        let billing_url = match correction {
            Some(correction) => self.evidence(&correction.receipt, None)?,
            None => Value::Null,
        };
        let usage = at(&record, "usage")?;
        let lower_bound = match &lookups.lower_bounds {
            Some(bounds) => bounds.entry(identifier, usage)?.map(|_| &bounds.receipt),
            None => None,
        };
        let usage_url = match lower_bound {
            Some(receipt) => self.evidence(receipt, None)?,
            None => Value::Null,
        };
        let stop = if exists(&stop_path) {
            read(&stop_path)?
        } else {
            Value::Null
        };
        let assessor = graded
            .map(|graded| &graded.assessor)
            .filter(|assessor| assessor.truthy());
        let state = match grade {
            Some(grade) => at(grade, "state")?.clone(),
            None => unassessed.clone(),
        };
        let (receipt_url, verdicts_url) = match assessor {
            Some(assessor) => {
                let receipt = join(&self.root, at(at(assessor, "receipt")?, "path")?)?;
                let receipt = self.evidence(&receipt, None)?;
                let verdicts = join(&self.root, at(at(assessor, "verdicts")?, "path")?)?;
                (receipt, self.evidence(&verdicts, None)?)
            }
            None => (Value::Null, Value::Null),
        };
        let record_url = self.evidence(&record_path, None)?;
        let normalized_url = if exists(&normalized_path) {
            self.evidence(&normalized_path, None)?
        } else {
            Value::Null
        };
        let archive_status = match archive {
            Some(archive) => at(archive, "status")?.clone(),
            None => text("missing"),
        };
        let observed = count(items.len());
        let detail = object(vec![
            ("id", identifier.clone()),
            ("items", Value::Array(items)),
            ("record", record.clone()),
            ("billingCorrectionUrl", billing_url),
            ("usageCorrectionUrl", usage_url),
            ("stop", stop),
            (
                "assessment",
                object(vec![
                    ("state", state),
                    ("receiptUrl", receipt_url),
                    ("verdictsUrl", verdicts_url),
                ]),
            ),
            ("recordUrl", record_url),
            ("normalizedUrl", normalized_url),
            ("archiveUrl", archive_url),
            ("archiveStatus", archive_status),
        ]);
        let run_name = name(run);
        if let Some(stage) = &self.stage {
            let detail_path = stage
                .join(b"data/attempts")
                .join(run_name.as_str().unwrap_or_default().as_bytes())
                .join(format!("{attempt_id}.json").as_bytes());
            make_directories(&parent(&detail_path))?;
            write_text(&detail_path, &dump(&detail))?;
        }
        let cell = at(&record, "cell")?;
        let requests = join(&directory, at(usage, "requests")?)?;
        let (cost, output_tokens) = if lower_bound.is_some() {
            (Value::Null, Value::Null)
        } else {
            (
                optional(get(usage, "priced_total_usd")?),
                usage_tokens(&requests, &record)?,
            )
        };
        let predecessor = match get(&record, "predecessor")? {
            Some(previous) if previous.truthy() => {
                concat(&concat(&run_name, &text("/"))?, previous)?
            }
            _ => Value::Null,
        };
        Ok(object(vec![
            ("id", identifier.clone()),
            ("label", Value::Str(attempt_id)),
            ("runId", run_name),
            ("taskId", at(cell, "target")?.clone()),
            ("replicate", at(cell, "replicate")?.clone()),
            ("disposition", at(&record, "disposition")?.clone()),
            (
                "complete",
                Value::Bool(at(at(facts, "completion")?, "state")?.py_eq(&text("complete"))),
            ),
            (
                "admitted",
                Value::Bool(at(at(facts, "admission")?, "state")?.py_eq(&text("admitted"))),
            ),
            ("observedItems", observed),
            (
                "assessment",
                match grade {
                    Some(grade) => assessment(grade)?,
                    None => Value::Null,
                },
            ),
            ("cost", cost),
            (
                "costLowerBound",
                at(at(usage, "cost_bounds_usd")?, "low")?.clone(),
            ),
            ("outputTokens", output_tokens),
            ("durationSeconds", duration_seconds(&record)?),
            (
                "billing",
                match correction {
                    Some(correction) => correction.billing.clone(),
                    None => at(usage, "billing")?.clone(),
                },
            ),
            ("predecessor", predecessor),
            ("retryReason", optional(get(&record, "retry_reason")?)),
            (
                "detailUrl",
                concat(
                    &text(&format!("{}/data/attempts/", self.base)),
                    &concat(identifier, &text(".json"))?,
                )?,
            ),
        ]))
    }

    /// `skill_provenance.verify_pin`: the frozen release record a run's arm pins, if any.
    fn verify_pin(run: &PyPath, arm: &Value) -> Result<Option<Value>> {
        let Some(pin) = get(arm, "skill_provenance")?.filter(|pin| !matches!(pin, Value::Null))
        else {
            return Ok(None);
        };
        let path = PyPath::new(&pyfs::resolve(join(run, at(pin, "path")?)?.as_bytes()));
        let base = pyfs::resolve(run.as_bytes());
        if pyfs::relative_parts(path.as_bytes(), &base).is_none() {
            return Err(Failure::new(
                "ValueError",
                "skill provenance must be inside the run directory",
            ));
        }
        if !Value::Str(sha256::hex_digest(&read_bytes(&path)?)).py_eq(at(pin, "sha256")?) {
            return Err(Failure::new(
                "ValueError",
                "skill provenance no longer matches its frozen hash",
            ));
        }
        let record = read(&path)?;
        if !at(&record, "benchmark_tree")?.py_eq(at(arm, "resolved_skill_tree")?) {
            return Err(Failure::new(
                "ValueError",
                "skill provenance identifies a different benchmark tree",
            ));
        }
        Ok(Some(record))
    }

    /// `skill_releases`: the distinct skill releases a configuration's runs used.
    fn skill_releases(
        &mut self,
        entry: &Value,
        sources: &[Value],
        recovered: &Value,
    ) -> Result<Value> {
        let mut records: Vec<(Value, Value)> = Vec::new();
        for source in sources {
            let run_dir = join(&self.bench, at(source, "run")?)?;
            let manifest = read(&run_dir.join(b"manifest.json"))?;
            let wanted = at(source, "arm")?;
            let mut arm = None;
            for candidate in iterate(at(&manifest, "arms")?)?.iter() {
                if at(candidate, "id")?.py_eq(wanted) {
                    arm = Some(candidate.clone());
                    break;
                }
            }
            let arm = arm.ok_or_else(|| Failure::new("StopIteration", ""))?;
            let pinned = Self::verify_pin(&run_dir, &arm)?;
            let pinned_truthy = pinned.as_ref().is_some_and(Value::truthy);
            let provenance_path = if pinned_truthy {
                join(&run_dir, at(at(&arm, "skill_provenance")?, "path")?)?
            } else {
                self.bench.join(b"skill-provenance.v2.json")
            };
            let record = match pinned {
                Some(record) if record.truthy() => Some(record),
                _ => lookup(recovered, at(&arm, "resolved_skill_tree")?)?.cloned(),
            };
            let Some(record) = record.filter(Value::truthy) else {
                continue;
            };
            let version = at(&record, "version")?.clone();
            let date = at(&record, "date")?.clone();
            let date_source = at(&record, "date_source")?.clone();
            let url = self.evidence(&provenance_path, None)?;
            for part in [&version, &date, &date_source] {
                if !part.hashable() {
                    return Err(unhashable(part));
                }
            }
            let key = Value::Array(vec![version.clone(), date.clone(), date_source.clone()]);
            let release = object(vec![
                ("version", version),
                ("date", date),
                ("dateSource", date_source),
                ("provenanceUrl", url),
            ]);
            match records.iter_mut().find(|(known, _)| known.py_eq(&key)) {
                Some((_, value)) => *value = release,
                None => records.push((key, release)),
            }
        }
        if !builtin(at(entry, "method")?)? && records.is_empty() {
            return Err(Failure::new(
                "ValueError",
                format!(
                    "{}: skill release provenance is missing",
                    at(entry, "id")?.py_str()
                ),
            ));
        }
        Ok(Value::Array(
            records.into_iter().map(|(_, release)| release).collect(),
        ))
    }
}

/// `recorded(values)`: the distinct recorded values as text, or `unrecorded`.
fn recorded(values: Vec<Value>) -> Result<Value> {
    let mut set = Dict::new();
    for value in values {
        if !matches!(value, Value::Null) {
            set.insert(Value::Str(value.py_str()), ())?;
        }
    }
    let names = sorted(set.keys().cloned().collect())?;
    Ok(Value::Array(if names.is_empty() {
        vec![text("unrecorded")]
    } else {
        names
    }))
}

/// `conditions`: the recorded comparison conditions of a configuration.
fn conditions(
    arms: &[Value],
    records: &[Value],
    effort: &Value,
    billing: &Dict<()>,
) -> Result<Value> {
    let mut isolation = Vec::new();
    for arm in arms {
        isolation.push(get_or(arm, "isolation", Value::Object(Object::default()))?);
    }
    let mut clients = Vec::new();
    for record in records {
        let observed = at(record, "observed")?;
        clients.push(Value::Str(format!(
            "{} {}",
            at(observed, "harness")?.py_str(),
            at(observed, "cli_version")?.py_str()
        )));
    }
    let field = |name: &str| -> Result<Vec<Value>> {
        isolation
            .iter()
            .map(|entry| Ok(optional(get(entry, name)?)))
            .collect()
    };
    let mut safe = Vec::new();
    for value in field("safe_mode")? {
        if !value.hashable() {
            return Err(unhashable(&value));
        }
        safe.push(if value.py_eq(&Value::Bool(true)) {
            text("on")
        } else if value.py_eq(&Value::Bool(false)) {
            text("off")
        } else {
            Value::Null
        });
    }
    let rows = vec![
        ("Client", recorded(clients)?),
        ("Reasoning effort", recorded(vec![effort.clone()])?),
        ("Network access", recorded(field("network")?)?),
        ("Sandbox", recorded(field("sandbox")?)?),
        ("Safe mode", recorded(safe)?),
        (
            "Billing basis",
            recorded(billing.keys().cloned().collect())?,
        ),
    ];
    Ok(Value::Array(
        rows.into_iter()
            .map(|(name, values)| object(vec![("name", text(name)), ("values", values)]))
            .collect(),
    ))
}

/// The `transcripts.json` files of `bench/runs/*`, sorted as `pathlib` paths sort, by their parts.
fn transcript_files(bench: &PyPath) -> Vec<PyPath> {
    let runs = bench.join(b"runs");
    let mut paths: Vec<PyPath> = pyfs::names(runs.as_bytes())
        .into_iter()
        .map(|name| runs.join(&name))
        .filter(|directory| pyfs::is_dir(directory.as_bytes()))
        .map(|directory| directory.join(b"transcripts.json"))
        .filter(|path| pyfs::lexists(path.as_bytes()))
        .collect();
    let parts = |path: &PyPath| -> Vec<Vec<u32>> {
        path.as_bytes()
            .split(|byte| *byte == b'/')
            .map(pytext::surrogateescape)
            .collect()
    };
    paths.sort_by_cached_key(parts);
    paths
}

/// The current facts: `load_current`'s selection and documents and their coverage.
pub struct Facts {
    selected: Value,
    documents: Value,
    status: Value,
}

impl Facts {
    /// The facts a file holds, as the removed Python front wrote them for `evidence` and `stage`.
    pub fn read(path: &[u8]) -> std::result::Result<Self, Failure> {
        Self::of(&read(&PyPath::new(path))?)
    }

    fn of(facts: &Value) -> std::result::Result<Self, Failure> {
        Ok(Self {
            selected: at(facts, "selected")?.clone(),
            documents: at(facts, "documents")?.clone(),
            status: at(facts, "status")?.clone(),
        })
    }

    /// The facts passed through `json.dumps(allow_nan=False)` and `json.loads`, as the Python front's temporary
    /// file passed them, so a non-finite number, or text that is not UTF-8, is refused with the same message.
    fn through_file(facts: &Value) -> std::result::Result<Self, Failure> {
        if let Some(number) = non_finite(facts) {
            return Err(Failure::new(
                "ValueError",
                format!("Out of range float values are not JSON compliant: {number}"),
            ));
        }
        Self::of(&loads(&pyjson::dumps(facts, &pyjson::Dumps::default()))?)
    }
}

/// The first non-finite number in the order `json.dumps` writes the value, spelled as `float.__repr__` spells it.
fn non_finite(value: &Value) -> Option<&'static str> {
    match value {
        Value::Float(number) if number.is_nan() => Some("nan"),
        Value::Float(number) if number.is_infinite() => Some(if number.is_sign_positive() {
            "inf"
        } else {
            "-inf"
        }),
        Value::Array(values) => values.iter().find_map(non_finite),
        Value::Object(object) => object.iter().find_map(|(_, member)| non_finite(member)),
        _ => None,
    }
}

impl Exporter {
    /// `write_export`: the dataset, with every detail file and linked evidence staged when staging.
    #[allow(clippy::too_many_lines)]
    fn write_export(&mut self, facts: &Facts) -> Result<Value> {
        let (selected, documents, status) = (&facts.selected, &facts.documents, &facts.status);
        let registry = read(&self.bench.join(b"scoreboard.current.json"))?;
        let profiles = read(&self.bench.join(b"profiles.json"))?;
        let imported = read(&self.bench.join(b"import-manifest.json"))?;
        let mut archives = Dict::new();
        for archive in iterate(at(&imported, "transcripts")?)?.iter() {
            archives.insert(at(archive, "attempt")?.clone(), archive.clone())?;
        }
        for path in transcript_files(&self.bench) {
            let listed = read(&path)?;
            for archive in iterate(&listed)?.iter() {
                archives.insert(at(archive, "attempt")?.clone(), archive.clone())?;
            }
        }
        let mut references = Dict::new();
        for reference in iterate(at(at(documents, "reference")?, "targets")?)?.iter() {
            references.insert(at(reference, "target")?.clone(), reference.clone())?;
        }
        let reference_of = |key: &Value| -> Result<&Value> {
            references
                .get(key)?
                .ok_or_else(|| Failure::new("KeyError", key.py_repr()))
        };
        let selected_tasks = iterate(at(selected, "tasks")?)?;
        let profile_tasks = at(&profiles, "tasks")?;
        let mut unlabelled = Vec::new();
        for task in selected_tasks.iter() {
            let task_id = at(task, "id")?;
            for family in iterate(at(reference_of(task_id)?, "families")?)?.iter() {
                let labels = get_or(
                    index(profile_tasks, task_id)?,
                    "labels",
                    Value::Object(Object::default()),
                )?;
                let family_id = at(family, "id")?;
                if !lookup(&labels, family_id)?.is_some_and(Value::truthy) {
                    unlabelled.push(family_id.clone());
                }
            }
        }
        if !unlabelled.is_empty() {
            return Err(Failure::new(
                "ValueError",
                format!(
                    "bench/profiles.json has no short label for {}",
                    join_text(&unlabelled)?
                ),
            ));
        }
        let reference_url =
            self.evidence(&self.bench.join(b"grading/current/references.json"), None)?;
        let claims = iterate(at(at(documents, "claim")?, "claims")?)?;
        let mut manifestations: Dict<Vec<Value>> = Dict::new();
        for claim in claims.iter() {
            let family = at(claim, "family_id")?;
            if matches!(family, Value::Null) {
                continue;
            }
            let identifier = at(claim, "id")?.clone();
            match manifestations.get_mut(family)? {
                Some(list) => list.push(identifier),
                None => manifestations.insert(family.clone(), vec![identifier])?,
            }
        }
        let mut grades = Dict::new();
        for batch in iterate(at(at(documents, "grade")?, "batches")?)?.iter() {
            for review in iterate(at(batch, "reviews")?)?.iter() {
                let key = concat(
                    &concat(&name(&path_of(at(batch, "run")?)?), &text("/"))?,
                    at(review, "attempt_id")?,
                )?;
                let graded = Graded {
                    review: review.clone(),
                    assessor: at(batch, "assessor")?.clone(),
                };
                grades.insert(key, graded)?;
            }
        }
        let mut decisions = Dict::new();
        for decision in iterate(at(at(documents, "adjudication")?, "decisions")?)?.iter() {
            decisions.insert(at(decision, "id")?.clone(), decision.clone())?;
        }
        let mut rulings = Dict::new();
        for claim in claims.iter() {
            let identifier = at(claim, "id")?.clone();
            let url = self.ruling(&decisions, at(claim, "adjudication")?)?;
            rulings.insert(identifier, url)?;
        }
        let mut attempt_facts = Dict::new();
        let selected_attempts = iterate(at(selected, "attempts")?)?;
        for attempt in selected_attempts.iter() {
            attempt_facts.insert(at(attempt, "id")?.clone(), attempt.clone())?;
        }
        let controls = at(status, "controls")?;
        let mut tasks = Vec::new();
        for task in selected_tasks.iter() {
            let task_id = at(task, "id")?;
            let target = read(&join(&self.root, at(at(task, "target")?, "path")?)?)?;
            let profile = index(profile_tasks, task_id)?;
            let reference = reference_of(task_id)?;
            let kept: Object = match profile {
                Value::Object(entries) => entries
                    .iter()
                    .filter(|(key, _)| key.as_str() != "findings" && key.as_str() != "labels")
                    .map(|(key, value)| (key.clone(), value.clone()))
                    .collect(),
                other => {
                    return Err(Failure::new(
                        "AttributeError",
                        format!("'{}' object has no attribute 'items'", other.type_name()),
                    ));
                }
            };
            let mut families = Vec::new();
            for family in iterate(at(reference, "families")?)?.iter() {
                let family_id = at(family, "id")?;
                let short_label = index(at(profile, "labels")?, family_id)?.clone();
                let concerns = lookup(at(profile, "findings")?, family_id)?
                    .cloned()
                    .unwrap_or_else(|| Value::Array(Vec::new()));
                let eligibility = at(family, "eligibility")?;
                let impact = at(family, "impact")?;
                let mut family_rulings = Vec::new();
                for dimension in ["eligibility", "impact"] {
                    let url =
                        self.ruling(&decisions, at(at(family, dimension)?, "adjudication")?)?;
                    if url.truthy() {
                        family_rulings
                            .push(object(vec![("dimension", text(dimension)), ("url", url)]));
                    }
                }
                let manifested = manifestations.get(family_id)?.cloned().unwrap_or_default();
                families.push(object(vec![
                    ("id", family_id.clone()),
                    ("shortLabel", short_label),
                    ("title", at(family, "title")?.clone()),
                    ("trigger", at(family, "trigger")?.clone()),
                    ("consequence", at(family, "mechanism")?.clone()),
                    ("requiredOutcome", at(family, "obligation")?.clone()),
                    ("concerns", concerns),
                    ("eligibility", at(eligibility, "state")?.clone()),
                    ("eligibilityReason", at(eligibility, "reason")?.clone()),
                    ("impact", at(impact, "band")?.clone()),
                    ("impactReason", at(impact, "reason")?.clone()),
                    ("rulings", Value::Array(family_rulings)),
                    ("manifestations", Value::Array(sorted(manifested)?)),
                ]));
            }
            let mut pre_existing = Vec::new();
            for problem in iterate(&get_or(
                reference,
                "pre_existing_problems",
                Value::Array(Vec::new()),
            )?)?
            .iter()
            {
                let fields = vec![
                    ("id", at(problem, "id")?.clone()),
                    ("title", at(problem, "title")?.clone()),
                    ("trigger", at(problem, "trigger")?.clone()),
                    ("consequence", at(problem, "consequence")?.clone()),
                    ("requiredOutcome", at(problem, "obligation")?.clone()),
                    ("concerns", at(problem, "concerns")?.clone()),
                    ("impact", at(at(problem, "impact")?, "band")?.clone()),
                ];
                let admission = self.ruling(&decisions, at(problem, "admission")?)?;
                let mut fields = fields;
                fields.push(("admissionUrl", admission));
                pre_existing.push(object(fields));
            }
            let control = index(controls, task_id)?.clone();
            let control_record = at(reference, "control")?;
            let control_reason = at(control_record, "reason")?.clone();
            let control_url = self.ruling(&decisions, at(control_record, "adjudication")?)?;
            let packet_url =
                self.evidence(&join(&self.root, at(at(task, "packet")?, "path")?)?, None)?;
            let source_url = format!(
                "https://github.com/{}/pull/{}",
                at(&target, "repo")?.py_str(),
                at(&target, "pr")?.py_str()
            );
            tasks.push(object(vec![
                ("id", task_id.clone()),
                ("repo", at(&target, "repo")?.clone()),
                ("pr", at(&target, "pr")?.clone()),
                ("head", at(&target, "head")?.clone()),
                ("base", at(&target, "base_sha")?.clone()),
                ("shape", at(&target, "shape")?.clone()),
                ("language", at(&target, "language")?.clone()),
                ("profile", Value::Object(kept)),
                ("families", Value::Array(families)),
                ("preExistingProblems", Value::Array(pre_existing)),
                ("control", control),
                ("controlReason", control_reason),
                ("controlRulingUrl", control_url),
                ("registerUrl", reference_url.clone()),
                ("packetUrl", packet_url),
                ("sourceUrl", Value::Str(source_url)),
            ]));
        }
        let mut configurations = Vec::new();
        let mut outcomes = Vec::new();
        let mut attempts: Dict<Value> = Dict::new();
        let recovered = at(
            &read(&self.bench.join(b"skill-provenance.v2.json"))?,
            "skills",
        )?
        .clone();
        let lookups = Lookups {
            archives,
            rulings,
            lower_bounds: LowerBounds::read(&self.bench)?,
        };
        let all_sources = iterate(at(selected, "sources")?)?;
        let all_cells = iterate(at(selected, "cells")?)?;
        for entry in iterate(at(&registry, "configurations")?)?.iter() {
            let entry_id = at(entry, "id")?;
            let mut sources = Vec::new();
            for source in all_sources.iter() {
                if at(source, "configuration")?.py_eq(entry_id) {
                    sources.push(source.clone());
                }
            }
            let releases = self.skill_releases(entry, &sources, &recovered)?;
            let mut arms = Vec::new();
            for source in &sources {
                arms.push(read(&join(
                    &self.root,
                    at(at(source, "arm_file")?, "path")?,
                )?)?);
            }
            let mut efforts = Dict::new();
            for arm in &arms {
                efforts.insert(optional(get(arm, "effort")?), ())?;
            }
            let mut identifiers = Dict::new();
            for cell in all_cells.iter() {
                if at(cell, "configuration")?.py_eq(entry_id) {
                    for attempt in iterate(at(cell, "attempts")?)?.iter() {
                        identifiers.insert(attempt.clone(), ())?;
                    }
                }
            }
            let mut records = Vec::new();
            for attempt in selected_attempts.iter() {
                if identifiers.get(at(attempt, "id")?)?.is_some() {
                    records.push(read(&join(
                        &self.root,
                        at(at(attempt, "record")?, "path")?,
                    )?)?);
                }
            }
            let mut models = Dict::new();
            for arm in &arms {
                if let Some(model) = get(arm, "model")?.filter(|model| model.truthy()) {
                    models.insert(model.clone(), ())?;
                }
            }
            if models.len() == 0 {
                for record in &records {
                    for model in iterate(at(at(record, "observed")?, "models")?)?.iter() {
                        models.insert(model.clone(), ())?;
                    }
                }
            }
            let mut billing = Dict::new();
            for source in &sources {
                for record in &records {
                    let same_arm = at(at(record, "cell")?, "arm")?.py_eq(at(source, "arm")?);
                    if !same_arm
                        || !at(record, "run_id")?.py_eq(&name(&path_of(at(source, "run")?)?))
                    {
                        continue;
                    }
                    let value = match get(source, "billing_correction")?.filter(|pin| pin.truthy())
                    {
                        Some(pin) => {
                            at(&read(&join(&self.bench, at(pin, "path")?)?)?, "billing")?.clone()
                        }
                        None => at(at(record, "usage")?, "billing")?.clone(),
                    };
                    billing.insert(value, ())?;
                }
            }
            let method = at(entry, "method")?;
            let provenance = match get(entry, "skill_provenance")?.filter(|value| value.truthy()) {
                Some(value) => value.clone(),
                None if method.py_eq(&text("review-code")) => text("skill-provenance.json"),
                None => Value::Null,
            };
            let short = match at(entry, "short")? {
                Value::Str(short) => Value::Str(short.replace(" · ", " / ")),
                other => {
                    return Err(Failure::new(
                        "AttributeError",
                        format!("'{}' object has no attribute 'replace'", other.type_name()),
                    ));
                }
            };
            let provenance_url = if provenance.truthy() {
                self.evidence(&join(&self.bench, &provenance)?, None)?
            } else {
                Value::Null
            };
            let list_price = text("list-price-equivalent");
            let billing_basis = if billing.get(&list_price)?.is_some() {
                list_price
            } else if billing.len() == 1 {
                billing.keys().next().cloned().unwrap_or(Value::Null)
            } else {
                text("mixed")
            };
            let single_effort =
                (efforts.len() == 1).then(|| efforts.keys().next().cloned().unwrap_or(Value::Null));
            let effort = get_or(
                entry,
                "reasoning_effort",
                single_effort.clone().unwrap_or(Value::Null),
            )?;
            let source = get_or(
                entry,
                "reasoning_source",
                text(if single_effort.is_some() {
                    "explicit"
                } else {
                    "unrecorded"
                }),
            )?;
            let row_conditions = conditions(&arms, &records, &effort, &billing)?;
            configurations.push(object(vec![
                ("id", entry_id.clone()),
                ("label", at(entry, "label")?.clone()),
                ("short", short),
                ("version", at(entry, "version")?.clone()),
                ("method", method.clone()),
                ("reviewEdition", at(entry, "review_edition")?.clone()),
                ("reviewChange", at(entry, "review_change")?.clone()),
                ("skillReleases", releases),
                ("skillProvenanceUrl", provenance_url),
                ("experimental", at(entry, "experimental")?.clone()),
                ("builtin", Value::Bool(builtin(method)?)),
                ("note", at(entry, "note")?.clone()),
                ("billing", billing_basis),
                (
                    "models",
                    Value::Array(sorted(models.keys().cloned().collect())?),
                ),
                ("reasoningEffort", effort),
                ("reasoningSource", source),
                ("conditions", row_conditions),
            ]));
            for task in &tasks {
                let task_id = at(task, "id")?;
                let mut cells = Vec::new();
                for cell in all_cells.iter() {
                    if at(cell, "configuration")?.py_eq(entry_id)
                        && at(cell, "target")?.py_eq(task_id)
                    {
                        cells.push(cell);
                    }
                }
                let mut trials = Vec::new();
                let mut attempt_ids = Vec::new();
                for cell in &cells {
                    let mut chosen = None;
                    for source in &sources {
                        if at(source, "run")?.py_eq(at(cell, "run")?)
                            && at(source, "arm")?.py_eq(at(cell, "arm")?)
                        {
                            chosen = Some(source);
                            break;
                        }
                    }
                    let source = chosen.ok_or_else(|| Failure::new("StopIteration", ""))?;
                    let correction = if contains(source, "billing_correction")? {
                        let receipt =
                            join(&self.bench, at(at(source, "billing_correction")?, "path")?)?;
                        let billing = at(&read(&receipt)?, "billing")?.clone();
                        Some(Correction { receipt, billing })
                    } else {
                        None
                    };
                    let run = join(&self.bench, at(cell, "run")?)?;
                    for identifier in iterate(at(cell, "attempts")?)?.iter() {
                        let graded = grades.get(identifier)?.cloned();
                        let facts = attempt_facts
                            .get(identifier)?
                            .ok_or_else(|| Failure::new("KeyError", identifier.py_repr()))?;
                        let exported = self.export_attempt(
                            &run,
                            facts,
                            &lookups,
                            graded.as_ref(),
                            correction.as_ref(),
                        )?;
                        attempts.insert(identifier.clone(), exported)?;
                    }
                    trials.push(object(vec![
                        ("replicate", at(cell, "replicate")?.clone()),
                        ("state", at(cell, "state")?.clone()),
                        ("reason", at(cell, "reason")?.clone()),
                        ("attemptIds", at(cell, "attempts")?.clone()),
                        ("terminal", at(cell, "terminal")?.clone()),
                    ]));
                }
                for cell in &cells {
                    attempt_ids.extend(iterate(at(cell, "attempts")?)?.iter().cloned());
                }
                let ran = !cells.is_empty();
                outcomes.push(object(vec![
                    ("configurationId", entry_id.clone()),
                    ("taskId", task_id.clone()),
                    ("status", text(if ran { "ran" } else { "not run" })),
                    (
                        "reason",
                        text(if ran {
                            "Selected scheduled cells."
                        } else {
                            "No selected scheduled cell."
                        }),
                    ),
                    ("trials", Value::Array(trials)),
                    ("attemptIds", Value::Array(attempt_ids)),
                ]));
            }
        }
        let audit = at(documents, "audit")?;
        let mut candidates = Vec::new();
        for candidate in iterate(at(at(documents, "candidate")?, "candidates")?)?.iter() {
            if !matches!(at(candidate, "decision")?, Value::Null) {
                continue;
            }
            candidates.push(object(vec![
                ("id", at(candidate, "id")?.clone()),
                ("taskId", at(candidate, "target")?.clone()),
                ("recordedAt", at(candidate, "recorded_at")?.clone()),
                ("claim", at(candidate, "claim")?.clone()),
                ("limits", at(candidate, "limits")?.clone()),
                ("relevance", at(candidate, "relevance")?.clone()),
                ("kind", get_or(candidate, "kind", text("ordinary"))?),
            ]));
        }
        let transcripts = at(&imported, "transcripts")?;
        let mut mismatches = 0usize;
        for archive in iterate(transcripts)?.iter() {
            if at(archive, "status")?.py_eq(&text("mismatch")) {
                mismatches += 1;
            }
        }
        let dataset = object(vec![
            ("schemaVersion", Value::Int("5".to_string())),
            ("release", text("Current v1 preview")),
            ("revision", at(&imported, "revision")?.clone()),
            (
                "evidence",
                object(vec![
                    ("datasetHash", at(status, "dataset_hash")?.clone()),
                    (
                        "coverage",
                        object(vec![
                            ("requiredReviews", at(status, "required_reviews")?.clone()),
                            ("assessedReviews", at(status, "assessed_reviews")?.clone()),
                            (
                                "unresolvedRecoveries",
                                at(status, "unresolved_recoveries")?.clone(),
                            ),
                            ("unresolvedClaims", at(status, "unresolved_claims")?.clone()),
                            ("complete", at(status, "complete")?.clone()),
                            ("reason", at(status, "reason")?.clone()),
                        ]),
                    ),
                    (
                        "audit",
                        object(vec![
                            ("state", at(audit, "state")?.clone()),
                            ("reason", optional(get(audit, "reason")?)),
                        ]),
                    ),
                ]),
            ),
            ("profileStatus", at(&profiles, "status")?.clone()),
            ("tasks", Value::Array(tasks)),
            ("configurations", Value::Array(configurations)),
            ("outcomes", Value::Array(outcomes)),
            (
                "attempts",
                Value::Array(attempts.values().cloned().collect()),
            ),
            ("candidates", Value::Array(candidates)),
            (
                "import",
                object(vec![
                    ("files", length(at(&imported, "files")?)?),
                    ("transcripts", length(transcripts)?),
                    ("mismatches", count(mismatches)),
                ]),
            ),
        ]);
        if let Some(stage) = &self.stage {
            let destination = stage.join(b"data/benchmark.json");
            make_directories(&parent(&destination))?;
            write_text(&destination, &dump(&dataset))?;
        }
        Ok(dataset)
    }

    fn staged_file(&self, url: &Value) -> Result<PyPath> {
        let Some(stage) = &self.stage else {
            return Err(inconsistent("nothing was staged"));
        };
        let url = str_text(url)?;
        let prefix = format!("{}/", self.base);
        let Some(rest) = url.strip_prefix(&prefix) else {
            return Err(inconsistent(format!(
                "exported link leaves the site: {url}"
            )));
        };
        let path = stage.join(rest.as_bytes());
        if !path.is_file() {
            return Err(inconsistent(format!(
                "exported link has no staged file: {url}"
            )));
        }
        Ok(path)
    }

    /// `validate_export`, without the dataset-hash check the Python front repeats after staging.
    fn validate(&self, dataset: &Value) -> Result<()> {
        let stage = self
            .stage
            .as_ref()
            .ok_or_else(|| inconsistent("nothing was staged"))?;
        if !read(&stage.join(b"data/benchmark.json"))?.py_eq(dataset) {
            return Err(inconsistent("staged dataset differs from the export"));
        }
        let mut links = Vec::new();
        let tasks = iterate(at(dataset, "tasks")?)?;
        if let Some(first) = tasks.first() {
            links.push(at(first, "registerUrl")?.clone());
        }
        let truthy = |value: &Value, links: &mut Vec<Value>| {
            if value.truthy() {
                links.push(value.clone());
            }
        };
        for task in tasks.iter() {
            links.push(at(task, "packetUrl")?.clone());
            truthy(at(task, "controlRulingUrl")?, &mut links);
            for problem in iterate(at(task, "preExistingProblems")?)?.iter() {
                truthy(at(problem, "admissionUrl")?, &mut links);
            }
            for family in iterate(at(task, "families")?)?.iter() {
                for ruling in iterate(at(family, "rulings")?)?.iter() {
                    links.push(at(ruling, "url")?.clone());
                }
            }
        }
        for configuration in iterate(at(dataset, "configurations")?)?.iter() {
            for release in iterate(at(configuration, "skillReleases")?)?.iter() {
                links.push(at(release, "provenanceUrl")?.clone());
            }
            truthy(at(configuration, "skillProvenanceUrl")?, &mut links);
        }
        for attempt in iterate(at(dataset, "attempts")?)?.iter() {
            let detail = read(&self.staged_file(at(attempt, "detailUrl")?)?)?;
            let identifier = at(attempt, "id")?;
            if !at(&detail, "id")?.py_eq(identifier) {
                return Err(inconsistent(format!(
                    "{}: staged review detail belongs to another attempt",
                    identifier.py_str()
                )));
            }
            for key in [
                "recordUrl",
                "normalizedUrl",
                "archiveUrl",
                "billingCorrectionUrl",
                "usageCorrectionUrl",
            ] {
                truthy(at(&detail, key)?, &mut links);
            }
            let assessed = at(&detail, "assessment")?;
            truthy(at(assessed, "receiptUrl")?, &mut links);
            truthy(at(assessed, "verdictsUrl")?, &mut links);
            for item in iterate(at(&detail, "items")?)?.iter() {
                for claim in iterate(at(item, "claims")?)?.iter() {
                    truthy(at(claim, "rulingUrl")?, &mut links);
                }
            }
        }
        for url in &links {
            self.staged_file(url)?;
        }
        Ok(())
    }
}

/// `export-explorer evidence`: every file the export would link, with its frozen SHA-256 or null, in the
/// order the export first names it, so the Python front can restore the missing ones before staging.
pub fn evidence(root: &[u8], facts: &Facts, base_path: &str) -> Result<Value> {
    let mut exporter = Exporter::new(root, base_path, None);
    exporter.write_export(facts)?;
    Ok(Value::Array(exporter.listed))
}

/// `export-explorer evidence --restore`: each listed file that is not a regular file under `root`, restored through
/// `bench_tools::evidence::resolve` and checked against its frozen hash, as the Python front restored it.
pub fn restore(root: &[u8], listed: &Value, store: &mut crate::evidence::Store<'_>) -> Result<()> {
    let root = PyPath::new(root);
    for entry in iterate(listed)?.iter() {
        let path = at(entry, "path")?.py_str();
        if root.join(path.as_bytes()).is_file() {
            continue;
        }
        let sha256 = match at(entry, "sha256")? {
            Value::Null => None,
            other => Some(other.py_str()),
        };
        crate::evidence::resolve(store, &root, &path, sha256.as_deref(), false).map_err(
            |error| match error {
                crate::evidence::Error::Violation(message) => {
                    Failure::new("evidence_store.EvidenceError", message)
                }
                crate::evidence::Error::Io(message) => Failure::new("OSError", message),
            },
        )?;
    }
    Ok(())
}

/// `export-explorer stage`: the export written and checked under `stage`, refused before the facts are read while
/// an earlier publication's backups remain. Returns the summary line printed once it is published.
pub fn stage(root: &[u8], facts: &[u8], base_path: &str, stage: &[u8]) -> Result<String> {
    reset_stage(stage)?;
    staged(root, &Facts::read(facts)?, base_path, stage)
}

fn reset_stage(stage: &[u8]) -> Result<()> {
    let directory = PyPath::new(stage);
    let previous = directory.join(b"previous");
    if exists(&previous) && has_entries(&previous)? {
        return Err(inconsistent(format!(
            "previous export restoration is incomplete; backups retained at {}",
            previous.display()
        )));
    }
    if exists(&directory) {
        remove_tree(&directory)?;
    }
    make_directories(&directory.join(b"evidence"))
}

fn staged(root: &[u8], facts: &Facts, base_path: &str, stage: &[u8]) -> Result<String> {
    let mut exporter = Exporter::new(root, base_path, Some(stage));
    let dataset = exporter.write_export(facts)?;
    exporter.validate(&dataset)?;
    let coverage = at(at(&dataset, "evidence")?, "coverage")?;
    let size = |key: &str| -> Result<usize> { Ok(iterate(at(&dataset, key)?)?.len()) };
    Ok(format!(
        "Exported current facts: {} tasks, {} configurations, {} attempts, {}/{} admitted reviews assessed",
        size("tasks")?,
        size("configurations")?,
        size("attempts")?,
        at(coverage, "assessedReviews")?.py_str(),
        at(coverage, "requiredReviews")?.py_str()
    ))
}

/// The renames `publish` makes, so a test can fail one.
pub trait Moves {
    fn replace(&mut self, from: &[u8], to: &[u8]) -> io::Result<()>;
}

/// `os.replace`.
pub struct System;

impl Moves for System {
    fn replace(&mut self, from: &[u8], to: &[u8]) -> io::Result<()> {
        std::fs::rename(os(from), os(to))
    }
}

const PUBLISHED: [&[u8]; 2] = [b"data", b"evidence"];

/// A failure of the whole export: the export's own, or one the current record raised while it was read.
pub enum ExportFailure {
    Export(Failure),
    Current(current_grading::Failure),
}

impl From<Failure> for ExportFailure {
    fn from(failure: Failure) -> Self {
        Self::Export(failure)
    }
}

impl From<current_grading::Failure> for ExportFailure {
    fn from(failure: current_grading::Failure) -> Self {
        Self::Current(failure)
    }
}

impl ExportFailure {
    /// The last line of the traceback the Python front printed for the same failure.
    pub fn line(&self) -> String {
        match self {
            Self::Export(failure) => failure.line(),
            Self::Current(current_grading::Failure::Inconsistent(message)) => format!(
                "current_grading.Inconsistent: {}",
                pytext::stderr_text(message)
            ),
            Self::Current(current_grading::Failure::Input(message)) => {
                format!("current_grading.InputError: {message}")
            }
            Self::Current(current_grading::Failure::Crash(message)) => message.clone(),
        }
    }
}

/// `load_current(root)` and its coverage, as the facts and the `dataset_hash` they hold.
fn current(root: &[u8]) -> current_grading::R<(Value, Value)> {
    let root = current_grading::Root::new(root)?;
    let (selected, documents) =
        current_grading::load_current(&root, current_grading::CURRENT.as_bytes(), true)?;
    let status = current_grading::coverage_status(&selected, &documents)?;
    let hash = at(&status, "dataset_hash")
        .map_err(|failure| current_grading::Failure::Crash(failure.line()))?
        .clone();
    let documents = Value::Object(
        documents
            .kinds
            .into_iter()
            .map(|(kind, value)| (kind.to_string(), value))
            .collect(),
    );
    let facts = object(vec![
        ("selected", selected),
        ("documents", documents),
        ("status", status),
    ]);
    Ok((facts, hash))
}

/// The steps `export` takes besides reading and writing files, which tests replace.
pub struct Effects<'a, 'b> {
    pub store: &'a mut crate::evidence::Store<'b>,
    pub moves: &'a mut dyn Moves,
    /// Runs between staging and reading the record again.
    pub staged: &'a mut dyn FnMut(),
}

/// `bench-tools export-explorer`: read the current facts, restore the evidence they link, stage and check the
/// export under `ROOT/.cache/explorer-export`, read the record again and refuse an export whose dataset changed,
/// then publish with `SIGINT` blocked until the process exits. Returns the summary line. The stage directory is
/// removed unless backups that could not be restored remain in it.
pub fn export(
    root: &[u8],
    public: Option<&[u8]>,
    base_path: &str,
    effects: Effects<'_, '_>,
) -> std::result::Result<String, ExportFailure> {
    let (loaded, hash) = current(root)?;
    let facts = Facts::through_file(&loaded)?;
    let directory = PyPath::new(root).join(b".cache/explorer-export");
    let public = public.map_or_else(|| PyPath::new(root).join(b"public"), PyPath::new);
    restore(root, &evidence(root, &facts, base_path)?, effects.store)?;
    let outcome = (|| -> std::result::Result<String, ExportFailure> {
        reset_stage(directory.as_bytes())?;
        let summary = staged(root, &facts, base_path, directory.as_bytes())?;
        (effects.staged)();
        if !current(root)?.1.py_eq(&hash) {
            return Err(inconsistent("current evidence changed during export").into());
        }
        block_interrupts()?;
        publish(directory.as_bytes(), public.as_bytes(), effects.moves)?;
        Ok(summary)
    })();
    let previous = directory.join(b"previous");
    if !exists(&previous) || matches!(has_entries(&previous), Ok(false)) {
        remove_tree_quietly(&directory);
    }
    outcome
}

/// `signal.signal(SIGINT, SIG_IGN)` as the Python front set it around `publish`. Blocking needs no `unsafe`; a
/// blocked `SIGINT` stays pending until the process exits, which discards it.
fn block_interrupts() -> Result<()> {
    let mut interrupt = nix::sys::signal::SigSet::empty();
    interrupt.add(nix::sys::signal::Signal::SIGINT);
    interrupt
        .thread_block()
        .map_err(|error| Failure::new("OSError", error.desc()))
}

/// `export-explorer publish`: replace `public/data` and `public/evidence` with the staged pair, restoring the
/// previous pair if a move fails. Backups that cannot be restored stay in `stage/previous`.
pub fn publish(stage: &[u8], public: &[u8], moves: &mut dyn Moves) -> Result<()> {
    let stage = PyPath::new(stage);
    let public = PyPath::new(public);
    make_directories(&public)?;
    let previous = stage.join(b"previous");
    std::fs::create_dir(previous.to_path_buf())
        .map_err(|error| file_failure(&error, previous.as_bytes()))?;
    let mut replace = |from: &PyPath, to: &PyPath| {
        moves
            .replace(from.as_bytes(), to.as_bytes())
            .map_err(|error| move_failure(&error, from.as_bytes(), to.as_bytes()))
    };
    let mut outcome = Ok(());
    for name in PUBLISHED {
        if exists(&public.join(name)) {
            outcome = replace(&public.join(name), &previous.join(name));
            if outcome.is_err() {
                break;
            }
        }
        outcome = replace(&stage.join(name), &public.join(name));
        if outcome.is_err() {
            break;
        }
    }
    if let Err(failure) = outcome {
        for name in PUBLISHED {
            if exists(&previous.join(name)) {
                remove_tree_quietly(&public.join(name));
                replace(&previous.join(name), &public.join(name))?;
            } else if !exists(&stage.join(name)) {
                remove_tree_quietly(&public.join(name));
            }
        }
        return Err(failure);
    }
    remove_tree(&previous)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn timing(disposition: &str, fields: &[(&str, &str)]) -> Value {
        object(vec![
            ("disposition", text(disposition)),
            (
                "timing",
                object(
                    fields
                        .iter()
                        .map(|(key, value)| (*key, text(value)))
                        .collect(),
                ),
            ),
        ])
    }

    fn facts(selected: Value) -> Value {
        object(vec![
            ("selected", selected),
            ("documents", object(vec![("note", text("kept"))])),
            ("status", object(vec![])),
        ])
    }

    #[test]
    fn facts_are_refused_as_the_front_file_refused_them() {
        for (number, spelled) in [
            (f64::NAN, "nan"),
            (f64::INFINITY, "inf"),
            (f64::NEG_INFINITY, "-inf"),
        ] {
            let selected = Value::Array(vec![Value::Float(1.5), Value::Float(number)]);
            let failure = Facts::through_file(&facts(selected)).err().unwrap();
            assert_eq!(
                failure.line(),
                format!("ValueError: Out of range float values are not JSON compliant: {spelled}")
            );
        }
        let failure = Facts::through_file(&facts(Value::os_text(b"run-\xff")))
            .err()
            .unwrap();
        assert!(
            failure
                .line()
                .starts_with("ValueError: the string starting at char 13 holds a lone surrogate"),
            "{}",
            failure.line()
        );
        let kept = facts(Value::Array(vec![
            Value::Float(0.1),
            Value::Int("9007199254740993".into()),
        ]));
        let read = Facts::through_file(&kept).ok().unwrap();
        assert!(read.selected.py_eq(at(&kept, "selected").unwrap()));
        assert!(read.documents.py_eq(at(&kept, "documents").unwrap()));
    }

    #[test]
    fn the_usage_deviation_applies_only_to_listed_attempts_at_their_filed_cost() {
        let bench =
            std::env::temp_dir().join(format!("bench-tools-lower-bounds-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&bench);
        let bench_path = PyPath::new(bench.as_os_str().as_bytes());
        assert!(LowerBounds::read(&bench_path).unwrap().is_none());
        std::fs::create_dir_all(bench.join("deviations")).unwrap();
        std::fs::write(
            bench.join("deviations/usage-lower-bounds.v1.json"),
            r#"{"attempts": [
                {"run": "run-a", "attempt": "att-001", "probe": false, "filed_cost_usd": 1.25},
                {"run": "run-a", "attempt": "att-002", "probe": true, "filed_cost_usd": 2.5}
            ]}"#,
        )
        .unwrap();
        let bounds = LowerBounds::read(&bench_path).unwrap().unwrap();
        let usage = |status: &str, cost: f64| {
            object(vec![
                ("metering_status", text(status)),
                ("priced_total_usd", Value::Float(cost)),
            ])
        };
        let listed = text("run-a/att-001");
        assert!(
            bounds
                .entry(&listed, &usage("complete", 1.25))
                .unwrap()
                .is_some()
        );
        assert!(
            bounds
                .entry(&text("run-a/att-002"), &usage("complete", 2.5))
                .unwrap()
                .is_none()
        );
        assert!(
            bounds
                .entry(&text("run-b/att-001"), &usage("complete", 1.25))
                .unwrap()
                .is_none()
        );
        for changed in [usage("complete", 1.5), usage("incomplete", 1.25)] {
            let failure = bounds.entry(&listed, &changed).err().unwrap();
            assert_eq!(
                failure.line(),
                "current_grading.Inconsistent: run-a/att-001: the usage deviation names another filed cost"
            );
        }
        std::fs::remove_dir_all(&bench).unwrap();
    }

    #[test]
    fn review_duration_uses_the_filed_end_event() {
        let completed = timing(
            "valid completed",
            &[
                ("dispatched_at", "2026-09-29T10:00:00Z"),
                ("completed_at", "2026-09-29T10:02:00Z"),
                ("stopped_at", "2026-09-29T10:00:30Z"),
            ],
        );
        assert!(
            matches!(duration_seconds(&completed), Ok(Value::Float(seconds)) if seconds == 120.0)
        );
        let stopped = timing(
            "harness-invalid",
            &[
                ("dispatched_at", "2026-09-29T10:00:00Z"),
                ("stopped_at", "2026-09-29T10:00:30Z"),
            ],
        );
        assert!(matches!(duration_seconds(&stopped), Ok(Value::Float(seconds)) if seconds == 30.0));
        for fields in [
            vec![],
            vec![
                ("dispatched_at", "invalid"),
                ("completed_at", "2026-09-29T10:00:00Z"),
            ],
            vec![
                ("dispatched_at", "2026-09-29T10:01:00Z"),
                ("completed_at", "2026-09-29T10:00:00Z"),
            ],
            vec![
                ("dispatched_at", "2026-09-29T10:00:00"),
                ("completed_at", "2026-09-29T10:01:00Z"),
            ],
        ] {
            assert!(matches!(
                duration_seconds(&timing("valid completed", &fields)),
                Ok(Value::Null)
            ));
        }
        let instant = timing(
            "valid completed",
            &[
                ("dispatched_at", "2026-09-29T10:00:00Z"),
                ("completed_at", "2026-09-29T10:00:00Z"),
            ],
        );
        assert!(matches!(duration_seconds(&instant), Ok(Value::Float(seconds)) if seconds == 0.0));
    }

    #[test]
    fn transcripts_are_listed_as_python_3_14_globs_and_sorts_them() {
        let base = scratch("transcripts");
        for run in ["a-b", "a", "c"] {
            std::fs::create_dir_all(base.join("bench/runs").join(run)).unwrap();
        }
        std::fs::write(base.join("bench/runs/a/transcripts.json"), "[]").unwrap();
        std::fs::write(base.join("bench/runs/a-b/transcripts.json"), "[]").unwrap();
        std::os::unix::fs::symlink("missing.json", base.join("bench/runs/c/transcripts.json"))
            .unwrap();
        std::fs::write(base.join("bench/runs/file"), "").unwrap();
        let bench = PyPath::new(&bytes(&base.join("bench")));
        let listed: Vec<Vec<u8>> = transcript_files(&bench)
            .iter()
            .map(|path| path.as_bytes().to_vec())
            .collect();
        let expected: Vec<Vec<u8>> = ["a", "a-b", "c"]
            .iter()
            .map(|run| bytes(&base.join("bench/runs").join(run).join("transcripts.json")))
            .collect();
        assert_eq!(listed, expected);
        std::fs::remove_dir_all(&base).unwrap();
    }

    type FailWhen = Box<dyn Fn(usize, &[u8], &[u8]) -> bool>;

    struct Failing {
        calls: usize,
        fail_on: FailWhen,
    }

    impl Moves for Failing {
        fn replace(&mut self, from: &[u8], to: &[u8]) -> io::Result<()> {
            self.calls += 1;
            if (self.fail_on)(self.calls, from, to) {
                return Err(io::Error::from_raw_os_error(28));
            }
            std::fs::rename(os(from), os(to))
        }
    }

    fn scratch(label: &str) -> std::path::PathBuf {
        let base =
            std::env::temp_dir().join(format!("bench-tools-export-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        base
    }

    fn tree(root: &Path) -> Vec<(String, Vec<u8>)> {
        let mut found = Vec::new();
        let mut pending = vec![root.to_path_buf()];
        while let Some(directory) = pending.pop() {
            let Ok(entries) = std::fs::read_dir(&directory) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    pending.push(path);
                } else {
                    let name = path
                        .strip_prefix(root)
                        .unwrap()
                        .to_string_lossy()
                        .into_owned();
                    found.push((name, std::fs::read(&path).unwrap()));
                }
            }
        }
        found.sort();
        found
    }

    fn staged(base: &Path, label: &str) {
        for name in ["data", "evidence"] {
            std::fs::create_dir_all(base.join("stage").join(name)).unwrap();
            std::fs::write(
                base.join("stage").join(name).join("file"),
                format!("{label} {name}"),
            )
            .unwrap();
        }
    }

    fn bytes(path: &Path) -> Vec<u8> {
        path.as_os_str().as_bytes().to_vec()
    }

    fn publish_with(base: &Path, fail_on: FailWhen) -> Result<()> {
        let mut moves = Failing { calls: 0, fail_on };
        publish(
            &bytes(&base.join("stage")),
            &bytes(&base.join("public")),
            &mut moves,
        )
    }

    #[test]
    fn a_failed_move_restores_the_previous_pair() {
        for failing_call in 1..=4 {
            let base = scratch(&format!("restore-{failing_call}"));
            staged(&base, "old");
            publish(
                &bytes(&base.join("stage")),
                &bytes(&base.join("public")),
                &mut System,
            )
            .unwrap();
            let published = tree(&base.join("public"));
            staged(&base, "new");
            let failure =
                publish_with(&base, Box::new(move |call, _, _| call == failing_call)).unwrap_err();
            assert_eq!(failure.kind, "OSError");
            assert_eq!(tree(&base.join("public")), published, "call {failing_call}");
            assert!(tree(&base.join("stage/previous")).is_empty());
            std::fs::remove_dir_all(&base).unwrap();
        }
    }

    #[test]
    fn a_failed_restoration_keeps_the_backups() {
        let base = scratch("backups");
        staged(&base, "old");
        publish(
            &bytes(&base.join("stage")),
            &bytes(&base.join("public")),
            &mut System,
        )
        .unwrap();
        let published = tree(&base.join("public/evidence"));
        staged(&base, "new");
        let evidence = bytes(&base.join("public/evidence"));
        let failure =
            publish_with(&base, Box::new(move |_, _, to| to == evidence.as_slice())).unwrap_err();
        assert_eq!(failure.kind, "OSError");
        assert_eq!(tree(&base.join("stage/previous/evidence")), published);
        let refused = stage(
            b"/nonexistent",
            b"/nonexistent/facts.json",
            "",
            &bytes(&base.join("stage")),
        )
        .unwrap_err();
        assert!(
            refused.message.contains("backups retained at"),
            "{}",
            refused.message
        );
        assert_eq!(tree(&base.join("stage/previous/evidence")), published);
        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn a_failed_first_publication_leaves_no_directories() {
        for failing_call in 1..=2 {
            let base = scratch(&format!("first-{failing_call}"));
            staged(&base, "new");
            assert!(publish_with(&base, Box::new(move |call, _, _| call == failing_call)).is_err());
            assert!(!base.join("public/data").exists());
            assert!(!base.join("public/evidence").exists());
            std::fs::remove_dir_all(&base).unwrap();
        }
    }
}
