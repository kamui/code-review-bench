//! `bench-tools packet-selection` on a disposable git repository holding a frozen run, its target and a packet
//! replacement pin.

use bench_tools::pyjson::{self, Duplicates, Value};
use bench_tools::sha256::hex_digest;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn digest(path: &Path) -> String {
    hex_digest(&fs::read(path).unwrap())
}

fn text(value: &str) -> Value {
    Value::Str(value.to_string())
}

fn child<'a>(value: &'a mut Value, key: &str) -> &'a mut Value {
    match value {
        Value::Object(object) => object.get_mut(key).unwrap(),
        Value::Array(items) => &mut items[key.parse::<usize>().unwrap()],
        other => panic!("{key} of {other:?}"),
    }
}

fn set(value: &mut Value, path: &[&str], new: Value) {
    let (last, parents) = path.split_last().unwrap();
    match parents.iter().fold(value, |value, key| child(value, key)) {
        Value::Object(object) => object.insert(last.to_string(), new),
        Value::Array(items) => items[last.parse::<usize>().unwrap()] = new,
        other => panic!("{last} of {other:?}"),
    }
}

fn remove(value: &mut Value, key: &str) {
    let Value::Object(object) = value else {
        panic!("{key} of {value:?}");
    };
    *object = object
        .iter()
        .filter(|(name, _)| *name != key)
        .map(|(name, value)| (name.clone(), value.clone()))
        .collect();
}

fn write(path: &Path, doc: &Value) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, pyjson::dumps(doc, &pyjson::Dumps::default())).unwrap();
}

fn relative(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .unwrap()
        .to_string_lossy()
        .into_owned()
}

