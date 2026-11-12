//! Reconciliations of an evaluator audit settle under policy v3 (ADR-0008) on the reconciler's call at medium or
//! high confidence, except the owner's five calls of the audit round and every unit that shares a question with
//! one. These cases take the `ruling-route-settles-level-1` corpus case, turn its closed round into fifteen agreed
//! reconciliations on four pull requests, and replace its open round with one before-record for each disputed unit
//! of an audit's comparison.

use bench_tools::current_grading::Root as Current;
use bench_tools::delegation;
use bench_tools::evaluator_audit;
use bench_tools::pyjson::{self, Dumps, Duplicates, Object, Value};
use bench_tools::pytext::PyPath;
use bench_tools::ruling::{Repo, record, summary};
use bench_tools::sha256::hex_digest;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

const EARNED: &str = "docs/research/earned-2026-01-01/rulings";
const ROUND: &str = "docs/research/current-2026-02-01";
const CURRENT: &str = "docs/research/current-2026-02-01/rulings";
const V3: &str = "bench/grading/delegation.v3.json";
const RECEIPT: &str = "bench/grading/rulings/delegation.v3.md";
const SCOPE: &str = "Adopt policy v3 as written";
const AUDITS: &str = "bench/grading/current/audits.json";
const AUDIT: &str = "bench/grading/current/audit/evaluator-audit-v2-2026-10-29";
const SEED: &str = "evaluator-audit-v2-2026-10-29";
const INDEX: &str = "docs/research/shared/rulings.index.json";
const SETTLE: &str = "SETTLE under policy v3, on agreement\n";
const AUDIT_CALL: &str = "this unit is one of its audit round's five calls, which the owner decides without the reconciler's call";
const SHARES: &str = "this unit shares its question with one of the audit round's five calls, so the owner decides the question whole";
const UNCOVERED: &str = "the round's reconciliation before-records must file each disputed unit of the pinned comparison exactly once, and no other";
const LOW: &str = "the parties did not give the agreement policy v3 needs: the recommender's answer, recommender at medium or high confidence";
const SHAPE: &str = "the before-record's `filing` must bind the audit unit it files: exactly `comparison`, the pinned comparison.json of the audit, and `unit`, the record's `group`";
const UNREADABLE: &str = "the comparison the filing pins cannot be read as an evaluator audit's: its units and the seed of the sample it pins";
const SUSPENDED: &str = "a review in this round changed an agent settlement of this decision type, so policy v3 suspends it for the rest of the round";
const WITHHELD: &str = "First answers: withheld until the records show that this unit is neither one of its audit round's five calls nor in a question with one.";

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
    fn read(&self, path: &str) -> String {
        std::fs::read_to_string(self.0.join(path)).unwrap()
    }

    fn write(&self, path: &str, contents: &str) {
        let file = self.0.join(path);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(file, contents).unwrap();
    }

    fn remove(&self, path: &str) {
        std::fs::remove_file(self.0.join(path)).unwrap();
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

    fn run(&self, arguments: &[&str]) -> String {
        let output = Command::new(env!("CARGO_BIN_EXE_bench-tools"))
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

    fn route(&self, before: &str) -> String {
        self.run(&["ruling", "route", before])
    }

    /// The reasons `route` gives the before-record at `before`, read from every record of the root.
    fn reasons(&self, before: &str) -> Vec<String> {
        let repo = self.repo();
        let record = record::read_before(&repo, &self.py_path(before)).unwrap();
        delegation::route_under(&repo, &record, None, None, Some(&self.py_path(before)))
            .unwrap()
            .reasons
    }
}

fn corpus(name: &str) -> Root {
    let corpus = json(
        &std::fs::read_to_string(
            repository().join("tools/behavior_contracts/ruling-record.v1.json"),
        )
        .unwrap(),
    );
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
    let path = std::env::temp_dir().join(format!(
        "ruling-reconciliation-v3-{name}-{}",
        std::process::id()
    ));
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

/// A credit before-record as a reconciliation: the recommender's answer alone, on `first-error`.
fn reconciliation(object: &mut Object) {
    object.insert("decision_type".into(), text("reconciliation"));
    if let Some(Value::Array(all)) = object.get_mut("answers") {
        all.truncate(1);
    }
    for answer in answers(object) {
        answer.insert("outcome".into(), text("first-error"));
        *answer = without(answer, "facts");
    }
}

/// The disputed units of the saved audit round this test copies, in the comparison's order.
fn disputed() -> &'static [String] {
    static UNITS: OnceLock<Vec<String>> = OnceLock::new();
    UNITS.get_or_init(|| {
        let compared = json(
            &std::fs::read_to_string(repository().join(format!("{AUDIT}/comparison.json")))
                .unwrap(),
        );
        compared
            .as_object()
            .unwrap()
            .get("units")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row.as_object().unwrap())
            .filter(|row| !matches!(row.get("agreement"), Some(Value::Bool(true))))
            .map(|row| row.get("id").unwrap().as_str().unwrap().to_string())
            .collect()
    })
}

