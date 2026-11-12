//! `bench-tools evaluator-audit second-pass` plan, run and authorize against fake grading steps and meters: no model
//! call, no repository write. The refusals before any queue write run the built command.

use bench_tools::evaluator_audit::second_pass::{self, Config, Effects, Run};
use bench_tools::pyjson::{self, Duplicates, Value};
use bench_tools::regrade;
use bench_tools::sha256::hex_digest;
use bench_tools::subscription_meters::{Pinned, Unreadable};
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

const ROUND: &str = "evaluator-audit-v2-test-round";
const PIN: &str = r#"{"bench-tools": {"sha256": "aaaa", "receipt_sha256": "bbbb"}}"#;

fn json(text: &str) -> Value {
    pyjson::loads(text, Duplicates::KeepLast).unwrap()
}

fn read(path: &Path) -> Value {
    json(&fs::read_to_string(path).unwrap())
}

fn get<'v>(value: &'v Value, key: &str) -> &'v Value {
    value.as_object().unwrap().get(key).unwrap()
}

fn write(path: &Path, contents: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, contents).unwrap();
}

fn sha(path: &Path) -> String {
    hex_digest(&fs::read(path).unwrap())
}

fn argument(args: &[String], name: &str) -> String {
    let at = args.iter().position(|arg| arg == name).unwrap();
    args[at + 1].clone()
}

/// A repository holding a round's sample, a plan of `targets` in `runs/r1` and a config.
struct Bench {
    root: PathBuf,
}

impl Drop for Bench {
    fn drop(&mut self) {
        let _ = std::process::Command::new("chmod")
            .args(["-R", "u+w"])
            .arg(&self.root)
            .status();
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn config(approval: &str) -> String {
    format!(
        r#"{{"contract": "evaluator-audit-second-pass-config/v1", "round": "{ROUND}", "plan": "plan.json",
  "regrades": [], "stateDirectory": ".local/state", "archiveRoot": "archive", "cacheRoot": ".local/cache",
  "plainQueue": "plain", "cacheReplacementQueues": {{}},
  "grader": {{"client": "claude", "model": "claude-sonnet-5-5"}}, "firstGrader": "claude-opus-5-5",
  "budgetPolicy": "claude-list-price-cap",
  "launch": {{"trialBatches": 5, "checkpointBatches": 5, "chunkBatches": 10, "workers": 3, "weekPointsLimit": 30,
    "resetMarginSeconds": 120, "signInMarginSeconds": 2400, "maxRetriesPerBatch": 1, "maxRetries": 8,
    "meterFailuresBeforeStop": 6}},
  "authorization": {{"path": "out/authorization.json", "scope": "the test round", "maxBudgetUsd": 6.0,
    "grader": {{"effort": "high", "cliVersion": "2.1.314", "timeoutSeconds": 60, "fastMode": "off",
      "client_pin": "the fake claude"}},
    "preflight": "preflight.json", "pinned": ["bench/rates.json"], "quotaStopPercent": 90,
    "quotaReceipt": {{"text": "stop at 90%", "path": "quota.json"}},
    "approval": {approval}}}}}"#
    )
}

