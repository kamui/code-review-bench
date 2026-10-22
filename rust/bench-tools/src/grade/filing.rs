//! `bench-tools grade validate`, `inventory`, `map` and `invalidate`, and the assessment steps that
//! `bench-tools evaluator-audit second` shares with `map`.

use super::{
    Runner, abspath, check_attempts, check_prepared, crash, dumps, inconsistent, make_directory,
    name_of, object, provision, read_file, realpath, runner_deviation, string, text,
    unchanged_inputs, utf8, write,
};
use crate::current_grading::{self as current, Documents, Failure, R, Root};
use crate::pyjson::{Object, Value};
use crate::pytext::{self, PyPath};
use crate::pyvalue::{self, Raised};
use crate::{claim_grading, grading_validation, pre_existing, pyfs, pyobject, pyunicode, sha256};
use std::cmp::Ordering;

const RECEIPT_CONTRACT: &str = "current-assessment-receipt/v1";
/// `sorted(codex_grading.AUX_TOOL_NAMES)`.
const AUX_TOOL_NAMES: [&str; 3] = [
    "list_mcp_resource_templates",
    "list_mcp_resources",
    "read_mcp_resource",
];
const MAP_CLEANUP: &str =
    "the verdicts passed every check, but workspace cleanup failed, so no grades were written: ";
const MALFORMED_VERDICTS: &str = "verdicts.json: unreadable or malformed JSON, or duplicate keys";

/// Independent safety checks by `(review token, recommendation id)`, in first-seen order.
pub type Checks = Vec<((Value, Value), Vec<Value>)>;

/// The new candidates an assessment raises, by their id in the verdicts: the register id and its anchors.
pub type Candidates = Vec<(Value, Value)>;

/// One safety check file: who checked, whom it is independent of, and each row's review, recommendation, result and
/// reason.
type CheckFile = (Value, Value, Vec<[Value; 4]>);

fn int(number: usize) -> Value {
    Value::Int(number.to_string())
}

fn is(value: &Value, literal: &str) -> bool {
    pyobject::eq(value, &text(literal))
}

fn lookup<'a, T>(entries: &'a [(Value, T)], key: &Value) -> Option<&'a T> {
    entries
        .iter()
        .find(|(known, _)| pyobject::same(known, key))
        .map(|(_, value)| value)
}

/// `d[key] = value`: a new key goes last, an existing one keeps its place.
fn assign<T>(entries: &mut Vec<(Value, T)>, key: Value, value: T) {
    match entries
        .iter_mut()
        .find(|(known, _)| pyobject::same(known, &key))
    {
        Some(entry) => entry.1 = value,
        None => entries.push((key, value)),
    }
}

fn integer(value: &Value) -> R<i128> {
    match value {
        Value::Int(digits) => digits
            .parse()
            .map_err(|_| Failure::Crash(format!("unsupported integer {digits}"))),
        Value::Bool(flag) => Ok(i128::from(*flag)),
        other => crash(format!(
            "TypeError: unsupported operand type(s) for -: '{}' and 'int'",
            other.type_name()
        )),
    }
}

/// `int(text)`.
fn python_int(text: &str) -> R<i128> {
    let digits = pyunicode::parse_int(text)
        .map_err(|message| Failure::Crash(format!("ValueError: {message}")))?;
    digits
        .parse()
        .map_err(|_| Failure::Crash(format!("unsupported integer {digits}")))
}

/// `sorted(values, key=lambda v: v[field])`.
pub fn sorted_by(values: &Value, field: &str) -> R<Vec<Value>> {
    let mut keyed = Vec::new();
    for value in pyvalue::iterate(values)? {
        keyed.push((pyvalue::item(&value, field)?.clone(), value));
    }
    sort_keyed(keyed)
}

fn sort_keyed(mut keyed: Vec<(Value, Value)>) -> R<Vec<Value>> {
    if let Some((first, _)) = keyed.first() {
        for (key, _) in &keyed[1..] {
            pyvalue::compare(first, key)?;
        }
    }
    keyed.sort_by(|left, right| pyvalue::compare(&left.0, &right.0).unwrap_or(Ordering::Equal));
    Ok(keyed.into_iter().map(|(_, value)| value).collect())
}

/// `Path(value).resolve()`.
pub fn resolve(path: &[u8]) -> R<PyPath> {
    Ok(PyPath::new(&realpath(&abspath(path)?)?))
}

/// `read_verdicts(work)`: the raw bytes, the verdicts and their problems against WORK's blinded validator inputs.
pub fn read_verdicts(work: &PyPath) -> R<(Vec<u8>, Value, Vec<String>)> {
    let path = work.join(b"verdicts.json");
    let raw = read_file(&path)?;
    let malformed = || Failure::Inconsistent(MALFORMED_VERDICTS.as_bytes().to_vec());
    let verdicts = grading_validation::read_verdicts(path.as_bytes()).map_err(|_| malformed())?;
    let snapshot = current::read_json(&work.join(b"validator/inputs.json"))?;
    let problems =
        grading_validation::validate(&verdicts, &snapshot).map_err(|raised| match raised {
            pyobject::Raised::Refused => Failure::from(raised),
            _ => malformed(),
        })?;
    Ok((raw, verdicts, problems))
}

/// `bench-tools grade validate`.
pub fn validate(work: &[u8]) -> R<Vec<u8>> {
    let (_, _, problems) = read_verdicts(&PyPath::new(work))?;
    if problems.is_empty() {
        return Ok(
            b"verdicts.json satisfies the blinded output contract; no judgment was checked\n"
                .to_vec(),
        );
    }
    inconsistent(problems.join("\n"))
}

/// `bench-tools grade inventory`: each review's item kinds and claim quotes, for a second grader's workspace.
pub fn inventory(checkout: &PyPath, work: &[u8], key: &[u8], out: &[u8]) -> R<Vec<u8>> {
    let work = resolve(work)?;
    let key = current::read_json(&PyPath::new(key))?;
    let problems = check_prepared(checkout, &work, &key, false, std::path::Path::new(""))?;
    if !problems.is_empty() {
        return inconsistent(problems.join("\n"));
    }
    let snapshot = current::read_json(&work.join(b"validator/inputs.json"))?;
    let contract = pyvalue::item(&snapshot, "contract")?;
    if !is(contract, grading_validation::CONTRACT_V2) && !is(contract, pre_existing::CONTRACT) {
        return inconsistent("inventory requires a current-verdicts/v2 workspace");
    }
    let (_, verdicts, problems) = read_verdicts(&work)?;
    if !problems.is_empty() {
        return inconsistent(problems.join("\n"));
    }
    let mut result = Object::default();
    for review in pyvalue::iterate(pyvalue::item(&key, "reviews")?)? {
        let items = pyvalue::item(
            pyvalue::subscript(
                pyvalue::item(&verdicts, "reviews")?,
                pyvalue::item(&review, "token")?,
            )?,
            "items",
        )?;
        let mut numbered = Object::default();
        for (number, item) in items.as_object().into_iter().flat_map(Object::iter) {
            let mut quotes = Vec::new();
            for claim in pyvalue::iterate(pyvalue::item(item, "claims")?)? {
                quotes.push(pyvalue::item(&claim, "quote")?.clone());
            }
            numbered.insert(
                number.clone(),
                object(vec![
                    ("kind", pyvalue::item(item, "kind")?.clone()),
                    ("quotes", Value::Array(quotes)),
                ]),
            );
        }
        result.insert(
            string(pyvalue::item(&review, "attempt_id")?)?.to_string(),
            Value::Object(numbered),
        );
    }
    replace_json(&PyPath::new(out), &Value::Object(result))?;
    Ok(Vec::new())
}

