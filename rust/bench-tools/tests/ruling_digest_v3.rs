//! The summary gives the owner ADR-0008's digest for a round that runs under an adopted policy v3: every agent
//! settlement, which ones the owner reads, and the gaps and conflicts the agreed answers stated. A round that
//! runs under policy v2 gets none of it. These cases take the `ruling-route-settles-level-1` corpus case, with
//! every record turned into a candidate decision on `refuted`.

use bench_tools::pyjson::{self, Dumps, Duplicates, Object, Value};
use bench_tools::ruling::{Repo, summary};
use bench_tools::sha256::hex_digest;
use std::path::{Path, PathBuf};

const EARNED: &str = "docs/research/earned-2026-01-01/rulings";
const CURRENT: &str = "docs/research/current-2026-02-01/rulings";
const MIDDLE: &str = "docs/research/middle-2026-01-15/rulings";
const V2: &str = "bench/grading/delegation.v2.json";
const V3: &str = "bench/grading/delegation.v3.json";
const RECEIPT: &str = "bench/grading/rulings/delegation.v3.md";
const SCOPE: &str = "Adopt policy v3 as written";
const DOSSIER: &str = "docs/research/shared/dossier";

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
    let path = std::env::temp_dir().join(format!("ruling-digest-v3-{name}-{}", std::process::id()));
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

fn convert(object: &mut Object) {
    if object.get("decision_type").is_some() {
        object.insert("decision_type".into(), text("candidate"));
        object.insert("dossier".into(), text(DOSSIER));
        for answer in answers(object) {
            answer.insert("outcome".into(), text("refuted"));
            *answer = without(answer, "facts");
        }
    } else {
        object.insert("outcome".into(), text("refuted"));
        *object = without(object, "facts");
    }
}

fn policy_text(root: &Root) -> String {
    let mut value = json(&std::fs::read_to_string(repository().join(V3)).unwrap());
    let Value::Object(object) = &mut value else {
        panic!("the shipped policy is not an object");
    };
    object.insert("status".into(), text("adopted"));
    object.insert("receipt".into(), root.pin(RECEIPT));
    object.insert("adoption_scope".into(), text(SCOPE));
    dumps(&value)
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

/// The corpus with every record a candidate decision, the receipt and the adopted policy v3 saved.
fn fixture(name: &str) -> Root {
    let root = corpus(name);
    root.write(
        &format!("{DOSSIER}/summary.json"),
        r#"[{"kind": "candidate", "group": "G1", "recommendation": "refuted"}]"#,
    );
    root.write(
        RECEIPT,
        &format!("# Policy v3 adoption\n\nThe owner answered: {SCOPE}.\n"),
    );
    root.write(V3, &policy_text(&root));
    for n in 1..=15 {
        let before = format!("{EARNED}/case-{n}.before.json");
        root.edit(&before, convert);
        root.edit(&format!("{EARNED}/case-{n}.after.json"), |object| {
            convert(object);
            object.insert("before".into(), root.pin(&before));
        });
    }
    root.edit(&format!("{CURRENT}/case-1.before.json"), convert);
    root
}

/// The current round's first before-record under another ruling name.
fn renamed(root: &Root, ruling: &str) -> String {
    let mut record = json(&root.read(&format!("{CURRENT}/case-1.before.json")));
    if let Value::Object(object) = &mut record {
        object.insert("ruling".into(), text(ruling));
    }
    dumps(&record)
}

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

/// Save `count` agent-settled candidates `{prefix}-1`… in `rulings`.
fn settle(root: &Root, rulings: &str, prefix: &str, count: u32) {
    for n in 1..=count {
        let ruling = format!("{prefix}-{n}");
        root.write(
            &format!("{rulings}/case-{n}.before.json"),
            &renamed(root, &ruling),
        );
        root.write(
            &format!("{rulings}/case-{n}.after.json"),
            &after(&ruling, "agents", root.pin(V3), 0, "refuted"),
        );
    }
}

/// A closed round `middle` under policy v3 with `count` agent settlements, opened after the earned round.
fn middle(root: &Root, count: u32) {
    root.write(
        &format!("{MIDDLE}/round.json"),
        r#"{"round": "middle", "opened": "2026-01-15", "closed": true, "opening": null, "no_record": {}}"#,
    );
    settle(root, MIDDLE, "mid", count);
    open_round(
        root,
        "docs/research/middle-2026-01-15",
        "middle",
        V3,
        &[&format!("{EARNED}/round.json")],
    );
}

/// Open the current round under `policy` after the earned round and, when `after_middle`, the middle round.
fn open_current(root: &Root, policy: &str, after_middle: bool) {
    let earned = format!("{EARNED}/round.json");
    let middle = format!("{MIDDLE}/round.json");
    let mut closed = vec![earned.as_str()];
    if after_middle {
        closed.push(middle.as_str());
    }
    open_round(
        root,
        "docs/research/current-2026-02-01",
        "current",
        policy,
        &closed,
    );
}

fn candidate(summary: &summary::Summary) -> Value {
    let value = summary.to_value().unwrap();
    value
        .as_object()
        .unwrap()
        .get("contract_2")
        .unwrap()
        .as_object()
        .unwrap()
        .get("candidate")
        .unwrap()
        .clone()
}

fn member<'a>(value: &'a Value, key: &str) -> &'a Value {
    value.as_object().unwrap().get(key).unwrap()
}

