//! `bench-tools evaluator-audit second-pass launch` against a fake driver, fake Claude meters and a fake clock: no
//! model call, no repository write.

use bench_tools::evaluator_audit::second_pass::launch::{self, Options};
use bench_tools::evaluator_audit::second_pass::{Config, Effects};
use bench_tools::normalize_review::utc_stamp;
use bench_tools::pyjson::{self, Duplicates, Value};
use bench_tools::regrade;
use bench_tools::subscription_meters::{Pinned, Unreadable};
use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const ROUND: &str = "evaluator-audit-v2-test-round";
const START: f64 = 1_800_000_000.0;

fn json(text: &str) -> Value {
    pyjson::loads(text, Duplicates::KeepLast).unwrap()
}

fn read(path: &Path) -> Value {
    json(&fs::read_to_string(path).unwrap())
}

fn get<'v>(value: &'v Value, key: &str) -> &'v Value {
    value.as_object().unwrap().get(key).unwrap()
}

fn number(value: &Value) -> f64 {
    value.py_str().parse().unwrap()
}

fn write(path: &Path, contents: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, contents).unwrap();
}

fn argument(args: &[String], name: &str) -> String {
    let at = args.iter().position(|arg| arg == name).unwrap();
    args[at + 1].clone()
}

struct Meters {
    five_hour: Option<f64>,
    week: f64,
    week_resets_at: f64,
    high_five_hour_reads: usize,
}

/// The fake driver, meters, clock and stored sign-in.
struct Fake {
    root: PathBuf,
    cost: f64,
    rate: f64,
    fail_target: Option<&'static str>,
    fail_text: &'static str,
    fail_always: bool,
    clock: Mutex<f64>,
    meters: Mutex<Meters>,
    /// When the stored sign-in expires, and the sleeps after which a person renews it.
    sign_in: Mutex<(f64, Option<usize>)>,
    drivers: Mutex<Vec<Vec<String>>>,
}

impl Fake {
    fn new(root: &Path, week: f64) -> Fake {
        Fake {
            root: root.to_path_buf(),
            cost: 1.0,
            rate: 0.05,
            fail_target: None,
            fail_text: "",
            fail_always: false,
            clock: Mutex::new(START),
            meters: Mutex::new(Meters {
                five_hour: Some(3.0),
                week,
                week_resets_at: START + 7.0 * 86_400.0,
                high_five_hour_reads: 0,
            }),
            sign_in: Mutex::new((START + 8.0 * 3_600.0, None)),
            drivers: Mutex::new(Vec::new()),
        }
    }

    fn time(&self) -> f64 {
        *self.clock.lock().unwrap()
    }

    fn saved(&self, record: &Path, target: &str) -> bool {
        record
            .join("second/r1")
            .join(target)
            .join("assessment-1/receipt.json")
            .exists()
    }

