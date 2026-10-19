//! Pre-existing-problem admission and whether a review raised one, independent of ordinary recovery, ported from
//! `bench/tools/pre_existing_grading.py`.

use crate::claim_grading::{self, filled, has_exactly, is, one_of};
use crate::pyjson::{Object, Value};
use crate::pyobject::{self as py, PyResult, Raised};

pub const CONTRACT: &str = "current-verdicts/v3";
pub const FIELDS: [&str; 2] = ["pre_existing_problems", "pre_existing_candidate"];
const FACTS: [&str; 3] = ["yes", "no", "cannot-tell"];

/// Problems with one verdict contract v3 claim: the v2 rules on its v2 fields, then its pre-existing problems.
pub fn verdict_problems(
    claim: &Value,
    families: &Value,
    pre_existing: &Value,
) -> PyResult<Vec<String>> {
    let fields: Vec<&str> = claim_grading::V2_CLAIM_FIELDS
        .iter()
        .chain(&FIELDS)
        .copied()
        .collect();
    let Some(object) = claim.as_object().filter(|_| has_exactly(claim, &fields)) else {
        return Ok(vec![
            "v3 claims need the v2 fields plus pre_existing_problems and pre_existing_candidate"
                .to_string(),
        ]);
    };
    let ordinary: Object = object
        .iter()
        .filter(|(key, _)| !FIELDS.contains(&key.as_str()))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();
    let mut problems = claim_grading::verdict_problems_v2(&Value::Object(ordinary), families)?;
    problems.extend(raising_problems(claim, pre_existing)?);
    Ok(problems)
}

/// What a claim's pre-existing-problem entries and candidate break: an unlisted problem, an ordinary problem named
/// beside them, or a claim that is neither true and outside this change nor unresolved for want of evidence.
pub fn raising_problems(claim: &Value, pre_existing: &Value) -> PyResult<Vec<String>> {
    let entries = py::getitem(claim, "pre_existing_problems")?;
    let candidate = py::getitem(claim, "pre_existing_candidate")?;
    let Some(entries) = entries.as_array() else {
        return Ok(vec!["pre_existing_problems must be a list".to_string()]);
    };
    let mut problems = Vec::new();
    let mut ids: Vec<&str> = Vec::new();
    for entry in entries {
        let field = |name: &str| py::getitem(entry, name).unwrap_or(&Value::Null);
        let problem = field("problem");
        let valid = has_exactly(
            entry,
            &["problem", "says_what", "identifies_cause", "reason"],
        ) && problem.as_str().is_some()
            && py::contains(pre_existing, problem)?
            && one_of(field("says_what"), &FACTS)
            && one_of(field("identifies_cause"), &FACTS)
            && filled(field("reason"));
        if !valid {
            problems.push(
                "pre_existing_problems entries need a listed problem, both wording facts and a reason"
                    .to_string(),
            );
            continue;
        }
        let problem = problem.as_str().unwrap_or_default();
        if ids.contains(&problem) {
            problems.push("pre_existing_problems names each problem once".to_string());
        }
        ids.push(problem);
    }
    if !matches!(candidate, Value::Null) && !filled(candidate) {
        problems.push("pre_existing_candidate must be null or a non-empty string".to_string());
    }
    if !entries.is_empty() || !matches!(candidate, Value::Null) {
        if py::getitem(claim, "known_problems")?.truthy()
            || !matches!(py::getitem(claim, "candidate")?, Value::Null)
        {
            problems.push(
                "one assertion cannot name both ordinary and pre-existing problems or candidates"
                    .to_string(),
            );
        }
        let outcome = py::getitem(claim, "outcome")?;
        let supported =
            is(outcome, "outside-this-change") && is(py::getitem(claim, "true")?, "yes");
        let inaccessible = is(outcome, "unresolved")
            && is(py::getitem(claim, "true")?, "cannot-check")
            && matches!(candidate, Value::Null)
            && entries.iter().all(|entry| {
                entry
                    .as_object()
                    .and_then(|entry| entry.get("says_what"))
                    .is_some_and(|answer| is(answer, "cannot-tell"))
            });
        if !supported && !inaccessible {
            problems.push(
                "raising a pre-existing problem requires a true outside-this-change claim or unresolved inaccessible evidence"
                    .to_string(),
            );
        }
        if !entries.is_empty() && !matches!(candidate, Value::Null) {
            problems.push(
                "a listed pre-existing problem cannot also be a new pre-existing-problem candidate"
                    .to_string(),
            );
        }
    }
    Ok(problems)
}