struct Shown {
    text: String,
    candidate: Value,
}

fn show(root: &Root, round: Option<&str>) -> Shown {
    let repo = root.repo();
    let pairs = summary::records(&repo).unwrap();
    let summary = summary::summarize(&repo, &pairs, round).unwrap();
    Shown {
        text: summary.render().unwrap(),
        candidate: candidate(&summary),
    }
}

impl Shown {
    fn digest(&self) -> &[Value] {
        member(&self.candidate, "digest").as_array().unwrap()
    }

    fn read(&self) -> Vec<bool> {
        self.digest()
            .iter()
            .map(|settled| matches!(member(settled, "read"), Value::Bool(true)))
            .collect()
    }

    fn reading(&self) -> &[Value] {
        member(&self.candidate, "reading").as_array().unwrap()
    }

    fn mode(&self, round: &str) -> &str {
        let reading = self
            .reading()
            .iter()
            .find(|reading| member(reading, "round").as_str() == Some(round))
            .unwrap();
        member(reading, "mode").as_str().unwrap()
    }

    fn reasons(&self, round: &str) -> Vec<&str> {
        let reading = self
            .reading()
            .iter()
            .find(|reading| member(reading, "round").as_str() == Some(round))
            .unwrap();
        member(reading, "reasons")
            .as_array()
            .unwrap()
            .iter()
            .map(|reason| reason.as_str().unwrap())
            .collect()
    }

    fn gaps(&self) -> &[Value] {
        member(&self.candidate, "stated_gaps").as_array().unwrap()
    }

    fn read_lines(&self) -> Vec<&str> {
        self.text
            .lines()
            .filter(|line| line.starts_with("- ") && line.ends_with(" [read]"))
            .collect()
    }
}

#[test]
fn a_first_round_under_policy_v3_lists_every_settlement_and_marks_them_all_to_read() {
    let root = fixture("first");
    settle(&root, CURRENT, "now", 3);
    open_current(&root, V3, false);
    let shown = show(&root, Some("current"));
    assert_eq!(shown.digest().len(), 3);
    assert_eq!(shown.read(), [true, true, true]);
    assert_eq!(shown.mode("current"), "all");
    assert_eq!(
        shown.reasons("current"),
        ["the first round under policy v3 with an agent settlement of this type"]
    );
    assert_eq!(shown.read_lines().len(), 3);
    assert!(
        shown.text.contains(
            "- round current: read every settlement: the first round under policy v3 with an agent settlement of this type"
        ),
        "{}",
        shown.text
    );
    assert!(shown.gaps().is_empty());
    assert!(shown.text.contains("Stated gaps and conflicts:\n- none"));
}

#[test]
fn a_later_round_with_the_same_models_marks_one_settlement_in_five_by_hash() {
    let root = fixture("sampled");
    middle(&root, 1);
    settle(&root, CURRENT, "now", 6);
    open_current(&root, V3, true);
    let repo = root.repo();
    let pairs = summary::records(&repo).unwrap();
    let mut hashes: Vec<(String, String)> = pairs
        .iter()
        .filter(|pair| pair.path.starts_with(CURRENT) && pair.path.ends_with(".before.json"))
        .map(|pair| (pair.hash.clone(), pair.path.clone()))
        .collect();
    hashes.sort();
    let expected: Vec<&str> = [0, 5].iter().map(|at| hashes[*at].1.as_str()).collect();

    let shown = show(&root, Some("current"));
    assert_eq!(shown.digest().len(), 6);
    assert_eq!(shown.mode("current"), "one-in-five");
    assert_eq!(shown.reasons("current"), Vec::<&str>::new());
    let marked: Vec<&str> = shown
        .digest()
        .iter()
        .filter(|settled| matches!(member(settled, "read"), Value::Bool(true)))
        .map(|settled| member(settled, "path").as_str().unwrap())
        .collect();
    let mut marked_sorted = marked.clone();
    marked_sorted.sort_by_key(|path| hashes.iter().position(|(_, known)| known == path));
    assert_eq!(marked_sorted, expected);
    assert_eq!(shown.read_lines().len(), 2);
    assert!(
        shown.text.contains(
            "- round current: read one in five, every fifth by before-record hash starting with the first"
        ),
        "{}",
        shown.text
    );
}

