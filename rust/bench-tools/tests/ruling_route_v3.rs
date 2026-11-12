//! A round whose opening pins policy v3 routes, settles and files on agreement (ADR-0008); a round whose opening
//! pins no policy routes under policy v2 as before. These cases take the `ruling-route-settles-level-1` corpus case:
//! a closed round of fifteen agreed credit decisions on four pull requests that the owner decided the same, and an
//! open round with one agreed decision. Most cases turn every record into a candidate decision on `refuted`.

use bench_tools::delegation;
use bench_tools::pyjson::{self, Dumps, Duplicates, Object, Value};
use bench_tools::pytext::PyPath;
use bench_tools::ruling::{Repo, record, summary};
use bench_tools::sha256::hex_digest;
use std::path::{Path, PathBuf};
use std::process::Command;

const EARNED: &str = "docs/research/earned-2026-01-01/rulings";
const CURRENT: &str = "docs/research/current-2026-02-01/rulings";
const BEFORE: &str = "docs/research/current-2026-02-01/rulings/case-1.before.json";
const AFTER: &str = "docs/research/current-2026-02-01/rulings/case-1.after.json";
const V3: &str = "bench/grading/delegation.v3.json";
const RECEIPT: &str = "bench/grading/rulings/delegation.v3.md";
const SCOPE: &str = "Adopt policy v3 as written";
const DOSSIER: &str = "docs/research/shared/dossier";
const ALWAYS_ASK: &str =
    r#"["cannot-tell", "gap", "conflict", "relied-on", "problem", "reviews-saved-ruling"]"#;
const SETTLE: &str = "SETTLE under policy v3, on agreement\n";

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn json(text: &str) -> Value {
    pyjson::loads(text, Duplicates::KeepLast).unwrap()
}

fn dumps(value: &Value) -> String {
    pyjson::dumps(value, &Dumps::default())
}