    /// The driver of launch.py's test: it launches up to `--limit` batches, each costing `cost` and moving the week
    /// `cost * rate` points, and fails `fail_target`'s first attempt with `fail_text` in its output.
    fn drive(&self, args: &[String]) -> i32 {
        let authorization = read(&self.root.join(argument(args, "--authorization")));
        let limit: usize = argument(args, "--limit").parse().unwrap();
        let plan = read(
            &self
                .root
                .join(get(get(&authorization, "plan"), "path").py_str()),
        );
        let queue = self.root.join(get(&plan, "queueDirectory").py_str());
        let record = self.root.join(get(&plan, "recordRoot").py_str());
        fs::create_dir_all(&queue).unwrap();
        let status_path = queue.join("status.json");
        let mut spent = if status_path.exists() {
            number(get(&read(&status_path), "spentUpperUsd"))
        } else {
            0.0
        };
        let order: Vec<String> = get(&plan, "order")
            .as_array()
            .unwrap()
            .iter()
            .map(|batch| get(batch, "target").py_str())
            .collect();
        let finish = |state: &str, code: i32, reason: Option<String>, spent: f64| {
            let saved = order
                .iter()
                .filter(|target| self.saved(&record, target))
                .count();
            write(
                &status_path,
                &format!(
                    r#"{{"spentUpperUsd": {spent:?}, "state": "{state}", "reason": {}, "saved": {saved},
                        "planned": {}, "budgetCapUsd": {}}}"#,
                    reason.map_or("null".into(), |reason| format!("\"{reason}\"")),
                    order.len(),
                    get(&authorization, "budgetCapUsd").py_str()
                ),
            );
            code
        };
        let mut launched = 0;
        for target in &order {
            if self.saved(&record, target) {
                continue;
            }
            let folder = queue.join("attempts/r1").join(target);
            let mut attempts: Vec<PathBuf> = fs::read_dir(&folder)
                .map(|entries| entries.map(|entry| entry.unwrap().path()).collect())
                .unwrap_or_default();
            attempts.sort();
            if attempts
                .last()
                .is_some_and(|last| !last.join("disposition.json").exists())
            {
                return finish("failed", 1, Some(format!("{target}: unresolved")), spent);
            }
            if launched >= limit {
                break;
            }
            let attempt = folder.join(format!("attempt-{}", attempts.len() + 1));
            fs::create_dir_all(attempt.join("work")).unwrap();
            write(
                &attempt.join("reservation.json"),
                &format!(r#"{{"at": "{}"}}"#, utc_stamp(self.time() as u64)),
            );
            launched += 1;
            self.meters.lock().unwrap().week += self.cost * self.rate;
            spent += self.cost;
            if self.fail_target == Some(target.as_str())
                && (attempts.is_empty() || self.fail_always)
            {
                write(&attempt.join("work/dispatch.json"), "{}");
                write(&attempt.join("work/stdout.txt"), self.fail_text);
                return finish(
                    "failed",
                    1,
                    Some(format!("{target}: receipt-needs-investigation")),
                    spent,
                );
            }
            write(
                &record
                    .join("second/r1")
                    .join(target)
                    .join("assessment-1/receipt.json"),
                "{}",
            );
        }
        let left = order.iter().any(|target| !self.saved(&record, target));
        finish(
            if left { "limit-reached" } else { "complete" },
            0,
            None,
            spent,
        )
    }
}

impl regrade::Effects for Fake {
    fn step(&self, _: &[OsString], _: &Path, _: &OsStr) -> std::io::Result<i32> {
        unreachable!()
    }

    fn fingerprints(&self) -> Result<Value, String> {
        unreachable!()
    }

    fn meters(&self, clients: &[Pinned], _: &OsStr) -> Result<Value, Unreadable> {
        assert_eq!(clients[0].name, "claude");
        let now = self.time();
        let mut meters = self.meters.lock().unwrap();
        let mut five = meters.five_hour;
        if meters.high_five_hour_reads > 0 {
            meters.high_five_hour_reads -= 1;
            five = Some(95.0);
        }
        let five = five.map_or("null".into(), |five| {
            format!(
                r#"{{"utilization": {five:?}, "resets_at": "{}"}}"#,
                utc_stamp(now as u64 + 1)
            )
        });
        Ok(json(&format!(
            r#"{{"claude": {{"five_hour": {five}, "seven_day": {{"utilization": {:?},
                "resets_at": "{}"}}}}}}"#,
            meters.week.floor(),
            utc_stamp(meters.week_resets_at as u64)
        )))
    }

    fn installed_client(&self, _: &str) -> Result<PathBuf, String> {
        Ok(self.root.join("bin/claude"))
    }

    fn client_version(&self, _: &Path) -> Result<String, String> {
        unreachable!()
    }

    fn now(&self) -> f64 {
        self.time()
    }

    fn elapsed(&self) -> f64 {
        self.time() - START
    }
}

impl Effects for Fake {
    fn tool(&self, _: &[OsString], _: &Path, _: &OsStr) -> std::io::Result<i32> {
        unreachable!("the launcher runs no grading step itself")
    }

    fn driver(&self, args: &[OsString], log: &Path) -> std::io::Result<i32> {
        assert!(log.ends_with(".local/state/driver.log"));
        let args: Vec<String> = args
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        self.drivers.lock().unwrap().push(args.clone());
        Ok(self.drive(&args))
    }

    fn runtime_pin(&self) -> Result<Value, String> {
        unreachable!()
    }

    fn sleep(&self, seconds: f64) {
        *self.clock.lock().unwrap() += seconds;
        let mut sign_in = self.sign_in.lock().unwrap();
        if let Some(sleeps) = &mut sign_in.1 {
            *sleeps = sleeps.saturating_sub(1);
            if *sleeps == 0 {
                *sign_in = (self.time() + 8.0 * 3_600.0, None);
            }
        }
    }

