//! A batch's blind grading workspace, its private key and the check that a prepared workspace is still the one its
//! key pins. A key this module writes names the running `bench-tools` executable as its runner (`runner_deviation`
//! version 3), as `grade-prepare-decisions.v9.json` records.
//!
//! The checkout is the working directory: it holds the policies, Codex catalog and schemas grading reads.

pub mod client;
pub mod dispatch;
mod filing;
pub(crate) mod provision;
pub(crate) mod runtime;
mod source;

pub use filing::{
    Mapping, graded_v2, invalidate, inventory, key_counts, key_reviews, map_verdicts,
    next_assessment, provenance_of, prune_grading, raised_candidates, read_verdicts, resolve,
    sorted_by, validate,
};

use crate::claims::{self, Decisions, Repository, blind::Entropy};
use crate::current_grading::{self as current, Documents, Failure, R, Root};
use crate::pyjson::{self, Dumps, Object, Value};
use crate::pytext::{self, PyPath};
use crate::pyvalue::{self, Raised};
use crate::{grading_validation, normalize_review, pre_existing, pyfs, pyobject, sha256};
use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::Path;

pub const PROGRAM: &str = "bench-tools grade";
pub const KEY_CONTRACT: &str = "current-grading-key/v1";
pub const RUNNER_VERSION: i64 = 3;
const PLACEHOLDERS: [&str; 4] = ["{TARGET}", "{FAMILY_IDS}", "{REVIEWS}", "{ALLOWANCE}"];
const FAMILY_FIELDS: [&str; 5] = ["id", "title", "obligation", "trigger", "mechanism"];
const QUOTABLE: [&str; 3] = ["claim", "consequence", "proposed_fix"];
const MISSING_FAULT: &str = "Missing pre-existing fault:";
const EVIDENCE_CONTRACT: &str = "claim-evidence-v2";
/// Command profile versions, newest first. A checkout uses the newest it holds; the behavior-contract fixtures hold
/// only v1.
const PROFILES: [&[u8]; 3] = [
    b"bench/policies/grading-commands.v3.json",
    b"bench/policies/grading-commands.v2.json",
    b"bench/policies/grading-commands.v1.json",
];
const EXECUTION_POLICY: &[u8] = b"bench/policies/empty-harness-v1.md";
const CODEX_CATALOG: &[u8] = b"bench/harness/codex-grading-models.v1.json";
const REMEDY_CONTRACT: &str = "remedy-assessment/v1";

/// What a command reads besides its arguments: the checkout, the running executable and the token source.
pub struct Runner<'a> {
    pub checkout: PyPath,
    pub executable: std::path::PathBuf,
    pub entropy: &'a mut dyn Entropy,
}

/// `bench-tools grade prepare`'s options, and the ones `preflight` passes it.
#[derive(Clone, Default)]
pub struct Options {
    pub run: Vec<u8>,
    pub target: String,
    pub work: Vec<u8>,
    pub key: Vec<u8>,
    pub inventory: Option<Vec<u8>>,
    pub claim_evidence: Option<Vec<u8>>,
    pub cache_root: Option<Vec<u8>>,
    pub cache_replacements: Option<Vec<u8>>,
    /// A provisioner script run with python3; this executable's `provision` when absent.
    pub provision: Option<Vec<u8>>,
    pub root: Vec<u8>,
    pub current: Vec<u8>,
}

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

fn inconsistent<T>(message: impl Into<Vec<u8>>) -> R<T> {
    Err(Failure::Inconsistent(message.into()))
}

fn crash<T>(message: String) -> R<T> {
    Err(Failure::Crash(message))
}

fn utf8(bytes: &[u8], what: &str) -> R<String> {
    String::from_utf8(bytes.to_vec()).map_err(|_| {
        Failure::Crash(format!(
            "unsupported {what}: {} is not UTF-8",
            pytext::stderr_text(bytes)
        ))
    })
}

fn string(value: &Value) -> R<&str> {
    value.as_str().ok_or_else(|| {
        Failure::Crash(format!(
            "TypeError: expected str, not {}",
            value.type_name()
        ))
    })
}

/// `grade.read_bytes(path)`.
fn read_file(path: &PyPath) -> R<Vec<u8>> {
    pyvalue::read_bytes(path).map_err(|raised| match raised {
        Raised::Os(message) => Failure::Input(format!("cannot read {}: {message}", path.display())),
        other => other.into(),
    })
}

fn digest(path: &PyPath) -> R<String> {
    Ok(sha256::hex_digest(&read_file(path)?))
}

fn dumps(value: &Value, ensure_ascii: bool) -> String {
    pyjson::dumps(
        value,
        &Dumps {
            ensure_ascii,
            indent: Some(2),
            ..Dumps::default()
        },
    )
}

fn write(path: &PyPath, bytes: &[u8]) -> R<()> {
    pytext::write_bytes(path, bytes).map_err(Failure::Input)
}

fn make_directory(path: &PyPath, parents: bool) -> R<()> {
    let made = if parents {
        std::fs::create_dir_all(path.to_path_buf())
    } else {
        std::fs::create_dir(path.to_path_buf())
    };
    made.map_err(|error| Failure::Input(pytext::os_error(&error, path.as_bytes())))
}

/// `Path(value).name`.
fn name_of(path: &[u8]) -> Vec<u8> {
    let path = PyPath::new(path);
    let bytes = path.as_bytes();
    let start = bytes
        .iter()
        .rposition(|byte| *byte == b'/')
        .map_or(0, |at| at + 1);
    match &bytes[start..] {
        b"." => Vec::new(),
        name => name.to_vec(),
    }
}

/// `run_identity(value)`.
pub fn run_identity(value: &[u8]) -> R<String> {
    Ok(format!("runs/{}", utf8(&name_of(value), "run")?))
}

fn member(values: &[Value], value: &Value) -> bool {
    values.iter().any(|known| pyobject::eq(known, value))
}

/// `attempts_on(selected, run, target)`: the batch's selected attempts by attempt id, in inventory order.
pub fn attempts_on(selected: &Value, run: &str, target: &str) -> R<Vec<(String, Value)>> {
    let mut cells = Vec::new();
    for cell in pyvalue::iterate(pyvalue::item(selected, "cells")?)? {
        let (cell_run, cell_target) = (
            pyvalue::item(&cell, "run")?,
            pyvalue::item(&cell, "target")?,
        );
        if pyobject::eq(cell_run, &text(run)) && pyobject::eq(cell_target, &text(target)) {
            cells.push(pyvalue::item(&cell, "id")?.clone());
        }
    }
    let mut attempts: Vec<(String, Value)> = Vec::new();
    for attempt in pyvalue::iterate(pyvalue::item(selected, "attempts")?)? {
        let id = string(pyvalue::item(&attempt, "id")?)?;
        let name = id
            .split_once('/')
            .map(|(_, name)| name.to_string())
            .ok_or_else(|| Failure::Crash("IndexError: list index out of range".into()))?;
        if member(&cells, pyvalue::item(&attempt, "cell")?) {
            match attempts.iter_mut().find(|(known, _)| *known == name) {
                Some(slot) => slot.1 = attempt,
                None => attempts.push((name, attempt)),
            }
        }
    }
    Ok(attempts)
}

/// What `check_attempts` returns: the selected attempts that saved a review, their readable normalized reviews
/// by attempt id in sorted order, and the problems found.
pub struct Checked {
    pub attempts: Vec<(String, Value)>,
    pub docs: Vec<(String, Value)>,
    pub problems: Vec<String>,
}

/// Whether `check_attempts` catches a failure as a malformed review: `InputError`, `KeyError`, `TypeError` or
/// `ValueError` and its subclasses.
fn malformed(failure: &Failure) -> bool {
    match failure {
        Failure::Input(_) => true,
        Failure::Crash(line) => {
            let class = line
                .split_once(": ")
                .map_or(line.as_str(), |(class, _)| class);
            matches!(
                class,
                "KeyError" | "TypeError" | "ValueError" | "JSONDecodeError" | "UnicodeDecodeError"
            )
        }
        Failure::Inconsistent(_) => false,
    }
}

/// `check_attempts(root, selected, run, target, sources)`: `sources` gives each named source's item count per
/// attempt.
pub fn check_attempts(
    root: &Root,
    selected: &Value,
    run: &str,
    target: &str,
    sources: &[(String, Object)],
) -> R<Checked> {
    let mut attempts = Vec::new();
    for (name, facts) in attempts_on(selected, run, target)? {
        if pyvalue::item(&facts, "review")?.truthy() {
            attempts.push((name, facts));
        }
    }
    let names: BTreeSet<&str> = attempts.iter().map(|(name, _)| name.as_str()).collect();
    let mut problems = Vec::new();
    for (source, counts) in sources {
        let counted: BTreeSet<&str> = counts.iter().map(|(name, _)| name.as_str()).collect();
        problems.extend(names.difference(&counted).map(|attempt| {
            format!("{attempt}: a selected saved review on {target} but not in {source}")
        }));
        problems.extend(counted.difference(&names).map(|attempt| {
            format!("{attempt}: in {source} but not a selected saved review on {target}")
        }));
    }
    let common: BTreeSet<&str> = names
        .iter()
        .copied()
        .filter(|name| sources.iter().all(|(_, counts)| counts.get(name).is_some()))
        .collect();
    let mut docs = Vec::new();
    for attempt in common {
        let facts = &attempts
            .iter()
            .find(|(name, _)| name == attempt)
            .expect("selected")
            .1;
        let read = (|| -> R<Option<Value>> {
            let doc = current::read_json(&root.resolve_pin(pyvalue::item(facts, "review")?)?)?;
            if !matches!(pyvalue::get(&doc, "items")?, Value::Array(_)) {
                return Ok(None);
            }
            render(&doc)?;
            Ok(Some(doc))
        })();
        match read {
            Ok(Some(doc)) => docs.push((attempt.to_string(), doc)),
            Ok(None) => problems.push(format!("{attempt}: normalized review needs an items list")),
            Err(failure) if malformed(&failure) => {
                problems.push(format!("{attempt}: missing or malformed normalized review"));
            }
            Err(failure) => return Err(failure),
        }
    }
    for (source, counts) in sources {
        for (attempt, doc) in &docs {
            let length = Value::Int(items_of(doc).len().to_string());
            let count = counts.get(attempt).expect("common attempts are counted");
            if !pyobject::eq(&length, count) {
                problems.push(format!(
                    "{attempt}: normalized.json has {} items, {source} {}",
                    length.py_str(),
                    count.py_str()
                ));
            }
        }
    }
    Ok(Checked {
        attempts,
        docs,
        problems,
    })
}