fn bench(label: &str, targets: &[&str]) -> Bench {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("second-pass-{label}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    let root = fs::canonicalize(&root).unwrap();
    let current = root.join("bench/grading/current");
    write(&current.join("audits.json"), "{}");
    write(&current.join("grades.json"), "{}");
    write(&current.join("audit").join(ROUND).join("sample.json"), "{}");
    write(&root.join("receipt.json"), "{}");
    write(
        &root.join("bin/claude"),
        "#!/bin/sh\necho '2.1.314 (Claude Code)'\n",
    );
    let order: Vec<String> = targets
        .iter()
        .map(|target| {
            format!(
                r#"{{"run": "runs/r1", "target": "{target}", "units": 1, "queue": "plain", "cacheReplacements": null,
                    "firstGradeUsd": 1.0}}"#
            )
        })
        .collect();
    write(
        &root.join("plan.json"),
        &format!(
            r#"{{"contract": "evaluator-audit-second-pass-plan/v1", "round": "{ROUND}",
  "sample": {{"path": "bench/grading/current/audit/{ROUND}/sample.json", "sha256": "{}"}},
  "grades": {{"path": "bench/grading/current/grades.json", "sha256": "{}"}},
  "recordRoot": "bench/grading/current/audit/{ROUND}", "queueDirectory": ".local/q", "workspaceRoot": ".local/w",
  "archiveRoot": "archive", "cacheRoot": ".local/cache", "cacheReplacements": {{}}, "order": [{}]}}"#,
            sha(&current.join("audit").join(ROUND).join("sample.json")),
            sha(&current.join("grades.json")),
            order.join(", ")
        ),
    );
    write(&root.join("config.json"), &config("null"));
    Bench { root }
}

impl Bench {
    fn authorize(&self, cap: f64, reserve: f64) {
        write(
            &self.root.join("auth.json"),
            &format!(
                r#"{{"schemaVersion": 1, "budgetPolicy": "claude-list-price-cap", "budgetCapUsd": {cap:?},
  "maxBudgetUsd": {reserve:?}, "budgetReceipt": {{"ref": {{"path": "receipt.json", "sha256": "{}"}}}},
  "grader": {{"client": "claude", "model": "claude-sonnet-5-5", "effort": "high", "cliVersion": "2.1.314",
    "timeoutSeconds": 60}},
  "plan": {{"path": "plan.json", "sha256": "{}"}}, "runtimes": {PIN}, "quotaStopPercent": 90}}"#,
                sha(&self.root.join("receipt.json")),
                sha(&self.root.join("plan.json"))
            ),
        );
    }

    fn record(&self) -> PathBuf {
        self.root.join("bench/grading/current/audit").join(ROUND)
    }

    fn saved(&self) -> usize {
        fs::read_dir(self.record().join("second/r1"))
            .map(|targets| {
                targets
                    .filter(|target| {
                        target
                            .as_ref()
                            .unwrap()
                            .path()
                            .join("assessment-1/receipt.json")
                            .exists()
                    })
                    .count()
            })
            .unwrap_or(0)
    }

    fn status(&self) -> Value {
        read(&self.root.join(".local/q/status.json"))
    }

    fn run(&self, fake: &Fake, workers: usize, limit: Option<usize>) -> u8 {
        let run = Run {
            root: self.root.clone(),
            config: Config::load(&self.root, &self.root.join("config.json")).unwrap(),
            authorization: regrade::authorize(&self.root, &self.root.join("auth.json")).unwrap(),
            workers,
            limit,
        };
        second_pass::execute(fake, &run).unwrap()
    }

    fn fake(&self) -> Fake {
        Fake {
            root: self.root.clone(),
            used: 1.0,
            cost: "0.5".into(),
            dispatch_fail: None,
            second_fail: None,
            calls: Mutex::new(Vec::new()),
        }
    }
}

/// The fake bench-tools steps, meters and installed client.
struct Fake {
    root: PathBuf,
    used: f64,
    cost: String,
    dispatch_fail: Option<String>,
    second_fail: Option<String>,
    calls: Mutex<Vec<Vec<String>>>,
}

impl Fake {
    fn calls(&self) -> Vec<Vec<String>> {
        self.calls.lock().unwrap().clone()
    }

    fn budgets(&self) -> Vec<String> {
        self.calls()
            .iter()
            .filter(|call| call[..2] == ["grade", "dispatch"])
            .map(|call| argument(call, "--max-budget-usd"))
            .collect()
    }