/// `dispatch_record(work, key)`: WORK's `dispatch.json` checked against the key; no record when it is missing.
fn dispatch_record(work: &PyPath, key: &Value) -> R<(Option<Value>, Vec<String>)> {
    let path = work.join(b"dispatch.json");
    if !pyvalue::is_file(&path) {
        return Ok((
            None,
            vec![format!(
                "no {}/dispatch.json: dispatch the grader, or name a local or manual assessor with --assessor",
                work.display()
            )],
        ));
    }
    let record = current::read_json(&path)?;
    let mut problems = Vec::new();
    let empty = Value::Object(Object::default());
    let enforcement = pyvalue::get_or(&record, "enforcement", &empty)?;
    let mut native = is(pyvalue::get(enforcement, "native_tools")?, "none");
    if is(pyvalue::get(&record, "budget_policy")?, "codex-unbounded") {
        let model = pyvalue::item(&record, "model")?;
        let Value::Str(model) = model else {
            return crash(format!(
                "AttributeError: '{}' object has no attribute 'startswith'",
                model.type_name()
            ));
        };
        if model.starts_with("gpt-") {
            let helpers = Value::Array(AUX_TOOL_NAMES.iter().map(|name| text(name)).collect());
            native = is(
                pyvalue::get(enforcement, "native_tools")?,
                "mcp-metadata-only",
            ) && pyobject::eq(pyvalue::get(enforcement, "native_helpers")?, &helpers)
                && {
                    let probe = pyvalue::get_or(enforcement, "client_probe", &empty)?;
                    pyobject::eq(pyvalue::get(probe, "native_helpers")?, &helpers)
                        && matches!(
                            pyvalue::get(
                                pyvalue::get_or(enforcement, "client_probe", &empty)?,
                                "native_resource_helpers_confined"
                            )?,
                            Value::Bool(true)
                        )
                        && matches!(
                            pyvalue::get(
                                pyvalue::get_or(enforcement, "client_probe", &empty)?,
                                "all_tools_completed"
                            )?,
                            Value::Bool(true)
                        )
                };
        }
    }
    if !native
        || !pyobject::eq(pyvalue::get(enforcement, "probe_exit")?, &int(0))
        || !pyobject::eq(
            pyvalue::get(enforcement, "command_policy_sha256")?,
            pyvalue::item(key, "command_policy_sha256")?,
        )
    {
        problems.push("dispatch lacks the pinned command-enforcement receipt".to_string());
    }
    let logs = pyvalue::get_or(enforcement, "logs", &empty)?;
    let Value::Object(logs) = logs else {
        return crash(format!(
            "AttributeError: '{}' object has no attribute 'items'",
            logs.type_name()
        ));
    };
    for (name, pinned) in logs.iter() {
        if !matches!(name.as_str(), "command-audit.jsonl" | "policy-audit.jsonl")
            || !pyobject::eq(
                &Value::Str(sha256::hex_digest(&read_file(&work.join(name.as_bytes()))?)),
                pinned,
            )
        {
            problems.push("dispatch command audit changed".to_string());
        }
    }
    let exit_code = pyvalue::item(&record, "exit_code")?;
    if !pyobject::eq(exit_code, &int(0)) {
        problems.push(format!(
            "dispatch session exit {}: a failed dispatch is graded again from a new prepare",
            exit_code.py_str()
        ));
    }
    if !pyvalue::item(&record, "verdicts_present")?.truthy() {
        problems.push(
            "dispatch wrote no verdicts.json: a failed dispatch is graded again from a new prepare"
                .to_string(),
        );
    }
    if matches!(
        pyvalue::item(pyvalue::item(&record, "usage")?, "priced_total_usd")?,
        Value::Null
    ) {
        problems.push(
            "dispatch usage was not priced: a failed dispatch is graded again from a new prepare"
                .to_string(),
        );
    }
    for violation in pyvalue::iterate(pyvalue::item(&record, "audit_violations")?)? {
        problems.push(format!("dispatch read audit: {}", violation.py_str()));
    }
    let (ran, pinned) = (
        pyvalue::item(&record, "prompt_sha256")?,
        pyvalue::item(key, "prompt_sha256")?,
    );
    if !pyobject::eq(ran, pinned) {
        let prefix = |value: &Value| -> R<String> { Ok(string(value)?.chars().take(12).collect()) };
        problems.push(format!(
            "dispatch ran prompt {}, the key's is {}",
            prefix(ran)?,
            prefix(pinned)?
        ));
    }
    let model = pyvalue::item(&record, "model")?;
    let observed = pyvalue::item(&record, "models_observed")?;
    let subagents = pyvalue::item(&record, "subagents")?;
    if !pyobject::eq(observed, &Value::Array(vec![model.clone()])) || subagents.truthy() {
        problems.push(format!(
            "dispatch observed models {} and {} subagent(s); expected {} alone",
            observed.py_str(),
            subagents.py_str(),
            model.py_str()
        ));
    }
    Ok((Some(record), problems))
}

/// `provenance_of(work, key, assessor)`: who assessed the batch. A paid session is proven by its dispatch receipt;
/// a local or manual assessor states who it is and how it worked, and claims no session, model or charge.
pub fn provenance_of(
    work: &PyPath,
    key: &Value,
    assessor: Option<&[u8]>,
) -> R<(Option<Value>, Vec<String>)> {
    let Some(assessor) = assessor else {
        let (record, problems) = dispatch_record(work, key)?;
        let Some(record) = record else {
            return Ok((None, problems));
        };
        let mut provenance = vec![
            ("kind", text("dispatch")),
            ("identity", pyvalue::item(&record, "session_id")?.clone()),
        ];
        let fields = [
            "session_id",
            "cli_version",
            "model",
            "effort",
            "prompt_sha256",
            "dispatched_at",
            "completed_at",
            "usage",
            "budget_policy",
        ];
        for field in fields {
            if let Some(value) = record.as_object().and_then(|record| record.get(field)) {
                provenance.push((field, value.clone()));
            }
        }
        provenance.push((
            "dispatch_sha256",
            Value::Str(sha256::hex_digest(&read_file(
                &work.join(b"dispatch.json"),
            )?)),
        ));
        return Ok((Some(object(provenance)), problems));
    };
    if pyvalue::exists(&work.join(b"dispatch.json")) || pyvalue::exists(&work.join(b"home")) {
        return Ok((
            None,
            vec![
                "WORK holds a dispatched session; a manual assessor cannot stand in for its receipt"
                    .to_string(),
            ],
        ));
    }
    let record = current::read_json(&PyPath::new(assessor))?;
    let stated = record.as_object().is_some_and(|record| {
        let mut names: Vec<&str> = record.iter().map(|(name, _)| name.as_str()).collect();
        names.sort_unstable();
        names == ["assessor", "completed_at", "method"]
            && record.iter().all(|(_, value)| claim_grading::filled(value))
    });
    if !stated {
        return Ok((
            None,
            vec![
                "the assessor record needs exactly assessor, method and completed_at; it claims no session, \
                 model or charge"
                    .to_string(),
            ],
        ));
    }
    let mut provenance = Object::default();
    provenance.insert("kind".into(), text("manual"));
    provenance.insert(
        "identity".into(),
        pyvalue::item(&record, "assessor")?.clone(),
    );
    for (name, value) in record.as_object().into_iter().flat_map(Object::iter) {
        provenance.insert(name.clone(), value.clone());
    }
    Ok((Some(Value::Object(provenance)), Vec::new()))
}