fn items_of(doc: &Value) -> &[Value] {
    doc.as_object()
        .and_then(|doc| doc.get("items"))
        .and_then(Value::as_array)
        .unwrap_or_default()
}

/// `grade.render(doc)`: the normalized review as graders read it, with each source link's private attempt
/// directory removed.
pub fn render(doc: &Value) -> R<String> {
    normalize_review::render(doc)
        .map(|text| source::blind(&text))
        .map_err(|message| Failure::Crash(format!("ValueError: {message}")))
}

/// `source_view(item)`: what a quotation may cite.
fn source_view(item: &Value) -> R<Value> {
    let mut segments = Vec::new();
    for field in QUOTABLE {
        if let Value::Str(value) = pyvalue::get(item, field)? {
            segments.extend(
                source::split(value)
                    .into_iter()
                    .filter(|part| !part.is_empty())
                    .map(text),
            );
        }
    }
    Ok(object(vec![
        ("segments", Value::Array(segments)),
        (
            "proposed_fix",
            Value::Bool(pyvalue::get(item, "proposed_fix")?.truthy()),
        ),
    ]))
}

/// `batch_inputs(root, current, run, target, loaded)`: the selected inventory, the current documents and the
/// batch's input fingerprint.
pub fn batch_inputs(
    root: &Root,
    current: &[u8],
    run: &str,
    target: &str,
    loaded: Option<&(Value, Documents)>,
) -> R<(Value, Documents, String)> {
    let (selected, documents) = match loaded {
        Some((selected, documents)) => (
            selected.clone(),
            Documents {
                kinds: documents.kinds.clone(),
            },
        ),
        None => current::load_current(root, current, false)?,
    };
    let batch = object(vec![("run", text(run)), ("target", text(target))]);
    if !pyvalue::contains(pyvalue::item(&selected, "batches")?, &batch)? {
        return inconsistent(format!(
            "{run}/{target} is not a selected batch of the current inventory"
        ));
    }
    let fingerprint =
        current::grading_fingerprint(root, &batch, &selected, &documents, documents.get("policy"))?;
    Ok((selected, documents, fingerprint))
}

/// `unchanged_inputs(root, current, key)`: the key's batch, refused when its inputs are not the ones it was
/// prepared from.
pub fn unchanged_inputs(root: &Root, current: &[u8], key: &Value) -> R<(Value, Documents)> {
    let (run, target) = (pyvalue::item(key, "run")?, pyvalue::item(key, "target")?);
    let (selected, documents, fingerprint) =
        batch_inputs(root, current, string(run)?, string(target)?, None)?;
    let pinned = pyvalue::item(key, "input_fingerprint")?;
    if !pyobject::eq(&Value::Str(fingerprint.clone()), pinned) {
        let prefix = |value: &str| value.chars().take(12).collect::<String>();
        return inconsistent(format!(
            "{}/{}: the batch's inputs changed since preparation ({} became {}); prepare it again",
            run.py_str(),
            target.py_str(),
            prefix(string(pinned)?),
            prefix(&fingerprint)
        ));
    }
    Ok((selected, documents))
}

/// `check_current_record(key)`: stop a dispatch whose verdicts `map` would refuse, before the session is paid for.
pub fn check_current_record(key: &Value) -> R<()> {
    let record = pyvalue::get(key, "current_record")?;
    if matches!(record, Value::Null) {
        return inconsistent(
            "the key names no current record to check the batch's inputs against; prepare a fresh workspace",
        );
    }
    let root = Root::new(string(pyvalue::item(record, "root")?)?.as_bytes())?;
    let current = string(pyvalue::item(record, "current")?)?;
    unchanged_inputs(&root, current.as_bytes(), key).map(|_| ())
}

/// The newest command profile file in `checkout`.
fn profiles_path(checkout: &PyPath) -> PyPath {
    PROFILES
        .iter()
        .map(|relative| checkout.join(relative))
        .find(|path| path.to_path_buf().is_file())
        .unwrap_or_else(|| checkout.join(PROFILES[0]))
}

/// `command_policy(target, provisioning, revision)`.
pub fn command_policy(
    checkout: &PyPath,
    target: &str,
    provisioning: &Value,
    revision: &Value,
) -> R<Value> {
    let path = profiles_path(checkout);
    let profiles = current::read_json(&path)?;
    let fallback = object(vec![
        ("test_kind", text("none")),
        ("once", Value::Bool(false)),
    ]);
    let profile = pyvalue::get_or(pyvalue::item(&profiles, "targets")?, target, &fallback)?;
    let Value::Object(profile) = profile else {
        return crash(format!(
            "TypeError: 'dict' object argument after ** must be a mapping, not {}",
            profile.type_name()
        ));
    };
    let mut policy = Object::default();
    policy.insert(
        "version".into(),
        pyvalue::item(&profiles, "version")?.clone(),
    );
    for (name, value) in profile.iter() {
        policy.insert(name.clone(), value.clone());
    }
    policy.insert("profiles_sha256".into(), Value::Str(digest(&path)?));
    let empty = text("");
    let revision = match revision {
        value if value.truthy() => value.clone(),
        _ => Value::Object(Object::default()),
    };
    policy.insert(
        "git_refs".into(),
        Value::Array(vec![
            text("HEAD"),
            text("main"),
            text("review-head"),
            pyvalue::get_or(&revision, "head", &empty)?.clone(),
            pyvalue::get_or(&revision, "merge_base", &empty)?.clone(),
        ]),
    );
    let allowance = string(pyvalue::item(provisioning, "allowance")?)?;
    policy.insert(
        "allowance_sha256".into(),
        Value::Str(sha256::hex_digest(allowance.as_bytes())),
    );
    Ok(Value::Object(policy))
}

/// The runner identity a key or receipt records: the `bench-tools` executable and the policies it pins.
fn runner_deviation(
    checkout: &PyPath,
    pin: Value,
    execution_policy: &[u8],
) -> R<Vec<(&'static str, Value)>> {
    Ok(vec![
        ("version", Value::Int(RUNNER_VERSION.to_string())),
        ("bench_tools", pin),
        (
            "execution_policy_sha256",
            Value::Str(sha256::hex_digest(execution_policy)),
        ),
        (
            "codex_catalog_sha256",
            Value::Str(digest(&checkout.join(CODEX_CATALOG))?),
        ),
    ])
}

/// `n + 1` for a signed decimal string.
fn successor(number: &str) -> String {
    fn increment(digits: &str) -> String {
        let mut out: Vec<u8> = digits.bytes().collect();
        for at in (0..out.len()).rev() {
            if out[at] == b'9' {
                out[at] = b'0';
            } else {
                out[at] += 1;
                return String::from_utf8(out).unwrap_or_default();
            }
        }
        format!("1{}", String::from_utf8(out).unwrap_or_default())
    }
    fn decrement(digits: &str) -> String {
        let mut out: Vec<u8> = digits.bytes().collect();
        for at in (0..out.len()).rev() {
            if out[at] == b'0' {
                out[at] = b'9';
            } else {
                out[at] -= 1;
                break;
            }
        }
        let text = String::from_utf8(out).unwrap_or_default();
        let trimmed = text.trim_start_matches('0');
        if trimmed.is_empty() {
            "0".into()
        } else {
            trimmed.into()
        }
    }
    match number.strip_prefix('-') {
        Some(magnitude) => match decrement(magnitude).as_str() {
            "0" => "0".to_string(),
            rest => format!("-{rest}"),
        },
        None => increment(number),
    }
}

/// `str(int(item_id.removeprefix("item-")) + 1)`, and the integer itself for ordering.
fn item_number(item_id: &Value) -> R<(String, String)> {
    let Value::Str(id) = item_id else {
        return crash(format!(
            "AttributeError: '{}' object has no attribute 'removeprefix'",
            item_id.type_name()
        ));
    };
    let (negative, digits) =
        current::item_number(id.strip_prefix("item-").unwrap_or(id)).map_err(|failure| {
            match failure {
                Failure::Crash(line) if line.starts_with("ValueError: ") => {
                    shared(line["ValueError: ".len()..].to_string())
                }
                other => other,
            }
        })?;
    let number = if negative {
        format!("-{digits}")
    } else {
        digits
    };
    Ok((successor(&number), number))
}

/// `Inconsistent(f"shared claims: {error}")` for an error the claims block catches.
fn shared(message: String) -> Failure {
    Failure::Inconsistent(format!("shared claims: {message}").into_bytes())
}

/// The claims block catches `ValueError`, `KeyError`, `IndexError` and `OSError`.
fn caught(raised: Raised) -> Failure {
    match raised {
        Raised::Key(message)
        | Raised::Value(message)
        | Raised::Os(message)
        | Raised::Index(message) => shared(message),
        other => other.into(),
    }
}

fn claims_failure(failure: claims::Failure) -> Failure {
    shared(failure.0)
}

/// `d.setdefault(token, {}).setdefault(number, []).append(value)`.
fn append(map: &mut Object, token: &str, number: &str, value: Value) {
    if map.get(token).is_none() {
        map.insert(token.to_string(), Value::Object(Object::default()));
    }
    let Some(Value::Object(numbers)) = map.get_mut(token) else {
        return;
    };
    if numbers.get(number).is_none() {
        numbers.insert(number.to_string(), Value::Array(Vec::new()));
    }
    if let Some(Value::Array(values)) = numbers.get_mut(number) {
        values.push(value);
    }
}