    fn dispatch(&self, args: &[String]) -> i32 {
        assert!(!args.iter().any(|arg| arg.starts_with("--allow-unbounded")));
        assert_eq!(argument(args, "--model"), "claude-sonnet-5-5");
        let work = PathBuf::from(argument(args, "--work"));
        let key = read(Path::new(&argument(args, "--key")));
        let target = get(&key, "target").py_str();
        fs::create_dir(work.join("home")).unwrap();
        let failed = self.dispatch_fail.as_deref() == Some(target.as_str());
        write(
            &work.join("dispatch.json"),
            &format!(
                r#"{{"exit_code": {}, "verdicts_present": {}, "audit_violations": [], "usage": {{"high": {}}}}}"#,
                i32::from(failed),
                if failed { "false" } else { "true" },
                self.cost
            ),
        );
        i32::from(failed)
    }

    fn second(&self, args: &[String]) -> i32 {
        let key = read(Path::new(&argument(args, "--key")));
        let target = get(&key, "target").py_str();
        if self.second_fail.as_deref() == Some(target.as_str()) {
            return 1;
        }
        let run = get(&key, "run").py_str();
        let out = self
            .root
            .join("bench/grading/current/audit")
            .join(ROUND)
            .join("second")
            .join(Path::new(&run).file_name().unwrap())
            .join(target)
            .join("assessment-1");
        fs::create_dir_all(out.parent().unwrap()).unwrap();
        fs::create_dir(&out).unwrap();
        write(&out.join("receipt.json"), "{}");
        0
    }
}

impl regrade::Effects for Fake {
    fn step(&self, _: &[OsString], _: &Path, _: &OsStr) -> std::io::Result<i32> {
        unreachable!("a second pass runs its steps with tool")
    }

    fn fingerprints(&self) -> Result<Value, String> {
        unreachable!("a second pass reads no current fingerprints")
    }

    fn meters(&self, clients: &[Pinned], _: &OsStr) -> Result<Value, Unreadable> {
        assert_eq!(clients[0].name, "claude");
        Ok(json(&format!(
            r#"{{"at": "now", "claude": {{"five_hour": {{"utilization": 1.0}}, "seven_day": {{"utilization": {:?}}}}},
                "claude_sign_in_expires_at": 4102444800000}}"#,
            self.used
        )))
    }

    fn installed_client(&self, _: &str) -> Result<PathBuf, String> {
        Ok(self.root.join("bin/claude"))
    }

    fn client_version(&self, _: &Path) -> Result<String, String> {
        Ok("2.1.314".into())
    }

    fn now(&self) -> f64 {
        1_800_000_000.0
    }

    fn elapsed(&self) -> f64 {
        0.0
    }
}

impl Effects for Fake {
    fn tool(&self, args: &[OsString], log: &Path, _: &OsStr) -> std::io::Result<i32> {
        let args: Vec<String> = args
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        let mut handle = fs::OpenOptions::new().append(true).create(true).open(log)?;
        writeln!(handle, "$ bench-tools {}", args.join(" "))?;
        self.calls.lock().unwrap().push(args.clone());
        let words: Vec<&str> = args[..2].iter().map(String::as_str).collect();
        Ok(match words[..] {
            ["evaluator-audit", "claims"] => {
                write(Path::new(&argument(&args, "--out")), "{}");
                0
            }
            ["grade", "prepare"] => {
                assert_eq!(
                    argument(&args, "--cache-root"),
                    self.root.join(".local/cache").display().to_string()
                );
                fs::create_dir(argument(&args, "--work")).unwrap();
                write(
                    Path::new(&argument(&args, "--key")),
                    &format!(
                        r#"{{"run": "{}", "target": "{}"}}"#,
                        argument(&args, "--run"),
                        argument(&args, "--target")
                    ),
                );
                0
            }
            ["grade", "check-prepared"] | ["regrade", "archive-attempt"] => 0,
            ["grade", "dispatch"] => self.dispatch(&args),
            ["evaluator-audit", "second"] => self.second(&args),
            _ => panic!("unexpected bench-tools {args:?}"),
        })
    }