/// The fields of one safety check file, or `None` when Python's parse raises `ValueError`, `KeyError` or
/// `TypeError`.
fn check_rows(raw: &[u8]) -> R<Option<CheckFile>> {
    let Ok(decoded) = pytext::decode_utf8(raw) else {
        return Ok(None);
    };
    let document = match pyvalue::loads(&decoded) {
        Ok(document) => document,
        Err(Raised::Value(_)) => return Ok(None),
        Err(other) => return Err(other.into()),
    };
    let caught = |raised: Raised| match raised {
        Raised::Key(_) | Raised::Type(_) | Raised::Value(_) => Ok(None),
        other => Err(Failure::from(other)),
    };
    let parsed = (|| -> pyvalue::Py<CheckFile> {
        let checker = pyvalue::item(&document, "checker")?.clone();
        let independent_of = pyvalue::item(&document, "independent_of")?.clone();
        let mut rows = Vec::new();
        for row in pyvalue::iterate(pyvalue::item(&document, "checks")?)? {
            rows.push(
                ["review", "recommendation", "result", "reason"]
                    .map(|field| pyvalue::item(&row, field).cloned())
                    .into_iter()
                    .collect::<pyvalue::Py<Vec<Value>>>()?
                    .try_into()
                    .expect("four fields"),
            );
        }
        Ok((checker, independent_of, rows))
    })();
    match parsed {
        Ok(parsed) => Ok(Some(parsed)),
        Err(raised) => caught(raised),
    }
}

/// `safety_checks(paths, verdicts, provenance)`: the independent confirmations of proposed safety, the raw files
/// and the problems.
fn safety_checks(
    paths: &[Vec<u8>],
    verdicts: &Value,
    provenance: &Value,
) -> R<(Checks, Vec<Vec<u8>>, Vec<String>)> {
    let mut checks: Checks = Vec::new();
    let mut files: Vec<Vec<u8>> = Vec::new();
    let mut problems = Vec::new();
    let mut proposed: Vec<(Value, Value)> = Vec::new();
    let reviews = pyvalue::item(verdicts, "reviews")?;
    for (token, review) in reviews.as_object().into_iter().flat_map(Object::iter) {
        for recommendation in pyvalue::iterate(pyvalue::item(review, "recommendations")?)? {
            let id = pyvalue::item(&recommendation, "id")?;
            pyvalue::hashable(id)?;
            let state = pyvalue::item(pyvalue::item(&recommendation, "safety")?, "state")?;
            assign(
                &mut proposed,
                Value::Array(vec![text(token), id.clone()]),
                state.clone(),
            );
        }
    }
    for path in paths {
        let shown = pytext::stderr_text(path);
        let raw = read_file(&PyPath::new(path))?;
        let Some((checker, independent_of, rows)) = check_rows(&raw)? else {
            problems.push(format!(
                "{shown}: needs checker, independent_of and checks of review, recommendation, result and reason"
            ));
            continue;
        };
        if !(claim_grading::filled(&checker) && claim_grading::filled(&independent_of))
            || pyobject::eq(&checker, &independent_of)
        {
            problems.push(format!(
                "{shown}: the checker must differ from the assessor it is independent of"
            ));
        }
        if !pyobject::eq(&independent_of, pyvalue::item(provenance, "identity")?) {
            problems.push(format!(
                "{shown}: declared independent of {}, not of this assessment's assessor",
                independent_of.py_repr()
            ));
        }
        for [token, recommendation, result, reason] in rows {
            let where_ = format!("{shown}: {} {}", token.py_str(), recommendation.py_str());
            pyvalue::hashable(&token)?;
            pyvalue::hashable(&recommendation)?;
            let pair = Value::Array(vec![token.clone(), recommendation.clone()]);
            let state = lookup(&proposed, &pair)
                .cloned()
                .unwrap_or(text("unassessed"));
            if is(&state, "unassessed") {
                problems.push(format!(
                    "{where_}: no such recommendation with a proposed safety state"
                ));
            } else if !(is(&result, "confirmed")
                || is(&result, "refuted")
                || is(&result, "unresolved"))
                || !claim_grading::filled(&reason)
            {
                problems.push(format!(
                    "{where_}: needs result confirmed, refuted or unresolved and a reason"
                ));
            } else {
                let row = object(vec![
                    ("file", int(files.len())),
                    ("checker", checker.clone()),
                    ("independent_of", independent_of.clone()),
                    ("result", result),
                    ("reason", reason),
                ]);
                match checks.iter_mut().find(|((left, right), _)| {
                    pyobject::same(left, &token) && pyobject::same(right, &recommendation)
                }) {
                    Some((_, rows)) => rows.push(row),
                    None => checks.push(((token, recommendation), vec![row])),
                }
            }
        }
        files.push(raw);
    }
    Ok((checks, files, problems))
}

/// `recorded_safety(proposed, checks)`: a safety conclusion needs independent confirmation; a proposal alone or a
/// disagreement stays unassessed.
fn recorded_safety(proposed: &Value, checks: Vec<Value>) -> R<Value> {
    let mut results = Vec::new();
    for check in &checks {
        results.push(pyvalue::item(check, "result")?.clone());
    }
    let holds = |name: &str| results.iter().any(|result| is(result, name));
    let state = pyvalue::item(proposed, "state")?;
    if is(state, "unassessed") || (holds("confirmed") && !holds("refuted")) {
        return Ok(object(vec![
            ("state", state.clone()),
            ("reason", pyvalue::item(proposed, "reason")?.clone()),
            ("independent_checks", Value::Array(checks)),
        ]));
    }
    let limit = if holds("refuted") {
        "An independent check disagrees."
    } else {
        "No independent check confirms it."
    };
    Ok(object(vec![
        ("state", text("unassessed")),
        (
            "reason",
            Value::Str(format!(
                "The assessor proposed {}: {} {limit}",
                state.py_str(),
                pyvalue::item(proposed, "reason")?.py_str()
            )),
        ),
        ("independent_checks", Value::Array(checks)),
    ]))
}

/// `anchor(number, quote)` of `graded` and `graded_v2`.
fn anchor(facts: &Value, number: i128, quote: &Value) -> R<Value> {
    Ok(object(vec![
        ("review", pyvalue::item(facts, "review")?.clone()),
        ("item_id", Value::Str(format!("item-{}", number - 1))),
        ("quote", quote.clone()),
    ]))
}

/// The items of one review's verdict, numbered from 1 to the key's count.
fn numbered_items(entry: &Value, verdict: &Value) -> R<Vec<(i128, Value)>> {
    let count = integer(pyvalue::item(entry, "items")?)?;
    let items = pyvalue::item(verdict, "items")?;
    let mut numbered = Vec::new();
    for number in 1..=count {
        numbered.push((number, pyvalue::item(items, &number.to_string())?.clone()));
    }
    Ok(numbered)
}

/// The recommendations of one review's grade, each with its independent checks.
fn recommendations(
    entry: &Value,
    verdict: &Value,
    facts: &Value,
    evidence: &[Value],
    checks: &Checks,
    check_pins: &[Value],
    v2: bool,
) -> R<Vec<Value>> {
    let token = pyvalue::item(entry, "token")?;
    let mut graded = Vec::new();
    for recommendation in pyvalue::iterate(pyvalue::item(verdict, "recommendations")?)? {
        let id = pyvalue::item(&recommendation, "id")?;
        let mut independent = Vec::new();
        let found = checks
            .iter()
            .find(|((left, right), _)| pyobject::same(left, token) && pyobject::same(right, id));
        for check in found.map(|(_, rows)| rows.as_slice()).unwrap_or_default() {
            let file = integer(pyvalue::item(check, "file")?)?;
            let source = usize::try_from(file)
                .ok()
                .and_then(|file| check_pins.get(file))
                .ok_or_else(|| Failure::Crash("IndexError: list index out of range".into()))?;
            let mut row = vec![("source", source.clone())];
            for field in ["checker", "independent_of", "result", "reason"] {
                row.push((field, pyvalue::item(check, field)?.clone()));
            }
            independent.push(object(row));
        }
        let mut anchors = Vec::new();
        for anchored in pyvalue::iterate(pyvalue::item(&recommendation, "anchors")?)? {
            anchors.push(anchor(
                facts,
                integer(pyvalue::item(&anchored, "item")?)?,
                pyvalue::item(&anchored, "quote")?,
            )?);
        }
        let mut entries = vec![
            ("id", id.clone()),
            ("anchors", Value::Array(anchors)),
            (
                "addressed_claims",
                pyvalue::item(&recommendation, "addressed_claims")?.clone(),
            ),
        ];
        let safety = if v2 {
            let unassessed = object(vec![
                ("state", text("unassessed")),
                ("reason", text("The second grader does not assess safety.")),
                ("evidence", Value::Array(Vec::new())),
            ]);
            let proposed = pyvalue::get_or(&recommendation, "safety", &unassessed)?.clone();
            recorded_safety(&proposed, independent)?
        } else {
            entries.push((
                "duplicate_group",
                pyvalue::item(&recommendation, "duplicate_group")?.clone(),
            ));
            recorded_safety(pyvalue::item(&recommendation, "safety")?, independent)?
        };
        entries.push(("safety", safety));
        let mut sufficiency = Vec::new();
        for assessed in pyvalue::iterate(pyvalue::item(&recommendation, "sufficiency")?)? {
            sufficiency.push(object(vec![
                ("family_id", pyvalue::item(&assessed, "family")?.clone()),
                ("outcome", pyvalue::item(&assessed, "outcome")?.clone()),
                ("reason", pyvalue::item(&assessed, "reason")?.clone()),
                ("evidence", Value::Array(evidence.to_vec())),
            ]));
        }
        entries.push(("sufficiency", Value::Array(sufficiency)));
        graded.push(object(entries));
    }
    Ok(graded)
}

