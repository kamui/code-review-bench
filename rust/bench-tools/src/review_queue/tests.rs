use super::*;
use crate::skill_runner::Ran as ToolRan;
use std::cell::{Cell, RefCell};
use std::os::unix::fs::PermissionsExt;
use std::time::Duration;

const RUNNER: &str = include_str!("fake-run-cell.sh");
const START: f64 = 1_800_000_000.0;
const RUN_A: &str = "run-a";
const RUN_B: &str = "run-b";

fn json(text: &str) -> Value {
    pyjson::loads(text, Duplicates::KeepLast).expect("fixture JSON")
}

fn save(path: &Path, value: &Value) {
    fs::create_dir_all(path.parent().expect("a parent")).expect("mkdir");
    fs::write(path, pyjson::dumps(value, &Dumps::default())).expect("write");
}

fn git(repo: &Path, args: &[&str]) -> String {
    let done = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args([
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "core.hooksPath=/dev/null",
        ])
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .expect("git");
    assert!(
        done.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&done.stderr)
    );
    String::from_utf8_lossy(&done.stdout).trim().to_string()
}

fn uid() -> u32 {
    nix::unistd::getuid().as_raw()
}

fn no_wait(_: f64) {}

fn short_wait(_: f64) {
    std::thread::sleep(Duration::from_millis(5));
}

/// A checkout with two frozen runs on one client, a queue over them, a fake runner, and fake meters and clock.
struct Fixture {
    base: PathBuf,
    proc_root: PathBuf,
    queue: PathBuf,
    state: PathBuf,
    real_proc: bool,
    clock: Cell<f64>,
    uid: Cell<u32>,
    meters: RefCell<Result<Value, (String, bool)>>,
    cleanup_refusal: RefCell<Option<String>>,
    runner: RefCell<Option<Vec<String>>>,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base);
    }
}

impl Fixture {
    fn new(real_proc: bool) -> Self {
        let parent = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/queue-tests");
        let base = parent.join(evidence::random_name("q-"));
        fs::create_dir_all(&base).expect("mkdir");
        let base = base.canonicalize().expect("canonical");
        fs::create_dir_all(base.join("bench/tools")).expect("mkdir");
        fs::write(base.join("bench/tools/runner.txt"), "v1\n").expect("write");
        git(&base, &["init", "-q"]);
        git(&base, &["add", "-A"]);
        git(&base, &["commit", "-qm", "freeze"]);
        for arm in ["arm-a", "arm-b", "arm-c"] {
            save(
                &base.join(format!("bench/arms/{arm}.json")),
                &json(&format!(
                    r#"{{"id": "{arm}", "kind": "claude-builtin", "budget_usd_per_attempt": 5.0}}"#
                )),
            );
        }
        save(
            &base.join("bench/targets/t1/target.json"),
            &json(r#"{"id": "t1"}"#),
        );
        let client = base.join("clients/claude");
        fs::create_dir_all(client.parent().expect("a parent")).expect("mkdir");
        fs::write(&client, "fake claude\n").expect("write");
        let script = base.join("fake/run-cell.sh");
        fs::create_dir_all(script.parent().expect("a parent")).expect("mkdir");
        fs::write(&script, RUNNER).expect("write");
        let proc_root = if real_proc {
            PathBuf::from("/proc")
        } else {
            let root = base.join("proc");
            fs::create_dir_all(root.join("self/ns")).expect("mkdir");
            fs::create_dir_all(root.join("sys/kernel/random")).expect("mkdir");
            std::os::unix::fs::symlink("pid:[4026531836]", root.join("self/ns/pid"))
                .expect("symlink");
            fs::write(root.join("sys/kernel/random/boot_id"), "boot-1\n").expect("write");
            fs::write(root.join("uptime"), "1000.00 0.00\n").expect("write");
            root
        };
        let fixture = Self {
            queue: base.join("queue.json"),
            state: base.join("state"),
            proc_root,
            real_proc,
            clock: Cell::new(START),
            uid: Cell::new(uid()),
            meters: RefCell::new(Ok(json(
                r#"{"at": "2027-01-15T08:00:00Z", "claude": {"five_hour": {"utilization": 10}}}"#,
            ))),
            cleanup_refusal: RefCell::new(None),
            runner: RefCell::new(Some(vec!["/bin/sh".to_string(), display(&script)])),
            base,
        };
        let head = fixture.head();
        fixture.freeze(
            RUN_A,
            &["arm-a", "arm-b"],
            &["t1/arm-a/1", "t1/arm-b/1"],
            &head,
        );
        fixture.freeze(RUN_B, &["arm-c"], &["t1/arm-c/1", "t1/arm-c/2"], &head);
        fixture.write_queue(&fixture.spec());
        fixture
    }

    fn head(&self) -> String {
        git(&self.base, &["rev-parse", "HEAD"])
    }

    fn run_dir(&self, run: &str) -> PathBuf {
        self.base.join("bench/runs").join(run)
    }

    fn work(&self, run: &str) -> PathBuf {
        self.base.join("work").join(run)
    }

    fn freeze(&self, run: &str, arms: &[&str], cells: &[&str], commit: &str) {
        let arms: Vec<String> = arms
            .iter()
            .map(|arm| format!(r#"{{"id": "{arm}"}}"#))
            .collect();
        let planned: Vec<String> = cells
            .iter()
            .map(|key| {
                let parts: Vec<&str> = key.split('/').collect();
                format!(
                    r#"{{"target": "{}", "arm": "{}", "replicate": {}}}"#,
                    parts[0], parts[1], parts[2]
                )
            })
            .collect();
        let order: Vec<String> = cells.iter().map(|key| format!("\"{key}\"")).collect();
        save(
            &self.run_dir(run).join("manifest.json"),
            &json(&format!(
                r#"{{"run_id": "{run}", "frozen_at": "2027-01-01T00:00:00Z", "freeze_commit": "{commit}",
                "arms": [{}], "planned_cells": [{}], "sealed_order": [{}],
                "caps": {{"spend_usd": 100.0, "closeout_reserve_usd": 0.0, "replacements": 2}}}}"#,
                arms.join(", "),
                planned.join(", "),
                order.join(", ")
            )),
        );
    }

    fn spec(&self) -> Value {
        let client = self.base.join("clients/claude");
        json(&format!(
            r#"{{"queue_id": "q-test", "runs": ["{{repo}}/bench/runs/{RUN_A}", "{{repo}}/bench/runs/{RUN_B}"],
            "clients": {{"claude": {{"path": "{}", "sha256": "{}", "version": "9.9.9"}}}},
            "ceilings_usd": {{"claude": 900}},
            "environment": {{"QUEUE_REPO": "{{repo}}/x"}},
            "work_root": "{{repo}}/work", "state_dir": "{{repo}}/state"}}"#,
            display(&client),
            sha256_file(&client).expect("hash"),
        ))
    }

    fn write_queue(&self, spec: &Value) {
        save(&self.queue, spec);
    }

    fn edit_queue(&self, key: &str, value: Value) {
        let mut spec = self.spec();
        put(&mut spec, key, value);
        self.write_queue(&spec);
    }

    fn env(&self) -> Vec<(String, String)> {
        vec![
            ("PATH".into(), std::env::var("PATH").unwrap_or_default()),
            ("HOME".into(), display(&self.base.join("home"))),
            ("FAKE_ROOT".into(), display(&self.base)),
            ("CLAUDE_CONFIG_DIR".into(), "/inherited".into()),
            ("ANTHROPIC_API_KEY".into(), "inherited".into()),
            ("BENCH_RUST_STALE".into(), "/inherited".into()),
            ("AI_AGENT".into(), "inherited".into()),
        ]
    }

    fn cleanup(&self, argv: &[String]) -> Result<ToolRan, String> {
        self.append("tools.log", &argv.join(" "));
        if let Some(why) = self.cleanup_refusal.borrow().clone() {
            return Ok(ToolRan {
                code: 1,
                stdout: format!("cleanup refused: {why}\n"),
                stderr: String::new(),
            });
        }
        let at = argv
            .iter()
            .position(|arg| arg == "--workspace")
            .expect("a workspace");
        let workspace = PathBuf::from(&argv[at + 1]);
        let _ = fs::remove_file(workspace.join("clone-cache"));
        let _ = fs::remove_dir_all(workspace.join("clone"));
        fs::write(
            workspace.join("workspace-pruned.json"),
            r#"{"applied": true}"#,
        )
        .expect("write");
        Ok(ToolRan {
            code: 0,
            stdout: String::new(),
            stderr: String::new(),
        })
    }

    fn with<T>(&self, sleep: &dyn Fn(f64), body: impl FnOnce(&Effects<'_>) -> T) -> T {
        let clock = || self.clock.get();
        let nap = |seconds: f64| {
            if self.real_proc {
                std::thread::sleep(Duration::from_secs_f64(seconds));
            }
        };
        let meters = |_: &[Pinned], _: &[(String, String)]| match &*self.meters.borrow() {
            Ok(usage) => Ok(usage.clone()),
            Err((message, sign_in)) => Err(Unreadable {
                message: message.clone(),
                sign_in: *sign_in,
            }),
        };
        let tool = |argv: &[String], _: Option<&[(String, String)]>| self.cleanup(argv);
        let effects = Effects {
            repo: self.base.clone(),
            env: self.env(),
            runner: self.runner.borrow().clone(),
            executable: PathBuf::from("bench-tools"),
            proc_root: self.proc_root.clone(),
            uid: self.uid.get(),
            clock: &clock,
            sleep,
            nap: &nap,
            meters: &meters,
            tool: &tool,
            poll_seconds: 0.01,
            scan_gap_seconds: 0.01,
            unreadable_wait_seconds: if self.real_proc { 5.0 } else { 0.0 },
        };
        body(&effects)
    }

    /// Tests run on parallel threads, and a child another test is spawning holds a copy of this
    /// test's lock until it execs, so a just-released lock can stay held for a moment.
    fn command(&self, action: Action) -> Result<Outcome, Failure> {
        for _ in 0..100 {
            match self.with(&short_wait, |effects| {
                command(&self.queue, action.clone(), effects)
            }) {
                Err(Failure::Blocked(reasons))
                    if reasons.len() == 1
                        && reasons[0].starts_with("another serial controller holds") =>
                {
                    std::thread::sleep(Duration::from_millis(10));
                }
                other => return other,
            }
        }
        panic!("the serial lock stayed held");
    }

    fn run(&self, count: Option<u64>) -> u8 {
        code(self.command(Action::Run {
            count,
            detach: false,
        }))
    }

    fn recover(&self) -> u8 {
        code(self.command(Action::Recover))
    }

    fn replace(&self, run: &str, attempt: &str) -> u8 {
        code(self.command(Action::Replace {
            run: run.into(),
            attempt: attempt.into(),
        }))
    }

    fn check(&self) -> Value {
        self.command(Action::Check)
            .unwrap_or_else(|failure| panic!("{failure:?}"))
            .printed
            .expect("a report")
    }

    fn status(&self) -> Value {
        self.command(Action::Status {
            snapshot: false,
            acknowledge: None,
        })
        .unwrap_or_else(|failure| panic!("{failure:?}"))
        .printed
        .expect("a report")
    }

    fn touch(&self, name: &str, content: &str) {
        fs::write(self.base.join(name), content).expect("write");
    }

    fn append(&self, name: &str, line: &str) {
        let mut handle = OpenOptions::new()
            .append(true)
            .create(true)
            .open(self.base.join(name))
            .expect("open");
        writeln!(handle, "{line}").expect("append");
    }

    fn lines(&self, name: &str) -> Vec<String> {
        fs::read_to_string(self.base.join(name))
            .unwrap_or_default()
            .lines()
            .map(str::to_string)
            .collect()
    }

    /// Each cell launched, as `run cell`, in order.
    fn launched(&self) -> Vec<String> {
        self.lines("calls.log")
            .iter()
            .filter_map(|line| {
                let words: Vec<&str> = line.split_whitespace().collect();
                let run = Path::new(words.get(1)?).file_name()?.to_string_lossy();
                match words.get(4..6)? {
                    ["--cell", key] if !line.contains("--dry-run") => Some(format!("{run} {key}")),
                    ["--replace", attempt] => Some(format!("{run} replace {attempt}")),
                    _ => None,
                }
            })
            .collect()
    }

    fn events(&self) -> Vec<Value> {
        fs::read_to_string(self.state.join("controller.jsonl"))
            .unwrap_or_default()
            .lines()
            .map(json)
            .collect()
    }

    fn blocked(&self) -> Vec<String> {
        self.events()
            .iter()
            .rev()
            .find(|event| has(event, "blocked"))
            .and_then(|event| get(event, "blocked").as_array())
            .map(|reasons| reasons.iter().map(Value::py_str).collect())
            .unwrap_or_default()
    }

    fn assert_blocked(&self, code: u8, needle: &str) {
        let reasons = self.blocked();
        assert_eq!(code, 1, "{reasons:?}");
        assert!(
            reasons.iter().any(|reason| reason.contains(needle)),
            "{needle:?} not in {reasons:?}"
        );
    }

    fn open(&self, run: &str) -> Run {
        Run::open(&self.run_dir(run), &self.work(run)).expect("run")
    }

    fn diagnose(&self, run: &str, attempt: &str, cause: &str) {
        save(
            &self.run_dir(run).join(format!("deviations/{attempt}.json")),
            &json(&format!(
                r#"{{"predecessor": "{attempt}", "reason": "seen in the transcript", "cause": "{cause}"}}"#
            )),
        );
    }

    /// The process table entry of a fake process.
    fn process(&self, pid: i64, state: &str, ppid: i64, group: i64, start: i64, environ: &str) {
        let directory = self.proc_root.join(pid.to_string());
        fs::create_dir_all(&directory).expect("mkdir");
        fs::write(
            directory.join("stat"),
            format!(
                "{pid} (fake {pid}) {state} {ppid} {group} {group} {} {start} 0\n",
                ["0"; 15].join(" ")
            ),
        )
        .expect("write");
        fs::write(directory.join("environ"), environ.replace(' ', "\0")).expect("write");
        fs::write(directory.join("cmdline"), "fake\0--flag\0").expect("write");
        let _ = fs::remove_file(directory.join("cwd"));
        std::os::unix::fs::symlink("/", directory.join("cwd")).expect("symlink");
    }
}