/// Whether one review raised one pre-existing problem: its outcome, the claims behind it and both item facts.
pub fn assessment(problem: &Value, claims: &[Value], admitted: bool) -> PyResult<Value> {
    let mut facts: Vec<(Value, &Value)> = Vec::new();
    for claim in claims {
        for entry in py::iterate(py::getitem(claim, "pre_existing_problems")?)? {
            let std::borrow::Cow::Borrowed(entry) = entry else {
                return Err(Raised::TypeError);
            };
            if py::eq(py::getitem(entry, "problem")?, py::getitem(problem, "id")?) {
                facts.push((py::getitem(claim, "id")?.clone(), entry));
            }
        }
    }
    let with = |answer: &str| -> PyResult<Vec<Value>> {
        let mut found = Vec::new();
        for (identifier, entry) in &facts {
            if is(py::getitem(entry, "says_what")?, answer) {
                found.push(identifier.clone());
            }
        }
        Ok(found)
    };
    let raised = with("yes")?;
    let unresolved = with("cannot-tell")?;
    let (outcome, ids) = if !admitted {
        (
            "unresolved",
            if raised.is_empty() {
                unresolved
            } else {
                raised
            },
        )
    } else if !raised.is_empty() {
        ("raised", raised)
    } else if !unresolved.is_empty() {
        ("unresolved", unresolved)
    } else {
        ("not-raised", Vec::new())
    };
    let mut result = Object::default();
    result.insert(
        "problem_id".to_string(),
        py::getitem(problem, "id")?.clone(),
    );
    result.insert("outcome".to_string(), Value::Str(outcome.to_string()));
    result.insert("claim_ids".to_string(), Value::Array(ids));
    for name in ["says_what", "identifies_cause"] {
        let answers = facts
            .iter()
            .map(|(_, entry)| py::getitem(entry, name))
            .collect::<PyResult<Vec<_>>>()?;
        result.insert(
            name.to_string(),
            Value::Str(claim_grading::item_fact(&answers).to_string()),
        );
    }
    Ok(Value::Object(result))
}

/// A saved claim's pre-existing fields in the shape a verdict holds them.
pub fn saved_claim(claim: &Value) -> PyResult<Value> {
    let mut entries = Vec::new();
    for entry in py::iterate(py::getitem(claim, "pre_existing_problems")?)? {
        let entry = entry.as_ref();
        let mut saved = Object::default();
        saved.insert(
            "problem".to_string(),
            py::getitem(entry, "problem_id")?.clone(),
        );
        for name in ["says_what", "identifies_cause", "reason"] {
            saved.insert(name.to_string(), py::getitem(entry, name)?.clone());
        }
        entries.push(Value::Object(saved));
    }
    let mut result = Object::default();
    result.insert("pre_existing_problems".to_string(), Value::Array(entries));
    result.insert(
        "pre_existing_candidate".to_string(),
        py::getitem(claim, "pre_existing_candidate_id")?.clone(),
    );
    Ok(Value::Object(result))
}

/// What a credit ruling rules on: its pre-existing problem, else its family; `None` when it names neither.
pub fn credit_subject(ruling: &Value) -> PyResult<Value> {
    let family = py::get(ruling, "family_id")?
        .cloned()
        .unwrap_or(Value::Null);
    Ok(py::get(ruling, "pre_existing_problem_id")?
        .cloned()
        .unwrap_or(family))
}