    fn driver(&self, _: &[OsString], _: &Path) -> std::io::Result<i32> {
        unreachable!()
    }

    fn runtime_pin(&self) -> Result<Value, String> {
        Ok(json(PIN))
    }

    fn sleep(&self, _: f64) {
        unreachable!()
    }

    fn sign_in_expires(&self) -> Option<f64> {
        unreachable!()
    }

    fn booted_at(&self) -> f64 {
        unreachable!()
    }
}

fn number(value: &Value) -> f64 {
    value.py_str().parse().unwrap()
}

#[test]
fn a_pass_saves_every_batch_with_the_documented_steps_and_never_maps_a_grade() {
    let bench = bench("completes", &["t0", "t1", "t2", "t3", "t4"]);
    bench.authorize(200.0, 6.0);
    let fake = bench.fake();
    assert_eq!(bench.run(&fake, 2, None), 0);
    let status = bench.status();
    assert_eq!(get(&status, "state").py_str(), "complete");
    assert_eq!(bench.saved(), 5);
    assert!((number(get(&status, "spentUpperUsd")) - 2.5).abs() < 1e-9);
    assert_eq!(number(get(&status, "reservedUsd")), 0.0);
    assert_eq!(
        fake.budgets(),
        vec!["6.0"; 5],
        "a session ran without the $6 limit"
    );
    assert_eq!(
        get(&read(&bench.root.join(".local/q/client.json")), "version").py_str(),
        "2.1.314"
    );
    let calls = fake.calls();
    assert!(
        calls.iter().all(|call| call[..2] != ["grade", "map"]),
        "a grade was mapped: {calls:?}"
    );
    let attempt = bench.root.join(".local/q/attempts/r1/t0/attempt-1");
    let work = fs::read_link(attempt.join("work")).unwrap();
    assert_eq!(work.parent().unwrap(), bench.root.join(".local/w"));
    let name = work.file_name().unwrap().to_str().unwrap();
    assert!(
        name.len() == 32 && name.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "the workspace name {name} is not a random hex id"
    );
    let (work, key, claims) = (
        work.display().to_string(),
        attempt.join("key.json").display().to_string(),
        attempt.join("claims.json").display().to_string(),
    );
    let root = bench.root.display().to_string();
    let cache = bench.root.join(".local/cache").display().to_string();
    let attempt = attempt.display().to_string();
    let expected: Vec<Vec<&str>> = vec![
        vec![
            "evaluator-audit",
            "claims",
            "--run",
            "runs/r1",
            "--target",
            "t0",
            "--out",
            &claims,
        ],
        vec![
            "grade",
            "prepare",
            "--run",
            "runs/r1",
            "--target",
            "t0",
            "--work",
            &work,
            "--key",
            &key,
            "--inventory",
            &claims,
            "--cache-root",
            &cache,
        ],
        vec![
            "grade",
            "check-prepared",
            "--work",
            &work,
            "--key",
            &key,
            "--dispatching",
        ],
        vec![
            "grade",
            "dispatch",
            "--work",
            &work,
            "--key",
            &key,
            "--model",
            "claude-sonnet-5-5",
            "--effort",
            "high",
            "--expected-cli-version",
            "2.1.314",
            "--max-budget-usd",
            "6.0",
            "--timeout",
            "60",
        ],
        vec!["evaluator-audit", "second", "--work", &work, "--key", &key],
        vec![
            "regrade",
            "archive-attempt",
            &attempt,
            "archive",
            "--root",
            &root,
        ],
    ];
    let of_t0: Vec<Vec<String>> = calls
        .into_iter()
        .filter(|call| call.iter().any(|arg| arg == "t0" || arg.contains("/t0/")))
        .collect();
    assert_eq!(of_t0, expected);
    let log = fs::read_to_string(
        bench
            .root
            .join(".local/q/attempts/r1/t0/attempt-1/audit.log"),
    )
    .unwrap();
    assert!(
        log.starts_with("$ bench-tools evaluator-audit claims --run runs/r1 --target t0"),
        "{log}"
    );
    let reservation = read(
        &bench
            .root
            .join(".local/q/attempts/r1/t0/attempt-1/reservation.json"),
    );
    assert_eq!(get(&reservation, "maxBudgetUsd").py_str(), "6.0");
    assert_eq!(
        get(&reservation, "budgetPolicy").py_str(),
        "claude-list-price-cap"
    );

    assert_eq!(bench.run(&fake, 1, None), 0);
    let status = bench.status();
    assert_eq!(get(&status, "state").py_str(), "complete");
    assert_eq!(get(&status, "invocations").as_array().unwrap().len(), 2);
}