fn text(value: &str) -> Value {
    Value::Str(value.to_string())
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

impl Root {
    fn path(&self) -> &Path {
        &self.0
    }

    fn read(&self, path: &str) -> String {
        std::fs::read_to_string(self.0.join(path)).unwrap()
    }

    fn write(&self, path: &str, contents: &str) {
        let file = self.0.join(path);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(file, contents).unwrap();
    }

    fn pin(&self, path: &str) -> Value {
        let digest = hex_digest(&std::fs::read(self.0.join(path)).unwrap());
        Value::Object(
            [
                ("path".to_string(), text(path)),
                ("sha256".to_string(), text(&digest)),
            ]
            .into_iter()
            .collect(),
        )
    }

    fn edit(&self, path: &str, change: impl FnOnce(&mut Object)) {
        let mut value = json(&self.read(path));
        let Value::Object(object) = &mut value else {
            panic!("{path} is not an object");
        };
        change(object);
        self.write(path, &dumps(&value));
    }

    fn repo(&self) -> Repo {
        Repo::new(self.0.as_os_str().as_encoded_bytes())
    }

    fn py_path(&self, path: &str) -> PyPath {
        PyPath::new(self.0.join(path).as_os_str().as_encoded_bytes())
    }

    /// `bench-tools ruling route BEFORE ARGS...` run from the root.
    fn route(&self, before: &str, arguments: &[&str]) -> String {
        let output = Command::new(env!("CARGO_BIN_EXE_bench-tools"))
            .args(["ruling", "route", before])
            .args(arguments)
            .current_dir(&self.0)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    }
}

fn corpus(name: &str) -> Root {
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
    let path = std::env::temp_dir().join(format!("ruling-route-v3-{name}-{}", std::process::id()));
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

fn without(object: &Object, key: &str) -> Object {
    object
        .iter()
        .filter(|(name, _)| *name != key)
        .map(|(name, value)| (name.clone(), value.clone()))
        .collect()
}

fn answers(object: &mut Object) -> impl Iterator<Item = &mut Object> {
    let Some(Value::Array(answers)) = object.get_mut("answers") else {
        panic!("no answers");
    };
    answers.iter_mut().map(|answer| match answer {
        Value::Object(answer) => answer,
        _ => panic!("an answer is not an object"),
    })
}

/// A before-record or after-record of a credit decision as one of `kind` on `outcome`, with no credit facts.
fn convert(object: &mut Object, kind: &str, outcome: &str) {
    if object.get("decision_type").is_some() {
        object.insert("decision_type".into(), text(kind));
        object.insert("dossier".into(), text(DOSSIER));
        for answer in answers(object) {
            answer.insert("outcome".into(), text(outcome));
            *answer = without(answer, "facts");
        }
    } else {
        object.insert("outcome".into(), text(outcome));
        *object = without(object, "facts");
    }
}

/// The corpus case with every record turned into one of `kind` on `outcome`, the receipt policy v3 would pin, and
/// a candidate dossier that names the records' group.
fn fixture(name: &str, kind: &str, outcome: &str) -> Root {
    let root = corpus(name);
    root.write(
        &format!("{DOSSIER}/summary.json"),
        r#"[{"kind": "candidate", "group": "G1", "recommendation": "refuted"}]"#,
    );
    root.write(
        RECEIPT,
        &format!("# Policy v3 adoption\n\nThe owner answered: {SCOPE}.\n"),
    );
    for n in 1..=15 {
        let before = format!("{EARNED}/case-{n}.before.json");
        root.edit(&before, |object| convert(object, kind, outcome));
        root.edit(&format!("{EARNED}/case-{n}.after.json"), |object| {
            convert(object, kind, outcome);
            object.insert("before".into(), root.pin(&before));
        });
    }
    root.edit(BEFORE, |object| convert(object, kind, outcome));
    root
}

/// The shipped policy v3 file, adopted with the receipt at its canonical path when `adopted`, and with `change`
/// applied.
fn policy_text(root: &Root, adopted: bool, change: impl FnOnce(&mut Object)) -> String {
    let mut value = json(&std::fs::read_to_string(repository().join(V3)).unwrap());
    let Value::Object(object) = &mut value else {
        panic!("the shipped policy is not an object");
    };
    if adopted {
        object.insert("status".into(), text("adopted"));
        object.insert("receipt".into(), root.pin(RECEIPT));
        object.insert("adoption_scope".into(), text(SCOPE));
    }
    change(object);
    dumps(&value)
}

fn adopted(root: &Root) {
    root.write(V3, &policy_text(root, true, |_| {}));
}

/// Save the opening of round `name` in `directory` with `policy` pinned and `closed` as its closed rounds, and move
/// every pin of the round to it: the round file, each before-record's `round_opening` and each after-record's
/// `before`.
fn open_round(root: &Root, directory: &str, name: &str, policy: &str, closed: &[&str]) {
    let closed: Vec<String> = closed.iter().map(|path| dumps(&root.pin(path))).collect();
    let opening = format!("{directory}/opening.json");
    root.write(
        &opening,
        &format!(
            r#"{{"round": "{name}", "source_commit": "{}", "closed_rounds": [{}], "policy": {}}}"#,
            "a".repeat(40),
            closed.join(", "),
            dumps(&root.pin(policy))
        ),
    );
    let pinned = root.pin(&opening);
    let rulings = format!("{directory}/rulings");
    root.edit(&format!("{rulings}/round.json"), |object| {
        object.insert("opening".into(), pinned.clone());
    });
    let mut names: Vec<String> = std::fs::read_dir(root.path().join(&rulings))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    for name in names.iter().filter(|name| name.ends_with(".before.json")) {
        root.edit(&format!("{rulings}/{name}"), |object| {
            object.insert("round_opening".into(), pinned.clone());
        });
    }
    for name in names.iter().filter(|name| name.ends_with(".after.json")) {
        let before = format!("{rulings}/{}", name.replace(".after.json", ".before.json"));
        root.edit(&format!("{rulings}/{name}"), |object| {
            object.insert("before".into(), root.pin(&before));
        });
    }
}

const ROUND: &str = "docs/research/current-2026-02-01";
const MIDDLE: &str = "docs/research/middle-2026-01-15";

/// Open the current round under `policy`, after the earned round and every other closed round in `closed`.
fn open_under(root: &Root, policy: &str, closed: &[&str]) {
    let earned = format!("{EARNED}/round.json");
    let mut all = vec![earned.as_str()];
    all.extend_from_slice(closed);
    open_round(root, ROUND, "current", policy, &all);
}

fn open(root: &Root) {
    open_under(root, V3, &[]);
}

/// Edit the earned round's before-record `n` and pin its after-record to it.
fn edit_earned(root: &Root, n: u32, change: impl FnOnce(&mut Object)) {
    let before = format!("{EARNED}/case-{n}.before.json");
    root.edit(&before, change);
    root.edit(&format!("{EARNED}/case-{n}.after.json"), |object| {
        object.insert("before".into(), root.pin(&before));
    });
}

fn disagree(object: &mut Object) {
    if let Some(answer) = answers(object).last() {
        answer.insert("outcome".into(), text("minor-defect"));
    }
}

/// An after-record; `open_round` pins its before-record.
fn after(ruling: &str, settled_by: &str, policy: Value, asked: u64, outcome: &str) -> String {
    let miss = if settled_by == "owner" && outcome != "refuted" {
        r#"{"cause": "owner-weighs-differently", "note": "The owner weighed it differently."}"#
    } else {
        "null"
    };
    format!(
        r#"{{"contract": 2, "ruling": "{ruling}", "before": null, "settled_by": "{settled_by}", "policy": {}, "asked": {asked}, "outcome": "{outcome}", "by_default": false, "ground": null, "miss": {miss}, "lesson": null}}"#,
        dumps(&policy)
    )
}

/// The reasons an ASK lists, in order.
fn reasons(routed: &str) -> Vec<&str> {
    assert!(routed.starts_with("ASK\n"), "{routed}");
    routed
        .lines()
        .skip(1)
        .take_while(|line| !line.is_empty())
        .map(|line| line.strip_prefix("- ").unwrap())
        .collect()
}

fn candidates(name: &str) -> Root {
    let root = fixture(name, "candidate", "refuted");
    adopted(&root);
    open(&root);
    root
}

fn bar(passed: u64, pull_requests: u64, percent: u64) -> String {
    format!(
        "candidate has not met policy v3's entry bar in the rounds closed at opening: {passed} decisions passed the gate (needs 15), on {pull_requests} pull requests (needs 4), and {percent}% of them stand (needs 90%)"
    )
}

const SUSPENDED: &str = "a review in this round changed an agent settlement of this decision type, so policy v3 suspends it for the rest of the round";
const CHANGED_RULE: &str = "the round opening's policy v3 states terms other than those this tool enforces; a changed rule needs a new version";
const NOT_ADOPTED: &str = "policy v3 is not adopted, so agents settle nothing beyond ADR-0006";

fn index(root: &Root, ruling: &str) {
    root.edit("docs/research/shared/rulings.index.json", |object| {
        if let Some(Value::Object(rulings)) = object.get_mut("rulings") {
            rulings.insert(
                ruling.into(),
                json(r#"{"reading": "current", "aliases": []}"#),
            );
        }
    });
}

/// The current round's before-record as a review of `reviewed`, named `ruling`.
fn review_of(root: &Root, ruling: &str, reviewed: &str) -> String {
    let mut review = json(&root.read(BEFORE));
    if let Value::Object(object) = &mut review {
        object.insert("ruling".into(), text(ruling));
        object.insert(
            "reviews".into(),
            json(&format!(
                r#"{{"ruling": "{reviewed}", "saved_outcome": "refuted"}}"#
            )),
        );
    }
    dumps(&review)
}

fn renamed(root: &Root, ruling: &str) -> String {
    let mut record = json(&root.read(BEFORE));
    if let Value::Object(object) = &mut record {
        object.insert("ruling".into(), text(ruling));
    }
    dumps(&record)
}

#[test]
fn the_shipped_policy_v3_states_the_enforced_terms() {
    let root = repository();
    let repo = Repo::new(root.as_os_str().as_encoded_bytes());
    let source = PyPath::new(root.join(V3).as_os_str().as_encoded_bytes());
    assert!(delegation::v3::enforced_terms(&repo, &source).unwrap());
}

#[test]
fn a_round_whose_opening_pins_no_policy_routes_under_policy_v2_beside_a_v3_file() {
    let root = corpus("v2");
    assert_eq!(root.route(BEFORE, &[]), "SETTLE level 1\n");
    root.write(V3, &std::fs::read_to_string(repository().join(V3)).unwrap());
    assert_eq!(root.route(BEFORE, &[]), "SETTLE level 1\n");
    root.write(
        RECEIPT,
        &format!("# Policy v3 adoption\n\nThe owner answered: {SCOPE}.\n"),
    );
    adopted(&root);
    assert_eq!(root.route(BEFORE, &[]), "SETTLE level 1\n");
    assert_eq!(
        root.route(BEFORE, &["--policy", "bench/grading/delegation.v2.json"]),
        "SETTLE level 1\n"
    );
}

#[test]
fn an_agreed_candidate_settles_under_an_adopted_policy_v3() {
    let root = candidates("settles");
    assert_eq!(root.route(BEFORE, &[]), SETTLE);
}

#[test]
fn an_outcome_reserved_to_the_owner_asks() {
    let root = candidates("reserved");
    root.edit(BEFORE, |object| {
        for answer in answers(object) {
            answer.insert("outcome".into(), text("problem"));
        }
    });
    open(&root);
    assert_eq!(
        reasons(&root.route(BEFORE, &[])),
        [
            "recommender: problem stays with the owner",
            "assessor-1: problem stays with the owner",
            "assessor-2: problem stays with the owner",
        ]
    );
}

#[test]
fn parties_who_disagree_ask() {
    let root = candidates("disagree");
    root.edit(BEFORE, disagree);
    open(&root);
    assert_eq!(
        reasons(&root.route(BEFORE, &[])),
        [
            "the parties did not give the agreement policy v3 needs: the recommender and 2 blind answers agree"
        ]
    );
}

#[test]
fn a_recovery_decision_stays_with_the_owner_under_policy_v3() {
    let root = corpus("recovery");
    root.write(
        RECEIPT,
        &format!("# Policy v3 adoption\n\nThe owner answered: {SCOPE}.\n"),
    );
    adopted(&root);
    open(&root);
    assert_eq!(
        reasons(&root.route(BEFORE, &[])),
        ["recovery stays with the owner under policy v3"]
    );
}

#[test]
fn a_type_short_of_the_entry_bar_in_the_closed_rounds_asks() {
    let root = candidates("entry-passed");
    edit_earned(&root, 1, disagree);
    open(&root);
    assert_eq!(reasons(&root.route(BEFORE, &[])), [bar(14, 4, 100)]);

    let root = candidates("entry-pull-requests");
    for n in [4, 8, 12] {
        edit_earned(&root, n, |object| {
            object.insert("target".into(), text("t-1"));
        });
    }
    open(&root);
    assert_eq!(reasons(&root.route(BEFORE, &[])), [bar(15, 3, 100)]);

    let root = candidates("entry-rate");
    for n in [3, 4] {
        root.edit(&format!("{EARNED}/case-{n}.after.json"), |object| {
            object.insert("outcome".into(), text("minor-defect"));
            object.insert(
                "miss".into(),
                json(r#"{"cause": "slip", "note": "The agreed answer was not the decision."}"#),
            );
        });
    }
    open(&root);
    assert_eq!(reasons(&root.route(BEFORE, &[])), [bar(15, 4, 86)]);

    let root = candidates("entry-none");
    for n in 1..=15 {
        edit_earned(&root, n, disagree);
    }
    open(&root);
    assert_eq!(reasons(&root.route(BEFORE, &[])), [bar(0, 0, 0)]);
}

/// An agent settlement in a closed round that no owner record reviewed counts on neither side of the entry bar.
#[test]
fn an_unread_agent_settlement_does_not_count_toward_the_entry_bar() {
    let root = candidates("unread");
    let repo = root.repo();
    let mut pairs = summary::records(&repo).unwrap();
    let at = pairs
        .iter()
        .position(|pair| pair.path == format!("{EARNED}/case-15.before.json"))
        .unwrap();
    if let Value::Object(settled) = &mut pairs[at].after {
        settled.insert("settled_by".into(), text("agents"));
    }
    let before = record::read_before(&repo, &root.py_path(BEFORE)).unwrap();
    let routed = delegation::route_under(
        &repo,
        &before,
        None,
        Some(&pairs),
        Some(&root.py_path(BEFORE)),
    )
    .unwrap();
    assert_eq!(routed.reasons, [bar(14, 4, 100)]);
}

/// The owner reviews an agent settlement earlier in the round and decides otherwise; the next decision of the type
/// asks.
#[test]
fn an_owner_change_to_an_agent_settlement_suspends_the_type_for_the_round() {
    let root = candidates("suspended");
    index(&root, "now-1");
    root.edit(BEFORE, |object| {
        object.insert("ruling".into(), text("now-1"));
    });
    root.write(
        &format!("{CURRENT}/case-2.before.json"),
        &review_of(&root, "now-2", "now-1"),
    );
    root.write(
        &format!("{CURRENT}/case-3.before.json"),
        &renamed(&root, "now-3"),
    );
    root.write(AFTER, &after("now-1", "agents", root.pin(V3), 0, "refuted"));
    root.write(
        &format!("{CURRENT}/case-2.after.json"),
        &after("now-2", "owner", Value::Null, 1, "minor-defect"),
    );
    open(&root);
    assert_eq!(
        reasons(&root.route(&format!("{CURRENT}/case-3.before.json"), &[])),
        [SUSPENDED]
    );
}

/// The owner changes an agent settlement and a later review in the round restores its outcome; the type stays
/// suspended.
#[test]
fn an_owner_review_restoring_a_changed_agent_settlement_keeps_the_type_suspended() {
    let root = candidates("suspended-restored");
    index(&root, "now-1");
    index(&root, "now-2");
    root.edit(BEFORE, |object| {
        object.insert("ruling".into(), text("now-1"));
    });
    root.write(
        &format!("{CURRENT}/case-2.before.json"),
        &review_of(&root, "now-2", "now-1"),
    );
    root.write(
        &format!("{CURRENT}/case-3.before.json"),
        &review_of(&root, "now-3", "now-2"),
    );
    root.write(
        &format!("{CURRENT}/case-4.before.json"),
        &renamed(&root, "now-4"),
    );
    root.write(AFTER, &after("now-1", "agents", root.pin(V3), 0, "refuted"));
    root.write(
        &format!("{CURRENT}/case-2.after.json"),
        &after("now-2", "owner", Value::Null, 1, "minor-defect"),
    );
    root.write(
        &format!("{CURRENT}/case-3.after.json"),
        &after("now-3", "owner", Value::Null, 1, "refuted"),
    );
    open(&root);
    assert_eq!(
        reasons(&root.route(&format!("{CURRENT}/case-4.before.json"), &[])),
        [SUSPENDED]
    );
}

/// A pending record sorted before the review that changed an agent settlement still asks.
#[test]
fn a_pending_record_before_the_changing_review_asks() {
    let root = candidates("suspended-pending");
    index(&root, "now-1");
    root.edit(BEFORE, |object| {
        object.insert("ruling".into(), text("now-1"));
    });
    root.write(
        &format!("{CURRENT}/case-2.before.json"),
        &renamed(&root, "now-2"),
    );
    root.write(
        &format!("{CURRENT}/case-3.before.json"),
        &review_of(&root, "now-3", "now-1"),
    );
    root.write(AFTER, &after("now-1", "agents", root.pin(V3), 0, "refuted"));
    root.write(
        &format!("{CURRENT}/case-3.after.json"),
        &after("now-3", "owner", Value::Null, 1, "minor-defect"),
    );
    open(&root);
    assert_eq!(
        reasons(&root.route(&format!("{CURRENT}/case-2.before.json"), &[])),
        [SUSPENDED]
    );
}

/// An agent settlement in an earlier round that the owner's review in this round changed suspends the type here.
#[test]
fn a_review_in_this_round_of_an_earlier_rounds_settlement_suspends_the_type() {
    let root = candidates("suspended-earlier");
    index(&root, "now-1");
    let rulings = format!("{MIDDLE}/rulings");
    root.write(
        &format!("{rulings}/round.json"),
        r#"{"round": "middle", "opened": "2026-01-15", "closed": true, "opening": null, "no_record": {}}"#,
    );
    root.write(
        &format!("{rulings}/case-1.before.json"),
        &renamed(&root, "now-1"),
    );
    root.write(
        &format!("{rulings}/case-1.after.json"),
        &after("now-1", "agents", root.pin(V3), 0, "refuted"),
    );
    open_round(
        &root,
        MIDDLE,
        "middle",
        V3,
        &[&format!("{EARNED}/round.json")],
    );
    root.write(
        &format!("{CURRENT}/case-2.before.json"),
        &renamed(&root, "now-3"),
    );
    root.write(BEFORE, &review_of(&root, "now-2", "now-1"));
    root.write(
        AFTER,
        &after("now-2", "owner", Value::Null, 1, "minor-defect"),
    );
    let middle = format!("{rulings}/round.json");
    open_under(&root, V3, &[&middle]);
    assert_eq!(
        reasons(&root.route(&format!("{CURRENT}/case-2.before.json"), &[])),
        [SUSPENDED]
    );
}

/// Close a middle round that holds agent settlements `now-1` and `now-2`, then owner reviews `case-<n>` of
/// `reviewed` deciding `outcome`, and open the current round after it.
fn after_settlements(root: &Root, reviews: &[(u32, &str, &str)]) {
    let rulings = format!("{MIDDLE}/rulings");
    root.write(
        &format!("{rulings}/round.json"),
        r#"{"round": "middle", "opened": "2026-01-15", "closed": true, "opening": null, "no_record": {}}"#,
    );
    for n in [1, 2] {
        let settled = format!("now-{n}");
        root.write(
            &format!("{rulings}/case-{n}.before.json"),
            &renamed(root, &settled),
        );
        root.write(
            &format!("{rulings}/case-{n}.after.json"),
            &after(&settled, "agents", root.pin(V3), 0, "refuted"),
        );
    }
    for &(n, reviewed, outcome) in reviews {
        let ruling = format!("now-{n}");
        index(root, reviewed);
        root.write(
            &format!("{rulings}/case-{n}.before.json"),
            &review_of(root, &ruling, reviewed),
        );
        root.write(
            &format!("{rulings}/case-{n}.after.json"),
            &after(&ruling, "owner", Value::Null, 1, outcome),
        );
    }
    open_round(
        root,
        MIDDLE,
        "middle",
        V3,
        &[&format!("{EARNED}/round.json")],
    );
    let middle = format!("{rulings}/round.json");
    open_under(root, V3, &[&middle]);
}

/// In a closed round the owner changes two agent settlements and later reviews restore both outcomes; at the next
/// opening each counts against the entry bar.
#[test]
fn an_agent_settlement_changed_then_restored_counts_against_the_entry_bar_at_the_next_opening() {
    let root = candidates("entry-restored");
    after_settlements(
        &root,
        &[
            (3, "now-1", "minor-defect"),
            (4, "now-2", "minor-defect"),
            (5, "now-3", "refuted"),
            (6, "now-4", "refuted"),
        ],
    );
    assert_eq!(reasons(&root.route(BEFORE, &[])), [bar(17, 4, 88)]);
}

/// A confirmed agent settlement stands; one whose confirmation a later review changed and another restored does
/// not.
#[test]
fn a_change_anywhere_in_a_settlements_review_chain_counts_against_the_entry_bar() {
    let root = candidates("entry-chain");
    root.edit(&format!("{EARNED}/case-3.after.json"), |object| {
        object.insert("outcome".into(), text("minor-defect"));
        object.insert(
            "miss".into(),
            json(r#"{"cause": "slip", "note": "The agreed answer was not the decision."}"#),
        );
    });
    after_settlements(
        &root,
        &[
            (3, "now-1", "refuted"),
            (4, "now-2", "refuted"),
            (5, "now-4", "minor-defect"),
            (6, "now-5", "refuted"),
        ],
    );
    assert_eq!(reasons(&root.route(BEFORE, &[])), [bar(17, 4, 88)]);
}

#[test]
fn a_stated_gap_passes_under_the_enforced_terms_and_a_policy_that_keeps_gaps_is_refused() {
    let root = candidates("gap");
    root.edit(BEFORE, |object| {
        if let Some(answer) = answers(object).last() {
            answer.insert("rule_gap".into(), text("The rule does not say."));
            answer.insert("confidence".into(), text("medium"));
            answer.insert("short_of_high".into(), json(r#"["gap"]"#));
            answer.insert("would_settle".into(), Value::Bool(false));
        }
    });
    open(&root);
    assert_eq!(root.route(BEFORE, &[]), SETTLE);

    root.write(
        V3,
        &policy_text(&root, true, |object| {
            object.insert("always_ask".into(), json(ALWAYS_ASK));
        }),
    );
    open(&root);
    assert_eq!(reasons(&root.route(BEFORE, &[]))[0], CHANGED_RULE);
}

#[test]
fn a_canonical_policy_v3_with_other_terms_asks() {
    let root = candidates("terms");
    root.write(
        V3,
        &policy_text(&root, true, |object| {
            object.insert(
                "entry".into(),
                json(r#"{"decisions": 0, "pull_requests": 0, "agreement_percent": 0}"#),
            );
        }),
    );
    open(&root);
    assert_eq!(reasons(&root.route(BEFORE, &[]))[0], CHANGED_RULE);
}

#[test]
fn a_receipt_at_another_path_adopts_nothing() {
    let root = candidates("receipt");
    let other = "bench/grading/rulings/other.md";
    root.write(other, &root.read(RECEIPT));
    root.write(
        V3,
        &policy_text(&root, true, |object| {
            object.insert("receipt".into(), root.pin(other));
        }),
    );
    open(&root);
    assert_eq!(reasons(&root.route(BEFORE, &[]))[0], NOT_ADOPTED);
}

/// A copy of policy v3 outside its canonical path, marked adopted with its own receipt, grants nothing, and an
/// agent settlement that pins it is refused.
#[test]
fn an_opening_that_pins_a_policy_outside_its_canonical_path_sends_everything_to_the_owner() {
    let root = candidates("copy");
    let copy = "docs/research/shared/policy.json";
    root.write(
        copy,
        &policy_text(&root, true, |object| {
            object.insert(
                "entry".into(),
                json(r#"{"decisions": 0, "pull_requests": 0, "agreement_percent": 0}"#),
            );
        }),
    );
    open_under(&root, copy, &[]);
    assert_eq!(
        reasons(&root.route(BEFORE, &[]))[0],
        "the round opening's `policy` must pin bench/grading/delegation.v2.json or bench/grading/delegation.v3.json"
    );
    root.write(
        AFTER,
        &after("case-1", "agents", root.pin(copy), 0, "refuted"),
    );
    open_under(&root, copy, &[]);
    let refused = summary::records(&root.repo()).unwrap_err().describe();
    assert!(refused.contains("agents cannot settle"), "{refused}");
}

#[test]
fn a_proposed_policy_v3_pinned_by_an_opening_sends_every_decision_to_the_owner() {
    let root = candidates("proposed");
    root.write(V3, &policy_text(&root, false, |_| {}));
    open(&root);
    assert_eq!(reasons(&root.route(BEFORE, &[]))[0], NOT_ADOPTED);
}

#[test]
fn a_policy_named_for_routing_that_the_opening_does_not_pin_asks() {
    let root = candidates("named");
    assert_eq!(root.route(BEFORE, &["--policy", V3]), SETTLE);
    assert_eq!(
        reasons(&root.route(BEFORE, &["--policy", "bench/grading/delegation.v2.json"])),
        [format!(
            "the policy named for routing, {}, is not the one the round opening pins",
            root.path()
                .join("bench/grading/delegation.v2.json")
                .display()
        )]
    );
}

#[test]
fn an_unpinned_policy_sends_every_decision_to_the_owner() {
    let root = candidates("unpinned");
    root.write(
        V3,
        &policy_text(&root, true, |object| {
            object.insert("always_ask".into(), json(ALWAYS_ASK));
        }),
    );
    assert_eq!(
        reasons(&root.route(BEFORE, &[]))[0],
        "the round opening's `policy` must pin a policy file by path and sha256"
    );
}

#[test]
fn a_type_whose_filing_is_not_enforced_yet_asks_even_when_its_gate_passes() {
    let root = fixture("admission", "pre-existing-admission", "admitted");
    adopted(&root);
    open(&root);
    assert_eq!(
        reasons(&root.route(BEFORE, &[])),
        ["policy v3 filing for pre-existing-admission is not enforced yet"]
    );
}

fn settlement(root: &Root, after: &Value) -> Vec<String> {
    let repo = root.repo();
    let before = record::read_before(&repo, &root.py_path(BEFORE)).unwrap();
    delegation::settlement_faults(&repo, &before, after, Some(&root.py_path(BEFORE)), None).unwrap()
}

fn settled(root: &Root, policy: Value, outcome: &str) -> Value {
    let mut value = json(&after("case-1", "agents", policy, 0, outcome));
    if let Value::Object(object) = &mut value {
        object.insert("before".into(), root.pin(BEFORE));
    }
    value
}

#[test]
fn an_agent_settlement_under_policy_v3_repeats_the_agreed_answer_and_pins_the_round_policy() {
    let root = candidates("settlement");
    assert_eq!(
        settlement(&root, &settled(&root, root.pin(V3), "refuted")),
        Vec::<String>::new()
    );
    assert_eq!(
        settlement(&root, &settled(&root, root.pin(V3), "minor-defect")),
        ["agent settlement changes the agreed outcome"]
    );
    assert_eq!(
        settlement(
            &root,
            &settled(
                &root,
                root.pin("bench/grading/delegation.v2.json"),
                "refuted"
            )
        ),
        ["agent settlement must pin the policy its round opening pins"]
    );

    root.write(AFTER, &dumps(&settled(&root, root.pin(V3), "refuted")));
    assert!(summary::records(&root.repo()).is_ok());
}

#[test]
fn an_agent_settlement_that_pins_v3_in_a_round_opened_under_v2_is_refused() {
    let root = candidates("v2-opening");
    open_under(&root, "bench/grading/delegation.v2.json", &[]);
    assert_eq!(
        settlement(&root, &settled(&root, root.pin(V3), "refuted")),
        ["agent settlement must pin the policy its round opening pins"]
    );
}

#[test]
fn a_candidate_settled_under_policy_v3_files_with_its_receipt() {
    let root = fixture("filing", "candidate", "refuted");
    adopted(&root);
    root.edit(BEFORE, |object| {
        object.insert(
            "filing".into(),
            json(r#"{"revision": {"head": "h1"}, "subject": "S1"}"#),
        );
    });
    root.write(
        AFTER,
        &after("case-1", "agents", root.pin(V3), 0, "refuted"),
    );
    open(&root);
    let decision = |receipt: &str, scope: &str, reason: &str| {
        json(&format!(
            r#"{{"target": "t-1", "revision": {{"head": "h1"}}, "subject": "S1", "review": "r1", "attempt_id": "att-001",
                "item_id": "item-0", "family_id": "F1", "dimension": "eligibility", "status": "approved",
                "authority": "human", "outcome": "refuted", "settled_by": "agents", "settlement": {},
                "receipt": {}, "receipt_scope": "{scope}", "reason": "{reason}"}}"#,
            dumps(&root.pin(AFTER)),
            dumps(&root.pin(receipt)),
        ))
    };
    let faults = |decision: &Value, kind: &str| {
        delegation::filing_faults(&root.repo(), decision, kind).unwrap()
    };
    let v3 = decision(RECEIPT, SCOPE, "Settled by agents under policy v3.");
    assert_eq!(faults(&v3, "candidate"), Vec::<String>::new());

    let v2 = decision(
        "bench/grading/rulings/delegation.v2.md",
        "Adopt policy v2 as written",
        "Settled by agents under policy v2.",
    );
    assert_eq!(
        faults(&v2, "candidate"),
        [
            "agent settlement must cite the owner's saved policy adoption",
            "reason must say agents settled under policy v3",
        ]
    );

    assert!(
        faults(&v3, "recovery").contains(
            &"credit stays with the owner under policy v3; agents settle no credit decision"
                .to_string()
        )
    );

    let owner = json(&format!(
        r#"{{"settled_by": "owner", "settlement": null, "receipt": {}}}"#,
        dumps(&root.pin(RECEIPT))
    ));
    assert_eq!(
        faults(&owner, "candidate"),
        ["a settlement pin or policy adoption receipt requires settled_by agents"]
    );
}