fn count() -> usize {
    disputed().len()
}

fn unit(n: usize) -> String {
    disputed()[n - 1].clone()
}

fn before_path(n: usize) -> String {
    format!("{CURRENT}/case-{n}.before.json")
}

fn after_path(n: usize) -> String {
    format!("{CURRENT}/case-{n}.after.json")
}

/// The owner's five calls, ranked here from the seed and the unit ids alone.
fn ranked(seed: &str, units: &[String]) -> Vec<String> {
    let mut order: Vec<(String, String)> = units
        .iter()
        .map(|unit| {
            (
                hex_digest(format!("{seed}:reconciliation:{unit}").as_bytes()),
                unit.clone(),
            )
        })
        .collect();
    order.sort();
    order.into_iter().take(5).map(|(_, unit)| unit).collect()
}

fn calls() -> Vec<usize> {
    let calls = ranked(SEED, disputed());
    (1..=count())
        .filter(|n| calls.contains(&unit(*n)))
        .collect()
}

fn others() -> Vec<usize> {
    let calls = calls();
    (1..=count()).filter(|n| !calls.contains(n)).collect()
}

fn comparison(root: &Root) -> Value {
    root.pin(&format!("{AUDIT}/comparison.json"))
}

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// Save the opening of the current round under policy v3 after the earned round, and move every pin of the round
/// to it.
fn open(root: &Root) {
    let opening = format!("{ROUND}/opening.json");
    root.write(
        &opening,
        &format!(
            r#"{{"round": "current", "source_commit": "{}", "closed_rounds": [{}], "policy": {}}}"#,
            "a".repeat(40),
            dumps(&root.pin(&format!("{EARNED}/round.json"))),
            dumps(&root.pin(V3))
        ),
    );
    let pinned = root.pin(&opening);
    root.edit(&format!("{CURRENT}/round.json"), |object| {
        object.insert("opening".into(), pinned.clone());
    });
    let mut names: Vec<String> = std::fs::read_dir(root.0.join(CURRENT))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    for name in names.iter().filter(|name| name.ends_with(".before.json")) {
        root.edit(&format!("{CURRENT}/{name}"), |object| {
            object.insert("round_opening".into(), pinned.clone());
        });
    }
    for name in names.iter().filter(|name| name.ends_with(".after.json")) {
        let before = format!("{CURRENT}/{}", name.replace(".after.json", ".before.json"));
        root.edit(&format!("{CURRENT}/{name}"), |object| {
            object.insert("before".into(), root.pin(&before));
        });
    }
}

fn filing_value(comparison: &Value, unit: &str) -> Value {
    Value::Object(
        [
            ("comparison".to_string(), comparison.clone()),
            ("unit".to_string(), text(unit)),
        ]
        .into_iter()
        .collect(),
    )
}

/// Point every current before-record's `filing` at the comparison `path` pins, and open the round again.
fn refile(root: &Root, path: &str) {
    let pinned = root.pin(path);
    for n in 1..=count() {
        root.edit(&before_path(n), |object| {
            object.insert("filing".into(), filing_value(&pinned, &unit(n)));
        });
    }
    open(root);
}