fn code(result: Result<Outcome, Failure>) -> u8 {
    result.unwrap_or_else(|failure| panic!("{failure:?}")).code
}

fn fake() -> Fixture {
    Fixture::new(false)
}

// --- a full pass -------------------------------------------------------------------------------

#[test]
fn a_full_pass_files_each_cell_once_and_every_run_takes_one_valid_review_before_its_second() {
    let fx = fake();
    assert_eq!(fx.run(None), 0, "{:?}", fx.events());
    assert_eq!(
        fx.launched(),
        [
            "run-a t1/arm-a/1",
            "run-b t1/arm-c/1",
            "run-a t1/arm-b/1",
            "run-b t1/arm-c/2"
        ]
    );
    assert_eq!(
        fx.lines("filed.log"),
        ["att-001", "att-001", "att-002", "att-002"]
    );
    let last = fx.events().pop().expect("an event");
    assert!(matches!(get(&last, "completed"), Value::Bool(true)));
    let records = fx.with(&no_wait, |effects| {
        launches(&Queue::open(&fx.queue, effects).expect("queue")).expect("launches")
    });
    assert_eq!(records.len(), 4);
    for record in &records {
        assert_eq!(get(record, "state").py_str(), "ended");
        assert_eq!(get(get(record, "observation"), "status").py_str(), "absent");
        assert_eq!(get(record, "exit").py_str(), "0");
    }
    let invocation = listed(&fx.state.join("invocations"), "", ".json").expect("invocations");
    let saved = read_json(&fx.state.join("invocations").join(&invocation[0])).expect("record");
    assert_eq!(get(&saved, "outcome").py_str(), "exit 0");
    assert_eq!(fx.run(None), 0);
    assert_eq!(fx.launched().len(), 4, "a finished queue launches nothing");
}