/// `{digest(a): a for r in recommendations for a in r["anchors"]}.values()`.
fn distinct_anchors(recommendations: &[Value]) -> R<Vec<Value>> {
    let mut anchors: Vec<(String, Value)> = Vec::new();
    for recommendation in recommendations {
        for anchored in pyvalue::iterate(pyvalue::item(recommendation, "anchors")?)? {
            let digest = current::digest(&anchored);
            match anchors.iter_mut().find(|(known, _)| *known == digest) {
                Some(entry) => entry.1 = anchored,
                None => anchors.push((digest, anchored)),
            }
        }
    }
    Ok(anchors.into_iter().map(|(_, anchored)| anchored).collect())
}

/// Each family's recovery from the claims and its fix sufficiency from the distinct recommendations.
fn recoveries(
    families: &Value,
    claims: &[Value],
    recommendations: &[Value],
    facts: &Value,
    complete: bool,
    v2: bool,
) -> R<Vec<Value>> {
    let mut recovered = Vec::new();
    for family in pyvalue::iterate(families)? {
        let admitted = is(
            pyvalue::item(pyvalue::item(facts, "admission")?, "state")?,
            "admitted",
        );
        let recovery = if v2 {
            claim_grading::family_recovery_v2(&family, claims, admitted)?
        } else {
            claim_grading::family_recovery(&family, claims, admitted)?
        };
        let id = pyvalue::item(&family, "id")?;
        let mut remedies = Vec::new();
        for recommendation in recommendations {
            for assessed in pyvalue::iterate(pyvalue::item(recommendation, "sufficiency")?)? {
                if pyobject::eq(pyvalue::item(&assessed, "family_id")?, id) {
                    remedies.push(pyvalue::item(&assessed, "outcome")?.clone());
                }
            }
        }
        let mut entries = vec![
            ("family_id", id.clone()),
            ("outcome", text(recovery.outcome)),
            ("claim_ids", Value::Array(recovery.claim_ids)),
            (
                "sufficiency",
                text(claim_grading::family_sufficiency(
                    recovery.outcome,
                    &remedies,
                    complete,
                )),
            ),
            ("reason", text(recovery.reason)),
        ];
        if v2 {
            entries.push(("cause_only", Value::Bool(recovery.cause_only)));
        }
        recovered.push(object(entries));
    }
    Ok(recovered)
}

fn remedy_inventory(verdict: &Value) -> R<(&Value, bool)> {
    let inventory = pyvalue::item(verdict, "remedy_inventory")?;
    let complete = is(pyvalue::item(inventory, "state")?, "complete");
    Ok((inventory, complete))
}

fn state_reason(inventory: &Value, complete: bool, assessed: &str) -> R<(Value, Value)> {
    if complete {
        return Ok((text("assessed"), text(assessed)));
    }
    let reason = pyvalue::item(inventory, "reason")?;
    let Value::Str(reason) = reason else {
        return crash(format!(
            "TypeError: can only concatenate str (not \"{}\") to str",
            reason.type_name()
        ));
    };
    Ok((
        text("unassessed"),
        Value::Str(format!("The remedy inventory is incomplete: {reason}")),
    ))
}

/// `graded(entry, verdict, facts, families, evidence, checks, check_pins)`: one review's current grade under
/// verdict contract v1.
fn graded(
    entry: &Value,
    verdict: &Value,
    facts: &Value,
    families: &Value,
    evidence: &[Value],
    checks: &Checks,
    check_pins: &[Value],
) -> R<Value> {
    let mut assessed = Vec::new();
    for (number, item) in numbered_items(entry, verdict)? {
        for claim in pyvalue::iterate(pyvalue::item(&item, "claims")?)? {
            let field = |name: &str| -> R<Value> { Ok(pyvalue::item(&claim, name)?.clone()) };
            assessed.push(object(vec![
                ("id", field("id")?),
                (
                    "anchor",
                    anchor(facts, number, pyvalue::item(&claim, "quote")?)?,
                ),
                ("canonical_id", field("canonical_claim_id")?),
                ("outcome", field("outcome")?),
                ("assessment", field("assessment")?),
                ("family_id", field("family")?),
                ("duplicate_group", field("duplicate_group")?),
                ("reason", field("notes")?),
                ("evidence", Value::Array(evidence.to_vec())),
            ]));
        }
    }
    let recommendations =
        recommendations(entry, verdict, facts, evidence, checks, check_pins, false)?;
    let (inventory, complete) = remedy_inventory(verdict)?;
    let anchors = distinct_anchors(&recommendations)?;
    let families = recoveries(
        families,
        &assessed,
        &recommendations,
        facts,
        complete,
        false,
    )?;
    let (state, reason) = state_reason(
        inventory,
        complete,
        "Every original item, causal family and corrective request was assessed.",
    )?;
    Ok(object(vec![
        ("attempt_id", pyvalue::item(entry, "attempt_id")?.clone()),
        ("state", state),
        ("reason", reason),
        ("claims", Value::Array(assessed)),
        ("families", Value::Array(families)),
        ("recommendations", Value::Array(recommendations)),
        (
            "remedy_inventory",
            object(vec![
                ("state", pyvalue::item(inventory, "state")?.clone()),
                ("reason", pyvalue::item(inventory, "reason")?.clone()),
                ("anchors", Value::Array(anchors)),
            ]),
        ),
        ("advice", Value::Array(Vec::new())),
    ]))
}

fn candidate_id(candidates: &Candidates, claim: &Value, field: &str) -> R<Value> {
    let named = pyvalue::item(claim, field)?;
    if !named.truthy() {
        return Ok(Value::Null);
    }
    pyvalue::hashable(named)?;
    let raised =
        lookup(candidates, named).ok_or_else(|| Failure::from(pyvalue::key_error(named)))?;
    Ok(pyvalue::item(raised, "id")?.clone())
}