/// A frozen run whose cohort names target `t1` with a re-cut packet, and the replacement manifest that pins it.
struct Fixture {
    root: PathBuf,
    run: PathBuf,
    target: PathBuf,
    original: PathBuf,
    packet: PathBuf,
    replacements: PathBuf,
    target_doc: Value,
    replacement_doc: Value,
    manifest: Value,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

impl Fixture {
    fn new(label: &str) -> Self {
        let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("packet-selection-{label}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let root = fs::canonicalize(&root).unwrap();
        let bench = root.join("bench");
        let target = bench.join("targets/t1");
        let run = bench.join("runs/2026-01-01-test");
        fs::create_dir_all(&target).unwrap();
        fs::create_dir_all(&run).unwrap();
        let original = target.join("packet.md");
        fs::write(&original, b"# Original packet\n\n").unwrap();
        let packet = target.join("packet.v2.md");
        fs::write(&packet, b"# Re-cut packet\r\n\n\n").unwrap();
        let target_doc = parse(&format!(
            r#"{{"id": "t1", "packet_sha256": "{}", "diff_manifest_sha256": "{}", "merge_base": "{}", "head": "{}",
                "provisioning": {{"allowance": "Run focused tests.", "unavailable": "network"}}}}"#,
            digest(&original),
            "d".repeat(64),
            "a".repeat(40),
            "b".repeat(40),
        ));
        write(&target.join("target.json"), &target_doc);
        let replacements = root.join("docs/packet-replacements.v1.json");
        let replacement_doc = parse(&format!(
            r#"{{"schema_version": 1, "targets": [{{"target": "t1", "target_sha256": "{}",
                "original": {{"path": "{}", "sha256": "{}"}},
                "replacement": {{"path": "{}", "sha256": "{}"}}}}]}}"#,
            digest(&target.join("target.json")),
            relative(&original, &root),
            digest(&original),
            relative(&packet, &root),
            digest(&packet),
        ));
        write(&replacements, &replacement_doc);
        let arm = bench.join("arms/test.json");
        write(
            &arm,
            &parse(r#"{"id": "test", "kind": "codex", "model": "test-model", "effort": "high"}"#),
        );
        let manifest = parse(&format!(
            r#"{{"schema_version": 1, "run_id": "2026-01-01-test", "created_at": "2026-01-01T00:00:00Z",
                "method_revision": "m", "rubric_version": 1, "metric_code_revision": "c",
                "arms": [{{"id": "test", "arm_file_sha256": "{}", "resolved_skill_tree": null,
                           "expected_cli_version": "test", "expected_prompt_hashes": [], "billing_mode": "subscription"}}],
                "cohort": [{{"target": "t1", "packet_sha256": "{}", "diff_manifest_sha256": "{}",
                             "register_version": 1, "cohort_group": "fixture"}}],
                "packet_replacements": {{"path": "{}", "sha256": "{}"}},
                "planned_cells": [{{"target": "t1", "arm": "test", "replicate": 1}}],
                "caps": {{"max_attempts": 1, "replacements": 0, "spend_usd": 2, "closeout_reserve_usd": 0,
                          "max_in_flight": 1, "attempt_usd": 1}},
                "sealed_order": ["t1/test/1"], "rates": [], "deviations": [],
                "execution_policy": {{"allowance": "Five minutes.", "branch_layout": "`main` is the merge-base."}}}}"#,
            digest(&arm),
            digest(&packet),
            "d".repeat(64),
            relative(&replacements, &root),
            digest(&replacements),
        ));
        let mut fixture = Self {
            root,
            run,
            target,
            original,
            packet,
            replacements,
            target_doc,
            replacement_doc,
            manifest,
        };
        fixture.git(&["init", "-q"]);
        fixture.git(&["config", "user.name", "Fixture"]);
        fixture.git(&["config", "user.email", "fixture@example.test"]);
        fixture.git(&["config", "commit.gpgsign", "false"]);
        fixture.freeze();
        fixture
    }

    fn git(&self, arguments: &[&str]) -> String {
        let output = Command::new("git")
            .arg("-C")
            .arg(&self.root)
            .args(arguments)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {arguments:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap().trim().to_string()
    }

    fn freeze(&mut self) {
        write(&self.run.join("manifest.json"), &self.manifest);
        self.git(&["add", "."]);
        self.git(&["commit", "-qm", "Freeze fixture"]);
        let head = self.git(&["rev-parse", "HEAD"]);
        set(
            &mut self.manifest,
            &["frozen_at"],
            text("2026-01-01T00:00:00Z"),
        );
        set(&mut self.manifest, &["freeze_commit"], text(&head));
        write(&self.run.join("manifest.json"), &self.manifest);
    }

    fn repin(&mut self) {
        write(&self.replacements, &self.replacement_doc);
        set(
            &mut self.manifest,
            &["packet_replacements", "sha256"],
            text(&digest(&self.replacements)),
        );
        self.freeze();
    }

    fn pin_the_original(&mut self) {
        remove(&mut self.manifest, "packet_replacements");
        let original = digest(&self.original);
        set(
            &mut self.manifest,
            &["cohort", "0", "packet_sha256"],
            text(&original),
        );
    }

    fn selection(&self, env: &[(&str, &Path)]) -> Output {
        write(&self.run.join("manifest.json"), &self.manifest);
        let mut command = Command::new(env!("CARGO_BIN_EXE_bench-tools"));
        command
            .arg("packet-selection")
            .arg("--run")
            .arg(&self.run)
            .arg("--target-dir")
            .arg(&self.target)
            .arg("--root")
            .arg(&self.root);
        for (name, value) in env {
            command.env(name, value);
        }
        command.output().unwrap()
    }

    fn select(&self, env: &[(&str, &Path)]) -> PathBuf {
        let output = self.selection(env);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        PathBuf::from(
            String::from_utf8(output.stdout)
                .unwrap()
                .trim_end_matches('\n'),
        )
    }

    fn assert_refused(&self, messages: &[&str]) {
        let output = self.selection(&[]);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(!output.status.success(), "{stderr}");
        assert!(output.stdout.is_empty(), "{output:?}");
        assert!(
            messages.iter().any(|message| stderr.contains(message)),
            "expected one of {messages:?}: {stderr}"
        );
    }
}

fn parse(json: &str) -> Value {
    pyjson::loads(json, Duplicates::KeepLast).unwrap()
}

#[test]
fn frozen_replacement_is_selected() {
    let fixture = Fixture::new("selected");
    assert_eq!(fixture.select(&[]), fixture.packet);
}

#[test]
fn changed_target_is_refused() {
    let mut fixture = Fixture::new("changed-target");
    set(&mut fixture.target_doc, &["head"], text(&"c".repeat(40)));
    write(&fixture.target.join("target.json"), &fixture.target_doc);
    fixture.assert_refused(&["frozen target.json changed"]);
}

