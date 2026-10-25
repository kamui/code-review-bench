//! `bench-tools grade dispatch` on the `grading/pending` fixture root, prepared by an installed copy of this
//! build, with a stub provisioner and fake clients. Refusals before the client run anywhere; a session needs
//! bubblewrap, git and ripgrep for the grading tools, and the Codex session the real Codex client for its local
//! probe, so those are ignored by default and CI's grading job runs them.

#[path = "support/runtime_store.rs"]
mod runtime_store;
#[path = "support/stand_in.rs"]
mod stand_in;

use bench_tools::pyjson::{self, Duplicates, Object, Value};
use bench_tools::sha256::hex_digest;
use nix::sys::signal::{Signal, kill};
use nix::unistd::Pid;
use runtime_store::{built, receipt};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output};
use std::time::{Duration, Instant};

const RUN: &str = "runs/2026-01-01-grade-test";
const TARGET: &str = "t-grade-1";
const MODEL: &str = "claude-sonnet-5";
const CODEX_MODEL: &str = "gpt-6.1-sol";

/// Makes `--out` and its `-cache` sibling, as the provisioner that `grade prepare --provision` runs would.
const PROVISIONER: &str = r#"set -eu
out=
while [ "$#" -gt 0 ]; do
    case $1 in
        --out) out=$2; shift 2 ;;
        *) shift ;;
    esac
done
mkdir -p "$(dirname "$out")"
mkdir "$out" "$out-cache"
"#;

/// `grade prepare` runs a `--provision` script as `python3 SCRIPT`; this `python3` runs the shell provisioner.
const INTERPRETER: &str = "#!/bin/sh\nexec sh \"$@\"\n";

fn repository() -> PathBuf {
    fs::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")).unwrap()
}

fn loads(text: &str) -> Value {
    pyjson::loads(text, Duplicates::KeepLast).unwrap()
}

fn read_json(path: &Path) -> Value {
    loads(&fs::read_to_string(path).unwrap())
}

fn get<'v>(value: &'v Value, key: &str) -> &'v Value {
    value
        .as_object()
        .and_then(|object| object.get(key))
        .unwrap_or_else(|| panic!("no {key} in {}", dumps(value)))
}

fn str_of<'v>(value: &'v Value, key: &str) -> &'v str {
    get(value, key).as_str().unwrap()
}

fn dumps(value: &Value) -> String {
    pyjson::dumps(value, &pyjson::Dumps::default())
}

fn number(value: &Value) -> f64 {
    match value {
        Value::Float(number) => *number,
        Value::Int(digits) => digits.parse().unwrap(),
        other => panic!("not a number: {}", dumps(other)),
    }
}

fn executable(path: &Path, body: &str) {
    fs::write(path, body).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

fn base64_decode(text: &str) -> Vec<u8> {
    let value = |byte: u8| match byte {
        b'A'..=b'Z' => byte - b'A',
        b'a'..=b'z' => byte - b'a' + 26,
        b'0'..=b'9' => byte - b'0' + 52,
        b'+' => 62,
        b'/' => 63,
        other => panic!("not base64: {other}"),
    };
    let mut bytes = Vec::new();
    let (mut buffer, mut bits) = (0u32, 0);
    for byte in text.bytes().filter(|byte| *byte != b'=') {
        buffer = (buffer << 6) | u32::from(value(byte));
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            bytes.push((buffer >> bits) as u8);
        }
    }
    bytes
}

/// The `grading/pending` fixture root of the grade corpora: its initial files and its leading setup writes.
fn fixture_files() -> Vec<(String, Vec<u8>)> {
    let corpus = read_json(&repository().join("tools/behavior_contracts/grade-prepare.v1.json"));
    let case = get(&corpus, "cases")
        .as_array()
        .unwrap()
        .iter()
        .find(|case| {
            case.as_object()
                .and_then(|case| case.get("fixture"))
                .and_then(Value::as_str)
                == Some("grading/pending")
        })
        .unwrap();
    let mut entries: Vec<&Value> = get(case, "initial").as_array().unwrap().iter().collect();
    for step in get(case, "steps").as_array().unwrap() {
        let Some(written) = step.as_object().and_then(|step| step.get("write")) else {
            break;
        };
        entries.extend(written.as_array().unwrap());
    }
    entries
        .into_iter()
        .map(|entry| {
            let path = base64_decode(str_of(entry, "path_b64"));
            (
                String::from_utf8(path).unwrap(),
                base64_decode(str_of(entry, "bytes_b64")),
            )
        })
        .collect()
}

/// A prepared batch: the fixture root, a store entry that holds this build, the prepared workspace and key, a
/// user home with sign-ins, and a `bin` directory first on `PATH` for the fake clients.
struct Bench {
    root: PathBuf,
    entry: PathBuf,
    tools: PathBuf,
    run_dir: PathBuf,
    work: PathBuf,
    key: PathBuf,
    home: PathBuf,
    bin: PathBuf,
}