/// `graded_v2(entry, verdict, facts, families, evidence, checks, check_pins, candidates, pre_existing)`: one
/// review's current grade under verdict contract v2, or v3 when `pre_existing` lists the target's pre-existing
/// problems.
#[allow(
    clippy::too_many_arguments,
    reason = "each argument is a separate input the caller already holds; a one-off parameter struct would not simplify the call"
)]
pub fn graded_v2(
    entry: &Value,
    verdict: &Value,
    facts: &Value,
    families: &Value,
    evidence: &[Value],
    checks: &Checks,
    check_pins: &[Value],
    candidates: &Candidates,
    pre_existing: Option<&Value>,
) -> R<Value> {
    let items = numbered_items(entry, verdict)?;
    let mut claims = Vec::new();
    for (_, item) in &items {
        claims.extend(pyvalue::iterate(pyvalue::item(item, "claims")?)?);
    }
    let mut assessed = Vec::new();
    for (number, item) in &items {
        for claim in pyvalue::iterate(pyvalue::item(item, "claims")?)? {
            let field = |name: &str| -> R<Value> { Ok(pyvalue::item(&claim, name)?.clone()) };
            let mut answers = Object::default();
            for name in [
                "true",
                "this_change",
                "promised",
                "promise_source",
                "delivered",
            ] {
                answers.insert(name.into(), field(name)?);
            }
            let mut known = Vec::new();
            for found in pyvalue::iterate(pyvalue::item(&claim, "known_problems")?)? {
                known.push(object(vec![
                    ("family_id", pyvalue::item(&found, "family")?.clone()),
                    ("says_what", pyvalue::item(&found, "says_what")?.clone()),
                    (
                        "identifies_cause",
                        pyvalue::item(&found, "identifies_cause")?.clone(),
                    ),
                    ("reason", pyvalue::item(&found, "reason")?.clone()),
                ]));
            }
            assessed.push(object(vec![
                ("id", field("id")?),
                (
                    "anchor",
                    anchor(facts, *number, pyvalue::item(&claim, "quote")?)?,
                ),
                ("canonical_id", field("canonical_claim_id")?),
                ("outcome", field("outcome")?),
                ("kind", field("kind")?),
                ("answers", Value::Object(answers)),
                ("known_problems", Value::Array(known)),
                ("open", field("open")?),
                (
                    "candidate_id",
                    candidate_id(candidates, &claim, "candidate")?,
                ),
                ("duplicate_group", field("duplicate_group")?),
                ("reason", field("notes")?),
                ("evidence", Value::Array(evidence.to_vec())),
            ]));
        }
    }
    if pre_existing.is_some() {
        for (saved, original) in assessed.iter_mut().zip(&claims) {
            let mut problems = Vec::new();
            for found in pyvalue::iterate(pyvalue::item(original, "pre_existing_problems")?)? {
                let mut entries = vec![("problem_id", pyvalue::item(&found, "problem")?.clone())];
                for name in ["says_what", "identifies_cause", "reason"] {
                    entries.push((name, pyvalue::item(&found, name)?.clone()));
                }
                problems.push(object(entries));
            }
            let candidate = candidate_id(candidates, original, "pre_existing_candidate")?;
            let Value::Object(saved) = saved else {
                unreachable!("each saved claim is an object")
            };
            saved.insert("pre_existing_problems".into(), Value::Array(problems));
            saved.insert("pre_existing_candidate_id".into(), candidate);
        }
    }
    let recommendations =
        recommendations(entry, verdict, facts, evidence, checks, check_pins, true)?;
    let (inventory, complete) = remedy_inventory(verdict)?;
    let anchors = distinct_anchors(&recommendations)?;
    let families = recoveries(families, &claims, &recommendations, facts, complete, true)?;
    let (state, reason) = state_reason(
        inventory,
        complete,
        "Every original item, known problem and corrective request was assessed.",
    )?;
    let mut not_findings = Vec::new();
    for (number, item) in &items {
        if is(pyvalue::item(item, "kind")?, "not-a-finding") {
            not_findings.push(object(vec![
                ("item_id", Value::Str(format!("item-{}", number - 1))),
                ("note", pyvalue::item(item, "note")?.clone()),
            ]));
        }
    }
    let listed = pyvalue::item(inventory, "reason")?;
    let mut grade = vec![
        ("attempt_id", pyvalue::item(entry, "attempt_id")?.clone()),
        ("state", state),
        ("reason", reason),
        ("claims", Value::Array(assessed)),
        ("not_findings", Value::Array(not_findings)),
        ("families", Value::Array(families)),
        ("recommendations", Value::Array(recommendations)),
        (
            "remedy_inventory",
            object(vec![
                ("state", pyvalue::item(inventory, "state")?.clone()),
                (
                    "reason",
                    if listed.truthy() {
                        listed.clone()
                    } else {
                        text("Every fix the review asks for is listed.")
                    },
                ),
                ("anchors", Value::Array(anchors)),
            ]),
        ),
        ("advice", Value::Array(Vec::new())),
    ];
    if let Some(pre_existing) = pre_existing {
        let admitted = |facts: &Value| -> R<bool> {
            Ok(is(
                pyvalue::item(pyvalue::item(facts, "admission")?, "state")?,
                "admitted",
            ))
        };
        let mut assessments = Vec::new();
        for problem in pyvalue::iterate(pre_existing)? {
            assessments.push(pre_existing::assessment(
                &problem,
                &claims,
                admitted(facts)?,
            )?);
        }
        grade.push(("pre_existing_problems", Value::Array(assessments)));
    }
    Ok(object(grade))
}

fn attempt_facts<'a>(attempts: &'a [(String, Value)], attempt: &Value) -> R<&'a Value> {
    attempts
        .iter()
        .find(|(name, _)| attempt.as_str() == Some(name.as_str()))
        .map(|(_, facts)| facts)
        .ok_or_else(|| pyvalue::key_error(attempt).into())
}

/// The three parts of a candidate anchor's identity, compared as Python compares the tuples.
fn compare_identity(left: &[Value; 3], right: &[Value; 3]) -> R<Ordering> {
    for (left, right) in left.iter().zip(right) {
        if !pyobject::eq(left, right) {
            return Ok(pyvalue::compare(left, right)?);
        }
    }
    Ok(Ordering::Equal)
}

/// `raised_candidates(verdicts, reviews, attempts, task)`: each new candidate of the verdicts, by its id there, with
/// its register id and the original wording its claims quote.
pub fn raised_candidates(
    verdicts: &Value,
    reviews: &Value,
    attempts: &[(String, Value)],
    task: &Value,
) -> R<Candidates> {
    let mut raised: Candidates = Vec::new();
    let mut identifiers: Vec<(String, Value)> = Vec::new();
    for candidate in pyvalue::iterate(pyvalue::item(verdicts, "new_candidates")?)? {
        let named = pyvalue::item(&candidate, "id")?;
        let mut anchors = Vec::new();
        for review in sorted_by(reviews, "attempt_id")? {
            let items = pyvalue::item(
                pyvalue::subscript(
                    pyvalue::item(verdicts, "reviews")?,
                    pyvalue::item(&review, "token")?,
                )?,
                "items",
            )?;
            let Value::Object(items) = items else {
                return crash(format!(
                    "AttributeError: '{}' object has no attribute 'items'",
                    items.type_name()
                ));
            };
            let mut numbered = Vec::new();
            for (number, item) in items.iter() {
                numbered.push((python_int(number)?, item));
            }
            numbered.sort_by_key(|(number, _)| *number);
            for (number, item) in numbered {
                for claim in pyvalue::iterate(pyvalue::item(item, "claims")?)? {
                    let pre_existing = pyvalue::get(&claim, "pre_existing_candidate")?;
                    let named_by = [pyvalue::item(&claim, "candidate")?, pre_existing];
                    if named_by.iter().any(|value| pyobject::same(value, named)) {
                        anchors.push(object(vec![
                            (
                                "review",
                                pyvalue::item(
                                    attempt_facts(attempts, pyvalue::item(&review, "attempt_id")?)?,
                                    "review",
                                )?
                                .clone(),
                            ),
                            ("item_id", Value::Str(format!("item-{}", number - 1))),
                            ("quote", pyvalue::item(&claim, "quote")?.clone()),
                        ]));
                    }
                }
            }
        }
        let mut identity: Vec<[Value; 3]> = Vec::new();
        for anchored in &anchors {
            let parts = [
                pyvalue::item(pyvalue::item(anchored, "review")?, "path")?.clone(),
                pyvalue::item(anchored, "item_id")?.clone(),
                pyvalue::item(anchored, "quote")?.clone(),
            ];
            for part in &parts {
                pyvalue::hashable(part)?;
            }
            if !identity
                .iter()
                .any(|known| known.iter().zip(&parts).all(|(a, b)| pyobject::same(a, b)))
            {
                identity.push(parts);
            }
        }
        if let Some(first) = identity.first() {
            for other in &identity[1..] {
                compare_identity(first, other)?;
            }
        }
        identity.sort_by(|left, right| compare_identity(left, right).unwrap_or(Ordering::Equal));
        let wording = Value::Array(
            identity
                .into_iter()
                .map(|parts| Value::Array(parts.to_vec()))
                .collect(),
        );
        let digest = current::digest(&object(vec![
            ("target", pyvalue::item(task, "id")?.clone()),
            ("wording", wording),
        ]));
        let identifier = format!("NC-{}", &digest[..12]);
        match identifiers.iter().find(|(known, _)| *known == identifier) {
            Some((_, first)) if !pyobject::eq(first, named) => {
                return inconsistent(format!(
                    "new candidates {} and {} quote the same original wording; one assertion is one candidate",
                    first.py_str(),
                    named.py_str()
                ));
            }
            Some(_) => {}
            None => identifiers.push((identifier.clone(), named.clone())),
        }
        pyvalue::hashable(named)?;
        assign(
            &mut raised,
            named.clone(),
            object(vec![
                ("id", Value::Str(identifier)),
                ("anchors", Value::Array(anchors)),
            ]),
        );
    }
    Ok(raised)
}

