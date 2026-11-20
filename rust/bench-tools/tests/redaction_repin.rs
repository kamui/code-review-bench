//! `bench-tools redaction repin` and the checks of the record it writes, on a git repository holding the grading
//! fixture with one graded batch whose claim and eligibility decision pin one evidence file. Every value is made up.

#[path = "support/grading_fixture.rs"]
#[allow(dead_code, reason = "this file builds only the proposed fixture")]
mod grading_fixture;

use bench_tools::pyjson::{self, Dumps, Duplicates, Value};
use bench_tools::sha256::hex_digest;
use grading_fixture::{RUN, TARGET, Variant, build};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const VALUES: &str = r#"{"schema_version": 1, "email": ["person@example.com"], "hosts": {"examplehost": "host-01-xyz"}}"#;
const MORE_VALUES: &str = r#"{"schema_version": 1, "email": ["person@example.com"], "account": ["acct-0000-example"], "hosts": {"examplehost": "host-01-xyz"}}"#;
const EVIDENCE: &str = "docs/research/fixture/RESULTS.md";
const RECEIPT: &str = "bench/grading/rulings/redaction-approval.md";
const CURRENT: &str = "bench/grading/current";
const RECORD: &str = "bench/grading/current/redacted-evidence.json";
const CLAIMS: &str = "bench/grading/current/claims.json";
const DECISIONS: &str = "bench/grading/current/adjudications.json";
const GRADES: &str = "bench/grading/current/grades.json";
const REVIEW: &str = "bench/runs/2026-01-01-grade-test/attempts/att-001/normalized.json";
const TEXT: &str = "# Fixture run results\n\nThe run deployed from examplehost as acct-0000-example.\nMail went to person@example.com after the build.\n\nThe cache key keeps the visitor path,\nso a second visitor reads the first visitor's page.\n";
const REDACTED: &str = "# Fixture run results\n\nThe run deployed from host-01-xyz as acct-0000-example.\nMail went to [redacted-email] after the build.\n\nThe cache key keeps the visitor path,\nso a second visitor reads the first visitor's page.\n";

fn s(text: &str) -> Value {
    Value::Str(text.to_string())
}

fn o(members: Vec<(&str, Value)>) -> Value {
    Value::Object(
        members
            .into_iter()
            .map(|(key, value)| (key.to_string(), value))
            .collect(),
    )
}

fn parse(text: &[u8]) -> Value {
    pyjson::loads(std::str::from_utf8(text).unwrap(), Duplicates::Refuse).unwrap()
}

fn indented(value: &Value, indent: Option<usize>) -> Vec<u8> {
    let mut text = pyjson::dumps(
        value,
        &Dumps {
            indent,
            ..Dumps::default()
        },
    );
    text.push('\n');
    text.into_bytes()
}

fn member<'a>(value: &'a mut Value, path: &[&str]) -> &'a mut Value {
    let mut value = value;
    for step in path {
        value = match value {
            Value::Object(object) => object.get_mut(step).unwrap(),
            Value::Array(members) => &mut members[step.parse::<usize>().unwrap()],
            _ => panic!("{step}"),
        };
    }
    value
}

fn field(value: &Value, path: &[&str]) -> String {
    let mut value = value;
    for step in path {
        value = match value {
            Value::Object(object) => object.get(step).unwrap(),
            Value::Array(members) => &members[step.parse::<usize>().unwrap()],
            _ => panic!("{step}"),
        };
    }
    value.py_str()
}

fn quiet_git(root: &Path, arguments: &[&str]) -> Output {
    Command::new("git")
        .arg("-C")
        .arg(root)
        .args(arguments)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_AUTHOR_NAME", "Example")
        .env("GIT_AUTHOR_EMAIL", "example@example.com")
        .env("GIT_COMMITTER_NAME", "Example")
        .env("GIT_COMMITTER_EMAIL", "example@example.com")
        .output()
        .unwrap()
}

fn git(root: &Path, arguments: &[&str]) -> String {
    let output = quiet_git(root, arguments);
    assert!(output.status.success(), "git {arguments:?}: {output:?}");
    String::from_utf8(output.stdout).unwrap().trim().to_string()
}

struct Fixture {
    root: PathBuf,
    values: PathBuf,
}

