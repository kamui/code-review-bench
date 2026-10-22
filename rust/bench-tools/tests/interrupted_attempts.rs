//! `bench-tools run-cell --file`, `--file --interrupted` and `--release` on a frozen fixture run, and the workspace
//! cleanup filing runs. Every attempt directory is a scratch fixture shaped like a dispatched built-in attempt; the
//! one reviewer started is a stand-in script under `dispatch.sh`, so no model or network is called.

use bench_tools::pyjson::{self, Dumps, Duplicates, Value};
use bench_tools::sha256;
use nix::sys::signal::{Signal, killpg};
use nix::unistd::{Pid, getpgid};
use std::fmt::Write as _;
use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

const RUN_ID: &str = "2026-01-01-interrupted";
const PROMPT: &str = "`high effort variant`\n\nReview the diff.";
const RUBRIC: &str = "You are acting as a reviewer for a proposed code change";
/// An observation long after any fixture output.
const LATER: &str = "2999-01-01T00:00:00+00:00";

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn write(path: &Path, content: impl AsRef<[u8]>) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

fn parse(text: &str) -> Value {
    pyjson::loads(text, Duplicates::KeepLast).unwrap()
}

fn read(path: &Path) -> Value {
    parse(&fs::read_to_string(path).unwrap())
}

fn quoted(text: &str) -> String {
    pyjson::dumps(&Value::Str(text.to_string()), &Dumps::default())
}

fn lines(records: &[String]) -> String {
    records.iter().fold(String::new(), |mut out, record| {
        let _ = writeln!(out, "{record}");
        out
    })
}

/// The value at a dotted path, or null.
fn field<'a>(value: &'a Value, path: &str) -> &'a Value {
    static NULL: Value = Value::Null;
    path.split('.').fold(value, |value, key| {
        value
            .as_object()
            .and_then(|object| object.get(key))
            .unwrap_or(&NULL)
    })
}

fn text(value: &Value) -> String {
    value.py_str()
}

fn number(value: &Value) -> Option<f64> {
    match value {
        Value::Int(digits) => digits.parse().ok(),
        Value::Float(number) => Some(*number),
        _ => None,
    }
}

fn amount(value: &Value) -> f64 {
    number(value).unwrap_or_else(|| panic!("{value:?} is no number"))
}

fn captured() -> f64 {
    (10.0 * 2.0 + 100.0 * 2.5 + 1000.0 * 0.2 + 50.0 * 10.0) / 1e6
}

fn report(done: &Output) -> String {
    format!(
        "exit {:?}\nstdout: {}\nstderr: {}",
        done.status.code(),
        String::from_utf8_lossy(&done.stdout),
        String::from_utf8_lossy(&done.stderr)
    )
}

fn stdout(done: &Output) -> String {
    String::from_utf8_lossy(&done.stdout).into_owned()
}

fn stderr(done: &Output) -> String {
    String::from_utf8_lossy(&done.stderr).into_owned()
}

fn git(directory: &Path, arguments: &[&str]) -> String {
    let done = Command::new("git")
        .args([
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "-C",
        ])
        .arg(directory)
        .args(arguments)
        .env("GIT_AUTHOR_DATE", "2026-01-01T00:00:00Z")
        .env("GIT_COMMITTER_DATE", "2026-01-01T00:00:00Z")
        .output()
        .unwrap();
    assert!(done.status.success(), "{}", report(&done));
    String::from_utf8(done.stdout).unwrap().trim().to_string()
}

fn reply(stop_reason: &str) -> String {
    format!(
        r#"{{"type": "assistant", "requestId": "r1", "effort": "high", "timestamp": "2026-01-01T00:00:01Z", "message": {{"model": "m-1", "stop_reason": "{stop_reason}", "content": [{{"type": "text", "text": "[]"}}], "usage": {{"input_tokens": 10, "cache_creation_input_tokens": 100, "cache_read_input_tokens": 1000, "cache_creation": {{"ephemeral_5m_input_tokens": 100, "ephemeral_1h_input_tokens": 0}}, "output_tokens": 50}}}}}}"#
    )
}

