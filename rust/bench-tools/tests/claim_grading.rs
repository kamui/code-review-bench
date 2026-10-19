//! The grading rules of `bench_tools::claim_grading`, `pre_existing` and `grading_validation` that no command runs
//! yet: hand-stated outcomes for pending, proposed and approved eligibility, and a differential against the retained
//! Python modules over varied and malformed inputs, including which exception each raises.

use bench_tools::pyjson::{self, Dumps, Duplicates, Object, Value};
use bench_tools::pyobject::{self, Raised};
use bench_tools::{claim_grading, grading_validation, pre_existing};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn json(text: &str) -> Value {
    pyjson::loads(text, Duplicates::KeepLast).unwrap()
}

fn s(text: &str) -> Value {
    Value::Str(text.to_string())
}

fn object(members: Vec<(&str, Value)>) -> Value {
    Value::Object(
        members
            .into_iter()
            .map(|(key, value)| (key.to_string(), value))
            .collect::<Object>(),
    )
}

fn raised(kind: Raised) -> Value {
    let name = match kind {
        Raised::KeyError => "KeyError",
        Raised::TypeError => "TypeError",
        Raised::IndexError => "IndexError",
        Raised::AttributeError => "AttributeError",
        Raised::ValueError => "ValueError",
        Raised::Refused => "Refused",
    };
    object(vec![("raised", s(name))])
}

fn ok(value: Value) -> Value {
    object(vec![("ok", value)])
}

fn recovery(found: claim_grading::Recovery, version_two: bool) -> Value {
    let mut parts = vec![
        s(found.outcome),
        Value::Array(found.claim_ids),
        s(found.reason),
    ];
    if version_two {
        parts.push(Value::Bool(found.cause_only));
    }
    Value::Array(parts)
}

fn list(values: &Value) -> Vec<Value> {
    values.as_array().unwrap_or_default().to_vec()
}

fn strings(values: &Value) -> Vec<String> {
    list(values)
        .iter()
        .filter_map(|value| value.as_str().map(str::to_string))
        .collect()
}

fn problems(found: Vec<String>) -> Value {
    Value::Array(found.into_iter().map(Value::Str).collect())
}

/// What the Rust port returns for one call, in the shape the Python driver prints.
fn call(name: &str, args: &[Value], schema: &Value) -> Value {
    let truthy = |index: usize| args[index].truthy();
    let outcome = match name {
        "assessment_problems" => {
            claim_grading::assessment_problems(&args[0], &args[1]).map(problems)
        }
        "verdict_problems" => claim_grading::verdict_problems(&args[0], &args[1]).map(problems),
        "verdict_problems_v2" => {
            claim_grading::verdict_problems_v2(&args[0], &args[1]).map(problems)
        }
        "family_recovery" => claim_grading::family_recovery(&args[0], &list(&args[1]), truthy(2))
            .map(|found| recovery(found, false)),
        "family_recovery_v2" => {
            claim_grading::family_recovery_v2(&args[0], &list(&args[1]), truthy(2))
                .map(|found| recovery(found, true))
        }
        "family_sufficiency" => Ok(s(claim_grading::family_sufficiency(
            args[0].as_str().unwrap_or_default(),
            &list(&args[1]),
            truthy(2),
        ))),
        "item_fact" => {
            let answers = list(&args[0]);
            Ok(s(claim_grading::item_fact(
                &answers.iter().collect::<Vec<_>>(),
            )))
        }
        "credit_problems" => {
            let entries = list(&args[0]);
            claim_grading::credit_problems(&entries.iter().collect::<Vec<_>>(), &args[1])
                .map(problems)
        }
        "legacy_assignment" => claim_grading::legacy_assignment(&args[0]),
        "primary" => claim_grading::primary(&list(&args[0])),
        "claim_problems" => {
            let ids = strings(&args[1]);
            let ids: Vec<&str> = ids.iter().map(String::as_str).collect();
            Ok(problems(
                claim_grading::claim_problems(&args[0], &ids, args[2].as_str(), schema).unwrap(),
            ))
        }
        "mapping_problems" => {
            let ids = strings(&args[1]);
            let ids: Vec<&str> = ids.iter().map(String::as_str).collect();
            match claim_grading::mapping_problems(&args[0], &ids, schema) {
                Ok(found) => Ok(problems(found)),
                Err(claim_grading::MappingError::Raised(kind)) => Err(kind),
                Err(claim_grading::MappingError::Schema(error)) => panic!("{error:?}"),
            }
        }
        "pinned_matches_v2" => {
            grading_validation::pinned_matches_v2(&args[0], &args[1]).map(Value::Bool)
        }
        "inventory_problems" => {
            grading_validation::inventory_problems(&args[0], &args[1]).map(problems)
        }
        "pre_existing.verdict_problems" => {
            pre_existing::verdict_problems(&args[0], &args[1], &args[2]).map(problems)
        }
        "pre_existing.raising_problems" => {
            pre_existing::raising_problems(&args[0], &args[1]).map(problems)
        }
        "pre_existing.assessment" => pre_existing::assessment(&args[0], &list(&args[1]), truthy(2)),
        "pre_existing.saved_claim" => pre_existing::saved_claim(&args[0]),
        "pre_existing.credit_subject" => pre_existing::credit_subject(&args[0]),
        _ => panic!("unknown rule {name}"),
    };
    outcome.map_or_else(raised, ok)
}