/// The corpus with a closed round of fifteen agreed reconciliations, an adopted policy v3, a copy of the saved audit
/// round `evaluator-audit-v2-2026-10-29` (its plan, sample, own grades, second assessments and comparison), and an
/// open round with one before-record for each disputed unit, filed when `filed`. Unless `plan`, the copied round is
/// recorded as superseded, its sample pinned by the plan file as a closed round's is; as the plan's own round it must
/// redraw from an inventory this root does not hold.
fn fixture_with(name: &str, filed: bool, plan: bool) -> Root {
    let root = corpus(name);
    root.write(
        RECEIPT,
        &format!("# Policy v3 adoption\n\nThe owner answered: {SCOPE}.\n"),
    );
    for n in 1..=15 {
        let before = format!("{EARNED}/case-{n}.before.json");
        root.edit(&before, reconciliation);
        root.edit(&format!("{EARNED}/case-{n}.after.json"), |object| {
            object.insert("outcome".into(), text("first-error"));
            *object = without(object, "facts");
            object.insert("before".into(), root.pin(&before));
        });
    }
    let mut policy = json(&std::fs::read_to_string(repository().join(V3)).unwrap());
    if let Value::Object(object) = &mut policy {
        object.insert("status".into(), text("adopted"));
        object.insert("receipt".into(), root.pin(RECEIPT));
        object.insert("adoption_scope".into(), text(SCOPE));
    }
    root.write(V3, &dumps(&policy));
    root.write(
        AUDITS,
        &std::fs::read_to_string(repository().join(AUDITS)).unwrap(),
    );
    let directory = root.0.join(AUDIT);
    std::fs::create_dir_all(&directory).unwrap();
    for name in ["sample.json", "comparison.json", "grades.json"] {
        std::fs::copy(repository().join(AUDIT).join(name), directory.join(name)).unwrap();
    }
    copy_tree(
        &repository().join(AUDIT).join("second"),
        &directory.join("second"),
    );
    if !plan {
        let sample = root.pin(&format!("{AUDIT}/sample.json"));
        root.edit(AUDITS, |object| {
            let mut closed = Object::default();
            closed.insert("plan".into(), object.get("plan").unwrap().clone());
            closed.insert("evidence".into(), Value::Array(vec![sample]));
            if let Some(Value::Object(plan)) = object.get_mut("plan") {
                plan.insert("seed".into(), text("evaluator-audit-v3-test"));
            }
            if let Some(Value::Array(superseded)) = object.get_mut("superseded") {
                superseded.push(Value::Object(closed));
            }
        });
    }
    let mut template = json(&root.read(&before_path(1)));
    let Value::Object(object) = &mut template else {
        panic!("the before-record is not an object");
    };
    reconciliation(object);
    let pinned = comparison(&root);
    for n in 1..=count() {
        let mut record = object.clone();
        record.insert("ruling".into(), text(&format!("case-{n}")));
        record.insert("group".into(), text(&unit(n)));
        if filed {
            record.insert("filing".into(), filing_value(&pinned, &unit(n)));
        }
        root.write(&before_path(n), &dumps(&Value::Object(record)));
    }
    open(&root);
    root
}

fn fixture(name: &str) -> Root {
    fixture_with(name, true, false)
}

fn confidence(object: &mut Object, level: &str, short: &str) {
    for answer in answers(object) {
        answer.insert("confidence".into(), text(level));
        answer.insert("short_of_high".into(), json(short));
        answer.insert("would_settle".into(), Value::Bool(level == "high"));
    }
}

fn decided(root: &Root, n: usize, settled_by: &str, outcome: &str) {
    let (policy, asked) = if settled_by == "agents" {
        (dumps(&root.pin(V3)), 0)
    } else {
        ("null".to_string(), 1)
    };
    let miss = if settled_by == "owner" && outcome != "first-error" {
        r#"{"cause": "owner-weighs-differently", "note": "The owner weighed it differently."}"#
    } else {
        "null"
    };
    root.write(
        &after_path(n),
        &format!(
            r#"{{"contract": 2, "ruling": "case-{n}", "before": {}, "settled_by": "{settled_by}", "policy": {policy}, "asked": {asked}, "outcome": "{outcome}", "by_default": false, "ground": null, "miss": {miss}, "lesson": null}}"#,
            dumps(&root.pin(&before_path(n)))
        ),
    );
}

fn row(root: &Root, n: usize, finding: &str, change: impl FnOnce(&mut Object)) -> Value {
    let mut value = json(&format!(
        r#"{{"unit": "{}", "strata": ["refuted"], "first": "refuted", "second": "suggestion", "finding": "{finding}",
            "reason": "Settled by agents under policy v3.", "evidence": ["{}"], "settled_by": "agents",
            "settlement": {}, "receipt": {}, "receipt_scope": "{SCOPE}"}}"#,
        unit(n),
        after_path(n),
        dumps(&root.pin(&after_path(n))),
        dumps(&root.pin(RECEIPT))
    ));
    if let Value::Object(object) = &mut value {
        change(object);
    }
    value
}

fn owner_row(n: usize, finding: &str) -> Value {
    json(&format!(
        r#"{{"unit": "{}", "strata": ["refuted"], "first": "refuted", "second": "suggestion",
            "finding": "{finding}", "reason": "The owner decided {finding}.", "evidence": []}}"#,
        unit(n)
    ))
}

