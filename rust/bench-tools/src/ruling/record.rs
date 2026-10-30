//! `ruling_record.py`: check a ruling's before-record and after-record, and say why a ruling stays with the
//! user. Each function keeps the Python's order of reads, so the first exception it meets is the same.

use super::summary::Pair;
use super::{Repo, dossier};
use crate::delegation;
use crate::pyjson::{Object, Value};
use crate::pytext::{self, PyPath};
use crate::pyvalue::{self, Py, Raised, get, has, item, one_of};

const CONTRACT_1_TYPES: [&str; 5] = ["candidate", "recovery", "grouping", "band", "control"];
const RANK: [(&str, i64); 4] = [
    ("suggestion", 0),
    ("relied-on", 0),
    ("minor-defect", 1),
    ("problem", 2),
];
const OUTCOMES: [&str; 11] = [
    "suggestion",
    "relied-on",
    "minor-defect",
    "problem",
    "refuted",
    "unproven",
    "outside-scope",
    "duplicate",
    "recovers",
    "does-not-recover",
    "cannot-tell",
];
const BEFORE: [&str; 10] = [
    "ruling",
    "decision_type",
    "group",
    "reconstructed",
    "dossier",
    "rule",
    "clauses",
    "rulings",
    "reviews",
    "answers",
];
const ANSWER: [&str; 13] = [
    "by",
    "model",
    "family",
    "blind",
    "exposure",
    "outcome",
    "clauses",
    "conflict",
    "nearest",
    "confidence",
    "would_settle",
    "rule_gap",
    "reason",
];
const AFTER: [&str; 7] = [
    "ruling",
    "before",
    "asked",
    "outcome",
    "by_default",
    "ground",
    "rule_sentence_shown",
];

/// Each decision type with its outcomes and the blind answers it needs, in the Python's order.
pub const DECISION_TYPES: [(&str, &[&str], usize); 9] = [
    (
        "pre-existing-admission",
        &["admitted", "not-admitted", "cannot-tell"],
        2,
    ),
    (
        "pre-existing-raise",
        &["recovers", "does-not-recover", "cannot-tell"],
        2,
    ),
    (
        "pre-existing-grouping",
        &["same-family", "separate", "cannot-tell"],
        2,
    ),
    (
        "candidate",
        &[
            "suggestion",
            "relied-on",
            "minor-defect",
            "problem",
            "refuted",
            "unproven",
            "outside-scope",
            "duplicate",
            "cannot-tell",
        ],
        2,
    ),
    (
        "recovery",
        &["recovers", "does-not-recover", "cannot-tell"],
        2,
    ),
    ("grouping", &["same-family", "separate", "cannot-tell"], 2),
    (
        "band",
        &["serious", "other-material", "unknown", "not-applicable"],
        1,
    ),
    (
        "control",
        &["audited-clean", "provisional", "known-problems"],
        1,
    ),
    (
        "reconciliation",
        &["first-error", "first-correct", "undetermined"],
        0,
    ),
];
const EARLIER_TYPES: [(&str, &str); 3] = [
    ("older-admission", "pre-existing-admission"),
    ("older-credit", "pre-existing-raise"),
    ("older-grouping", "pre-existing-grouping"),
];
const EARLIER_ROUND_PATH: &str = "docs/research/older-problem-activation-2026-10-13/opening.json";
const EARLIER_ROUND_SHA256: &str =
    "f5764d9090b87173c6905ced49d33e83fd0ebb0849450a02c35d2e421a0a3a15";
pub const NAMES_A_PROBLEM: [&str; 2] = ["duplicate", "same-family"];
pub const COULD_NOT_TELL: [&str; 4] = ["unproven", "cannot-tell", "unknown", "undetermined"];
pub const FACTS: [&str; 2] = ["says_what", "identifies_cause"];
pub const LEVELS: [&str; 3] = ["high", "medium", "low"];
const SHORT_OF_HIGH: [&str; 6] = [
    "fact-reported",
    "no-single-rule",
    "conflict",
    "gap",
    "no-precedent",
    "flip-fact-open",
];
const STATED: [(&str, &str); 2] = [("rule_gap", "gap"), ("conflict", "conflict")];
const WRITTEN: [&str; 3] = ["before-question", "after-answer", "unknown"];
const BEFORE_2_EXTRA: [&str; 2] = ["target", "case"];
const ANSWER_2_EXTRA: [&str; 4] = ["effort", "brief", "written", "short_of_high"];
const UNRECORDED: [&str; 4] = ["effort", "confidence", "short_of_high", "would_settle"];
const AFTER_2: [&str; 11] = [
    "contract",
    "ruling",
    "before",
    "settled_by",
    "policy",
    "asked",
    "outcome",
    "by_default",
    "ground",
    "miss",
    "lesson",
];
const SETTLED_BY: [&str; 2] = ["owner", "agents"];
const CAUSES: [&str; 9] = [
    "fact-not-fetched",
    "fact-misread",
    "not-shown",
    "precedent-not-shown",
    "term-unclear",
    "rule-gap",
    "rule-changed-later",
    "owner-weighs-differently",
    "slip",
];
const ROUND: [&str; 4] = ["round", "opened", "closed", "no_record"];

pub fn decision_type(name: &str) -> Option<(&'static [&'static str], usize)> {
    DECISION_TYPES
        .iter()
        .find(|(known, _, _)| *known == name)
        .map(|(_, outcomes, blind)| (*outcomes, *blind))
}

/// `name in DECISION_TYPES`: a dict lookup, so an unhashable name raises.
fn is_decision_type(value: &Value) -> Py<bool> {
    pyvalue::hashable(value)?;
    Ok(value.as_str().and_then(decision_type).is_some())
}

/// `DECISION_TYPES[name]`.
fn decision_type_of(value: &Value) -> Py<(&'static [&'static str], usize)> {
    pyvalue::hashable(value)?;
    value
        .as_str()
        .and_then(decision_type)
        .ok_or_else(|| pyvalue::key_error(value))
}

fn plural_join(names: &[&str]) -> String {
    names.join(", ")
}

