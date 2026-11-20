//! `evaluator-audit conclude` saves the round's own copy of the grades its sample pins whenever it writes a
//! conclusion, so that grades corrected afterwards leave the round's comparison derivable. These cases replay the
//! evaluator-audit corpus's two conclusion cases up to their first `conclude`.

use bench_tools::delegation::v3;
use bench_tools::pyjson::{self, Dumps, Duplicates, Object, Value};
use bench_tools::ruling::Repo;
use bench_tools::sha256::hex_digest;
use std::path::{Path, PathBuf};
use std::process::Command;

const CORPUS: &str = "tools/behavior_contracts/evaluator-audit.v1.json";
const PASSES: &str =
    "evaluator-audit-a-reconciled-audit-within-tolerance-is-assessed-with-pinned-evidence";
const FAILS: &str = "evaluator-audit-a-sampled-stratum-over-its-tolerance-fails-the-round-and-a-new-seed-draws-again";
const AUDITS: &str = "bench/grading/current/audits.json";
const ROUND: &str = "bench/grading/current/audit/fixture";
const LIVE: &str = "bench/grading/current/grades.json";
const COPY: &str = "bench/grading/current/audit/fixture/grades.json";
const CONCLUSION: &str = "bench/grading/current/audit/fixture/conclusion.json";
const RECONCILIATION: &str = "reconciliation.json";
const CONCLUDE: [&str; 3] = ["conclude", "--reconciliation", RECONCILIATION];
const ASSESSED: &str = "audits.json is assessed: Every stratum was drawn, assessed a second time and reconciled, and \
                        every sampled stratum is within its tolerance. conclusion.json holds each stratum's agreements \
                        and confirmed first-grader errors.\n";
const FAILED: &str = "this round failed and audits.json stays unassessed; outside tolerance: suggestion. Grade the \
                      affected batches again, record the next seed in audits.json and draw every stratum again\n";
const CHANGED: &str = "grades.json changed since the sample was drawn, so its first assessments are no longer the \
                       grades on record; correct grades after the conclusion, or draw a fresh sample under a new \
                       recorded seed\n";

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn json(text: &str) -> Value {
    pyjson::loads(text, Duplicates::KeepLast).unwrap()
}

fn dumps(value: &Value) -> String {
    pyjson::dumps(value, &Dumps::default())
}

fn field<'a>(value: &'a Value, name: &str) -> &'a Value {
    value.as_object().unwrap().get(name).unwrap()
}

fn string<'a>(value: &'a Value, name: &str) -> &'a str {
    field(value, name).as_str().unwrap()
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
    fn bytes(&self, path: &str) -> Vec<u8> {
        std::fs::read(self.0.join(path)).unwrap()
    }

    fn exists(&self, path: &str) -> bool {
        self.0.join(path).exists()
    }

    fn write(&self, path: &str, bytes: &[u8]) {
        let file = self.0.join(path);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(file, bytes).unwrap();
    }

    fn edit(&self, path: &str, change: impl FnOnce(&mut Object)) {
        let mut value = json(&String::from_utf8(self.bytes(path)).unwrap());
        let Value::Object(object) = &mut value else {
            panic!("{path} is not an object");
        };
        change(object);
        self.write(path, format!("{}\n", dumps(&value)).as_bytes());
    }

    fn populate(&self, entries: &[Value]) {
        for entry in entries {
            let path = self
                .0
                .join(String::from_utf8(decode(string(entry, "path_b64"))).unwrap());
            match string(entry, "kind") {
                "directory" => std::fs::create_dir_all(&path).unwrap(),
                "file" => {
                    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                    std::fs::write(&path, decode(string(entry, "bytes_b64"))).unwrap();
                }
                other => panic!("no fixture entry of kind {other}"),
            }
        }
    }

    /// Runs a bench-tools command in the root and gives its exit status and stdout.
    fn run(&self, arguments: &[&str]) -> (i32, String) {
        let output = Command::new(env!("CARGO_BIN_EXE_bench-tools"))
            .args(arguments)
            .current_dir(&self.0)
            .env("HOME", self.0.join(".home"))
            .output()
            .unwrap();
        (
            output.status.code().unwrap(),
            String::from_utf8(output.stdout).unwrap(),
        )
    }

    fn audit(&self, arguments: &[&str]) -> (i32, String) {
        let mut all = vec!["evaluator-audit"];
        all.extend_from_slice(arguments);
        self.run(&all)
    }

    fn pin(&self, path: &str) -> Value {
        json(&format!(
            r#"{{"path": "{path}", "sha256": "{}"}}"#,
            hex_digest(&self.bytes(path))
        ))
    }

    /// The SHA-256 of the grades the round's sample pins.
    fn pinned(&self) -> String {
        let sample = json(&String::from_utf8(self.bytes(&format!("{ROUND}/sample.json"))).unwrap());
        string(field(&sample, "grades"), "sha256").to_string()
    }

    /// Whether policy v3 reads the round's comparison as its audit's, the check that lets an agent-settled
    /// reconciliation of the round stand. Each call runs on a new thread, because the check is remembered per thread.
    fn derives(&self) -> bool {
        let root = self.0.clone();
        let comparison = self.pin(&format!("{ROUND}/comparison.json"));
        std::thread::spawn(move || {
            let repo = Repo::new(root.as_os_str().as_encoded_bytes());
            v3::audit(&repo, &comparison).unwrap().is_some()
        })
        .join()
        .unwrap()
    }

    /// Replaces one field of the first sampled batch in the live grades, as grading it again would.
    fn correct_a_sampled_grade(&self) {
        self.edit(LIVE, |grades| {
            let Some(Value::Array(batches)) = grades.get_mut("batches") else {
                panic!("the grades hold no batches");
            };
            let Value::Object(batch) = &mut batches[0] else {
                panic!("a batch is not an object");
            };
            batch.insert("corrected".into(), Value::Bool(true));
        });
    }

    /// Adds a batch that no unit of the round samples to the live grades.
    fn file_a_later_batch(&self) {
        self.edit(LIVE, |grades| {
            let Some(Value::Array(batches)) = grades.get_mut("batches") else {
                panic!("the grades hold no batches");
            };
            let mut later = batches[0].clone();
            if let Value::Object(batch) = &mut later {
                batch.insert("target".into(), Value::Str("t-filed-later".into()));
            }
            batches.push(later);
        });
    }
}