/// One saved review in the workspace.
struct Review {
    token: String,
    attempt_id: String,
    items: usize,
    review: Value,
    text: String,
}

/// What the claims block of `prepare` builds.
#[derive(Default)]
struct ClaimContext {
    text: String,
    canonical: Object,
    matches: Object,
    links: Object,
    credits: Object,
    evidence: Option<Vec<(String, claims::blind::Packet)>>,
}

fn lookup<'a>(by_path: &'a [(Value, String)], path: &Value) -> R<Option<&'a str>> {
    if !path.hashable() {
        return crash(format!(
            "TypeError: unhashable type: '{}'",
            path.type_name()
        ));
    }
    Ok(by_path
        .iter()
        .find(|(known, _)| known.py_eq(path))
        .map(|(_, token)| token.as_str()))
}

fn review_path(value: &Value) -> R<&Value> {
    pyvalue::item(pyvalue::item(value, "review")?, "path").map_err(caught)
}

fn claim_context(
    root: &Root,
    checkout: &PyPath,
    documents: &Documents,
    reviews: &[Review],
    target_id: &str,
    contract: &Value,
    claim_evidence: Option<&[u8]>,
) -> R<ClaimContext> {
    let mut context = ClaimContext::default();
    let adjudication = pyvalue::item(documents.get("adjudication"), "decisions").map_err(caught)?;
    let decisions = Decisions::new(adjudication).map_err(claims_failure)?;
    let mut by_path: Vec<(Value, String)> = Vec::new();
    for review in reviews {
        let path = pyvalue::item(&review.review, "path")
            .map_err(caught)?
            .clone();
        match by_path.iter_mut().find(|(known, _)| known.py_eq(&path)) {
            Some(slot) => slot.1 = review.token.clone(),
            None => by_path.push((path, review.token.clone())),
        }
    }
    let mut applicable = Vec::new();
    for case in pyvalue::iterate(pyvalue::item(documents.get("claim"), "claims").map_err(caught)?)
        .map_err(caught)?
    {
        if !pyobject::eq(
            pyvalue::item(&case, "target").map_err(caught)?,
            &text(target_id),
        ) {
            continue;
        }
        let mut linked = false;
        for link in
            pyvalue::iterate(pyvalue::item(&case, "links").map_err(caught)?).map_err(caught)?
        {
            if lookup(&by_path, review_path(&link)?)?.is_some() {
                linked = true;
                break;
            }
        }
        if linked {
            applicable.push(case);
        }
    }
    context.text = claims::grading_context(&applicable, &decisions).map_err(claims_failure)?;
    for case in &applicable {
        let id = string(pyvalue::item(case, "id").map_err(caught)?)?.to_string();
        context.canonical.insert(
            id.clone(),
            claims::pinned(case, &decisions).map_err(claims_failure)?,
        );
        for link in
            pyvalue::iterate(pyvalue::item(case, "links").map_err(caught)?).map_err(caught)?
        {
            let Some(token) = lookup(&by_path, review_path(&link)?)? else {
                continue;
            };
            let (number, _) = item_number(pyvalue::item(&link, "item_id").map_err(caught)?)?;
            append(&mut context.links, token, &number, text(&id));
            let relation = pyvalue::item(&link, "relation").map_err(caught)?;
            if pyobject::eq(relation, &text("equivalent")) {
                append(&mut context.matches, token, &number, text(&id));
            }
            let _ = write!(
                context.text,
                "\n{id} {}: {token} item {number}\n",
                relation.py_str()
            );
        }
    }
    let mut ruled = Vec::new();
    for ruling in
        pyvalue::iterate(pyvalue::item(documents.get("credit"), "rulings").map_err(caught)?)
            .map_err(caught)?
    {
        if pyobject::eq(
            pyvalue::item(&ruling, "target").map_err(caught)?,
            &text(target_id),
        ) && lookup(&by_path, review_path(&ruling)?)?.is_some()
        {
            ruled.push(ruling);
        }
    }
    let ruled_contract = [grading_validation::CONTRACT_V2, pre_existing::CONTRACT]
        .iter()
        .any(|known| pyobject::eq(contract, &text(known)));
    if !ruled.is_empty() && ruled_contract {
        context.text += "\n# Rulings on one comment\n\nThe user ruled on these comments one at a time. In that \
                         item's claims, record these facts for the known problem named.\n";
        let mut keyed = Vec::new();
        for ruling in ruled {
            let token = lookup(&by_path, review_path(&ruling)?)?
                .expect("ruled reviews are in the workspace")
                .to_string();
            let (number, integer) =
                item_number(pyvalue::item(&ruling, "item_id").map_err(caught)?)?;
            let subject = pre_existing::credit_subject(&ruling)?;
            keyed.push((token, integer, subject, number, ruling));
        }
        let mut failed = None;
        keyed.sort_by(|left, right| {
            left.0
                .cmp(&right.0)
                .then_with(|| pyvalue::compare_integers(&left.1, &right.1))
                .then_with(|| {
                    if left.2.py_eq(&right.2) {
                        return Ordering::Equal;
                    }
                    pyvalue::compare(&left.2, &right.2).unwrap_or_else(|raised| {
                        failed.get_or_insert(raised);
                        Ordering::Equal
                    })
                })
        });
        if let Some(raised) = failed {
            return Err(raised.into());
        }
        for (token, _, subject, number, ruling) in keyed {
            let says_what = pyvalue::item(&ruling, "says_what").map_err(caught)?;
            let identifies_cause = pyvalue::item(&ruling, "identifies_cause").map_err(caught)?;
            let mut credit = object(vec![
                ("family", subject.clone()),
                ("says_what", says_what.clone()),
                ("identifies_cause", identifies_cause.clone()),
            ]);
            if pyvalue::has(&ruling, "pre_existing_problem_id")?
                && let Value::Object(fields) = &mut credit
            {
                fields.insert("pre_existing".into(), Value::Bool(true));
            }
            append(&mut context.credits, &token, &number, credit);
            let cause = if identifies_cause.truthy() {
                identifies_cause.py_str()
            } else {
                "not ruled".to_string()
            };
            let _ = write!(
                context.text,
                "\n{token} item {number}, {}: says what goes wrong, {}; identifies the cause as a fault, {cause}.\n",
                subject.py_str(),
                says_what.py_str()
            );
        }
    }
    if let Some(path) = claim_evidence {
        let extracts = Repository::new(checkout.as_bytes())
            .load_extracts(path)
            .map_err(claims_failure)?;
        let packets = Repository::new(root.path.as_bytes())
            .grading_evidence(&applicable, &decisions, &extracts)
            .map_err(claims_failure)?;
        if !packets.is_empty() {
            let ids: Vec<String> = packets.iter().map(|(id, _)| id.clone()).collect();
            context.text += &claims::evidence_index(&ids);
        }
        context.evidence = Some(packets);
    }
    Ok(context)
}

/// `sorted(set(re.findall(r"\{[A-Z_]+\}", text)))`.
fn placeholders(text: &str) -> BTreeSet<&str> {
    let bytes = text.as_bytes();
    let mut found = BTreeSet::new();
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at] == b'{' {
            let end = bytes[at + 1..]
                .iter()
                .position(|byte| !(byte.is_ascii_uppercase() || *byte == b'_'))
                .map(|offset| at + 1 + offset);
            if let Some(end) = end
                && end > at + 1
                && bytes[end] == b'}'
            {
                found.insert(&text[at..=end]);
                at = end + 1;
                continue;
            }
        }
        at += 1;
    }
    found
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    needle.is_empty()
        || haystack
            .windows(needle.len())
            .any(|window| window == needle)
}

/// `os.path.expanduser("~")`.
fn home() -> Vec<u8> {
    match std::env::var_os("HOME") {
        Some(home) => {
            let bytes = pytext::os_bytes(&home);
            let end = bytes
                .iter()
                .rposition(|byte| *byte != b'/')
                .map_or(0, |at| at + 1);
            if end == 0 {
                b"/".to_vec()
            } else {
                bytes[..end].to_vec()
            }
        }
        None => b"~".to_vec(),
    }
}

/// `os.path.abspath(path)`.
fn abspath(path: &[u8]) -> R<Vec<u8>> {
    if path.starts_with(b"/") {
        return Ok(pyfs::normpath(path));
    }
    let cwd =
        std::env::current_dir().map_err(|error| Failure::Input(pytext::os_error(&error, b".")))?;
    Ok(pyfs::normpath(
        &[pytext::os_bytes(cwd.as_os_str()), b"/", path].concat(),
    ))
}

fn realpath(path: &[u8]) -> R<Vec<u8>> {
    pytext::realpath(path, false).map_err(Failure::Input)
}

