//! One prepared batch's grader session, and the client half of a queue's preflight.
//!
//! `dispatch` runs one grader session in the workspace with the private key and the pinned client version,
//! native tools off and only the six confined grading tools allowed. Before it reads a credential or starts the
//! client it checks the workspace and the batch's inputs against the key (`check_prepared` with dispatching, then
//! the current record), the client, its credentials and pricing, the sandbox and the client's tool enforcement.
//! The grading tools run this executable, which must be the one the key pins. The session leaves `dispatch.json`
//! with the session, observed models, access audit and priced usage, and with a run directory a charge line in
//! that run's `charges.jsonl`. It succeeds only for a clean, priced, single-model session that left `verdicts.json`.

use super::{Options, Runner, check_current_record, check_prepared, client, preflight_inputs};
use crate::current_grading::{self as current, Failure, R};
use crate::grading_tools::{self, Host, Refusal, Tools};
use crate::pyjson::{self, Dumps, Object, Value};
use crate::pytext::{self, PyPath};
use crate::{attempt_audit, clean_context, interrupt, review_isolation, sha256, usage};
use nix::sys::signal::{Signal, killpg};
use nix::unistd::Pid;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const ALLOWED_TOOLS: [&str; 6] = [
    "mcp__grading__inspect",
    "mcp__grading__run",
    "mcp__grading__write_verdicts",
    "mcp__grading__edit_verdicts",
    "mcp__grading__write_scratch",
    "mcp__grading__validate",
];
const PROBE_FAILED: &str =
    "pinned client enforcement probe failed; dispatch refused before payment";
const LOGS: [&str; 2] = ["command-audit.jsonl", "policy-audit.jsonl"];

/// The grader client a dispatch or preflight checks.
pub struct Client {
    pub model: String,
    pub expected_cli_version: String,
    pub allow_unbounded_codex: bool,
}

/// `bench-tools grade dispatch`'s options.
pub struct Dispatch {
    pub work: PathBuf,
    pub key: PathBuf,
    pub client: Client,
    pub effort: String,
    pub max_budget_usd: Option<f64>,
    /// The run directory whose `charges.jsonl` gets the charge, with its step label.
    pub charge: Option<(PathBuf, String)>,
    pub timeout: u64,
}

fn inconsistent<T>(message: impl Into<String>) -> R<T> {
    Err(Failure::Inconsistent(message.into().into_bytes()))
}

fn input(message: String) -> Failure {
    Failure::Input(message)
}

fn text(value: &str) -> Value {
    Value::Str(value.to_string())
}

fn path_text(path: &Path) -> Value {
    Value::Str(path.to_string_lossy().into_owned())
}

fn strings<'a>(items: impl IntoIterator<Item = &'a str>) -> Value {
    Value::Array(items.into_iter().map(text).collect())
}

fn object(entries: Vec<(&str, Value)>) -> Value {
    Value::Object(
        entries
            .into_iter()
            .map(|(key, value)| (key.to_string(), value))
            .collect(),
    )
}

fn insert(value: &mut Value, key: &str, entry: Value) {
    if let Value::Object(entries) = value {
        entries.insert(key.to_string(), entry);
    }
}

fn get<'v>(value: &'v Value, key: &str) -> Option<&'v Value> {
    value.as_object().and_then(|entries| entries.get(key))
}

fn json(value: &Value) -> String {
    pyjson::dumps(value, &Dumps::default())
}

fn indented(value: &Value) -> String {
    pyjson::dumps(
        value,
        &Dumps {
            indent: Some(2),
            ..Dumps::default()
        },
    )
}

fn shown(error: &std::io::Error, path: &Path) -> String {
    pytext::os_error(error, pytext::os_bytes(path.as_os_str()))
}

fn read_bytes(path: &Path) -> R<Vec<u8>> {
    std::fs::read(path).map_err(|error| {
        input(format!(
            "cannot read {}: {}",
            path.display(),
            shown(&error, path)
        ))
    })
}

fn write_file(path: &Path, contents: impl AsRef<[u8]>) -> R<()> {
    std::fs::write(path, contents).map_err(|error| input(shown(&error, path)))
}

/// A file only this process creates, readable by its owner alone.
fn exclusive(path: &Path, mode: u32) -> std::io::Result<File> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(mode)
        .open(path)
}

fn first(text: &str, limit: usize) -> String {
    text.trim().chars().take(limit).collect()
}

fn version_in(stdout: &[u8]) -> Option<String> {
    regex::Regex::new(r"\d+\.\d+\.\d+")
        .ok()?
        .find(&String::from_utf8_lossy(stdout))
        .map(|found| found.as_str().to_string())
}

fn now() -> String {
    crate::normalize_review::utc_now()
}