#[test]
fn a_limited_pass_resumes_where_it_stopped() {
    let bench = bench("limit", &["t0", "t1", "t2"]);
    bench.authorize(200.0, 6.0);
    let fake = bench.fake();
    assert_eq!(bench.run(&fake, 1, Some(1)), 0);
    assert_eq!(get(&bench.status(), "state").py_str(), "limit-reached");
    assert_eq!(bench.saved(), 1);
    assert_eq!(bench.run(&fake, 1, None), 0);
    assert_eq!(bench.saved(), 3);
}

#[test]
fn the_cap_stops_the_pass_with_exit_3_before_a_reservation_it_cannot_fund() {
    let bench = bench("budget", &["t0", "t1", "t2", "t3", "t4"]);
    bench.authorize(12.0, 6.0);
    let mut fake = bench.fake();
    fake.cost = "2".into();
    assert_eq!(bench.run(&fake, 1, None), 3);
    let status = bench.status();
    assert_eq!(get(&status, "state").py_str(), "budget-stopped");
    assert_eq!(bench.saved(), 3);
    assert_eq!(number(get(&status, "spentUpperUsd")), 6.0);
}

#[test]
fn the_weekly_quota_stops_the_pass_with_exit_4_before_any_batch() {
    let bench = bench("quota", &["t0"]);
    bench.authorize(200.0, 6.0);
    let mut fake = bench.fake();
    fake.used = 90.0;
    assert_eq!(bench.run(&fake, 1, None), 4);
    let status = bench.status();
    assert_eq!(get(&status, "state").py_str(), "quota-stopped");
    assert_eq!(
        get(&status, "reason").py_str(),
        "Claude usage reached the quota stop"
    );
    assert_eq!(bench.saved(), 0);
    assert!(
        !bench.root.join(".local/q/attempts").exists(),
        "a batch was prepared after the quota stop"
    );
}

#[test]
fn a_failed_batch_stops_the_pass_until_its_disposition_allows_one_more_attempt() {
    let bench = bench("failure", &["t0", "t1", "t2"]);
    bench.authorize(200.0, 6.0);
    let mut fake = bench.fake();
    fake.dispatch_fail = Some("t1".into());
    assert_eq!(bench.run(&fake, 1, None), 1);
    assert_eq!(get(&bench.status(), "state").py_str(), "failed");
    assert_eq!(bench.saved(), 1);
    fake.dispatch_fail = None;
    assert_eq!(bench.run(&fake, 1, None), 1);
    assert!(
        get(&bench.status(), "reason")
            .py_str()
            .contains("disposition.json")
    );
    assert_eq!(bench.saved(), 1);
    write(
        &bench
            .root
            .join(".local/q/attempts/r1/t1/attempt-1/disposition.json"),
        r#"{"decision": "retry"}"#,
    );
    assert_eq!(bench.run(&fake, 1, None), 0);
    assert_eq!(bench.saved(), 3);
    assert!(
        bench
            .root
            .join(".local/q/attempts/r1/t1/attempt-2")
            .is_dir()
    );
    assert!((number(get(&bench.status(), "spentUpperUsd")) - 2.0).abs() < 1e-9);
}