/// The corpus case's root after every step before its first `conclude`: drawn, second-assessed, compared, and with
/// the case's reconciliation saved at `reconciliation.json`.
fn round(name: &str, case: &str) -> Root {
    let corpus = json(&std::fs::read_to_string(repository().join(CORPUS)).unwrap());
    let case = field(&corpus, "cases")
        .as_array()
        .unwrap()
        .iter()
        .find(|found| string(found, "id") == case)
        .unwrap();
    let root = Root(std::env::temp_dir().join(format!(
        "evaluator-audit-conclude-{name}-{}",
        std::process::id()
    )));
    let _ = std::fs::remove_dir_all(&root.0);
    root.populate(field(case, "initial").as_array().unwrap());
    std::fs::create_dir_all(root.0.join(".home")).unwrap();
    for step in field(case, "steps").as_array().unwrap() {
        if let Some(Value::Array(entries)) = step.as_object().unwrap().get("write") {
            root.populate(entries);
            continue;
        }
        let argv: Vec<&str> = field(step, "argv")
            .as_array()
            .unwrap()
            .iter()
            .map(|argument| argument.as_str().unwrap())
            .collect();
        if argv[0] == "conclude" {
            break;
        }
        let (status, output) = root.audit(&argv);
        assert_eq!(status, 0, "{argv:?}: {output}");
    }
    root
}

fn reconciled(root: &Root, finding: &str) {
    root.edit(RECONCILIATION, |reconciliation| {
        let Some(Value::Array(decisions)) = reconciliation.get_mut("decisions") else {
            panic!("the reconciliation holds no decisions");
        };
        if let Value::Object(decision) = &mut decisions[0] {
            decision.insert("finding".into(), Value::Str(finding.into()));
        }
    });
}

#[test]
fn a_passing_conclusion_saves_the_pinned_grades_beside_it() {
    let root = round("passes", PASSES);
    let live = root.bytes(LIVE);
    assert!(!root.exists(COPY));
    assert_eq!(root.audit(&CONCLUDE), (0, ASSESSED.to_string()));
    assert!(root.exists(CONCLUSION));
    assert_eq!(root.bytes(COPY), live);
    assert_eq!(hex_digest(&root.bytes(COPY)), root.pinned());
}

#[test]
fn a_failing_conclusion_saves_the_pinned_grades_too() {
    let root = round("fails", FAILS);
    let live = root.bytes(LIVE);
    assert_eq!(root.audit(&CONCLUDE), (1, FAILED.to_string()));
    assert!(root.exists(CONCLUSION));
    assert_eq!(root.bytes(COPY), live);
    assert_eq!(hex_digest(&root.bytes(COPY)), root.pinned());
}