/// `read_before`: a before-record is never rewritten, so one of the round that opened before the decision types
/// were renamed keeps its earlier spelling on disk and is read under the current name.
pub fn read_before(repo: &Repo, path: &PyPath) -> Py<Value> {
    let record = repo.load(path)?;
    let earlier: Object = [
        ("path".to_string(), pyvalue::text(EARLIER_ROUND_PATH)),
        ("sha256".to_string(), pyvalue::text(EARLIER_ROUND_SHA256)),
    ]
    .into_iter()
    .collect();
    if !get(&record, "round_opening")?.py_eq(&Value::Object(earlier)) {
        return Ok(record);
    }
    let named = get(&record, "decision_type")?;
    pyvalue::hashable(named)?;
    let current = EARLIER_TYPES
        .iter()
        .find(|(earlier, _)| pyvalue::is(named, earlier))
        .map(|(_, current)| *current);
    Ok(match (current, record) {
        (Some(current), Value::Object(mut object)) => {
            object.insert("decision_type".into(), pyvalue::text(current));
            Value::Object(object)
        }
        (_, record) => record,
    })
}

/// The clause table, or `None` (JSON null) when the record names none, and the ruling index.
pub fn tables(repo: &Repo, record: &Value) -> Py<(Value, Value)> {
    let named = item(record, "clauses")?;
    let clauses = if named.truthy() {
        item(&repo.load(&repo.path(named)?)?, "clauses")?.clone()
    } else {
        Value::Null
    };
    let rulings = item(
        &repo.load(&repo.path(item(record, "rulings")?)?)?,
        "rulings",
    )?
    .clone();
    Ok((clauses, rulings))
}

/// The ruling `name` is, by its key in the index or one of its aliases, or `None`.
pub fn canonical(name: &Value, rulings: &Value) -> Py<Value> {
    if pyvalue::contains(rulings, name)? {
        return Ok(name.clone());
    }
    let Value::Object(index) = rulings else {
        return Err(Raised::Attribute(format!(
            "'{}' object has no attribute 'items'",
            rulings.type_name()
        )));
    };
    for (key, ruling) in index.iter() {
        if pyvalue::contains(item(ruling, "aliases")?, name)? {
            return Ok(pyvalue::text(key));
        }
    }
    Ok(Value::Null)
}

/// The position and answer of the recommender's answer.
pub fn recommender(record: &Value) -> Py<(usize, Value)> {
    for (position, answer) in pyvalue::iterate(item(record, "answers")?)?
        .into_iter()
        .enumerate()
    {
        if pyvalue::is(item(&answer, "by")?, "recommender") {
            return Ok((position, answer));
        }
    }
    Err(Raised::StopIteration)
}

fn outcomes(record: &Value) -> Py<Vec<&'static str>> {
    if get(record, "contract")?.py_eq(&Value::Int("2".into())) {
        Ok(decision_type_of(item(record, "decision_type")?)?.0.to_vec())
    } else {
        Ok(OUTCOMES.to_vec())
    }
}

/// Whether `pin` names a file under the root by `path` whose SHA-256 is its `sha256`.
pub fn pinned(repo: &Repo, pin: &Value) -> Py<bool> {
    let path = match pin {
        Value::Object(object) => match object.get("path") {
            Some(path) if path.truthy() => Some(repo.path(path)?),
            _ => None,
        },
        _ => None,
    };
    let Some(path) = path else { return Ok(false) };
    if !pyvalue::is_file(&path) {
        return Ok(false);
    }
    Ok(pyvalue::text(&repo.digest(&path)?).py_eq(get(pin, "sha256")?))
}

/// Whether a pinned file is one that is never edited: a copy under docs/research or a versioned file.
pub fn kept(repo: &Repo, pin: &Value) -> Py<bool> {
    let path = crate::pyfs::resolve(repo.path(item(pin, "path")?)?.as_bytes());
    let research = crate::pyfs::resolve(repo.join("docs/research").as_bytes());
    let below = path != research
        && (research == b"/" || path.starts_with(&[research.as_slice(), b"/"].concat()));
    let name = path.rsplit(|byte| *byte == b'/').next().unwrap_or_default();
    Ok(below || pyvalue::has_version(&pytext::surrogateescape(name)))
}

pub fn two_facts(facts: &Value) -> Py<bool> {
    if !matches!(facts, Value::Object(_)) {
        return Ok(false);
    }
    for fact in FACTS {
        if !one_of(get(facts, fact)?, &["yes", "no", "cannot-tell"]) {
            return Ok(false);
        }
    }
    Ok(true)
}

fn dossier_faults(repo: &Repo, record: &Value) -> Py<Vec<String>> {
    if !pyvalue::is(item(record, "decision_type")?, "candidate")
        || item(record, "reconstructed")?.truthy()
    {
        return Ok(Vec::new());
    }
    let named = item(record, "dossier")?;
    if !named.truthy() {
        return Ok(vec![
            "a candidate's record needs `dossier`, the directory its facts come from".to_string(),
        ]);
    }
    let mut found = Vec::new();
    for fault in dossier::faults(repo, &repo.path(named)?)? {
        let prefix = format!("{}:", item(record, "group")?.py_str());
        if fault.starts_with(&prefix) {
            found.push(format!("the dossier is refused: {fault}"));
        }
    }
    Ok(found)
}

fn name_faults(answer: &Value, clauses: &Value, rulings: &Value) -> Py<Vec<String>> {
    let mut found = Vec::new();
    for clause in pyvalue::iterate(item(answer, "clauses")?)? {
        if !matches!(clauses, Value::Null) && !pyvalue::contains(clauses, &clause)? {
            found.push(format!(
                "{}: clause {} is not in the clause table",
                item(answer, "by")?.py_str(),
                clause.py_str()
            ));
        }
    }
    for name in pyvalue::iterate(item(answer, "nearest")?)? {
        if !canonical(&name, rulings)?.truthy() {
            found.push(format!(
                "{}: {} is not a ruling in the index",
                item(answer, "by")?.py_str(),
                name.py_str()
            ));
        }
    }
    Ok(found)
}

fn answers(record: &Value) -> Py<Vec<Value>> {
    pyvalue::iterate(item(record, "answers")?)
}

/// The before-record field that carries the owner's saved permission for blind answers from the recommender's
/// model family in one named round. Such a record always goes to the owner and counts nothing toward delegation.
pub const SAME_FAMILY: &str = "same_family_blind";

