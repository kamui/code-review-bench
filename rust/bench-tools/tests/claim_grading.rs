//! Hand-stated outcomes of `bench_tools::claim_grading`, `pre_existing` and `grading_validation` for pending,
//! proposed and approved eligibility.

use bench_tools::pyjson::{self, Duplicates, Value};
use bench_tools::pyobject;
use bench_tools::{claim_grading, grading_validation, pre_existing};

fn json(text: &str) -> Value {
    pyjson::loads(text, Duplicates::KeepLast).unwrap()
}

fn s(text: &str) -> Value {
    Value::Str(text.to_string())
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