fn already_dispatched(work: &Path) -> R<()> {
    if work.join("home").exists() || work.join("dispatch.json").exists() {
        return inconsistent(format!(
            "{} was already dispatched; prepare a new directory",
            work.display()
        ));
    }
    Ok(())
}

/// The key, once the batch passes every check its paid session relies on.
fn checked_batch(runner: &Runner, work: &Path, key_path: &Path) -> R<Value> {
    if !work.join("command-policy.json").is_file() || !work.join("validator/inputs.json").is_file()
    {
        return inconsistent(
            "legacy workspace has no enforcement/validator; prepare a fresh versioned workspace",
        );
    }
    let key = current::read_json(&PyPath::new(pytext::os_bytes(key_path.as_os_str())))?;
    let problems = check_prepared(
        &runner.checkout,
        &PyPath::new(pytext::os_bytes(work.as_os_str())),
        &key,
        true,
        &runner.executable,
    )?;
    if !problems.is_empty() {
        return inconsistent(problems.join("\n"));
    }
    check_current_record(&key)?;
    Ok(key)
}

/// The problems the dispatching check finds once the session is over; a check that cannot run is one of them.
fn problems_after(runner: &Runner, work: &Path, key: &Value) -> Vec<String> {
    match check_prepared(
        &runner.checkout,
        &PyPath::new(pytext::os_bytes(work.as_os_str())),
        key,
        true,
        &runner.executable,
    ) {
        Ok(problems) => problems,
        Err(Failure::Inconsistent(message)) => String::from_utf8_lossy(&message)
            .lines()
            .map(str::to_string)
            .collect(),
        Err(Failure::Input(message) | Failure::Crash(message)) => vec![format!(
            "bench-tools grade check-prepared failed (exit 2): {}",
            first(&message, 300)
        )],
    }
}

fn rate_for(runner: &Runner, model: &str) -> R<Option<Value>> {
    client::rate_for(&runner.checkout.to_path_buf(), model).map_err(input)
}

/// Check the client, its sign-in and its price entry before anything is paid for, returning its version.
fn client_preflight(runner: &Runner, grader: &Client) -> R<String> {
    let (model, expected) = (grader.model.as_str(), grader.expected_cli_version.as_str());
    let home = client::user_home();
    if model.starts_with("gpt-") {
        if !grader.allow_unbounded_codex {
            return inconsistent(
                "Codex grading requires --allow-unbounded-codex; the client has no verified per-session dollar \
                 limit and cannot use a bounded reservation",
            );
        }
        let catalog = current::read_json(&runner.checkout.join(client::CATALOG.as_bytes()))?;
        let supported = get(&catalog, "models")
            .and_then(Value::as_array)
            .unwrap_or_default()
            .iter()
            .any(|entry| get(entry, "slug").and_then(Value::as_str) == Some(model));
        if !supported {
            return inconsistent(format!(
                "Codex model {model} has no pinned grading tool profile"
            ));
        }
        if rate_for(runner, model)?.is_none() {
            return inconsistent(format!(
                "no price entry for {model}; dispatch refused before a model call"
            ));
        }
        return client::codex_preflight(expected, &home).or_else(inconsistent);
    }
    if rate_for(runner, model)?.is_none() {
        return inconsistent(format!(
            "no rates.json entry for {model}; dispatch refused before payment"
        ));
    }
    client::claude_preflight(expected, &home).or_else(inconsistent)
}

/// The client's own enforcement probe against a local fake API; no model is called.
fn client_probe(runner: &Runner, model: &str) -> R<Value> {
    let probed = if model.starts_with("gpt-") {
        client::probe_codex(&runner.checkout.to_path_buf(), &runner.executable, None)
    } else {
        client::probe_claude(&runner.executable)
    };
    probed.or_else(|_| inconsistent(PROBE_FAILED))
}

/// The sandbox probe and the client probe, as the session's enforcement record.
fn enforcement(runner: &Runner, work: &Path, key_path: &Path, model: &str) -> R<Value> {
    let tools = Tools {
        work: work.to_path_buf(),
        policy: Value::Object(Object::default()),
        protected: vec![key_path.to_path_buf(), client::user_home()],
        bench_tools: Some(runner.executable.clone()),
        host: Host::current(),
    };
    let mut enforcement = match grading_tools::probe(&tools) {
        Ok(receipt) => receipt,
        Err(Refusal::Denied(message)) => return inconsistent(message),
        Err(Refusal::Failed) => {
            return Err(input("the grading sandbox could not be probed".to_string()));
        }
    };
    let probe = client_probe(runner, model)?;
    insert(&mut enforcement, "client_probe", probe);
    Ok(enforcement)
}