/// The routing reason a record under that permission always carries.
pub const SAME_FAMILY_REASON: &str = "the owner allowed blind answers from the recommender's model family for this round; the decision stays with the owner and counts nothing toward delegation";

/// Whether the record claims the owner's permission for same-family blind answers.
pub fn same_family(record: &Value) -> Py<bool> {
    Ok(get(record, SAME_FAMILY)?.truthy())
}

/// What the permission lacks: the owner's receipt pinned by path and sha256, a `scope` that is a passage of it
/// naming the `round`, and a `round` that is the one the record's `round_opening` names.
fn same_family_faults(repo: &Repo, record: &Value) -> Py<Vec<String>> {
    let allowance = get(record, SAME_FAMILY)?;
    if !allowance.truthy() {
        return Ok(Vec::new());
    }
    let refused = || {
        Ok(vec![format!(
            "`{SAME_FAMILY}` needs `round`, the round its `round_opening` names, `receipt`, the owner's saved receipt by path and sha256, and `scope`, the passage of that receipt that names the round and allows it"
        )])
    };
    if !matches!(allowance, Value::Object(_)) {
        return refused();
    }
    let (Value::Str(round), Value::Str(scope)) =
        (get(allowance, "round")?, get(allowance, "scope")?)
    else {
        return refused();
    };
    let receipt = get(allowance, "receipt")?;
    if round.trim().is_empty() || scope.trim().is_empty() || !pinned(repo, receipt)? {
        return refused();
    }
    let text = pyvalue::read_bytes(&repo.path(item(receipt, "path")?)?)?;
    let text = String::from_utf8_lossy(&text);
    let opening = get(record, "round_opening")?;
    let opened = if pinned(repo, opening)? {
        repo.load(&repo.path(item(opening, "path")?)?)?
    } else {
        Value::Null
    };
    if !text.contains(scope.as_str())
        || !scope.contains(round.as_str())
        || !matches!(opened, Value::Object(_))
        || !get(&opened, "round")?.py_eq(&pyvalue::text(round))
    {
        return refused();
    }
    Ok(Vec::new())
}

fn party_faults(
    record: &Value,
    rulings: &Value,
    blind_needed: usize,
    same_family_allowed: bool,
) -> Py<Vec<String>> {
    let mut recommenders = Vec::new();
    for answer in answers(record)? {
        if pyvalue::is(item(&answer, "by")?, "recommender") {
            recommenders.push(answer);
        }
    }
    if recommenders.len() != 1 {
        return Ok(vec![
            "the record needs one answer by `recommender`, written before the user is asked".into(),
        ]);
    }
    let mut found = Vec::new();
    let mut blind = Vec::new();
    for answer in answers(record)? {
        if item(&answer, "blind")?.truthy() {
            blind.push(answer);
        }
    }
    if blind_needed > 0 {
        let mut short = blind.len() < blind_needed;
        if !short && !same_family_allowed {
            for answer in &blind {
                if item(answer, "family")?.py_eq(item(&recommenders[0], "family")?) {
                    short = true;
                    break;
                }
            }
        }
        if short {
            let count = if blind_needed == 1 {
                "one blind answer"
            } else {
                "two blind answers"
            };
            found.push(format!(
                "the record needs {count} from another model family than the recommender's"
            ));
        }
    }
    let reviews = item(record, "reviews")?;
    if reviews.truthy()
        && (!canonical(get(reviews, "ruling")?, rulings)?.truthy()
            || !has(reviews, "saved_outcome")?)
    {
        found.push(
            "`reviews` needs the saved ruling, by its name in the index, and its `saved_outcome`"
                .into(),
        );
    }
    Ok(found)
}

/// docs/adjudication-record.md, "Confidence": high leaves no condition short, medium one, low two or more.
fn confidence_faults(answer: &Value, rulings: &Value) -> Py<Vec<String>> {
    let by = item(answer, "by")?.py_str();
    let level = item(answer, "confidence")?;
    let short = item(answer, "short_of_high")?;
    let settle = item(answer, "would_settle")?;
    let mut found = Vec::new();
    let known_level = one_of(level, &LEVELS);
    if !matches!(level, Value::Null) && !known_level {
        found.push(format!(
            "{by}: confidence must be one of {}",
            plural_join(&LEVELS)
        ));
    }
    if !matches!(short, Value::Null) {
        let malformed = match short {
            Value::Array(names) => {
                pyvalue::distinct(names)?.len() != names.len()
                    || names.iter().any(|name| !one_of(name, &SHORT_OF_HIGH))
            }
            _ => true,
        };
        if malformed {
            found.push(format!(
                "{by}: `short_of_high` must list, once each, the conditions of high that fail: {}",
                plural_join(&SHORT_OF_HIGH)
            ));
            return Ok(found);
        }
        let names = short.as_array().unwrap_or_default();
        if let Some(rank) = level
            .as_str()
            .and_then(|level| LEVELS.iter().position(|known| *known == level))
            && names.len().min(2) != rank
        {
            found.push(format!(
                "{by}: {} confidence names {} condition(s) in `short_of_high`; high has none, medium one, low two or more",
                level.py_str(),
                names.len()
            ));
        }
        for (field, name) in STATED {
            if item(answer, field)?.truthy() != names.iter().any(|named| pyvalue::is(named, name)) {
                found.push(format!(
                    "{by}: `short_of_high` names `{name}` exactly when `{field}` says what it is"
                ));
            }
        }
        let mut nearest = Vec::new();
        for name in pyvalue::iterate(item(answer, "nearest")?)? {
            nearest.push(canonical(&name, rulings)?);
        }
        if !names.iter().any(|named| pyvalue::is(named, "no-precedent")) {
            let mut none_current = true;
            for name in &nearest {
                if !name.truthy()
                    || pyvalue::is(
                        item(pyvalue::subscript(rulings, name)?, "reading")?,
                        "current",
                    )
                {
                    none_current = false;
                    break;
                }
            }
            if none_current {
                found.push(format!(
                    "{by}: `nearest` names no ruling made under the current reading, so `short_of_high` must name `no-precedent`"
                ));
            }
        }
    }
    if !matches!(settle, Value::Null) && known_level {
        let high = pyvalue::is(level, "high");
        if !matches!(settle, Value::Bool(flag) if *flag == high) {
            found.push(format!(
                "{by}: `would_settle` must be true at high confidence and false below it"
            ));
        }
    }
    Ok(found)
}