#[test]
fn a_launch_gets_the_pinned_client_and_queue_environment_and_nothing_inherited_from_the_agent() {
    let fx = fake();
    assert_eq!(fx.run(Some(1)), 0);
    let env = fx.lines("env.log");
    let client = display(&fx.base.join("clients/claude"));
    assert!(env.contains(&format!("BENCH_CLAUDE={client}")), "{env:?}");
    assert!(env.contains(&"BENCH_CLAUDE_VERSION=9.9.9".to_string()));
    assert!(env.contains(&format!("QUEUE_REPO={}/x", display(&fx.base))));
    assert!(
        env.iter()
            .any(|line| line.starts_with("BENCH_QUEUE_LAUNCH="))
    );
    for leaked in [
        "CLAUDE_CONFIG_DIR",
        "ANTHROPIC_API_KEY",
        "BENCH_RUST_STALE",
        "AI_AGENT",
    ] {
        assert!(
            !env.iter().any(|line| line.starts_with(leaked)),
            "{leaked}: {env:?}"
        );
    }
}

// --- the gates before a launch -----------------------------------------------------------------

#[test]
fn quota_at_the_stop_ends_the_queue_without_a_launch() {
    let fx = fake();
    *fx.meters.borrow_mut() = Ok(json(
        r#"{"at": "2027-01-15T08:00:00Z", "claude": {"five_hour": {"utilization": 95}}}"#,
    ));
    assert_eq!(fx.run(None), 0);
    assert!(fx.launched().is_empty());
    let last = fx.events().pop().expect("an event");
    assert_eq!(
        get(&last, "stopped").py_str(),
        "Claude usage reached the quota stop"
    );
    assert!(fx.state.join("meters.json").is_file());
}

#[test]
fn a_claude_sign_in_about_to_expire_stops_the_queue() {
    let fx = fake();
    *fx.meters.borrow_mut() = Ok(json(&format!(
        r#"{{"at": "2027-01-15T08:00:00Z", "claude": {{"five_hour": {{"utilization": 10}}}},
        "claude_sign_in_expires_at": {}}}"#,
        (START + 1799.0) * 1000.0
    )));
    assert_eq!(fx.run(None), 0);
    assert!(fx.launched().is_empty());
    assert!(
        get(&fx.events().pop().expect("an event"), "stopped")
            .py_str()
            .contains("renew it while no review runs")
    );
}

