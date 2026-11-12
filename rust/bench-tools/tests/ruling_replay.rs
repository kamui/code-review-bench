//! `bench-tools ruling replay` counts what a version 3 policy's gates would have passed among decided records. These
//! cases use the closed round of the `ruling-route-settles-level-1` corpus case: fifteen credit decisions on four
//! pull requests, each with the recommender and two blind assessors agreeing and the owner deciding the same.

use bench_tools::pyjson::{self, Dumps, Duplicates, Value};
use bench_tools::pytext::PyPath;
use bench_tools::ruling::{Repo, replay, summary};
use bench_tools::sha256::hex_digest;
use std::path::{Path, PathBuf};

const AFTER: &str = "docs/research/earned-2026-01-01/rulings/case-3.after.json";
const POLICY: &str = "bench/grading/delegation.v3.json";

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn json(text: &str) -> Value {
    pyjson::loads(text, Duplicates::KeepLast).unwrap()
}

fn decode(text: &str) -> Vec<u8> {
    let digit = |byte: u8| match byte {
        b'A'..=b'Z' => byte - b'A',
        b'a'..=b'z' => byte - b'a' + 26,
        b'0'..=b'9' => byte - b'0' + 52,
        b'+' => 62,
        _ => 63,
    };
    let bytes: Vec<u8> = text.bytes().filter(|byte| *byte != b'=').collect();
    let mut out = Vec::new();
    for chunk in bytes.chunks(4) {
        let mut bits = 0u32;
        for (at, byte) in chunk.iter().enumerate() {
            bits |= u32::from(digit(*byte)) << (18 - 6 * at);
        }
        out.extend_from_slice(&bits.to_be_bytes()[1..chunk.len()]);
    }
    out
}

struct Root(PathBuf);