#[test]
fn a_good_receipt_whose_save_failed_is_saved_again_without_another_dispatch() {
    let bench = bench("resave", &["t0"]);
    bench.authorize(200.0, 6.0);
    let mut fake = bench.fake();
    fake.second_fail = Some("t0".into());
    assert_eq!(bench.run(&fake, 1, None), 1);
    assert_eq!(bench.saved(), 0);
    fake.second_fail = None;
    assert_eq!(bench.run(&fake, 1, None), 0);
    assert_eq!(bench.saved(), 1);
    assert!(
        !bench
            .root
            .join(".local/q/attempts/r1/t0/attempt-2")
            .exists(),
        "the batch was dispatched again"
    );
    assert!((number(get(&bench.status(), "spentUpperUsd")) - 0.5).abs() < 1e-9);
}

#[test]
fn a_pass_runs_on_after_the_live_grades_change() {
    let bench = bench("grades", &["t0", "t1"]);
    bench.authorize(200.0, 6.0);
    let fake = bench.fake();
    assert_eq!(bench.run(&fake, 1, Some(1)), 0);
    assert_eq!(bench.saved(), 1);
    write(
        &bench.root.join("bench/grading/current/grades.json"),
        r#"{"batches": ["a new batch"]}"#,
    );
    assert_eq!(bench.run(&fake, 1, None), 0);
    assert_eq!(get(&bench.status(), "state").py_str(), "complete");
    assert_eq!(bench.saved(), 2);
}

fn command(bench: &Bench, args: &[&str], env: &[(&str, &str)]) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_bench-tools"));
    command
        .args(["evaluator-audit", "second-pass"])
        .args(args)
        .arg("--root")
        .arg(&bench.root)
        .arg("--config")
        .arg(bench.root.join("config.json"))
        .env_remove("BENCH_REGRADE_PINNED_RUNTIME")
        .env_remove(second_pass::FAST_MODE_OFF);
    for (name, value) in env {
        command.env(name, value);
    }
    command.output().unwrap()
}

fn refused(output: &std::process::Output) -> String {
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    stderr
        .strip_prefix("refused: ")
        .unwrap_or_else(|| panic!("{stderr}"))
        .trim_end()
        .to_string()
}

#[test]
fn run_refuses_without_fast_mode_disabled_and_writes_nothing() {
    let bench = bench("fast-mode", &["t0"]);
    bench.authorize(200.0, 6.0);
    let output = command(
        &bench,
        &["run", "--authorization", "auth.json"],
        &[("BENCH_RUNTIME_STORE", "/fake")],
    );
    assert!(refused(&output).contains("fast mode"), "{output:?}");
    assert!(!bench.root.join(".local/q").exists());
}

#[test]
fn run_refuses_a_runtime_the_authorization_does_not_pin_and_writes_nothing() {
    let bench = bench("runtime", &["t0"]);
    bench.authorize(200.0, 6.0);
    let output = command(
        &bench,
        &["run", "--authorization", "auth.json"],
        &[
            ("BENCH_RUNTIME_STORE", "/fake"),
            (second_pass::FAST_MODE_OFF, "1"),
        ],
    );
    assert!(
        refused(&output).starts_with("the authorization's pinned bench-tools is unavailable"),
        "{output:?}"
    );
    assert!(!bench.root.join(".local/q").exists());
}

fn approval(bench: &Bench, words: &str) -> String {
    let receipt = bench.root.join("bench/grading/rulings/approval.md");
    write(
        &receipt,
        &format!("# Audit decision\n\nRound `{ROUND}`, $130 cap.\n\n> {words}\n"),
    );
    format!(
        r#"{{"receipt": "bench/grading/rulings/approval.md", "sha256": "{}", "text": "go with rec",
            "context": "the test approval", "capUsd": 130.0}}"#,
        sha(&receipt)
    )
}