fn answer_faults(repo: &Repo, record: &Value, answer: &Value) -> Py<Vec<String>> {
    let by = item(answer, "by")?.py_str();
    let kind = item(record, "decision_type")?;
    let reconstructed = item(record, "reconstructed")?.truthy();
    let mut found = Vec::new();
    for field in UNRECORDED {
        if matches!(item(answer, field)?, Value::Null) && !reconstructed {
            found.push(format!(
                "{by}: needs `{field}`; only a reconstructed record may leave it null, when nobody recorded it"
            ));
        }
    }
    let outcome = item(answer, "outcome")?;
    let (allowed, _) = decision_type_of(kind)?;
    let kind_name = kind.py_str();
    if !one_of(outcome, allowed) {
        found.push(format!(
            "{by}: a {kind_name} outcome must be one of {}",
            plural_join(allowed)
        ));
    }
    if one_of(item(answer, "outcome")?, &NAMES_A_PROBLEM) && !get(answer, "same_fault_as")?.truthy()
    {
        found.push(format!(
            "{by}: `{}` needs `same_fault_as`, the problem it names",
            item(answer, "outcome")?.py_str()
        ));
    }
    let facts = get(answer, "facts")?;
    if one_of(kind, &["recovery", "pre-existing-raise"])
        && !(reconstructed && matches!(facts, Value::Null))
        && !two_facts(facts)?
    {
        found.push(format!(
            "{by}: a recovery answer needs `facts`: {}, each yes, no or cannot-tell",
            FACTS.join(" and ")
        ));
    }
    let written = item(answer, "written")?;
    if !one_of(written, &WRITTEN) || (!reconstructed && !pyvalue::is(written, "before-question")) {
        found.push(format!(
            "{by}: `written` must be one of {}, and before-question in a record that is not reconstructed",
            plural_join(&WRITTEN)
        ));
    }
    let brief = item(answer, "brief")?;
    if (item(answer, "blind")?.truthy() || brief.truthy())
        && !(pinned(repo, brief)? && kept(repo, brief)?)
    {
        found.push(format!(
            "{by}: `brief` must pin the copy of the brief this answer was given, by path and sha256; a blind answer needs one"
        ));
    }
    let sheet = get(item(record, "case")?, "precedents")?;
    if !matches!(sheet, Value::Null) || !matches!(get(answer, "precedents")?, Value::Null) {
        let read = get(answer, "precedents")?;
        if !read.py_eq(sheet) || !pinned(repo, read)? {
            found.push(format!(
                "{by}: `precedents` must pin the same sheet as `case.precedents`, the one this answer read"
            ));
        }
    }
    Ok(found)
}