/// `prepare(args, loaded)`: the lines printed. With `preflight_only`, the checks alone and the provisioner's
/// pinned source and cache check, and nothing written.
pub fn prepare(
    options: &Options,
    runner: &mut Runner,
    loaded: Option<&(Value, Documents)>,
    preflight_only: bool,
) -> R<Vec<u8>> {
    let root = Root::new(&options.root)?;
    let work = PyPath::new(&pyfs::resolve(&options.work));
    let key_path = PyPath::new(&abspath(&options.key)?);
    let work_os = work.to_path_buf();
    if work_os.exists() {
        let empty = work_os.is_dir()
            && std::fs::read_dir(&work_os)
                .map_err(|error| Failure::Input(pytext::os_error(&error, work.as_bytes())))?
                .next()
                .is_none();
        if !empty {
            return inconsistent(
                [work.as_bytes(), b" exists and is not an empty directory"].concat(),
            );
        }
    }
    let (real_work, real_key) = (realpath(work.as_bytes())?, realpath(key_path.as_bytes())?);
    if real_key == real_work || real_key.starts_with(&[real_work.as_slice(), b"/"].concat()) {
        return inconsistent(
            [
                b"the key ",
                key_path.as_bytes(),
                b" is inside ",
                work.as_bytes(),
            ]
            .concat(),
        );
    }
    if key_path.to_path_buf().exists() {
        return inconsistent(
            [key_path.as_bytes(), b" exists; a key is never overwritten"].concat(),
        );
    }
    let run = run_identity(&options.run)?;
    let target_id = options.target.as_str();
    let (selected, documents, fingerprint) =
        batch_inputs(&root, &options.current, &run, target_id, loaded)?;
    let policy_path = pyvalue::item(documents.get("policy"), "path")?;
    let policy = current::read_json(&root.path.join(string(policy_path)?.as_bytes()))?;
    let default_contract = text(grading_validation::CONTRACT);
    let contract = pyvalue::get_or(&policy, "verdicts", &default_contract)?.clone();
    let is_contract = |known: &str| pyobject::eq(&contract, &text(known));
    if !is_contract(grading_validation::CONTRACT)
        && !is_contract(grading_validation::CONTRACT_V2)
        && !is_contract(pre_existing::CONTRACT)
    {
        return inconsistent(format!(
            "unsupported verdict contract {}",
            contract.py_repr()
        ));
    }
    let v2_or_v3 =
        is_contract(grading_validation::CONTRACT_V2) || is_contract(pre_existing::CONTRACT);
    let v3 = is_contract(pre_existing::CONTRACT);
    if options.inventory.is_some() && !v2_or_v3 {
        return inconsistent("--inventory requires current-verdicts/v2");
    }
    let mut remedy_policy = None;
    let mut safety_policy = "legacy";
    let mut text_policy = policy.clone();
    let remedy_path = root.local(&options.current)?.join(b"remedy-policy.json");
    if remedy_path.to_path_buf().exists() {
        let remedy_source = current::read_json(&remedy_path)?;
        let remedies = current::read_json(&root.resolve_pin(&remedy_source)?)?;
        if !pyobject::eq(pyvalue::get(&remedies, "contract")?, &text(REMEDY_CONTRACT)) || !v3 {
            return inconsistent(
                "remedy policy requires remedy-assessment/v1 and verdict contract v3",
            );
        }
        if !pyobject::eq(
            pyvalue::get(&remedies, "validation_policy")?,
            documents.get("policy"),
        ) {
            return inconsistent("remedy policy must pin the current validation policy");
        }
        let role = if options.inventory.is_some() {
            "second"
        } else {
            "first"
        };
        text_policy = pyvalue::item(&remedies, role)?.clone();
        safety_policy = if role == "second" {
            grading_validation::NO_SAFETY
        } else {
            grading_validation::CONDITIONAL_SAFETY
        };
        remedy_policy = Some(object(vec![
            ("source", remedy_source),
            ("role", text(role)),
        ]));
    }
    let pinned_text = |name: &str| -> R<Vec<u8>> {
        let missing = |key: &str| {
            Failure::Inconsistent(format!("the validation policy pins no {key}").into_bytes())
        };
        let pin = match &text_policy {
            Value::Object(fields) => fields.get(name).ok_or_else(|| missing(name))?,
            other => {
                return crash(format!(
                    "TypeError: '{}' object is not subscriptable",
                    other.type_name()
                ));
            }
        };
        let path = root.resolve_pin(pin).map_err(|failure| match failure {
            Failure::Crash(line) if line.starts_with("KeyError: ") => {
                missing(line["KeyError: ".len()..].trim_matches('\''))
            }
            other => other,
        })?;
        read_file(&path)
    };
    let mut rubric = pinned_text("rubric")?;
    let template_raw = pinned_text("grader")?;
    if let Some(rules) = policy.as_object().and_then(|policy| policy.get("rules")) {
        let end = rubric
            .iter()
            .rposition(|byte| *byte != b'\n')
            .map_or(0, |at| at + 1);
        rubric.truncate(end);
        rubric.extend_from_slice(b"\n\n");
        rubric.extend(read_file(&root.resolve_pin(rules)?)?);
    }
    let template = pytext::decode_utf8(&template_raw)
        .map_err(|message| Failure::Crash(format!("UnicodeDecodeError: {message}")))?;
    let mut problems: Vec<String> = PLACEHOLDERS
        .iter()
        .filter(|placeholder| !template.contains(*placeholder))
        .map(|placeholder| format!("template lacks {placeholder}"))
        .collect();
    let checked = check_attempts(&root, &selected, &run, target_id, &[])?;
    problems.extend(checked.problems);
    if checked.attempts.is_empty() {
        problems.push(format!("no selected saved reviews on {target_id}"));
    }
    if !problems.is_empty() {
        return inconsistent(problems.join("\n"));
    }
    let (attempts, docs) = (checked.attempts, checked.docs);
    let directory = root.path.join(b"bench/targets").join(target_id.as_bytes());
    let task = pyvalue::iterate(pyvalue::item(&selected, "tasks")?)?
        .into_iter()
        .map(|task| {
            Ok((
                pyobject::eq(pyvalue::item(&task, "id")?, &text(target_id)),
                task,
            ))
        })
        .collect::<R<Vec<_>>>()?
        .into_iter()
        .find_map(|(found, task)| found.then_some(task))
        .ok_or_else(|| Failure::Crash("StopIteration".into()))?;
    let packet_raw = read_file(&root.resolve_pin(pyvalue::item(&task, "packet")?)?)?;
    let manifest = current::read_json(
        &root
            .path
            .join(b"bench")
            .join(run.as_bytes())
            .join(b"manifest.json"),
    )?;
    let mut reference = None;
    for candidate in pyvalue::iterate(pyvalue::item(documents.get("reference"), "targets")?)? {
        if pyobject::eq(pyvalue::item(&candidate, "target")?, &text(target_id)) {
            reference = Some(candidate);
            break;
        }
    }
    let reference = reference.ok_or_else(|| Failure::Crash("StopIteration".into()))?;
    let pick = |value: &Value, fields: &[&str]| -> R<Value> {
        Ok(Value::Object(
            fields
                .iter()
                .map(|field| Ok((field.to_string(), pyvalue::item(value, field)?.clone())))
                .collect::<R<Object>>()?,
        ))
    };
    let families = pyvalue::iterate(pyvalue::item(&reference, "families")?)?
        .iter()
        .map(|family| pick(family, &FAMILY_FIELDS))
        .collect::<R<Vec<Value>>>()?;
    let no_problems = Value::Array(Vec::new());
    let pre_existing_problems = pyvalue::iterate(pyvalue::get_or(
        &reference,
        "pre_existing_problems",
        &no_problems,
    )?)?;
    let definition_fields: Vec<&str> = FAMILY_FIELDS
        .iter()
        .copied()
        .chain(["consequence", "grouping_reason"])
        .collect();
    let definitions = pre_existing_problems
        .iter()
        .map(|problem| pick(problem, &definition_fields))
        .collect::<R<Vec<Value>>>()?;
    let mut references = vec![
        ("target", text(target_id)),
        ("families", Value::Array(families.clone())),
    ];
    if v3 {
        references.push(("pre_existing_problems", Value::Array(definitions)));
    }
    let references_raw = format!("{}\n", dumps(&object(references), false));
    let family_ids: Vec<Value> = families
        .iter()
        .map(|family| pyvalue::item(family, "id").cloned())
        .collect::<Result<_, _>>()?;

    let mut reviews: Vec<Review> = Vec::new();
    for (attempt_id, doc) in &docs {
        let token = unused_token(runner.entropy, |token| {
            reviews.iter().any(|review| review.token == token)
        })?;
        let facts = &attempts
            .iter()
            .find(|(name, _)| name == attempt_id)
            .expect("selected")
            .1;
        reviews.push(Review {
            text: format!("# Review {token}\n\n{}", render(doc)?),
            token,
            attempt_id: attempt_id.clone(),
            items: items_of(doc).len(),
            review: pyvalue::item(facts, "review")?.clone(),
        });
    }
    let mut sources = Object::default();
    for review in &reviews {
        let doc = &docs
            .iter()
            .find(|(name, _)| *name == review.attempt_id)
            .expect("read")
            .1;
        let views = items_of(doc)
            .iter()
            .map(source_view)
            .collect::<R<Vec<Value>>>()?;
        sources.insert(
            review.token.clone(),
            object(vec![("items", Value::Array(views))]),
        );
    }
    let sources = Value::Object(sources);
    let inventory = match &options.inventory {
        Some(path) => Some(read_inventory(path, &reviews, &docs, &sources)?),
        None => None,
    };
    let target_path = directory.join(b"target.json");
    let mut target = current::read_json(&target_path)?;
    if let Some(manifest_path) = &options.cache_replacements
        && let Err(message) =
            provision::apply_replacement(&mut target, &target_path, &PyPath::new(manifest_path))?
    {
        return inconsistent(message);
    }
    let provisioning = pyvalue::item(&target, "provisioning")?.clone();
    let stripped = |value: &Value, field: &str| -> R<String> {
        Ok(pytext::strip(string(pyvalue::item(value, field)?)?).to_string())
    };
    let mut command = command_policy(&runner.checkout, target_id, &provisioning, &target)?;
    let profile_allowance = match pyvalue::get(&command, "allowance")? {
        Value::Null => String::new(),
        found => format!(" {}", pytext::strip(string(found)?)),
    };
    let allowance = format!(
        "{} {}{profile_allowance}\n\nUnavailable: {}\n\nHere `<clone>` is `clone/`, `<cache>` is `clone-cache/` and the work directory \
         is `clone-work/`, all in your working directory; from inside `clone/` they are `.`, `../clone-cache` and \
         `../clone-work`.",
        stripped(pyvalue::item(&manifest, "execution_policy")?, "allowance")?,
        stripped(&provisioning, "allowance")?,
        stripped(&provisioning, "unavailable")?
    );
    let mut listed: Vec<&Review> = reviews.iter().collect();
    listed.sort_by(|left, right| left.token.cmp(&right.token));
    let listing = listed
        .iter()
        .map(|review| {
            format!(
                "- `reviews/{}.md`: {} item{}",
                review.token,
                review.items,
                if review.items == 1 { "" } else { "s" }
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let joined_ids = family_ids
        .iter()
        .map(|id| string(id).map(str::to_string))
        .collect::<R<Vec<_>>>()?
        .join(", ");
    let family_text = if joined_ids.is_empty() {
        "none: no causal families are recorded; this does not establish that the entire PR is correct".to_string()
    } else {
        joined_ids
    };
    let mut prompt = template
        .replace("{TARGET}", target_id)
        .replace("{FAMILY_IDS}", &family_text)
        .replace("{REVIEWS}", &listing)
        .replace("{ALLOWANCE}", &allowance);
    let claim_evidence = options.claim_evidence.as_deref();
    let context = claim_context(
        &root,
        &runner.checkout,
        &documents,
        &reviews,
        target_id,
        &contract,
        claim_evidence,
    )?;
    if context
        .evidence
        .as_ref()
        .is_some_and(|packets| !packets.is_empty())
    {
        prompt += "\n\nclaims.md lists evidence/ files holding the pinned evidence, counterevidence and limits \
                   behind matched approved decisions. They support eligibility only.\n";
    }
    prompt += "\n\nUse the grading inspect/run tools for local inspection and focused tests. run takes argv, not \
               shell text. Use write_verdicts to save even unfinished output, then validate to report schema, \
               quote, coverage and pinned canonical violations before exit. To change a saved verdicts.json, use \
               edit_verdicts with the exact text to replace; send the whole file again only when most of it \
               changes.\n";
    if let Some(inventory) = &inventory {
        let _ = write!(
            prompt,
            "\n\n## Claims to grade\n\n```json\n{}\n```\n",
            dumps(inventory, false)
        );
        let mut listed_ids = false;
        for items in inventory.as_object().into_iter().flat_map(Object::iter) {
            for (_, item) in items.1.as_object().into_iter().flat_map(Object::iter) {
                if pyvalue::contains(item, &text("ids"))? {
                    listed_ids = true;
                }
            }
        }
        if listed_ids {
            prompt += "\nWhere an item lists ids, give its claims those ids, in that order.\n";
            if v3 {
                let _ = write!(
                    prompt,
                    "\nWhen a review's text states a pre-existing fault that this list does not record, note it as \
                     a concern about the list, with the evidence: write it in that item's note after the words \
                     \"{MISSING_FAULT}\".\n"
                );
            }
        }
    }
    let mut problems: Vec<Vec<u8>> = placeholders(&prompt)
        .into_iter()
        .map(|placeholder| format!("prompt.md keeps the placeholder {placeholder}").into_bytes())
        .collect();
    let mut identifying: BTreeSet<String> = BTreeSet::new();
    let cells = pyvalue::iterate(pyvalue::item(&selected, "cells")?)?;
    for (name, facts) in &attempts {
        identifying.insert(name.clone());
        let cell_id = pyvalue::item(facts, "cell")?;
        let mut arm = None;
        for cell in &cells {
            if pyobject::eq(pyvalue::item(cell, "id")?, cell_id) {
                arm = Some(pyvalue::item(cell, "arm")?.clone());
            }
        }
        let arm = arm.ok_or_else(|| Failure::Crash(format!("KeyError: {}", cell_id.py_repr())))?;
        identifying.insert(string(&arm)?.to_string());
    }
    identifying.insert(utf8(&name_of(run.as_bytes()), "run")?);
    let run_directory = pyfs::resolve(root.path.join(b"bench").join(run.as_bytes()).as_bytes());
    identifying.insert(utf8(&run_directory, "path")?);
    identifying.remove("");
    for found in &identifying {
        if contains(work.as_bytes(), found.as_bytes()) {
            problems
                .push(format!("grader workspace names {}", pytext::repr_str(found)).into_bytes());
        }
    }
    let mut named: Vec<(String, &str)> = vec![
        ("prompt.md".into(), prompt.as_str()),
        ("claims.md".into(), context.text.as_str()),
    ];
    named.extend(
        reviews
            .iter()
            .map(|review| (format!("reviews/{}.md", review.token), review.text.as_str())),
    );
    for (id, packet) in context.evidence.iter().flatten() {
        named.push((format!("evidence/{id}.md"), packet.text.as_str()));
    }
    for (name, body) in &named {
        for found in &identifying {
            if body.contains(found.as_str()) {
                problems.push(format!("{name} names {}", pytext::repr_str(found)).into_bytes());
            }
        }
    }
    for path in [
        work.as_bytes().to_vec(),
        home(),
        runner.checkout.as_bytes().to_vec(),
    ] {
        if contains(prompt.as_bytes(), &path) {
            problems.push([b"prompt.md names the absolute path ".as_slice(), &path].concat());
        }
    }
    if !problems.is_empty() {
        return inconsistent(problems.join(&b'\n'));
    }

    let mut snapshot = vec![
        ("contract", contract.clone()),
        ("families", Value::Array(family_ids.clone())),
        ("canonical", Value::Object(context.canonical.clone())),
        ("matches", Value::Object(context.matches.clone())),
        ("links", Value::Object(context.links.clone())),
        ("reviews", sources),
    ];
    if remedy_policy.is_some() {
        snapshot.push(("safety_policy", text(safety_policy)));
    }
    if let Some(inventory) = &inventory {
        snapshot.push(("inventory", inventory.clone()));
    }
    if v2_or_v3 {
        snapshot.push(("credits", Value::Object(context.credits.clone())));
    }
    if v3 {
        let ids = pre_existing_problems
            .iter()
            .map(|problem| pyvalue::item(problem, "id").cloned())
            .collect::<Result<Vec<_>, _>>()?;
        snapshot.push(("pre_existing_problems", Value::Array(ids)));
    }
    let snapshot_raw = dumps(&object(snapshot), false);
    if preflight_only {
        return preflight_target(
            runner,
            options,
            &directory,
            &target,
            target_id,
            reviews.len(),
            family_ids.len(),
            &fingerprint,
        );
    }
    let pin = runtime::pin(&runner.executable)?;
    make_directory(&current::parent_of(&key_path), true)?;
    make_directory(&work, true)?;
    provision_clone(runner, options, &directory, &work, &command)?;
    if let (Some(runtime), Value::Object(policy)) = (
        crate::grading_tools::python_runtime(&work.to_path_buf()),
        &mut command,
    ) {
        policy.insert("python_runtime".into(), runtime);
    }
    make_directory(&work.join(b"reviews"), false)?;
    make_directory(&work.join(b"validator"), true)?;
    write(
        &work.join(b"validator/inputs.json"),
        snapshot_raw.as_bytes(),
    )?;
    let policy_raw = dumps(&command, true);
    write(&work.join(b"command-policy.json"), policy_raw.as_bytes())?;
    let execution_policy = read_file(&runner.checkout.join(EXECUTION_POLICY))?;
    write(&work.join(b"execution-policy.md"), &execution_policy)?;
    for review in &reviews {
        write(
            &work.join(format!("reviews/{}.md", review.token).as_bytes()),
            review.text.as_bytes(),
        )?;
    }
    write(&work.join(b"references.json"), references_raw.as_bytes())?;
    write(&work.join(b"rubric.md"), &rubric)?;
    write(&work.join(b"packet.md"), &packet_raw)?;
    write(&work.join(b"prompt.md"), prompt.as_bytes())?;
    write(&work.join(b"claims.md"), context.text.as_bytes())?;
    if let Some(packets) = context
        .evidence
        .as_ref()
        .filter(|packets| !packets.is_empty())
    {
        make_directory(&work.join(b"evidence"), false)?;
        for (id, packet) in packets {
            write(
                &work.join(format!("evidence/{id}.md").as_bytes()),
                packet.text.as_bytes(),
            )?;
        }
    }
    let mut canonical_ids: Vec<&String> = context.canonical.iter().map(|(id, _)| id).collect();
    canonical_ids.sort();
    let mut claim_snapshot = vec![
        (
            "claims",
            Value::Array(canonical_ids.into_iter().map(|id| text(id)).collect()),
        ),
        (
            "context_sha256",
            Value::Str(sha256::hex_digest(context.text.as_bytes())),
        ),
    ];
    if let (Some(packets), Some(path)) = (&context.evidence, claim_evidence) {
        let extracts = Repository::new(root.path.as_bytes())
            .reference(path)
            .map_err(|failure| Failure::Crash(failure.0))?;
        let mut sorted: Vec<&(String, claims::blind::Packet)> = packets.iter().collect();
        sorted.sort_by(|left, right| left.0.cmp(&right.0));
        let pinned = sorted
            .into_iter()
            .map(|(id, packet)| {
                object(vec![
                    ("claim_id", text(id)),
                    ("path", Value::Str(format!("evidence/{id}.md"))),
                    (
                        "sha256",
                        Value::Str(sha256::hex_digest(packet.text.as_bytes())),
                    ),
                    ("sources", Value::Array(packet.sources.clone())),
                    ("withheld", Value::Array(packet.withheld.clone())),
                ])
            })
            .collect();
        claim_snapshot.push((
            "evidence",
            object(vec![
                ("contract", text(EVIDENCE_CONTRACT)),
                ("extracts", extracts),
                ("packets", Value::Array(pinned)),
            ]),
        ));
    }
    let mut prepared = Object::default();
    let mut paths: Vec<String> = [
        "packet.md",
        "prompt.md",
        "rubric.md",
        "claims.md",
        "references.json",
        "execution-policy.md",
    ]
    .map(str::to_string)
    .to_vec();
    for directory in ["reviews", "evidence"] {
        let mut names: Vec<String> = pyfs::names(work.join(directory.as_bytes()).as_bytes())
            .into_iter()
            .filter(|name| name.ends_with(b".md"))
            .map(|name| utf8(&name, "path").map(|name| format!("{directory}/{name}")))
            .collect::<R<_>>()?;
        names.sort();
        paths.extend(names);
    }
    for relative in paths {
        let sha = digest(&work.join(relative.as_bytes()))?;
        prepared.insert(relative, Value::Str(sha));
    }
    let mut deviation = runner_deviation(&runner.checkout, pin, &execution_policy)?;
    if let Some(replacement) = target
        .as_object()
        .and_then(|target| target.get("_cache_replacement"))
    {
        deviation.push(("provisioning", replacement.clone()));
    }
    let identifying_relative: Vec<Value> = identifying
        .iter()
        .filter(|found| !found.starts_with('/'))
        .map(|found| text(found))
        .collect();
    let mut key = vec![
        ("contract", text(KEY_CONTRACT)),
        ("run", text(&run)),
        ("target", text(target_id)),
        ("input_fingerprint", Value::Str(fingerprint)),
        (
            "current_record",
            object(vec![
                ("root", Value::Str(utf8(root.path.as_bytes(), "path")?)),
                ("current", Value::Str(utf8(&options.current, "path")?)),
            ]),
        ),
        ("workspace_identity_blinded", Value::Bool(true)),
        ("identifying", Value::Array(identifying_relative)),
        (
            "prompt_sha256",
            Value::Str(sha256::hex_digest(prompt.as_bytes())),
        ),
        ("created_at", Value::Str(normalize_review::utc_now())),
        ("prepared_files", Value::Object(prepared)),
        (
            "validator",
            object(vec![(
                "inputs.json",
                Value::Str(sha256::hex_digest(snapshot_raw.as_bytes())),
            )]),
        ),
        (
            "command_policy_sha256",
            Value::Str(sha256::hex_digest(policy_raw.as_bytes())),
        ),
        (
            "profiles_sha256",
            pyvalue::item(&command, "profiles_sha256")?.clone(),
        ),
        ("runner_deviation", object(deviation)),
        ("claim_snapshot", object(claim_snapshot)),
        (
            "reviews",
            Value::Array(
                reviews
                    .iter()
                    .map(|review| {
                        object(vec![
                            ("token", text(&review.token)),
                            ("attempt_id", text(&review.attempt_id)),
                            ("items", Value::Int(review.items.to_string())),
                            ("review", review.review.clone()),
                        ])
                    })
                    .collect(),
            ),
        ),
    ];
    if let Some(remedy_policy) = remedy_policy {
        key.push(("remedy_policy", remedy_policy));
    }
    write_key(
        &key_path,
        format!("{}\n", dumps(&object(key), true)).as_bytes(),
    )?;
    let items: usize = reviews.iter().map(|review| review.items).sum();
    Ok([
        b"prepared ".as_slice(),
        work.as_bytes(),
        format!(
            ": {} reviews, {items} items, {} causal families; key ",
            reviews.len(),
            family_ids.len()
        )
        .as_bytes(),
        key_path.as_bytes(),
        b"\n",
    ]
    .concat())
}

fn blind_token(entropy: &mut dyn Entropy) -> R<String> {
    let mut bytes = [0u8; 3];
    entropy
        .fill(&mut bytes)
        .map_err(|failure| Failure::Input(failure.0))?;
    Ok(format!(
        "blind-{}",
        bytes.iter().fold(String::new(), |mut out, byte| {
            let _ = write!(out, "{byte:02x}");
            out
        })
    ))
}

fn unused_token(entropy: &mut dyn Entropy, taken: impl Fn(&str) -> bool) -> R<String> {
    loop {
        let token = blind_token(entropy)?;
        if !taken(&token) {
            return Ok(token);
        }
    }
}

/// The `--inventory` of a prior v2 grade, by token and renumbered.
fn read_inventory(
    path: &[u8],
    reviews: &[Review],
    docs: &[(String, Value)],
    sources: &Value,
) -> R<Value> {
    let supplied = grading_validation::read_verdicts(path)
        .map_err(|message| Failure::Inconsistent(format!("inventory: {message}").into_bytes()))?;
    let covers = supplied.as_object().is_some_and(|supplied| {
        let keys: BTreeSet<&str> = supplied.iter().map(|(name, _)| name.as_str()).collect();
        let names: BTreeSet<&str> = docs.iter().map(|(name, _)| name.as_str()).collect();
        keys == names
    });
    if !covers {
        return inconsistent("inventory must cover exactly the workspace's reviews");
    }
    let mut by_token = Object::default();
    for review in reviews {
        by_token.insert(
            review.token.clone(),
            pyvalue::item(&supplied, &review.attempt_id)?.clone(),
        );
    }
    let by_token = Value::Object(by_token);
    let found = grading_validation::inventory_problems(&by_token, sources)?;
    if !found.is_empty() {
        return inconsistent(found.join("\n"));
    }
    let mut renumbered = Object::default();
    for (token, items) in by_token.as_object().into_iter().flat_map(Object::iter) {
        let count = pyvalue::length(items)?;
        let mut numbered = Object::default();
        for number in 1..=count {
            let name = number.to_string();
            numbered.insert(name.clone(), pyvalue::item(items, &name)?.clone());
        }
        renumbered.insert(token.clone(), Value::Object(numbered));
    }
    Ok(Value::Object(renumbered))
}

/// `provision.archive_path` and the archive check of the offline preflight, after `bench-tools provision check`.
#[allow(
    clippy::too_many_arguments,
    reason = "each argument is a separate input the caller already holds; a one-off parameter struct would not simplify the call"
)]
fn preflight_target(
    runner: &Runner,
    options: &Options,
    directory: &PyPath,
    target: &Value,
    target_id: &str,
    reviews: usize,
    families: usize,
    fingerprint: &str,
) -> R<Vec<u8>> {
    let cache_root = match options.cache_root.as_ref() {
        Some(root) => root.clone(),
        None => [home(), b"/.t3/bench-cache".to_vec()].concat(),
    };
    let mut command = provisioner(runner, options);
    command.extend([
        b"check".to_vec(),
        b"--target".to_vec(),
        directory.as_bytes().to_vec(),
        b"--cache-root".to_vec(),
        cache_root.clone(),
    ]);
    let done = provision::run(&command)?;
    let output = done.output()?;
    if done.code != 0 {
        return inconsistent(format!(
            "pinned source/cache preflight: {}",
            pytext::strip(&output)
        ));
    }
    let provisioning = pyvalue::item(target, "provisioning")?;
    let empty_cache = object(vec![("kind", text(provision::EMPTY_CACHE_KIND))]);
    let cache = pyvalue::get_or(provisioning, "cache", &empty_cache)?;
    let kind = pyvalue::item(cache, "kind")?;
    if !pyobject::eq(kind, &text(provision::EMPTY_CACHE_KIND)) {
        let id = string(pyvalue::item(target, "id")?)?;
        let wanted = text(&format!("cache archive ({})", kind.py_str()));
        let no_identities = Value::Array(Vec::new());
        let mut expected = Vec::new();
        for value in pyvalue::iterate(pyvalue::get_or(
            provisioning,
            "dependency_identity",
            &no_identities,
        )?)? {
            if pyobject::eq(pyvalue::item(&value, "name")?, &wanted) {
                expected.push(pyvalue::item(&value, "sha256")?.clone());
            }
        }
        let keyed = match expected.as_slice() {
            [only] => only.as_str(),
            _ => None,
        };
        let archive =
            crate::provision::archive_path(&PyPath::new(&cache_root).to_path_buf(), id, keyed);
        let archive = PyPath::new(std::os::unix::ffi::OsStrExt::as_bytes(archive.as_os_str()));
        let matches = expected.len() == 1
            && archive.is_file()
            && pyvalue::read_bytes(&archive)
                .map_err(Failure::from)
                .is_ok_and(|raw| pyobject::eq(&Value::Str(sha256::hex_digest(&raw)), &expected[0]));
        if !matches {
            return inconsistent("pinned dependency archive missing or changed");
        }
    }
    Ok(format!(
        "preflight passed: {target_id}, {reviews} saved reviews, {families} causal families, inputs {}\n",
        &fingerprint[..fingerprint.len().min(12)]
    )
    .into_bytes())
}

/// `python3 SCRIPT` for `--provision SCRIPT`, otherwise this executable's `provision`.
fn provisioner(runner: &Runner, options: &Options) -> Vec<Vec<u8>> {
    use std::os::unix::ffi::OsStrExt;
    match &options.provision {
        Some(script) => vec![b"python3".to_vec(), script.clone()],
        None => vec![
            runner.executable.as_os_str().as_bytes().to_vec(),
            b"provision".to_vec(),
        ],
    }
}

/// Run the provisioner into WORK/clone, with the command profile's `post_clone` steps after the target's; on
/// failure remove what it left and report its exit and output tail.
fn provision_clone(
    runner: &Runner,
    options: &Options,
    directory: &PyPath,
    work: &PyPath,
    command_policy: &Value,
) -> R<()> {
    let mut command = provisioner(runner, options);
    command.extend([
        b"prepare".to_vec(),
        b"--target".to_vec(),
        directory.as_bytes().to_vec(),
        b"--out".to_vec(),
        work.join(b"clone").as_bytes().to_vec(),
    ]);
    if let Some(cache_root) = &options.cache_root {
        command.extend([b"--cache-root".to_vec(), cache_root.clone()]);
    }
    if let Some(manifest) = &options.cache_replacements {
        command.extend([b"--cache-replacements".to_vec(), manifest.clone()]);
    }
    if let supplement @ Value::Object(_) = pyvalue::get(command_policy, "dependency_supplement")? {
        let archive = runner
            .checkout
            .join(string(pyvalue::item(supplement, "path")?)?.as_bytes());
        let expected = string(pyvalue::item(supplement, "sha256")?)?;
        if digest(&archive)? != expected {
            return inconsistent("dependency supplement SHA-256 mismatch");
        }
        command.extend([
            b"--supplement".to_vec(),
            archive.as_bytes().to_vec(),
            b"--supplement-sha256".to_vec(),
            expected.as_bytes().to_vec(),
        ]);
    }
    if let Value::Array(steps) = pyvalue::get(command_policy, "post_clone")? {
        for step in steps {
            command.extend([b"--post-clone".to_vec(), string(step)?.as_bytes().to_vec()]);
        }
    }
    let done = provision::run(&command)?;
    if done.code == 0 {
        return done.output().map(drop);
    }
    let mut lines = vec![
        format!(
            "{} exited {}",
            pytext::stderr_text(&command.join(&b' ')),
            done.code
        ),
        done.tail(2000),
    ];
    for name in ["clone", "clone-cache", "clone-work"] {
        if let Err(message) = provision::remove_tree(work.join(name.as_bytes()).as_bytes()) {
            lines.push(message);
        }
    }
    Err(Failure::Input(lines.join("\n")))
}

/// The key, created only when the path is unused and readable by its owner alone.
fn write_key(path: &PyPath, bytes: &[u8]) -> R<()> {
    use std::io::Write;
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    let failure = |error: std::io::Error| Failure::Input(pytext::os_error(&error, path.as_bytes()));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path.to_path_buf())
        .map_err(failure)?;
    file.set_permissions(std::fs::Permissions::from_mode(0o600))
        .map_err(failure)?;
    file.write_all(bytes).map_err(failure)
}