/// `candidate_records(verdicts, raised, task, run, receipt, existing)`: the candidate register after this
/// assessment. A candidate keeps its first-recorded time and decision while a reassessment raises it again.
fn candidate_records(
    verdicts: &Value,
    raised: &Candidates,
    task: &Value,
    run: &Value,
    receipt: &Value,
    existing: &Value,
) -> R<Vec<Value>> {
    let mut records: Vec<(Value, Value)> = Vec::new();
    for candidate in pyvalue::iterate(existing)? {
        let id = pyvalue::item(&candidate, "id")?.clone();
        pyvalue::hashable(&id)?;
        assign(&mut records, id, candidate);
    }
    let empty = Value::Object(Object::default());
    for candidate in pyvalue::iterate(pyvalue::item(verdicts, "new_candidates")?)? {
        let named = pyvalue::item(&candidate, "id")?;
        let entry =
            lookup(raised, named).ok_or_else(|| Failure::from(pyvalue::key_error(named)))?;
        let identifier = pyvalue::item(entry, "id")?.clone();
        let earlier = lookup(&records, &identifier)
            .cloned()
            .unwrap_or(empty.clone());
        let recorded_at = Value::Str(crate::normalize_review::utc_now());
        let mut record = vec![
            ("id", identifier.clone()),
            ("target", pyvalue::item(task, "id")?.clone()),
            ("revision", pyvalue::item(task, "revision")?.clone()),
            (
                "recorded_at",
                pyvalue::get_or(&earlier, "recorded_at", &recorded_at)?.clone(),
            ),
        ];
        for field in [
            "claim",
            "evidence",
            "limits",
            "relevance",
            "confidence",
            "would_settle",
            "kind",
        ] {
            if let Some(value) = candidate
                .as_object()
                .and_then(|candidate| candidate.get(field))
            {
                record.push((field, value.clone()));
            }
        }
        record.push(("anchors", pyvalue::item(entry, "anchors")?.clone()));
        record.push((
            "source",
            object(vec![("run", run.clone()), ("receipt", receipt.clone())]),
        ));
        record.push(("decision", pyvalue::get(&earlier, "decision")?.clone()));
        assign(&mut records, identifier, object(record));
    }
    let keyed = records
        .into_iter()
        .map(|(_, record)| Ok((pyvalue::item(&record, "id")?.clone(), record)))
        .collect::<R<Vec<_>>>()?;
    sort_keyed(keyed)
}

/// `replace_json(path, value)`: the file replaced by a rename of its temporary sibling.
fn replace_json(path: &PyPath, value: &Value) -> R<()> {
    let temporary = PyPath::new(&[path.as_bytes(), b".tmp"].concat());
    write(&temporary, format!("{}\n", dumps(value, true)).as_bytes())?;
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

/// `1 + max(int(p.name.split("-")[1]) for p in directory.glob("assessment-*"))`, or 1 when there is none.
pub fn next_assessment(directory: &PyPath) -> R<PyPath> {
    let mut highest: i128 = 0;
    for name in pyfs::names(directory.as_bytes()) {
        if !name.starts_with(b"assessment-") {
            continue;
        }
        let name = utf8(&name, "assessment directory")?;
        highest = highest.max(python_int(name.split('-').nth(1).unwrap_or_default())?);
    }
    Ok(directory.join(format!("assessment-{}", highest + 1).as_bytes()))
}

/// `prune_workspace.prune_grading(work, head, receipt, apply=True)`, run through `prune_workspace.py --grading-work`
/// until #159 moves cleanup here. `failed` begins the message of a cleanup that fails.
pub fn prune_grading(
    checkout: &PyPath,
    root: &Root,
    target: &str,
    work: &PyPath,
    receipt: &PyPath,
    failed: &str,
) -> R<()> {
    let directory = root.path.join(b"bench/targets").join(target.as_bytes());
    let facts = current::read_json(&directory.join(b"target.json"))?;
    pyvalue::get(&facts, "head")?;
    let command = vec![
        b"python3".to_vec(),
        checkout
            .join(super::TOOLS)
            .join(b"prune_workspace.py")
            .as_bytes()
            .to_vec(),
        b"--grading-work".to_vec(),
        work.as_bytes().to_vec(),
        b"--target".to_vec(),
        directory.as_bytes().to_vec(),
        b"--receipt".to_vec(),
        receipt.as_bytes().to_vec(),
        b"--apply".to_vec(),
    ];
    let cleanup = |message: &str| Failure::Input(format!("{failed}{message}"));
    let done = match provision::run(&command) {
        Ok(done) => done,
        Err(Failure::Input(message)) => return Err(cleanup(&message)),
        Err(other) => return Err(other),
    };
    let reported = |prefix: &str| {
        done.stdout()
            .strip_prefix(prefix.as_bytes())
            .map(|rest| rest.strip_suffix(b"\n").unwrap_or(rest))
            .and_then(|rest| std::str::from_utf8(rest).ok())
    };
    let outcome = match done.code {
        0 => Some(Ok(())),
        1 => reported("cleanup refused: ").map(|message| Err(cleanup(message))),
        2 => reported("cleanup failed: ").map(|message| Err(cleanup(message))),
        _ => None,
    };
    match outcome {
        Some(outcome) => {
            use std::io::Write;
            let _ = std::io::stderr().write_all(done.stderr());
            outcome
        }
        None => Err(cleanup(&format!(
            "{} exited {}\n{}",
            pytext::stderr_text(&command.join(&b' ')),
            done.code,
            done.tail(2000)
        ))),
    }
}

/// `bench-tools grade map`'s options.
pub struct Mapping<'a> {
    pub root: &'a [u8],
    pub current: &'a [u8],
    pub work: &'a [u8],
    pub key: &'a [u8],
    pub assessor: Option<&'a [u8]>,
    pub safety_checks: &'a [Vec<u8>],
}

fn documents_with(documents: &Documents, replaced: &[(&str, Value)]) -> Documents {
    Documents {
        kinds: documents
            .kinds
            .iter()
            .map(|(kind, value)| {
                let value = replaced
                    .iter()
                    .find(|(name, _)| name == kind)
                    .map_or(value, |(_, value)| value);
                (*kind, value.clone())
            })
            .collect(),
    }
}

fn with_field(document: &Value, field: &str, value: Value) -> R<Value> {
    let Value::Object(document) = document else {
        return crash(format!(
            "TypeError: 'dict' object argument after ** must be a mapping, not {}",
            document.type_name()
        ));
    };
    let mut document = document.clone();
    document.insert(field.into(), value);
    Ok(Value::Object(document))
}