#[test]
fn changed_original_is_refused() {
    let fixture = Fixture::new("changed-original");
    fs::write(&fixture.original, "changed").unwrap();
    fixture.assert_refused(&["original packet.md differs"]);
}

#[test]
fn changed_replacement_is_refused() {
    let fixture = Fixture::new("changed-replacement");
    fs::write(&fixture.packet, "changed").unwrap();
    fixture.assert_refused(&["replacement packet differs"]);
}

#[test]
fn changed_manifest_is_refused() {
    let fixture = Fixture::new("changed-manifest");
    fs::write(&fixture.replacements, "{}").unwrap();
    fixture.assert_refused(&["replacement manifest differs"]);
}

#[test]
fn replacement_must_match_cohort() {
    let mut fixture = Fixture::new("cohort");
    set(
        &mut fixture.manifest,
        &["cohort", "0", "packet_sha256"],
        text(&"0".repeat(64)),
    );
    fixture.freeze();
    fixture.assert_refused(&["selected packet differs"]);
}

#[test]
fn manifest_original_must_match_target_pin() {
    let mut fixture = Fixture::new("original-pin");
    set(
        &mut fixture.replacement_doc,
        &["targets", "0", "original", "sha256"],
        text(&"0".repeat(64)),
    );
    fixture.repin();
    fixture.assert_refused(&["original packet identity differs"]);
}

#[test]
fn pin_and_cohort_cannot_change_after_freeze() {
    let mut fixture = Fixture::new("after-freeze");
    let frozen = fixture.manifest.clone();
    for path in [
        &["packet_replacements", "sha256"][..],
        &["cohort", "0", "packet_sha256"],
    ] {
        fixture.manifest = frozen.clone();
        set(&mut fixture.manifest, path, text(&"0".repeat(64)));
        fixture.assert_refused(&["differs from freeze_commit"]);
    }
}

#[test]
fn removing_frozen_pin_refuses_original_packet() {
    let mut fixture = Fixture::new("removed-pin");
    fixture.pin_the_original();
    fixture.assert_refused(&["differs from freeze_commit"]);
}

#[test]
fn legacy_run_without_readable_freeze_remains_supported() {
    let mut fixture = Fixture::new("legacy");
    fixture.pin_the_original();
    for freeze in [Value::Null, text(&"0".repeat(40))] {
        set(&mut fixture.manifest, &["freeze_commit"], freeze.clone());
        assert_eq!(fixture.select(&[]), fixture.original, "{freeze:?}");
    }
}

#[test]
fn duplicate_replacement_is_refused() {
    let mut fixture = Fixture::new("duplicate");
    let targets = child(&mut fixture.replacement_doc, "targets");
    let Value::Array(rows) = targets else {
        panic!("{targets:?}");
    };
    rows.extend(rows.clone());
    fixture.repin();
    fixture.assert_refused(&["duplicate packet replacement target"]);
}

#[test]
fn missing_and_escaping_paths_refuse() {
    let mut fixture = Fixture::new("paths");
    for name in ["missing.md", "../outside.md", "/tmp/outside.md"] {
        set(
            &mut fixture.replacement_doc,
            &["targets", "0", "replacement", "path"],
            text(name),
        );
        fixture.repin();
        fixture.assert_refused(&["No such file", "must stay inside"]);
    }
}

#[test]
fn environment_does_not_override_the_frozen_pin() {
    let mut fixture = Fixture::new("environment");
    fixture.pin_the_original();
    fixture.freeze();
    let replacements = fixture.replacements.clone();
    assert_eq!(
        fixture.select(&[("BENCH_PACKET_REPLACEMENTS", &replacements)]),
        fixture.original
    );
}

#[test]
fn unlisted_target_uses_original_pin() {
    let mut fixture = Fixture::new("unlisted");
    set(
        &mut fixture.replacement_doc,
        &["targets"],
        Value::Array(Vec::new()),
    );
    let original = digest(&fixture.original);
    set(
        &mut fixture.manifest,
        &["cohort", "0", "packet_sha256"],
        text(&original),
    );
    fixture.repin();
    assert_eq!(fixture.select(&[]), fixture.original);
}