#[test]
fn a_replaced_model_makes_the_owner_read_every_settlement() {
    let root = fixture("model");
    middle(&root, 1);
    settle(&root, CURRENT, "now", 3);
    root.edit(&format!("{CURRENT}/case-3.before.json"), |object| {
        if let Some(answer) = answers(object).last() {
            answer.insert("model".into(), text("a-replacement-model"));
        }
    });
    open_current(&root, V3, true);
    let shown = show(&root, Some("current"));
    assert_eq!(shown.mode("current"), "all");
    assert_eq!(shown.read(), [true, true, true]);
    let reasons = shown.reasons("current");
    assert_eq!(reasons.len(), 1, "{reasons:?}");
    assert!(
        reasons[0].starts_with("a party's model was replaced")
            && reasons[0].contains(&format!("{CURRENT}/case-3.before.json")),
        "{reasons:?}"
    );
}

#[test]
fn a_model_replaced_first_on_an_owner_decision_makes_the_owner_read_every_settlement() {
    let root = fixture("model-owner-first");
    middle(&root, 1);
    settle(&root, CURRENT, "now", 4);
    root.write(
        &format!("{CURRENT}/case-1.after.json"),
        &after("now-1", "owner", Value::Null, 1, "refuted"),
    );
    for n in 1..=4 {
        root.edit(&format!("{CURRENT}/case-{n}.before.json"), |object| {
            if let Some(answer) = answers(object).last() {
                answer.insert("model".into(), text("a-replacement-model"));
            }
        });
    }
    open_current(&root, V3, true);
    let shown = show(&root, Some("current"));
    assert_eq!(shown.digest().len(), 3);
    assert_eq!(shown.mode("current"), "all");
    assert_eq!(shown.read(), [true, true, true]);
    let reasons = shown.reasons("current");
    assert_eq!(reasons.len(), 1, "{reasons:?}");
    assert!(
        reasons[0].starts_with("a party's model was replaced")
            && reasons[0].contains(&format!("{CURRENT}/case-1.before.json")),
        "{reasons:?}"
    );
}

#[test]
fn a_model_replaced_on_an_owner_decision_that_closes_a_round_makes_the_next_round_read_every_settlement()
 {
    let root = fixture("model-across-rounds");
    middle(&root, 1);
    root.write(
        &format!("{MIDDLE}/case-2.before.json"),
        &renamed(&root, "mid-2"),
    );
    settle(&root, CURRENT, "now", 3);
    let replaced = [
        format!("{MIDDLE}/case-2.before.json"),
        format!("{CURRENT}/case-1.before.json"),
        format!("{CURRENT}/case-2.before.json"),
        format!("{CURRENT}/case-3.before.json"),
    ];
    for path in &replaced {
        root.edit(path, |object| {
            if let Some(answer) = answers(object).last() {
                answer.insert("model".into(), text("a-replacement-model"));
            }
        });
    }
    let mut owner = json(&after("mid-2", "owner", Value::Null, 1, "refuted"));
    if let Value::Object(object) = &mut owner {
        object.insert(
            "before".into(),
            root.pin(&format!("{MIDDLE}/case-2.before.json")),
        );
    }
    root.write(&format!("{MIDDLE}/case-2.after.json"), &dumps(&owner));
    open_current(&root, V3, true);
    let shown = show(&root, Some("current"));
    assert_eq!(shown.mode("current"), "all");
    assert_eq!(shown.read(), [true, true, true]);
    assert_eq!(
        shown.reasons("current"),
        [format!(
            "a party's model was replaced: {CURRENT}/case-1.before.json differs from the latest agent settlement of this type in an earlier round"
        )]
    );
}