/// The offline preflight checks for every named target, or every selected batch of the run: the
/// lines printed, in order, as far as they got before a failure.
pub fn preflight_inputs(
    options: &Options,
    targets: &[String],
    work_root: &[u8],
    key_root: &[u8],
    runner: &mut Runner,
    printed: &mut Vec<u8>,
) -> R<()> {
    let root = Root::new(&options.root)?;
    let run = run_identity(&options.run)?;
    let loaded = current::load_current(&root, &options.current, false)?;
    let targets = if targets.is_empty() {
        let mut found = Vec::new();
        for batch in pyvalue::iterate(pyvalue::item(&loaded.0, "batches")?)? {
            if pyobject::eq(pyvalue::item(&batch, "run")?, &text(&run)) {
                found.push(string(pyvalue::item(&batch, "target")?)?.to_string());
            }
        }
        found
    } else {
        targets.to_vec()
    };
    if targets.is_empty() {
        return inconsistent(format!(
            "{run} has no selected batch in the current inventory"
        ));
    }
    for target in &targets {
        let mut each = options.clone();
        each.target = target.clone();
        each.work = PyPath::new(work_root)
            .join(target.as_bytes())
            .as_bytes()
            .to_vec();
        each.key = PyPath::new(key_root)
            .join(format!("{target}.json").as_bytes())
            .as_bytes()
            .to_vec();
        let line = prepare(&each, runner, Some(&loaded), true)?;
        printed.extend(line);
    }
    printed.extend(
        format!(
            "offline queue preflight passed for {} targets; client, credentials, pricing and dispatch enforcement \
             unchecked\n",
            targets.len()
        )
        .into_bytes(),
    );
    Ok(())
}