/// The command policy and the hashes of the tool logs the session left.
fn record_logs(enforcement: &mut Value, work: &Path, key: &Value) -> R<()> {
    let policy = get(key, "command_policy_sha256")
        .cloned()
        .unwrap_or(Value::Null);
    insert(enforcement, "command_policy_sha256", policy);
    let mut logs = Object::default();
    for name in LOGS {
        let path = work.join(name);
        if path.is_file() {
            logs.insert(
                name.to_string(),
                text(&sha256::hex_digest(&read_bytes(&path)?)),
            );
        }
    }
    insert(enforcement, "logs", Value::Object(logs));
    Ok(())
}

fn append_charge(run: &Path, charge: &Value) -> R<()> {
    let path = run.join("charges.jsonl");
    OpenOptions::new()
        .append(true)
        .create(true)
        .open(&path)
        .and_then(|mut file| file.write_all(format!("{}\n", json(charge)).as_bytes()))
        .map_err(|error| input(shown(&error, &path)))
}

/// Run `dispatch` and return the line it prints on success; the reasons it failed are its inconsistency.
pub fn dispatch(runner: &Runner, options: &Dispatch) -> R<Vec<u8>> {
    let reasons_or_line = if options.client.model.starts_with("gpt-") {
        dispatch_codex(runner, options)?
    } else {
        dispatch_claude(runner, options)?
    };
    match reasons_or_line {
        Ok(line) => Ok(format!("{line}\n").into_bytes()),
        Err(reasons) => Err(Failure::Inconsistent(reasons.join("\n").into_bytes())),
    }
}

fn resolved_work(work: &Path) -> PathBuf {
    review_isolation::resolve(work)
}

/// The workspace's transcripts, root sessions and subagents, each sorted.
fn transcripts(work: &Path) -> Result<(Vec<PathBuf>, Vec<PathBuf>), String> {
    let projects = work.join("home/.claude/projects");
    let listed = |directory: &Path| -> Result<Vec<PathBuf>, String> {
        match std::fs::read_dir(directory) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(error) => Err(shown(&error, directory)),
            Ok(entries) => entries
                .map(|entry| {
                    entry
                        .map(|entry| entry.path())
                        .map_err(|error| shown(&error, directory))
                })
                .collect(),
        }
    };
    let (mut roots, mut subagents) = (Vec::new(), Vec::new());
    for project in listed(&projects)? {
        if !project.is_dir() {
            continue;
        }
        for entry in listed(&project)? {
            if entry
                .extension()
                .is_some_and(|extension| extension == "jsonl")
            {
                roots.push(entry);
            } else if entry.is_dir() {
                for agent in listed(&entry.join("subagents"))? {
                    let name = agent.file_name().unwrap_or_default().to_string_lossy();
                    if name.starts_with("agent-") && name.ends_with(".jsonl") {
                        subagents.push(agent);
                    }
                }
            }
        }
    }
    roots.sort();
    subagents.sort();
    Ok((roots, subagents))
}

/// The assistant models the transcripts name, sorted, without the client's synthetic messages.
fn models_in(paths: &[PathBuf]) -> Result<Vec<String>, String> {
    let mut models = std::collections::BTreeSet::new();
    for path in paths {
        let raw = std::fs::read(path).map_err(|error| shown(&error, path))?;
        let raw = String::from_utf8(raw).map_err(|_| format!("{}: not UTF-8", path.display()))?;
        for line in raw.lines() {
            let Ok(record) = pyjson::loads(line, pyjson::Duplicates::KeepLast) else {
                continue;
            };
            if get(&record, "type").and_then(Value::as_str) != Some("assistant") {
                continue;
            }
            if let Some(model) = get(&record, "message")
                .and_then(|message| get(message, "model"))
                .and_then(Value::as_str)
                .filter(|model| !model.is_empty() && *model != "<synthetic>")
            {
                models.insert(model.to_string());
            }
        }
    }
    Ok(models.into_iter().collect())
}

fn rounded(value: &Value) -> Option<Value> {
    match value {
        Value::Int(_) => Some(value.clone()),
        Value::Float(number) => Some(Value::Float(usage::round_to(*number, 6))),
        _ => None,
    }
}