/// The retained Python's result for each call: `{"ok": value}`, or `{"raised": name}` for the exception it raises.
const DRIVER: &str = r#"
import json, sys
sys.path.insert(0, sys.argv[1])
import claim_grading, grading_validation, pre_existing_grading
calls = json.loads(sys.stdin.read())
def run(name, args):
    if name in ("family_recovery", "family_recovery_v2"):
        return list(getattr(claim_grading, name)(*args))
    if name in ("item_fact",):
        return claim_grading.item_fact(args[0])
    if name in ("claim_problems", "mapping_problems"):
        return getattr(claim_grading, name)(args[0], set(args[1]), *args[2:])
    if name in ("pinned_matches_v2", "inventory_problems"):
        return getattr(grading_validation, name)(*args)
    if name.startswith("pre_existing."):
        return getattr(pre_existing_grading, name.split(".", 1)[1])(*args)
    return getattr(claim_grading, name)(*args)
results = []
for name, args in calls:
    try:
        results.append({"ok": run(name, args)})
    except Exception as error:
        results.append({"raised": type(error).__name__})
print(json.dumps(results))
"#;

fn python(calls: &[(String, Vec<Value>)]) -> Vec<Value> {
    let payload = Value::Array(
        calls
            .iter()
            .map(|(name, args)| Value::Array(vec![s(name), Value::Array(args.clone())]))
            .collect(),
    );
    let mut child = Command::new("python3")
        .arg("-c")
        .arg(DRIVER)
        .arg(repository().join("bench/tools"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(pyjson::dumps(&payload, &Dumps::default()).as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    list(&json(&String::from_utf8(output.stdout).unwrap()))
}

fn family(state: &str) -> Value {
    json(&format!(
        r#"{{"id": "GT-a", "eligibility": {{"state": "{state}"}}}}"#
    ))
}

const ELIGIBLE_CLAIM: &str = r#"{"id": "c1", "family_id": "GT-a", "outcome": "eligible"}"#;
const SAYS_WHAT: &str = r#"{"id": "c1", "known_problems": [{"family": "GT-a", "says_what": "yes", "identifies_cause": "no"}]}"#;

#[test]
fn pending_and_proposed_eligibility_keep_recovery_unresolved() {
    let claims = vec![json(ELIGIBLE_CLAIM)];
    for state in ["pending", "proposed"] {
        let found = claim_grading::family_recovery(&family(state), &claims, true).unwrap();
        assert_eq!(found.outcome, "unresolved", "{state}");
        assert_eq!(
            found.reason,
            "The family's eligibility awaits a saved human ruling."
        );
        let found =
            claim_grading::family_recovery_v2(&family(state), &[json(SAYS_WHAT)], true).unwrap();
        assert_eq!(found.outcome, "unresolved", "{state}");
        assert_eq!(
            found.reason,
            "The family's eligibility awaits a saved human ruling."
        );
    }
    let caught = claim_grading::family_recovery(&family("approved"), &claims, true).unwrap();
    assert_eq!(caught.outcome, "caught");
    let unadmitted =
        claim_grading::family_recovery_v2(&family("approved"), &[json(SAYS_WHAT)], false).unwrap();
    assert_eq!(unadmitted.outcome, "unresolved");
    assert_eq!(
        claim_grading::family_sufficiency("unresolved", &[s("sufficient")], true),
        "unassessed"
    );
}

#[test]
fn an_unresolved_canonical_pin_accepts_only_an_unresolved_claim() {
    let pin = json(r#"{"outcome": "unresolved", "family": "GT-a"}"#);
    let held = json(r#"{"outcome": "unresolved", "known_problems": []}"#);
    let resolved = json(
        r#"{"outcome": "problem", "known_problems": [{"family": "GT-a", "says_what": "yes"}]}"#,
    );
    assert!(grading_validation::pinned_matches_v2(&held, &pin).unwrap());
    assert!(!grading_validation::pinned_matches_v2(&resolved, &pin).unwrap());
}

#[test]
fn an_unadmitted_review_leaves_a_pre_existing_problem_unresolved() {
    let claims = vec![json(
        r#"{"id": "c1", "pre_existing_problems": [{"problem": "PE-1", "says_what": "yes", "identifies_cause": "no"}]}"#,
    )];
    let found = pre_existing::assessment(&json(r#"{"id": "PE-1"}"#), &claims, false).unwrap();
    assert!(pyobject::eq(
        &found,
        &json(
            r#"{"problem_id": "PE-1", "outcome": "unresolved", "claim_ids": ["c1"], "says_what": "yes", "identifies_cause": "no"}"#
        )
    ));
}

/// Replaces the value at one position of a document, counted depth first, with `junk`.
fn mutate(value: &Value, target: usize, junk: &Value, seen: &mut usize) -> Value {
    let here = *seen;
    *seen += 1;
    if here == target {
        return junk.clone();
    }
    match value {
        Value::Array(items) => Value::Array(
            items
                .iter()
                .map(|item| mutate(item, target, junk, seen))
                .collect(),
        ),
        Value::Object(members) => Value::Object(
            members
                .iter()
                .map(|(key, item)| (key.clone(), mutate(item, target, junk, seen)))
                .collect(),
        ),
        _ => value.clone(),
    }
}

fn size(value: &Value) -> usize {
    1 + match value {
        Value::Array(items) => items.iter().map(size).sum(),
        Value::Object(members) => members.iter().map(|(_, item)| size(item)).sum(),
        _ => 0,
    }
}

/// Typical arguments for each rule, which the differential varies one position at a time.
fn bases() -> Vec<(&'static str, Value)> {
    let v2_claim = r#"{"id": "c1", "quote": "q", "true": "yes", "this_change": null, "promised": null, "promise_source": [], "delivered": null, "outcome": "problem", "kind": null, "known_problems": [{"family": "GT-a", "says_what": "yes", "identifies_cause": "no", "reason": "r"}], "canonical_claim_id": null, "duplicate_group": null, "candidate": null, "open": null, "notes": "n", "evidence": ["e"]}"#;
    let v3_claim = v2_claim.replacen(
        "\"evidence\": [\"e\"]}",
        "\"evidence\": [\"e\"], \"pre_existing_problems\": [], \"pre_existing_candidate\": null}",
        1,
    );
    let outside = r#"{"id": "c2", "quote": "q", "true": "yes", "this_change": "no", "promised": null, "promise_source": [], "delivered": null, "outcome": "outside-this-change", "kind": null, "known_problems": [], "canonical_claim_id": null, "duplicate_group": null, "candidate": null, "open": null, "notes": "n", "evidence": ["e"], "pre_existing_problems": [{"problem": "PE-1", "says_what": "yes", "identifies_cause": "cannot-tell", "reason": "r"}], "pre_existing_candidate": null}"#;
    let graded = r#"{"id": "c1", "quote": "Races", "assignment": "defect:GT-t1", "canonical_claim_id": null, "duplicate_group": "g", "fix_sufficiency": "partial", "candidate": null, "notes": "n", "evidence": ["e"], "assessment": {"support": "supported", "attribution": "introduced", "reachability": "reachable", "materiality": "material"}}"#;
    let refuted = graded
        .replace("\"c1\"", "\"c2\"")
        .replace("defect:GT-t1", "refuted")
        .replace("partial", "n/a")
        .replace("\"supported\"", "\"contradicted\"");
    let mapping = format!(
        r#"{{"rubric_version": 2, "attempts": [{{"attempt_id": "att-1", "items": [{{"item_id": "item-0", "assignment": "defect:GT-t1", "duplicate_group": "g", "fix_sufficiency": "partial", "notes": "n", "claims": [{graded}, {refuted}]}}]}}]}}"#
    );
    vec![
        (
            "assessment_problems",
            json(
                r#"["eligible", {"support": "supported", "attribution": "introduced", "reachability": "reachable", "materiality": "material"}]"#,
            ),
        ),
        (
            "assessment_problems",
            json(
                r#"["advisory", {"support": "supported", "attribution": "pre-existing", "reachability": "unsettled", "materiality": "material"}]"#,
            ),
        ),
        (
            "verdict_problems",
            json(
                r#"[{"id": "c1", "quote": "q", "outcome": "eligible", "family": "GT-a", "canonical_claim_id": null, "duplicate_group": null, "candidate": null, "notes": "n", "evidence": ["e"], "assessment": {"support": "supported", "attribution": "introduced", "reachability": "reachable", "materiality": "material"}}, ["GT-a", "GT-b"]]"#,
            ),
        ),
        (
            "verdict_problems_v2",
            json(&format!("[{v2_claim}, [\"GT-a\"]]")),
        ),
        (
            "verdict_problems_v2",
            json(
                r#"[{"id": "c1", "quote": "q", "true": "yes", "this_change": "yes", "promised": "no", "promise_source": [], "delivered": null, "outcome": "unresolved", "kind": "relied-on", "known_problems": [], "canonical_claim_id": null, "duplicate_group": null, "candidate": "N-1", "open": {"kind": "relied-on", "would_settle": "w"}, "notes": "n", "evidence": ["e"]}, ["GT-a"]]"#,
            ),
        ),
        (
            "family_recovery",
            json(&format!(
                r#"[{{"id": "GT-a", "eligibility": {{"state": "approved"}}}}, [{ELIGIBLE_CLAIM}, {{"id": "c2", "family_id": "GT-a", "outcome": "unresolved"}}], true]"#
            )),
        ),
        (
            "family_recovery",
            json(
                r#"[{"id": "GT-a", "eligibility": {"state": "approved"}}, [{"id": "c2", "family_id": "GT-a", "outcome": "unresolved"}], true]"#,
            ),
        ),
        (
            "family_recovery_v2",
            json(
                r#"[{"id": "GT-a", "eligibility": {"state": "approved"}}, [{"id": "c1", "known_problems": [{"family": "GT-a", "says_what": "cannot-tell", "identifies_cause": "yes"}]}], true]"#,
            ),
        ),
        (
            "family_recovery_v2",
            json(&format!(
                r#"[{{"id": "GT-a", "eligibility": {{"state": "pending"}}}}, [{SAYS_WHAT}], true]"#
            )),
        ),
        (
            "family_sufficiency",
            json(r#"["caught", ["partial", "unassessed"], true]"#),
        ),
        ("family_sufficiency", json(r#"["caught", [], true]"#)),
        ("item_fact", json(r#"[["no", "cannot-tell"]]"#)),
        (
            "credit_problems",
            json(
                r#"[[{"says_what": "no", "identifies_cause": "yes"}], {"family": "GT-a", "says_what": "yes", "identifies_cause": "no"}]"#,
            ),
        ),
        ("legacy_assignment", json(r#"["unsupported"]"#)),
        ("primary", json(&format!("[[{refuted}, {graded}]]"))),
        (
            "claim_problems",
            json(&format!(
                "[[{graded}, {refuted}], [\"GT-t1\"], \"Races on close\"]"
            )),
        ),
        (
            "mapping_problems",
            json(&format!("[{mapping}, [\"GT-t1\"]]")),
        ),
        (
            "pinned_matches_v2",
            json(&format!(
                "[{v2_claim}, {{\"outcome\": \"eligible\", \"family\": \"GT-a\"}}]"
            )),
        ),
        (
            "pinned_matches_v2",
            json(&format!(
                "[{v2_claim}, {{\"outcome\": \"advisory\", \"family\": null}}]"
            )),
        ),
        (
            "inventory_problems",
            json(
                r#"[{"t": {"1": {"kind": "finding", "quotes": ["Races"], "ids": ["c1"]}, "2": {"kind": "not-a-finding", "quotes": []}}}, {"t": {"items": [{"segments": ["Races on close"], "proposed_fix": true}, {"segments": [], "proposed_fix": false}]}}]"#,
            ),
        ),
        (
            "pre_existing.verdict_problems",
            json(&format!("[{v3_claim}, [\"GT-a\"], [\"PE-1\"]]")),
        ),
        (
            "pre_existing.raising_problems",
            json(&format!("[{outside}, [\"PE-1\"]]")),
        ),
        (
            "pre_existing.assessment",
            json(&format!("[{{\"id\": \"PE-1\"}}, [{outside}], true]")),
        ),
        (
            "pre_existing.saved_claim",
            json(
                r#"[{"pre_existing_problems": [{"problem_id": "PE-1", "says_what": "yes", "identifies_cause": "no", "reason": "r"}], "pre_existing_candidate_id": null}]"#,
            ),
        ),
        (
            "pre_existing.credit_subject",
            json(r#"[{"family_id": "GT-a", "pre_existing_problem_id": "PE-1"}]"#),
        ),
    ]
}

/// Whether the arguments have the types the Rust signature fixes, which its callers always build.
fn typed(name: &str, args: &[Value]) -> bool {
    let is_list = |index: usize| matches!(args[index], Value::Array(_));
    let names = |index: usize| {
        args[index]
            .as_array()
            .is_some_and(|items| items.iter().all(|item| item.as_str().is_some()))
    };
    match name {
        "family_recovery" | "family_recovery_v2" | "pre_existing.assessment" => is_list(1),
        "family_sufficiency" => args[0].as_str().is_some() && is_list(1),
        "item_fact" | "credit_problems" | "primary" => is_list(0),
        "claim_problems" => names(1) && matches!(args[2], Value::Str(_) | Value::Null),
        "mapping_problems" => names(1),
        _ => true,
    }
}

#[test]
fn every_rule_matches_the_retained_python() {
    let schema = claim_grading::graded_claim_schema(&repository()).unwrap();
    let junk = [
        "null",
        "true",
        "0",
        "1",
        "2.5",
        "NaN",
        "\"\"",
        "\" \"",
        "\"x\"",
        "\"yes\"",
        "\"GT-a\"",
        "[]",
        "[1]",
        "[\"x\"]",
        "{}",
        "{\"id\": \"GT-a\"}",
    ]
    .map(json);
    let mut calls: Vec<(String, Vec<Value>)> = Vec::new();
    for (name, args) in bases() {
        calls.push((name.to_string(), list(&args)));
        for target in 1..size(&args) {
            for (index, value) in junk.iter().enumerate() {
                if (target + index) % 3 == 0 {
                    let mut seen = 0;
                    let mutated = list(&mutate(&args, target, value, &mut seen));
                    if typed(name, &mutated) {
                        calls.push((name.to_string(), mutated));
                    }
                }
            }
        }
    }
    let expected = python(&calls);
    assert_eq!(expected.len(), calls.len());
    let mut differences = Vec::new();
    let mut raising = 0;
    for ((name, args), wanted) in calls.iter().zip(&expected) {
        if wanted
            .as_object()
            .is_some_and(|result| result.get("raised").is_some())
        {
            raising += 1;
        }
        let actual = call(name, args, &schema);
        if !pyobject::eq(&actual, wanted) {
            differences.push(format!(
                "{name}{}: python {}, rust {}",
                Value::Array(args.clone()).py_repr(),
                wanted.py_repr(),
                actual.py_repr()
            ));
        }
    }
    assert!(differences.is_empty(), "{differences:#?}");
    assert!(
        calls.len() > 500 && raising > 50,
        "{} calls, {raising} raising",
        calls.len()
    );
}