    fn sign_in_expires(&self) -> Option<f64> {
        Some(self.sign_in.lock().unwrap().0)
    }

    fn booted_at(&self) -> f64 {
        0.0
    }
}

struct Bench {
    root: PathBuf,
}

impl Drop for Bench {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn bench(label: &str, batches: usize, cap: f64) -> Bench {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("second-pass-launch-{label}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    let root = fs::canonicalize(&root).unwrap();
    write(&root.join("bench/grading/current/audits.json"), "{}");
    let order: Vec<String> = (0..batches)
        .map(|index| {
            format!(r#"{{"run": "runs/r1", "target": "t{index:03}", "firstGradeUsd": 1.0}}"#)
        })
        .collect();
    write(
        &root.join("plan.json"),
        &format!(
            r#"{{"recordRoot": "bench/grading/current/audit/{ROUND}", "queueDirectory": ".local/state/queue",
                "order": [{}]}}"#,
            order.join(", ")
        ),
    );
    write(
        &root.join("auth.json"),
        &format!(
            r#"{{"plan": {{"path": "plan.json"}}, "budgetCapUsd": {cap:?}, "quotaStopPercent": 90,
                "grader": {{"cliVersion": "2.1.314"}}}}"#
        ),
    );
    write(
        &root.join("config.json"),
        &format!(
            r#"{{"contract": "evaluator-audit-second-pass-config/v1", "round": "{ROUND}", "plan": "plan.json",
  "regrades": [], "stateDirectory": ".local/state", "archiveRoot": "archive", "cacheRoot": ".local/cache",
  "plainQueue": "plain", "cacheReplacementQueues": {{}},
  "grader": {{"client": "claude", "model": "claude-sonnet-5-5"}}, "firstGrader": "claude-opus-5-5",
  "budgetPolicy": "claude-list-price-cap",
  "launch": {{"trialBatches": 5, "checkpointBatches": 5, "chunkBatches": 10, "workers": 3, "weekPointsLimit": 30,
    "resetMarginSeconds": 120, "signInMarginSeconds": 2400, "maxRetriesPerBatch": 1, "maxRetries": 8,
    "meterFailuresBeforeStop": 6}},
  "authorization": {{"approval": null}}}}"#
        ),
    );
    Bench { root }
}

impl Bench {
    fn launch(&self, fake: &Fake, not_before: Option<f64>) -> (u8, Value) {
        let options = Options {
            root: self.root.clone(),
            config: Config::load(&self.root, &self.root.join("config.json")).unwrap(),
            authorization: "auth.json".into(),
            not_before,
        };
        let code = launch::launch(fake, &options).unwrap();
        let path = self.root.join(".local/state/launcher.json");
        let state = if path.exists() {
            read(&path)
        } else {
            json("{}")
        };
        (code, state)
    }

