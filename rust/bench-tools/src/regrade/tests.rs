use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Condvar, Mutex};

fn json(text: &str) -> Value {
    pyjson::loads(text, Duplicates::KeepLast).unwrap()
}

fn write(path: &Path, contents: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, contents).unwrap();
}

fn pin(root: &Path, relative: &str) -> String {
    format!(
        r#"{{"path": "{relative}", "sha256": "{}"}}"#,
        digest(&root.join(relative)).unwrap()
    )
}

fn usd(text: &str) -> Usd {
    Usd::parse(text).unwrap()
}

/// What the fake grading steps and meters do.
struct Script {
    /// Claude utilization, or the reason the meters cannot be read.
    usage: Result<f64, String>,
    /// The seconds the clock moves at each dispatch.
    tick: f64,
    /// Utilization set by the dispatch of this target.
    raise_after: Option<(String, f64)>,
    cost: &'static str,
    failing: HashMap<String, i32>,
    receiptless: HashSet<String>,
    /// Targets whose dispatch writes a receipt and then fails to report an exit code.
    lost: HashSet<String>,
    shared_session: bool,
    map_code: i32,
    blocked: HashSet<String>,
    preflight_code: i32,
    version: &'static str,
    /// When the stored Claude sign-in expires, in milliseconds since 1970.
    expires: Option<f64>,
    /// The cache-replacement manifest digest prepare pins, in place of the digest of the manifest it was given.
    prepared_manifest: Option<&'static str>,
    /// After this step runs, overwrite the file with these contents.
    rewrite_after: Option<(&'static str, PathBuf, &'static str)>,
}

impl Default for Script {
    fn default() -> Self {
        Script {
            usage: Ok(10.0),
            tick: 0.0,
            raise_after: None,
            cost: "1.0",
            failing: HashMap::new(),
            receiptless: HashSet::new(),
            lost: HashSet::new(),
            shared_session: false,
            map_code: 0,
            blocked: HashSet::new(),
            preflight_code: 0,
            version: "1.2.3",
            expires: None,
            prepared_manifest: None,
            rewrite_after: None,
        }
    }
}

struct Fake {
    root: PathBuf,
    client: PathBuf,
    script: Mutex<Script>,
    calls: Mutex<Vec<Vec<String>>>,
    reads: AtomicUsize,
    clock: Mutex<f64>,
    together: usize,
    running: Mutex<usize>,
    joined: Condvar,
    codex: bool,
}

impl Fake {
    fn calls(&self, step: &str) -> Vec<Vec<String>> {
        self.calls
            .lock()
            .unwrap()
            .iter()
            .filter(|call| call[1] == step)
            .cloned()
            .collect()
    }

    fn reading(&self, percent: f64, expires: Option<f64>) -> Value {
        let mut reading = if self.codex {
            json(&format!(
                r#"{{"at": "now", "codex": {{"ordinaryUsageAllowed": true, "rateLimits":
                    {{"primary": {{"usedPercent": {percent}}}, "secondary": {{"usedPercent": {percent}}}}}}}}}"#
            ))
        } else {
            json(&format!(
                r#"{{"at": "now", "claude": {{"five_hour": {{"utilization": {percent}}}}}}}"#
            ))
        };
        if let (Some(expires), Value::Object(object)) = (expires, &mut reading) {
            object.insert("claude_sign_in_expires_at".into(), Value::Float(expires));
        }
        reading
    }

    fn prepare(&self, args: &[String]) -> std::io::Result<i32> {
        let argument = |name: &str| {
            let at = args.iter().position(|arg| arg == name)?;
            Some(PathBuf::from(&args[at + 1]))
        };
        std::fs::create_dir_all(argument("--work").unwrap()).unwrap();
        let target = argument("--target").unwrap();
        let mut key = json(&format!(
            r#"{{"input_fingerprint": "fp-{}"}}"#,
            target.display()
        ));
        if let (Some(manifest), Value::Object(object)) =
            (argument("--cache-replacements"), &mut key)
        {
            let sha256 = match self.script.lock().unwrap().prepared_manifest {
                Some(sha256) => sha256.to_string(),
                None => digest(&manifest).unwrap(),
            };
            object.insert(
                "runner_deviation".into(),
                json(&format!(
                    r#"{{"provisioning": {{"manifest": {{"sha256": "{sha256}"}}}}}}"#
                )),
            );
        }
        write(&argument("--key").unwrap(), &dumps(&key));
        Ok(0)
    }

    fn dispatch(&self, args: &[String]) -> std::io::Result<i32> {
        let argument = |name: &str| {
            let at = args.iter().position(|arg| arg == name).unwrap();
            PathBuf::from(&args[at + 1])
        };
        let (work, key) = (argument("--work"), argument("--key"));
        let attempt = key.parent().unwrap();
        let target = attempt
            .parent()
            .unwrap()
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        let tag = format!(
            "{target}-{}",
            attempt.file_name().unwrap().to_str().unwrap()
        );
        if self.together > 1 {
            let mut running = self.running.lock().unwrap();
            *running += 1;
            self.joined.notify_all();
            let (running, _) = self
                .joined
                .wait_timeout_while(running, Duration::from_secs(10), |running| {
                    *running < self.together
                })
                .unwrap();
            drop(running);
        }
        let script = self.script.lock().unwrap();
        *self.clock.lock().unwrap() += script.tick;
        let code = script.failing.get(&target).copied().unwrap_or(0);
        if !script.receiptless.contains(&target) {
            std::fs::create_dir_all(work.join("home")).unwrap();
            let session = if script.shared_session {
                "shared".to_string()
            } else {
                format!("session-{tag}")
            };
            write(
                &work.join("dispatch.json"),
                &format!(
                    r#"{{"exit_code": {code}, "verdicts_present": true, "audit_violations": [],
                        "usage": {{"high": {}}}, "session_id": "{session}", "models_observed": ["m"]}}"#,
                    script.cost
                ),
            );
            write(
                &work.join("clean-context.json"),
                &format!(r#"{{"context_id": "context-{tag}"}}"#),
            );
            write(
                &work.join("verdicts.json"),
                if script.blocked.contains(&target) {
                    r#"{"reviews": {"review-1": {"items": {"1": {"claims": [{"id": "c1",
                        "open": {"kind": "blocked-check", "command": "go run ./cmd/import-boss",
                            "would_settle": "Whether any file still imports the package."}}]}}}}}"#
                } else {
                    r#"{"reviews": {}}"#
                },
            );
            write(
                &work.join("home/.claude/projects/fixture/session.jsonl"),
                "fixture transcript\n",
            );
        }
        let lost = script.lost.contains(&target);
        drop(script);
        let mut script = self.script.lock().unwrap();
        if let Some((after, percent)) = script.raise_after.clone()
            && after == target
        {
            script.usage = Ok(percent);
        }
        if lost {
            return Err(std::io::Error::other("the controller lost the dispatch"));
        }
        Ok(code)
    }
}