#[test]
fn authorize_refuses_and_writes_nothing_without_the_owner_approval_receipt() {
    let bench = bench("unapproved", &["t0"]);
    let output = command(&bench, &["authorize"], &[]);
    assert!(
        refused(&output).starts_with(&format!(
            "no owner approval of round {ROUND}'s second grader and spend is filed"
        )),
        "{output:?}"
    );
    let pinned = approval(&bench, "go with rec");
    fs::write(
        bench.root.join("bench/grading/rulings/approval.md"),
        "an edited receipt",
    )
    .unwrap();
    write(&bench.root.join("config.json"), &config(&pinned));
    let output = command(&bench, &["authorize"], &[]);
    assert!(
        refused(&output).contains("is missing or is not the one authorization.approval pins"),
        "{output:?}"
    );
    write(
        &bench.root.join("config.json"),
        &config(&approval(&bench, "not now")),
    );
    let output = command(&bench, &["authorize"], &[]);
    assert!(
        refused(&output).contains("does not hold the owner's words"),
        "{output:?}"
    );
    assert!(
        !bench.root.join("out").exists(),
        "an authorization was written"
    );
}

#[test]
fn authorize_pins_the_approval_grader_cap_quota_and_runner() {
    let bench = bench("approved", &["t0"]);
    for path in ["preflight.json", "quota.json", "bench/rates.json"] {
        write(&bench.root.join(path), "{}");
    }
    write(
        &bench.root.join("config.json"),
        &config(&approval(&bench, "go with rec")),
    );
    let config = Config::load(&bench.root, &bench.root.join("config.json")).unwrap();
    let fake = bench.fake();
    assert_eq!(
        second_pass::authorize(&fake, &bench.root, &config).unwrap(),
        "cap $130.00, 1 batches"
    );
    let written = bench.root.join("out/authorization.json");
    let authorization = read(&written);
    let receipt = format!(
        r#"{{"path": "bench/grading/rulings/approval.md", "sha256": "{}"}}"#,
        sha(&bench.root.join("bench/grading/rulings/approval.md"))
    );
    let budget = get(&authorization, "budgetReceipt");
    assert!(get(budget, "ref").py_eq(&json(&receipt)));
    assert_eq!(get(budget, "text").py_str(), "go with rec");
    assert_eq!(
        get(&authorization, "budgetPolicy").py_str(),
        "claude-list-price-cap"
    );
    assert_eq!(get(&authorization, "budgetCapUsd").py_str(), "130.0");
    assert_eq!(get(&authorization, "maxBudgetUsd").py_str(), "6.0");
    assert_eq!(get(&authorization, "quotaStopPercent").py_str(), "90");
    let grader = get(&authorization, "grader");
    for (key, value) in [
        ("client", "claude"),
        ("model", "claude-sonnet-5-5"),
        ("effort", "high"),
        ("cliVersion", "2.1.314"),
        ("fastMode", "off"),
    ] {
        assert_eq!(get(grader, key).py_str(), value);
    }
    assert!(get(&authorization, "runtimes").py_eq(&json(PIN)));
    let runner = get(&authorization, "runner");
    assert_eq!(get(runner, "retriesPerBatch").py_str(), "1");
    assert!(get(runner, "workspaceNames").py_str().starts_with("random"));
    let deviations: Vec<String> = get(&authorization, "runnerDeviations")
        .as_array()
        .unwrap()
        .iter()
        .map(|pin| get(pin, "path").py_str())
        .collect();
    assert_eq!(
        deviations,
        [
            "config.json",
            "bench/rates.json",
            "bench/grading/rulings/approval.md"
        ]
    );
    assert!(
        second_pass::authorize(&fake, &bench.root, &config)
            .unwrap_err()
            .contains("an authorization is written once")
    );
    assert!(regrade::authorize(&bench.root, &written).is_ok());
}