#[test]
fn an_owner_change_to_a_settlement_makes_the_owner_read_every_settlement() {
    let root = fixture("suspended");
    middle(&root, 1);
    index(&root, "now-1");
    settle(&root, CURRENT, "now", 1);
    let mut review = json(&renamed(&root, "now-2"));
    if let Value::Object(object) = &mut review {
        object.insert(
            "reviews".into(),
            json(r#"{"ruling": "now-1", "saved_outcome": "refuted"}"#),
        );
    }
    root.write(&format!("{CURRENT}/case-2.before.json"), &dumps(&review));
    root.write(
        &format!("{CURRENT}/case-2.after.json"),
        &after("now-2", "owner", Value::Null, 1, "minor-defect"),
    );
    open_current(&root, V3, true);
    let shown = show(&root, Some("current"));
    assert_eq!(shown.mode("current"), "all");
    assert_eq!(shown.read(), [true]);
    assert_eq!(
        shown.reasons("current"),
        [
            "an owner review in this round changed an agent settlement of this type, so the type is suspended for the rest of the round"
        ]
    );
}

#[test]
fn an_owner_change_that_a_later_review_restores_still_makes_the_owner_read_every_settlement() {
    let root = fixture("restored");
    middle(&root, 1);
    index(&root, "now-1");
    index(&root, "now-2");
    settle(&root, CURRENT, "now", 1);
    for (case, ruling, reviewed, saved, outcome) in [
        (2, "now-2", "now-1", "refuted", "minor-defect"),
        (3, "now-3", "now-2", "minor-defect", "refuted"),
    ] {
        let mut review = json(&renamed(&root, ruling));
        if let Value::Object(object) = &mut review {
            object.insert(
                "reviews".into(),
                json(&format!(
                    r#"{{"ruling": "{reviewed}", "saved_outcome": "{saved}"}}"#
                )),
            );
        }
        root.write(
            &format!("{CURRENT}/case-{case}.before.json"),
            &dumps(&review),
        );
        root.write(
            &format!("{CURRENT}/case-{case}.after.json"),
            &after(ruling, "owner", Value::Null, 1, outcome),
        );
    }
    open_current(&root, V3, true);
    let shown = show(&root, Some("current"));
    assert_eq!(shown.mode("current"), "all");
    assert_eq!(
        shown.reasons("current"),
        [
            "an owner review in this round changed an agent settlement of this type, so the type is suspended for the rest of the round"
        ]
    );
}

#[test]
fn a_gap_stated_before_an_agreed_settlement_is_quoted_with_its_party() {
    let root = fixture("gap");
    middle(&root, 1);
    settle(&root, CURRENT, "now", 2);
    let said = "The rule does not say.";
    root.edit(&format!("{CURRENT}/case-2.before.json"), |object| {
        if let Some(answer) = answers(object).last() {
            answer.insert("rule_gap".into(), text(said));
            answer.insert("confidence".into(), text("medium"));
            answer.insert("short_of_high".into(), json(r#"["gap"]"#));
            answer.insert("would_settle".into(), Value::Bool(false));
        }
    });
    root.edit(&format!("{CURRENT}/case-1.before.json"), |object| {
        if let Some(answer) = answers(object).last() {
            answer.insert("conflict".into(), text("Two rules disagree."));
            answer.insert("confidence".into(), text("medium"));
            answer.insert("short_of_high".into(), json(r#"["conflict"]"#));
            answer.insert("would_settle".into(), Value::Bool(false));
        }
    });
    open_current(&root, V3, true);
    let shown = show(&root, Some("current"));
    assert_eq!(shown.gaps().len(), 2);
    let gap = shown
        .gaps()
        .iter()
        .find(|gap| member(gap, "stated").as_str() == Some("rule_gap"))
        .unwrap();
    assert_eq!(member(gap, "said").as_str(), Some(said));
    assert_eq!(member(gap, "ruling").as_str(), Some("now-2"));
    assert_eq!(member(gap, "by").as_str(), Some("assessor-2"));
    assert!(
        shown.text.contains(&format!(
            "- now-2 G1, assessor-2: rule_gap: {said} {CURRENT}/case-2.before.json"
        )),
        "{}",
        shown.text
    );
    assert!(shown.text.contains(": conflict: Two rules disagree."));
}

#[test]
fn a_round_without_a_policy_pin_shows_none_of_it_beside_an_adopted_policy_v3_file() {
    let root = fixture("v2");
    for round in [None, Some("current"), Some("earned")] {
        let repo = root.repo();
        let pairs = summary::records(&repo).unwrap();
        let summary = summary::summarize(&repo, &pairs, round).unwrap();
        let rendered = summary.render().unwrap();
        let value = dumps(&summary.to_value().unwrap());
        for new in [
            "Owner reading under policy v3",
            "Stated gaps and conflicts",
            "[read]",
            "\"reading\"",
            "\"stated_gaps\"",
            "\"read\"",
        ] {
            assert!(!rendered.contains(new), "{new} in {rendered}");
            assert!(!value.contains(new), "{new} in {value}");
        }
    }
}

#[test]
fn a_round_that_pins_policy_v2_shows_none_of_it_for_a_settled_decision() {
    let root = fixture("v2-pinned");
    settle(&root, CURRENT, "now", 2);
    for n in 1..=2 {
        root.write(
            &format!("{CURRENT}/case-{n}.after.json"),
            &after(&format!("now-{n}"), "agents", root.pin(V2), 0, "refuted"),
        );
    }
    open_current(&root, V2, false);
    let shown = show(&root, Some("current"));
    assert_eq!(shown.digest().len(), 2);
    let rendered = &shown.text;
    assert!(!rendered.contains("Owner reading under policy v3"));
    assert!(!rendered.contains("[read]"));
    assert!(
        shown
            .candidate
            .as_object()
            .unwrap()
            .get("reading")
            .is_none()
    );
}