fn next_with<'a>(values: &'a Value, field: &str, wanted: &Value) -> R<&'a Value> {
    let Value::Array(values) = values else {
        return crash(format!(
            "TypeError: '{}' object is not iterable",
            values.type_name()
        ));
    };
    for value in values {
        if pyobject::eq(pyvalue::item(value, field)?, wanted) {
            return Ok(value);
        }
    }
    crash("StopIteration".into())
}

/// `bench-tools grade map`: file one batch's verdicts as its current grades, or refuse with nothing written.
pub fn map_verdicts(runner: &Runner, options: &Mapping) -> R<Vec<u8>> {
    let root = Root::new(options.root)?;
    let work = resolve(options.work)?;
    let key = current::read_json(&PyPath::new(options.key))?;
    let snapshot = current::read_json(&work.join(b"validator/inputs.json"))?;
    if is(
        pyvalue::get(&snapshot, "safety_policy")?,
        grading_validation::NO_SAFETY,
    ) {
        return inconsistent("a second-grader assessment cannot be mapped as a first grade");
    }
    let contract = pyvalue::get(&snapshot, "contract")?.clone();
    let v3 = is(&contract, pre_existing::CONTRACT);
    let v2 = is(&contract, grading_validation::CONTRACT_V2) || v3;
    let run = pyvalue::item(&key, "run")?;
    let target = pyvalue::item(&key, "target")?;
    let fingerprint = pyvalue::item(&key, "input_fingerprint")?;
    let (selected, documents) = unchanged_inputs(&root, options.current, &key)?;
    let (provenance, mut problems) = provenance_of(&work, &key, options.assessor)?;
    let (run_text, target_text) = (string(run)?, string(target)?);
    let checked = check_attempts(
        &root,
        &selected,
        run_text,
        target_text,
        &[("the key".to_string(), key_counts(&key)?)],
    )?;
    problems.extend(checked.problems);
    problems.extend(check_prepared(
        &runner.checkout,
        &work,
        &key,
        false,
        &runner.executable,
    )?);
    if !problems.is_empty() {
        return inconsistent(problems.join("\n"));
    }
    let provenance = provenance.expect("a missing provenance comes with a problem");
    let (raw, verdicts, problems) = read_verdicts(&work)?;
    if !problems.is_empty() {
        return inconsistent(problems.join("\n"));
    }
    let (checks, check_files, mut problems) =
        safety_checks(options.safety_checks, &verdicts, &provenance)?;
    for dispute in pyvalue::iterate(pyvalue::item(&verdicts, "link_disputes")?)? {
        let field = |name: &str| -> R<String> { Ok(pyvalue::item(&dispute, name)?.py_str()) };
        problems.push(format!(
            "{} item {}: the equivalence link to {} is disputed ({}); correct the link in the current claims and \
             prepare the batch again",
            field("review")?,
            field("item")?,
            field("canonical_claim_id")?,
            field("reason")?
        ));
    }
    if !problems.is_empty() {
        return inconsistent(problems.join("\n"));
    }

    let current = root.local(options.current)?;
    let directory = current
        .join(b"assessments")
        .join(&name_of(run_text.as_bytes()))
        .join(target_text.as_bytes());
    let out = next_assessment(&directory)?;
    make_directory(&out, true)?;
    let filed = (|| -> R<(Documents, Vec<Value>)> {
        write(&out.join(b"verdicts.json"), &raw)?;
        let mut check_pins = Vec::new();
        for (index, content) in check_files.iter().enumerate() {
            let path = out.join(format!("safety-checks-{}.json", index + 1).as_bytes());
            write(&path, content)?;
            check_pins.push(root.pin_file(&path)?);
        }
        let mapped_at = Value::Str(crate::normalize_review::utc_now());
        let mut receipt = vec![
            ("contract", text(RECEIPT_CONTRACT)),
            ("run", run.clone()),
            ("target", target.clone()),
            ("input_fingerprint", fingerprint.clone()),
            ("mapped_at", mapped_at),
            ("provenance", provenance.clone()),
            ("verdicts_sha256", Value::Str(sha256::hex_digest(&raw))),
            (
                "prepared_files",
                pyvalue::item(&key, "prepared_files")?.clone(),
            ),
            ("validator", pyvalue::item(&key, "validator")?.clone()),
            (
                "claim_snapshot",
                pyvalue::item(&key, "claim_snapshot")?.clone(),
            ),
            ("safety_checks", Value::Array(check_pins.clone())),
        ];
        let prepared = pyvalue::item(&key, "runner_deviation")?.clone();
        let execution_policy = read_file(&runner.checkout.join(super::EXECUTION_POLICY))?;
        let mapping = runner_deviation(
            &runner.checkout,
            super::runtime::pin(&runner.executable)?,
            &execution_policy,
        )?;
        receipt.push((
            "runner",
            object(vec![("prepared", prepared), ("mapping", object(mapping))]),
        ));
        receipt.push(("reviews", key_reviews(&key)?));
        if let Some(policy) = key.as_object().and_then(|key| key.get("remedy_policy")) {
            receipt.push(("remedy_policy", policy.clone()));
        }
        let receipt_path = out.join(b"receipt.json");
        write(
            &receipt_path,
            format!("{}\n", dumps(&object(receipt), true)).as_bytes(),
        )?;
        let assessor = object(vec![
            ("kind", pyvalue::item(&provenance, "kind")?.clone()),
            ("receipt", root.pin_file(&receipt_path)?),
            ("verdicts", root.pin_file(&out.join(b"verdicts.json"))?),
        ]);
        let reference = next_with(
            pyvalue::item(documents.get("reference"), "targets")?,
            "target",
            target,
        )?;
        let task = next_with(pyvalue::item(&selected, "tasks")?, "id", target)?;
        let reviews = pyvalue::item(&key, "reviews")?;
        let raised = raised_candidates(&verdicts, reviews, &checked.attempts, task)?;
        let evidence = [
            pyvalue::item(&assessor, "verdicts")?.clone(),
            pyvalue::item(&assessor, "receipt")?.clone(),
        ];
        let none = Value::Array(Vec::new());
        let mut grades = Vec::new();
        for entry in sorted_by(reviews, "attempt_id")? {
            let verdict = pyvalue::subscript(
                pyvalue::item(&verdicts, "reviews")?,
                pyvalue::item(&entry, "token")?,
            )?;
            let facts = attempt_facts(&checked.attempts, pyvalue::item(&entry, "attempt_id")?)?;
            let families = pyvalue::item(reference, "families")?;
            grades.push(if v2 {
                let pre_existing = if v3 {
                    Some(pyvalue::get_or(reference, "pre_existing_problems", &none)?)
                } else {
                    None
                };
                graded_v2(
                    &entry,
                    verdict,
                    facts,
                    families,
                    &evidence,
                    &checks,
                    &check_pins,
                    &raised,
                    pre_existing,
                )?
            } else {
                graded(
                    &entry,
                    verdict,
                    facts,
                    families,
                    &evidence,
                    &checks,
                    &check_pins,
                )?
            });
        }
        let mut batch = vec![
            ("run", run.clone()),
            ("target", target.clone()),
            ("input_fingerprint", fingerprint.clone()),
            ("assessor", assessor.clone()),
            ("reviews", Value::Array(grades.clone())),
        ];
        if v2 {
            batch.push(("verdicts", contract.clone()));
        }
        let grade = documents.get("grade");
        let mut kept = Vec::new();
        for saved in pyvalue::iterate(pyvalue::item(grade, "batches")?)? {
            let same = pyobject::eq(pyvalue::item(&saved, "run")?, run)
                && pyobject::eq(pyvalue::item(&saved, "target")?, target);
            if !same {
                kept.push(saved);
            }
        }
        let candidates = candidate_records(
            &verdicts,
            &raised,
            task,
            run,
            pyvalue::item(&assessor, "receipt")?,
            pyvalue::item(documents.get("candidate"), "candidates")?,
        )?;
        kept.push(object(batch));
        let mut keyed = Vec::new();
        for saved in kept {
            let identity = Value::Array(vec![
                pyvalue::item(&saved, "run")?.clone(),
                pyvalue::item(&saved, "target")?.clone(),
            ]);
            keyed.push((identity, saved));
        }
        let batches = sort_pairs(keyed)?;
        let updated = documents_with(
            &documents,
            &[
                (
                    "candidate",
                    with_field(
                        documents.get("candidate"),
                        "candidates",
                        Value::Array(candidates),
                    )?,
                ),
                (
                    "grade",
                    with_field(grade, "batches", Value::Array(batches))?,
                ),
            ],
        );
        match current::validate_documents(&root, &updated, &selected, true) {
            Err(Failure::Inconsistent(message)) => {
                return Err(Failure::Inconsistent(
                    [
                        b"the current record would be inconsistent, so nothing was replaced: "
                            .as_slice(),
                        &message,
                    ]
                    .concat(),
                ));
            }
            other => other?,
        }
        prune_grading(
            &runner.checkout,
            &root,
            target_text,
            &work,
            &receipt_path,
            MAP_CLEANUP,
        )?;
        Ok((updated, grades))
    })();
    let (updated, grades) = match filed {
        Ok(filed) => filed,
        Err(failure) => {
            let _ = provision::remove_tree(out.as_bytes());
            return Err(failure);
        }
    };
    replace_json(&current.join(b"candidates.json"), updated.get("candidate"))?;
    replace_json(&current.join(b"grades.json"), updated.get("grade"))?;
    let mut claims = 0;
    let mut outcomes = Vec::new();
    for grade in &grades {
        claims += pyvalue::length(pyvalue::item(grade, "claims")?)?;
        for family in pyvalue::iterate(pyvalue::item(grade, "families")?)? {
            outcomes.push(pyvalue::item(&family, "outcome")?.clone());
        }
    }
    let count = |name: &str| outcomes.iter().filter(|outcome| is(outcome, name)).count();
    let relative = pyfs::relative_to(out.as_bytes(), root.path.as_bytes())
        .map_err(|message| Failure::Crash(format!("ValueError: {message}")))?;
    Ok([
        format!(
            "replaced {}/{} in ",
            run.py_str(),
            target.py_str()
        )
        .as_bytes(),
        current.join(b"grades.json").as_bytes(),
        format!(
            ": {} reviews, {claims} claims, {} caught, {} missed, {} unresolved; {} new candidate(s); evidence ",
            grades.len(),
            count("caught"),
            count("missed"),
            count("unresolved"),
            pyvalue::length(pyvalue::item(&verdicts, "new_candidates")?)?
        )
        .as_bytes(),
        &relative,
        b"\n",
    ]
    .concat())
}