/// The priced total and bounds of the transcripts at the rate entry, or why they could not be priced.
fn meter(paths: &[PathBuf], rate: &Value) -> Result<Value, String> {
    let price =
        |field: &str| client::price(rate, field).map_err(|key| format!("rate entry lacks {key}"));
    let input = price("input")?;
    let prices = usage::claude::Prices {
        input,
        output: price("output")?,
        read_mult: client::multiplier(price("cache_read")?, input),
        write_5m_mult: client::multiplier(price("cache_write_5m")?, input),
        write_1h_mult: client::multiplier(price("cache_write_1h")?, input),
    };
    let outcome = usage::claude::run(&usage::claude::Options {
        transcripts: paths
            .iter()
            .map(|path| pytext::os_bytes(path.as_os_str()).to_vec())
            .collect(),
        receipts: Vec::new(),
        prices,
        report: None,
        timing: None,
        by_kind: false,
        output: usage::claude::Output::Json,
    });
    if outcome.code != 0 {
        return Err(format!(
            "metering failed (bench-tools usage claude exit {}): {}",
            outcome.code,
            first(&String::from_utf8_lossy(&outcome.stderr), 300)
        ));
    }
    let report = pyjson::loads(
        &String::from_utf8_lossy(&outcome.stdout),
        pyjson::Duplicates::KeepLast,
    )
    .map_err(|error| format!("metering failed: {error}"))?;
    let total = get(&report, "total").ok_or("metering failed: no total")?;
    let priced = get(total, "cost")
        .and_then(rounded)
        .ok_or("metering failed: no priced total")?;
    let bound = |name: &str| {
        get(total, "cost_bounds")
            .and_then(|bounds| get(bounds, name))
            .and_then(rounded)
            .unwrap_or_else(|| priced.clone())
    };
    Ok(object(vec![
        ("priced_total_usd", priced.clone()),
        ("low", bound("low")),
        ("high", bound("high")),
    ]))
}

/// The read audit of the workspace, or why it could not run.
fn audit(work: &Path) -> Vec<String> {
    let audited = attempt_audit::run(&attempt_audit::Options {
        arm: "review-code".to_string(),
        attempt: pytext::os_bytes(work.as_os_str()).to_vec(),
        clone: work.join("clone").to_string_lossy().into_owned(),
        allowed: Vec::new(),
        allowed_prefix: Vec::new(),
        isolation_settings: None,
        allow_network: false,
        mount_sandbox: None,
        json: true,
    });
    let report = audited.and_then(|(stdout, _)| {
        pyjson::loads(&stdout, pyjson::Duplicates::KeepLast).map_err(|error| error.to_string())
    });
    match report {
        Ok(report) => get(&report, "violations")
            .and_then(Value::as_array)
            .unwrap_or_default()
            .iter()
            .map(|violation| {
                violation
                    .as_str()
                    .map_or_else(|| json(violation), str::to_string)
            })
            .collect(),
        Err(error) => vec![format!(
            "read audit could not run (exit 2): {}",
            first(&format!("{}: {error}", attempt_audit::PROGRAM), 300)
        )],
    }
}

/// The directory the client starts in, once it names none of the key's identities.
fn start_directory(key: &Value) -> R<PathBuf> {
    let Some(identifying) = get(key, "identifying") else {
        return inconsistent(
            "the key lists no identities to check the client's start directory against; prepare a fresh workspace",
        );
    };
    let start = clean_context::neutral_directory().or_else(inconsistent)?;
    let shown_start = start.to_string_lossy();
    let named: Vec<String> = identifying
        .as_array()
        .unwrap_or_default()
        .iter()
        .filter_map(Value::as_str)
        .filter(|identity| shown_start.contains(identity))
        .map(|identity| {
            format!(
                "client start directory names {}; set TMPDIR to a neutral directory",
                pytext::repr_str(identity)
            )
        })
        .collect();
    if !named.is_empty() {
        let _ = std::fs::remove_dir(&start);
        return inconsistent(named.join("\n"));
    }
    Ok(start)
}

/// The copied sign-in and the client's start directory, removed however the dispatch ends.
struct SignIn {
    credentials: PathBuf,
    start: Option<PathBuf>,
}

impl Drop for SignIn {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.credentials);
        if let Some(start) = &self.start {
            let _ = std::fs::remove_dir_all(start);
        }
    }
}

/// A client session leading its own process group, which is killed whole unless the client exited.
struct Group {
    child: Child,
    exited: bool,
}

impl Drop for Group {
    fn drop(&mut self) {
        if !self.exited {
            if let Ok(id) = i32::try_from(self.child.id()) {
                let _ = killpg(Pid::from_raw(id), Signal::SIGKILL);
            }
            let _ = self.child.wait();
        }
    }
}

fn stopped(signal: Signal) -> Failure {
    input(format!("stopped by {signal}"))
}

fn hold() -> R<interrupt::Held> {
    interrupt::hold().map_err(|error| input(format!("cannot hold the stop signals: {error}")))
}

/// `client` from `PATH`, started so that it begins with no stop signals held.
fn released(runner: &Runner, client: &str) -> R<Command> {
    let program = review_isolation::which(client)
        .ok_or_else(|| input(format!("cannot run {client}: not found on PATH")))?;
    Ok(interrupt::released(&runner.executable, &program, client))
}