#[test]
fn meters_that_are_silent_or_unreadable_start_no_launch() {
    let fx = fake();
    *fx.meters.borrow_mut() = Ok(json(r#"{"at": "2027-01-15T08:00:00Z", "claude": {}}"#));
    fx.assert_blocked(fx.run(None), "no usage window was reported for claude");
    *fx.meters.borrow_mut() = Err(("the meter service refused".into(), false));
    fx.assert_blocked(
        fx.run(None),
        "the subscription meters could not be read, and no launch starts on a reading older than 3600 s: \
         the meter service refused",
    );
    assert!(fx.launched().is_empty());
}

#[test]
fn a_stored_sign_in_that_cannot_be_read_is_an_input_error() {
    let fx = fake();
    *fx.meters.borrow_mut() = Err(("the stored Claude sign-in is not JSON".into(), true));
    let failed = fx.command(Action::Run {
        count: None,
        detach: false,
    });
    assert!(
        matches!(&failed, Err(Failure::Input(why)) if why == "the stored Claude sign-in is not JSON"),
        "{:?}",
        failed.err()
    );
    assert!(fx.launched().is_empty());
}

#[test]
fn no_launch_starts_on_a_meter_reading_older_than_an_hour() {
    let fx = fake();
    // The first review waits for the release, so the hour passes while it runs and before the next launch.
    fx.touch("hold", "");
    let waited = Cell::new(false);
    let sleep = |_: f64| {
        if !waited.replace(true) {
            fx.clock.set(fx.clock.get() + 3601.0);
            *fx.meters.borrow_mut() = Err(("offline".into(), false));
            fx.touch("release", "");
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    let outcome = fx.with(&sleep, |effects| {
        command(
            &fx.queue,
            Action::Run {
                count: None,
                detach: false,
            },
            effects,
        )
    });
    fx.assert_blocked(
        code(outcome),
        "no launch starts on a reading older than 3600 s: offline",
    );
    assert_eq!(fx.launched(), ["run-a t1/arm-a/1"]);
}

#[test]
fn filed_usage_and_charges_count_against_the_client_ceiling() {
    let fx = fake();
    let mut spec = fx.spec();
    put(&mut spec, "ceilings_usd", json(r#"{"claude": 12}"#));
    fx.write_queue(&spec);
    fx.touch("usd", "2.0");
    assert_eq!(fx.run(Some(1)), 0);
    fs::write(
        fx.run_dir(RUN_A).join("charges.jsonl"),
        r#"{"usd": 4.0, "step": "probe", "at": "2027-01-01T00:00:00Z"}"#,
    )
    .expect("write");
    fx.assert_blocked(
        fx.run(None),
        "claude spend 8.00 + bound 5.00 exceeds the queue ceiling 12.00",
    );
    assert_eq!(fx.launched().len(), 3, "the 2.00 review, then two at 1.00");
    let spend = get(get(&fx.status(), "spend_usd"), "claude").clone();
    assert_eq!(get(&spend, "total").py_str(), "8.0");
    assert_eq!(get(&spend, "charges").py_str(), "4.0");
}

#[test]
fn usage_beyond_a_reservation_in_one_run_stops_every_launch_of_the_queue() {
    let fx = fake();
    fx.touch("usd", "9.0");
    assert_eq!(fx.run(Some(1)), 0);
    fx.assert_blocked(
        fx.run(None),
        "run-a: att-001 used more than was reserved; nothing in the queue launches until charges.jsonl carries \
         a line with \"reconciles\" naming each",
    );
    assert_eq!(fx.launched().len(), 1);
    fs::write(
        fx.run_dir(RUN_A).join("charges.jsonl"),
        r#"{"usd": 4.0, "step": "overrun", "at": "2027-01-01T00:00:00Z", "reconciles": "att-001"}"#,
    )
    .expect("write");
    assert_eq!(fx.run(Some(1)), 0);
    assert_eq!(fx.launched().len(), 2);
}

#[test]
fn incomplete_cleanup_of_a_valid_review_blocks_the_next_launch_until_it_is_pruned() {
    let fx = fake();
    assert_eq!(fx.run(Some(1)), 0);
    let workspace = fx.work(RUN_A).join("att-001");
    fs::remove_file(workspace.join("workspace-pruned.json")).expect("remove");
    *fx.cleanup_refusal.borrow_mut() = Some("clone or cache is a symlink".into());
    fx.assert_blocked(
        fx.run(None),
        "run-a: cleanup incomplete: att-001: evidence filed, but workspace cleanup failed: clone or cache is a symlink",
    );
    let report = fx.check();
    assert!(
        texts_of(get(&report, "blocked"))
            .iter()
            .any(|reason| reason.contains("recover or run prunes it when it is eligible")),
        "{report:?}"
    );
    assert_eq!(fx.lines("tools.log").len(), 1, "check prunes nothing");
    *fx.cleanup_refusal.borrow_mut() = None;
    assert_eq!(fx.run(Some(1)), 0);
    let calls = fx.lines("tools.log");
    assert!(calls[1].contains(&format!("--workspace {}", display(&workspace))));
    assert!(calls[1].ends_with("--apply"));
    assert_eq!(fx.launched().len(), 2);
}

fn texts_of(value: &Value) -> Vec<String> {
    value
        .as_array()
        .unwrap_or(&[])
        .iter()
        .map(Value::py_str)
        .collect()
}

#[test]
fn check_dry_runs_the_next_cell_of_each_run_and_launches_nothing() {
    let fx = fake();
    let report = fx.check();
    assert!(
        matches!(get(&report, "ready"), Value::Bool(true)),
        "{report:?}"
    );
    assert!(matches!(get(&report, "quota_stop"), Value::Null));
    let calls = fx.lines("calls.log");
    assert_eq!(calls.len(), 2);
    assert!(calls[0].ends_with("--cell t1/arm-a/1 --dry-run"));
    assert!(calls[1].ends_with("--cell t1/arm-c/1 --dry-run"));
    assert!(!fx.state.join("queue.pin.json").exists());
    fx.touch("refuse", "the target's packet changed\n");
    let report = fx.check();
    assert!(matches!(get(&report, "ready"), Value::Bool(false)));
    assert_eq!(
        texts_of(get(&report, "blocked"))[0],
        "run-a: the target's packet changed"
    );
}

// --- pins ----------------------------------------------------------------------------------------

#[test]
fn a_second_controller_is_refused_while_the_first_holds_the_queue() {
    let fx = fake();
    let held = fx.with(&no_wait, |effects| {
        serial(&Queue::open(&fx.queue, effects).expect("queue")).expect("lock")
    });
    let refused = fx.with(&no_wait, |effects| {
        command(
            &fx.queue,
            Action::Run {
                count: None,
                detach: false,
            },
            effects,
        )
    });
    assert!(
        matches!(&refused, Err(Failure::Blocked(reasons)) if reasons[0].starts_with("another serial controller holds")),
        "{:?}",
        refused.err()
    );
    drop(held);
    assert!(fx.launched().is_empty());
    assert_eq!(fx.run(Some(1)), 0);
}

#[test]
fn a_changed_queue_or_manifest_is_refused_after_the_queue_was_pinned() {
    let fx = fake();
    assert_eq!(fx.run(Some(1)), 0);
    let mut spec = fx.spec();
    put(&mut spec, "ceilings_usd", json(r#"{"claude": 800}"#));
    fx.write_queue(&spec);
    fx.assert_blocked(
        fx.run(None),
        "the queue file or a run manifest changed since",
    );
    fx.write_queue(&fx.spec());
    let head = fx.head();
    fx.freeze(RUN_B, &["arm-c"], &["t1/arm-c/1"], &head);
    fx.assert_blocked(
        fx.run(None),
        "the queue file or a run manifest changed since",
    );
    assert_eq!(fx.launched().len(), 1);
}

fn blocked_before_launch(fx: &Fixture) -> Vec<String> {
    match fx.command(Action::Check) {
        Err(Failure::Blocked(reasons)) => reasons,
        Err(other) => panic!("{other:?}"),
        Ok(outcome) => texts_of(get(&outcome.printed.expect("a report"), "blocked")),
    }
}

#[test]
fn a_client_that_no_longer_hashes_to_its_pin_or_differs_from_the_frozen_pin_is_refused() {
    let fx = fake();
    fs::write(fx.base.join("clients/claude"), "another claude\n").expect("write");
    assert_eq!(
        blocked_before_launch(&fx),
        ["the claude executable no longer hashes to its pin"]
    );
    fs::write(fx.base.join("clients/claude"), "fake claude\n").expect("write");
    let mut frozen = Object::default();
    frozen.insert(
        "claude".into(),
        json(r#"{"path": "/elsewhere/claude", "sha256": "0", "version": "9.9.9"}"#),
    );
    save(
        &fx.run_dir(RUN_B).join("clients.frozen.json"),
        &Value::Object(frozen),
    );
    assert_eq!(
        blocked_before_launch(&fx),
        ["run-b: the queue's claude pin differs from the run's clients.frozen.json"]
    );
    assert!(fx.launched().is_empty());
}

#[test]
fn a_companion_file_is_handed_to_the_launch_and_a_missing_one_is_refused() {
    let fx = fake();
    let companion = fx.base.join("clients/lib.txt");
    fs::write(&companion, "lib\n").expect("write");
    let mut spec = fx.spec();
    let mut clients = get(&spec, "clients").clone();
    let mut pin = get(&clients, "claude").clone();
    put(
        &mut pin,
        "companions",
        json(&format!(
            r#"{{"lib.txt": "{}"}}"#,
            sha256_file(&companion).expect("hash")
        )),
    );
    put(&mut clients, "claude", pin);
    put(&mut spec, "clients", clients);
    fx.write_queue(&spec);
    assert_eq!(fx.run(Some(1)), 0);
    assert!(
        fx.lines("env.log")
            .iter()
            .any(|line| line.starts_with("BENCH_CLAUDE_COMPANIONS=") && line.ends_with("lib.txt"))
    );
    fs::remove_file(&companion).expect("remove");
    fx.assert_blocked(
        fx.run(None),
        "the pinned claude companion file lib.txt cannot be read",
    );
}

#[test]
fn state_on_scratch_or_memory_storage_is_refused() {
    let fx = fake();
    fx.edit_queue("state_dir", text("/tmp/q-test-state"));
    assert_eq!(
        blocked_before_launch(&fx),
        [format!(
            "the state directory is not persistent storage: {} is under /tmp",
            display(&resolved(Path::new("/tmp/q-test-state")))
        )]
    );
    fx.write_queue(&fx.spec());
    fs::create_dir_all(fx.proc_root.join("self")).expect("mkdir");
    fs::write(
        fx.proc_root.join("self/mountinfo"),
        format!(
            "22 1 8:1 / / rw - ext4 /dev/sda rw\n23 22 0:5 / {} rw - tmpfs tmpfs rw\n",
            display(&fx.state).replace(' ', "\\040")
        ),
    )
    .expect("write");
    assert_eq!(
        blocked_before_launch(&fx),
        [format!(
            "the state directory is not persistent storage: {} is on tmpfs",
            display(&fx.state)
        )]
    );
}

#[test]
fn the_file_system_is_the_one_mounted_deepest_over_the_path() {
    let fx = fake();
    fs::create_dir_all(fx.proc_root.join("self")).expect("mkdir");
    fs::write(
        fx.proc_root.join("self/mountinfo"),
        "22 1 8:1 / / rw - ext4 /dev/sda rw\n\
         23 22 0:5 / /data rw - tmpfs tmpfs rw\n\
         24 23 8:2 / /data/disk rw - ext4 /dev/sdb rw\n\
         25 22 0:6 / /with\\040space rw - ramfs none rw\n",
    )
    .expect("write");
    let kind = |path: &str| filesystem(Path::new(path), &fx.proc_root);
    assert_eq!(kind("/data/x").as_deref(), Some("tmpfs"));
    assert_eq!(kind("/data/disk/x").as_deref(), Some("ext4"));
    assert_eq!(kind("/with space/x").as_deref(), Some("ramfs"));
    assert_eq!(kind("/elsewhere").as_deref(), Some("ext4"));
}

#[test]
fn the_queue_environment_may_not_set_a_pinned_name_and_runtime_names_may_not_clash() {
    let fx = fake();
    fx.edit_queue(
        "environment",
        json(r#"{"BENCH_CLAUDE": "/other", "BENCH_RUST_X": "y"}"#),
    );
    let refused = fx.command(Action::Check);
    assert!(
        matches!(&refused, Err(Failure::Input(why)) if why.ends_with(
            "environment may not set BENCH_CLAUDE, BENCH_RUST_X; clients are pinned under clients")),
        "{:?}",
        refused.err()
    );
    fx.edit_queue(
        "runtimes",
        json(
            r#"{"a-b": {"sha256": "1", "receipt_sha256": "2"}, "a_b": {"sha256": "1", "receipt_sha256": "2"}}"#,
        ),
    );
    let refused = fx.command(Action::Check);
    assert!(
        matches!(&refused, Err(Failure::Input(why)) if why.ends_with(
            "runtime names generate the same launch variable BENCH_RUST_A_B, BENCH_RUST_A_B_SHA256; rename one")),
        "{:?}",
        refused.err()
    );
}

#[test]
fn runner_files_that_differ_from_the_freeze_commit_are_refused_and_name_it() {
    let fx = fake();
    let head = fx.head();
    fs::write(fx.base.join("bench/tools/runner.txt"), "v2\n").expect("write");
    assert_eq!(
        blocked_before_launch(&fx),
        [format!(
            "run-a: the runner files differ from freeze commit {head}"
        )]
    );
    assert!(fx.launched().is_empty());
}

#[test]
fn a_run_frozen_with_bench_tools_but_no_pin_or_a_queue_with_no_runner_is_refused() {
    let fx = fake();
    *fx.runner.borrow_mut() = None;
    assert_eq!(
        blocked_before_launch(&fx),
        ["the queue pins no bench-tools runtime to run its cells"]
    );
    fs::create_dir_all(fx.base.join("rust/bench-tools")).expect("mkdir");
    fs::write(fx.base.join("rust/bench-tools/Cargo.toml"), "[package]\n").expect("write");
    git(&fx.base, &["add", "-A", "rust"]);
    git(&fx.base, &["commit", "-qm", "crate"]);
    let head = fx.head();
    fx.freeze(
        RUN_A,
        &["arm-a", "arm-b"],
        &["t1/arm-a/1", "t1/arm-b/1"],
        &head,
    );
    fx.freeze(RUN_B, &["arm-c"], &["t1/arm-c/1", "t1/arm-c/2"], &head);
    assert_eq!(
        blocked_before_launch(&fx),
        [format!(
            "run-a: freeze commit {head} has rust/bench-tools, which normalizes its reviews and runs its cells, \
             but the run pins no bench-tools runtime"
        )]
    );
}

#[test]
fn a_queue_pin_that_differs_from_the_runtimes_a_run_froze_is_refused() {
    let fx = fake();
    save(
        &fx.run_dir(RUN_A).join("runtimes.frozen.json"),
        &json(r#"{"bench-tools": {"sha256": "1", "receipt_sha256": "2"}}"#),
    );
    assert_eq!(
        blocked_before_launch(&fx),
        ["run-a: the queue's Rust runtime pins differ from the run's runtimes.frozen.json"]
    );
}

// --- interrupted launches ------------------------------------------------------------------------

#[test]
fn a_runner_killed_before_the_reviewer_started_releases_its_claim_and_the_cell_runs_once() {
    let fx = fake();
    fx.touch("crash", "before_review");
    fx.assert_blocked(
        fx.run(None),
        "run-a: run-cell exit 9 and filed no attempt (launch-0001)",
    );
    assert_eq!(fx.open(RUN_A).in_flight(), ["att-001"]);
    let report = fx.check();
    assert!(
        texts_of(get(&report, "blocked")).iter().any(|reason| reason
            == "run-a/att-001 is claimed and not filed; its processes are gone, so recover or run settles it"),
        "{report:?}"
    );
    assert_eq!(
        fx.open(RUN_A).in_flight(),
        ["att-001"],
        "check changes nothing"
    );
    assert_eq!(fx.recover(), 0);
    let calls = fx.lines("calls.log");
    let release = calls
        .iter()
        .find(|line| line.contains("--release"))
        .expect("a release");
    assert!(release.contains("--release att-001 --interrupted "));
    let evidence =
        listed(&fx.state.join("recoveries"), "run-a-att-001-", ".json").expect("recoveries");
    assert_eq!(evidence.len(), 1);
    let saved = read_json(&fx.state.join("recoveries").join(&evidence[0])).expect("evidence");
    assert_eq!(get(&saved, "status").py_str(), "absent");
    assert_eq!(get(get(&saved, "claim"), "reserved_usd").py_str(), "5.0");
    let record = read_json(&fx.state.join("launches/launch-0001.json")).expect("record");
    assert_eq!(get(&record, "state").py_str(), "recovered");
    assert_eq!(
        get(&record, "outcome").py_str(),
        "released: no reviewer had started"
    );
    assert_eq!(
        get(
            get(get(&fx.status(), "spend_usd"), "claude"),
            "in_flight_reserved"
        )
        .py_str(),
        "0.0"
    );
    assert_eq!(fx.run(None), 0);
    assert_eq!(
        fx.launched(),
        [
            "run-a t1/arm-a/1",
            "run-a t1/arm-a/1",
            "run-b t1/arm-c/1",
            "run-a t1/arm-b/1",
            "run-b t1/arm-c/2"
        ]
    );
    assert_eq!(fx.lines("filed.log").len(), 4);
}

/// A directory whose mode denies listing it, restored on drop so a failed test leaves no undeletable files.
struct Denied<'a>(&'a Path);

impl Drop for Denied<'_> {
    fn drop(&mut self) {
        let _ = fs::set_permissions(self.0, fs::Permissions::from_mode(0o755));
    }
}

/// None when a privileged user reads past the mode, so there is no denial to observe.
fn deny(directory: &Path, mode: u32) -> Option<Denied<'_>> {
    fs::set_permissions(directory, fs::Permissions::from_mode(mode)).expect("chmod");
    let denied = Denied(directory);
    fs::read_dir(directory).is_err().then_some(denied)
}

fn permission_denied(result: Result<Outcome, Failure>, what: &str) {
    match result {
        Err(Failure::Input(message)) => assert!(
            message.contains("[Errno 13] Permission denied"),
            "{what}: {message}"
        ),
        Err(failure) => panic!("{what}: {failure:?}"),
        Ok(outcome) => panic!(
            "{what} read unreadable state as empty: exit {}",
            outcome.code
        ),
    }
}

#[test]
fn an_unreadable_launch_listing_is_an_error_not_an_empty_queue() {
    let fx = fake();
    assert_eq!(fx.run(Some(1)), 0);
    let launched = fx.launched();
    let launches = fx.state.join("launches");
    let Some(denied) = deny(&launches, 0o111) else {
        return;
    };
    permission_denied(
        fx.command(Action::Status {
            snapshot: false,
            acknowledge: None,
        }),
        "status",
    );
    permission_denied(
        fx.command(Action::Run {
            count: None,
            detach: false,
        }),
        "run",
    );
    drop(denied);
    assert_eq!(fx.launched(), launched, "nothing launched");
}

#[test]
fn an_unreadable_claim_is_never_released() {
    let fx = fake();
    fx.touch("crash", "before_review");
    fx.assert_blocked(fx.run(None), "run-cell exit 9 and filed no attempt");
    let claim = fx.work(RUN_A).join("att-001");
    let Some(denied) = deny(&claim, 0o000) else {
        return;
    };
    permission_denied(fx.command(Action::Recover), "recover");
    drop(denied);
    assert!(
        !fx.lines("calls.log")
            .iter()
            .any(|line| line.contains("--release")),
        "released"
    );
    assert_eq!(fx.open(RUN_A).in_flight(), ["att-001"]);
}

#[test]
fn a_runner_killed_during_the_review_files_it_interrupted_and_keeps_its_reservation() {
    let fx = fake();
    fx.touch("crash", "during_review");
    fx.assert_blocked(fx.run(None), "run-cell exit 9 and filed no attempt");
    fx.assert_blocked(
        fx.run(None),
        "run-a/att-001: 'stopped: interrupted' has no diagnosis in deviations/",
    );
    let filing = fx
        .lines("calls.log")
        .into_iter()
        .find(|line| line.contains("--file"))
        .expect("a filing");
    assert!(filing.contains("--file att-001 --interrupted "));
    assert!(filing.ends_with(
        "--reason the controller stopped and the dispatch never returned; its processes were observed gone"
    ));
    let spend = get(get(&fx.status(), "spend_usd"), "claude").clone();
    assert_eq!(get(&spend, "unknown_attempts").py_str(), "1");
    assert_eq!(get(&spend, "unknown_reserved").py_str(), "5.0");
    assert_eq!(get(&spend, "total").py_str(), "5.0");
    fx.assert_blocked(fx.run(None), "has no diagnosis in deviations/");
    assert_eq!(fx.lines("filed.log"), ["att-001"], "nothing is filed twice");
}

#[test]
fn a_runner_killed_after_the_dispatch_returned_files_what_the_return_proves() {
    let fx = fake();
    fx.touch("crash", "after_return");
    fx.assert_blocked(fx.run(None), "run-cell exit 9 and filed no attempt");
    assert_eq!(fx.run(Some(1)), 0);
    let calls = fx.lines("calls.log");
    assert!(calls.iter().any(|line| line.ends_with("--file att-001")));
    let record = read_json(&fx.state.join("launches/launch-0001.json")).expect("record");
    assert_eq!(
        get(&record, "outcome").py_str(),
        "filed: the dispatch had ended"
    );
    assert_eq!(fx.lines("filed.log"), ["att-001", "att-001"]);
    assert_eq!(fx.launched(), ["run-a t1/arm-a/1", "run-b t1/arm-c/1"]);
}

#[test]
fn a_launch_the_controller_never_recorded_as_ended_is_closed_with_its_claim() {
    let fx = fake();
    fx.touch("crash", "during_review");
    fx.assert_blocked(fx.run(None), "run-cell exit 9");
    let path = fx.state.join("launches/launch-0001.json");
    let mut record = read_json(&path).expect("record");
    put(&mut record, "state", text("spawned"));
    put(&mut record, "attempt_id", Value::Null);
    save(&path, &record);
    let intent = merged(
        &record,
        vec![
            ("launch_id", text("launch-0002")),
            ("state", text("intent")),
            ("attempts_before", texts(&["att-001"])),
        ],
    );
    save(&fx.state.join("launches/launch-0002.json"), &intent);
    assert_eq!(fx.recover(), 0, "{:?}", fx.events());
    let closed = read_json(&path).expect("record");
    assert_eq!(get(&closed, "outcome").py_str(), "filed as interrupted");
    assert_eq!(get(&closed, "attempt_id").py_str(), "att-001");
    let closed = read_json(&fx.state.join("launches/launch-0002.json")).expect("record");
    assert_eq!(get(&closed, "state").py_str(), "recovered");
    assert_eq!(get(&closed, "outcome").py_str(), "no claim was made");
    assert_eq!(fx.lines("filed.log"), ["att-001"]);
}

#[test]
fn a_claim_that_no_launch_record_names_is_never_filed_by_the_queue() {
    let fx = fake();
    let claim = fx.work(RUN_A).join("att-001");
    fs::create_dir_all(&claim).expect("mkdir");
    fs::write(
        claim.join("cell.json"),
        r#"{"cell": {"target": "t1", "arm": "arm-a", "replicate": 1}, "reserved_usd": 5.0, "predecessor": null}"#,
    )
    .expect("write");
    for _ in 0..2 {
        fx.assert_blocked(
            fx.run(None),
            "run-a/att-001 is claimed and not filed, and its processes are unknown: no launch record names this \
             claim, so nothing says where its processes ran",
        );
    }
    assert!(fx.lines("calls.log").is_empty());
}

#[test]
fn a_launch_record_from_another_boot_is_absent_and_from_another_host_is_unknown() {
    let fx = fake();
    fx.touch("crash", "during_review");
    fx.assert_blocked(fx.run(None), "run-cell exit 9");
    let path = fx.state.join("launches/launch-0001.json");
    let record = read_json(&path).expect("record");
    let mut process = get(&record, "process").clone();
    put(&mut process, "host", text("elsewhere"));
    save(&path, &merged(&record, vec![("process", process.clone())]));
    fx.assert_blocked(
        fx.run(None),
        "its processes are unknown: the launch ran on host elsewhere; observe it there",
    );
    put(
        &mut process,
        "host",
        get(&record, "process")
            .clone()
            .pipe(|p| get(&p, "host").clone()),
    );
    put(&mut process, "boot_id", Value::Null);
    save(&path, &merged(&record, vec![("process", process.clone())]));
    fx.assert_blocked(fx.run(None), "the launch record names no boot");
    put(&mut process, "boot_id", text("boot-0"));
    save(&path, &merged(&record, vec![("process", process)]));
    assert_eq!(fx.recover(), 0);
    let observed = get(&read_json(&path).expect("record"), "observation").clone();
    assert!(
        get(&observed, "verified_by")
            .py_str()
            .contains("the boot id is boot-1, not the boot-0 the launch ran under")
    );
}

trait Pipe: Sized {
    fn pipe<T>(self, f: impl FnOnce(Self) -> T) -> T {
        f(self)
    }
}

impl Pipe for Value {}

// --- diagnoses and replacements ------------------------------------------------------------------

#[test]
fn a_harness_stop_is_replaced_once_on_the_frozen_run_and_a_valid_review_never_is() {
    let fx = fake();
    fx.touch("disposition", "stopped: harness");
    fx.assert_blocked(
        fx.run(None),
        "run-a: the review filed as 'stopped: harness'; save a diagnosis before the queue goes on",
    );
    fx.assert_blocked(
        fx.run(None),
        "run-a/att-001: 'stopped: harness' has no diagnosis in deviations/",
    );
    fx.diagnose(RUN_A, "att-001", "harness-stop");
    fx.assert_blocked(
        fx.run(None),
        "run-a/att-001: diagnosed as harness-stop; replace it before the queue goes on",
    );
    assert_eq!(fx.replace(RUN_A, "att-001"), 0, "{:?}", fx.events());
    let replacement = fx
        .lines("calls.log")
        .into_iter()
        .find(|line| line.contains("--replace"))
        .expect("a replacement");
    let digest = sha256_file(&fx.run_dir(RUN_A).join("deviations/att-001.json")).expect("hash");
    assert!(replacement.contains(&format!(
        "--replace att-001 --reason seen in the transcript (diagnosis deviations/att-001.json, sha256 {digest}) --quota "
    )));
    assert!(replacement.ends_with("--stopped-by-harness"));
    let filed = &fx.open(RUN_A).filed["att-002"];
    assert_eq!(get(filed, "predecessor").py_str(), "att-001");
    assert_eq!(get(get(filed, "cell"), "arm").py_str(), "arm-a");
    fx.assert_blocked(
        fx.replace(RUN_A, "att-001"),
        "run-a/att-001 already has the replacement att-002",
    );
    fx.assert_blocked(
        fx.replace(RUN_A, "att-002"),
        "run-a/att-002 is not a filed failure; a valid attempt is never replaced",
    );
    assert_eq!(fx.run(None), 0);
    assert_eq!(fx.launched().len(), 5);
}

#[test]
fn a_harness_invalid_replacement_is_not_told_the_harness_stopped_it() {
    let fx = fake();
    fx.touch("disposition", "harness-invalid: missing transcript");
    fx.assert_blocked(fx.run(None), "save a diagnosis");
    fx.diagnose(RUN_A, "att-001", "harness-invalid");
    assert_eq!(fx.replace(RUN_A, "att-001"), 0);
    let replacement = fx
        .lines("calls.log")
        .into_iter()
        .find(|line| line.contains("--replace"))
        .expect("a replacement");
    assert!(!replacement.contains("--stopped-by-harness"));
}

#[test]
fn capacity_before_any_call_succeeded_stops_only_that_arm_and_the_others_go_on() {
    let fx = fake();
    fx.touch("disposition", "stopped: capacity");
    fx.assert_blocked(fx.run(None), "save a diagnosis");
    fx.diagnose(RUN_A, "att-001", "transient-capacity");
    fx.assert_blocked(
        fx.replace(RUN_A, "att-001"),
        "run-a/att-001: arm-a was rejected before any call of it succeeded, so the arm stops",
    );
    assert_eq!(fx.run(None), 0, "{:?}", fx.events());
    assert_eq!(
        fx.launched(),
        [
            "run-a t1/arm-a/1",
            "run-a t1/arm-b/1",
            "run-b t1/arm-c/1",
            "run-b t1/arm-c/2"
        ]
    );
    let last = fx.events().pop().expect("an event");
    assert!(
        get(get(get(&last, "stopped_arms"), RUN_A), "arm-a")
            .py_str()
            .starts_with("att-001: arm-a was rejected")
    );
}

#[test]
fn capacity_after_a_call_succeeded_is_replaced() {
    let fx = fake();
    fx.touch("disposition", "stopped: capacity");
    fx.assert_blocked(fx.run(None), "save a diagnosis");
    fx.diagnose(RUN_A, "att-001", "transient-capacity");
    fs::write(
        fx.run_dir(RUN_A)
            .join("attempts/att-001/usage-requests.jsonl"),
        "{\"model\": \"m\"}\n",
    )
    .expect("write");
    assert_eq!(fx.replace(RUN_A, "att-001"), 0, "{:?}", fx.events());
    assert!(
        fx.lines("calls.log")
            .iter()
            .any(|line| line.ends_with("--stopped-by-harness"))
    );
}

#[test]
fn capacity_after_a_valid_review_of_the_same_arm_is_replaced() {
    let fx = fake();
    assert_eq!(fx.run(Some(3)), 0);
    fx.touch("disposition", "stopped: capacity");
    fx.assert_blocked(
        fx.run(None),
        "run-b: the review filed as 'stopped: capacity'",
    );
    fx.diagnose(RUN_B, "att-002", "transient-capacity");
    assert_eq!(fx.replace(RUN_B, "att-002"), 0, "{:?}", fx.events());
    assert_eq!(
        fx.launched().last().map(String::as_str),
        Some("run-b replace att-002")
    );
}

#[test]
fn a_setup_rejection_stops_the_arm_and_a_kept_result_is_never_replaced() {
    let fx = fake();
    fx.touch("disposition", "stopped: setup");
    fx.assert_blocked(fx.run(Some(1)), "save a diagnosis");
    fx.diagnose(RUN_A, "att-001", "setup-rejection");
    fx.touch("disposition", "stopped: timeout");
    fx.assert_blocked(
        fx.run(Some(1)),
        "run-a: the review filed as 'stopped: timeout'",
    );
    fx.diagnose(RUN_A, "att-002", "skill-timeout");
    fx.assert_blocked(
        fx.replace(RUN_A, "att-002"),
        "run-a/att-002: a skill timeout is the cell's result, not a rerun opportunity",
    );
    assert_eq!(fx.run(None), 0, "{:?}", fx.events());
    assert_eq!(
        fx.launched(),
        [
            "run-a t1/arm-a/1",
            "run-a t1/arm-b/1",
            "run-b t1/arm-c/1",
            "run-b t1/arm-c/2"
        ]
    );
}

#[test]
fn a_reviewer_that_ended_its_session_without_a_result_is_kept_and_the_queue_goes_on() {
    let fx = fake();
    fx.touch("disposition", "stopped: reviewer");
    fx.assert_blocked(fx.run(Some(1)), "save a diagnosis");
    fx.diagnose(RUN_A, "att-001", "reviewer-stop");
    assert_eq!(fx.run(None), 0);
    assert_eq!(fx.launched().len(), 4);
}

#[test]
fn a_diagnosis_names_one_known_cause_and_a_reason() {
    let fx = fake();
    fx.touch("disposition", "stopped: other");
    fx.assert_blocked(fx.run(None), "save a diagnosis");
    fx.diagnose(RUN_A, "att-001", "flaky");
    fx.assert_blocked(
        fx.run(None),
        &format!("deviations/att-001.json: a diagnosis gives a reason and a cause among {CAUSES}"),
    );
    fx.diagnose(RUN_A, "att-001", "harness-stop");
    save(
        &fx.run_dir(RUN_A).join("deviations/second.json"),
        &json(r#"{"predecessor": "att-001", "reason": "again", "cause": "harness-stop"}"#),
    );
    let refused = fx.command(Action::Run {
        count: None,
        detach: false,
    });
    assert!(
        matches!(&refused, Ok(outcome) if outcome.code == 1),
        "{:?}",
        refused.err()
    );
    fx.assert_blocked(1, "run-a/att-001 has 2 diagnoses in deviations/; keep one");
}

// --- status ----------------------------------------------------------------------------------------

#[test]
fn a_snapshot_is_not_due_before_thirty_minutes_and_its_delivery_is_recorded_apart() {
    let fx = fake();
    let (first, second) = fx.with(&no_wait, |effects| {
        let queue = Queue::open(&fx.queue, effects).expect("queue");
        let first = snapshot(&queue, START, true, effects).expect("snapshot");
        assert!(
            snapshot(&queue, START + 1799.0, true, effects)
                .expect("snapshot")
                .is_none()
        );
        let second = snapshot(&queue, START + 1800.0, true, effects).expect("snapshot");
        (first.expect("saved"), second.expect("saved"))
    });
    let name = |path: &PathBuf| {
        path.file_name()
            .expect("a name")
            .to_string_lossy()
            .into_owned()
    };
    let saved = fs::read(&first).expect("read");
    let report = fx.status();
    assert_eq!(
        get(&report, "snapshots").py_repr(),
        format!(
            "{{'saved': 2, 'unacknowledged': ['{}', '{}']}}",
            name(&first),
            name(&second)
        )
    );
    assert!(
        get(&report, "delivery")
            .py_str()
            .contains("nothing here sends it")
    );
    let acknowledged = fx.command(Action::Status {
        snapshot: false,
        acknowledge: Some((name(&first), Some("posted to the owner in chat".into()))),
    });
    assert_eq!(code(acknowledged), 0);
    assert_eq!(
        texts_of(get(get(&fx.status(), "snapshots"), "unacknowledged")),
        [name(&second)]
    );
    assert_eq!(fs::read(&first).expect("read"), saved);
    let receipt =
        json(&fs::read_to_string(fx.state.join("status/acknowledged.jsonl")).expect("read"));
    assert_eq!(get(&receipt, "snapshot").py_str(), name(&first));
    assert_eq!(
        get(&receipt, "note").py_str(),
        "posted to the owner in chat"
    );
    for (snapshot, note, expected) in [
        (
            "20260101T000000Z.json",
            Some("sent"),
            "no saved snapshot 20260101T000000Z.json",
        ),
        (
            name(&second).as_str(),
            None,
            "--acknowledge needs --note saying how the snapshot was delivered",
        ),
    ] {
        let refused = fx.command(Action::Status {
            snapshot: false,
            acknowledge: Some((snapshot.to_string(), note.map(str::to_string))),
        });
        assert!(
            matches!(&refused, Err(Failure::Input(why)) if why == expected),
            "{:?}",
            refused.err()
        );
    }
}

#[test]
fn the_marker_finds_a_claims_launch_and_a_legacy_claim_needs_an_ended_launch_naming_it() {
    let fx = fake();
    let claim = fx.work(RUN_A).join("att-001");
    fs::create_dir_all(&claim).expect("mkdir");
    fs::write(
        claim.join("cell.json"),
        r#"{"cell": {"target": "t1", "arm": "arm-a", "replicate": 1}, "reserved_usd": 5.0, "queue_launch_marker": "m2"}"#,
    )
    .expect("write");
    let record = |run: &str, state: &str, marker: &str, attempt: Value| {
        json(&format!(
            r#"{{"run_id": "{run}", "state": "{state}", "process": {{"marker": "{marker}"}}}}"#
        ))
        .pipe(|value| merged(&value, vec![("attempt_id", attempt)]))
    };
    let run = fx.open(RUN_A);
    let records = [
        record(RUN_A, "spawned", "m2", Value::Null),
        record(RUN_A, "ended", "m1", text("att-001")),
        record(RUN_B, "spawned", "m2", Value::Null),
        record(RUN_A, "recovered", "m2", text("att-001")),
    ];
    assert_eq!(launch_for(&records, &run, "att-001"), Some(0));
    assert_eq!(launch_for(&records[1..], &run, "att-001"), None);
    fs::write(
        claim.join("cell.json"),
        r#"{"cell": {"target": "t1", "arm": "arm-a", "replicate": 1}, "reserved_usd": 5.0}"#,
    )
    .expect("write");
    let run = fx.open(RUN_A);
    assert_eq!(launch_for(&records, &run, "att-001"), Some(1));
    assert_eq!(launch_for(&records[..1], &run, "att-001"), None);
}

// --- what a reading of the process table shows ---------------------------------------------------

fn observed(fx: &Fixture, pairs: Vec<(&str, Value)>, attempt_dir: Option<&Path>) -> Value {
    fx.with(&no_wait, |effects| {
        let evidence = merged(&identity(effects), pairs);
        observe(Some(&evidence), attempt_dir, effects)
    })
}

fn launch_evidence(marker: &str) -> Vec<(&'static str, Value)> {
    vec![
        ("marker", text(marker)),
        ("pid", int(100)),
        ("start_ticks", int(50)),
        ("process_group", int(100)),
    ]
}

#[test]
fn a_zombie_wrapper_does_not_hide_the_child_it_left_running() {
    let fx = fake();
    fx.process(100, "Z", 1, 100, 50, "");
    fx.process(101, "S", 100, 101, 60, "HOME=/h");
    let seen = observed(&fx, launch_evidence("m1"), None);
    assert_eq!(status_of(&seen), "running", "{seen:?}");
    assert_eq!(get(&seen, "wrapper").py_str(), "a zombie");
    assert_eq!(
        get(&seen, "reason").py_str(),
        "live processes of the launch: [101]"
    );
}

#[test]
fn a_process_carrying_the_marker_or_naming_the_attempt_is_the_launchs_unless_it_is_older() {
    let fx = fake();
    fx.process(200, "S", 1, 200, 60, "BENCH_QUEUE_LAUNCH=m1 HOME=/h");
    let seen = observed(&fx, launch_evidence("m1"), None);
    assert_eq!(status_of(&seen), "running", "{seen:?}");
    assert_eq!(
        get(
            get(&seen, "processes")
                .as_array()
                .expect("rows")
                .first()
                .expect("a row"),
            "matched_by"
        )
        .py_str(),
        "the launch marker or the attempt directory"
    );
    assert_eq!(get(&seen, "wrapper").py_str(), "gone");
    fx.process(200, "S", 1, 200, 40, "BENCH_QUEUE_LAUNCH=m1");
    assert_eq!(
        status_of(&observed(&fx, launch_evidence("m1"), None)),
        "absent"
    );
    fx.process(200, "S", 1, 200, 60, "PWD=/work/att-007");
    assert_eq!(
        status_of(&observed(
            &fx,
            launch_evidence("m1"),
            Some(Path::new("/work/att-007"))
        )),
        "running"
    );
}

#[test]
fn a_reused_pid_is_another_process_and_is_left_alone() {
    let fx = fake();
    fx.process(100, "S", 1, 100, 70, "HOME=/h");
    let seen = observed(&fx, launch_evidence("m1"), None);
    assert_eq!(status_of(&seen), "absent", "{seen:?}");
    assert_eq!(get(&seen, "wrapper").py_str(), "reused by another process");
    assert!(
        get(&seen, "verified_by")
            .py_str()
            .contains("2 times as uid")
    );
}

#[test]
fn a_launch_this_process_cannot_attribute_is_unknown() {
    let fx = fake();
    let unknown = |pairs: Vec<(&str, Value)>, reason: &str| {
        let seen = observed(&fx, pairs, None);
        assert_eq!(status_of(&seen), "unknown", "{seen:?}");
        assert!(get(&seen, "reason").py_str().contains(reason), "{seen:?}");
    };
    unknown(
        vec![("boot_id", Value::Null)],
        "the launch record names no boot",
    );
    unknown(
        vec![("host", text("elsewhere"))],
        "the launch ran on host elsewhere",
    );
    unknown(
        vec![("pid_namespace", text("pid:[1]"))],
        "the launch ran in PID namespace pid:[1], which this process cannot see into",
    );
    unknown(vec![("uid", int(uid() + 1))], "whose processes uid");
    let seen = fx.with(&no_wait, |effects| observe(None, None, effects));
    assert_eq!(status_of(&seen), "unknown");
    fs::remove_file(fx.proc_root.join("sys/kernel/random/boot_id")).expect("remove");
    unknown(vec![], "this host exposes no process table to read");
}

#[test]
fn a_reboot_ends_every_process_of_the_boot_before_it() {
    let fx = fake();
    fx.process(100, "S", 1, 100, 50, "BENCH_QUEUE_LAUNCH=m1");
    let mut pairs = launch_evidence("m1");
    pairs.push(("boot_id", text("boot-0")));
    let seen = observed(&fx, pairs, None);
    assert_eq!(status_of(&seen), "absent");
    assert!(
        get(&seen, "verified_by")
            .py_str()
            .contains("no process outlives the boot it started in")
    );
}

#[test]
fn an_uninspectable_process_of_this_user_is_unknown_and_of_another_user_is_ignored() {
    if nix::unistd::geteuid().is_root() {
        return;
    }
    let fx = fake();
    fx.process(300, "S", 1, 300, 60, "HOME=/h");
    let environ = fx.proc_root.join("300/environ");
    fs::set_permissions(
        &environ,
        std::os::unix::fs::PermissionsExt::from_mode(0o000),
    )
    .expect("chmod");
    let seen = observed(&fx, launch_evidence("m1"), None);
    assert_eq!(status_of(&seen), "unknown", "{seen:?}");
    assert!(
        get(&seen, "reason").py_str().contains(
            "could not inspect processes of this user: fake 300 (pid 300, state S, started "
        ),
        "{seen:?}"
    );
    fx.uid.set(uid() + 1);
    let seen = observed(&fx, launch_evidence("m1"), None);
    assert_eq!(status_of(&seen), "absent", "{seen:?}");
}

// --- with this host's process table --------------------------------------------------------------

#[cfg(target_os = "linux")]
#[test]
fn snapshots_are_saved_every_thirty_minutes_while_one_review_runs() {
    let fx = Fixture::new(true);
    fx.touch("hold", "");
    let naps = Cell::new(0);
    let claim = fx.work(RUN_A).join("att-001/cell.json");
    let sleep = |_: f64| {
        if claim.is_file() {
            naps.set(naps.get() + 1);
            fx.clock.set(fx.clock.get() + 600.0);
            if naps.get() >= 9 {
                fx.touch("release", "");
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let outcome = fx.with(&sleep, |effects| {
        command(
            &fx.queue,
            Action::Run {
                count: Some(1),
                detach: false,
            },
            effects,
        )
    });
    assert_eq!(code(outcome), 0, "{:?}", fx.events());
    let directory = fx.state.join("status");
    let saved: Vec<Value> = listed(&directory, "", ".json")
        .expect("snapshots")
        .iter()
        .map(|name| read_json(&directory.join(name)).expect("snapshot"))
        .collect();
    let moments: Vec<f64> = saved
        .iter()
        .map(|row| moment_of(&get(row, "at").py_str()).expect("a time"))
        .collect();
    assert!(saved.len() >= 3, "{moments:?}");
    assert!(
        moments
            .windows(2)
            .all(|pair| (pair[1] - pair[0] - SNAPSHOT_SECONDS).abs() < 1e-9),
        "{moments:?}"
    );
    let during = &saved[1];
    let claim = get(during, "unfiled_claims").as_array().expect("claims")[0].clone();
    assert_eq!(get(during, "valid").py_str(), "0");
    assert_eq!(
        get(get(&claim, "process"), "status").py_str(),
        "running",
        "{claim:?}"
    );
    assert_eq!(
        get(
            get(get(during, "spend_usd"), "claude"),
            "in_flight_reserved"
        )
        .py_str(),
        "5.0"
    );
    assert_eq!(
        get(get(get(during, "controller"), "process"), "status").py_str(),
        "running"
    );
    assert_eq!(fx.launched(), ["run-a t1/arm-a/1"]);
}

#[cfg(target_os = "linux")]
#[test]
fn a_surviving_reviewer_child_blocks_every_restart_and_is_settled_once_it_is_gone() {
    if Command::new("setsid").arg("--version").output().is_err() {
        return;
    }
    let fx = Fixture::new(true);
    fx.touch("crash", "during_review");
    fx.touch("survivor", "");
    fx.assert_blocked(fx.run(None), "run-cell exit 9 and filed no attempt");
    let survivor: i64 = fs::read_to_string(fx.base.join("survivor.pid"))
        .expect("a survivor")
        .trim()
        .parse()
        .expect("a pid");
    for _ in 0..2 {
        fx.assert_blocked(
            fx.run(None),
            "run-a/att-001 is claimed and not filed, and its processes are running",
        );
    }
    assert_eq!(fx.recover(), 1);
    let claims = get(&fx.status(), "unfiled_claims")
        .as_array()
        .expect("claims")
        .to_vec();
    let process = get(&claims[0], "process");
    assert_eq!(get(process, "status").py_str(), "running");
    assert_eq!(get(process, "wrapper").py_str(), "gone");
    let pids: Vec<String> = get(process, "processes")
        .as_array()
        .expect("rows")
        .iter()
        .map(|row| get(row, "pid").py_str())
        .collect();
    assert_eq!(pids, [survivor.to_string()]);
    let alive = stat_of(Path::new("/proc"), survivor)
        .expect("stat")
        .expect("alive");
    assert!(!alive.ended(), "the queue never signals a process");
    assert!(fx.lines("filed.log").is_empty());
    Command::new("kill")
        .args(["-9", &survivor.to_string()])
        .status()
        .expect("kill");
    for _ in 0..500 {
        if stat_of(Path::new("/proc"), survivor)
            .ok()
            .flatten()
            .is_none_or(|stat| stat.ended())
        {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    fx.assert_blocked(
        fx.run(None),
        "run-a/att-001: 'stopped: interrupted' has no diagnosis in deviations/",
    );
    assert_eq!(fx.lines("filed.log"), ["att-001"]);
}