impl Effects for Fake {
    fn step(&self, args: &[OsString], log: &Path, path: &OsStr) -> std::io::Result<i32> {
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(log)?;
        assert!(
            path.to_string_lossy().contains("client-bin"),
            "steps run the queue's client"
        );
        let args: Vec<String> = args
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        self.calls.lock().unwrap().push(args.clone());
        assert_eq!(args[0], "grade");
        let code = match args[1].as_str() {
            "preflight" => Ok(self.script.lock().unwrap().preflight_code),
            "prepare" => self.prepare(&args),
            "dispatch" => self.dispatch(&args),
            "map" => {
                let code = self.script.lock().unwrap().map_code;
                if code != 0 {
                    Ok(code)
                } else {
                    let at = args.iter().position(|arg| arg == "--work").unwrap();
                    let verdicts =
                        read_json(&Path::new(&args[at + 1]).join("verdicts.json")).unwrap();
                    let blocked = crate::grade::blocked_checks(&verdicts);
                    if blocked.is_empty() {
                        Ok(0)
                    } else {
                        write(log, &format!("{}\n", blocked.join("\n")));
                        Ok(i32::from(crate::grade::BLOCKED_CHECK_EXIT))
                    }
                }
            }
            other => panic!("unexpected step {other}"),
        };
        if let Some((step, path, contents)) = &self.script.lock().unwrap().rewrite_after
            && step == &args[1]
        {
            write(path, contents);
        }
        code
    }

    fn fingerprints(&self) -> R<Value> {
        read_json(&self.root.join("fingerprints.json"))
    }

    fn meters(&self, _clients: &[Pinned], path: &OsStr) -> Result<Value, Unreadable> {
        assert!(path.to_string_lossy().contains("client-bin"));
        self.reads.fetch_add(1, Ordering::SeqCst);
        let script = self.script.lock().unwrap();
        match script.usage.clone() {
            Ok(percent) => Ok(self.reading(percent, script.expires)),
            Err(message) => Err(Unreadable {
                message,
                sign_in: false,
            }),
        }
    }

    fn installed_client(&self, _name: &str) -> R<PathBuf> {
        Ok(self.client.clone())
    }

    fn client_version(&self, _executable: &Path) -> R<String> {
        Ok(self.script.lock().unwrap().version.to_string())
    }

    fn now(&self) -> f64 {
        *self.clock.lock().unwrap()
    }

    fn elapsed(&self) -> f64 {
        0.0
    }
}

/// A disposable repository with one planned run of reviewed targets and a queue directory.
struct Bench {
    root: PathBuf,
    directory: PathBuf,
    fake: Fake,
}

impl Drop for Bench {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

const RUN: &str = "runs/r1";

impl Bench {
    fn new(targets: &[&str], authorization: &str) -> Bench {
        Bench::with(targets, authorization, |_| {})
    }