fn row_faults(root: &Root, row: &Value, comparison: &Value) -> Vec<String> {
    let current = Current::new(root.0.as_os_str().as_encoded_bytes()).unwrap();
    let prefix = format!(
        "{}: ",
        row.as_object()
            .unwrap()
            .get("unit")
            .unwrap()
            .as_str()
            .unwrap()
    );
    evaluator_audit::reconciliation_faults(&current, std::slice::from_ref(row), comparison)
        .unwrap()
        .into_iter()
        .map(|fault| fault.strip_prefix(&prefix).unwrap().to_string())
        .collect()
}

/// The units whose route names them an audit call.
fn calls_of(root: &Root) -> Vec<usize> {
    (1..=count())
        .filter(|n| {
            root.reasons(&before_path(*n))
                .iter()
                .any(|reason| reason == AUDIT_CALL)
        })
        .collect()
}

fn withheld(root: &Root, n: usize, reasons: &[&str]) {
    let routed = root.route(&before_path(n));
    let case = root.read(&format!("{ROUND}/cases/case-1.md"));
    let listed: Vec<String> = reasons.iter().map(|reason| format!("- {reason}")).collect();
    assert_eq!(
        routed,
        format!(
            "ASK\n{}\n\nThe case, {ROUND}/cases/case-1.md:\n\n{}\n\n{WITHHELD}\n",
            listed.join("\n"),
            case.trim()
        )
    );
    assert!(!routed.contains("first-error"));
    assert!(!routed.contains("recommender"));
}

#[test]
fn the_ranking_depends_on_the_seed_and_unit_ids_alone() {
    let units = disputed().to_vec();
    let ranks = delegation::v3::audit_calls(SEED, &units);
    assert_eq!(ranks, ranked(SEED, &units));
    let mut reversed = units.clone();
    reversed.reverse();
    assert_eq!(delegation::v3::audit_calls(SEED, &reversed), ranks);
    assert_ne!(delegation::v3::audit_calls("seed-y", &units), ranks);
    assert_eq!(delegation::v3::audit_calls(SEED, &units[..3]).len(), 3);

    let root = fixture("ranking");
    let call = calls_of(&root);
    assert_eq!(call, calls());
    for n in 1..=count() {
        root.edit(&before_path(n), |object| {
            for answer in answers(object) {
                answer.insert(
                    "reason".into(),
                    text("Reworded, so the record's bytes and hash change."),
                );
            }
        });
    }
    open(&root);
    assert_eq!(calls_of(&root), call);
}