impl Drop for Root {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn root(name: &str) -> Root {
    let corpus = std::fs::read_to_string(
        repository().join("tools/behavior_contracts/ruling-record.v1.json"),
    )
    .unwrap();
    let corpus = json(&corpus);
    let case = corpus
        .as_object()
        .unwrap()
        .get("cases")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .find(|case| {
            case.as_object().unwrap().get("id").unwrap().as_str()
                == Some("ruling-route-settles-level-1")
        })
        .unwrap();
    let path = std::env::temp_dir().join(format!("ruling-replay-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&path);
    for entry in case
        .as_object()
        .unwrap()
        .get("initial")
        .unwrap()
        .as_array()
        .unwrap()
    {
        let entry = entry.as_object().unwrap();
        let name =
            String::from_utf8(decode(entry.get("path_b64").unwrap().as_str().unwrap())).unwrap();
        let file = path.join(name);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(
            file,
            decode(entry.get("bytes_b64").unwrap().as_str().unwrap()),
        )
        .unwrap();
    }
    Root(path)
}

/// A version 3 policy that keeps every decision type with the owner except credit, which takes `recovery`.
fn policy(recovery: &str, always_ask: &str, percent: u64) -> String {
    format!(
        r#"{{"version": 3, "status": "proposed", "receipt": null, "written": "before-question",
            "always_ask": {always_ask},
            "entry": {{"decisions": 15, "pull_requests": 4, "agreement_percent": {percent}}},
            "decision_types": {{"pre-existing-admission": "owner", "pre-existing-raise": "owner",
                "pre-existing-grouping": "owner", "candidate": "owner", "recovery": {recovery},
                "grouping": "owner", "band": "owner", "control": "owner", "reconciliation": "owner"}}}}"#
    )
}

const ALWAYS_ASK: &str =
    r#"["cannot-tell", "gap", "conflict", "relied-on", "problem", "reviews-saved-ruling"]"#;
const AGREED: &str = r#"{"blind_answers": 2, "recommender_confidence": "any"}"#;

/// The replay of `text` as a policy over the root's records, or the refusal.
fn replay_of(root: &Path, text: &str) -> Result<replay::Replay, String> {
    std::fs::write(root.join(POLICY), text).unwrap();
    let repo = Repo::new(root.as_os_str().as_encoded_bytes());
    let source = PyPath::new(root.join(POLICY).as_os_str().as_encoded_bytes());
    let policy = replay::policy(&repo, &source).map_err(|raised| raised.traceback())?;
    let pairs = summary::records(&repo).unwrap();
    Ok(replay::replay(&repo, &pairs, policy, POLICY).unwrap())
}

/// The replay of `text` as a policy over the root's records, as `--json` prints it, or the refusal.
fn replayed(root: &Path, text: &str) -> Result<Value, String> {
    replay_of(root, text).map(|replay| replay.to_value())
}

fn row(replayed: &Value, kind: &str) -> String {
    let types = replayed.as_object().unwrap().get("decision_types").unwrap();
    pyjson::dumps(
        types.as_object().unwrap().get(kind).unwrap(),
        &Dumps::default(),
    )
}

fn credit(replayed: &Value) -> String {
    row(replayed, "recovery")
}

fn rulings(root: &Path, n: u32) -> (PathBuf, PathBuf) {
    let directory = root.join("docs/research/earned-2026-01-01/rulings");
    (
        directory.join(format!("case-{n}.before.json")),
        directory.join(format!("case-{n}.after.json")),
    )
}

/// Rewrite one pair, with the after-record pinning the before-record as edited.
fn rewrite(root: &Path, n: u32, before: impl Fn(&str) -> String, after: impl Fn(&str) -> String) {
    let (before_path, after_path) = rulings(root, n);
    let old = std::fs::read(&before_path).unwrap();
    let edited = before(std::str::from_utf8(&old).unwrap());
    let pinned = std::fs::read_to_string(&after_path)
        .unwrap()
        .replace(&hex_digest(&old), &hex_digest(edited.as_bytes()));
    std::fs::write(&before_path, edited).unwrap();
    std::fs::write(&after_path, after(&pinned)).unwrap();
}

fn keep(text: &str) -> String {
    text.to_string()
}

const GAP_FREE: &str = r#"["cannot-tell", "relied-on", "problem", "reviews-saved-ruling"]"#;
const FACTS: &str = r#", "facts": {"says_what": "yes", "identifies_cause": "yes"}"#;
const SETTLED: &str = r#""confidence": "high", "short_of_high": [], "would_settle": true"#;
const MISS: &str =
    r#""miss": {"cause": "slip", "note": "The agreed answer was not the decision."}"#;

/// The recommender's answer only, with the decision type and outcome of a band or reconciliation record.
fn without_blind(text: &str, kind: &str, outcome: &str, keep_assessors: usize) -> String {
    let cut = match keep_assessors {
        0 => text.find(r#", {"by": "assessor-1""#),
        _ => text.find(r#", {"by": "assessor-2""#),
    }
    .unwrap();
    format!("{}]}}", &text[..cut])
        .replace(
            r#""decision_type": "recovery""#,
            &format!(r#""decision_type": "{kind}""#),
        )
        .replace(
            r#""outcome": "recovers""#,
            &format!(r#""outcome": "{outcome}""#),
        )
        .replace(FACTS, "")
}

fn without_facts(text: &str, outcome: &str, miss: bool) -> String {
    let text = text.replace(FACTS, "").replace(
        r#""outcome": "recovers""#,
        &format!(r#""outcome": "{outcome}""#),
    );
    match miss {
        true => text.replace(r#""miss": null"#, MISS),
        false => text,
    }
}

fn medium(text: &str) -> String {
    text.replace(
        SETTLED,
        r#""confidence": "medium", "short_of_high": ["no-precedent"], "would_settle": false"#,
    )
}

fn low(text: &str) -> String {
    text.replace(
        SETTLED,
        r#""confidence": "low", "short_of_high": ["no-precedent", "flip-fact-open"], "would_settle": false"#,
    )
}

/// Apply `change` to the recommender's answer only.
fn recommender(text: &str, change: impl Fn(&str) -> String) -> String {
    let cut = text.find(r#"{"by": "assessor-1""#).unwrap();
    format!("{}{}", change(&text[..cut]), &text[cut..])
}

#[test]
fn agreed_answers_the_owner_kept_pass_the_gate_and_meet_the_entry_bar() {
    let root = root("kept");
    let replayed = replayed(&root.0, &policy(AGREED, ALWAYS_ASK, 90)).unwrap();
    assert_eq!(
        credit(&replayed),
        concat!(
            r#"{"gate": "the recommender and 2 blind answers agree", "decisions": 15, "never_needed": 0, "#,
            r#""settled_by_agents_unread": 0, "parties_agreed": 15, "held": [], "passed": 15, "stands": 15, "#,
            r#""pull_requests": ["t-1", "t-2", "t-3", "t-4"], "entry_bar_met": true, "#,
            r#""outcomes": [{"outcome": "recovers", "passed": 15, "stands": 15}], "differences": []}"#
        )
    );
}

#[test]
fn a_decision_the_owner_changed_is_listed_and_can_miss_the_entry_bar() {
    let root = root("changed");
    let after = std::fs::read_to_string(root.0.join(AFTER)).unwrap();
    let changed = after
        .replace(
            r#""outcome": "recovers""#,
            r#""outcome": "does-not-recover""#,
        )
        .replace(
            r#""miss": null"#,
            r#""miss": {"cause": "slip", "note": "The agreed answer was not the decision."}"#,
        )
        .replace(
            r#""facts": {"says_what": "yes", "identifies_cause": "yes"}"#,
            r#""facts": {"says_what": "no", "identifies_cause": "no"}"#,
        );
    assert_ne!(after, changed);
    std::fs::write(root.0.join(AFTER), changed).unwrap();

    let credit = credit(&replayed(&root.0, &policy(AGREED, ALWAYS_ASK, 95)).unwrap());
    assert!(
        credit.contains(r#""passed": 15, "stands": 14, "#)
            && credit.contains(r#""entry_bar_met": false"#),
        "{credit}"
    );
    assert!(
        credit.contains(concat!(
            r#""differences": [{"path": "docs/research/earned-2026-01-01/rulings/case-3.before.json", "#,
            r#""agreed": "recovers (says what goes wrong: yes, identifies the cause as a fault: yes)", "#,
            r#""decided": ["does-not-recover (says what goes wrong: no, identifies the cause as a fault: no)"]}]"#
        )),
        "{credit}"
    );
}

#[test]
fn an_owner_decision_type_passes_nothing() {
    let root = root("owner");
    let replayed = replayed(&root.0, &policy(r#""owner""#, ALWAYS_ASK, 90)).unwrap();
    assert_eq!(credit(&replayed), r#"{"gate": "owner", "decisions": 15}"#);
}

#[test]
fn a_policy_the_replay_would_not_apply_as_written_is_refused() {
    let root = root("refused");
    let refusal = |text: &str| replayed(&root.0, text).unwrap_err();
    assert_eq!(
        refusal(&policy(AGREED, ALWAYS_ASK, 90).replace(r#""version": 3"#, r#""version": 2"#)),
        "ValueError: unsupported replay policy: the replay reads version 3"
    );
    assert_eq!(
        refusal(&policy(
            r#"{"blind_answers": 1, "recommender_confidence": "any"}"#,
            ALWAYS_ASK,
            90
        )),
        "ValueError: unsupported replay policy: `recovery` must name the 2 blind answers its records hold"
    );
    assert_eq!(
        refusal(&policy(
            r#"{"blind_answers": 2, "recommender_confidence": "any", "outcomes": ["recovers"]}"#,
            ALWAYS_ASK,
            90
        )),
        "ValueError: unsupported replay policy: `recovery` has a gate field the replay does not apply"
    );
    assert_eq!(
        refusal(&policy(AGREED, r#"["cannot-tell"]"#, 90)),
        "ValueError: unsupported replay policy: `always_ask` must be policy v2's list in its order, with or without `gap` and `conflict`"
    );
    assert_eq!(
        refusal(&policy(AGREED, ALWAYS_ASK, 90).replace(r#""proposed""#, r#""draft""#)),
        "ValueError: unsupported replay policy: `status` must be \"proposed\" or \"adopted\""
    );
    assert!(
        replayed(
            &root.0,
            &policy(
                AGREED,
                r#"["cannot-tell", "relied-on", "problem", "reviews-saved-ruling"]"#,
                90
            )
        )
        .is_ok()
    );
}

/// A stated rule gap in any party's answer holds an otherwise agreed decision only while `always_ask` keeps `gap`
/// and `conflict`.
#[test]
fn a_stated_gap_holds_an_agreed_answer_only_when_the_policy_keeps_gap_and_conflict() {
    let root = root("gap");
    rewrite(
        &root.0,
        1,
        |text| {
            let cut = text.find(r#"{"by": "assessor-2""#).unwrap();
            format!(
                "{}{}",
                &text[..cut],
                text[cut..].replace(
                    &format!(r#""rule_gap": null, {SETTLED}"#),
                    r#""rule_gap": "The rule does not say.", "confidence": "medium", "short_of_high": ["gap"], "would_settle": false"#
                )
            )
        },
        keep,
    );

    let kept = credit(&replayed(&root.0, &policy(AGREED, ALWAYS_ASK, 90)).unwrap());
    assert!(
        kept.contains(r#""parties_agreed": 15, "held": [{"reason": "gap or conflict in the rule", "decisions": 1}], "passed": 14,"#),
        "{kept}"
    );

    let through = credit(&replayed(&root.0, &policy(AGREED, GAP_FREE, 90)).unwrap());
    assert!(
        through.contains(r#""parties_agreed": 15, "held": [], "passed": 15,"#),
        "{through}"
    );
}

/// A recommender below medium confidence does not make an agreed answer under `medium-or-high`.
#[test]
fn a_low_confidence_recommender_does_not_count_as_agreed_under_medium_or_high() {
    let root = root("confidence");
    rewrite(&root.0, 1, |text| recommender(text, low), keep);
    let gate = r#"{"blind_answers": 2, "recommender_confidence": "medium-or-high"}"#;

    let strict = credit(&replayed(&root.0, &policy(gate, ALWAYS_ASK, 90)).unwrap());
    assert!(strict.contains(r#""parties_agreed": 14,"#), "{strict}");

    let any = credit(&replayed(&root.0, &policy(AGREED, ALWAYS_ASK, 90)).unwrap());
    assert!(any.contains(r#""parties_agreed": 15,"#), "{any}");
}

/// After admission, a label the owner found not applicable was never needed and is counted on neither side.
#[test]
fn a_band_gate_asked_after_admission_counts_a_not_applicable_label_as_never_needed() {
    let root = root("band");
    rewrite(
        &root.0,
        1,
        |text| without_blind(text, "band", "serious", 1),
        |text| without_facts(text, "not-applicable", true),
    );
    rewrite(
        &root.0,
        2,
        |text| without_blind(text, "band", "serious", 1),
        |text| without_facts(text, "serious", false),
    );
    let policy = policy(AGREED, ALWAYS_ASK, 90).replace(
        r#""band": "owner""#,
        r#""band": {"blind_answers": 1, "recommender_confidence": "any", "asked": "after-admission"}"#,
    );

    let band = row(&replayed(&root.0, &policy).unwrap(), "band");
    assert_eq!(
        band,
        concat!(
            r#"{"gate": "the recommender and 1 blind answer agree, asked after admission", "decisions": 2, "#,
            r#""never_needed": 1, "settled_by_agents_unread": 0, "parties_agreed": 1, "held": [], "passed": 1, "#,
            r#""stands": 1, "pull_requests": ["t-2"], "entry_bar_met": false, "#,
            r#""outcomes": [{"outcome": "serious", "passed": 1, "stands": 1}], "differences": []}"#
        )
    );
}

/// A reconciliation record holds the recommender's answer alone, so its gate names no blind answer.
#[test]
fn a_reconciliation_gate_with_no_blind_answers_passes_a_medium_or_high_call_only() {
    let root = root("reconciliation");
    for (n, confidence) in [(1, medium as fn(&str) -> String), (2, keep), (3, low)] {
        rewrite(
            &root.0,
            n,
            |text| confidence(&without_blind(text, "reconciliation", "first-error", 0)),
            |text| without_facts(text, "first-error", false),
        );
    }
    let policy = policy(AGREED, ALWAYS_ASK, 90).replace(
        r#""reconciliation": "owner""#,
        r#""reconciliation": {"blind_answers": 0, "recommender_confidence": "medium-or-high"}"#,
    );

    let reconciliation = row(&replayed(&root.0, &policy).unwrap(), "reconciliation");
    assert!(
        reconciliation.contains(r#""gate": "the recommender's answer, recommender at medium or high confidence", "decisions": 3,"#)
            && reconciliation.contains(r#""parties_agreed": 2, "held": [], "passed": 2, "stands": 2,"#),
        "{reconciliation}"
    );
}

#[test]
fn the_shipped_proposed_policy_parses() {
    let root = repository();
    let repo = Repo::new(root.as_os_str().as_encoded_bytes());
    let source = PyPath::new(root.join(POLICY).as_os_str().as_encoded_bytes());
    assert!(replay::policy(&repo, &source).is_ok());
}

#[test]
fn the_rendered_replay_has_each_type_row_and_the_closing_sentence() {
    let root = root("render");
    let text = replay_of(&root.0, &policy(AGREED, ALWAYS_ASK, 90))
        .unwrap()
        .render();
    assert!(
        text.contains("| recovery | the recommender and 2 blind answers agree | 15 | 0 | 15 | 15 | 15 | 4 | met |"),
        "{text}"
    );
    assert!(
        text.ends_with("The replay settles nothing and grants no authority."),
        "{text}"
    );
}