    /// `authorization` is the authorization's fields beyond its pinned plans; `adjust` edits the plan first.
    fn with(targets: &[&str], authorization: &str, adjust: impl FnOnce(&mut Object)) -> Bench {
        let root = std::env::temp_dir().join(format!("regrade-{}", hex_id().unwrap()));
        std::fs::create_dir_all(&root).unwrap();
        let root = root.canonicalize().unwrap();
        let mut reviews = Vec::new();
        let mut batches = Vec::new();
        let mut order = Vec::new();
        let mut rows = Vec::new();
        for target in targets {
            write(&root.join(format!("reviews/{target}.md")), target);
            write(&root.join(format!("records/{target}.json")), "{}");
            reviews.push(format!(
                r#"{{"run": "{RUN}", "target": "{target}", "items": 10, "review": {}, "record": {}}}"#,
                pin(&root, &format!("reviews/{target}.md")),
                pin(&root, &format!("records/{target}.json"))
            ));
            batches.push(format!(
                r#"{{"run": "{RUN}", "target": "{target}", "state": "pending", "inputFingerprint": "fp-{target}"}}"#
            ));
            order.push(format!(r#"{{"run": "{RUN}", "target": "{target}"}}"#));
            rows.push(format!(
                r#"{{"run": "{RUN}", "target": "{target}", "state": "pending", "input_fingerprint": "fp-{target}"}}"#
            ));
        }
        let mut plan = json(&format!(
            r#"{{"contract": "{CONTRACT}", "reviews": [{}], "batches": [{}]}}"#,
            reviews.join(","),
            batches.join(",")
        ));
        if let Value::Object(object) = &mut plan {
            adjust(object);
        }
        write(&root.join("plan.json"), &dumps(&plan));
        write(
            &root.join("execution.json"),
            &format!(
                r#"{{"order": [{}], "workspaceRoot": "queue/workspaces", "archiveRoot": "queue/archives"}}"#,
                order.join(",")
            ),
        );
        write(
            &root.join("fingerprints.json"),
            &format!("[{}]", rows.join(",")),
        );
        let grants = format!(
            r#"{{"sourcePlan": {}, "executionPlan": {}, {authorization}}}"#,
            pin(&root, "plan.json"),
            pin(&root, "execution.json")
        );
        write(&root.join("authorization.json"), &grants);
        write(&root.join("bin/claude"), "the installed client");
        let codex = authorization.contains("gpt-");
        Bench {
            directory: root.join("queue/q"),
            fake: Fake {
                client: root.join("bin/claude"),
                root: root.clone(),
                script: Mutex::new(Script::default()),
                calls: Mutex::new(Vec::new()),
                reads: AtomicUsize::new(0),
                clock: Mutex::new(1_000_000.0),
                together: 1,
                running: Mutex::new(0),
                joined: Condvar::new(),
                codex,
            },
            root,
        }
    }

    /// Pin a cache-replacement manifest in the authorization, and a cache root in the execution plan when given.
    fn replace_cache(&self, cache_root: Option<&str>) {
        write(&self.root.join(REPLACEMENTS), r#"{"targets": []}"#);
        if let Some(cache_root) = cache_root {
            let path = self.root.join("execution.json");
            let mut execution = read_json(&path).unwrap();
            if let Value::Object(object) = &mut execution {
                object.insert("cacheRoot".into(), string(cache_root));
            }
            write(&path, &dumps(&execution));
        }
        let path = self.root.join("authorization.json");
        let mut authorization = read_json(&path).unwrap();
        if let Value::Object(object) = &mut authorization {
            object.insert(
                "executionPlan".into(),
                json(&pin(&self.root, "execution.json")),
            );
            object.insert(
                "cacheReplacements".into(),
                json(&pin(&self.root, REPLACEMENTS)),
            );
        }
        write(&path, &dumps(&authorization));
    }

    fn reservations(&self) -> Vec<PathBuf> {
        reservations(&self.directory).unwrap()
    }

    fn script(&self) -> std::sync::MutexGuard<'_, Script> {
        self.fake.script.lock().unwrap()
    }

    fn run_with(&self, limit: Option<usize>, workers: usize, expected: Option<&str>) -> R<u8> {
        std::fs::create_dir_all(&self.directory).unwrap();
        let run = Run {
            root: self.root.clone(),
            directory: self.directory.clone(),
            authorization: authorize(&self.root, &self.root.join("authorization.json"))?,
            limit,
            workers,
            expected_cli_version: expected.map(str::to_string),
        };
        execute(&self.fake, &run)
    }

    fn run(&self) -> R<u8> {
        self.run_with(None, 1, None)
    }

    fn status(&self) -> Value {
        read_json(&self.directory.join("status.json")).unwrap()
    }

    fn rows(&self) -> Vec<(String, String)> {
        list(&self.status(), "batches")
            .unwrap()
            .iter()
            .map(|row| {
                (
                    text(row, "target").unwrap().into(),
                    text(row, "state").unwrap().into(),
                )
            })
            .collect()
    }

    fn attempt(&self, target: &str, number: u32) -> PathBuf {
        self.directory
            .join(format!("batches/r1/{target}/attempt-{number}"))
    }

    fn dispatched(&self) -> Vec<String> {
        self.fake
            .calls("dispatch")
            .iter()
            .map(|call| {
                let key = &call[call.iter().position(|arg| arg == "--key").unwrap() + 1];
                Path::new(key)
                    .parent()
                    .unwrap()
                    .parent()
                    .unwrap()
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .into()
            })
            .collect()
    }
}

const CLAUDE: &str = r#""budgetCapUsd": 20, "grader": {"model": "claude-test", "effort": "high"}"#;

const REPLACEMENTS: &str = "cache-replacements.json";

fn states(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs
        .iter()
        .map(|(target, state)| (target.to_string(), state.to_string()))
        .collect()
}

fn reason(bench: &Bench) -> String {
    text(&bench.status(), "reason").unwrap().to_string()
}

fn state(bench: &Bench) -> String {
    text(&bench.status(), "state").unwrap().to_string()
}

#[test]
fn money_is_exact_and_never_negative() {
    assert_eq!(Usd::of(&json("2.1")).unwrap().to_string(), "2.1");
    assert_eq!(Usd::of(&json("20")).unwrap(), Usd::ONE.times(20));
    assert_eq!(Usd::of(&json("-0.0")).unwrap(), Usd::default());
    for refused in ["-1", "\"1\"", "true", "null"] {
        assert_eq!(
            Usd::of(&json(refused)).unwrap_err(),
            format!("invalid USD amount: {}", json(refused).py_str())
        );
    }
    assert!(Usd::of(&Value::Float(f64::NAN)).is_err());
    assert_eq!(usd("0.1").plus(usd("0.2")), usd("0.3"));
}

#[test]
fn allowance_keeps_a_dollar_of_headroom_and_floors_to_cents() {
    assert_eq!(desired(0), usd("3"));
    assert_eq!(desired(40), usd("3.9"));
    assert_eq!(desired(100), usd("6"));
    assert_eq!(allowance(usd("10"), usd("6.5"), 0), Some(usd("2.5")));
    assert_eq!(allowance(usd("10"), usd("5.555"), 100), Some(usd("3.44")));
    assert_eq!(allowance(usd("10"), usd("8.01"), 0), None);
    assert_eq!(allowance(usd("10"), usd("8"), 0), Some(usd("1")));
}

#[test]
fn plan_paths_stay_in_the_repository() {
    for escaping in ["/tmp/x", "a/../../x"] {
        assert_eq!(
            planned_root(Path::new("/repo"), escaping).unwrap_err(),
            format!("execution plan path leaves the repository: {escaping}")
        );
    }
}

#[test]
fn maps_every_batch_within_its_reservation() {
    let bench = Bench::new(&["t1", "t2", "t3"], CLAUDE);
    assert_eq!(bench.run(), Ok(0));
    assert_eq!(state(&bench), "mapped");
    assert_eq!(reason(&bench), "Awaiting the declared evaluator audit");
    assert_eq!(
        bench.rows(),
        states(&[("t1", "mapped"), ("t2", "mapped"), ("t3", "mapped")])
    );
    let status = bench.status();
    assert_eq!(dumps(field(&status, "spentUpperUsd").unwrap()), "3.0");
    assert_eq!(
        dumps(field(&status, "outstandingReservations").unwrap()),
        "0"
    );
    let row = &list(&status, "batches").unwrap()[0];
    assert_eq!(
        text(row, "workspace").unwrap(),
        "queue/q/batches/r1/t1/attempt-1"
    );
    assert_eq!(text(row, "sessionId").unwrap(), "session-t1-attempt-1");
    assert!(field(row, "evidence").unwrap().as_object().is_some());
    let reservation = read_json(&bench.attempt("t1", 1).join("reservation.json")).unwrap();
    assert_eq!(dumps(field(&reservation, "maxBudgetUsd").unwrap()), "3.0");
    assert_eq!(
        text(&reservation, "authorizationSha256").unwrap(),
        digest(&bench.root.join("authorization.json")).unwrap()
    );
    let preflight = bench.fake.calls("preflight");
    assert_eq!(preflight.len(), 1);
    assert_eq!(
        preflight[0].iter().filter(|arg| *arg == "--target").count(),
        3
    );
    assert!(
        preflight[0]
            .windows(2)
            .any(|pair| pair == ["--expected-cli-version", "1.2.3"])
    );
    let dispatch = &bench.fake.calls("dispatch")[0];
    for pair in [
        ["--max-budget-usd", "3.00"],
        ["--timeout", "900"],
        ["--effort", "high"],
    ] {
        assert!(
            dispatch.windows(2).any(|found| found == pair),
            "{pair:?} in {dispatch:?}"
        );
    }
    assert!(!dispatch.contains(&"--allow-unbounded-codex".to_string()));
    assert_eq!(bench.fake.calls("map").len(), 3);
}

#[test]
fn stops_at_the_cap_and_a_rerun_dispatches_nothing_more() {
    let bench = Bench::new(&["t1", "t2", "t3"], &CLAUDE.replace("20", "7"));
    bench.script().cost = "3.0";
    assert_eq!(bench.run(), Ok(3));
    assert_eq!(state(&bench), "budget-stopped");
    assert_eq!(
        reason(&bench),
        "Insufficient reserved budget for another batch"
    );
    assert_eq!(
        bench.rows(),
        states(&[("t1", "mapped"), ("t2", "mapped"), ("t3", "pending")])
    );
    assert_eq!(bench.run(), Ok(3));
    assert_eq!(bench.dispatched(), ["t1", "t2"]);
    assert_eq!(bench.fake.calls("preflight").len(), 1);
}

#[test]
fn a_metered_charge_over_the_cap_stops_the_queue() {
    let bench = Bench::new(&["t1", "t2"], &CLAUDE.replace("20", "4"));
    bench.script().cost = "4.5";
    assert_eq!(bench.run(), Ok(3));
    assert_eq!(reason(&bench), "Metered usage reached the total cap");
    assert_eq!(
        bench.rows(),
        states(&[("t1", "budget-stopped"), ("t2", "pending")])
    );
}

#[test]
fn the_quota_stops_launches_and_a_rerun_resumes() {
    let bench = Bench::new(&["t1", "t2"], CLAUDE);
    bench.script().usage = Ok(96.0);
    assert_eq!(bench.run(), Ok(4));
    assert_eq!(state(&bench), QUOTA_STOPPED);
    assert_eq!(reason(&bench), "Claude usage reached the quota stop");
    assert!(bench.fake.calls.lock().unwrap().is_empty());
    bench.script().usage = Ok(10.0);
    assert_eq!(bench.run(), Ok(0));
    assert_eq!(bench.rows(), states(&[("t1", "mapped"), ("t2", "mapped")]));
    assert_eq!(list(&bench.status(), "invocations").unwrap().len(), 2);
}

#[test]
fn a_reading_that_crosses_the_stop_halts_later_launches() {
    let bench = Bench::new(&["t1", "t2", "t3"], CLAUDE);
    {
        let mut script = bench.script();
        script.tick = 301.0;
        script.raise_after = Some(("t1".into(), 95.0));
    }
    assert_eq!(bench.run(), Ok(4));
    assert_eq!(
        bench.rows(),
        states(&[("t1", "mapped"), ("t2", "pending"), ("t3", "pending")])
    );
    assert_eq!(bench.dispatched(), ["t1"]);
}

#[test]
fn unreadable_meters_stop_like_the_quota() {
    let bench = Bench::new(&["t1"], CLAUDE);
    bench.script().usage = Err("no stored sign-in".into());
    assert_eq!(bench.run(), Ok(4));
    assert_eq!(
        reason(&bench),
        "The subscription meters could not be read: no stored sign-in"
    );
}

#[test]
fn a_reading_serves_five_minutes_of_launches() {
    let bench = Bench::new(&["t1", "t2", "t3"], CLAUDE);
    assert_eq!(bench.run(), Ok(0));
    assert_eq!(bench.fake.reads.load(Ordering::SeqCst), 1);
    let bench = Bench::new(&["t1", "t2", "t3"], CLAUDE);
    bench.script().tick = 301.0;
    assert_eq!(bench.run(), Ok(0));
    assert_eq!(bench.fake.reads.load(Ordering::SeqCst), 3);
}

#[test]
fn an_expiring_sign_in_stops_launches() {
    let bench = Bench::new(&["t1"], CLAUDE);
    let expires = (bench.fake.now() + 60.0) * 1000.0;
    bench.script().expires = Some(expires);
    assert_eq!(bench.run(), Ok(4));
    assert_eq!(
        reason(&bench),
        "the stored Claude sign-in expires within half an hour; renew it while no grading session runs"
    );
    bench.script().expires = Some(expires + 7_200_000.0);
    assert_eq!(bench.run(), Ok(0));
}

#[test]
fn a_failed_dispatch_needs_investigation_and_never_runs_again() {
    let bench = Bench::new(&["t1", "t2", "t3"], CLAUDE);
    bench.script().failing.insert("t2".into(), 1);
    assert_eq!(bench.run(), Ok(1));
    assert_eq!(
        reason(&bench),
        format!(
            "Paid attempt requires investigation: {}",
            bench.attempt("t2", 1).display()
        )
    );
    assert_eq!(
        bench.rows(),
        states(&[
            ("t1", "mapped"),
            ("t2", "dispatch-failed"),
            ("t3", "pending")
        ])
    );
    bench.script().failing.clear();
    assert_eq!(bench.run(), Ok(1));
    assert_eq!(bench.dispatched(), ["t1", "t2"]);
    assert_eq!(bench.fake.calls("map").len(), 1);
}

#[test]
fn a_reservation_without_a_receipt_stays_unsettled() {
    let bench = Bench::new(&["t1", "t2"], CLAUDE);
    {
        let mut script = bench.script();
        script.receiptless.insert("t1".into());
        script.failing.insert("t1".into(), 137);
    }
    assert_eq!(bench.run(), Ok(137));
    assert_eq!(
        bench.rows(),
        states(&[("t1", "unsettled"), ("t2", "pending")])
    );
    assert_eq!(
        dumps(field(&bench.status(), "outstandingReservations").unwrap()),
        "1"
    );
    assert_eq!(bench.run(), Ok(1));
    assert_eq!(bench.dispatched(), ["t1"]);
}

#[test]
fn a_restart_settles_a_dispatched_attempt_once_and_continues() {
    let bench = Bench::new(&["t1", "t2", "t3"], CLAUDE);
    bench.script().lost.insert("t1".into());
    assert_eq!(bench.run(), Ok(2));
    assert_eq!(state(&bench), "failed");
    assert_eq!(
        bench.rows(),
        states(&[("t1", "dispatching"), ("t2", "pending"), ("t3", "pending")])
    );
    bench.script().lost.clear();
    assert_eq!(bench.run(), Ok(0));
    assert_eq!(
        bench.rows(),
        states(&[("t1", "mapped"), ("t2", "mapped"), ("t3", "mapped")])
    );
    assert_eq!(bench.dispatched(), ["t1", "t2", "t3"]);
    assert_eq!(bench.fake.calls("map").len(), 3);
    assert_eq!(bench.fake.calls("prepare").len(), 3);
}

#[test]
fn a_command_blocked_check_is_archived_and_the_queue_continues_without_retrying_it() {
    let bench = Bench::new(&["t1", "t2"], CLAUDE);
    bench.script().blocked.insert("t1".into());
    assert_eq!(bench.run(), Ok(5));
    assert_eq!(state(&bench), "blocked-check");
    assert_eq!(
        bench.rows(),
        states(&[("t1", "blocked-check"), ("t2", "mapped")])
    );
    assert_eq!(bench.dispatched(), ["t1", "t2"]);
    let status = bench.status();
    assert_eq!(
        Usd::of(field(&status, "spentUpperUsd").unwrap()).unwrap(),
        usd("2")
    );
    assert_eq!(
        dumps(field(&status, "outstandingReservations").unwrap()),
        "0"
    );
    let blocked = &list(&status, "batches").unwrap()[0];
    let refusals: Vec<String> = list(blocked, "blockedChecks")
        .unwrap()
        .iter()
        .map(Value::py_str)
        .collect();
    assert_eq!(
        refusals.join("\n") + "\n",
        std::fs::read_to_string(bench.attempt("t1", 1).join("map.log")).unwrap()
    );
    assert!(refusals[0].contains("the run tool refused go run ./cmd/import-boss"));
    assert_eq!(text(blocked, "sessionId").unwrap(), "session-t1-attempt-1");
    assert_eq!(text(blocked, "contextId").unwrap(), "context-t1-attempt-1");
    assert_eq!(
        Usd::of(field(blocked, "costUpperUsd").unwrap()).unwrap(),
        usd("1")
    );
    let receipt = checked(&bench.root, field(blocked, "evidence").unwrap()).unwrap();
    let evidence = read_json(&receipt).unwrap();
    let archive = checked(&bench.root, field(&evidence, "archive").unwrap()).unwrap();
    let archived = std::fs::read(&archive).unwrap();
    let files = list(&evidence, "files").unwrap();
    for name in [
        "key.json",
        "reservation.json",
        "prepare.log",
        "dispatch.log",
        "map.log",
        "work/verdicts.json",
        "work/dispatch.json",
        "work/clean-context.json",
        "work/home/.claude/projects/fixture/session.jsonl",
    ] {
        let file = files
            .iter()
            .find(|file| text(file, "path").unwrap() == name)
            .unwrap();
        assert_eq!(
            text(file, "sha256").unwrap(),
            digest(&bench.attempt("t1", 1).join(name)).unwrap()
        );
    }

    assert_eq!(bench.run(), Ok(5));
    assert_eq!(bench.dispatched(), ["t1", "t2"]);
    assert_eq!(bench.fake.calls("map").len(), 2);
    assert_eq!(std::fs::read(&archive).unwrap(), archived);

    std::fs::create_dir_all(bench.attempt("t1", 2)).unwrap();
    bench.script().blocked.clear();
    assert_eq!(bench.run(), Ok(0));
    assert_eq!(bench.rows(), states(&[("t1", "mapped"), ("t2", "mapped")]));
    assert_eq!(bench.dispatched(), ["t1", "t2", "t1"]);
    assert_eq!(std::fs::read(&archive).unwrap(), archived);
    let status = bench.status();
    assert_eq!(
        Usd::of(field(&status, "spentUpperUsd").unwrap()).unwrap(),
        usd("3")
    );
    let retried = &list(&status, "batches").unwrap()[0];
    assert_eq!(
        text(retried, "workspace").unwrap(),
        "queue/q/batches/r1/t1/attempt-2"
    );
    assert!(list(retried, "blockedChecks").unwrap().is_empty());
}

#[test]
fn a_mapping_failure_after_a_blocked_check_still_stops_new_launches() {
    let bench = Bench::new(&["t1", "t2", "t3"], CLAUDE);
    bench.script().blocked.insert("t1".into());
    assert_eq!(bench.run_with(Some(1), 1, None), Ok(5));
    bench.script().map_code = 1;
    assert_eq!(bench.run(), Ok(1));
    assert_eq!(state(&bench), "failed");
    assert_eq!(
        bench.rows(),
        states(&[
            ("t1", "blocked-check"),
            ("t2", "mapping-failed"),
            ("t3", "pending")
        ])
    );
    assert_eq!(bench.dispatched(), ["t1", "t2"]);
}

#[test]
fn a_mapped_row_missing_its_evidence_is_archived_on_resume() {
    let bench = Bench::new(&["t1"], CLAUDE);
    assert_eq!(bench.run(), Ok(0));
    let mut status = bench.status();
    let Value::Object(document) = &mut status else {
        unreachable!()
    };
    let Some(Value::Array(rows)) = document.get_mut("batches") else {
        unreachable!()
    };
    let Value::Object(row) = &mut rows[0] else {
        unreachable!()
    };
    let evidence = dumps(row.get("evidence").unwrap());
    *row = row
        .iter()
        .filter(|(name, _)| *name != "evidence")
        .map(|(name, value)| (name.clone(), value.clone()))
        .collect();
    write(&bench.directory.join("status.json"), &dumps(&status));
    assert_eq!(bench.run(), Ok(0));
    assert_eq!(
        dumps(field(&list(&bench.status(), "batches").unwrap()[0], "evidence").unwrap()),
        evidence
    );
    assert_eq!(bench.dispatched(), ["t1"]);
}

#[test]
fn the_limit_stops_after_that_many_mapped_batches() {
    let bench = Bench::new(&["t1", "t2", "t3"], CLAUDE);
    assert_eq!(bench.run_with(Some(1), 1, None), Ok(0));
    assert_eq!(state(&bench), "limited");
    assert_eq!(reason(&bench), "Requested batch limit reached");
    assert_eq!(
        bench.rows(),
        states(&[("t1", "mapped"), ("t2", "pending"), ("t3", "pending")])
    );
    assert_eq!(bench.run(), Ok(0));
    assert_eq!(bench.dispatched(), ["t1", "t2", "t3"]);
}

#[test]
fn workers_dispatch_together_and_each_batch_once() {
    let mut bench = Bench::new(&["t1", "t2", "t3", "t4"], CLAUDE);
    bench.fake.together = 2;
    assert_eq!(bench.run_with(None, 2, None), Ok(0));
    let status = bench.status();
    let invocation = &list(&status, "invocations").unwrap()[0];
    assert_eq!(dumps(field(invocation, "peakActive").unwrap()), "2");
    assert_eq!(dumps(field(invocation, "workers").unwrap()), "2");
    let mut dispatched = bench.dispatched();
    dispatched.sort();
    assert_eq!(dispatched, ["t1", "t2", "t3", "t4"]);
    assert_eq!(state(&bench), "mapped");
}

#[test]
fn a_session_shared_by_two_batches_stops_the_queue() {
    let bench = Bench::new(&["t1", "t2"], CLAUDE);
    bench.script().shared_session = true;
    assert_eq!(bench.run(), Ok(2));
    assert_eq!(
        reason(&bench),
        format!(
            "{RUN}/t2: sessionId shared is not unique to {}",
            bench.attempt("t2", 1).display()
        )
    );
    assert_eq!(bench.fake.calls("map").len(), 1);
}

#[test]
fn failing_steps_stop_the_queue() {
    let bench = Bench::new(&["t1"], CLAUDE);
    bench.script().preflight_code = 5;
    assert_eq!(bench.run(), Ok(5));
    assert_eq!(reason(&bench), format!("Queue preflight failed for {RUN}"));
    assert_eq!(bench.fake.calls("prepare"), [] as [Vec<String>; 0]);
    let bench = Bench::new(&["t1"], CLAUDE);
    bench.script().map_code = 1;
    assert_eq!(bench.run(), Ok(1));
    assert_eq!(reason(&bench), format!("Mapping failed for {RUN}/t1"));
    assert_eq!(bench.rows(), states(&[("t1", "mapping-failed")]));
    bench.script().map_code = 0;
    assert_eq!(bench.run(), Ok(0));
    assert!(bench.attempt("t1", 1).join("map.retry-1.log").is_file());
}

#[test]
fn the_queue_keeps_the_client_it_pinned_first() {
    let bench = Bench::new(&["t1", "t2"], CLAUDE);
    assert_eq!(bench.run_with(Some(1), 1, None), Ok(0));
    let record = std::fs::read(bench.directory.join("client.json")).unwrap();
    write(&bench.root.join("bin/claude"), "an updated client");
    bench.script().version = "2.0.0";
    assert_eq!(bench.run(), Ok(0));
    assert_eq!(
        std::fs::read(bench.directory.join("client.json")).unwrap(),
        record
    );
    assert_eq!(
        std::fs::read_to_string(bench.directory.join("client-bin/claude")).unwrap(),
        "the installed client"
    );
    assert_eq!(
        bench.run_with(None, 1, Some("9.9.9")).unwrap_err(),
        "the queue's pinned claude is 1.2.3, not the requested 9.9.9"
    );
    let copy = bench.directory.join("client-bin/claude");
    std::fs::remove_file(&copy).unwrap();
    write(&copy, "tampered");
    assert!(
        bench
            .run()
            .unwrap_err()
            .contains("is missing or changed; restore it, or remove")
    );
}

#[test]
fn a_changed_authorization_is_refused() {
    let bench = Bench::new(&["t1", "t2"], CLAUDE);
    assert_eq!(bench.run_with(Some(1), 1, None), Ok(0));
    let path = bench.root.join("authorization.json");
    let changed = std::fs::read_to_string(&path)
        .unwrap()
        .replace("\"high\"", "\"high\", \"note\": 1");
    write(&path, &changed);
    let refused = bench.run().unwrap_err();
    assert!(
        refused.starts_with("reserved attempt authorization changed: "),
        "{refused}"
    );
    write(&bench.root.join("deviation.md"), "v1");
    let pinned = pin(&bench.root, "deviation.md");
    write(
        &path,
        &changed.replacen('{', &format!("{{\"runnerDeviations\": [{pinned}], "), 1),
    );
    write(&bench.root.join("deviation.md"), "v2");
    assert_eq!(
        authorize(&bench.root, &path).err().unwrap(),
        "pinned input changed: deviation.md"
    );
}

#[test]
fn an_unbounded_codex_queue_reserves_no_amount() {
    let bench = Bench::new(
        &["t1"],
        r#""budgetCapUsd": null, "budgetPolicy": "codex-unbounded", "grader": {"model": "gpt-test", "effort": "high"}"#,
    );
    assert_eq!(bench.run(), Ok(0));
    let dispatch = &bench.fake.calls("dispatch")[0];
    assert!(dispatch.contains(&"--allow-unbounded-codex".to_string()));
    assert!(!dispatch.contains(&"--max-budget-usd".to_string()));
    assert!(bench.fake.calls("preflight")[0].contains(&"--allow-unbounded-codex".to_string()));
    let reservation = read_json(&bench.attempt("t1", 1).join("reservation.json")).unwrap();
    assert!(matches!(
        field(&reservation, "maxBudgetUsd").unwrap(),
        Value::Null
    ));
}

#[test]
fn spending_policies_are_checked_before_any_step() {
    for (grants, refusal) in [
        (
            r#""budgetCapUsd": 20, "grader": {"model": "gpt-test", "effort": "high"}"#,
            "bounded Codex dispatch requires a verified dollar hard cap",
        ),
        (
            r#""budgetCapUsd": null, "grader": {"model": "claude-test", "effort": "high"}"#,
            "a null budget cap requires a Codex grader and budgetPolicy codex-unbounded",
        ),
        (
            r#""budgetCapUsd": 5, "budgetPolicy": "codex-unbounded", "grader": {"model": "gpt-test", "effort": "high"}"#,
            "budgetPolicy codex-unbounded requires a null budget cap",
        ),
        (
            r#""budgetCapUsd": 5, "quotaStopPercent": 0, "grader": {"model": "claude-test", "effort": "high"}"#,
            "quotaStopPercent must be a number above 0 and at most 100",
        ),
        (
            r#""budgetCapUsd": 5, "grader": {"model": "claude-test", "effort": "high", "timeoutSeconds": null}"#,
            "grader.timeoutSeconds must be a positive integer",
        ),
    ] {
        let bench = Bench::new(&["t1"], grants);
        assert_eq!(bench.run().unwrap_err(), refusal);
        assert!(bench.fake.calls.lock().unwrap().is_empty());
        assert!(!bench.directory.join("client.json").exists());
    }
}

#[test]
fn a_plan_that_no_longer_matches_is_refused() {
    let bench = Bench::new(&["t1", "t2"], CLAUDE);
    let fingerprints = bench.root.join("fingerprints.json");
    let saved = std::fs::read_to_string(&fingerprints).unwrap();
    write(&fingerprints, &saved.replace("fp-t2", "fp-new"));
    assert_eq!(
        bench.run().unwrap_err(),
        format!(
            "relevant inputs changed since the plan; plan and authorize the queue again: {RUN}/t2"
        )
    );
    write(
        &fingerprints,
        &saved.replace(
            r#""target": "t2", "state": "pending", "input_fingerprint": "fp-t2""#,
            r#""target": "t9", "state": "pending", "input_fingerprint": "fp-t2""#,
        ),
    );
    assert_eq!(
        bench.run().unwrap_err(),
        format!("planned batch is not selected: {RUN}/t2")
    );
    let bench = Bench::with(&["t1", "t2"], CLAUDE, |plan| {
        let Some(Value::Array(batches)) = plan.get_mut("batches") else {
            unreachable!()
        };
        batches[1] = json(&format!(
            r#"{{"run": "{RUN}", "target": "t2", "state": "current", "inputFingerprint": "fp-t2"}}"#
        ));
    });
    assert_eq!(
        bench.run().unwrap_err(),
        "execution plan order must name every batch awaiting grading exactly once"
    );
    let bench = Bench::with(&["t1"], CLAUDE, |plan| {
        plan.insert("contract".into(), string("current-reconciliation/v0"));
    });
    assert_eq!(
        bench.run().unwrap_err(),
        "authorization pins a plan of another grading contract; plan the current queue again"
    );
}

#[test]
fn a_saved_grade_gone_stale_is_refused() {
    let bench = Bench::with(&["t1", "t2"], CLAUDE, |plan| {
        let Some(Value::Array(batches)) = plan.get_mut("batches") else {
            unreachable!()
        };
        batches.push(json(&format!(
            r#"{{"run": "{RUN}", "target": "t0", "state": "current", "inputFingerprint": "fp-t0"}}"#
        )));
    });
    let fingerprints = bench.root.join("fingerprints.json");
    let saved = std::fs::read_to_string(&fingerprints).unwrap();
    let stale = format!(
        r#"{{"run": "{RUN}", "target": "t0", "state": "stale", "input_fingerprint": "fp-t0"}}"#
    );
    write(
        &fingerprints,
        &saved.replacen('[', &format!("[{stale},"), 1),
    );
    assert_eq!(
        bench.run().unwrap_err(),
        format!(
            "planned current batches need grading; plan and authorize the queue again: {RUN}/t0"
        )
    );
}

#[test]
fn a_zero_charge_proof_settles_a_reservation_only_before_the_session_starts() {
    for started in [false, true] {
        let bench = Bench::new(&["t1"], CLAUDE);
        std::fs::create_dir_all(&bench.directory).unwrap();
        let first = bench.attempt("t1", 1);
        std::fs::create_dir_all(first.join("work")).unwrap();
        if started {
            std::fs::create_dir_all(first.join("work/home")).unwrap();
        }
        let authorization = digest(&bench.root.join("authorization.json")).unwrap();
        write(
            &first.join("reservation.json"),
            &format!(r#"{{"maxBudgetUsd": 2.1, "authorizationSha256": "{authorization}"}}"#),
        );
        write(
            &first.join("budget-resolution.json"),
            r#"{"evidence": [], "chargeUpperUsd": 0}"#,
        );
        std::fs::create_dir_all(bench.attempt("t1", 2)).unwrap();
        if started {
            assert_eq!(bench.run(), Ok(2));
            assert!(
                reason(&bench).starts_with("Queue budget: invalid zero-charge proof: "),
                "{}",
                reason(&bench)
            );
            assert!(matches!(
                field(&bench.status(), "spentUpperUsd").unwrap(),
                Value::Null
            ));
        } else {
            assert_eq!(bench.run(), Ok(0));
            assert_eq!(
                dumps(field(&bench.status(), "outstandingReservations").unwrap()),
                "0"
            );
            assert_eq!(
                text(&list(&bench.status(), "batches").unwrap()[0], "workspace").unwrap(),
                "queue/q/batches/r1/t1/attempt-2"
            );
        }
    }
}

fn argument(call: &[String], name: &str) -> String {
    call[call.iter().position(|arg| arg == name).unwrap() + 1].clone()
}

#[test]
fn the_replacement_manifest_and_cache_root_reach_preflight_and_preparation() {
    let bench = Bench::new(&["t1"], CLAUDE);
    bench.replace_cache(Some(".local/replacement-cache"));
    assert_eq!(bench.run(), Ok(0));
    let calls = [
        &bench.fake.calls("preflight")[0],
        &bench.fake.calls("prepare")[0],
    ];
    for call in calls {
        assert_eq!(
            argument(call, "--cache-replacements"),
            bench.root.join(REPLACEMENTS).display().to_string()
        );
        assert_eq!(
            argument(call, "--cache-root"),
            bench
                .root
                .join(".local/replacement-cache")
                .display()
                .to_string()
        );
    }
    write(&bench.root.join(REPLACEMENTS), "changed");
    assert_eq!(
        bench.run().unwrap_err(),
        format!("pinned input changed: {REPLACEMENTS}")
    );
}

#[test]
fn a_replacement_changed_after_preflight_reserves_and_dispatches_nothing() {
    let bench = Bench::new(&["t1"], CLAUDE);
    bench.replace_cache(None);
    bench.script().rewrite_after = Some((
        "preflight",
        bench.root.join(REPLACEMENTS),
        r#"{"targets": ["changed"]}"#,
    ));
    assert_eq!(bench.run(), Ok(2));
    assert_eq!(bench.dispatched(), [] as [String; 0]);
    assert_eq!(bench.reservations(), [] as [PathBuf; 0]);
    assert!(
        reason(&bench).contains("pinned input changed"),
        "{}",
        reason(&bench)
    );
}

#[test]
fn a_prepared_key_must_pin_the_authorized_replacement_before_reserving() {
    let bench = Bench::new(&["t1"], CLAUDE);
    bench.replace_cache(None);
    bench.script().prepared_manifest =
        Some("0000000000000000000000000000000000000000000000000000000000000000");
    assert_eq!(bench.run(), Ok(2));
    assert_eq!(bench.dispatched(), [] as [String; 0]);
    assert_eq!(bench.reservations(), [] as [PathBuf; 0]);
    assert!(
        reason(&bench).contains("prepared cache replacement differs from authorization"),
        "{}",
        reason(&bench)
    );
}

#[test]
fn an_existing_key_must_pin_the_authorized_replacement_on_restart() {
    let bench = Bench::new(&["t1"], CLAUDE);
    bench.replace_cache(None);
    let attempt = bench.attempt("t1", 1);
    write(
        &attempt.join("key.json"),
        r#"{"input_fingerprint": "fp-t1"}"#,
    );
    assert_eq!(bench.run(), Ok(2));
    assert_eq!(bench.fake.calls("prepare"), [] as [Vec<String>; 0]);
    assert_eq!(bench.dispatched(), [] as [String; 0]);
    assert!(!attempt.join("reservation.json").exists());
    assert!(
        reason(&bench).contains("prepared cache replacement differs from authorization"),
        "{}",
        reason(&bench)
    );
}

#[test]
fn a_manifest_changed_during_preparation_reserves_nothing() {
    let bench = Bench::new(&["t1"], CLAUDE);
    bench.replace_cache(None);
    bench.script().rewrite_after = Some((
        "prepare",
        bench.root.join(REPLACEMENTS),
        r#"{"targets": ["changed"]}"#,
    ));
    assert_eq!(bench.run(), Ok(2));
    assert_eq!(bench.dispatched(), [] as [String; 0]);
    assert_eq!(bench.reservations(), [] as [PathBuf; 0]);
    assert!(
        reason(&bench).contains("pinned input changed"),
        "{}",
        reason(&bench)
    );
}