/// `sorted(values, key=lambda v: (v["run"], v["target"]))` with each pair given as its key.
fn sort_pairs(mut keyed: Vec<(Value, Value)>) -> R<Vec<Value>> {
    let compare = |left: &Value, right: &Value| -> R<Ordering> {
        let (Value::Array(left), Value::Array(right)) = (left, right) else {
            unreachable!("pairs");
        };
        for (left, right) in left.iter().zip(right) {
            if !pyobject::eq(left, right) {
                return Ok(pyvalue::compare(left, right)?);
            }
        }
        Ok(Ordering::Equal)
    };
    if let Some((first, _)) = keyed.first() {
        for (key, _) in &keyed[1..] {
            compare(first, key)?;
        }
    }
    keyed.sort_by(|left, right| compare(&left.0, &right.0).unwrap_or(Ordering::Equal));
    Ok(keyed.into_iter().map(|(_, value)| value).collect())
}

/// `{r["attempt_id"]: r["items"] for r in key["reviews"]}`.
pub fn key_counts(key: &Value) -> R<Object> {
    let mut counts = Object::default();
    for review in pyvalue::iterate(pyvalue::item(key, "reviews")?)? {
        counts.insert(
            string(pyvalue::item(&review, "attempt_id")?)?.to_string(),
            pyvalue::item(&review, "items")?.clone(),
        );
    }
    Ok(counts)
}

/// The key's reviews as a receipt records them.
pub fn key_reviews(key: &Value) -> R<Value> {
    let mut reviews = Vec::new();
    for review in pyvalue::iterate(pyvalue::item(key, "reviews")?)? {
        let mut entries = Vec::new();
        for field in ["token", "attempt_id", "items", "review"] {
            entries.push((field, pyvalue::item(&review, field)?.clone()));
        }
        reviews.push(object(entries));
    }
    Ok(Value::Array(reviews))
}

/// `bench-tools grade invalidate`: return every stale batch's reviews to the queue.
pub fn invalidate(root: &[u8], current_arg: &[u8]) -> R<Vec<u8>> {
    let root = Root::new(root)?;
    let (selected, documents) = current::load_current(&root, current_arg, false)?;
    let grade = documents.get("grade");
    let (mut kept, mut stale) = (Vec::new(), Vec::new());
    for batch in pyvalue::iterate(pyvalue::item(grade, "batches")?)? {
        let identity = object(vec![
            ("run", pyvalue::item(&batch, "run")?.clone()),
            ("target", pyvalue::item(&batch, "target")?.clone()),
        ]);
        let current = pyvalue::contains(pyvalue::item(&selected, "batches")?, &identity)?
            && current::batch_state(&root, &identity, &selected, &documents)?.1 == "current";
        if current {
            kept.push(batch);
        } else {
            stale.push(batch);
        }
    }
    let remain = kept.len();
    if !stale.is_empty() {
        let grades = with_field(grade, "batches", Value::Array(kept))?;
        let updated = documents_with(&documents, &[("grade", grades.clone())]);
        current::validate_documents(&root, &updated, &selected, true)?;
        replace_json(&root.local(current_arg)?.join(b"grades.json"), &grades)?;
    }
    let mut printed = String::new();
    for batch in &stale {
        printed.push_str(&format!(
            "invalidated {}/{}: {} reviews return to the queue\n",
            pyvalue::item(batch, "run")?.py_str(),
            pyvalue::item(batch, "target")?.py_str(),
            pyvalue::length(pyvalue::item(batch, "reviews")?)?
        ));
    }
    printed.push_str(&format!(
        "{} stale batch(es) invalidated; {remain} remain current\n",
        stale.len()
    ));
    Ok(printed.into_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pyjson::{self, Duplicates};

    #[test]
    fn a_dispatch_that_ran_another_prompt_names_both_prefixes() {
        let work = std::env::temp_dir().join(format!("bench-tools-filing-{}", std::process::id()));
        std::fs::create_dir_all(&work).unwrap();
        std::fs::write(
            work.join("dispatch.json"),
            r#"{"enforcement": {"native_tools": "none", "probe_exit": 0, "command_policy_sha256": "policy",
                "logs": {}}, "budget_policy": "bounded", "model": "m", "exit_code": 0, "verdicts_present": true,
                "usage": {"priced_total_usd": 1.0}, "audit_violations": [], "prompt_sha256": "aaaaaaaaaaaa-ran",
                "models_observed": ["m"], "subagents": 0}"#,
        )
        .unwrap();
        let key = pyjson::loads(
            r#"{"command_policy_sha256": "policy", "prompt_sha256": "bbbbbbbbbbbb-pinned"}"#,
            Duplicates::Refuse,
        )
        .unwrap();
        let checked = dispatch_record(&PyPath::new(pytext::os_bytes(work.as_os_str())), &key);
        std::fs::remove_dir_all(&work).unwrap();
        let (record, problems) = checked.unwrap();
        assert!(record.is_some());
        assert_eq!(
            problems,
            ["dispatch ran prompt aaaaaaaaaaaa, the key's is bbbbbbbbbbbb"]
        );
    }
}