fn second_faults(repo: &Repo, record: &Value) -> Py<Vec<String>> {
    let mut found = Vec::new();
    for field in BEFORE.iter().chain(&BEFORE_2_EXTRA) {
        if !has(record, field)? {
            found.push(format!("the record needs `{field}`"));
        }
    }
    if found.is_empty() && !is_decision_type(item(record, "decision_type")?)? {
        found.push(format!(
            "decision_type must be one of {}",
            DECISION_TYPES
                .iter()
                .map(|(name, _, _)| *name)
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if !found.is_empty() {
        return Ok(found);
    }
    if !matches!(item(record, "group")?, Value::Str(group) if !group.is_empty()) {
        return Ok(vec![
            "`group` must name the one thing decided; a question that holds several decisions gets one record for each".into(),
        ]);
    }
    let rule = item(record, "rule")?;
    if !pinned(repo, rule)? {
        found.push("`rule` must pin the rule text the answers applied, by path and sha256".into());
    } else if !kept(repo, rule)? {
        found.push("`rule` must pin a versioned file or the copy the round saved under docs/research, never a file that is edited in place".into());
    }
    let case = item(record, "case")?;
    let case_faulty = !pinned(repo, case)?
        || !get(case, "written_by")?.truthy()
        || !has(case, "precedents")?
        || {
            let precedents = item(case, "precedents")?;
            !matches!(precedents, Value::Null) && !pinned(repo, precedents)?
        };
    if case_faulty {
        found.push("`case` must pin the neutral file every blind party read, by path and sha256, with `written_by` and `precedents`, the pinned sheet of earlier rulings or null".into());
    }
    let (clauses, rulings) = tables(repo, record)?;
    found.extend(dossier_faults(repo, record)?);
    if pyvalue::is(item(record, "decision_type")?, "candidate")
        && !item(record, "reconstructed")?.truthy()
        && item(record, "dossier")?.truthy()
    {
        let entries = dossier::entries(repo, &repo.path(item(record, "dossier")?)?);
        let mut none_named = true;
        for entry in &entries.items {
            if item(entry, "group")?.py_eq(item(record, "group")?) {
                none_named = false;
                break;
            }
        }
        if none_named {
            if let Some(error) = entries.error {
                return Err(error);
            }
            found.push(
                "`group` must be one candidate of the dossier, as its summary.json names it".into(),
            );
        }
    }
    for answer in answers(record)? {
        let mut missing = Vec::new();
        for field in ANSWER.iter().chain(&ANSWER_2_EXTRA) {
            if !has(&answer, field)? {
                missing.push(*field);
            }
        }
        if !missing.is_empty() {
            let by = pyvalue::get_or(&answer, "by", &pyvalue::text("an answer"))?.py_str();
            found.push(format!("{by}: needs {}", missing.join(", ")));
            continue;
        }
        found.extend(answer_faults(repo, record, &answer)?);
        found.extend(confidence_faults(&answer, &rulings)?);
        found.extend(name_faults(&answer, &clauses, &rulings)?);
    }
    let mut parties = Vec::new();
    for answer in answers(record)? {
        parties.push(get(&answer, "by")?.clone());
    }
    let (_, blind_needed) = decision_type_of(item(record, "decision_type")?)?;
    if pyvalue::distinct(&parties)?.len() != parties.len() {
        found.push("each party answers once; a `by` name is repeated".into());
    }
    if blind_needed == 0 && parties.len() != 1 {
        found.push(format!(
            "a {} record holds one first answer, the recommender's",
            item(record, "decision_type")?.py_str()
        ));
    }
    found.extend(same_family_faults(repo, record)?);
    if found.is_empty() {
        party_faults(record, &rulings, blind_needed, same_family(record)?)
    } else {
        Ok(found)
    }
}

/// What the before-record lacks; empty when it follows docs/adjudication-record.md.
pub fn faults(repo: &Repo, record: &Value) -> Py<Vec<String>> {
    let contract = pyvalue::get_or(record, "contract", &Value::Int("1".into()))?.clone();
    if !contract.py_eq(&Value::Int("1".into())) {
        return if contract.py_eq(&Value::Int("2".into())) {
            second_faults(repo, record)
        } else {
            Ok(vec![
                "`contract` must be 1 or 2; a record without it is contract 1".into(),
            ])
        };
    }
    let mut found = Vec::new();
    for field in BEFORE {
        if !has(record, field)? {
            found.push(format!("the record needs `{field}`"));
        }
    }
    if !found.is_empty() {
        return Ok(found);
    }
    if !one_of(item(record, "decision_type")?, &CONTRACT_1_TYPES) {
        found.push(format!(
            "decision_type must be one of {}",
            plural_join(&CONTRACT_1_TYPES)
        ));
    }
    let rule = repo.path(item(item(record, "rule")?, "path")?)?;
    let rule_faulty = !pyvalue::is_file(&rule)
        || (!item(record, "reconstructed")?.truthy()
            && !pyvalue::text(&repo.digest(&rule)?).py_eq(item(item(record, "rule")?, "sha256")?));
    if rule_faulty {
        found.push("`rule` must pin the rule text the answers applied, by path and sha256".into());
    }
    if !item(record, "clauses")?.truthy() {
        found.push(
            "contract 1 needs `clauses`, the table of rules; only a contract 2 record may leave it null".into(),
        );
        return Ok(found);
    }
    let (clauses, rulings) = tables(repo, record)?;
    found.extend(dossier_faults(repo, record)?);
    for answer in answers(record)? {
        let mut missing = Vec::new();
        for field in ANSWER {
            if !has(&answer, field)? {
                missing.push(field);
            }
        }
        if !missing.is_empty() {
            let by = pyvalue::get_or(&answer, "by", &pyvalue::text("an answer"))?.py_str();
            found.push(format!("{by}: needs {}", missing.join(", ")));
            continue;
        }
        if !one_of(item(&answer, "outcome")?, &OUTCOMES) {
            found.push(format!(
                "{}: outcome must be one of {}",
                item(&answer, "by")?.py_str(),
                plural_join(&OUTCOMES)
            ));
        }
        found.extend(name_faults(&answer, &clauses, &rulings)?);
    }
    if found.is_empty() {
        party_faults(record, &rulings, 2, false)
    } else {
        Ok(found)
    }
}

/// Why this ruling is the user's whatever anyone's confidence. An empty list is necessary for delegation, not
/// sufficient.
pub fn reasons(repo: &Repo, record: &Value) -> Py<Vec<String>> {
    let (clauses, rulings) = tables(repo, record)?;
    let all = answers(record)?;
    let (_, first) = recommender(record)?;
    let mut blind = Vec::new();
    for answer in &all {
        if item(answer, "blind")?.truthy() {
            blind.push(answer);
        }
    }
    let kind = item(record, "decision_type")?;
    let mut found = Vec::new();
    if same_family(record)? {
        found.push(SAME_FAMILY_REASON.into());
    }
    if !one_of(kind, &["candidate", "reconciliation"]) {
        found.push(format!(
            "a {} decision is the user's under ADR-0006",
            kind.py_str()
        ));
    }
    let mut blind_outcomes = Vec::new();
    for answer in &blind {
        blind_outcomes.push(item(answer, "outcome")?.clone());
    }
    if pyvalue::distinct(&blind_outcomes)?.len() > 1 {
        found.push("the blind assessors disagree with each other".into());
    }
    for answer in &blind {
        if !item(answer, "outcome")?.py_eq(item(&first, "outcome")?) {
            found.push("the recommendation differs from a blind assessor's answer".into());
            break;
        }
    }
    for answer in &all {
        if one_of(item(answer, "outcome")?, &COULD_NOT_TELL) {
            found.push(format!("{} could not tell", item(answer, "by")?.py_str()));
        }
    }
    for answer in &all {
        if pyvalue::is(item(answer, "outcome")?, "relied-on") {
            found.push(format!(
                "{} finds it relied on and not promised; only the user can put it on the answer key",
                item(answer, "by")?.py_str()
            ));
        }
    }
    for answer in &all {
        if item(answer, "rule_gap")?.truthy() {
            found.push(format!(
                "{} names a gap in the rule: {}",
                item(answer, "by")?.py_str(),
                item(answer, "rule_gap")?.py_str()
            ));
        }
    }
    for answer in &all {
        if item(answer, "conflict")?.truthy() {
            found.push(format!(
                "{} finds two rules pointing different ways: {}",
                item(answer, "by")?.py_str(),
                item(answer, "conflict")?.py_str()
            ));
        }
    }
    for answer in &all {
        if !item(answer, "nearest")?.truthy() {
            found.push(format!(
                "{} found no earlier ruling of this shape",
                item(answer, "by")?.py_str()
            ));
        }
    }
    let mut nearest = Vec::new();
    for answer in &all {
        for name in pyvalue::iterate(item(answer, "nearest")?)? {
            nearest.push(canonical(&name, &rulings)?);
        }
    }
    for name in pyvalue::sorted(pyvalue::distinct(&nearest)?)? {
        let ruling = pyvalue::subscript(&rulings, &name)?;
        if pyvalue::is(item(ruling, "reading")?, "earlier")
            && !get(pyvalue::subscript(&rulings, &name)?, "rule")?.truthy()
        {
            found.push(format!(
                "{} was decided under the earlier reading and has not been shown again",
                name.py_str()
            ));
        }
    }
    let reviews = item(record, "reviews")?;
    let reviewed = if reviews.truthy() {
        canonical(item(reviews, "ruling")?, &rulings)?
    } else {
        Value::Null
    };
    if matches!(clauses, Value::Null) {
        found.push("no rule of this decision type has been tested blind".into());
    }
    let mut cited = Vec::new();
    if clauses.truthy() {
        for answer in &all {
            cited.extend(pyvalue::iterate(item(answer, "clauses")?)?);
        }
    }
    for clause in pyvalue::sorted(pyvalue::distinct(&cited)?)? {
        let source = pyvalue::subscript(&clauses, &clause)?;
        let from = item(source, "from")?;
        if pyvalue::contains(from, &reviewed)? {
            found.push(format!(
                "clause {} was written from the ruling under review",
                clause.py_str()
            ));
        } else if pyvalue::length(item(source, "from")?)? < 2 {
            found.push(format!(
                "clause {} rests on {} ruling(s)",
                clause.py_str(),
                pyvalue::length(item(source, "from")?)?
            ));
        } else if !item(source, "tested_on")?.truthy() {
            found.push(format!(
                "clause {} has not been applied blind to a case it was not written from",
                clause.py_str()
            ));
        }
    }
    if item(record, "reviews")?.truthy()
        && !item(&first, "outcome")?.py_eq(item(item(record, "reviews")?, "saved_outcome")?)
    {
        found.push("the recommendation would change a saved ruling".into());
    }
    if pyvalue::is(item(record, "decision_type")?, "candidate") && item(record, "dossier")?.truthy()
    {
        let entries = dossier::entries(repo, &repo.path(item(record, "dossier")?)?);
        let mut places = Vec::new();
        for entry in &entries.items {
            if item(entry, "group")?.py_eq(item(record, "group")?) {
                places.extend(dossier::blocked(entry)?);
            }
        }
        if let Some(error) = entries.error {
            return Err(error);
        }
        found.extend(
            places
                .into_iter()
                .map(|place| format!("the search of {place} could not be made")),
        );
    }
    Ok(found)
}

/// `" ".join(str(text).split()).replace("|", "\\|")`.
fn cell(text: &Value) -> String {
    pyvalue::split_whitespace(&text.py_str())
        .join(" ")
        .replace('|', "\\|")
}

/// The answer's pick: its outcome, the problem it names and its two facts.
pub fn pick(answer: &Value) -> Py<String> {
    let facts = get(answer, "facts")?;
    let outcome = item(answer, "outcome")?;
    let same = get(answer, "same_fault_as")?;
    let named = if same.truthy() {
        format!(" of {}", item(answer, "same_fault_as")?.py_str())
    } else {
        String::new()
    };
    let Value::Str(outcome) = outcome else {
        return Err(Raised::Type(format!(
            "unsupported operand type(s) for +: '{}' and 'str'",
            outcome.type_name()
        )));
    };
    let stated = if facts.truthy() {
        format!(
            " (says what goes wrong: {}, identifies the cause as a fault: {})",
            item(facts, "says_what")?.py_str(),
            item(facts, "identifies_cause")?.py_str()
        )
    } else {
        String::new()
    };
    Ok(format!("{outcome}{named}{stated}"))
}

/// docs/claim-adjudication.md, "Reconcile": each party whose pick is not the recommender's.
fn disagreements(record: &Value) -> Py<Vec<Value>> {
    let (position, first) = recommender(record)?;
    let mut found = Vec::new();
    for (at, answer) in answers(record)?.into_iter().enumerate() {
        if at != position && pick(&answer)? != pick(&first)? {
            found.push(answer);
        }
    }
    Ok(found)
}

/// What the user is shown: the whole case file every blind party read, every party's first answer, and what the
/// recommender must reconcile.
pub fn question(repo: &Repo, record: &Value) -> Py<String> {
    let mut rows = Vec::new();
    for answer in answers(record)? {
        let by = cell(item(&answer, "by")?);
        let picked = cell(&pyvalue::text(&pick(&answer)?));
        let confidence = item(&answer, "confidence")?;
        let confidence = if confidence.truthy() {
            confidence.py_str()
        } else {
            "not recorded".to_string()
        };
        let short = item(&answer, "short_of_high")?;
        let short = if short.truthy() {
            let names = pyvalue::iterate(short)?;
            format!(" ({})", pyvalue::join(", ", &names)?)
        } else {
            String::new()
        };
        let reason = cell(item(&answer, "reason")?);
        rows.push(format!(
            "| {by} | {picked} | {confidence}{short} | {reason} |"
        ));
    }
    let (_, first) = recommender(record)?;
    let mut differing = Vec::new();
    for answer in disagreements(record)? {
        differing.push(format!(
            "- {} picks {} where the recommender picks {}. Its reason: {}",
            item(&answer, "by")?.py_str(),
            pick(&answer)?,
            pick(&first)?,
            pyvalue::split_whitespace(&item(&answer, "reason")?.py_str()).join(" ")
        ));
    }
    if differing.is_empty() {
        differing.push("- None: every party's pick is the recommender's.".into());
    }
    let case = item(item(record, "case")?, "path")?;
    let heading = format!("The case, {}:", case.py_str());
    let text = pyvalue::read_text(&repo.path(item(item(record, "case")?, "path")?)?)?;
    let mut lines = vec![
        heading,
        String::new(),
        pytext::strip(&text).to_string(),
        String::new(),
        "First answers:".into(),
        String::new(),
        "| Party | Pick | Confidence | Reason |".into(),
        "| --- | --- | --- | --- |".into(),
    ];
    lines.extend(rows);
    lines.extend([
        String::new(),
        "To reconcile before asking:".into(),
        String::new(),
    ]);
    lines.extend(differing);
    Ok(lines.join("\n"))
}

/// `(isinstance(value, str) and value.strip())`.
fn written_text(value: &Value) -> bool {
    matches!(value, Value::Str(text) if !pytext::strip(text).is_empty())
}

/// docs/adjudication-record.md, "The after-record".
fn second_after_faults(repo: &Repo, record: &Value, after: &Value) -> Py<Vec<String>> {
    if !get(record, "contract")?.py_eq(&Value::Int("2".into())) {
        return Ok(vec![
            "a contract 2 after-record belongs to a contract 2 before-record".into(),
        ]);
    }
    let mut found = Vec::new();
    let by_agents = pyvalue::is(item(after, "settled_by")?, "agents");
    if !one_of(item(after, "settled_by")?, &SETTLED_BY) {
        found.push(format!(
            "`settled_by` must be one of {}",
            plural_join(&SETTLED_BY)
        ));
    }
    let policy = item(after, "policy")?;
    if !matches!(policy, Value::Null) && !pinned(repo, policy)? {
        found.push(
            "`policy` must be null, or pin the policy the decision was made under, by path and sha256"
                .into(),
        );
    }
    let asked = item(after, "asked")?;
    if !matches!(asked, Value::Int(_))
        || pyvalue::compare_with(asked, if by_agents { 0 } else { 1 })?.is_lt()
    {
        found.push(
            "`asked` must count the times the user was asked: at least 1 when the user settled it"
                .into(),
        );
        return Ok(found);
    }
    if !matches!(item(after, "by_default")?, Value::Bool(_)) {
        found.push("`by_default` must be true or false".into());
    }
    let ground = item(after, "ground")?;
    if !matches!(ground, Value::Null) && !written_text(ground) {
        found.push("`ground` must be the user's own words, or null when they gave none".into());
    }
    let kind = item(record, "decision_type")?;
    let recovery = one_of(kind, &["recovery", "pre-existing-raise"]);
    if recovery && !two_facts(get(after, "facts")?)? {
        found.push(format!(
            "a recovery decision needs `facts`: {}, each yes, no or cannot-tell",
            FACTS.join(" and ")
        ));
    }
    let outcome = item(after, "outcome")?;
    if one_of(outcome, &NAMES_A_PROBLEM) && !written_text(get(after, "same_fault_as")?) {
        found.push(format!(
            "`{}` needs `same_fault_as`, the problem it names",
            outcome.py_str()
        ));
    }
    let miss = item(after, "miss")?;
    let lesson = item(after, "lesson")?;
    let (_, first) = recommender(record)?;
    let owed = !item(&first, "outcome")?.py_eq(item(after, "outcome")?)
        || pyvalue::compare_with(asked, 1)?.is_gt()
        || (one_of(
            item(record, "decision_type")?,
            &["recovery", "pre-existing-raise"],
        ) && two_facts(get(after, "facts")?)?
            && !get(&first, "facts")?.py_eq(get(after, "facts")?))
        || (one_of(item(after, "outcome")?, &NAMES_A_PROBLEM)
            && get(after, "same_fault_as")?.truthy()
            && !get(&first, "same_fault_as")?.py_eq(item(after, "same_fault_as")?));
    let miss_malformed = !matches!(miss, Value::Null)
        && !(matches!(miss, Value::Object(_))
            && one_of(get(miss, "cause")?, &CAUSES)
            && written_text(get(miss, "note")?));
    if miss_malformed {
        found.push(format!(
            "`miss` must be null, or hold `cause`, one of {}, and `note`, one sentence on what happened",
            plural_join(&CAUSES)
        ));
    } else if owed && matches!(miss, Value::Null) {
        found.push("`miss` must name its cause: the recommender's first answer was not the decision, or the user was asked more than once".into());
    } else if miss.truthy() && !owed {
        found.push("`miss` must be null: the recommender's first answer was the decision and the user was asked once".into());
    }
    let lesson_malformed = !matches!(lesson, Value::Null) && {
        let mut complete = matches!(lesson, Value::Object(_));
        if complete {
            for field in ["says", "goes_to"] {
                if !written_text(get(lesson, field)?) {
                    complete = false;
                    break;
                }
            }
        }
        !complete
    };
    if lesson_malformed {
        found.push("`lesson` must be null, or hold `says`, one sentence, and `goes_to`, the file that was changed".into());
    } else if lesson.truthy() && matches!(miss, Value::Null) {
        found.push("`lesson` needs a `miss`: it says what the miss changed".into());
    } else if lesson.truthy() {
        let goes_to = item(lesson, "goes_to")?.py_str();
        let file = goes_to.split('#').next().unwrap_or_default();
        if !pyvalue::is_file(&repo.root.join(file.as_bytes())) {
            found.push(
                "`lesson.goes_to` must name a file in the repository, the one that was changed"
                    .into(),
            );
        }
    }
    Ok(found)
}

/// What the after-record lacks, and, unless `check_settlement` is false, why agents could not settle it.
pub fn after_faults(
    repo: &Repo,
    record: &Value,
    after: &Value,
    before_path: &PyPath,
    check_settlement: bool,
    pairs: Option<&[Pair]>,
) -> Py<Vec<String>> {
    let contract = pyvalue::get_or(after, "contract", &Value::Int("1".into()))?.clone();
    let one = Value::Int("1".into());
    let two = Value::Int("2".into());
    if !contract.py_eq(&one) && !contract.py_eq(&two) {
        return Ok(vec![
            "`contract` must be 1 or 2; a record without it is contract 1".into(),
        ]);
    }
    if matches!(contract, Value::Float(_)) {
        return Err(Raised::Type(
            "tuple indices must be integers or slices, not float".into(),
        ));
    }
    let second = contract.py_eq(&two);
    let fields: Vec<&str> = if second {
        AFTER_2.to_vec()
    } else {
        AFTER.to_vec()
    };
    let mut found = Vec::new();
    for field in fields {
        if !has(after, field)? {
            found.push(format!("the after-record needs `{field}`"));
        }
    }
    if !found.is_empty() {
        return Ok(found);
    }
    let pins_before = item(after, "ruling")?.py_eq(item(record, "ruling")?)
        && get(item(after, "before")?, "sha256")?.py_eq(&pyvalue::text(&repo.digest(before_path)?));
    if !pins_before {
        found.push("`before` must pin the before-record as it was when the user was asked; that file is never edited".into());
    }
    let allowed = outcomes(record)?;
    if !one_of(item(after, "outcome")?, &allowed) {
        found.push(format!("outcome must be one of {}", allowed.join(", ")));
    }
    if check_settlement {
        found.extend(delegation::settlement_faults(
            repo,
            record,
            after,
            Some(before_path),
            pairs,
        )?);
    }
    if second {
        found.extend(second_after_faults(repo, record, after)?);
    }
    Ok(found)
}

/// docs/adjudication-record.md, "The round file": every ruling file of a round has its record, or the reason it
/// has none.
pub fn round_faults(repo: &Repo, directory: &PyPath) -> Py<Vec<String>> {
    let sheet = repo.load(&directory.join(b"round.json"))?;
    let mut found = Vec::new();
    for field in ROUND {
        if !has(&sheet, field)? {
            found.push(format!("round.json needs `{field}`"));
        }
    }
    if !found.is_empty() {
        return Ok(found);
    }
    let named = matches!(item(&sheet, "round")?, Value::Str(name) if !name.is_empty());
    if !named
        || !pyvalue::is_date(&item(&sheet, "opened")?.py_str())
        || !matches!(item(&sheet, "closed")?, Value::Bool(_))
    {
        found.push("round.json needs `round`, its name, `opened`, a date as YYYY-MM-DD, and `closed`, true or false".into());
    }
    let no_record = item(&sheet, "no_record")?;
    let reasons_given = match no_record {
        Value::Object(reasons) => reasons.iter().all(|(_, reason)| written_text(reason)),
        _ => false,
    };
    if !reasons_given {
        found.push("`no_record` must give, for each ruling file without a record, its name or a pattern of names and the reason".into());
        return Ok(found);
    }
    let patterns: Vec<Vec<u32>> = no_record
        .as_object()
        .map(|reasons| {
            reasons
                .iter()
                .map(|(pattern, _)| pattern.chars().map(u32::from).collect())
                .collect()
        })
        .unwrap_or_default();
    let names = listing(directory);
    for (name, points) in glob(&names, &points_of("*.md"))? {
        let stem = stem(&points);
        let mut single = stem.clone();
        single.extend(points_of(".before.json"));
        let mut several = stem;
        several.extend(points_of(".*.before.json"));
        let recorded = !glob(&names, &single)?.is_empty() || !glob(&names, &several)?.is_empty();
        if recorded {
            continue;
        }
        let mut excused = false;
        for pattern in &patterns {
            if pyvalue::fnmatchcase(&points, pattern)? {
                excused = true;
                break;
            }
        }
        if !excused {
            found.push(format!(
                "{} has no before-record, and `no_record` gives no reason",
                pytext::stderr_text(&name)
            ));
        }
    }
    if item(&sheet, "closed")?.truthy() {
        for (name, _) in glob(&names, &points_of("*.before.json"))? {
            let stem = &name[..name.len() - ".before.json".len()];
            let after = directory.join(&[stem, b".after.json"].concat());
            if !pyvalue::is_file(&after) {
                found.push(format!(
                    "{} has no after-record, and the round is closed",
                    pytext::stderr_text(&name)
                ));
            }
        }
    }
    Ok(found)
}

fn points_of(text: &str) -> Vec<u32> {
    text.chars().map(u32::from).collect()
}

fn listing(directory: &PyPath) -> Vec<(Vec<u8>, Vec<u32>)> {
    crate::pyfs::names(directory.as_bytes())
        .into_iter()
        .map(|name| {
            let points = pytext::surrogateescape(&name);
            (name, points)
        })
        .collect()
}

/// `sorted(directory.glob(pattern))` for a pattern of one component, by name.
fn glob(names: &[(Vec<u8>, Vec<u32>)], pattern: &[u32]) -> Py<Vec<(Vec<u8>, Vec<u32>)>> {
    let mut found = Vec::new();
    for (name, points) in names {
        if pyvalue::fnmatchcase(points, pattern)? {
            found.push((name.clone(), points.clone()));
        }
    }
    found.sort_by(|left, right| left.1.cmp(&right.1));
    Ok(found)
}

/// `PurePath.stem` as Python 3.14 reads it: the name before its last dot, when that leaves a character other
/// than a dot.
fn stem(name: &[u32]) -> Vec<u32> {
    let dot = u32::from('.');
    match name.iter().rposition(|point| *point == dot) {
        Some(position) if name[..position].iter().any(|point| *point != dot) => {
            name[..position].to_vec()
        }
        _ => name.to_vec(),
    }
}

const RANKED: [&str; 4] = ["suggestion", "relied-on", "minor-defect", "problem"];

fn rank(value: &Value) -> i64 {
    RANK.iter()
        .find(|(name, _)| pyvalue::is(value, name))
        .map_or(0, |(_, rank)| *rank)
}

/// What the user's decision went against. A later summary counts these as misses.
pub fn surprises(repo: &Repo, record: &Value, after: &Value) -> Py<Vec<String>> {
    let (clauses, rulings) = tables(repo, record)?;
    let (_, first) = recommender(record)?;
    let decided = item(after, "outcome")?;
    let mut found = Vec::new();
    let recommended = item(&first, "outcome")?;
    if !decided.py_eq(recommended) {
        pyvalue::hashable(recommended)?;
        pyvalue::hashable(decided)?;
        let lean = if one_of(recommended, &RANKED) && one_of(decided, &RANKED) {
            rank(recommended) - rank(decided)
        } else {
            0
        };
        let leaning = match lean.cmp(&0) {
            std::cmp::Ordering::Less => ", which was more lenient",
            std::cmp::Ordering::Greater => ", which was stricter",
            std::cmp::Ordering::Equal => "",
        };
        found.push(format!("against the first recommendation{leaning}"));
    }
    if pyvalue::compare_with(item(after, "asked")?, 1)?.is_gt() {
        found.push(format!(
            "asked {} times before it was settled",
            item(after, "asked")?.py_str()
        ));
    }
    let reviews = item(record, "reviews")?;
    let reviewed = if reviews.truthy() {
        canonical(item(reviews, "ruling")?, &rulings)?
    } else {
        Value::Null
    };
    for answer in answers(record)? {
        if !item(&answer, "blind")?.truthy() {
            continue;
        }
        if !item(&answer, "outcome")?.py_eq(decided) {
            found.push(format!(
                "against {} ({} confidence, would settle: {})",
                item(&answer, "by")?.py_str(),
                item(&answer, "confidence")?.py_str(),
                item(&answer, "would_settle")?.py_str()
            ));
        } else if reviewed.truthy() && clauses.truthy() {
            let mut through = false;
            for clause in pyvalue::iterate(item(&answer, "clauses")?)? {
                if pyvalue::contains(
                    item(pyvalue::subscript(&clauses, &clause)?, "from")?,
                    &reviewed,
                )? {
                    through = true;
                    break;
                }
            }
            if through {
                found.push(format!(
                    "{} agrees through a clause written from this ruling; not counted as agreement",
                    item(&answer, "by")?.py_str()
                ));
            }
        }
    }
    Ok(found)
}