    fn saved(&self) -> usize {
        fs::read_dir(
            self.root
                .join("bench/grading/current/audit")
                .join(ROUND)
                .join("second/r1"),
        )
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

    fn log(&self) -> String {
        fs::read_to_string(self.root.join(".local/state/launcher.log")).unwrap_or_default()
    }
}

fn phase(state: &Value) -> String {
    get(state, "phase").py_str()
}

fn stopped(state: &Value) -> String {
    get(get(state, "stopped"), "reason").py_str()
}

fn decisions(state: &Value) -> Vec<Value> {
    get(state, "decisions").as_array().unwrap().to_vec()
}

#[test]
fn a_low_rate_runs_the_full_pass_after_the_trial() {
    let bench = bench("low-rate", 40, 275.0);
    let fake = Fake::new(&bench.root, 2.0);
    let (code, state) = bench.launch(&fake, None);
    assert_eq!(
        (code, phase(&state).as_str()),
        (0, "finished"),
        "{}",
        bench.log()
    );
    assert_eq!(bench.saved(), 40);
    let decisions = decisions(&state);
    assert_eq!(decisions.len(), 1);
    assert_eq!(number(get(&decisions[0], "batches")), 5.0);
    assert_eq!(number(get(&decisions[0], "projectedCostUsd")), 40.0);
    let drivers = fake.drivers.lock().unwrap().clone();
    let first: Vec<&str> = drivers[0].iter().map(String::as_str).collect();
    assert_eq!(first[..3], ["evaluator-audit", "second-pass", "run"]);
    assert_eq!(argument(&drivers[0], "--authorization"), "auth.json");
    assert_eq!(argument(&drivers[0], "--config"), "config.json");
    assert_eq!(argument(&drivers[0], "--workers"), "3");
    assert_eq!(argument(&drivers[0], "--limit"), "5");
    assert_eq!(argument(&drivers[1], "--limit"), "10");
}

#[test]
fn an_unused_five_hour_window_reads_as_zero() {
    let bench = bench("unused-window", 10, 275.0);
    let fake = Fake::new(&bench.root, 2.0);
    fake.meters.lock().unwrap().five_hour = None;
    let (code, state) = bench.launch(&fake, None);
    assert_eq!(
        (code, phase(&state).as_str()),
        (0, "finished"),
        "{}",
        bench.log()
    );
    assert_eq!(bench.saved(), 10);
}

#[test]
fn a_high_rate_stops_after_the_trial_and_later_starts_do_nothing() {
    let bench = bench("high-rate", 40, 275.0);
    let mut fake = Fake::new(&bench.root, 2.0);
    fake.rate = 2.0;
    let (code, state) = bench.launch(&fake, None);
    assert_eq!((code, phase(&state).as_str()), (0, "stopped"));
    assert_eq!(bench.saved(), 5);
    assert!(
        stopped(&state).contains("lowest rate"),
        "{}",
        stopped(&state)
    );
    let drivers = fake.drivers.lock().unwrap().len();
    let (code, again) = bench.launch(&fake, None);
    assert_eq!(code, 0);
    assert_eq!(bench.saved(), 5);
    assert_eq!(fake.drivers.lock().unwrap().len(), drivers);
    assert!(again.py_eq(&state));
}

#[test]
fn an_unsettled_trial_measures_more_batches() {
    let bench = bench("unsettled", 170, 275.0);
    let mut fake = Fake::new(&bench.root, 2.0);
    fake.rate = 0.1;
    let (code, state) = bench.launch(&fake, None);
    assert_eq!(
        (code, phase(&state).as_str()),
        (0, "finished"),
        "{}",
        bench.log()
    );
    assert_eq!(bench.saved(), 170);
    let decisions = decisions(&state);
    let batches: Vec<f64> = decisions
        .iter()
        .map(|decision| number(get(decision, "batches")))
        .collect();
    assert_eq!(batches[0], 5.0);
    assert!(batches.len() >= 2 && batches.is_sorted(), "{batches:?}");
    let high = |decision: &Value| number(get(decision, "projectedPointsHigh"));
    assert!(high(decisions.last().unwrap()) < 30.0 && 30.0 <= high(&decisions[0]));
}

#[test]
fn a_projected_cost_over_the_cap_stops_the_pass() {
    let bench = bench("over-cap", 40, 30.0);
    let fake = Fake::new(&bench.root, 2.0);
    let (code, state) = bench.launch(&fake, None);
    assert_eq!((code, phase(&state).as_str()), (0, "stopped"));
    assert!(stopped(&state).contains("cap"), "{}", stopped(&state));
    assert_eq!(bench.saved(), 5);
}

#[test]
fn a_full_five_hour_window_is_waited_out() {
    let bench = bench("five-hour", 10, 275.0);
    let fake = Fake::new(&bench.root, 2.0);
    fake.meters.lock().unwrap().high_five_hour_reads = 1;
    let (code, state) = bench.launch(&fake, None);
    assert_eq!((code, phase(&state).as_str()), (0, "finished"));
    assert!(
        bench.log().contains("five-hour window is at 95%"),
        "{}",
        bench.log()
    );
}

#[test]
fn a_failure_known_to_pass_is_retried_once() {
    let bench = bench("retry", 10, 275.0);
    let mut fake = Fake::new(&bench.root, 2.0);
    fake.fail_target = Some("t002");
    fake.fail_text = "API Error: 529 Overloaded";
    let (code, state) = bench.launch(&fake, None);
    assert_eq!(
        (code, phase(&state).as_str()),
        (0, "finished"),
        "{}",
        bench.log()
    );
    assert_eq!(bench.saved(), 10);
    let disposition = read(
        &bench
            .root
            .join(".local/state/queue/attempts/r1/t002/attempt-1/disposition.json"),
    );
    assert_eq!(get(&disposition, "kind").py_str(), "overloaded");
    assert_eq!(get(&disposition, "decision").py_str(), "retry");
    assert!(get(&state, "retries").py_eq(&json(r#"{"runs/r1/t002": 1}"#)));
}

#[test]
fn a_batch_that_fails_again_after_its_retry_stops_the_pass() {
    let bench = bench("retry-spent", 10, 275.0);
    let mut fake = Fake::new(&bench.root, 2.0);
    fake.fail_target = Some("t002");
    fake.fail_text = "Connection error";
    fake.fail_always = true;
    let (code, state) = bench.launch(&fake, None);
    assert_eq!((code, phase(&state).as_str()), (0, "stopped"));
    assert_eq!(
        stopped(&state),
        "runs/r1/t002 attempt-2 failed (server-error) and the retries are spent."
    );
    let attempts = bench.root.join(".local/state/queue/attempts/r1/t002");
    assert!(attempts.join("attempt-1/disposition.json").exists());
    assert!(!attempts.join("attempt-2/disposition.json").exists());
    assert!(!attempts.join("attempt-3").exists());
}

#[test]
fn an_unknown_failure_stops_the_pass_without_a_retry() {
    let bench = bench("unknown", 10, 275.0);
    let mut fake = Fake::new(&bench.root, 2.0);
    fake.fail_target = Some("t002");
    fake.fail_text = "the verdicts did not validate";
    let (code, state) = bench.launch(&fake, None);
    assert_eq!((code, phase(&state).as_str()), (0, "stopped"));
    assert!(
        stopped(&state).contains("not known to pass"),
        "{}",
        stopped(&state)
    );
    assert!(
        !bench
            .root
            .join(".local/state/queue/attempts/r1/t002/attempt-1/disposition.json")
            .exists()
    );
}

#[test]
fn the_weekly_quota_stops_the_pass_before_any_batch() {
    let bench = bench("quota", 10, 275.0);
    let fake = Fake::new(&bench.root, 90.0);
    let (code, state) = bench.launch(&fake, None);
    assert_eq!((code, phase(&state).as_str()), (0, "stopped"));
    assert!(stopped(&state).contains("90%"), "{}", stopped(&state));
    assert_eq!(bench.saved(), 0);
}

#[test]
fn a_start_before_its_time_does_nothing() {
    let bench = bench("not-before", 10, 275.0);
    let fake = Fake::new(&bench.root, 2.0);
    let (code, state) = bench.launch(&fake, launch::timestamp("2099-01-01T00:00:00Z"));
    assert_eq!(code, 0);
    assert_eq!(bench.saved(), 0);
    assert!(state.py_eq(&json("{}")));
}

#[test]
fn an_expiring_sign_in_is_waited_for_until_a_person_renews_it() {
    let bench = bench("sign-in", 6, 275.0);
    let fake = Fake::new(&bench.root, 2.0);
    *fake.sign_in.lock().unwrap() = (START + 600.0, Some(1));
    let (code, state) = bench.launch(&fake, None);
    assert_eq!((code, phase(&state).as_str()), (0, "finished"));
    let log = bench.log();
    assert!(
        log.contains("Only a person renews it") && log.contains("was renewed"),
        "{log}"
    );
}

#[test]
fn eastern_time_follows_daylight_saving() {
    let at = |text: &str| launch::eastern(launch::timestamp(text).unwrap());
    assert_eq!(at("2026-11-12T18:40:00Z"), "Thu Nov 12, 1:40 PM ET");
    assert_eq!(at("2026-07-04T04:05:00+00:00"), "Sat Jul 4, 12:05 AM ET");
    assert_eq!(at("2026-03-08T06:59:00Z"), "Sun Mar 8, 1:59 AM ET");
    assert_eq!(at("2026-03-08T07:00:00Z"), "Sun Mar 8, 3:00 AM ET");
    assert_eq!(at("2026-11-01T05:59:00Z"), "Sun Nov 1, 1:59 AM ET");
    assert_eq!(at("2026-11-01T06:00:00Z"), "Sun Nov 1, 1:00 AM ET");
}

#[test]
fn general_format_writes_numbers_as_python_does() {
    for (value, written) in [
        (2.0, "2"),
        (2.5, "2.5"),
        (30.0, "30"),
        (0.1, "0.1"),
        (89.99999999, "90"),
        (1_234_567.0, "1.23457e+06"),
        (0.00001, "1e-05"),
        (-3.25, "-3.25"),
    ] {
        assert_eq!(launch::general(value), written, "{value}");
    }
}