/// The names in a workspace's evidence directory; none when it does not exist, and an error when it cannot be read.
fn evidence_names(directory: &PyPath) -> R<Vec<Vec<u8>>> {
    use std::os::unix::ffi::OsStrExt;
    let unreadable = |error: std::io::Error| {
        Failure::Input(format!(
            "cannot read {}: {}",
            directory.display(),
            pytext::os_error_without_file(&error)
        ))
    };
    let entries = match std::fs::read_dir(directory.to_path_buf()) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        entries => entries.map_err(unreadable)?,
    };
    entries
        .map(|entry| Ok(entry.map_err(unreadable)?.file_name().as_bytes().to_vec()))
        .collect()
}

fn pinned_sha(key_field: &Value) -> Option<&Value> {
    key_field.truthy().then_some(key_field)
}

/// The problems that make a prepared workspace no longer the one `key` pins. Dispatching also requires the key's
/// runner to be the running `bench-tools` executable, so a batch another runner prepared is refused.
pub fn check_prepared(
    checkout: &PyPath,
    work: &PyPath,
    key: &Value,
    dispatching: bool,
    executable: &Path,
) -> R<Vec<String>> {
    let mut problems = Vec::new();
    let differs = |path: &PyPath, pinned: &Value| -> R<bool> {
        Ok(!pyobject::eq(&Value::Str(digest(path)?), pinned))
    };
    if dispatching
        && let Some(pinned) = pinned_sha(pyvalue::get(key, "profiles_sha256")?)
        && differs(&profiles_path(checkout), pinned)?
    {
        problems.push("command profiles changed after preparation".to_string());
    }
    let empty = Value::Object(Object::default());
    let entries = |field: &str| -> R<Vec<(String, Value)>> {
        match pyvalue::get_or(key, field, &empty)? {
            Value::Object(entries) => Ok(entries
                .iter()
                .map(|(name, value)| (name.clone(), value.clone()))
                .collect()),
            other => crash(format!(
                "AttributeError: '{}' object has no attribute 'items'",
                other.type_name()
            )),
        }
    };
    for (relative, pinned) in entries("prepared_files")? {
        if differs(&work.join(relative.as_bytes()), &pinned)? {
            problems.push("grading inputs changed after preparation".to_string());
        }
    }
    for (relative, pinned) in entries("validator")? {
        if differs(&work.join(b"validator").join(relative.as_bytes()), &pinned)? {
            problems.push("blinded validator changed after preparation".to_string());
        }
    }
    if let Some(pinned) = pinned_sha(pyvalue::get(key, "command_policy_sha256")?)
        && differs(&work.join(b"command-policy.json"), pinned)?
    {
        problems.push("command policy changed after preparation".to_string());
    }
    let none = Value::Array(Vec::new());
    let snapshot = pyvalue::get_or(key, "claim_snapshot", &empty)?;
    let evidence = pyvalue::get_or(snapshot, "evidence", &empty)?;
    let mut pinned_packets = Vec::new();
    for packet in pyvalue::iterate(pyvalue::get_or(evidence, "packets", &none)?)? {
        pinned_packets.push(pyvalue::item(&packet, "path")?.clone());
    }
    let pinned_packets = pyvalue::distinct(&pinned_packets)?;
    let present: Vec<Value> = evidence_names(&work.join(b"evidence"))?
        .into_iter()
        .map(|name| Value::os_text(&[b"evidence/".as_slice(), &name].concat()))
        .collect();
    if present.len() != pinned_packets.len()
        || present.iter().any(|path| !member(&pinned_packets, path))
    {
        problems.push("evidence packets changed after preparation".to_string());
    }
    if !dispatching {
        return Ok(problems);
    }
    let deviation = pyvalue::get_or(key, "runner_deviation", &empty)?;
    if let Some(pinned) = pinned_sha(pyvalue::get(deviation, "execution_policy_sha256")?)
        && differs(&checkout.join(EXECUTION_POLICY), pinned)?
    {
        problems.push("execution policy changed after preparation".to_string());
    }
    if let Some(pinned) = pinned_sha(pyvalue::get(deviation, "codex_catalog_sha256")?)
        && differs(&checkout.join(CODEX_CATALOG), pinned)?
    {
        problems.push("Codex grading catalog changed after preparation".to_string());
    }
    let replacement = pyvalue::get(deviation, "provisioning")?;
    if replacement.truthy() {
        let path_of = |value: &Value| -> R<PyPath> { Ok(PyPath::new(string(value)?.as_bytes())) };
        let target_path = path_of(pyvalue::item(replacement, "target_path")?)?;
        match current::read_json(&target_path) {
            Ok(mut target) => {
                let manifest = path_of(pyvalue::item(
                    pyvalue::item(replacement, "manifest")?,
                    "path",
                )?)?;
                match provision::apply_replacement(&mut target, &target_path, &manifest)? {
                    Ok(()) => {
                        if !pyobject::eq(pyvalue::item(&target, "_cache_replacement")?, replacement)
                        {
                            problems
                                .push("cache replacement changed after preparation".to_string());
                        }
                    }
                    Err(message) => problems.push(message),
                }
            }
            Err(Failure::Input(message)) => problems.push(message),
            Err(other) => return Err(other),
        }
    }
    let version = pyvalue::get(deviation, "version")?;
    if pyobject::eq(version, &Value::Int(RUNNER_VERSION.to_string())) {
        if let Some(problem) = runtime::problem(
            pyvalue::get(deviation, "bench_tools")?,
            executable,
            "the key's pinned bench-tools is unavailable: ",
        ) {
            problems.push(problem);
        }
    } else {
        let shown = match version {
            Value::Null => "1".to_string(),
            other => pyjson::dumps(other, &Dumps::default()),
        };
        problems.push(format!(
            "prepared by runner identity v{shown}; prepare the batch again"
        ));
    }
    Ok(problems)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;

    struct Scratch(PathBuf);

    impl Scratch {
        fn new(label: &str) -> Scratch {
            static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
            let serial = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "bench-tools-grade-{label}-{}-{serial}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).unwrap();
            Scratch(path)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn py_path(path: &Path) -> PyPath {
        PyPath::new(pytext::os_bytes(path.as_os_str()))
    }

    fn checkout() -> PyPath {
        py_path(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
    }

    struct Store {
        _scratch: Scratch,
        executable: PathBuf,
        receipt: PathBuf,
    }

    /// An installed entry as `bench-tools runtime install` lays it out: `<store>/bench-tools/<sha>/bench-tools`.
    fn store(label: &str) -> Store {
        store_of(label, b"#!/bin/sh\n")
    }

    fn store_of(label: &str, body: &[u8]) -> Store {
        let scratch = Scratch::new(label);
        let sha = sha256::hex_digest(body);
        let entry = scratch.0.join("store/bench-tools").join(&sha);
        std::fs::create_dir_all(&entry).unwrap();
        let executable = entry.join("bench-tools");
        std::fs::write(&executable, body).unwrap();
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o755)).unwrap();
        let receipt = entry.join("receipt.json");
        std::fs::write(&receipt, complete_receipt(&sha)).unwrap();
        Store {
            _scratch: scratch,
            executable,
            receipt,
        }
    }

    fn complete_receipt(sha: &str) -> String {
        let commit = "0".repeat(40);
        let zero = "0".repeat(64);
        format!(
            r#"{{"schema": "rust-runtime-receipt/1",
 "crate": {{"name": "bench-tools", "bin": "bench-tools", "path": "rust/bench-tools"}},
 "source": {{"revision": "{commit}", "tree": "{commit}", "cargo_toml_sha256": "{zero}",
  "cargo_lock_sha256": "{zero}", "toolchain_file_sha256": "{zero}"}},
 "toolchain": {{"rustc": {{"release": "1.99.0"}}}}, "target": "x86_64-unknown-linux-gnu",
 "build": {{"command": ["cargo", "build"]}},
 "artifact": {{"sha256": "{sha}", "file": "bench-tools", "size": 10}}}}"#
        )
    }

    fn current_key(store: &Store) -> Value {
        object(vec![(
            "runner_deviation",
            object(vec![
                ("version", Value::Int(RUNNER_VERSION.to_string())),
                ("bench_tools", runtime::pin(&store.executable).unwrap()),
            ]),
        )])
    }

    fn shown(path: &Path) -> String {
        pytext::stderr_text(pytext::os_bytes(path.as_os_str()))
    }

    fn problems(store: &Store, key: &Value) -> Vec<String> {
        let work = Scratch::new("work");
        check_prepared(&checkout(), &py_path(&work.0), key, true, &store.executable).unwrap()
    }

    #[test]
    fn a_current_key_passes_when_the_running_executable_is_the_pinned_one() {
        let store = store("matching");
        assert_eq!(problems(&store, &current_key(&store)), Vec::<String>::new());
    }

    #[test]
    fn a_missing_or_changed_executable_or_receipt_is_one_problem_naming_it() {
        type Case = (&'static str, fn(&Store), fn(&Store) -> String);
        let cases: [Case; 4] = [
            (
                "executable-missing",
                |store| std::fs::remove_file(&store.executable).unwrap(),
                |store| {
                    format!(
                        "the pinned bench-tools executable {} is missing",
                        shown(&store.executable)
                    )
                },
            ),
            (
                "executable-changed",
                |store| std::fs::write(&store.executable, b"#!/bin/sh\nexit 1\n").unwrap(),
                |store| {
                    format!(
                        "the pinned bench-tools executable {} no longer hashes to its pin",
                        shown(&store.executable)
                    )
                },
            ),
            (
                "receipt-missing",
                |store| std::fs::remove_file(&store.receipt).unwrap(),
                |store| format!("{} is missing", shown(&store.receipt)),
            ),
            (
                "receipt-changed",
                |store| {
                    let raw = std::fs::read_to_string(&store.receipt).unwrap();
                    std::fs::write(&store.receipt, raw + "\n").unwrap();
                },
                |store| format!("{} no longer hashes to its pin", shown(&store.receipt)),
            ),
        ];
        for (label, change, expected) in cases {
            let store = store(label);
            let key = current_key(&store);
            change(&store);
            assert_eq!(
                problems(&store, &key),
                [format!(
                    "the key's pinned bench-tools is unavailable: {}",
                    expected(&store)
                )],
                "{label}"
            );
        }
    }

    #[test]
    fn an_incomplete_receipt_is_refused_as_the_runtime_check_refuses_it() {
        let store = store("incomplete");
        let sha = sha256::hex_digest(b"#!/bin/sh\n");
        let minimal = format!(
            r#"{{"crate": {{"name": "bench-tools"}}, "artifact": {{"sha256": "{sha}", "file": "bench-tools"}}}}"#
        );
        std::fs::write(&store.receipt, &minimal).unwrap();
        let why = format!(
            "{}: a field is missing or malformed: schema",
            shown(&store.receipt)
        );
        let pin = object(vec![
            ("sha256", text(&sha)),
            (
                "receipt_sha256",
                text(&sha256::hex_digest(minimal.as_bytes())),
            ),
        ]);
        assert_eq!(runtime::problem(&pin, &store.executable, ""), Some(why));
        assert!(runtime::pin(&store.executable).is_err());
    }

    #[test]
    fn a_batch_another_executable_prepared_is_refused() {
        let preparer = store("preparer");
        let body = b"#!/bin/sh\nexit 0\n";
        let sha = sha256::hex_digest(body);
        let entry = preparer.executable.parent().unwrap().with_file_name(&sha);
        std::fs::create_dir_all(&entry).unwrap();
        let running = entry.join("bench-tools");
        std::fs::write(&running, body).unwrap();
        std::fs::write(entry.join("receipt.json"), complete_receipt(&sha)).unwrap();
        let work = Scratch::new("other-work");
        let found = check_prepared(
            &checkout(),
            &py_path(&work.0),
            &current_key(&preparer),
            true,
            &running,
        )
        .unwrap();
        assert_eq!(
            found,
            [format!(
                "the running bench-tools {} is not the pinned executable {}",
                shown(&running),
                shown(&preparer.executable)
            )]
        );
    }

    #[test]
    fn a_key_of_an_earlier_runner_identity_is_refused_for_dispatch() {
        let store = store("earlier");
        for (version, shown) in [(Some("2"), "2"), (None, "1")] {
            let mut entries = vec![("bench_tools", runtime::pin(&store.executable).unwrap())];
            if let Some(version) = version {
                entries.push(("version", Value::Int(version.to_string())));
            }
            let key = object(vec![("runner_deviation", object(entries))]);
            assert_eq!(
                problems(&store, &key),
                [format!(
                    "prepared by runner identity v{shown}; prepare the batch again"
                )]
            );
        }
    }

    #[test]
    fn an_unreadable_evidence_directory_is_an_error_not_an_empty_one() {
        let store = store("evidence");
        let work = Scratch::new("unreadable");
        let evidence = work.0.join("evidence");
        std::fs::create_dir(&evidence).unwrap();
        std::fs::set_permissions(&evidence, std::fs::Permissions::from_mode(0o000)).unwrap();
        let blocked = std::fs::read_dir(&evidence).is_err();
        let checked = check_prepared(
            &checkout(),
            &py_path(&work.0),
            &current_key(&store),
            true,
            &store.executable,
        );
        std::fs::set_permissions(&evidence, std::fs::Permissions::from_mode(0o755)).unwrap();
        if blocked {
            assert!(matches!(checked, Err(Failure::Input(_))));
        }
    }

    #[test]
    fn a_key_is_created_once_and_readable_by_its_owner_alone() {
        let scratch = Scratch::new("key");
        let path = py_path(&scratch.0.join("key.json"));
        write_key(&path, b"{}\n").unwrap();
        let mode = std::fs::metadata(path.to_path_buf())
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
        assert!(write_key(&path, b"[]\n").is_err());
        assert_eq!(std::fs::read(path.to_path_buf()).unwrap(), b"{}\n");
    }

    struct Replay(Vec<[u8; 3]>);

    impl Entropy for Replay {
        fn fill(&mut self, buffer: &mut [u8]) -> Result<(), claims::Failure> {
            buffer.copy_from_slice(&self.0.remove(0));
            Ok(())
        }
    }

    #[test]
    fn a_token_already_given_is_drawn_again() {
        let mut entropy = Replay(vec![
            [0xab, 0xcd, 0xef],
            [0xab, 0xcd, 0xef],
            [0x01, 0x02, 0x03],
        ]);
        let first = unused_token(&mut entropy, |_| false).unwrap();
        assert_eq!(first, "blind-abcdef");
        let second = unused_token(&mut entropy, |token| token == first).unwrap();
        assert_eq!(second, "blind-010203");
    }

    #[test]
    fn the_newest_profile_version_supplies_the_command_policy() {
        let scratch = Scratch::new("profiles");
        let root = py_path(&scratch.0);
        let policies = scratch.0.join("bench/policies");
        std::fs::create_dir_all(&policies).unwrap();
        std::fs::write(
            policies.join("grading-commands.v1.json"),
            r#"{"version": 1, "targets": {"t-1": {"test_kind": "go", "once": true}}}"#,
        )
        .unwrap();
        let provisioning = object(vec![("allowance", text("Run tests."))]);
        let policy =
            |target: &str| command_policy(&root, target, &provisioning, &Value::Null).unwrap();
        let only_v1 = policy("t-1");
        assert!(pyobject::eq(
            pyvalue::item(&only_v1, "version").unwrap(),
            &Value::Int("1".into())
        ));
        assert_eq!(
            pyvalue::item(&only_v1, "test_kind").unwrap().as_str(),
            Some("go")
        );
        let v2 = r#"{"version": 2, "targets": {"t-1": {"once": true, "commands": {"node": {}},
            "post_clone": ["make fixtures"], "allowance": "Run node."}}}"#;
        std::fs::write(policies.join("grading-commands.v2.json"), v2).unwrap();
        let newest = policy("t-1");
        assert!(pyobject::eq(
            pyvalue::item(&newest, "version").unwrap(),
            &Value::Int("2".into())
        ));
        assert_eq!(
            pyvalue::item(&newest, "profiles_sha256").unwrap().as_str(),
            Some(sha256::hex_digest(v2.as_bytes()).as_str())
        );
        assert_eq!(
            pyvalue::item(&newest, "allowance").unwrap().as_str(),
            Some("Run node.")
        );
        assert!(matches!(
            pyvalue::get(&newest, "test_kind").unwrap(),
            Value::Null
        ));
        let unlisted = policy("t-2");
        assert_eq!(
            pyvalue::item(&unlisted, "test_kind").unwrap().as_str(),
            Some("none")
        );
    }
}