#[test]
fn a_conclude_that_concludes_nothing_saves_no_copy() {
    let root = round("undetermined", PASSES);
    reconciled(&root, "undetermined");
    let (status, output) = root.audit(&CONCLUDE);
    assert_eq!(status, 1);
    assert!(
        output.starts_with(
            "1 disagreement(s) are undetermined, so nothing was saved; settle them and conclude again: "
        ),
        "{output}"
    );
    assert!(!root.exists(COPY));
    assert!(!root.exists(CONCLUSION));

    let root = round("refused", PASSES);
    root.edit(RECONCILIATION, |reconciliation| {
        if let Some(Value::Array(decisions)) = reconciliation.get_mut("decisions") {
            decisions.truncate(1);
        }
    });
    let (status, output) = root.audit(&CONCLUDE);
    assert_eq!(status, 1);
    assert!(
        output.ends_with(": a disagreement without a reconciliation\n"),
        "{output}"
    );
    assert!(!root.exists(COPY));
    assert!(!root.exists(CONCLUSION));

    let root = round("again", PASSES);
    assert_eq!(root.audit(&CONCLUDE).0, 0);
    std::fs::remove_file(root.0.join(COPY)).unwrap();
    let (status, output) = root.audit(&CONCLUDE);
    assert_eq!(status, 1);
    assert!(
        output.ends_with(
            "/bench/grading/current/audit/fixture/reconciliation.json is already saved; an audit record is \
             written once\n"
        ),
        "{output}"
    );
    assert!(!root.exists(COPY));
}

#[test]
fn a_copy_the_round_already_holds_is_left_as_it_is() {
    let root = round("kept", PASSES);
    root.write(COPY, &root.bytes(LIVE));
    let kept = root.bytes(COPY);
    root.file_a_later_batch();
    assert_ne!(root.bytes(LIVE), kept);
    assert_eq!(root.audit(&CONCLUDE), (0, ASSESSED.to_string()));
    assert_eq!(root.bytes(COPY), kept);
}

#[test]
fn without_a_copy_the_steps_refuse_live_grades_that_differ_from_the_pin() {
    let root = round("later", PASSES);
    root.file_a_later_batch();
    assert_eq!(root.audit(&CONCLUDE), (1, CHANGED.to_string()));
    assert_eq!(
        root.audit(&[
            "claims",
            "--run",
            "runs/2026-01-01-grade-test",
            "--target",
            "t-grade-1",
            "--out",
            "claim-list-1.json"
        ]),
        (1, CHANGED.to_string())
    );
    assert!(!root.exists(COPY));
    assert!(!root.exists(CONCLUSION));
}

/// After an assessed round's sampled grade is corrected, its comparison derives from the copy `conclude` saved, and
/// from nothing once the copy is gone, as it did for round 4 before its copy was added by hand.
#[test]
fn an_assessed_round_derives_from_its_copy_after_a_correction() {
    let root = round("assessed", PASSES);
    assert_eq!(root.audit(&CONCLUDE).0, 0);
    assert!(root.derives());
    root.correct_a_sampled_grade();
    assert!(root.derives());
    std::fs::remove_file(root.0.join(COPY)).unwrap();
    assert!(!root.derives());
}

/// A failed round that the plan file still records as its own must draw again from the live grades, which the
/// correction changed, so the copy does not make it derivable; once the plan file records it as superseded, it
/// derives from its copy.
#[test]
fn a_failed_plan_round_derives_after_a_correction_only_once_superseded() {
    let root = round("failed", FAILS);
    assert_eq!(root.audit(&CONCLUDE).0, 1);
    assert!(root.derives());
    root.correct_a_sampled_grade();
    assert!(!root.derives());
    let sample = root.pin(&format!("{ROUND}/sample.json"));
    root.edit(AUDITS, |audits| {
        let plan = audits.get("plan").unwrap().clone();
        let closed = json(&format!(
            r#"{{"plan": {}, "evidence": [{}]}}"#,
            dumps(&plan),
            dumps(&sample)
        ));
        audits.insert("superseded".into(), Value::Array(vec![closed]));
        if let Some(Value::Object(plan)) = audits.get_mut("plan") {
            plan.insert("seed".into(), Value::Str("fixture-2".into()));
        }
    });
    assert!(root.derives());
}