#[test]
fn plan_orders_the_sample_with_each_batch_s_one_cache_manifest() {
    let bench = bench("plan", &[]);
    let root = &bench.root;
    write(
        &root
            .join("bench/grading/current/audit")
            .join(ROUND)
            .join("sample.json"),
        r#"{"grades": {"path": "bench/grading/current/grades.json", "sha256": "g"},
            "batches": [{"run": "runs/r1", "target": "a", "units": 2}, {"run": "runs/r2", "target": "b", "units": 3}]}"#,
    );
    write(&root.join("manifest.json"), "{}");
    write(
        &root.join("regrade/plan.json"),
        r#"{"order": [{"run": "runs/r1", "target": "a"}]}"#,
    );
    write(
        &root.join("regrade/authorization.v1.json"),
        &format!(
            r#"{{"executionPlan": {{"path": "regrade/plan.json"}}, "cacheReplacements": {{"path": "manifest.json", "sha256": "{}"}}}}"#,
            sha(&root.join("manifest.json"))
        ),
    );
    write(
        &root.join("regrade/plain.json"),
        r#"{"order": [{"run": "runs/r2", "target": "b"}]}"#,
    );
    write(
        &root.join("regrade/authorization.v2.json"),
        r#"{"executionPlan": {"path": "regrade/plain.json"}}"#,
    );
    write(
        &root.join("regrade/authorization.v3.json"),
        r#"{"no": "plan"}"#,
    );
    write(
        &root.join("bench/grading/current/assessments/r1/a/assessment-1/receipt.json"),
        r#"{"provenance": {"model": "claude-opus-5-5", "dispatched_at": "2026-11-01T00:00:00Z", "usage": {"high": 1.25}}}"#,
    );
    write(
        &root.join("bench/grading/current/assessments/r1/a/assessment-2/receipt.json"),
        r#"{"provenance": {"model": "claude-opus-5-5", "dispatched_at": "2026-11-02T00:00:00Z", "usage": {"high": 2.5}}}"#,
    );
    write(
        &root.join("bench/grading/current/assessments/r1/a/assessment-3/receipt.json"),
        r#"{"provenance": {"model": "gpt-6.1-sol", "dispatched_at": "2026-11-03T00:00:00Z", "usage": {"high": 9}}}"#,
    );
    let text = config("null")
        .replace(r#""regrades": []"#, r#""regrades": ["regrade"]"#)
        .replace(
            r#""cacheReplacementQueues": {}"#,
            r#""cacheReplacementQueues": {"manifest.json": "rebuilt"}"#,
        );
    write(&root.join("config.json"), &text);
    let config = Config::load(root, &root.join("config.json")).unwrap();
    let out = root.join("out/plan.json");
    assert_eq!(
        second_pass::plan(root, &config, &out).unwrap(),
        format!("planned 2 batches, 5 units: {}", out.display())
    );
    let plan = read(&out);
    assert_eq!(get(&plan, "queueDirectory").py_str(), ".local/state/queue");
    assert!(
        get(get(&plan, "cacheReplacements"), "rebuilt").py_eq(&json(&format!(
            r#"{{"path": "manifest.json", "sha256": "{}"}}"#,
            sha(&root.join("manifest.json"))
        )))
    );
    let order = get(&plan, "order").as_array().unwrap();
    assert_eq!(get(&order[0], "queue").py_str(), "rebuilt");
    assert_eq!(get(&order[0], "cacheReplacements").py_str(), "rebuilt");
    assert_eq!(get(&order[0], "firstGradeUsd").py_str(), "2.5");
    assert_eq!(get(&order[1], "queue").py_str(), "plain");
    assert!(matches!(get(&order[1], "cacheReplacements"), Value::Null));
    assert!(matches!(get(&order[1], "firstGradeUsd"), Value::Null));
    assert!(
        second_pass::plan(root, &config, &out)
            .unwrap_err()
            .contains("cannot create")
    );
}