/// Run the `client` session with stdin from `stdin` and its output into new files, killing its process group
/// once `timeout` passes or a held signal arrives.
fn session(
    client: &str,
    command: &mut Command,
    stdin: &Path,
    stdout: File,
    stderr: File,
    timeout: u64,
) -> R<Option<i32>> {
    let failed = |error: std::io::Error| input(format!("cannot run {client}: {error}"));
    if let Some(signal) = interrupt::received() {
        return Err(stopped(signal));
    }
    let child = command
        .stdin(File::open(stdin).map_err(failed)?)
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr))
        .process_group(0)
        .spawn()
        .map_err(failed)?;
    let mut group = Group {
        child,
        exited: false,
    };
    let deadline = Instant::now() + Duration::from_secs(timeout);
    loop {
        if let Some(status) = group.child.try_wait().map_err(failed)? {
            group.exited = true;
            return Ok(Some(crate::child::exit_code(status)));
        }
        if let Some(signal) = interrupt::received() {
            return Err(stopped(signal));
        }
        if Instant::now() >= deadline {
            return Ok(None);
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

type Outcome = Result<String, Vec<String>>;

fn dispatch_claude(runner: &Runner, options: &Dispatch) -> R<Outcome> {
    let Some(budget) = options.max_budget_usd else {
        return inconsistent("Claude dispatch requires --max-budget-usd");
    };
    let work = resolved_work(&options.work);
    let prompt = read_bytes(&work.join("prompt.md"))?;
    already_dispatched(&work)?;
    let key = checked_batch(runner, &work, &options.key)?;
    let mut cli_version = client_preflight(runner, &options.client)?;
    let mut enforcement = enforcement(runner, &work, &options.key, &options.client.model)?;
    let rate = rate_for(runner, &options.client.model)?;
    let user_home = client::user_home();
    let home = work.join("home");
    let credentials = home.join(".claude/.credentials.json");
    let session_id = clean_context::uuid4().map_err(input)?;
    let held = hold()?;
    let start = start_directory(&key)?;
    let sign_in = SignIn {
        credentials: credentials.clone(),
        start: Some(start.clone()),
    };
    let ran = (|| -> R<(String, Option<i32>)> {
        clean_context::prepare(&work).map_err(input)?;
        let mcp = object(vec![(
            "mcpServers",
            object(vec![(
                "grading",
                object(vec![
                    ("command", path_text(&runner.executable)),
                    (
                        "args",
                        Value::Array(vec![
                            text("grading"),
                            text("tools"),
                            text("--work"),
                            path_text(&work),
                            text("--policy"),
                            path_text(&work.join("command-policy.json")),
                            text("--protected"),
                            path_text(&review_isolation::resolve(&options.key)),
                            text("--protected"),
                            path_text(&review_isolation::resolve(&user_home)),
                            text("--bench-tools"),
                            path_text(&runner.executable),
                        ]),
                    ),
                ]),
            )]),
        )]);
        write_file(&work.join("grading-mcp.json"), json(&mcp))?;
        let settings = object(vec![
            ("claudeMdExcludes", strings(["**"])),
            ("autoMemoryEnabled", Value::Bool(false)),
            ("disableAllHooks", Value::Bool(true)),
        ]);
        write_file(&work.join("grading-settings.json"), json(&settings))?;
        std::fs::create_dir_all(home.join(".claude"))
            .map_err(|error| input(shown(&error, &home.join(".claude"))))?;
        let tmp = work.join("tmp");
        if let Err(error) = std::fs::create_dir(&tmp)
            && !tmp.is_dir()
        {
            return Err(input(shown(&error, &tmp)));
        }
        let source = current::read_json(&PyPath::new(pytext::os_bytes(
            user_home.join(".claude/.credentials.json").as_os_str(),
        )))?;
        let oauth = get(&source, "claudeAiOauth")
            .cloned()
            .unwrap_or(Value::Null);
        exclusive(&credentials, 0o600)
            .and_then(|mut file| {
                file.write_all(json(&object(vec![("claudeAiOauth", oauth)])).as_bytes())
            })
            .map_err(|error| {
                input(format!(
                    "cannot copy the Claude credentials: {}",
                    shown(&error, &credentials)
                ))
            })?;
        clean_context::claude_account(&user_home.join(".claude.json"), &home)
            .map_err(|error| input(format!("cannot read {error}")))?;
        let environment = [
            ("HOME", home.clone().into_os_string()),
            ("CLAUDE_CONFIG_DIR", home.join(".claude").into_os_string()),
            ("TMPDIR", tmp.into_os_string()),
            ("CLAUDE_CODE_PRINT_BG_WAIT_CEILING_MS", "0".into()),
            ("ENABLE_TOOL_SEARCH", "false".into()),
        ];
        let mut version = released(runner, "claude")?;
        version
            .arg("--version")
            .current_dir(&work)
            .envs(environment.clone());
        let version = crate::child::run(&mut version, Duration::from_secs(options.timeout))
            .map_err(|error| input(format!("cannot run claude: {error}")))?;
        let reported = version_in(&version.stdout)
            .unwrap_or_else(|| String::from_utf8_lossy(&version.stdout).trim().to_string());
        let dispatched_at = now();
        write_file(
            &work.join("timing.json"),
            format!(
                "{}\n",
                indented(&object(vec![("root_dispatched_at", text(&dispatched_at))]))
            ),
        )?;
        cli_version = reported;
        let output = |name: &str| {
            File::create(work.join(name)).map_err(|error| input(shown(&error, &work.join(name))))
        };
        let (stdout, stderr) = (output("stdout.txt")?, output("stderr.txt")?);
        let mut command = released(runner, "claude")?;
        command
            .args([
                "-p",
                "--model",
                &options.client.model,
                "--effort",
                &options.effort,
            ])
            .args(["--session-id", &session_id, "--restricted", "--tools", ""])
            .args(["--strict-mcp-config", "--setting-sources", ""])
            .arg("--mcp-config")
            .arg(work.join("grading-mcp.json"))
            .arg("--settings")
            .arg(work.join("grading-settings.json"))
            .arg("--allowedTools")
            .args(ALLOWED_TOOLS)
            .args(["--max-budget-usd", &json(&Value::Float(budget))])
            .current_dir(&start)
            .envs(environment);
        let exit = session(
            "claude",
            &mut command,
            &work.join("prompt.md"),
            stdout,
            stderr,
            options.timeout,
        )?;
        Ok((dispatched_at, exit))
    })();
    drop(sign_in);
    drop(held);
    let (dispatched_at, exit_code) = ran?;
    let completed_at = now();

    let mut violations = audit(&work);
    violations.extend(problems_after(runner, &work, &key));
    record_logs(&mut enforcement, &work, &key)?;
    let mut usage = object(vec![
        ("priced_total_usd", Value::Null),
        ("low", Value::Null),
        ("high", Value::Null),
    ]);
    let (models, subagents, meter_problem) = match transcripts(&work) {
        Err(error) => (
            Vec::new(),
            0,
            Some(format!("cannot list the transcripts: {error}")),
        ),
        Ok((roots, subagents)) => {
            let all: Vec<PathBuf> = roots.iter().chain(&subagents).cloned().collect();
            let (models, problem) = match models_in(&all) {
                Ok(models) => (models, None),
                Err(error) => (
                    Vec::new(),
                    Some(format!("cannot read a transcript: {error}")),
                ),
            };
            let problem = problem.or_else(|| match &rate {
                None => Some(format!(
                    "no rates.json entry for {}: usage not priced, no charge recorded",
                    options.client.model
                )),
                Some(_) if all.is_empty() => {
                    Some("no transcript to meter under home/.claude/projects".to_string())
                }
                Some(rate) => match meter(&all, rate) {
                    Ok(metered) => {
                        usage = metered;
                        None
                    }
                    Err(problem) => Some(problem),
                },
            });
            (models, subagents.len(), problem)
        }
    };
    let verdicts_present = work.join("verdicts.json").is_file();
    let priced = get(&usage, "priced_total_usd")
        .cloned()
        .unwrap_or(Value::Null);
    let record = object(vec![
        ("session_id", text(&session_id)),
        ("cli_version", text(&cli_version)),
        ("model", text(&options.client.model)),
        ("effort", text(&options.effort)),
        ("prompt_sha256", text(&sha256::hex_digest(&prompt))),
        ("dispatched_at", text(&dispatched_at)),
        ("completed_at", text(&completed_at)),
        ("exit_code", exit_value(exit_code)),
        (
            "models_observed",
            strings(models.iter().map(String::as_str)),
        ),
        ("subagents", Value::Int(subagents.to_string())),
        (
            "audit_violations",
            strings(violations.iter().map(String::as_str)),
        ),
        ("enforcement", enforcement),
        ("usage", usage),
        ("verdicts_present", Value::Bool(verdicts_present)),
    ]);
    write_file(
        &work.join("dispatch.json"),
        format!("{}\n", indented(&record)),
    )?;
    if let Some((run, step)) = &options.charge
        && !matches!(priced, Value::Null)
    {
        append_charge(
            run,
            &object(vec![
                ("at", text(&completed_at)),
                ("step", text(step)),
                ("usd", priced.clone()),
                ("model", text(&options.client.model)),
                ("billing", text("api-dollars")),
                ("session", text(&session_id[..8])),
            ]),
        )?;
    }

    let mut reasons = Vec::new();
    match exit_code {
        None => reasons.push(format!(
            "the session did not finish within {} s",
            options.timeout
        )),
        Some(0) => {}
        Some(code) => reasons.push(format!("the session exited {code}")),
    }
    reasons.extend(
        violations
            .iter()
            .map(|violation| format!("read audit: {violation}")),
    );
    if models != [options.client.model.clone()] {
        let observed = if models.is_empty() {
            "none".to_string()
        } else {
            models.join(", ")
        };
        reasons.push(format!(
            "models observed {observed}, expected {}",
            options.client.model
        ));
    }
    if subagents > 0 {
        reasons.push(format!(
            "{subagents} subagent transcript(s): the grader must work alone"
        ));
    }
    reasons.extend(meter_problem);
    if !verdicts_present {
        reasons.push("no verdicts.json written".to_string());
    }
    if !reasons.is_empty() {
        return Ok(Err(reasons));
    }
    Ok(Ok(format!(
        "graded in {}: session {}, ${}",
        work.display(),
        &session_id[..8],
        json(&priced)
    )))
}

fn field_models(result: &Value) -> Vec<Option<&str>> {
    get(result, "models_observed")
        .and_then(Value::as_array)
        .unwrap_or_default()
        .iter()
        .map(Value::as_str)
        .collect()
}

fn exit_value(code: Option<i32>) -> Value {
    code.map_or(Value::Null, |code| Value::Int(code.to_string()))
}

/// The Codex session's evidence, or a record that it is unavailable and so cannot prove a zero charge.
fn codex_result(work: &Path, rate: &Value, exit_code: Option<i32>) -> Value {
    let mut result = client::codex_evidence(work, rate).unwrap_or_else(|error| {
        object(vec![
            ("session_id", Value::Null),
            ("models_observed", Value::Array(Vec::new())),
            ("subagents", Value::Int("0".to_string())),
            (
                "audit_violations",
                strings([format!("Codex transcript evidence unavailable: {error}").as_str()]),
            ),
            (
                "usage",
                object(vec![
                    ("priced_total_usd", Value::Null),
                    ("low", Value::Null),
                    ("high", Value::Null),
                    ("billing", text("list-price-equivalent")),
                ]),
            ),
        ])
    });
    if exit_code != Some(0)
        && let Value::Object(entries) = &mut result
        && let Some(usage) = entries.get_mut("usage")
    {
        insert(usage, "high", Value::Null);
    }
    insert(&mut result, "exit_code", exit_value(exit_code));
    insert(&mut result, "budget_policy", text("codex-unbounded"));
    insert(&mut result, "provider_call_possible", Value::Bool(true));
    result
}

/// Run the Codex session with a copied sign-in that is removed however the session ends.
fn run_codex(runner: &Runner, work: &Path, options: &Dispatch) -> R<Option<i32>> {
    let home = work.join("home");
    let credentials = home.join(".codex/auth.json");
    let user_home = client::user_home();
    clean_context::prepare(work).map_err(input)?;
    client::configure_codex(
        &runner.checkout.to_path_buf(),
        work,
        &runner.executable,
        &[
            review_isolation::resolve(&options.key),
            review_isolation::resolve(&user_home),
        ],
        Some(&work.join("execution-policy.md")),
    )
    .map_err(input)?;
    let _held = hold()?;
    let _sign_in = SignIn {
        credentials: credentials.clone(),
        start: None,
    };
    (|| -> R<Option<i32>> {
        let source = read_bytes(&user_home.join(".codex/auth.json"))?;
        exclusive(&credentials, 0o600)
            .and_then(|mut file| file.write_all(&source))
            .map_err(|error| input(shown(&error, &credentials)))?;
        let tmp = work.join("tmp");
        if let Err(error) = std::fs::create_dir(&tmp)
            && !tmp.is_dir()
        {
            return Err(input(shown(&error, &tmp)));
        }
        let created = |name: &str| {
            exclusive(&work.join(name), 0o666)
                .map_err(|error| input(shown(&error, &work.join(name))))
        };
        let (stdout, stderr) = (created("stdout.jsonl")?, created("stderr.txt")?);
        let mut command = released(runner, "codex")?;
        command
            .args(["exec", "--skip-git-repo-check", "--ignore-rules", "--json"])
            .args(["--model", &options.client.model, "-c"])
            .arg(format!(
                "model_reasoning_effort={}",
                json(&text(&options.effort))
            ))
            .arg("-")
            .current_dir(work)
            .env_clear()
            .env(
                "PATH",
                std::env::var_os("PATH").unwrap_or_else(|| "/usr/bin:/bin".into()),
            )
            .env("LANG", "C.UTF-8")
            .env("HOME", &home)
            .env("CODEX_HOME", home.join(".codex"))
            .env("TMPDIR", &tmp);
        session(
            "codex",
            &mut command,
            &work.join("prompt.md"),
            stdout,
            stderr,
            options.timeout,
        )
    })()
}

fn dispatch_codex(runner: &Runner, options: &Dispatch) -> R<Outcome> {
    let work = resolved_work(&options.work);
    let prompt = read_bytes(&work.join("prompt.md"))?;
    already_dispatched(&work)?;
    if options.max_budget_usd.is_some() {
        return inconsistent(
            "Codex cannot enforce --max-budget-usd; use explicitly authorized unbounded mode",
        );
    }
    let key = checked_batch(runner, &work, &options.key)?;
    let version = client_preflight(runner, &options.client)?;
    let mut enforcement = enforcement(runner, &work, &options.key, &options.client.model)?;
    let dispatched_at = now();
    insert(&mut enforcement, "native_tools", text("mcp-metadata-only"));
    let mut helpers = client::RESOURCE_TOOLS.to_vec();
    helpers.sort_unstable();
    insert(&mut enforcement, "native_helpers", strings(helpers));
    write_file(
        &work.join("timing.json"),
        format!(
            "{}\n",
            json(&object(vec![("root_dispatched_at", text(&dispatched_at))]))
        ),
    )?;
    let rate = rate_for(runner, &options.client.model)?.unwrap_or(Value::Null);
    let exit_code = run_codex(runner, &work, options)?;
    let mut result = codex_result(&work, &rate, exit_code);
    let problems = problems_after(runner, &work, &key);
    if let Value::Object(entries) = &mut result
        && let Some(Value::Array(violations)) = entries.get_mut("audit_violations")
    {
        violations.extend(problems.iter().map(|problem| text(problem)));
    }
    let completed_at = now();
    record_logs(&mut enforcement, &work, &key)?;
    let verdicts_present = work.join("verdicts.json").is_file();
    let mut record = result.clone();
    for (name, value) in [
        ("cli_version", text(&version)),
        ("model", text(&options.client.model)),
        ("effort", text(&options.effort)),
        ("prompt_sha256", text(&sha256::hex_digest(&prompt))),
        ("dispatched_at", text(&dispatched_at)),
        ("completed_at", text(&completed_at)),
        ("verdicts_present", Value::Bool(verdicts_present)),
        ("enforcement", enforcement),
    ] {
        insert(&mut record, name, value);
    }
    write_file(
        &work.join("dispatch.json"),
        format!("{}\n", indented(&record)),
    )?;

    let field = |name: &str| get(&result, name).cloned().unwrap_or(Value::Null);
    let priced = get(&field("usage"), "priced_total_usd")
        .cloned()
        .unwrap_or(Value::Null);
    let mut reasons = Vec::new();
    if exit_code != Some(0) {
        let shown_exit = exit_code.map_or_else(|| "None".to_string(), |code| code.to_string());
        reasons.push(format!(
            "Codex session exit {shown_exit}; timeout limit {} s",
            options.timeout
        ));
    }
    reasons.extend(
        field("audit_violations")
            .as_array()
            .unwrap_or_default()
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_string),
    );
    let models: Vec<Option<&str>> = field_models(&result);
    let alone = models == [Some(options.client.model.as_str())] && !field("subagents").truthy();
    if !alone {
        reasons.push("Codex grader did not run the pinned model alone".to_string());
    }
    if matches!(priced, Value::Null) {
        reasons.push("Codex usage was not priced".to_string());
    }
    if !verdicts_present {
        reasons.push("no verdicts.json written".to_string());
    }
    if let Some((run, step)) = &options.charge
        && !matches!(priced, Value::Null)
    {
        append_charge(
            run,
            &object(vec![
                ("at", text(&completed_at)),
                ("step", text(step)),
                ("usd", priced.clone()),
                ("model", text(&options.client.model)),
                ("billing", text("list-price-equivalent")),
                ("session", field("session_id")),
            ]),
        )?;
    }
    if !reasons.is_empty() {
        return Ok(Err(reasons));
    }
    let session = field("session_id");
    Ok(Ok(format!(
        "graded in {}: Codex session {}, list-price equivalent ${}",
        work.display(),
        session.as_str().unwrap_or("None"),
        json(&priced)
    )))
}

/// `bench-tools grade preflight`: the client, its sign-in, pricing and tool enforcement, then every selected
/// batch's inputs; the lines it prints.
pub fn preflight(
    options: &Options,
    targets: &[String],
    work_root: &[u8],
    key_root: &[u8],
    grader: &Client,
    runner: &mut Runner,
    printed: &mut Vec<u8>,
) -> R<()> {
    client_preflight(runner, grader)?;
    client_probe(runner, &grader.model)?;
    let mut checked = Vec::new();
    preflight_inputs(options, targets, work_root, key_root, runner, &mut checked)
        .inspect_err(|_| printed.extend(&checked))?;
    let lines: Vec<&[u8]> = checked.split_inclusive(|byte| *byte == b'\n').collect();
    let Some((summary, lines)) = lines.split_last() else {
        return Err(Failure::Crash(
            "preflight-inputs printed no summary".to_string(),
        ));
    };
    for line in lines {
        printed.extend(*line);
    }
    let count = String::from_utf8_lossy(summary)
        .split_whitespace()
        .skip_while(|word| *word != "for")
        .nth(1)
        .unwrap_or("0")
        .to_string();
    printed.extend(
        format!(
            "queue preflight passed for {count} targets; local credential presence does not prove continuing \
             authentication\n"
        )
        .into_bytes(),
    );
    Ok(())
}