#[test]
fn a_reconciler_call_at_medium_confidence_settles_and_at_low_confidence_asks() {
    let root = fixture("confidence");
    let (medium, low) = (others()[0], others()[1]);
    root.edit(&before_path(medium), |object| {
        confidence(object, "medium", r#"["fact-reported"]"#);
    });
    root.edit(&before_path(low), |object| {
        confidence(object, "low", r#"["fact-reported", "flip-fact-open"]"#);
    });
    open(&root);
    assert_eq!(root.route(&before_path(medium)), SETTLE);
    assert_eq!(root.reasons(&before_path(low)), [LOW]);
}

#[test]
fn an_owner_call_asks_without_the_reconcilers_call() {
    let root = fixture("owner-call");
    withheld(&root, calls()[0], &[AUDIT_CALL]);
}

#[test]
fn an_owner_call_without_its_filing_still_withholds_the_call() {
    let root = fixture("owner-call-unfiled");
    let n = calls()[0];
    root.edit(&before_path(n), |object| {
        object.insert("filing".into(), Value::Null);
    });
    open(&root);
    withheld(&root, n, &[SHAPE]);
}

#[test]
fn a_unit_that_shares_a_question_with_an_owner_call_asks() {
    let root = fixture("shares");
    let (call, other) = (calls()[0], others()[0]);
    root.edit(&before_path(other), |object| {
        object.insert("ruling".into(), text("alias-of-call"));
    });
    root.edit(INDEX, |object| {
        if let Some(Value::Object(rulings)) = object.get_mut("rulings") {
            rulings.insert(
                format!("case-{call}"),
                json(r#"{"reading": "current", "aliases": ["alias-of-call"]}"#),
            );
        }
    });
    open(&root);
    assert_eq!(root.reasons(&before_path(other)), [SHARES]);
    withheld(&root, other, &[SHARES]);
    assert_eq!(
        root.reasons(&before_path(others()[1])),
        Vec::<String>::new()
    );
}

#[test]
fn a_round_that_does_not_file_every_disputed_unit_once_asks_for_every_unit() {
    let root = fixture("uncovered");
    let missing = others()[2];
    root.remove(&before_path(missing));
    open(&root);
    for n in (1..=count()).filter(|n| *n != missing) {
        assert!(
            root.reasons(&before_path(n))
                .iter()
                .any(|reason| reason == UNCOVERED),
            "unit {n}"
        );
    }

    let root = fixture("twice");
    root.write(
        &format!("{CURRENT}/case-90.before.json"),
        &root.read(&before_path(others()[0])),
    );
    open(&root);
    assert!(
        root.reasons(&before_path(others()[1]))
            .contains(&UNCOVERED.to_string())
    );
}

#[test]
fn a_missing_or_foreign_filing_asks() {
    let root = fixture("no-filing");
    let n = others()[0];
    root.edit(&before_path(n), |object| {
        object.insert("filing".into(), Value::Null);
    });
    open(&root);
    assert_eq!(root.reasons(&before_path(n)), [SHAPE]);
    assert!(
        root.reasons(&before_path(others()[1]))
            .contains(&UNCOVERED.to_string())
    );

    let root = fixture("foreign-unit");
    let foreign = "run-a/att-099#c99";
    root.edit(&before_path(n), |object| {
        object.insert("group".into(), text(foreign));
        if let Some(Value::Object(filing)) = object.get_mut("filing") {
            filing.insert("unit".into(), text(foreign));
        }
    });
    open(&root);
    assert_eq!(root.reasons(&before_path(n)), [UNCOVERED]);
}

/// A comparison the audit plan's records do not derive grants nothing: one saved anywhere else, and one at the
/// canonical path with five units added that rank below every real one.
#[test]
fn a_forged_comparison_withholds_every_call_and_settles_nothing() {
    let forged = |root: &Root| {
        let mut compared = json(&root.read(&format!("{AUDIT}/comparison.json")));
        if let Value::Object(object) = &mut compared
            && let Some(Value::Array(units)) = object.get_mut("units")
        {
            let mut added = 0;
            let mut n = 0;
            while added < 5 {
                n += 1;
                let id = format!("forged/att-{n:03}#c{n}");
                let mut real = disputed().to_vec();
                real.push(id.clone());
                if ranked(SEED, &real).contains(&id) {
                    units.push(json(&format!(
                        r#"{{"id": "{id}", "unit": "claim", "strata": ["refuted"], "first": {{}}, "second": {{}}, "agreement": false}}"#
                    )));
                    added += 1;
                }
            }
        }
        dumps(&compared)
    };
    let (call, other) = (calls()[0], others()[0]);

    let root = fixture("forged-elsewhere");
    root.write("docs/research/forged/comparison.json", &forged(&root));
    refile(&root, "docs/research/forged/comparison.json");
    assert_eq!(root.reasons(&before_path(call)), [UNREADABLE]);
    withheld(&root, call, &[UNREADABLE]);
    assert_eq!(root.reasons(&before_path(other)), [UNREADABLE]);

    let root = fixture("forged-canonical");
    root.write(&format!("{AUDIT}/comparison.json"), &forged(&root));
    refile(&root, &format!("{AUDIT}/comparison.json"));
    assert_eq!(root.reasons(&before_path(call)), [UNREADABLE]);
    withheld(&root, call, &[UNREADABLE]);

    let summary = root.run(&["ruling", "summary", "--round", "current"]);
    assert!(
        summary.contains(&format!(
            "- {} (undetermined: the comparison it pins is not one its audit's records derive)",
            before_path(call)
        )),
        "{summary}"
    );
}

#[test]
fn an_agent_settled_row_files_only_the_settled_finding_of_its_unit() {
    let root = fixture("row");
    let n = others()[0];
    decided(&root, n, "agents", "first-error");
    open(&root);
    let compared = comparison(&root);
    assert_eq!(
        row_faults(&root, &row(&root, n, "first-error", |_| {}), &compared),
        Vec::<String>::new()
    );
    assert_eq!(
        row_faults(&root, &row(&root, n, "first-correct", |_| {}), &compared),
        [
            "reconciliation filing differs from the settled finding",
            "the row's finding is not the decision that stands for the unit",
        ]
    );
    let other = root.pin(&format!("{AUDIT}/sample.json"));
    assert_eq!(
        row_faults(&root, &row(&root, n, "first-error", |_| {}), &other),
        [
            "the comparison is not one its audit's records derive, so no agent settlement can be filed against it"
        ]
    );
    assert_eq!(
        row_faults(
            &root,
            &row(&root, n, "first-error", |object| {
                *object = without(&without(object, "receipt"), "receipt_scope");
            }),
            &compared
        ),
        ["an agent-settled reconciliation needs receipt, receipt_scope"]
    );
    assert_eq!(
        row_faults(
            &root,
            &row(&root, n, "first-error", |object| {
                object.insert("reason".into(), text("Settled."));
            }),
            &compared
        ),
        ["reason must say agents settled under policy v3"]
    );
    assert_eq!(
        row_faults(&root, &owner_row(n, "first-error"), &compared),
        [
            "agents settled the unit and no owner review changed it, so its row must carry the settlement"
        ]
    );
}

#[test]
fn the_row_of_an_owner_call_is_refused_by_the_row_check() {
    let root = fixture("row-call");
    let n = calls()[0];
    decided(&root, n, "agents", "first-error");
    open(&root);
    assert_eq!(
        row_faults(
            &root,
            &row(&root, n, "first-error", |_| {}),
            &comparison(&root)
        ),
        [
            "the unit is one of its audit round's five calls, which the owner decides; agents settle none".to_string(),
            "the unit's ruling record is not decided".to_string(),
            format!(
                "{}: its agent settlement is not valid, so it does not stand: agents cannot settle: {AUDIT_CALL}",
                before_path(n)
            ),
        ]
    );
}

/// Once a ruling record binds a unit of the audit, every row must be the decision that stands for its unit.
#[test]
fn an_unmarked_row_must_be_the_decision_that_stands() {
    let root = fixture("unmarked");
    let (owner, pending, differs) = (others()[0], others()[1], calls()[0]);
    decided(&root, owner, "owner", "first-error");
    decided(&root, differs, "owner", "first-correct");
    open(&root);
    let compared = comparison(&root);
    assert_eq!(
        row_faults(&root, &owner_row(owner, "first-error"), &compared),
        Vec::<String>::new()
    );
    assert_eq!(
        row_faults(&root, &owner_row(pending, "first-error"), &compared),
        ["the unit's ruling record is not decided"]
    );
    assert_eq!(
        row_faults(&root, &owner_row(differs, "first-error"), &compared),
        ["the row's finding is not the decision that stands for the unit"]
    );
    root.remove(&before_path(pending));
    open(&root);
    assert_eq!(
        row_faults(&root, &owner_row(pending, "first-error"), &compared),
        [
            "an audit reconciled through ruling records needs one for every row, and no ruling record decides this unit"
        ]
    );
}

/// Ruling records that bind no unit through `filing`, as every saved audit's do, leave its rows as they were checked.
#[test]
fn a_legacy_audit_without_filed_records_validates_as_before() {
    let root = fixture_with("legacy", false, false);
    let compared = comparison(&root);
    for n in [others()[0], calls()[0]] {
        assert_eq!(
            row_faults(&root, &owner_row(n, "first-correct"), &compared),
            Vec::<String>::new()
        );
    }
}

/// The owner reviews an agent settlement in the same round and decides otherwise: the records still load, the
/// type is suspended for the round, and the unit's row must carry the owner's finding.
#[test]
fn an_owner_review_in_the_round_keeps_the_tools_working() {
    let root = fixture("review");
    let (settled, later) = (others()[0], others()[1]);
    decided(&root, settled, "agents", "first-error");
    let mut review = json(&root.read(&before_path(settled)));
    if let Value::Object(object) = &mut review {
        object.insert("ruling".into(), text("case-90"));
        object.insert(
            "reviews".into(),
            json(&format!(
                r#"{{"ruling": "case-{settled}", "saved_outcome": "first-error"}}"#
            )),
        );
    }
    root.write(&format!("{CURRENT}/case-90.before.json"), &dumps(&review));
    root.edit(INDEX, |object| {
        if let Some(Value::Object(rulings)) = object.get_mut("rulings") {
            rulings.insert(
                format!("case-{settled}"),
                json(r#"{"reading": "current", "aliases": []}"#),
            );
        }
    });
    root.write(
        &format!("{CURRENT}/case-90.after.json"),
        r#"{"contract": 2, "ruling": "case-90", "before": null, "settled_by": "owner", "policy": null, "asked": 1, "outcome": "first-correct", "by_default": false, "ground": null, "miss": {"cause": "owner-weighs-differently", "note": "The owner weighed it differently."}, "lesson": null}"#,
    );
    open(&root);
    assert!(summary::records(&root.repo()).is_ok());
    root.run(&["ruling", "summary", "--round", "current"]);
    root.run(&[
        "ruling",
        "check",
        &before_path(settled),
        &after_path(settled),
    ]);
    assert_eq!(root.reasons(&before_path(later)), [SUSPENDED]);
    let compared = comparison(&root);
    assert_eq!(
        row_faults(&root, &owner_row(settled, "first-correct"), &compared),
        Vec::<String>::new()
    );
    assert!(
        row_faults(
            &root,
            &row(&root, settled, "first-error", |_| {}),
            &compared
        )
        .contains(&"the row's finding is not the decision that stands for the unit".to_string())
    );
}

#[test]
fn the_summary_lists_the_ranked_calls_of_a_round_under_policy_v3() {
    let root = fixture("summary");
    let summary = root.run(&["ruling", "summary", "--round", "current"]);
    let listed: Vec<&str> = summary
        .lines()
        .skip_while(|line| *line != "Five audit calls:")
        .skip(1)
        .take_while(|line| line.starts_with("- "))
        .collect();
    let wanted: Vec<String> = ranked(SEED, disputed())
        .iter()
        .map(|call| {
            let n = (1..=count()).find(|n| unit(*n) == *call).unwrap();
            format!("- {}", before_path(n))
        })
        .collect();
    assert_eq!(listed, wanted, "{summary}");

    let root = fixture("summary-unfiled");
    let n = others()[0];
    root.edit(&before_path(n), |object| {
        object.insert("filing".into(), Value::Null);
    });
    open(&root);
    let summary = root.run(&["ruling", "summary", "--round", "current"]);
    assert!(
        summary.contains(&format!(
            "- {} (undetermined: its filing pins no comparison)",
            before_path(n)
        )),
        "{summary}"
    );
}

/// An agent writes a review of a unit the owner decided, settled by agents with another outcome: the review never
/// stands, so the owner's row stays valid and a row with the agent's outcome is refused, naming the review.
#[test]
fn an_agent_settled_review_of_an_owner_decision_does_not_stand() {
    let root = fixture("agent-review");
    let n = others()[0];
    decided(&root, n, "owner", "first-error");
    let mut review = json(&root.read(&before_path(n)));
    if let Value::Object(object) = &mut review {
        object.insert("ruling".into(), text("case-90"));
        object.insert(
            "reviews".into(),
            json(&format!(
                r#"{{"ruling": "case-{n}", "saved_outcome": "first-error"}}"#
            )),
        );
    }
    root.write(&format!("{CURRENT}/case-90.before.json"), &dumps(&review));
    root.edit(INDEX, |object| {
        if let Some(Value::Object(rulings)) = object.get_mut("rulings") {
            rulings.insert(
                format!("case-{n}"),
                json(r#"{"reading": "current", "aliases": []}"#),
            );
        }
    });
    root.write(
        &format!("{CURRENT}/case-90.after.json"),
        &format!(
            r#"{{"contract": 2, "ruling": "case-90", "before": null, "settled_by": "agents", "policy": {}, "asked": 0, "outcome": "first-correct", "by_default": false, "ground": null, "miss": {{"cause": "slip", "note": "An agent wrote this review."}}, "lesson": null}}"#,
            dumps(&root.pin(V3))
        ),
    );
    open(&root);
    assert!(summary::records(&root.repo()).is_err());
    let compared = comparison(&root);
    assert_eq!(
        row_faults(&root, &owner_row(n, "first-error"), &compared),
        Vec::<String>::new()
    );
    assert_eq!(
        row_faults(&root, &owner_row(n, "first-correct"), &compared),
        [
            "the row's finding is not the decision that stands for the unit".to_string(),
            format!(
                "{CURRENT}/case-90.before.json: a review of a saved ruling settled by agents is never valid, so it does not stand"
            ),
        ]
    );
}

/// A reconciliation record of the same unit id that does not file it against this audit's comparison, whether it
/// files nothing or another comparison, neither blocks nor stands in for the record that does.
#[test]
fn a_record_without_the_exact_filing_binds_no_row() {
    let other = |root: &Root, filing: Value| {
        let rulings = "docs/research/other-2026-01-20/rulings";
        root.write(
            &format!("{rulings}/round.json"),
            r#"{"round": "other", "opened": "2026-01-20", "closed": true, "opening": null, "no_record": {}}"#,
        );
        let n = others()[0];
        let mut record = json(&root.read(&before_path(n)));
        if let Value::Object(object) = &mut record {
            object.insert("round_opening".into(), Value::Null);
            object.insert("filing".into(), filing);
        }
        let before = format!("{rulings}/case-1.before.json");
        root.write(&before, &dumps(&record));
        root.write(
            &format!("{rulings}/case-1.after.json"),
            &format!(
                r#"{{"contract": 2, "ruling": "case-{n}", "before": {}, "settled_by": "owner", "policy": null, "asked": 1, "outcome": "first-correct", "by_default": false, "ground": null, "miss": {{"cause": "slip", "note": "Another audit."}}, "lesson": null}}"#,
                dumps(&root.pin(&before))
            ),
        );
    };
    let n = others()[0];
    for (name, filing) in [
        ("other-unfiled", "null".to_string()),
        ("other-audit", String::new()),
    ] {
        let root = fixture(name);
        let filing = if filing.is_empty() {
            filing_value(&root.pin(&format!("{AUDIT}/sample.json")), &unit(n))
        } else {
            json(&filing)
        };
        other(&root, filing);
        let compared = comparison(&root);
        assert_eq!(
            row_faults(&root, &owner_row(n, "first-correct"), &compared),
            ["the unit's ruling record is not decided"],
            "{name}"
        );
        root.remove(&before_path(n));
        open(&root);
        assert_eq!(
            row_faults(&root, &owner_row(n, "first-correct"), &compared),
            [
                "an audit reconciled through ruling records needs one for every row, and no ruling record decides this unit"
            ],
            "{name}"
        );
        let root = fixture(&format!("{name}-decided"));
        let filing = json(&dumps(&filing_value(
            &root.pin(&format!("{AUDIT}/sample.json")),
            &unit(n),
        )));
        other(
            &root,
            if name == "other-unfiled" {
                Value::Null
            } else {
                filing
            },
        );
        decided(&root, n, "owner", "first-error");
        open(&root);
        assert_eq!(
            row_faults(&root, &owner_row(n, "first-error"), &comparison(&root)),
            Vec::<String>::new(),
            "{name}"
        );
    }
}

/// The plan's own round is accepted only as the draw redrawn from the inventory; a sample, its grades and a
/// comparison saved in the plan seed's directory without one grant nothing, so every unit asks.
#[test]
fn a_plan_seed_sample_that_does_not_redraw_makes_every_unit_ask() {
    let root = fixture_with("forged-plan", true, true);
    for n in 1..=count() {
        assert!(
            root.reasons(&before_path(n))
                .contains(&UNREADABLE.to_string()),
            "unit {n}"
        );
    }
    withheld(&root, calls()[0], &[UNREADABLE]);
    withheld(&root, others()[0], &[UNREADABLE]);
}

/// A plan round that `conclude` assessed is anchored by the evidence it wrote to the plan file, so a sampled grade
/// corrected afterwards leaves its agent-settled rows valid; a `conclusion.json` the plan file does not pin is no
/// anchor, and the round must still redraw.
#[test]
fn an_assessed_plan_round_needs_no_redraw_and_an_unpinned_conclusion_is_no_anchor() {
    for anchored in [true, false] {
        let root = fixture_with(&format!("assessed-{anchored}"), true, true);
        std::fs::copy(
            repository().join(AUDIT).join("conclusion.json"),
            root.0.join(AUDIT).join("conclusion.json"),
        )
        .unwrap();
        root.write(
            "bench/grading/current/grades.json",
            r#"{"batches": [], "note": "a sampled grade corrected after the conclusion"}"#,
        );
        if anchored {
            let evidence = Value::Array(vec![
                root.pin(&format!("{AUDIT}/sample.json")),
                comparison(&root),
                root.pin(&format!("{AUDIT}/conclusion.json")),
            ]);
            root.edit(AUDITS, |object| {
                object.insert("state".into(), text("assessed"));
                object.insert("evidence".into(), evidence);
            });
        }
        let n = others()[0];
        decided(&root, n, "agents", "first-error");
        open(&root);
        let faults = row_faults(
            &root,
            &row(&root, n, "first-error", |_| {}),
            &comparison(&root),
        );
        if anchored {
            assert_eq!(faults, Vec::<String>::new());
        } else {
            assert_eq!(
                faults[0],
                "the comparison is not one its audit's records derive, so no agent settlement can be filed against it"
            );
        }
    }
}