impl Drop for Bench {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// Outside `/tmp`, which the grading sandbox replaces with an empty tmpfs.
fn bench(label: &str) -> Bench {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("grade-dispatch-{label}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    let root = fs::canonicalize(&root).unwrap();
    for (path, bytes) in fixture_files() {
        let path = root.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }

    let (source, sha) = built();
    let entry = root.join("store/bench-tools").join(sha);
    fs::create_dir_all(&entry).unwrap();
    let tools = entry.join("bench-tools");
    if fs::hard_link(source, &tools).is_err() {
        fs::copy(source, &tools).unwrap();
    }
    let size = fs::metadata(&tools).unwrap().len();
    fs::write(entry.join("receipt.json"), receipt(sha, size)).unwrap();

    let (work, key) = (root.join("work"), root.join("keys/key.json"));
    fs::write(root.join("provisioner.sh"), PROVISIONER).unwrap();
    fs::create_dir(root.join("interpreter")).unwrap();
    executable(&root.join("interpreter/python3"), INTERPRETER);
    prepare(&tools, &root, &work, &key);

    let (bin, home) = (root.join("bin"), root.join("userhome"));
    fs::create_dir(&bin).unwrap();
    stand_in::install(&bin, "claude");
    fs::create_dir_all(home.join(".claude")).unwrap();
    fs::write(
        home.join(".claude/.credentials.json"),
        r#"{"claudeAiOauth": {"accessToken": "secret"}, "mcpOAuth": {"unused": "secret"}}"#,
    )
    .unwrap();
    fs::write(
        home.join(".claude.json"),
        r#"{"oauthAccount": {"id": 1}, "projects": {"/elsewhere": {}}}"#,
    )
    .unwrap();
    fs::create_dir_all(home.join(".codex")).unwrap();
    fs::write(
        home.join(".codex/auth.json"),
        r#"{"auth_mode": "chatgpt", "tokens": {"access_token": "test-secret"}}"#,
    )
    .unwrap();
    let run_dir = root.join("bench").join(RUN);
    fs::write(
        run_dir.join("charges.jsonl"),
        "{\"at\": \"2026-01-01T00:00:00Z\", \"step\": \"earlier\", \"usd\": 1.0}\n",
    )
    .unwrap();
    Bench {
        root,
        entry,
        tools,
        run_dir,
        work,
        key,
        home,
        bin,
    }
}

fn prepare(tools: &Path, root: &Path, work: &Path, key: &Path) {
    let prepared = Command::new(tools)
        .args(["grade", "prepare", "--root"])
        .arg(root)
        .args(["--run", RUN, "--target", TARGET, "--work"])
        .arg(work)
        .arg("--key")
        .arg(key)
        .arg("--provision")
        .arg(root.join("provisioner.sh"))
        .current_dir(repository())
        .env(
            "PATH",
            format!(
                "{}:{}",
                root.join("interpreter").display(),
                std::env::var("PATH").unwrap()
            ),
        )
        .output()
        .unwrap();
    assert!(prepared.status.success(), "{}", output_text(&prepared));
}

/// A directory outside every repository, for a client start directory: the client probe refuses one inside a
/// repository, and this checkout's `target` is inside one.
struct Outside(PathBuf);

impl Outside {
    fn new(label: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("grade-dispatch-{label}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir(&path).unwrap();
        Outside(fs::canonicalize(path).unwrap())
    }
}

impl Drop for Outside {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn output_text(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

impl Bench {
    fn command(&self, argv: &[&str], extra: &[(&str, &str)]) -> Output {
        self.prepared(argv, extra).output().unwrap()
    }

    fn prepared(&self, argv: &[&str], extra: &[(&str, &str)]) -> Command {
        let mut command = Command::new(&self.tools);
        command.args(argv);
        self.configured(command, extra)
    }

    /// `prepared`, started by a shell that ignores `signal` first, as `nohup` or a background job does.
    fn ignoring(&self, signal: &str, argv: &[&str], extra: &[(&str, &str)]) -> Command {
        let mut command = Command::new("sh");
        command
            .args(["-c", &format!("trap '' {signal}; exec \"$@\""), "sh"])
            .arg(&self.tools)
            .args(argv);
        self.configured(command, extra)
    }

    fn configured(&self, mut command: Command, extra: &[(&str, &str)]) -> Command {
        command.current_dir(repository());
        for (name, _) in std::env::vars_os() {
            if name.to_string_lossy().starts_with("BENCH_") {
                command.env_remove(name);
            }
        }
        let path = std::env::var("PATH").unwrap_or_default();
        command
            .env("HOME", &self.home)
            .env("PATH", format!("{}:{path}", self.bin.display()));
        for (name, value) in extra {
            command.env(name, value);
        }
        command
    }

    fn dispatch_argv<'a>(&'a self, model: &'a str, budget: Option<&'a str>) -> Vec<&'a str> {
        let (work, key) = (self.work.to_str().unwrap(), self.key.to_str().unwrap());
        let run = self.run_dir.to_str().unwrap();
        let mut argv = vec![
            "grade", "dispatch", "--work", work, "--key", key, "--model", model,
        ];
        argv.extend(["--expected-cli-version", "9.9.9", "--effort", "high"]);
        if let Some(budget) = budget {
            argv.extend(["--max-budget-usd", budget]);
        }
        argv.extend(["--run", run, "--step", "grading t-grade-1"]);
        argv
    }

    fn dispatch_as(&self, model: &str, budget: Option<&str>, extra: &[(&str, &str)]) -> Output {
        self.command(&self.dispatch_argv(model, budget), extra)
    }

    fn dispatch(&self, extra: &[(&str, &str)]) -> Output {
        self.dispatch_as(MODEL, Some("5"), extra)
    }

    fn codex_argv<'a>(&'a self, extra_argv: &[&'a str], version: &'a str) -> Vec<&'a str> {
        let (work, key) = (self.work.to_str().unwrap(), self.key.to_str().unwrap());
        let run = self.run_dir.to_str().unwrap();
        let mut argv = vec!["grade", "dispatch", "--work", work, "--key", key];
        argv.extend(["--model", CODEX_MODEL, "--expected-cli-version", version]);
        argv.extend([
            "--effort",
            "high",
            "--run",
            run,
            "--step",
            "grading t-grade-1",
        ]);
        argv.extend(extra_argv);
        argv
    }

    fn codex(&self, extra_argv: &[&str], version: &str) -> Output {
        self.command(&self.codex_argv(extra_argv, version), &[])
    }

    /// Prepare the same batch again in a fresh workspace, and dispatch there from now on.
    fn prepare_again(&mut self) {
        (self.work, self.key) = (self.root.join("work-2"), self.root.join("keys/key-2.json"));
        prepare(&self.tools, &self.root, &self.work, &self.key);
    }

    /// Start a dispatch, send SIGINT to it alone once the fake client has written `pids`, and return how the
    /// dispatch ended with what the client wrote.
    fn interrupt(&self, argv: &[&str], extra: &[(&str, &str)], pids: &Path) -> (Output, Value) {
        let dispatch = in_session(self.prepared(argv, extra), pids);
        let id = Pid::from_raw(i32::try_from(dispatch.id()).unwrap());
        kill(id, Signal::SIGINT).unwrap();
        (dispatch.wait_with_output().unwrap(), read_json(pids))
    }

    /// Run `argv` until dispatch pauses after its session, send SIGINT to it, then let it finish. The client has
    /// exited and dispatch has written no record when the signal arrives.
    fn interrupt_after_session(&self, argv: &[&str], extra: &[(&str, &str)]) -> Output {
        let paused = self.root.join("paused");
        let pause = [("BENCH_PAUSE_AFTER_SESSION", paused.to_str().unwrap())];
        let dispatch = in_session(self.prepared(argv, &[extra, &pause].concat()), &paused);
        assert!(!self.work.join("dispatch.json").exists());
        kill(
            Pid::from_raw(i32::try_from(dispatch.id()).unwrap()),
            Signal::SIGINT,
        )
        .unwrap();
        fs::remove_file(&paused).unwrap();
        dispatch.wait_with_output().unwrap()
    }

    /// The interrupted dispatch died by SIGINT, stopped its client's process group, removed the copied
    /// `sign_in` and the start directory, filed nothing and charged nothing.
    fn assert_interrupted(&self, done: &Output, pids: &Value, sign_in: &str, start: Option<&Path>) {
        assert_eq!(done.status.signal(), Some(2), "{}", output_text(done));
        assert_eq!(
            dumps(get(pids, "held")),
            "[]",
            "the client inherited held signals"
        );
        for name in ["client", "child"] {
            let pid = Pid::from_raw(i32::try_from(number(get(pids, name)) as i64).unwrap());
            let started = Instant::now();
            while kill(pid, None).is_ok() {
                assert!(
                    started.elapsed() < Duration::from_secs(10),
                    "the {name} still runs"
                );
                std::thread::sleep(Duration::from_millis(50));
            }
        }
        assert!(
            !self.work.join("home").join(sign_in).exists(),
            "{sign_in} left on disk"
        );
        if let Some(start) = start {
            assert!(!start.exists(), "{} left on disk", start.display());
        }
        assert!(!self.work.join("dispatch.json").exists());
        assert_eq!(self.charges().len(), 1);
    }

    fn key_document(&self) -> Value {
        read_json(&self.key)
    }

    fn rewrite_key(&self, change: impl FnOnce(&mut Object)) {
        let Value::Object(mut key) = self.key_document() else {
            unreachable!()
        };
        change(&mut key);
        fs::set_permissions(&self.key, fs::Permissions::from_mode(0o600)).unwrap();
        fs::write(&self.key, dumps(&Value::Object(key))).unwrap();
    }

    fn seen(&self) -> Value {
        read_json(&self.work.join("tmp/stub.json"))
    }

    fn client_starts(&self) -> Vec<Value> {
        let mut starts = Vec::new();
        for log in ["claude.log", "codex.log"] {
            if let Ok(text) = fs::read_to_string(self.bin.join(log)) {
                starts.extend(text.lines().map(loads));
            }
        }
        starts
    }

    fn charges(&self) -> Vec<Value> {
        fs::read_to_string(self.run_dir.join("charges.jsonl"))
            .unwrap()
            .lines()
            .map(loads)
            .collect()
    }

    fn record(&self) -> Value {
        read_json(&self.work.join("dispatch.json"))
    }

    /// Sign-ins the refused command must not read: a read would fail and change the refusal.
    fn lock_sign_ins(&self) {
        for path in [".claude/.credentials.json", ".codex/auth.json"] {
            fs::set_permissions(self.home.join(path), fs::Permissions::from_mode(0o000)).unwrap();
        }
    }

    fn assert_refused_before_the_client(&self, done: &Output, message: &str) {
        self.assert_refused_after(done, message, &[]);
    }

    /// Refused once the client answered only `checks`: no probe, session, fresh home, record or charge.
    fn assert_refused_after(&self, done: &Output, message: &str, checks: &[&str]) {
        assert_eq!(done.status.code(), Some(1), "{}", output_text(done));
        let stdout = String::from_utf8_lossy(&done.stdout);
        assert!(stdout.contains(message), "{message:?} not in {stdout}");
        let starts: Vec<String> = self.client_starts().iter().map(dumps).collect();
        assert_eq!(starts, checks, "the client ran more than its checks");
        assert!(!self.work.join("home").exists() && !self.work.join("dispatch.json").exists());
        assert_eq!(self.charges().len(), 1);
    }
}

/// `dispatch` started, once the fake client has written `pids`.
fn in_session(mut dispatch: Command, pids: &Path) -> Child {
    let mut dispatch = dispatch
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let started = Instant::now();
    while !pids.exists() {
        if let Some(status) = dispatch.try_wait().unwrap() {
            panic!("dispatch ended before the session: {status}");
        }
        assert!(
            started.elapsed() < Duration::from_secs(120),
            "no session started"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    dispatch
}

fn refused(
    label: &str,
    change: impl FnOnce(&Bench),
    model: &str,
    budget: Option<&str>,
    message: &str,
) {
    let bench = bench(label);
    change(&bench);
    bench.lock_sign_ins();
    let done = bench.dispatch_as(model, budget, &[]);
    bench.assert_refused_before_the_client(&done, message);
}

#[test]
fn a_key_of_an_earlier_runner_identity_is_refused_before_the_client() {
    refused(
        "earlier-runner",
        |bench| {
            bench.rewrite_key(|key| {
                let Some(Value::Object(deviation)) = key.get_mut("runner_deviation") else {
                    unreachable!()
                };
                deviation.insert("version".into(), Value::Int("2".into()));
            })
        },
        MODEL,
        Some("5"),
        "prepared by runner identity v2; prepare the batch again",
    );
}

#[test]
fn a_changed_pinned_receipt_is_refused_before_the_client() {
    refused(
        "changed-receipt",
        |bench| {
            let receipt = bench.entry.join("receipt.json");
            let text = fs::read_to_string(&receipt).unwrap();
            fs::write(&receipt, text + " ").unwrap();
        },
        MODEL,
        Some("5"),
        "the key's pinned bench-tools is unavailable: ",
    );
}

/// An installed build beside the bench's own that differs from it by one byte.
fn another_build(bench: &Bench) -> PathBuf {
    let mut bytes = fs::read(&bench.tools).unwrap();
    bytes.push(b'\n');
    let sha = hex_digest(&bytes);
    let entry = bench.entry.with_file_name(&sha);
    fs::create_dir(&entry).unwrap();
    let other = entry.join("bench-tools");
    fs::write(&other, &bytes).unwrap();
    fs::set_permissions(&other, fs::Permissions::from_mode(0o755)).unwrap();
    fs::write(
        entry.join("receipt.json"),
        receipt(&sha, bytes.len() as u64),
    )
    .unwrap();
    other
}

#[test]
fn a_batch_prepared_by_another_runner_is_refused_before_the_client() {
    let bench = bench("other-runner");
    let other = another_build(&bench);
    bench.lock_sign_ins();
    let (work, key) = (bench.work.to_str().unwrap(), bench.key.to_str().unwrap());
    let done = Command::new(&other)
        .args([
            "grade", "dispatch", "--work", work, "--key", key, "--model", MODEL,
        ])
        .args([
            "--expected-cli-version",
            "9.9.9",
            "--effort",
            "high",
            "--max-budget-usd",
            "5",
        ])
        .current_dir(repository())
        .env("HOME", &bench.home)
        .env(
            "PATH",
            format!("{}:{}", bench.bin.display(), std::env::var("PATH").unwrap()),
        )
        .output()
        .unwrap();
    bench.assert_refused_before_the_client(&done, "is not the pinned executable");
}

#[test]
fn changed_inputs_are_refused_before_the_client() {
    refused(
        "changed-inputs",
        |bench| {
            bench.rewrite_key(|key| {
                key.insert("input_fingerprint".into(), Value::Str("0".repeat(64)));
            })
        },
        MODEL,
        Some("5"),
        "prepare it again",
    );
}

#[test]
fn a_key_that_names_no_current_record_is_refused() {
    refused(
        "no-current-record",
        |bench| {
            bench.rewrite_key(|key| {
                let kept: Object = key
                    .iter()
                    .filter(|(name, _)| name.as_str() != "current_record")
                    .map(|(name, value)| (name.clone(), value.clone()))
                    .collect();
                *key = kept;
            })
        },
        MODEL,
        Some("5"),
        "the key names no current record",
    );
}

#[test]
fn a_model_without_rates_is_not_priced() {
    refused(
        "unpriced",
        |_| {},
        "claude-unpriced-1",
        Some("5"),
        "no rates.json entry for claude-unpriced-1; dispatch refused before payment",
    );
}

#[test]
fn a_claude_dispatch_needs_a_budget() {
    refused(
        "no-budget",
        |_| {},
        MODEL,
        None,
        "Claude dispatch requires --max-budget-usd",
    );
}

#[test]
fn codex_cannot_take_a_dollar_limit() {
    refused(
        "codex-budget",
        |_| {},
        CODEX_MODEL,
        Some("5"),
        "Codex cannot enforce --max-budget-usd",
    );
}

#[test]
fn codex_requires_explicit_unbounded_authorization() {
    let bench = bench("codex-unbounded");
    bench.lock_sign_ins();
    let done = bench.codex(&[], "0.160.0");
    bench.assert_refused_before_the_client(&done, "requires --allow-unbounded-codex");
}

#[test]
fn codex_refuses_models_without_a_pinned_tool_profile() {
    let bench = bench("codex-unknown");
    bench.lock_sign_ins();
    let (work, key) = (bench.work.to_str().unwrap(), bench.key.to_str().unwrap());
    let done = bench.command(
        &[
            "grade",
            "dispatch",
            "--work",
            work,
            "--key",
            key,
            "--model",
            "gpt-unknown",
            "--expected-cli-version",
            "0.160.0",
            "--effort",
            "high",
            "--allow-unbounded-codex",
        ],
        &[],
    );
    bench.assert_refused_before_the_client(&done, "no pinned grading tool profile");
}

/// `argv` with its `--effort` value replaced by `effort`.
fn with_effort<'a>(mut argv: Vec<&'a str>, effort: &'a str) -> Vec<&'a str> {
    let at = argv.iter().position(|arg| *arg == "--effort").unwrap() + 1;
    argv[at] = effort;
    argv
}

#[test]
fn an_effort_the_claude_client_does_not_offer_is_refused_before_the_session() {
    let bench = bench("claude-effort");
    let done = bench.command(
        &with_effort(bench.dispatch_argv(MODEL, Some("5")), "ultra"),
        &[],
    );
    bench.assert_refused_after(
        &done,
        &format!(
            "effort 'ultra' is not supported for {MODEL}; supported: low, medium, high, xhigh, max; \
             dispatch refused before payment"
        ),
        &[r#"["--version"]"#, r#"["--help"]"#],
    );
}

#[test]
fn an_effort_the_codex_catalog_does_not_offer_the_model_is_refused_before_the_session() {
    let bench = bench("codex-effort");
    codex_stand_in(&bench, &[("mode", "version")]);
    let mut argv = with_effort(
        bench.codex_argv(&["--allow-unbounded-codex"], "0.160.0"),
        "ultra",
    );
    let at = argv.iter().position(|arg| *arg == CODEX_MODEL).unwrap();
    argv[at] = "gpt-6-luna";
    let done = bench.command(&argv, &[]);
    bench.assert_refused_after(
        &done,
        "effort 'ultra' is not supported for gpt-6-luna; supported: low, medium, high, xhigh, max; \
         dispatch refused before payment",
        &[r#"["--version"]"#],
    );
}

#[test]
#[ignore = "needs bubblewrap, git and ripgrep"]
fn a_client_start_directory_that_names_a_graded_identity_is_refused() {
    let bench = bench("start-directory");
    for marker in ["2026-01-01-grade-test", "codex-high", "att-003"] {
        let temporary = Outside::new("start-directory");
        let parent = temporary.0.join(marker);
        fs::create_dir(&parent).unwrap();
        let done = bench.dispatch(&[("TMPDIR", parent.to_str().unwrap())]);
        assert_eq!(done.status.code(), Some(1), "{}", output_text(&done));
        let stdout = String::from_utf8_lossy(&done.stdout);
        assert!(
            stdout.contains(&format!("client start directory names '{marker}'")),
            "{stdout}"
        );
        assert_eq!(fs::read_dir(&parent).unwrap().count(), 0);
        assert!(!bench.work.join("home").exists());
    }
}

#[test]
#[ignore = "needs bubblewrap, git and ripgrep"]
fn a_clean_session_starts_outside_the_workspace_records_and_charges() {
    let bench = bench("clean");
    let temporary = Outside::new("clean");
    let outside = temporary.0.clone();
    let done = bench.dispatch(&[("TMPDIR", outside.to_str().unwrap())]);
    assert_eq!(done.status.code(), Some(0), "{}", output_text(&done));
    assert!(
        String::from_utf8_lossy(&done.stdout)
            .starts_with(&format!("graded in {}: session ", bench.work.display()))
    );

    let seen = bench.seen();
    let start = PathBuf::from(str_of(&seen, "cwd"));
    assert_eq!(start.parent(), Some(outside.as_path()));
    assert!(!start.exists());
    assert_eq!(dumps(&bench.client_starts()[0]), r#"["--version"]"#);
    assert!(matches!(get(&seen, "credentials_seen"), Value::Bool(true)));
    assert!(matches!(
        get(
            &read_json(&bench.work.join("clean-context.json")),
            "fresh_home"
        ),
        Value::Bool(true)
    ));
    assert!(!bench.work.join("home/.claude/.credentials.json").exists());
    assert_eq!(
        dumps(&read_json(&bench.work.join("home/.claude.json"))),
        r#"{"oauthAccount": {"id": 1}, "hasCompletedOnboarding": true}"#
    );
    let argv: Vec<&str> = get(&seen, "argv")
        .as_array()
        .unwrap()
        .iter()
        .map(|part| part.as_str().unwrap())
        .collect();
    let after = |flag: &str| argv[argv.iter().position(|part| *part == flag).unwrap() + 1];
    assert!(argv.contains(&"--restricted") && argv.contains(&"--strict-mcp-config"));
    assert_eq!(after("--tools"), "");
    assert!(!argv.contains(&"Bash") && !argv.contains(&"Write"));
    let prompt = fs::read(bench.work.join("prompt.md")).unwrap();
    assert_eq!(str_of(&seen, "prompt").as_bytes(), prompt);
    assert_eq!(str_of(&seen, "wait_ceiling"), "0");
    assert_eq!(
        PathBuf::from(str_of(&seen, "config_directory")),
        bench.work.join("home/.claude")
    );
    assert_eq!(
        dumps(get(&seen, "credential_sections")),
        r#"["claudeAiOauth"]"#
    );
    assert_eq!(dumps(get(&seen, "credential_mode")), "384");

    let record = bench.record();
    assert_eq!(str_of(&record, "session_id"), after("--session-id"));
    assert_eq!(str_of(&record, "cli_version"), "9.9.9");
    assert_eq!(str_of(&record, "model"), MODEL);
    assert_eq!(
        dumps(get(&record, "models_observed")),
        format!(r#"["{MODEL}"]"#)
    );
    assert_eq!(dumps(get(&record, "subagents")), "0");
    assert_eq!(dumps(get(&record, "exit_code")), "0");
    assert_eq!(dumps(get(&record, "audit_violations")), "[]");
    assert!(matches!(
        get(&record, "verdicts_present"),
        Value::Bool(true)
    ));
    assert_eq!(str_of(&record, "prompt_sha256"), hex_digest(&prompt));
    let priced = number(get(get(&record, "usage"), "priced_total_usd"));
    let expected = (10.0 * 2.0 + 100.0 * 2.5 + 1000.0 * 0.2 + 50.0 * 10.0) / 1e6;
    assert!((priced - expected).abs() < 1e-9, "{priced}");
    let timing = read_json(&bench.work.join("timing.json"));
    assert_eq!(
        str_of(&timing, "root_dispatched_at"),
        str_of(&record, "dispatched_at")
    );

    let charges = bench.charges();
    assert_eq!(charges.len(), 2);
    let session = str_of(&record, "session_id");
    assert_eq!(
        dumps(&charges[1]),
        format!(
            r#"{{"at": "{}", "step": "grading t-grade-1", "usd": {}, "model": "{MODEL}", "billing": "api-dollars", "session": "{}"}}"#,
            str_of(&record, "completed_at"),
            dumps(get(get(&record, "usage"), "priced_total_usd")),
            &session[..8]
        )
    );

    let again = bench.dispatch(&[]);
    assert_eq!(again.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&again.stdout).contains("already dispatched"));
    assert_eq!(bench.charges().len(), 2);
}

#[test]
#[ignore = "needs bubblewrap, git and ripgrep"]
fn the_validate_tool_runs_the_pinned_executable_mounted_outside_the_workspace() {
    let bench = bench("validate");
    let done = bench.dispatch(&[("STUB_VERDICTS", r#"{"reviews": {}}"#)]);
    assert_eq!(done.status.code(), Some(0), "{}", output_text(&done));
    let direct = Command::new(&bench.tools)
        .args(["grading", "validate"])
        .arg(bench.work.join("verdicts.json"))
        .arg("--inputs")
        .arg(bench.work.join("validator/inputs.json"))
        .output()
        .unwrap();
    assert_eq!(direct.status.code(), Some(1));
    let seen = bench.seen();
    let content = get(get(&seen, "validated"), "content").as_array().unwrap();
    let validated = loads(str_of(&content[0], "text"));
    assert_eq!(dumps(get(&validated, "exit_code")), "1");
    assert_eq!(
        str_of(&validated, "violations"),
        String::from_utf8_lossy(&direct.stdout)
    );
    assert_eq!(str_of(&validated, "error"), "");

    let key = bench.key_document();
    let pin = get(get(&key, "runner_deviation"), "bench_tools");
    assert_eq!(str_of(pin, "sha256"), built().1);
    let mut stack = vec![bench.work.clone()];
    while let Some(directory) = stack.pop() {
        for entry in fs::read_dir(&directory).unwrap() {
            let entry = entry.unwrap();
            let kind = entry.file_type().unwrap();
            if kind.is_dir() {
                stack.push(entry.path());
            } else if kind.is_file() {
                let bytes = fs::read(entry.path()).unwrap();
                assert_ne!(hex_digest(&bytes), built().1, "{}", entry.path().display());
            }
        }
    }
    let validator: Vec<String> = fs::read_dir(bench.work.join("validator"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(validator, ["inputs.json"]);
}

#[test]
#[ignore = "needs bubblewrap, git and ripgrep"]
fn a_read_outside_work_fails_the_dispatch() {
    let bench = bench("read-outside");
    let outside = bench.key.clone();
    let done = bench.dispatch(&[("STUB_READ", outside.to_str().unwrap())]);
    assert_eq!(done.status.code(), Some(1), "{}", output_text(&done));
    let violation = format!(
        "file tool read outside allowed roots: {}",
        outside.display()
    );
    let stdout = String::from_utf8_lossy(&done.stdout);
    assert!(
        stdout
            .lines()
            .any(|line| line == format!("read audit: {violation}")),
        "{stdout}"
    );
    assert!(!bench.work.join("home/.claude/.credentials.json").exists());
    assert_eq!(
        dumps(get(&bench.record(), "audit_violations")),
        dumps(&Value::Array(vec![Value::Str(violation)]))
    );
}

#[test]
#[ignore = "needs bubblewrap, git and ripgrep"]
fn another_model_fails_the_dispatch() {
    let bench = bench("another-model");
    let done = bench.dispatch(&[("STUB_MODEL", "claude-opus-5-5")]);
    assert_eq!(done.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&done.stdout).contains(&format!(
        "models observed claude-opus-5-5, expected {MODEL}"
    )));
}

/// A bench whose `codex` is the stub in `mode`, and the real client's version.
fn codex_bench(label: &str, mode: &str) -> (Bench, String) {
    let bench = bench(label);
    let real = bench_tools::review_isolation::which("codex").expect("the Codex client");
    let version = Command::new(&real).arg("--version").output().unwrap();
    let version = String::from_utf8_lossy(&version.stdout)
        .split_whitespace()
        .last()
        .unwrap()
        .to_string();
    codex_stub(&bench, mode);
    (bench, version)
}

fn codex_stub(bench: &Bench, mode: &str) {
    let real = bench_tools::review_isolation::which("codex").expect("the Codex client");
    codex_stand_in(bench, &[("real", real.to_str().unwrap()), ("mode", mode)]);
}

/// The stand-in installed as `codex`, with the settings it reads from `codex.json` beside it.
fn codex_stand_in(bench: &Bench, settings: &[(&str, &str)]) {
    let settings = settings
        .iter()
        .map(|(name, value)| (name.to_string(), Value::Str(value.to_string())))
        .collect();
    fs::write(
        bench.bin.join("codex.json"),
        dumps(&Value::Object(settings)),
    )
    .unwrap();
    stand_in::install(&bench.bin, "codex");
}

#[test]
#[ignore = "needs bubblewrap, git, ripgrep and the Codex client"]
fn a_codex_session_runs_confined_and_charges_list_price() {
    let (bench, version) = codex_bench("codex-clean", "clean");
    let done = bench.codex(&["--allow-unbounded-codex"], &version);
    assert_eq!(done.status.code(), Some(0), "{}", output_text(&done));
    let seen = bench.seen();
    assert_eq!(
        dumps(get(&seen, "environment")),
        r#"["CODEX_HOME", "HOME", "LANG", "PATH", "TMPDIR"]"#
    );
    assert_eq!(dumps(get(&seen, "auth_mode")), "384");
    assert!(str_of(&seen, "auth").contains("test-secret"));
    let argv = dumps(get(&seen, "argv"));
    assert!(!argv.contains("resume"), "{argv}");
    let prompt = fs::read(bench.work.join("prompt.md")).unwrap();
    assert_eq!(str_of(&seen, "prompt").as_bytes(), prompt);
    assert!(!bench.work.join("home/.codex/auth.json").exists());
    assert!(matches!(
        get(
            &read_json(&bench.work.join("clean-context.json")),
            "fresh_home"
        ),
        Value::Bool(true)
    ));
    let record = bench.record();
    assert_eq!(str_of(&record, "session_id"), "fresh-session");
    assert!(matches!(
        get(&record, "provider_call_possible"),
        Value::Bool(true)
    ));
    assert_eq!(str_of(&record, "budget_policy"), "codex-unbounded");
    assert_eq!(dumps(get(&record, "audit_violations")), "[]");
    let charges = bench.charges();
    assert_eq!(charges.len(), 2);
    assert_eq!(str_of(&charges[1], "billing"), "list-price-equivalent");
    assert_eq!(str_of(&charges[1], "session"), "fresh-session");
}

#[test]
#[ignore = "needs bubblewrap, git, ripgrep and the Codex client"]
fn a_failed_or_interrupted_codex_session_removes_the_sign_in_and_proves_no_charge() {
    for mode in ["fail", "hang"] {
        let (bench, version) = codex_bench(&format!("codex-{mode}"), mode);
        let done = bench.codex(&["--allow-unbounded-codex", "--timeout", "5"], &version);
        assert_eq!(done.status.code(), Some(1), "{}", output_text(&done));
        let exit = if mode == "fail" { "3" } else { "None" };
        assert!(
            String::from_utf8_lossy(&done.stdout)
                .contains(&format!("Codex session exit {exit}; timeout limit 5 s")),
            "{}",
            output_text(&done)
        );
        assert!(!bench.work.join("home/.codex/auth.json").exists());
        let record = bench.record();
        assert!(matches!(get(get(&record, "usage"), "high"), Value::Null));
        assert!(matches!(
            get(&record, "provider_call_possible"),
            Value::Bool(true)
        ));
        assert_eq!(bench.charges().len(), 1);
    }
}

#[test]
#[ignore = "needs bubblewrap, git and ripgrep"]
fn an_interrupted_claude_session_is_stopped_and_its_sign_in_removed() {
    let mut bench = bench("claude-interrupt");
    let temporary = Outside::new("claude-interrupt");
    let tmpdir = temporary.0.to_str().unwrap();
    let pids = bench.bin.join("claude.pids");
    let argv = bench.dispatch_argv(MODEL, Some("5"));
    let (done, seen) = bench.interrupt(&argv, &[("TMPDIR", tmpdir), ("STUB_HANG", "1")], &pids);
    let start = PathBuf::from(str_of(&seen, "cwd"));
    assert_eq!(start.parent(), Some(temporary.0.as_path()));
    bench.assert_interrupted(&done, &seen, ".claude/.credentials.json", Some(&start));

    bench.prepare_again();
    let again = bench.dispatch(&[("TMPDIR", tmpdir)]);
    assert_eq!(again.status.code(), Some(0), "{}", output_text(&again));
    assert_eq!(bench.charges().len(), 2);
}

#[test]
#[ignore = "needs bubblewrap, git, ripgrep and the Codex client"]
fn an_interrupted_codex_session_is_stopped_and_its_sign_in_removed() {
    let (mut bench, version) = codex_bench("codex-interrupt", "interrupt");
    let pids = bench.bin.join("codex.pids");
    let argv = bench.codex_argv(&["--allow-unbounded-codex"], &version);
    let (done, seen) = bench.interrupt(&argv, &[], &pids);
    bench.assert_interrupted(&done, &seen, ".codex/auth.json", None);

    codex_stub(&bench, "clean");
    bench.prepare_again();
    let again = bench.codex(&["--allow-unbounded-codex"], &version);
    assert_eq!(again.status.code(), Some(0), "{}", output_text(&again));
    assert_eq!(bench.charges().len(), 2);
}

/// Under `trap '' NAME`, `signal` sent to the dispatch mid-session leaves the session to finish, recorded and
/// charged, and the client inherits the ignored signal.
fn an_ignored_signal_leaves_the_session_to_finish(signal: Signal, name: &str) {
    let bench = bench(&format!("claude-ignored-{name}"));
    let temporary = Outside::new(&format!("claude-ignored-{name}"));
    let tmpdir = temporary.0.to_str().unwrap();
    let pids = bench.bin.join("claude.pids");
    let argv = bench.dispatch_argv(MODEL, Some("5"));
    let extra = [("TMPDIR", tmpdir), ("STUB_PAUSE", "1")];
    let dispatch = in_session(bench.ignoring(name, &argv, &extra), &pids);
    kill(Pid::from_raw(i32::try_from(dispatch.id()).unwrap()), signal).unwrap();
    std::thread::sleep(Duration::from_millis(500));
    fs::write(bench.bin.join("resume"), "").unwrap();
    let done = dispatch.wait_with_output().unwrap();
    assert_eq!(done.status.code(), Some(0), "{}", output_text(&done));
    assert_eq!(
        dumps(get(&read_json(&pids), "ignored")),
        format!("[{}]", signal as i32)
    );
    assert!(bench.work.join("dispatch.json").is_file());
    assert_eq!(bench.charges().len(), 2);
    assert!(!bench.work.join("home/.claude/.credentials.json").exists());
}

#[test]
#[ignore = "needs bubblewrap, git and ripgrep"]
fn a_hangup_ignored_by_nohup_leaves_the_session_to_finish() {
    an_ignored_signal_leaves_the_session_to_finish(Signal::SIGHUP, "HUP");
}

#[test]
#[ignore = "needs bubblewrap, git and ripgrep"]
fn an_interrupt_ignored_by_a_background_job_leaves_the_session_to_finish() {
    an_ignored_signal_leaves_the_session_to_finish(Signal::SIGINT, "INT");
}

#[test]
#[ignore = "needs bubblewrap, git and ripgrep"]
fn an_interrupt_just_after_a_claude_session_waits_for_its_record_and_charge() {
    let bench = bench("claude-after");
    let temporary = Outside::new("claude-after");
    let tmpdir = temporary.0.to_str().unwrap();
    let argv = bench.dispatch_argv(MODEL, Some("5"));
    let done = bench.interrupt_after_session(&argv, &[("TMPDIR", tmpdir)]);
    assert_eq!(done.status.signal(), Some(2), "{}", output_text(&done));
    assert!(bench.work.join("dispatch.json").is_file());
    assert_eq!(bench.charges().len(), 2);
    assert!(!bench.work.join("home/.claude/.credentials.json").exists());
    assert!(!PathBuf::from(str_of(&bench.seen(), "cwd")).exists());
}

#[test]
#[ignore = "needs bubblewrap, git, ripgrep and the Codex client"]
fn an_interrupt_just_after_a_codex_session_waits_for_its_record_and_charge() {
    let (bench, version) = codex_bench("codex-after", "clean");
    let argv = bench.codex_argv(&["--allow-unbounded-codex"], &version);
    let done = bench.interrupt_after_session(&argv, &[]);
    assert_eq!(done.status.signal(), Some(2), "{}", output_text(&done));
    assert!(bench.work.join("dispatch.json").is_file());
    assert_eq!(bench.charges().len(), 2);
    assert!(!bench.work.join("home/.codex/auth.json").exists());
}

/// Hand-written verdicts for the prepared batch, as the grade-map corpus writes them, and their assessor. With
/// `blocked`, the first review's second claim reports a check the run tool refused.
fn write_verdicts(key: &Path, work: &Path, assessor: &Path, blocked: bool) {
    let key = read_json(key);
    let token = |attempt: &str| {
        let reviews = get(&key, "reviews").as_array().unwrap();
        let review = reviews
            .iter()
            .rfind(|review| str_of(review, "attempt_id") == attempt)
            .unwrap();
        str_of(review, "token").to_string()
    };
    let claim = |identifier: &str, quote: &str| {
        format!(
            r#"{{"id": "{identifier}", "quote": "{quote}", "true": "yes", "this_change": "yes", "promised": "no",
                "promise_source": [], "delivered": null, "outcome": "suggestion", "kind": "improvement",
                "known_problems": [], "canonical_claim_id": null, "duplicate_group": null, "candidate": null,
                "open": null, "notes": "Checked against the source.", "evidence": ["Read main.go."],
                "pre_existing_problems": [], "pre_existing_candidate": null}}"#
        )
    };
    let reviewed = |items: &[Vec<String>], recommendations: &str| {
        let items: Vec<String> = items
            .iter()
            .enumerate()
            .map(|(index, claims)| {
                let (kind, note) = if claims.is_empty() {
                    ("not-a-finding", "This item makes no claim.")
                } else {
                    ("finding", "")
                };
                format!(
                    r#""{}": {{"kind": "{kind}", "note": "{note}", "claims": [{}]}}"#,
                    index + 1,
                    claims.join(", ")
                )
            })
            .collect();
        format!(
            r#"{{"items": {{{}}}, "recommendations": [{recommendations}],
                "remedy_inventory": {{"state": "complete", "reason": ""}}}}"#,
            items.join(", ")
        )
    };
    let remedy = r#"{"id": "r1", "anchors": [{"item": 1, "quote": "Hold the lock while closing."}],
        "addressed_claims": ["c1"], "sufficiency": [],
        "safety": {"state": "unassessed", "reason": "Checked the changed path for new harm.", "evidence": []}}"#;
    let second = if blocked {
        r#"{"id": "c2", "quote": "It breaks.", "true": "cannot-check", "this_change": null, "promised": null,
            "promise_source": [], "delivered": null, "outcome": "unresolved", "kind": null,
            "known_problems": [], "canonical_claim_id": null, "duplicate_group": null, "candidate": null,
            "open": {"kind": "blocked-check", "command": "go run ./cmd/import-boss",
                     "would_settle": "Whether any file still imports the package."},
            "notes": "The import check was refused.", "evidence": ["Read main.go."],
            "pre_existing_problems": [], "pre_existing_candidate": null}"#
            .to_string()
    } else {
        claim("c2", "It breaks.")
    };
    let first = reviewed(
        &[
            vec![claim("c1", "Races on close"), second],
            vec![],
            vec![claim("c3", "Lock order is new")],
        ],
        remedy,
    );
    let verdicts = format!(
        r#"{{"reviews": {{"{}": {first}, "{}": {}, "{}": {}}}, "new_candidates": [], "link_disputes": []}}"#,
        token("att-001"),
        token("att-002"),
        reviewed(&[vec![claim("c1", "Leaks the conn")]], ""),
        token("att-003"),
        reviewed(&[], ""),
    );
    let indented = |text: &str| {
        let options = pyjson::Dumps {
            indent: Some(2),
            ..Default::default()
        };
        pyjson::dumps(&loads(text), &options)
    };
    fs::write(work.join("verdicts.json"), indented(&verdicts)).unwrap();
    fs::write(
        assessor,
        indented(
            r#"{"assessor": "fixture assessor", "method": "Hand-written synthetic verdicts",
                "completed_at": "2026-01-02T00:10:00Z"}"#,
        ),
    )
    .unwrap();
}

#[test]
fn a_batch_maps_only_with_the_build_that_prepared_it() {
    let bench = bench("map-pin");
    fs::remove_dir_all(bench.work.join("clone")).unwrap();
    let assessor = bench.root.join("assessor.json");
    write_verdicts(&bench.key, &bench.work, &assessor, false);
    let grades = bench.root.join("bench/grading/current/grades.json");
    let before = fs::read(&grades).unwrap();
    let map = |tools: &Path| {
        Command::new(tools)
            .args(["grade", "map", "--root"])
            .arg(&bench.root)
            .arg("--work")
            .arg(&bench.work)
            .arg("--key")
            .arg(&bench.key)
            .arg("--assessor")
            .arg(&assessor)
            .current_dir(repository())
            .env("HOME", &bench.home)
            .output()
            .unwrap()
    };

    let refused = map(&another_build(&bench));
    assert_eq!(refused.status.code(), Some(1), "{}", output_text(&refused));
    assert!(
        output_text(&refused).contains("is not the pinned executable"),
        "{}",
        output_text(&refused)
    );
    assert_eq!(fs::read(&grades).unwrap(), before);

    let mapped = map(&bench.tools);
    assert_eq!(mapped.status.code(), Some(0), "{}", output_text(&mapped));
    assert!(
        String::from_utf8_lossy(&mapped.stdout)
            .starts_with(&format!("replaced {RUN}/{TARGET} in ")),
        "{}",
        output_text(&mapped)
    );
}

#[test]
fn a_batch_with_a_blocked_check_is_not_filed() {
    let bench = bench("map-blocked");
    fs::remove_dir_all(bench.work.join("clone")).unwrap();
    let assessor = bench.root.join("assessor.json");
    let grades = bench.root.join("bench/grading/current/grades.json");
    let before = fs::read(&grades).unwrap();
    let map = || {
        Command::new(&bench.tools)
            .args(["grade", "map", "--root"])
            .arg(&bench.root)
            .arg("--work")
            .arg(&bench.work)
            .arg("--key")
            .arg(&bench.key)
            .arg("--assessor")
            .arg(&assessor)
            .current_dir(repository())
            .env("HOME", &bench.home)
            .output()
            .unwrap()
    };

    write_verdicts(&bench.key, &bench.work, &assessor, true);
    let refused = map();
    assert_eq!(refused.status.code(), Some(1), "{}", output_text(&refused));
    assert!(
        output_text(&refused).contains(
            " item 1 claim c2: the run tool refused go run ./cmd/import-boss, a check needed to settle \
             \"Whether any file still imports the package.\". Add the access and grade the batch again, or \
             record the user's ruling on the claim in the current claims and prepare the batch again"
        ),
        "{}",
        output_text(&refused)
    );
    assert_eq!(fs::read(&grades).unwrap(), before);

    write_verdicts(&bench.key, &bench.work, &assessor, false);
    let mapped = map();
    assert_eq!(mapped.status.code(), Some(0), "{}", output_text(&mapped));
}