impl Fixture {
    /// The grading fixture with a claim and an eligibility decision on the graded batch that pin [`EVIDENCE`], its
    /// grade saved at the batch's fingerprint, committed. `claims.json` and `adjudications.json` are written in
    /// layouts the tools never write, so that a rewrite that re-serializes them would show.
    fn new(label: &str) -> Self {
        let base = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("redaction-repin-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let root = base.join("repository");
        let values = base.join("values.json");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(&values, VALUES).unwrap();
        let fixture = Self { root, values };
        for (path, bytes) in build(Variant::Proposed).files {
            fixture.write(&path, &bytes);
        }
        fixture.write(EVIDENCE, TEXT.as_bytes());
        fixture.write(
            "docs/research/fixture/older-pins.json",
            &grading_fixture::json(&o(vec![(
                "files",
                Value::Array(vec![o(vec![
                    ("path", s(EVIDENCE)),
                    ("sha256", s(&"e".repeat(64))),
                ])]),
            )])),
        );
        fixture.write(
            RECEIPT,
            b"# Redaction approval\n\nThe owner approves redaction repin for the fixture's run results.\n",
        );
        let evidence = fixture.pin(EVIDENCE);
        let mut decisions = parse(&fixture.read(DECISIONS));
        *member(&mut decisions, &["decisions", "0", "evidence"]) =
            Value::Array(vec![evidence.clone()]);
        fixture.write(DECISIONS, &indented(&decisions, None));
        let references = format!("{CURRENT}/references.json");
        let mut reference = parse(&fixture.read(&references));
        *member(
            &mut reference,
            &[
                "targets",
                "0",
                "families",
                "0",
                "eligibility",
                "adjudication",
            ],
        ) = s("AD-GT-t1-eligibility");
        fixture.write(&references, &grading_fixture::json(&reference));
        let revision = field(
            &parse(&fixture.read(&references)),
            &["targets", "0", "revision", "packet_sha256"],
        );
        let claim = o(vec![
            ("id", s("CL-t-grade-1-close")),
            ("target", s(TARGET)),
            (
                "revision",
                o(vec![
                    ("head", s(&"1".repeat(40))),
                    ("base_sha", s(&"1".repeat(40))),
                    ("packet_sha256", s(&revision)),
                    ("diff_manifest_sha256", s(&"c".repeat(64))),
                ]),
            ),
            (
                "claim",
                o(vec![
                    ("trigger", s("Concurrent close")),
                    ("mechanism", s("Unsynchronized close")),
                    ("consequence", s("A send can panic")),
                    ("change_relation", s("Introduced")),
                    ("settlement_question", s("Is close serialized?")),
                ]),
            ),
            (
                "evidence",
                Value::Array(vec![o(vec![
                    ("source", evidence),
                    ("stance", s("supports")),
                    ("summary", s("The run shows the race")),
                ])]),
            ),
            (
                "links",
                Value::Array(vec![o(vec![
                    ("review", fixture.pin(REVIEW)),
                    ("attempt_id", s("att-001")),
                    ("item_id", s("item-0")),
                    ("relation", s("equivalent")),
                    ("reason", s("Same trigger and mechanism")),
                ])]),
            ),
            ("adjudication", Value::Null),
            ("family_id", Value::Null),
        ]);
        let claims = o(vec![
            ("schema_version", Value::Int("1".into())),
            ("claims", Value::Array(vec![claim])),
        ]);
        fixture.write(CLAIMS, &indented(&claims, Some(4)));
        let inventory = fixture.root.join(CURRENT).join("inventory.json");
        let (code, text) =
            fixture.current_grading(&["inventory", "--out", inventory.to_str().unwrap()]);
        assert_eq!(code, 0, "{text}");
        fixture.save_grade();
        git(&fixture.root, &["init", "-q", "-b", "main"]);
        fixture.commit("fixture");
        fixture
    }

    fn write(&self, path: &str, bytes: &[u8]) {
        let path = self.root.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }

    fn read(&self, path: &str) -> Vec<u8> {
        std::fs::read(self.root.join(path)).unwrap()
    }

    fn pin(&self, path: &str) -> Value {
        o(vec![
            ("path", s(path)),
            ("sha256", s(&hex_digest(&self.read(path)))),
        ])
    }

    fn commit(&self, message: &str) {
        git(&self.root, &["add", "-A"]);
        git(&self.root, &["commit", "-q", "-m", message]);
    }

    fn run(&self, values: &Path, arguments: &[&str]) -> (i32, String, String) {
        let output = Command::new(env!("CARGO_BIN_EXE_bench-tools"))
            .args(arguments)
            .current_dir(&self.root)
            .env("BENCH_REDACTION_VALUES", values)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .output()
            .unwrap();
        let text = |bytes: &[u8]| String::from_utf8_lossy(bytes).into_owned();
        let (stdout, stderr) = (text(&output.stdout), text(&output.stderr));
        for value in ["examplehost", "person@", "acct-0000"] {
            assert!(
                !stdout.contains(value) && !stderr.contains(value),
                "a value was printed: {stdout} {stderr}"
            );
        }
        (output.status.code().unwrap(), stdout, stderr)
    }

    fn repin(&self, arguments: &[&str]) -> (i32, String, String) {
        let mut all = vec!["redaction", "repin"];
        all.extend_from_slice(arguments);
        self.run(&self.values, &all)
    }

    fn current_grading(&self, arguments: &[&str]) -> (i32, String) {
        let mut all = vec!["current-grading"];
        all.extend_from_slice(arguments);
        all.extend_from_slice(&["--root", self.root.to_str().unwrap()]);
        let (code, stdout, stderr) = self.run(&self.values, &all);
        (code, stdout + &stderr)
    }

    fn check(&self) -> (i32, String) {
        self.current_grading(&["check"])
    }

    /// The batch's `(input_fingerprint, state)` as `current-grading fingerprints` reports it.
    fn batch(&self) -> (String, String) {
        let (code, text) = self.current_grading(&["fingerprints"]);
        assert_eq!(code, 0, "{text}");
        let rows = parse(text.as_bytes());
        let row = &rows.as_array().unwrap()[0];
        (field(row, &["input_fingerprint"]), field(row, &["state"]))
    }

    /// Saves a manual grade of the batch at its current fingerprint, under the selected remedy policy.
    fn save_grade(&self) {
        let (fingerprint, state) = self.batch();
        assert_eq!(state, "missing");
        self.grade_at(&fingerprint);
        assert_eq!(self.batch(), (fingerprint, "current".to_string()));
    }

    /// Saves a manual grade of the batch at `fingerprint`, under the selected remedy policy.
    fn grade_at(&self, fingerprint: &str) {
        let directory = format!("{CURRENT}/assessments/{RUN}/{TARGET}/assessment-1");
        let verdicts = format!("{directory}/verdicts.json");
        let receipt = format!("{directory}/receipt.json");
        self.write(&verdicts, b"{\"reviews\": {}}\n");
        let pointer = parse(&self.read(&format!("{CURRENT}/remedy-policy.json")));
        let saved = o(vec![
            ("contract", s("current-assessment-receipt/v1")),
            ("run", s(&format!("runs/{RUN}"))),
            ("target", s(TARGET)),
            ("input_fingerprint", s(fingerprint)),
            ("mapped_at", s("2026-01-01T00:10:00Z")),
            (
                "provenance",
                o(vec![("kind", s("manual")), ("identity", s("fixture"))]),
            ),
            ("verdicts_sha256", s(&hex_digest(&self.read(&verdicts)))),
            (
                "remedy_policy",
                o(vec![("role", s("first")), ("source", pointer)]),
            ),
        ]);
        self.write(&receipt, &grading_fixture::json(&saved));
        let grades = o(vec![
            ("schema_version", Value::Int("1".into())),
            (
                "batches",
                Value::Array(vec![o(vec![
                    ("run", s(&format!("runs/{RUN}"))),
                    ("target", s(TARGET)),
                    ("input_fingerprint", s(fingerprint)),
                    ("verdicts", s("current-verdicts/v3")),
                    (
                        "assessor",
                        o(vec![
                            ("kind", s("manual")),
                            ("receipt", self.pin(&receipt)),
                            ("verdicts", self.pin(&verdicts)),
                        ]),
                    ),
                    ("reviews", Value::Array(vec![])),
                ])]),
            ),
        ]);
        self.write(GRADES, &grading_fixture::json(&grades));
    }

    fn receipt_path(&self) -> String {
        format!("{CURRENT}/assessments/{RUN}/{TARGET}/assessment-1/receipt.json")
    }

    /// Replaces the redacted evidence with `text` and points the record's last entry and both pins at it by hand.
    fn swap_evidence(&self, text: &str) {
        let old = hex_digest(&self.read(EVIDENCE));
        let new = hex_digest(text.as_bytes());
        self.write(EVIDENCE, text.as_bytes());
        for path in [CLAIMS, DECISIONS, RECORD] {
            let bytes = String::from_utf8(self.read(path)).unwrap();
            self.write(path, bytes.replace(&old, &new).as_bytes());
        }
    }

    fn edit_record(&self, change: impl FnOnce(&mut Value)) {
        let mut record = parse(&self.read(RECORD));
        change(&mut record);
        self.write(RECORD, &grading_fixture::json(&record));
    }
}

const VALID: &str = "current evidence valid; 0/2 admitted reviews assessed\n";

fn repinned(fixture: &Fixture) {
    let (code, stdout, stderr) = fixture.repin(&[EVIDENCE, "--receipt", RECEIPT]);
    assert_eq!(
        (code, stdout.as_str(), stderr.as_str()),
        (
            0,
            "redacted: docs/research/fixture/RESULTS.md (1 [redacted-email], 1 host-01-xyz)\n\
             repinned: bench/grading/current/adjudications.json (1 pin)\n\
             repinned: bench/grading/current/claims.json (1 pin)\n\
             recorded: bench/grading/current/redacted-evidence.json\n",
            ""
        )
    );
}

#[test]
fn repin_keeps_every_saved_fingerprint_and_changes_only_the_two_hashes() {
    let fixture = Fixture::new("whole");
    assert_eq!(fixture.check(), (0, VALID.to_string()));
    let saved = fixture.batch();
    let kept: Vec<Vec<u8>> = [GRADES, fixture.receipt_path().as_str()]
        .iter()
        .map(|path| fixture.read(path))
        .collect();
    let documents: Vec<Vec<u8>> = [CLAIMS, DECISIONS]
        .iter()
        .map(|path| fixture.read(path))
        .collect();
    let blob = git(&fixture.root, &["rev-parse", &format!("HEAD:{EVIDENCE}")]);

    repinned(&fixture);

    assert_eq!(fixture.read(EVIDENCE), REDACTED.as_bytes());
    assert_eq!(fixture.check(), (0, VALID.to_string()));
    assert_eq!(fixture.batch(), saved);
    let after: Vec<Vec<u8>> = [GRADES, fixture.receipt_path().as_str()]
        .iter()
        .map(|path| fixture.read(path))
        .collect();
    assert_eq!(after, kept);
    let (old, new) = (hex_digest(TEXT.as_bytes()), hex_digest(REDACTED.as_bytes()));
    for (path, before) in [CLAIMS, DECISIONS].iter().zip(&documents) {
        let before = String::from_utf8(before.clone()).unwrap();
        assert_eq!(before.matches(&old).count(), 1, "{path}");
        assert_eq!(
            String::from_utf8(fixture.read(path)).unwrap(),
            before.replace(&old, &new),
            "{path}"
        );
    }
    let record = parse(&fixture.read(RECORD));
    let entry = |path: &[&str]| {
        let mut full = vec!["entries", "0"];
        full.extend_from_slice(path);
        field(&record, &full)
    };
    assert_eq!(entry(&["path"]), EVIDENCE);
    assert_eq!(entry(&["before", "sha256"]), old);
    assert_eq!(entry(&["before", "blob"]), blob);
    assert_eq!(entry(&["after", "sha256"]), new);
    assert_eq!(entry(&["placeholders", "0", "text"]), "[redacted-email]");
    assert_eq!(entry(&["placeholders", "1", "text"]), "host-01-xyz");
    assert_eq!(entry(&["placeholders", "1", "count"]), "1");
    assert_eq!(entry(&["receipt", "path"]), RECEIPT);
    assert_eq!(
        entry(&["receipt", "sha256"]),
        hex_digest(&fixture.read(RECEIPT))
    );
}

#[test]
fn the_entry_is_what_keeps_the_grades_current() {
    let fixture = Fixture::new("entry");
    let pinned_at_old: Vec<Vec<u8>> = [CLAIMS, DECISIONS]
        .iter()
        .map(|path| fixture.read(path))
        .collect();
    repinned(&fixture);
    let record = fixture.read(RECORD);

    std::fs::remove_file(fixture.root.join(RECORD)).unwrap();
    let (code, text) = fixture.check();
    assert_eq!(code, 1);
    assert!(text.contains("grade fingerprint is stale"), "{text}");

    fixture.write(RECORD, &record);
    for (path, bytes) in [CLAIMS, DECISIONS].iter().zip(&pinned_at_old) {
        fixture.write(path, bytes);
    }
    let (code, text) = fixture.check();
    assert_eq!(
        (code, text.as_str()),
        (1, "source hash changed: docs/research/fixture/RESULTS.md\n")
    );

    std::fs::remove_file(fixture.root.join(EVIDENCE)).unwrap();
    assert_eq!(
        fixture.check(),
        (
            1,
            format!(
                "redacted evidence {EVIDENCE}: the file is not the redacted bytes its last entry records; restore it \
                 from git or record its redaction with bench-tools redaction repin, or, to correct or remove the \
                 file on purpose, remove its entries from the record and grade the affected batches again\n"
            )
        )
    );
}

#[test]
fn a_file_redacted_twice_resolves_to_its_earliest_hash() {
    let fixture = Fixture::new("twice");
    let saved = fixture.batch();
    repinned(&fixture);
    fixture.commit("first redaction");
    let more = fixture.root.parent().unwrap().join("more-values.json");
    std::fs::write(&more, MORE_VALUES).unwrap();
    let (code, stdout, stderr) = fixture.run(
        &more,
        &["redaction", "repin", EVIDENCE, "--receipt", RECEIPT],
    );
    assert_eq!(
        (code, stdout.as_str(), stderr.as_str()),
        (
            0,
            "redacted: docs/research/fixture/RESULTS.md (1 [redacted-account])\n\
             repinned: bench/grading/current/adjudications.json (1 pin)\n\
             repinned: bench/grading/current/claims.json (1 pin)\n\
             recorded: bench/grading/current/redacted-evidence.json\n",
            ""
        )
    );
    let record = parse(&fixture.read(RECORD));
    assert_eq!(
        field(&record, &["entries", "1", "before", "sha256"]),
        field(&record, &["entries", "0", "after", "sha256"])
    );
    assert_eq!(fixture.batch(), saved);
    assert_eq!(fixture.check(), (0, VALID.to_string()));
}

#[test]
fn repin_refuses_what_it_cannot_record() {
    let fixture = Fixture::new("refusals");
    let refused = |arguments: &[&str]| {
        let (code, stdout, stderr) = fixture.repin(arguments);
        assert_eq!((code, stdout.as_str()), (1, ""), "{stderr}");
        assert_eq!(git(&fixture.root, &["status", "--porcelain"]), "");
        stderr
    };
    let with_receipt = |path: &str| [path, "--receipt", RECEIPT].map(str::to_string);
    let refused_path = |path: &str| {
        let arguments = with_receipt(path);
        refused(&arguments.iter().map(String::as_str).collect::<Vec<_>>())
    };

    let (code, _, stderr) = fixture.run(
        &fixture.root.join("absent.json"),
        &["redaction", "repin", EVIDENCE, "--receipt", RECEIPT],
    );
    assert_eq!(code, 2);
    assert!(stderr.contains("no redaction values file"), "{stderr}");

    assert_eq!(
        refused(&[EVIDENCE, "--receipt", "bench/grading/rulings/absent.md"]),
        "bench-tools redaction: the receipt bench/grading/rulings/absent.md is not a saved ruling under \
         bench/grading/rulings/\n"
    );
    assert_eq!(
        refused(&[EVIDENCE, "--receipt", "docs/research/fixture/RESULTS.md"]),
        "bench-tools redaction: the receipt docs/research/fixture/RESULTS.md is not a saved ruling under \
         bench/grading/rulings/\n"
    );

    fixture.write("notes/new.md", b"deployed from examplehost\n");
    let (code, _, stderr) =
        fixture.repin(&with_receipt("notes/new.md").each_ref().map(String::as_str));
    git(&fixture.root, &["clean", "-q", "-f", "notes"]);
    assert_eq!(code, 1);
    assert_eq!(
        stderr,
        "bench-tools redaction: notes/new.md is not committed at HEAD; commit it first, so its earlier bytes stay \
         in the repository's history\n"
    );

    fixture.write(EVIDENCE, format!("{TEXT}One more line.\n").as_bytes());
    let (code, _, stderr) = fixture.repin(&with_receipt(EVIDENCE).each_ref().map(String::as_str));
    git(&fixture.root, &["checkout", "-q", "--", EVIDENCE]);
    assert_eq!(code, 1);
    assert_eq!(
        stderr,
        "bench-tools redaction: docs/research/fixture/RESULTS.md differs from the version committed at HEAD; \
         commit it first, or, if an earlier repin was interrupted, restore it and the current grading documents \
         from HEAD and run repin again\n"
    );

    assert_eq!(
        refused_path(RECEIPT),
        "bench-tools redaction: the redaction rules do not change bench/grading/rulings/redaction-approval.md; \
         there is nothing to repin\n"
    );

    let mut database = b"SQLite format 3\0".to_vec();
    database.extend_from_slice(b" pages from examplehost");
    fixture.write("notes/run.db", &database);
    fixture.write("notes/loose.md", b"deployed from examplehost\n");
    fixture.commit("notes");
    assert_eq!(
        refused_path("notes/run.db"),
        "bench-tools redaction: cannot redact: notes/run.db (SQLite files are not rewritten); repin cannot \
         record it\n"
    );
    assert_eq!(
        refused_path("notes/loose.md"),
        "bench-tools redaction: no claim or decision evidence pins notes/loose.md; redact it with bench-tools \
         redaction apply\n"
    );

    let mut review = parse(&fixture.read(REVIEW));
    *member(&mut review, &["items", "0", "consequence"]) = s("It breaks on examplehost.");
    fixture.write(REVIEW, &grading_fixture::json(&review));
    let mut claims = parse(&fixture.read(CLAIMS));
    *member(
        &mut claims,
        &["claims", "0", "links", "0", "review", "sha256"],
    ) = s(&hex_digest(&fixture.read(REVIEW)));
    fixture.write(CLAIMS, &grading_fixture::json(&claims));
    fixture.commit("a review that names the machine");
    assert_eq!(
        refused_path(REVIEW),
        format!(
            "bench-tools redaction: {CLAIMS} pins {REVIEW} at /claims/0/links/0/review, which is not claim or \
             eligibility decision evidence; repin covers evidence files of claims and decisions only\n"
        )
    );

    let references = format!("{CURRENT}/references.json");
    let mut reference = parse(&fixture.read(&references));
    let Value::Array(evidence) = member(
        &mut reference,
        &["targets", "0", "families", "1", "evidence"],
    ) else {
        unreachable!()
    };
    evidence.push(fixture.pin(EVIDENCE));
    fixture.write(&references, &grading_fixture::json(&reference));
    fixture.commit("a family that pins the evidence");
    assert_eq!(
        refused_path(EVIDENCE),
        format!(
            "bench-tools redaction: {references} pins {EVIDENCE} at /targets/0/families/1/evidence/1, which is not \
             claim or eligibility decision evidence; repin covers evidence files of claims and decisions only\n"
        )
    );
    git(&fixture.root, &["reset", "-q", "--hard", "HEAD~1"]);

    let index = "bench/evidence-index.json";
    fixture.write(
        index,
        &grading_fixture::json(&o(vec![(
            "files",
            Value::Array(vec![o(vec![
                ("path", s(EVIDENCE)),
                ("sha256", s(&hex_digest(TEXT.as_bytes()))),
                ("bytes", Value::Int(TEXT.len().to_string())),
            ])]),
        )])),
    );
    fixture.commit("an index that pins the evidence");
    assert_eq!(
        refused_path(EVIDENCE),
        format!(
            "bench-tools redaction: {index} pins {EVIDENCE} at /files/0, which is not claim or eligibility decision \
             evidence; repin covers evidence files of claims and decisions only\n"
        )
    );
    git(&fixture.root, &["reset", "-q", "--hard", "HEAD~1"]);

    let beside = "docs/research/fixture/manifest.json";
    fixture.write(
        beside,
        &grading_fixture::json(&o(vec![(
            "results",
            o(vec![
                ("file", s("RESULTS.md")),
                ("expected_sha256", s(&hex_digest(TEXT.as_bytes()))),
            ]),
        )])),
    );
    fixture.commit("a manifest beside the evidence");
    assert_eq!(
        refused_path(EVIDENCE),
        format!(
            "bench-tools redaction: {beside} pins {EVIDENCE} at /results, which is not claim or eligibility decision \
             evidence; repin covers evidence files of claims and decisions only\n"
        )
    );
    git(&fixture.root, &["reset", "-q", "--hard", "HEAD~1"]);

    let hashes = "docs/research/fixture/artifact-hashes.json";
    fixture.write(
        hashes,
        &grading_fixture::json(&o(vec![("RESULTS.md", s(&hex_digest(TEXT.as_bytes())))])),
    );
    fixture.commit("a map from paths to hashes");
    assert_eq!(
        refused_path(EVIDENCE),
        format!(
            "bench-tools redaction: {hashes} pins {EVIDENCE} at /RESULTS.md, which is not claim or eligibility \
             decision evidence; repin covers evidence files of claims and decisions only\n"
        )
    );
    git(&fixture.root, &["reset", "-q", "--hard", "HEAD~1"]);

    let receipt = format!("{CURRENT}/assessments/{RUN}/{TARGET}/assessment-0/receipt.json");
    fixture.write(
        &receipt,
        &grading_fixture::json(&o(vec![(
            "claim_snapshot",
            o(vec![(
                "evidence",
                o(vec![(
                    "packets",
                    Value::Array(vec![o(vec![
                        ("claim_id", s("CL-t-grade-1-close")),
                        (
                            "sources",
                            Value::Array(vec![o(vec![
                                ("label", s("E1")),
                                ("stance", s("supports")),
                                (
                                    "source",
                                    o(vec![("path", s(EVIDENCE)), ("sha256", s(&"d".repeat(64)))]),
                                ),
                            ])]),
                        ),
                    ])]),
                )]),
            )]),
        )])),
    );
    fixture.commit("a receipt of a batch graded with extracts");
    assert_eq!(
        refused_path(EVIDENCE),
        format!(
            "bench-tools redaction: {receipt} lists {EVIDENCE} at \
             /claim_snapshot/evidence/packets/0/sources/0/source as an evidence source prepared for its batch's \
             grader; the grader read from this file, so redaction repin does not cover it\n"
        )
    );
    git(&fixture.root, &["reset", "-q", "--hard", "HEAD~1"]);

    let mut decisions = parse(&fixture.read(DECISIONS));
    *member(
        &mut decisions,
        &["decisions", "0", "evidence", "0", "sha256"],
    ) = s(&"0".repeat(64));
    fixture.write(DECISIONS, &grading_fixture::json(&decisions));
    fixture.commit("a stale pin");
    assert_eq!(
        refused_path(EVIDENCE),
        format!(
            "bench-tools redaction: {DECISIONS} pins {EVIDENCE} at /decisions/0/evidence/0 with a hash other than \
             its current bytes; correct that pin first\n"
        )
    );
}

#[test]
fn the_check_without_values_accepts_only_replaced_values() {
    let fixture = Fixture::new("recheck");
    repinned(&fixture);
    fixture.commit("redacted");
    let failed = |text: &str| {
        let (code, output) = fixture.check();
        assert_eq!(code, 1, "{output}");
        assert!(output.starts_with(text), "{output}");
        git(&fixture.root, &["reset", "-q", "--hard", "HEAD"]);
        assert_eq!(fixture.check(), (0, VALID.to_string()));
    };
    let parted = format!(
        "redacted evidence {EVIDENCE}: the redacted bytes are not the earlier bytes with values replaced by \
         placeholders; they part at byte "
    );

    fixture.swap_evidence(&REDACTED.replace("visitor path", "session path"));
    failed(&parted);
    fixture.swap_evidence(&REDACTED.replace(
        "The cache key keeps the visitor path,\nso a second visitor reads the first visitor's page.\n",
        "[redacted-account]\n",
    ));
    failed(&parted);
    fixture.swap_evidence(&REDACTED.replace(
        "Mail went to [redacted-email] after the build.",
        "[redacted-email]",
    ));
    failed(&parted);

    fixture.edit_record(|record| {
        *member(record, &["entries", "0", "placeholders", "1", "count"]) = Value::Int("2".into());
    });
    failed(&format!(
        "redacted evidence {EVIDENCE}: the entry records 1 [redacted-email], 2 host-01-xyz, but the redacted bytes \
         replace 1 [redacted-email], 1 host-01-xyz"
    ));

    let other = git(&fixture.root, &["rev-parse", &format!("HEAD:{RECEIPT}")]);
    fixture.edit_record(|record| {
        *member(record, &["entries", "0", "before", "blob"]) = s(&other);
    });
    failed(&format!(
        "redacted evidence {EVIDENCE}: blob {other} does not hash to the entry's before sha256, so the entry names \
         other earlier bytes"
    ));

    let gone = "0123456789abcdef0123456789abcdef01234567";
    let missing = |fixture: &Fixture| {
        fixture.edit_record(|record| {
            *member(record, &["entries", "0", "before", "blob"]) = s(gone);
        });
    };
    missing(&fixture);
    failed(&format!(
        "redacted evidence {EVIDENCE}: blob {gone} is not in the repository's object store, so the redaction cannot \
         be re-checked; fetch the full history, or pin the saved record of the history rewrite that removed it as \
         the entry's history"
    ));
}

#[test]
fn only_the_owners_receipt_naming_the_entry_skips_the_recheck() {
    let fixture = Fixture::new("history");
    repinned(&fixture);
    fixture.commit("redacted");
    let gone = "0123456789abcdef0123456789abcdef01234567";
    let graded = hex_digest(TEXT.as_bytes());
    let with_history = |history: &str, text: &str| {
        fixture.write(history, text.as_bytes());
        let pinned = fixture.pin(history);
        fixture.edit_record(|record| {
            *member(record, &["entries", "0", "before", "blob"]) = s(gone);
            if let Value::Object(entry) = member(record, &["entries", "0"]) {
                entry.insert("history".into(), pinned);
            }
        });
    };
    let refused = |history: &str, sha256: &str| {
        format!(
            "redacted evidence {EVIDENCE}: the history pin {history} is not the owner's receipt for the history \
             rewrite; save that receipt under bench/grading/rulings/ with one line that holds, as separate words, \
             {EVIDENCE} {gone} {sha256}, since nothing else skips the re-check\n"
        )
    };
    let line = format!("Rewrite removed `{EVIDENCE}` blob `{gone}` sha256 `{graded}`");

    let elsewhere = "docs/research/fixture/history-rewrite.md";
    with_history(elsewhere, &format!("# History rewrite\n\n{line}\n"));
    assert_eq!(fixture.check(), (1, refused(elsewhere, &graded)));
    let (code, _, stderr) = fixture.repin(&[EVIDENCE, "--receipt", RECEIPT]);
    assert_eq!(
        (code, stderr),
        (
            1,
            format!("bench-tools redaction: {}", refused(elsewhere, &graded))
        )
    );

    let climbing = "bench/grading/rulings/../../../docs/research/fixture/history-rewrite.md";
    with_history(climbing, &format!("# History rewrite\n\n{line}\n"));
    assert_eq!(fixture.check(), (1, refused(climbing, &graded)));

    let ruling = "bench/grading/rulings/history-rewrite.md";
    for text in [
        format!("{EVIDENCE} {gone}0123456789abcdef01234567 {graded}"),
        format!("{EVIDENCE} 0{gone} {graded}"),
        format!("{EVIDENCE}.old {gone} {graded}"),
        format!("{gone} {graded}"),
        format!("{EVIDENCE} {gone}"),
        format!("{EVIDENCE} {gone}\n{graded}"),
    ] {
        with_history(ruling, &format!("# History rewrite\n\n{text}\n"));
        assert_eq!(fixture.check(), (1, refused(ruling, &graded)), "{text}");
    }

    with_history(ruling, &format!("# History rewrite\n\n{line}\n"));
    assert_eq!(
        fixture.check(),
        (
            0,
            format!(
                "{VALID}redacted evidence {EVIDENCE}: not re-checked, because blob {gone} is gone and {ruling} \
                 records the history rewrite\n"
            )
        )
    );

    let earlier = "c".repeat(64);
    fixture.edit_record(|record| {
        *member(record, &["entries", "0", "before", "sha256"]) = s(&earlier);
        if let Value::Object(entry) = member(record, &["entries", "0"]) {
            *entry = entry
                .iter()
                .filter(|(key, _)| key.as_str() != "history")
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect();
        }
    });
    let (stale, _) = fixture.batch();
    fixture.grade_at(&stale);
    with_history(ruling, &format!("# History rewrite\n\n{line}\n"));
    assert_eq!(fixture.check(), (1, refused(ruling, &earlier)));
    git(&fixture.root, &["checkout", "-q", "--", CURRENT]);

    with_history(ruling, &format!("# History rewrite\n\n{line}\n"));
    fixture.write(ruling, b"# History rewrite\n\nEdited.\n");
    let (code, output) = fixture.check();
    assert_eq!(
        (code, output.as_str()),
        (1, format!("source hash changed: {ruling}\n").as_str())
    );
}

#[test]
fn the_commit_hook_keeps_recorded_entries_as_they_were() {
    let fixture = Fixture::new("hook-kept");
    repinned(&fixture);
    fixture.commit("first redaction");
    let staged = || {
        git(&fixture.root, &["add", "-A"]);
        let outcome = fixture.run(
            &fixture.values,
            &["redaction", "check", "--staged", "--require-values"],
        );
        git(&fixture.root, &["reset", "-q", "--hard", "HEAD"]);
        (outcome.0, outcome.1)
    };
    let gone = "0123456789abcdef0123456789abcdef01234567";
    let earlier = "c".repeat(64);
    let ruling = "bench/grading/rulings/history-rewrite.md";

    fixture.write(ruling, format!("{EVIDENCE} {gone} {earlier}\n").as_bytes());
    let pinned = fixture.pin(ruling);
    fixture.edit_record(|record| {
        *member(record, &["entries", "0", "before", "sha256"]) = s(&earlier);
        *member(record, &["entries", "0", "before", "blob"]) = s(gone);
        if let Value::Object(entry) = member(record, &["entries", "0"]) {
            entry.insert("history".into(), pinned.clone());
        }
    });
    assert_eq!(
        staged(),
        (
            1,
            "an entry already at HEAD changed in more than its history pin: docs/research/fixture/RESULTS.md \
             (bench/grading/current/redacted-evidence.json)\n"
                .to_string()
        )
    );

    fixture.write(ruling, b"# History rewrite\n");
    let pinned = fixture.pin(ruling);
    fixture.edit_record(|record| {
        if let Value::Object(entry) = member(record, &["entries", "0"]) {
            entry.insert("history".into(), pinned.clone());
        }
    });
    assert_eq!(staged(), (0, String::new()));

    let more = fixture.root.parent().unwrap().join("more-values.json");
    std::fs::write(&more, MORE_VALUES).unwrap();
    let (code, _, stderr) = fixture.run(
        &more,
        &["redaction", "repin", EVIDENCE, "--receipt", RECEIPT],
    );
    assert_eq!(code, 0, "{stderr}");
    fixture.commit("second redaction");
    fixture.edit_record(|record| {
        if let Value::Array(entries) = member(record, &["entries"]) {
            entries.remove(0);
        }
    });
    assert_eq!(
        staged(),
        (
            1,
            "an entry already at HEAD was moved, or removed while a later entry of its file stays: \
             docs/research/fixture/RESULTS.md (bench/grading/current/redacted-evidence.json)\n"
                .to_string()
        )
    );
    fixture.edit_record(|record| {
        if let Value::Array(entries) = member(record, &["entries"]) {
            entries.pop();
        }
    });
    assert_eq!(staged(), (0, String::new()));
}

#[test]
fn the_commit_hook_check_ties_a_new_entry_to_the_committed_file() {
    let fixture = Fixture::new("hook-blob");
    let graded = hex_digest(TEXT.as_bytes());
    repinned(&fixture);
    let edited = TEXT.replace("visitor path", "session path");
    let staged = || {
        git(&fixture.root, &["add", "-A"]);
        fixture.run(
            &fixture.values,
            &["redaction", "check", "--staged", "--require-values"],
        )
    };
    let refused = (
        1,
        "the entry's earlier bytes are not the file at HEAD: docs/research/fixture/RESULTS.md \
         (bench/grading/current/redacted-evidence.json)\n"
            .to_string(),
    );

    let path = fixture.root.parent().unwrap().join("edited.md");
    std::fs::write(&path, &edited).unwrap();
    let blob = git(
        &fixture.root,
        &["hash-object", "-w", path.to_str().unwrap()],
    );
    fixture.swap_evidence(&REDACTED.replace("visitor path", "session path"));
    fixture.edit_record(|record| {
        *member(record, &["entries", "0", "before", "blob"]) = s(&blob);
    });
    assert_eq!(
        field(
            &parse(&fixture.read(RECORD)),
            &["entries", "0", "before", "sha256"]
        ),
        graded
    );
    let (code, stdout, _) = staged();
    assert_eq!((code, stdout), refused);

    git(&fixture.root, &["reset", "-q", "--hard", "HEAD"]);
    repinned(&fixture);
    fixture.edit_record(|record| {
        *member(record, &["entries", "0", "before", "sha256"]) = s(&"0".repeat(64));
    });
    let (code, stdout, _) = staged();
    assert_eq!((code, stdout), refused);
}

#[test]
fn the_commit_hook_check_confirms_the_rules_own_output() {
    let fixture = Fixture::new("hook");
    repinned(&fixture);
    git(&fixture.root, &["add", "-A"]);
    let staged = || {
        fixture.run(
            &fixture.values,
            &["redaction", "check", "--staged", "--require-values"],
        )
    };
    assert_eq!(staged(), (0, String::new(), String::new()));

    fixture.swap_evidence(&REDACTED.replace("visitor path", "session path"));
    git(&fixture.root, &["add", "-A"]);
    let (code, stdout, stderr) = staged();
    assert_eq!(
        (code, stdout.as_str()),
        (
            1,
            "not what the rules make of its earlier bytes: docs/research/fixture/RESULTS.md \
             (bench/grading/current/redacted-evidence.json)\n"
        )
    );
    assert!(stderr.contains("bench-tools redaction repin"), "{stderr}");

    git(&fixture.root, &["commit", "-q", "-m", "recorded"]);
    assert_eq!(staged(), (0, String::new(), String::new()));
}

#[test]
fn an_entry_changes_no_other_pin() {
    let fixture = Fixture::new("other-pins");
    let saved = fixture.batch();
    let review = hex_digest(&fixture.read(REVIEW));
    let entry = o(vec![
        ("path", s(REVIEW)),
        (
            "before",
            o(vec![
                ("sha256", s(&"1".repeat(64))),
                ("blob", s(&"2".repeat(40))),
            ]),
        ),
        ("after", o(vec![("sha256", s(&review))])),
        (
            "placeholders",
            Value::Array(vec![o(vec![
                ("text", s("[redacted-email]")),
                ("count", Value::Int("1".into())),
            ])]),
        ),
        ("receipt", fixture.pin(RECEIPT)),
    ]);
    fixture.write(
        RECORD,
        &grading_fixture::json(&o(vec![
            ("schema_version", Value::Int("1".into())),
            ("entries", Value::Array(vec![entry])),
        ])),
    );
    assert_eq!(fixture.batch(), saved);
}
