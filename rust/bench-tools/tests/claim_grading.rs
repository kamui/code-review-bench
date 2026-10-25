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

fn unresolved_claim(open: &str) -> Value {
    json(&format!(
        r#"{{"id": "c1", "quote": "q", "true": "cannot-check", "this_change": null, "promised": null,
        "promise_source": [], "delivered": null, "outcome": "unresolved", "kind": null, "known_problems": [],
        "canonical_claim_id": null, "duplicate_group": null, "candidate": null, "open": {open},
        "notes": "n", "evidence": ["e"]}}"#
    ))
}

#[test]
fn a_blocked_check_names_its_refused_command() {
    let families = json("[]");
    let problems = |open: &str| {
        claim_grading::verdict_problems_v2(&unresolved_claim(open), &families).unwrap()
    };
    assert_eq!(
        problems(
            r#"{"kind": "blocked-check", "command": "go run ./cmd/import-boss", "would_settle": "Whether any scheduler file imports the package."}"#
        ),
        Vec::<String>::new()
    );
    for open in [
        r#"{"kind": "blocked-check", "would_settle": "Whether any scheduler file imports the package."}"#,
        r#"{"kind": "blocked-check", "command": " ", "would_settle": "Whether any scheduler file imports the package."}"#,
        r#"{"kind": "blocked-check", "command": "go run ./cmd/import-boss", "would_settle": ""}"#,
    ] {
        assert_eq!(
            problems(open),
            ["a blocked check needs open kind, command and would_settle"],
            "{open}"
        );
    }
    assert_eq!(
        problems(
            r#"{"kind": "missing-fact", "command": "go run ./cmd/import-boss", "would_settle": "w"}"#
        ),
        ["unresolved needs open kind and would_settle"]
    );
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