fn claude_transcript(stop_reason: &str) -> String {
    let target = format!(
        r#"{{"type": "user", "message": {{"content": {}}}}}"#,
        quoted(&format!(
            "Review target: `main...review-head high`\n\n{PROMPT}\n"
        ))
    );
    lines(&[target, reply(stop_reason)])
}

fn session_totals() -> String {
    lines(&[r#"{"type": "cost-state", "sessionId": "s1", "modelUsage": {"m-1": {"inputTokens": 10, "cacheCreationInputTokens": 100, "cacheReadInputTokens": 1000, "outputTokens": 50}}}"#.into()])
}

const RATES: &str = r#"{"rates": [{"model": "m-1", "as_of": "2026-01-01", "input": 2.0, "output": 10.0, "cache_read": 0.2, "cache_write_5m": 2.5, "cache_write_1h": 4.0, "billing": "api-dollars"}]}"#;

fn assert_unknown_total(record: &Value, minimum: f64) {
    assert_eq!(
        text(field(record, "usage.metering_status")),
        "incomplete",
        "{:?}",
        field(record, "notes")
    );
    assert!(matches!(
        field(record, "usage.priced_total_usd"),
        Value::Null
    ));
    assert!((amount(field(record, "usage.cost_bounds_usd.low")) - minimum).abs() < 1e-12);
    assert!(matches!(
        field(record, "usage.cost_bounds_usd.high"),
        Value::Null
    ));
}

/// A frozen fixture run under a scratch root: `bench/runs/<run>`, its claims under `work/<run>`.
struct Fixture {
    root: PathBuf,
    run_dir: PathBuf,
    work: PathBuf,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

impl Fixture {
    fn new(name: &str) -> Self {
        let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
            "interrupted-attempts-{}-{name}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let root = fs::canonicalize(&root).unwrap();
        let run_dir = root.join("bench/runs").join(RUN_ID);
        fs::create_dir_all(run_dir.join("fixture")).unwrap();
        symlink(
            repository().join("bench/harness"),
            root.join("bench/harness"),
        )
        .unwrap();
        let prompt = sha256::hex_digest(PROMPT.as_bytes());
        let rubric = sha256::hex_digest(RUBRIC.as_bytes());
        write(
            &root.join("bench/arms/arm-claude.json"),
            format!(
                r#"{{"id": "arm-claude", "kind": "claude-builtin", "model": "m-1", "effort": "high", "adapter": {{"expected_prompt_variants": ["{prompt}"]}}}}"#
            ),
        );
        write(
            &root.join("bench/arms/arm-codex.json"),
            format!(
                r#"{{"id": "arm-codex", "kind": "codex", "model": "m-1", "effort": "high", "adapter": {{"expected_prompt_variants": ["{rubric}"]}}}}"#
            ),
        );
        write(
            &run_dir.join("manifest.json"),
            format!(
                r#"{{"run_id": "{RUN_ID}", "frozen_at": "2026-01-01T00:00:00Z", "arms": [{{"id": "arm-claude", "expected_cli_version": "9.9.9", "resolved_skill_tree": null}}, {{"id": "arm-codex", "expected_cli_version": "9.9.9", "resolved_skill_tree": null}}], "planned_cells": [{{"target": "t-fixture", "arm": "arm-claude", "replicate": 1}}], "sealed_order": ["t-fixture/arm-claude/1"], "caps": {{"max_attempts": 4, "replacements": 2, "spend_usd": 50.0, "closeout_reserve_usd": 0.0, "max_in_flight": 1, "attempt_usd": 5.0}}}}"#
            ),
        );
        let digest = sha256::hex_digest(&fs::read(env!("CARGO_BIN_EXE_bench-tools")).unwrap());
        write(
            &run_dir.join("runtimes.frozen.json"),
            format!(r#"{{"bench-tools": {{"sha256": "{digest}"}}}}"#),
        );
        Self {
            work: root.join("work").join(RUN_ID),
            run_dir,
            root,
        }
    }

    /// A clone with `main` at the merge base and `review-head` checked out, which the fixture target names.
    fn clone(&self, attempt: &Path) -> PathBuf {
        let clone = attempt.join("clone");
        fs::create_dir_all(&clone).unwrap();
        git(&clone, &["init", "-q", "-b", "main"]);
        git(&clone, &["commit", "--allow-empty", "-qm", "base"]);
        git(&clone, &["checkout", "-q", "-b", "review-head"]);
        git(&clone, &["commit", "--allow-empty", "-qm", "head"]);
        let head = git(&clone, &["rev-parse", "review-head"]);
        let base = git(&clone, &["rev-parse", "main"]);
        write(
            &self.run_dir.join("fixture/target.json"),
            format!(r#"{{"id": "t-fixture", "head": "{head}", "merge_base": "{base}"}}"#),
        );
        clone
    }

    fn claim(&self, attempt_id: &str, extra: &str) -> PathBuf {
        let attempt = self.work.join(attempt_id);
        write(
            &attempt.join("cell.json"),
            format!(
                r#"{{"cell": {{"target": "t-fixture", "arm": "arm-claude", "replicate": 1}}, "reserved_usd": 5.0, "predecessor": null{extra}}}"#
            ),
        );
        attempt
    }

    /// A claimed built-in attempt whose reviewer ran to its end; `exit_code` None leaves the dispatch unreturned.
    fn attempt(&self, attempt_id: &str, exit_code: Option<i64>) -> PathBuf {
        let attempt = self.claim(attempt_id, r#", "billing_mode": "subscription""#);
        self.clone(&attempt);
        write(&attempt.join("rates.json"), RATES);
        let completed = if exit_code.is_some() {
            "\"2026-01-01T00:00:05Z\""
        } else {
            "null"
        };
        write(
            &attempt.join("timing.json"),
            format!(
                r#"{{"root_dispatched_at": "2026-01-01T00:00:00Z", "payload_validated_at": null, "completed_at": {completed}}}"#
            ),
        );
        write(&attempt.join("tree-before.txt"), "abc\n");
        let ended = exit_code
            .map(|code| format!("exit={code}\n"))
            .unwrap_or_default();
        write(
            &attempt.join("dispatch.txt"),
            format!("claude 9.9.9 (Claude Code)\nmodel=m-1 effort=high\n{ended}"),
        );
        write(
            &attempt.join("home/.claude/projects/p/s1.jsonl"),
            session_totals(),
        );
        write(
            &attempt.join("home/.claude/projects/p/s1/subagents/agent-a1.jsonl"),
            claude_transcript("end_turn"),
        );
        write(&attempt.join("home/.claude/.credentials.json"), "{}");
        write(
            &attempt.join("stdout.jsonl"),
            lines(&[
                r#"{"type": "system"}"#.into(),
                r#"{"type": "result", "subtype": "success"}"#.into(),
            ]),
        );
        if let Some(code) = exit_code {
            write(
                &attempt.join("native-return.json"),
                format!(r#"{{"returned_at": "2026-01-01T00:00:04Z", "exit_code": {code}}}"#),
            );
            write(&attempt.join("tree-after.txt"), "abc\n");
            write(
                &attempt.join("audit.json"),
                r#"{"violations": [], "guidance_probes": [], "network_commands": [], "diff_commands": ["git diff main...review-head"]}"#,
            );
            write(
                &attempt.join("normalized.json"),
                r#"{"parse_status": "parsed", "items": []}"#,
            );
            write(&attempt.join("payload.json"), "[]");
        }
        attempt
    }

    /// An attempt whose reviewer started and never returned, its child cut mid-review.
    fn interrupted(&self) -> PathBuf {
        let attempt = self.attempt("att-001", None);
        write(
            &attempt.join("stdout.jsonl"),
            lines(&[
                r#"{"type": "system"}"#.into(),
                r#"{"type": "rate_limit_event"}"#.into(),
            ]),
        );
        write(
            &attempt.join("home/.claude/projects/p/s1/subagents/agent-a1.jsonl"),
            claude_transcript("tool_use"),
        );
        attempt
    }

    /// A process-stop observation of `att-001`, with `changes` to its fields.
    fn evidence(&self, changes: &[(&str, &str)]) -> PathBuf {
        let mut fields = [
            ("run_id", RUN_ID),
            ("attempt_id", "att-001"),
            ("observed_at", LATER),
            ("status", "absent"),
            ("verified_by", "fixture: process table of the dispatch host"),
        ];
        for (key, value) in changes {
            fields.iter_mut().find(|(known, _)| known == key).unwrap().1 = *value;
        }
        let body: Vec<String> = fields
            .iter()
            .map(|(key, value)| format!("\"{key}\": {}", quoted(value)))
            .collect();
        let count = fs::read_dir(&self.root).unwrap().count();
        let path = self.root.join(format!("process-stop-{count}.json"));
        write(&path, format!("{{{}}}", body.join(", ")));
        path
    }

    fn runner(&self, arguments: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_bench-tools"))
            .arg("run-cell")
            .arg("--run")
            .arg(&self.run_dir)
            .arg("--work")
            .arg(&self.work)
            .args(arguments)
            .env("BENCH_ARCHIVE_ROOT", self.root.join("archive"))
            .output()
            .unwrap()
    }

    fn interrupt(&self, evidence: &Path, reason: &str) -> Output {
        self.runner(&[
            "--file",
            "att-001",
            "--interrupted",
            evidence.to_str().unwrap(),
            "--reason",
            reason,
        ])
    }

    fn release(&self, evidence: &Path) -> Output {
        self.runner(&[
            "--release",
            "att-001",
            "--interrupted",
            evidence.to_str().unwrap(),
        ])
    }

    fn status(&self) -> Value {
        let done = self.runner(&["--status"]);
        assert_eq!(done.status.code(), Some(0), "{}", report(&done));
        parse(&stdout(&done))
    }

    fn filed_dir(&self, attempt_id: &str) -> PathBuf {
        self.run_dir.join("attempts").join(attempt_id)
    }

    fn record(&self, attempt_id: &str) -> Value {
        read(&self.filed_dir(attempt_id).join("attempt.json"))
    }

    /// `--file` that files the attempt and then fails; the filed record.
    fn file_failing(&self, attempt_id: &str, needle: &str) -> Value {
        let done = self.runner(&["--file", attempt_id]);
        assert_eq!(done.status.code(), Some(2), "{}", report(&done));
        assert!(stderr(&done).contains(needle), "{}", report(&done));
        self.record(attempt_id)
    }
}

#[test]
fn absent_or_unverified_process_evidence_is_refused_before_anything_is_written() {
    let f = Fixture::new("unverified");
    let attempt = f.interrupted();
    let cases = [
        f.root.join("no-such-evidence.json"),
        f.evidence(&[("status", "unknown")]),
        f.evidence(&[("status", "running")]),
        f.evidence(&[("attempt_id", "att-009")]),
        f.evidence(&[("run_id", "2026-01-01-other")]),
        f.evidence(&[("verified_by", " ")]),
        f.evidence(&[("observed_at", "soon")]),
        f.evidence(&[("observed_at", "2999-01-01T00:00:00")]),
        f.evidence(&[("observed_at", "2026-01-01T00:30:00Z")]),
    ];
    for evidence in cases {
        let done = f.interrupt(&evidence, "host restarted");
        assert_eq!(done.status.code(), Some(1), "{}", report(&done));
        assert!(
            stdout(&done).contains("refused: att-001"),
            "{}",
            report(&done)
        );
        assert!(!f.filed_dir("att-001").exists());
        assert!(!attempt.join("interruption.json").exists());
        assert!(attempt.join("home/.claude/.credentials.json").exists());
    }
    let evidence = f.evidence(&[]);
    for reason in [None, Some(" ")] {
        let mut arguments = vec![
            "--file",
            "att-001",
            "--interrupted",
            evidence.to_str().unwrap(),
        ];
        arguments.extend(
            reason
                .map(|reason| ["--reason", reason])
                .into_iter()
                .flatten(),
        );
        let done = f.runner(&arguments);
        assert_eq!(
            (done.status.code(), stdout(&done).trim()),
            (Some(1), "refused: --interrupted needs --reason"),
            "{}",
            report(&done)
        );
        assert!(!attempt.join("interruption.json").exists());
    }
}

#[test]
fn an_interruption_is_closed_out_with_its_sign_ins_removed_and_its_clone_kept() {
    let f = Fixture::new("closed-out");
    let attempt = f.interrupted();
    write(&attempt.join("home/.codex/auth.json"), "{}");
    let done = f.interrupt(&f.evidence(&[]), "host restarted during the review");
    assert_eq!(done.status.code(), Some(0), "{}", report(&done));
    let record = f.record("att-001");
    assert_eq!(
        text(field(&record, "disposition")),
        "stopped: host restarted during the review"
    );
    assert!(matches!(field(&record, "timing.stopped_at"), Value::Null));
    assert_unknown_total(&record, captured());
    for dispatched in [
        attempt.join("dispatch.txt"),
        f.filed_dir("att-001").join("dispatch.txt"),
    ] {
        assert!(!fs::read_to_string(dispatched).unwrap().contains("exit="));
    }
    let closed = read(&f.filed_dir("att-001").join("interruption.json"));
    assert_eq!(text(field(&closed, "process_stop.status")), "absent");
    assert_eq!(
        text(field(&closed, "reason")),
        "host restarted during the review"
    );
    let last_output = text(field(&closed, "last_output_at"));
    assert!(
        last_output.starts_with("20") && last_output.ends_with('Z'),
        "{last_output}"
    );
    assert!(last_output.as_str() <= LATER, "{last_output}");
    assert!(!attempt.join("home/.claude/.credentials.json").exists());
    assert!(!attempt.join("home/.codex/auth.json").exists());
    assert!(attempt.join("clone").is_dir());
    assert!(!attempt.join("workspace-pruned.json").exists());
}

#[test]
fn a_completed_interruption_filing_is_reused() {
    let f = Fixture::new("reused");
    f.interrupted();
    let first = f.interrupt(&f.evidence(&[]), "host restarted");
    assert_eq!(first.status.code(), Some(0), "{}", report(&first));
    let record = f.filed_dir("att-001").join("attempt.json");
    let before = (
        fs::read(&record).unwrap(),
        fs::metadata(&record).unwrap().modified().unwrap(),
    );
    let again = f.interrupt(&f.root.join("no-such-evidence.json"), "another reason");
    assert_eq!(again.status.code(), Some(0), "{}", report(&again));
    assert_eq!(stdout(&again), stdout(&first));
    assert_eq!(
        (
            fs::read(&record).unwrap(),
            fs::metadata(&record).unwrap().modified().unwrap()
        ),
        before
    );
}

#[test]
fn an_ended_dispatch_is_not_filed_as_an_interruption() {
    let f = Fixture::new("ended");
    f.attempt("att-001", Some(0));
    let done = f.interrupt(&f.evidence(&[]), "host restarted");
    assert_eq!(done.status.code(), Some(1), "{}", report(&done));
    assert!(
        stdout(&done).contains("the dispatch ended"),
        "{}",
        report(&done)
    );
    assert!(!f.filed_dir("att-001").exists());
}

#[test]
fn a_claim_whose_reviewer_never_started_is_released_on_verified_absence_only() {
    let f = Fixture::new("release");
    let attempt = f.claim("att-001", "");
    assert!((amount(field(&f.status(), "spend_usd.in_flight_reserved")) - 5.0).abs() < 1e-12);
    let cases = [
        (
            f.evidence(&[("status", "running")]),
            "status is 'running', not 'absent'".to_string(),
        ),
        (
            f.evidence(&[("attempt_id", "att-009")]),
            format!("not {RUN_ID}/att-001"),
        ),
        (
            f.root.join("no-such-evidence.json"),
            "no readable process-stop evidence".to_string(),
        ),
    ];
    for (evidence, needle) in cases {
        let done = f.release(&evidence);
        assert_eq!(done.status.code(), Some(1), "{}", report(&done));
        assert!(
            stdout(&done).contains(&needle),
            "{needle}: {}",
            report(&done)
        );
        assert!(attempt.join("cell.json").is_file());
    }
    let done = f.release(&f.evidence(&[]));
    assert_eq!(done.status.code(), Some(0), "{}", report(&done));
    let released = parse(&stdout(&done));
    assert_eq!(text(field(&released, "attempt_id")), "att-001");
    assert!(matches!(field(&released, "released"), Value::Bool(true)));
    let state = f.status();
    assert!(!attempt.exists());
    assert_eq!(text(field(&state, "attempts")), "0");
    assert!(amount(field(&state, "spend_usd.in_flight_reserved")).abs() < 1e-12);
}

#[test]
fn a_claim_whose_reviewer_started_or_that_is_filed_is_never_released() {
    let f = Fixture::new("never-released");
    let started = f.interrupted();
    let refused = f.release(&f.evidence(&[]));
    assert_eq!(refused.status.code(), Some(1), "{}", report(&refused));
    assert!(
        stdout(&refused).contains("a reviewer was started, so the attempt is filed, not released"),
        "{}",
        report(&refused)
    );
    let filed = f.interrupt(&f.evidence(&[]), "host restarted");
    assert_eq!(filed.status.code(), Some(0), "{}", report(&filed));
    let refused = f.release(&f.evidence(&[]));
    assert_eq!(refused.status.code(), Some(1), "{}", report(&refused));
    assert!(
        stdout(&refused).contains("a filed attempt is never released"),
        "{}",
        report(&refused)
    );
    assert!(started.join("clone").is_dir());
}

/// Process groups that are killed when dropped, so a failed assertion leaves no stand-in reviewer running.
struct Groups(Vec<Pid>);

impl Drop for Groups {
    fn drop(&mut self) {
        for group in &self.0 {
            let _ = killpg(*group, Signal::SIGKILL);
        }
    }
}

#[test]
fn a_reviewer_killed_under_the_wrapper_leaves_no_return_and_is_closed_out() {
    let f = Fixture::new("killed");
    let attempt = f.claim("att-001", r#", "billing_mode": "subscription""#);
    let clone = f.clone(&attempt);
    write(&attempt.join("rates.json"), RATES);
    let source = f.root.join("source-home");
    write(
        &source.join(".claude/.credentials.json"),
        r#"{"token": "fixture"}"#,
    );
    write(&source.join(".claude.json"), "{}");
    let packet = f.root.join("packet.md");
    write(&packet, "Review fixture.");
    let transcript = f.root.join("transcript.jsonl");
    write(&transcript, claude_transcript("tool_use"));
    let client = f.root.join("fake-claude");
    write(
        &client,
        format!(
            r#"#!/bin/sh
if [ "$1" = --version ]; then echo "9.9.9 (Claude Code)"; exit 0; fi
while [ "$#" -gt 0 ]; do
  if [ "$1" = --session-id ]; then session=$2; fi
  shift
done
projects="$HOME/.claude/projects/p"
mkdir -p "$projects/$session/subagents"
: > "$projects/$session.jsonl"
cp '{}' "$projects/$session/subagents/agent-a1.jsonl"
printf '{{"type": "system", "subtype": "task_started", "pid": %s}}\n' "$$"
exec sleep 120
"#,
            transcript.display()
        ),
    );
    fs::set_permissions(&client, fs::Permissions::from_mode(0o755)).unwrap();
    let executable = Path::new(env!("CARGO_BIN_EXE_bench-tools"));
    let mut wrapper = Command::new(repository().join("bench/tools/dispatch.sh"))
        .arg("claude-builtin")
        .arg(&attempt)
        .arg(&clone)
        .arg("main")
        .arg(&packet)
        .args(["m-1", "high"])
        .env("HOME", &source)
        .env("BENCH_CLAUDE", &client)
        .env("BENCH_RUST_BENCH_TOOLS", executable)
        .env(
            "BENCH_RUST_BENCH_TOOLS_SHA256",
            sha256::hex_digest(&fs::read(executable).unwrap()),
        )
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
        .unwrap();
    let wrapper_group = Pid::from_raw(i32::try_from(wrapper.id()).unwrap());
    let mut groups = Groups(vec![wrapper_group]);
    let deadline = Instant::now() + Duration::from_secs(60);
    let started = loop {
        let started = fs::read_to_string(attempt.join("stdout.jsonl")).unwrap_or_default();
        if started.ends_with('\n') {
            break started;
        }
        assert!(
            wrapper.try_wait().unwrap().is_none(),
            "dispatch.sh ended before the stand-in reviewer started"
        );
        assert!(
            Instant::now() < deadline,
            "the stand-in reviewer never started"
        );
        std::thread::sleep(Duration::from_millis(50));
    };
    let reviewer = Pid::from_raw(
        text(field(&parse(started.trim_end()), "pid"))
            .parse()
            .unwrap(),
    );
    // timeout(1) runs the reviewer in a process group of its own, which outlives the wrapper's.
    groups.0.push(getpgid(Some(reviewer)).unwrap());
    killpg(wrapper_group, Signal::SIGKILL).unwrap();
    wrapper.wait().unwrap();
    drop(groups);

    assert!(
        !fs::read_to_string(attempt.join("dispatch.txt"))
            .unwrap()
            .contains("exit=")
    );
    assert!(!attempt.join("native-return.json").exists());
    assert!(attempt.join("home/.claude/.credentials.json").exists());
    let refused = f.runner(&["--file", "att-001"]);
    assert_eq!(refused.status.code(), Some(1), "{}", report(&refused));
    assert!(
        stdout(&refused).contains("the dispatch has not ended"),
        "{}",
        report(&refused)
    );
    let done = f.interrupt(&f.evidence(&[]), "reviewer and wrapper killed");
    assert_eq!(done.status.code(), Some(0), "{}", report(&done));
    let record = f.record("att-001");
    assert_eq!(
        text(field(&record, "disposition")),
        "stopped: reviewer and wrapper killed"
    );
    assert_unknown_total(&record, captured());
    assert!(!attempt.join("home/.claude/.credentials.json").exists());
}

#[test]
fn a_record_not_yet_published_is_not_a_filed_attempt() {
    let f = Fixture::new("staged");
    f.claim("att-001", "");
    write(
        &f.filed_dir("att-001").join("attempt.json.abc123.partial"),
        r#"{"disposition": "valid completed"}"#,
    );
    let state = f.status();
    for bucket in ["valid completed", "harness-invalid", "stopped", "other"] {
        assert_eq!(
            text(
                field(&state, "filed")
                    .as_object()
                    .unwrap()
                    .get(bucket)
                    .unwrap()
            ),
            "0"
        );
    }
    let in_flight: Vec<String> = field(&state, "in_flight")
        .as_array()
        .unwrap()
        .iter()
        .map(text)
        .collect();
    assert_eq!(in_flight, ["att-001"]);
}

#[test]
fn valid_fixture_filed_through_the_runner_prunes_with_its_resolved_target() {
    let f = Fixture::new("pruned");
    let attempt = f.attempt("att-001", Some(0));
    assert!(!f.root.join("bench/targets").exists());
    let done = f.runner(&["--file", "att-001"]);
    assert_eq!(done.status.code(), Some(0), "{}", report(&done));
    assert_eq!(
        text(field(&parse(&stdout(&done)), "disposition")),
        "valid completed"
    );
    let receipt = read(&attempt.join("workspace-pruned.json"));
    assert!(matches!(field(&receipt, "applied"), Value::Bool(true)));
    assert!(!attempt.join("clone").exists());
    assert!(attempt.join("home").is_dir());
}

#[test]
fn fixture_that_no_longer_matches_its_target_remains() {
    let f = Fixture::new("changed");
    let attempt = f.attempt("att-001", Some(0));
    write(
        &attempt.join("clone/untracked.txt"),
        "left by the reviewer\n",
    );
    f.file_failing(
        "att-001",
        "evidence filed, but workspace cleanup failed: clone revision or working tree changed",
    );
    assert!(attempt.join("clone").is_dir());
    assert!(!attempt.join("workspace-pruned.json").exists());
}

#[test]
fn failed_fixtures_remain() {
    let f = Fixture::new("failed");
    let invalid = f.attempt("att-001", Some(0));
    write(
        &invalid.join("audit.json"),
        r#"{"violations": ["read outside the clone: /etc/hosts"], "diff_commands": ["git diff main...review-head"]}"#,
    );
    let stopped = f.attempt("att-002", Some(1));
    write(
        &stopped.join("stop.json"),
        r#"{"stopped_at": "2026-01-01T00:00:06Z", "exit_code": 1, "reason": "reviewer exit 1"}"#,
    );
    let unknown = f.attempt("att-003", Some(0));
    fs::remove_file(unknown.join("stdout.jsonl")).unwrap();
    for (attempt, disposition) in [(&invalid, "harness-invalid"), (&stopped, "stopped")] {
        let name = attempt.file_name().unwrap().to_str().unwrap();
        let done = f.runner(&["--file", name]);
        assert_eq!(done.status.code(), Some(0), "{}", report(&done));
        let filed = text(field(&parse(&stdout(&done)), "disposition"));
        assert!(filed.starts_with(disposition), "{filed}");
        assert!(attempt.join("clone").is_dir());
        assert!(!attempt.join("workspace-pruned.json").exists());
    }
    let record = f.file_failing("att-003", "workspace cleanup failed: usage is incomplete");
    assert_eq!(text(field(&record, "disposition")), "valid completed");
    assert!(unknown.join("clone").is_dir());
    write(
        &f.run_dir.join("charges.jsonl"),
        r#"{"at": "2026-01-02", "step": "unknown usage of att-003 accepted at its reservation", "usd": 0, "reconciles": "att-003"}
"#,
    );
    let pruned = Command::new(env!("CARGO_BIN_EXE_bench-tools"))
        .arg("prune-workspace")
        .arg("--attempt")
        .arg(f.filed_dir("att-003"))
        .arg("--workspace")
        .arg(&unknown)
        .arg("--target")
        .arg(f.run_dir.join("fixture"))
        .arg("--apply")
        .output()
        .unwrap();
    assert_eq!(pruned.status.code(), Some(0), "{}", report(&pruned));
    assert!(matches!(
        field(&parse(&stdout(&pruned)), "applied"),
        Value::Bool(true)
    ));
    assert!(!unknown.join("clone").exists());
    assert!(amount(field(&f.status(), "spend_usd.attempts")) >= 5.0);
}
