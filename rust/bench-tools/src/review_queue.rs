//! `bench-tools queue`: run a pinned queue of frozen runs one review at a time, and track its recovery and status.
//!
//! ```text
//! bench-tools queue check --queue QUEUE.json
//! bench-tools queue run --queue QUEUE.json [--count N] [--detach]
//! bench-tools queue status --queue QUEUE.json [--snapshot]
//! bench-tools queue status --queue QUEUE.json --acknowledge SNAPSHOT --note TEXT
//! bench-tools queue recover --queue QUEUE.json
//! bench-tools queue replace --queue QUEUE.json --run RUN_ID --attempt att-NNN
//! ```
//!
//! The queue file names the frozen runs in dispatch order, the pinned client executables, the spend ceiling of each
//! client and the environment the runner needs:
//!
//! ```json
//! {"queue_id": "2026-10-10-builtin",
//!  "runs": ["bench/runs/<run>", ...],
//!  "clients": {"claude": {"path": "...", "sha256": "...", "version": "..."}},
//!  "ceilings_usd": {"claude": 900},
//!  "environment": {"BENCH_RATES": "{repo}/bench/rates.current.json"},
//!  "runtimes": {"bench-tools": {"sha256": "...", "receipt_sha256": "..."}},
//!  "quota_stop_percent": 95}
//! ```
//!
//! `work_root` (default `~/.t3/bench-runs`) and `state_dir` (default `~/.t3/bench-queues/<queue_id>`) may be set
//! too. `{repo}` is expanded in paths and environment values, and `~` in paths. `environment` may not name a client
//! executable, its hash, its version or its companion list. Every run uses one client.
//!
//! A client pin may also list `companions`, the files a client installs beside its executable, each as its path
//! relative to the executable's directory and its SHA-256. A file the pin does not list is not checked.
//! `dispatch.sh` checks the list again just before a built-in reviewer starts. The skill runners take their client
//! from the run's frozen `inputs/runner.json` and check no companions.
//!
//! `runtimes` pins Rust executables that `rust_runtime.py` retained, by the pins its `pin` command prints, in the
//! store that `runtime_store` names (default `~/.t3/bench-runtime`). Each run's `runtimes.frozen.json`, written when
//! the run is frozen, must hold the same pins. Before every launch each pinned executable and receipt must hash to
//! the pin, suit this host's target and C library, and come from the crate tree at each run's `freeze_commit`. A
//! launch gets the executable as `BENCH_RUST_<NAME>` with `BENCH_RUST_<NAME>_SHA256`, and its launch record keeps
//! the pins. A missing or changed pinned version blocks the queue: nothing falls back to the store's selected
//! version, and `rust_runtime.py rollback` changes only what new runs freeze. Each cell runs through the pinned
//! `bench-tools run-cell`, so a queue whose runs froze `rust/bench-tools` and that pins no `bench-tools` is refused
//! before any launch.
//!
//! `bench-tools run-cell` keeps the claims, the caps, the filing, the replacement rules and the cleanup. This
//! command decides only whether a launch may start, and it reads progress from the claims and the filed records
//! each time, never from its own notes. Before every launch:
//!
//! - The queue file and each run's manifest must hash as they did when the queue was first run
//!   (`queue.pin.json`), the runner files must match each run's `freeze_commit`, each client executable and each
//!   companion file its pin lists must hash to the pin, and so must each pinned Rust executable and its receipt.
//! - The state directory must be storage that survives a restart: not under `/tmp`, not on a memory file system,
//!   and a synced write to it must read back.
//! - One controller holds `<work_root>/serial.lock`. A second one is refused.
//! - A launch that was never recorded as ended, and a claim with no filed record, must be settled. Their processes
//!   are looked for in this host's process table. Only when that finds none is the claim settled through `run-cell`:
//!   `--release` when no reviewer started, `--file` when the dispatch ended, `--file --interrupted` otherwise, with
//!   the observation saved under `recoveries/`. Processes still running, or a process table this controller cannot
//!   read for that launch, block the queue. `recover` does this settlement and stops; `run` does it and goes on.
//! - A launch that ended must have left no process behind; the runner's return does not show that. Its processes
//!   are looked for in the same way until one observation finds none, which is saved in the launch record.
//!   Processes still running, or a launch this controller cannot see into, block the queue, also when the attempt
//!   is filed.
//! - A filed valid attempt whose clone or cache remains is pruned; a refusal blocks the queue.
//! - A filed attempt that is not valid and has no successor blocks the queue until the run's `deviations/` holds a
//!   diagnosis for it (see below).
//! - The subscription meters must have been read within the hour, report a usage window with a finite number for
//!   every pinned client, and be under the quota stop.
//! - The stored Claude sign-in must have more than half an hour left: a reviewer started on an expired one stops
//!   before its first request, and nothing renews it while no person is signed in.
//! - No filed attempt of any run may have used more than was reserved for it without a reconciling charge line.
//! - The client's spend over all its runs, counting failed attempts, charge lines such as probes, unknown totals at
//!   their reservation and attempts in flight, plus this attempt's bound, must fit the client's ceiling.
//!
//! A diagnosis is a JSON file in `<run>/deviations/` with `predecessor` (the attempt), `reason` and `cause`.
//! `harness-invalid`, `harness-stop` and `transient-capacity` are replaced by `replace`, which runs
//! `run-cell --replace` on the same frozen run, so the model, effort, policy, caps and budget are the run's own and
//! the reviewer starts in a fresh home. `transient-capacity` needs a call that succeeded first: a model response
//! captured in the failed attempt, or a valid completed attempt of the same arm. Without one it is a first-call
//! rejection, and like `setup-rejection` it stops the arm: the queue skips that arm's remaining cells and goes on
//! with the others. `skill-timeout` and `reviewer-stop`, a reviewer that ended its own session without a result,
//! are kept as the cell's result and never replaced. A valid attempt is never replaced.
//!
//! Process evidence. Each launch is recorded in `launches/` before it starts: the queue, the run, the command, the
//! attempts that existed, a marker placed in the child's environment, and the host, its boot, the PID namespace and
//! the user. The child starts in a process group of its own; its PID, start time and process group are added when
//! it starts. An observation reports `running`, `absent` or `unknown`:
//!
//! - `unknown` when there is no launch record, the record names no boot or is from another host, PID namespace or
//!   user, or a process of this user stays uninspectable, as described below. A free lock, a stale heartbeat, a
//!   missing PID or a zombie wrapper is never read as absence.
//! - `absent`, subject to the accepted limits below, when the host has rebooted since the launch, or two readings
//!   of the process table in a row find no live process that carries the marker, names the attempt directory in its
//!   environment, command or working directory, belongs to the launch's process group, or descends from one that
//!   does. A recorded PID now held by a process with another start time is that other process, and a process older
//!   than the launch is not the launch's. The process group counts only while it is shown to be the launch's: the
//!   wrapper still holds its PID with its recorded start time, or a live member carries the marker or names the
//!   attempt directory. A group number stays taken while any member lives, so once every process of the launch has
//!   ended, an unrelated group can take it.
//! - `running` otherwise, with the processes listed.
//!
//! A process of this user that is not older than the launch, is not a zombie, and refuses a read of its
//! environment, command or working directory is tied to the launch by its start time alone. It still blocks the
//! launch: the marker is in the environment that cannot be read, so nothing shows that the process is not a
//! reviewer child that left the launch's process group and lost its parent. Two kinds are known. An exiting process
//! has given up its address space and is not yet a zombie, and the kernel gives its `environ` to root from then on.
//! Measured on Linux 6.6, that lasted about a millisecond for a small process, up to 22 ms on a busy host, and
//! about 0.1 s for each GiB a large one held. A process that is not dumpable, such as systemd's `(sd-pam)`, refuses
//! for as long as it lives. An observation therefore reads the table again every `SCAN_GAP_SECONDS` until two
//! readings in a row have no such process. One such reading is not enough: while the table is being read, a process
//! can fork and exit, and that reading then lists neither the parent nor the child. A process that becomes a zombie
//! or disappears no longer blocks. Once `UNREADABLE_WAIT_SECONDS` have passed, a reading that still has such a
//! process leaves the launch `unknown`, and the reason names each one's command name, PID, state and start time,
//! and the error its read raised.
//!
//! Accepted limits. The decision on #107 applies #103's no-false-absence criterion to processes that cannot be
//! inspected, not to every process-table reading:
//! <https://github.com/kamui/code-review-bench/issues/107#issuecomment-6107142539>. All three cases in #107 are
//! outside that guarantee. When only the marker or attempt directory could tie a process to the launch, this check
//! can report `absent` while that process is alive:
//!
//! - Its first thread has ended while another runs. `/proc/PID/stat` can show state Z, so the check skips it as a
//!   zombie.
//! - Only another thread's own working directory is in the attempt directory, after `unshare(CLONE_FS)`. The check
//!   reads only the first thread's working directory.
//! - A single-threaded process is between images during `exec`. `environ` and `cmdline` can read empty without an
//!   error, so they expose neither the marker nor the attempt directory.
//!
//! One more limit comes from group reuse. Once the wrapper has ended, a live process left in its group counts only
//! while some live member carries the marker or names the attempt directory. Without one, nothing tells the
//! launch's group from an unrelated group that took the number later, so the launch reads `absent`.
//!
//! This command never signals a process.
//!
//! Status. The controller saves a snapshot under `status/` every 30 minutes, also while a review runs. `status`
//! prints the same report; `--snapshot` saves it. A snapshot is a local file. `--acknowledge` records separately
//! that a person or agent delivered one. `--detach` starts the controller in a process group of its own with its
//! output in `logs/`; that survives the launching shell, not a host or sandbox restart, and nothing here delivers a
//! message to anyone.
//!
//! Exit codes: 0 done, stopped at the quota, or printed; 1 blocked or refused, with the reasons on stdout; 2 an
//! input is missing or a helper failed, named on stderr.

use crate::grade::runtime::{self as rust_runtime, Checked};
use crate::normalize_review::{utc_now, utc_stamp};
use crate::pyjson::{self, Dumps, Duplicates, Object, Value};
use crate::run_cell::{
    self, Run, display, exists, get, is_file, number, object, read_json, sha256_file, text,
};
use crate::subscription_meters::{self, Pinned, Unreadable};
use crate::{evidence, file_attempt, pyfs, pysemantics, pytext, sha256, usage};
use regex::Regex;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions, TryLockError};
use std::io::{self, Write};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::os::unix::fs::MetadataExt;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::LazyLock;
use std::time::Instant;

const MARKER: &str = "BENCH_QUEUE_LAUNCH";
const RUNTIME_PATHS: [&str; 5] = [
    "bench/tools",
    "bench/schema",
    "bench/policies",
    "docs/clean-context.md",
    "rust",
];
const NORMALIZER: &str = "bench-tools";
const NORMALIZER_CRATE: &str = "rust/bench-tools";
const MEMORY_FILESYSTEMS: [&str; 3] = ["tmpfs", "ramfs", "devtmpfs"];
pub const SNAPSHOT_SECONDS: f64 = 1800.0;
const QUOTA_MAX_AGE_SECONDS: f64 = 3600.0;
const HEARTBEAT_SECONDS: f64 = 60.0;
pub const POLL_SECONDS: f64 = 5.0;
pub const SCAN_GAP_SECONDS: f64 = 0.2;
pub const UNREADABLE_WAIT_SECONDS: f64 = 5.0;
const CAUSES: &str = "['harness-invalid', 'harness-stop', 'reviewer-stop', 'setup-rejection', 'skill-timeout', \
                      'transient-capacity']";

static ENDED: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)^exit=-?\d+$").expect("pattern"));
static PINNED_NAME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:BENCH_(CLAUDE|CODEX)(_SHA256|_VERSION|_COMPANIONS)?|BENCH_RUST_\w+|BENCH_QUEUE_LAUNCH)$")
        .expect("pattern")
});
static QUEUE_ID: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[a-z0-9][a-z0-9-]*$").expect("pattern"));

fn client_of_kind(kind: &str) -> Option<&'static str> {
    match kind {
        "claude-builtin" | "claude-skill" | "review-code" => Some("claude"),
        "codex" | "codex-skill" => Some("codex"),
        _ => None,
    }
}

/// Whether `run-cell` is told the harness stopped the predecessor, for a cause that is replaced.
fn replaced(cause: &str) -> Option<bool> {
    match cause {
        "harness-invalid" => Some(false),
        "harness-stop" | "transient-capacity" => Some(true),
        _ => None,
    }
}

fn kept(cause: &str) -> Option<&'static str> {
    match cause {
        "skill-timeout" => Some("a skill timeout"),
        "reviewer-stop" => Some("a reviewer that ended its session without a result"),
        _ => None,
    }
}

fn known_cause(cause: &str) -> bool {
    replaced(cause).is_some() || kept(cause).is_some() || cause == "setup-rejection"
}

#[derive(Debug, PartialEq, Eq)]
pub enum Failure {
    /// Something must be settled before the queue may launch; `{"blocked": [...]}` on stdout, exit 1.
    Blocked(Vec<String>),
    /// A run is not in a state that allows this; stdout, exit 1.
    Refused(String),
    /// An input is missing or a helper failed; stderr, exit 2.
    Input(String),
}

impl From<run_cell::Failure> for Failure {
    fn from(failure: run_cell::Failure) -> Self {
        match failure {
            run_cell::Failure::Refused(message) => Self::Refused(message),
            run_cell::Failure::Input(message) => Self::Input(message),
        }
    }
}

type Q<T> = Result<T, Failure>;

fn blocked<T>(reason: impl Into<String>) -> Q<T> {
    Err(Failure::Blocked(vec![reason.into()]))
}

fn input<T>(message: impl Into<String>) -> Q<T> {
    Err(Failure::Input(message.into()))
}

/// Reads the subscription meters of the pinned clients in an environment.
pub type Meters<'a> = &'a dyn Fn(&[Pinned], &[(String, String)]) -> Result<Value, Unreadable>;

/// What one invocation acts on, and the effects a test replaces.
pub struct Effects<'a> {
    /// The checkout the runs belong to; launches run in it and `{repo}` names it.
    pub repo: PathBuf,
    /// The environment the command was started with.
    pub env: Vec<(String, String)>,
    /// Replaces the pinned `bench-tools run-cell` as the command each cell runs through.
    pub runner: Option<Vec<String>>,
    /// This bench-tools, which `--detach` starts again.
    pub executable: PathBuf,
    pub proc_root: PathBuf,
    pub uid: u32,
    pub clock: &'a dyn Fn() -> f64,
    /// Waits between two checks of a running launch.
    pub sleep: &'a dyn Fn(f64),
    /// Waits between two readings of the process table; the wait is measured by the monotonic clock.
    pub nap: &'a dyn Fn(f64),
    pub meters: Meters<'a>,
    /// Prunes a filed valid attempt's clone and cache.
    pub prune: run_cell::Prune<'a>,
    pub poll_seconds: f64,
    pub scan_gap_seconds: f64,
    pub unreadable_wait_seconds: f64,
}

impl Effects<'_> {
    fn var(&self, name: &str) -> Option<&str> {
        self.env
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }

    /// `{repo}` expanded, then `~`, then relative to the checkout.
    fn located(&self, text: &str) -> PathBuf {
        let text = text.replace("{repo}", &display(&self.repo));
        let expanded = match (text.strip_prefix('~'), self.var("HOME")) {
            (Some(rest), Some(home)) if rest.is_empty() || rest.starts_with('/') => {
                format!("{home}{rest}")
            }
            _ => text,
        };
        let path = PathBuf::from(expanded);
        if path.is_absolute() {
            path
        } else {
            self.repo.join(path)
        }
    }
}

// --- values ----------------------------------------------------------------------------------

fn int(value: impl ToString) -> Value {
    Value::Int(value.to_string())
}

fn texts<S: AsRef<str>>(values: &[S]) -> Value {
    Value::Array(values.iter().map(|value| text(value.as_ref())).collect())
}

fn integer(value: &Value) -> Option<i64> {
    match value {
        Value::Int(digits) => digits.parse().ok(),
        _ => None,
    }
}

fn put(target: &mut Value, key: &str, value: Value) {
    if let Value::Object(fields) = target {
        fields.insert(key.to_string(), value);
    }
}

fn merged(base: &Value, pairs: Vec<(&str, Value)>) -> Value {
    let mut out = base.clone();
    for (key, value) in pairs {
        put(&mut out, key, value);
    }
    out
}

fn has(value: &Value, key: &str) -> bool {
    value
        .as_object()
        .is_some_and(|fields| fields.get(key).is_some())
}

fn indented(value: &Value) -> String {
    let mut out = pyjson::dumps(
        value,
        &Dumps {
            indent: Some(2),
            ..Dumps::default()
        },
    );
    out.push('\n');
    out
}

fn os_failure(path: &Path) -> impl Fn(io::Error) -> Failure + '_ {
    move |error| Failure::Input(pytext::os_error(&error, path.as_os_str().as_bytes()))
}

/// The JSON document at `path`, replaced in one step.
fn write(path: &Path, value: &Value) -> Q<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(os_failure(parent))?;
    }
    file_attempt::publish(&display(path), &indented(value)).map_err(os_failure(path))
}

fn append(path: &Path, line: &str) -> Q<()> {
    let mut handle = OpenOptions::new()
        .append(true)
        .create(true)
        .open(path)
        .map_err(os_failure(path))?;
    handle
        .write_all(format!("{line}\n").as_bytes())
        .map_err(os_failure(path))
}

/// The last `count` characters of `text`.
fn tail(text: &str, count: usize) -> &str {
    let skip = text.chars().count().saturating_sub(count);
    text.char_indices()
        .nth(skip)
        .map_or("", |(at, _)| &text[at..])
}

/// `datetime.fromtimestamp(moment, timezone.utc).isoformat(timespec="seconds")` with `Z`.
fn stamp(moment: f64) -> String {
    utc_stamp(moment.max(0.0).floor() as u64)
}

fn compact(stamp: &str) -> String {
    stamp.replace(['-', ':'], "")
}

fn now_micros() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| {
            i64::try_from(elapsed.as_micros()).unwrap_or(i64::MAX)
        })
}

fn moment_of(stamp: &str) -> Option<f64> {
    usage::timestamp_micros(&stamp.replace('Z', "+00:00")).map(|micros| micros as f64 / 1e6)
}

/// A random lowercase hex token of 32 characters.
fn token() -> String {
    let seed = format!(
        "{}{}{}{}",
        evidence::random_name(""),
        evidence::random_name(""),
        now_micros(),
        std::process::id()
    );
    sha256::hex_digest(seed.as_bytes())[..32].to_string()
}

/// The sorted names in `directory` that start with `prefix` and end with `suffix`, hidden names left out.
fn listed(directory: &Path, prefix: &str, suffix: &str) -> Q<Vec<String>> {
    let mut found = run_cell::names(directory)?;
    found.retain(|name| {
        !name.starts_with('.') && name.starts_with(prefix) && name.ends_with(suffix)
    });
    Ok(found)
}

// --- the queue file --------------------------------------------------------------------------

struct Client {
    name: String,
    path: String,
    sha256: String,
    version: String,
    companions: Option<Vec<(String, String)>>,
}

impl Client {
    fn value(&self) -> Value {
        let mut pairs = vec![
            ("path", text(&self.path)),
            ("sha256", text(&self.sha256)),
            ("version", text(&self.version)),
        ];
        if let Some(companions) = &self.companions {
            pairs.push((
                "companions",
                Value::Object(
                    companions
                        .iter()
                        .map(|(name, digest)| (name.clone(), text(digest)))
                        .collect(),
                ),
            ));
        }
        object(pairs)
    }

    /// Each file the pin covers, as what it is, its path and its SHA-256.
    fn files(&self) -> Vec<(String, PathBuf, &str)> {
        let executable = PathBuf::from(&self.path);
        let directory = executable
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_default();
        let mut files = vec![("executable".to_string(), executable, self.sha256.as_str())];
        for (name, digest) in self.companions.iter().flatten() {
            files.push((
                format!("companion file {name}"),
                directory.join(name),
                digest.as_str(),
            ));
        }
        files
    }
}

struct RuntimePin {
    name: String,
    sha256: String,
    receipt_sha256: String,
}

/// The variable a launch finds a pinned Rust executable in; its SHA-256 is in the same name with `_SHA256`.
fn runtime_variable(name: &str) -> String {
    format!("BENCH_RUST_{}", name.to_uppercase().replace('-', "_"))
}

/// The queue file: which frozen runs, in what order, with which pinned clients and ceilings.
pub struct Queue {
    path: PathBuf,
    id: String,
    run_dirs: Vec<PathBuf>,
    clients: Vec<Client>,
    ceilings: Vec<(String, f64)>,
    environment: Vec<(String, String)>,
    runtimes: Vec<RuntimePin>,
    runtime_store: PathBuf,
    quota_stop: f64,
    work_root: PathBuf,
    state: PathBuf,
}

fn resolved(path: &Path) -> PathBuf {
    std::ffi::OsString::from_vec(pyfs::resolve(path.as_os_str().as_bytes())).into()
}

impl Queue {
    pub fn open(path: &Path, effects: &Effects<'_>) -> Q<Self> {
        let path = resolved(path);
        let spec = read_json(&path)?;
        let wrong = |what: String| {
            Failure::Input(format!("{} is not a queue file: {what}", path.display()))
        };
        let required = |key: &str| -> Q<&Value> {
            spec.as_object()
                .and_then(|fields| fields.get(key))
                .ok_or_else(|| wrong(format!("it has no {key}")))
        };
        let members = |value: &Value, what: &str| -> Q<Vec<(String, Value)>> {
            value
                .as_object()
                .map(|fields| {
                    fields
                        .iter()
                        .map(|(key, value)| (key.clone(), value.clone()))
                        .collect()
                })
                .ok_or_else(|| wrong(format!("{what} is not an object")))
        };
        let field = |value: &Value, key: &str, what: &str| -> Q<String> {
            match get(value, key) {
                Value::Null if !has(value, key) => Err(wrong(format!("{what} has no {key}"))),
                found => Ok(found.py_str()),
            }
        };
        let id = required("queue_id")?.py_str();
        let run_dirs = required("runs")?
            .as_array()
            .ok_or_else(|| wrong("runs is not a list".into()))?
            .iter()
            .map(|entry| resolved(&effects.located(&entry.py_str())))
            .collect::<Vec<_>>();
        let mut clients = Vec::new();
        for (name, pin) in members(required("clients")?, "clients")? {
            let what = format!("the {name} pin");
            let companions = match get(&pin, "companions") {
                Value::Null if !has(&pin, "companions") => None,
                found => Some(
                    members(found, &format!("{what}'s companions"))?
                        .into_iter()
                        .map(|(file, digest)| (file, digest.py_str()))
                        .collect(),
                ),
            };
            clients.push(Client {
                path: field(&pin, "path", &what)?,
                sha256: field(&pin, "sha256", &what)?,
                version: field(&pin, "version", &what)?,
                companions,
                name,
            });
        }
        let mut ceilings = Vec::new();
        for (client, usd) in members(required("ceilings_usd")?, "ceilings_usd")? {
            let usd = number(&usd)
                .ok_or_else(|| wrong(format!("the {client} ceiling is not a number")))?;
            ceilings.push((client, usd));
        }
        let optional = |key: &str| spec.as_object().and_then(|fields| fields.get(key));
        let environment = match optional("environment") {
            None => Vec::new(),
            Some(found) => members(found, "environment")?
                .into_iter()
                .map(|(name, value)| (name, value.py_str()))
                .collect(),
        };
        let mut runtimes = Vec::new();
        if let Some(found) = optional("runtimes") {
            for (name, pin) in members(found, "runtimes")? {
                let what = format!("the {name} runtime pin");
                runtimes.push(RuntimePin {
                    sha256: field(&pin, "sha256", &what)?,
                    receipt_sha256: field(&pin, "receipt_sha256", &what)?,
                    name,
                });
            }
        }
        let text_or = |key: &str, default: String| optional(key).map_or(default, Value::py_str);
        let quota_stop = match optional("quota_stop_percent") {
            None => 95.0,
            Some(found) => {
                number(found).ok_or_else(|| wrong("quota_stop_percent is not a number".into()))?
            }
        };
        let queue = Self {
            runtime_store: effects.located(&text_or("runtime_store", "~/.t3/bench-runtime".into())),
            work_root: effects.located(&text_or("work_root", "~/.t3/bench-runs".into())),
            state: effects.located(&text_or("state_dir", format!("~/.t3/bench-queues/{id}"))),
            path,
            id,
            run_dirs,
            clients,
            ceilings,
            environment,
            runtimes,
            quota_stop,
        };
        let shown = queue.path.display();
        if !QUEUE_ID.is_match(&queue.id) || queue.run_dirs.is_empty() {
            return input(format!(
                "{shown}: queue_id is lowercase letters, digits and hyphens, and runs is not empty"
            ));
        }
        let mut reserved: Vec<&str> = queue
            .environment
            .iter()
            .map(|(name, _)| name.as_str())
            .filter(|name| PINNED_NAME.is_match(name))
            .collect();
        reserved.sort_unstable();
        if !reserved.is_empty() {
            return input(format!(
                "{shown}: environment may not set {}; clients are pinned under clients",
                reserved.join(", ")
            ));
        }
        let generated: Vec<String> = queue
            .runtimes
            .iter()
            .flat_map(|pin| {
                let variable = runtime_variable(&pin.name);
                [variable.clone(), format!("{variable}_SHA256")]
            })
            .collect();
        let clashing: BTreeSet<&String> = generated
            .iter()
            .filter(|variable| generated.iter().filter(|other| other == variable).count() > 1)
            .collect();
        if !clashing.is_empty() {
            let names: Vec<&str> = clashing.iter().map(|name| name.as_str()).collect();
            return input(format!(
                "{shown}: runtime names generate the same launch variable {}; rename one",
                names.join(", ")
            ));
        }
        Ok(queue)
    }

    fn runs(&self) -> Q<Vec<Run>> {
        self.run_dirs
            .iter()
            .map(|directory| {
                Run::open(
                    directory,
                    &self
                        .work_root
                        .join(directory.file_name().unwrap_or_default()),
                )
                .map_err(Failure::from)
            })
            .collect()
    }

    fn client(&self, name: &str) -> Option<&Client> {
        self.clients.iter().find(|client| client.name == name)
    }

    fn ceiling(&self, name: &str) -> Option<f64> {
        self.ceilings
            .iter()
            .find(|(client, _)| client == name)
            .map(|(_, usd)| *usd)
    }

    fn runtimes_value(&self) -> Value {
        Value::Object(
            self.runtimes
                .iter()
                .map(|pin| {
                    (
                        pin.name.clone(),
                        object(vec![
                            ("sha256", text(&pin.sha256)),
                            ("receipt_sha256", text(&pin.receipt_sha256)),
                        ]),
                    )
                })
                .collect(),
        )
    }

    fn pinned(&self) -> Vec<Pinned> {
        self.clients
            .iter()
            .map(|client| Pinned {
                name: client.name.clone(),
                path: PathBuf::from(&client.path),
                version: client.version.clone(),
            })
            .collect()
    }
}

fn client_of(run: &Run) -> Q<&'static str> {
    let mut clients: Vec<Option<&'static str>> = Vec::new();
    for arm in get(&run.manifest, "arms").as_array().unwrap_or(&[]) {
        let kind = get(&read_json(&run.arm_file(&get(arm, "id").py_str()))?, "kind").py_str();
        let client = client_of_kind(&kind);
        if !clients.contains(&client) {
            clients.push(client);
        }
    }
    match clients.as_slice() {
        [Some(client)] => Ok(client),
        _ => {
            let mut names: Vec<String> = clients
                .iter()
                .map(|client| pytext::repr_str(client.unwrap_or("None")))
                .collect();
            names.sort();
            input(format!(
                "{}: a queue run uses one known client, not [{}]",
                run.id,
                names.join(", ")
            ))
        }
    }
}

// --- pins ------------------------------------------------------------------------------------

/// `\ooo` escapes in a mountinfo path.
fn unescaped(field: &str) -> PathBuf {
    let bytes = field.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        let octal = bytes.get(index + 1..index + 4).filter(|digits| {
            bytes[index] == b'\\' && digits.iter().all(|digit| (b'0'..=b'7').contains(digit))
        });
        if let Some(digits) = octal {
            let value = digits
                .iter()
                .fold(0_u32, |value, digit| value * 8 + u32::from(digit - b'0'));
            out.push(u8::try_from(value & 0xff).unwrap_or(0));
            index += 4;
        } else {
            out.push(bytes[index]);
            index += 1;
        }
    }
    std::ffi::OsString::from_vec(out).into()
}

/// The type of the file system mounted deepest over `path`; at equal depth the later mount.
fn filesystem(path: &Path, proc_root: &Path) -> Option<String> {
    let rows = fs::read_to_string(proc_root.join("self/mountinfo")).ok()?;
    let mut depth = 0;
    let mut kind = None;
    for row in rows.lines() {
        let (mounted, source) = row.split_once(" - ").unwrap_or((row, ""));
        let Some(mount) = mounted.split_whitespace().nth(4).map(unescaped) else {
            continue;
        };
        let parts = mount.components().count();
        if path.starts_with(&mount) && parts >= depth {
            depth = parts;
            kind = source.split_whitespace().next().map(str::to_string);
        }
    }
    kind
}

/// Why `path` is not storage that survives a restart, or None once a synced write to it reads back.
fn volatile(path: &Path, proc_root: &Path) -> Option<String> {
    let path = resolved(path);
    for scratch in [
        PathBuf::from("/tmp"),
        std::env::temp_dir(),
        PathBuf::from("/dev/shm"),
        PathBuf::from("/run"),
    ] {
        if path.starts_with(resolved(&scratch)) {
            return Some(format!("{} is under {}", path.display(), scratch.display()));
        }
    }
    let shown = path.display();
    if let Err(error) = fs::create_dir_all(&path) {
        return Some(format!(
            "{shown} cannot be written: {}",
            pytext::os_error(&error, path.as_os_str().as_bytes())
        ));
    }
    if let Some(kind) =
        filesystem(&path, proc_root).filter(|kind| MEMORY_FILESYSTEMS.contains(&kind.as_str()))
    {
        return Some(format!("{shown} is on {kind}"));
    }
    let probe = path.join(".persistence-probe");
    let expected = token();
    let kept = file_attempt::publish(&display(&probe), &expected)
        .and_then(|()| fs::read_to_string(&probe))
        .and_then(|found| fs::remove_file(&probe).map(|()| found == expected));
    match kept {
        Err(error) => Some(format!(
            "{shown} cannot be written: {}",
            pytext::os_error(&error, probe.as_os_str().as_bytes())
        )),
        Ok(false) => Some(format!("a write to {shown} did not read back")),
        Ok(true) => None,
    }
}

fn runtime_problem(run: &Run, repo: &Path) -> Option<String> {
    let commit = get(&run.manifest, "freeze_commit");
    if !commit.truthy() {
        return Some(format!(
            "{}: the manifest has no freeze_commit to compare the runner files with",
            run.id
        ));
    }
    let commit = commit.py_str();
    let done = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["diff", "--quiet", &commit, "--"])
        .args(RUNTIME_PATHS)
        .stdin(Stdio::null())
        .output();
    match done {
        Ok(done) if done.status.success() => None,
        Ok(done) if done.status.code() == Some(1) => Some(format!(
            "{}: the runner files differ from freeze commit {commit}",
            run.id
        )),
        Ok(done) => Some(format!(
            "{}: cannot compare the runner files with {commit}: {}",
            run.id,
            pytext::strip(&String::from_utf8_lossy(&done.stderr))
        )),
        Err(error) => Some(format!(
            "{}: cannot compare the runner files with {commit}: {error}",
            run.id
        )),
    }
}

/// Each pinned Rust executable and its receipt, once it hashes to its pin and suits this host.
fn runtimes(queue: &Queue) -> Q<Vec<(String, rust_runtime::Runtime)>> {
    let mut checked = Vec::new();
    for pin in &queue.runtimes {
        match rust_runtime::check(
            &queue.runtime_store,
            &pin.name,
            &pin.sha256,
            &pin.receipt_sha256,
            true,
        ) {
            Ok(found) => checked.push((pin.name.clone(), found)),
            Err(Checked::Refused(why)) => {
                return blocked(format!("the {} Rust runtime: {why}", pin.name));
            }
            Err(Checked::Input(why)) => {
                return input(format!(
                    "{}: runtime {}: {why}",
                    queue.path.display(),
                    pin.name
                ));
            }
        }
    }
    Ok(checked)
}

fn set(env: &mut Vec<(String, String)>, name: &str, value: String) {
    env.retain(|(key, _)| key != name);
    env.push((name.to_string(), value));
}

fn environment(
    queue: &Queue,
    pinned: &[(String, rust_runtime::Runtime)],
    effects: &Effects<'_>,
) -> Q<Vec<(String, String)>> {
    let mut env: Vec<(String, String)> = effects
        .env
        .iter()
        .filter(|(name, _)| {
            !["CLAUDE", "ANTHROPIC", "CODEX_COMPANION", "BENCH_RUST_"]
                .iter()
                .any(|prefix| name.starts_with(prefix))
                && name != "AI_AGENT"
        })
        .cloned()
        .collect();
    for client in &queue.clients {
        let mut companions = Vec::new();
        for (what, path, digest) in client.files() {
            match fs::read(&path) {
                Ok(bytes) if sha256::hex_digest(&bytes) == digest => {}
                Ok(_) => {
                    return blocked(format!(
                        "the {} {what} no longer hashes to its pin",
                        client.name
                    ));
                }
                Err(error) => {
                    return blocked(format!(
                        "the pinned {} {what} cannot be read: {}",
                        client.name,
                        pytext::os_error(&error, path.as_os_str().as_bytes())
                    ));
                }
            }
            if what != "executable" {
                companions.push(format!("{digest}  {}", path.display()));
            }
        }
        let prefix = format!("BENCH_{}", client.name.to_uppercase());
        set(&mut env, &prefix, client.path.clone());
        set(&mut env, &format!("{prefix}_SHA256"), client.sha256.clone());
        set(
            &mut env,
            &format!("{prefix}_VERSION"),
            client.version.clone(),
        );
        // dispatch.sh checks this list with sha256sum just before a built-in reviewer starts.
        env.retain(|(key, _)| *key != format!("{prefix}_COMPANIONS"));
        if !companions.is_empty() {
            env.push((format!("{prefix}_COMPANIONS"), companions.join("\n")));
        }
    }
    for (name, runtime) in pinned {
        let variable = runtime_variable(name);
        set(&mut env, &variable, display(&runtime.executable));
        set(
            &mut env,
            &format!("{variable}_SHA256"),
            get(get(&runtime.receipt, "artifact"), "sha256").py_str(),
        );
    }
    for (name, value) in &queue.environment {
        set(
            &mut env,
            name,
            value.replace("{repo}", &display(&effects.repo)),
        );
    }
    Ok(env)
}

/// What a launch runs: its environment and the command each cell runs through.
#[derive(Clone, Default)]
struct Launcher {
    env: Vec<(String, String)>,
    runner: Vec<String>,
}

/// Refuse a queue that is not the one pinned, state that could vanish, or a runner or client that drifted.
fn verify(queue: &Queue, runs: &[Run], pin: bool, effects: &Effects<'_>) -> Q<Launcher> {
    if let Some(problem) = volatile(&queue.state, &effects.proc_root) {
        return blocked(format!(
            "the state directory is not persistent storage: {problem}"
        ));
    }
    let mut pins = Object::default();
    for run in runs {
        pins.insert(
            run.id.clone(),
            object(vec![
                (
                    "manifest_sha256",
                    text(&sha256_file(&run.dir.join("manifest.json"))?),
                ),
                ("freeze_commit", get(&run.manifest, "freeze_commit").clone()),
            ]),
        );
    }
    let current = object(vec![
        ("queue_id", text(&queue.id)),
        ("queue_sha256", text(&sha256_file(&queue.path)?)),
        ("runs", Value::Object(pins)),
    ]);
    let pinned = queue.state.join("queue.pin.json");
    if is_file(&pinned)? {
        let saved = read_json(&pinned)?;
        let same = ["queue_id", "queue_sha256", "runs"]
            .iter()
            .all(|key| get(&saved, key).py_eq(get(&current, key)) && has(&saved, key));
        if !same {
            return blocked(format!(
                "the queue file or a run manifest changed since {} was written; a changed queue is a new queue_id",
                pinned.display()
            ));
        }
    } else if pin {
        write(
            &pinned,
            &merged(&current, vec![("pinned_at", text(&utc_now()))]),
        )?;
    }
    let checked = runtimes(queue)?;
    let env = environment(queue, &checked, effects)?;
    let frozen_runtimes = queue.runtimes_value();
    for run in runs {
        let client = client_of(run)?;
        let Some(pin) = queue
            .client(client)
            .filter(|_| queue.ceiling(client).is_some())
        else {
            return input(format!(
                "{} runs on {client}, which the queue gives no pinned executable or no ceiling",
                run.id
            ));
        };
        let frozen = run.dir.join("clients.frozen.json");
        if is_file(&frozen)? && !get(&read_json(&frozen)?, client).py_eq(&pin.value()) {
            return blocked(format!(
                "{}: the queue's {client} pin differs from the run's clients.frozen.json",
                run.id
            ));
        }
        let frozen = run.dir.join("runtimes.frozen.json");
        let saved = if is_file(&frozen)? {
            read_json(&frozen)?
        } else {
            Value::Object(Object::default())
        };
        if !saved.py_eq(&frozen_runtimes) {
            return blocked(format!(
                "{}: the queue's Rust runtime pins differ from the run's runtimes.frozen.json",
                run.id
            ));
        }
        if let Some(problem) = runtime_problem(run, &effects.repo) {
            return blocked(problem);
        }
        let commit = get(&run.manifest, "freeze_commit").py_str();
        if !queue.runtimes.iter().any(|pin| pin.name == NORMALIZER)
            && rust_runtime::crate_tree(&effects.repo, &commit, NORMALIZER_CRATE).is_some()
        {
            return blocked(format!(
                "{}: freeze commit {commit} has {NORMALIZER_CRATE}, which normalizes its reviews and runs its \
                 cells, but the run pins no {NORMALIZER} runtime",
                run.id
            ));
        }
        for (name, runtime) in &checked {
            if let Some(problem) =
                rust_runtime::source_problem(&runtime.receipt, Some(&commit), &effects.repo)
            {
                return blocked(format!("{}: the {name} Rust runtime: {problem}", run.id));
            }
        }
    }
    let runner = match &effects.runner {
        Some(runner) => runner.clone(),
        None => match checked.iter().find(|(name, _)| name == NORMALIZER) {
            Some((_, runtime)) => vec![display(&runtime.executable), "run-cell".to_string()],
            None => {
                return blocked(format!(
                    "the queue pins no {NORMALIZER} runtime to run its cells"
                ));
            }
        },
    };
    Ok(Launcher { env, runner })
}

/// Holds `<work_root>/serial.lock` until dropped.
fn serial(queue: &Queue) -> Q<File> {
    fs::create_dir_all(&queue.work_root).map_err(os_failure(&queue.work_root))?;
    let path = queue.work_root.join("serial.lock");
    let handle = OpenOptions::new()
        .append(true)
        .create(true)
        .open(&path)
        .map_err(os_failure(&path))?;
    match handle.try_lock() {
        Ok(()) => Ok(handle),
        Err(TryLockError::WouldBlock) => blocked(format!(
            "another serial controller holds {}",
            path.display()
        )),
        Err(TryLockError::Error(error)) => Err(os_failure(&path)(error)),
    }
}

// --- process evidence ------------------------------------------------------------------------

fn trimmed(path: &Path) -> Option<String> {
    fs::read_to_string(path)
        .ok()
        .map(|found| pytext::strip(&found).to_string())
}

fn identity(effects: &Effects<'_>) -> Value {
    let host = nix::unistd::gethostname()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let machine = match trimmed(Path::new("/etc/machine-id")) {
        None => Value::Null,
        Some(id) if id.is_empty() => text(""),
        Some(id) => text(&sha256::hex_digest(id.as_bytes())),
    };
    let namespace = fs::read_link(effects.proc_root.join("self/ns/pid"))
        .map_or(Value::Null, |target| text(&target.to_string_lossy()));
    object(vec![
        ("host", text(&host)),
        ("machine_id_sha256", machine),
        (
            "boot_id",
            trimmed(&effects.proc_root.join("sys/kernel/random/boot_id"))
                .map_or(Value::Null, |id| text(&id)),
        ),
        ("pid_namespace", namespace),
        ("uid", int(effects.uid)),
    ])
}

struct Stat {
    pid: i64,
    name: String,
    state: String,
    ppid: i64,
    process_group: i64,
    session: i64,
    start_ticks: i64,
}

impl Stat {
    fn value(&self) -> Value {
        object(vec![
            ("pid", int(self.pid)),
            ("name", text(&self.name)),
            ("state", text(&self.state)),
            ("ppid", int(self.ppid)),
            ("process_group", int(self.process_group)),
            ("session", int(self.session)),
            ("start_ticks", int(self.start_ticks)),
        ])
    }

    fn ended(&self) -> bool {
        matches!(self.state.as_str(), "Z" | "X")
    }
}

/// An error that means the process is gone.
fn gone(error: &io::Error) -> bool {
    error.kind() == io::ErrorKind::NotFound
        || error.raw_os_error() == Some(nix::errno::Errno::ESRCH as i32)
}

fn stat_of(proc_root: &Path, pid: i64) -> io::Result<Option<Stat>> {
    let path = proc_root.join(pid.to_string()).join("stat");
    let raw = match fs::read(&path) {
        Ok(raw) => String::from_utf8_lossy(&raw).into_owned(),
        Err(error) if gone(&error) => return Ok(None),
        Err(error) => return Err(error),
    };
    let malformed = || io::Error::other(format!("{} is not a process status", path.display()));
    let (head, rest) = raw.rsplit_once(')').ok_or_else(malformed)?;
    let name = head.split_once('(').ok_or_else(malformed)?.1.to_string();
    let fields: Vec<&str> = rest.split_whitespace().collect();
    let number = |index: usize| -> io::Result<i64> {
        fields
            .get(index)
            .and_then(|field| field.parse().ok())
            .ok_or_else(malformed)
    };
    Ok(Some(Stat {
        pid,
        name,
        state: fields.first().ok_or_else(malformed)?.to_string(),
        ppid: number(1)?,
        process_group: number(2)?,
        session: number(3)?,
        start_ticks: number(19)?,
    }))
}

fn command_of(proc_root: &Path, pid: i64) -> Vec<String> {
    let Ok(raw) = fs::read(proc_root.join(pid.to_string()).join("cmdline")) else {
        return Vec::new();
    };
    let mut parts: Vec<String> = raw
        .split(|byte| *byte == 0)
        .map(|part| String::from_utf8_lossy(part).into_owned())
        .collect();
    parts.pop();
    parts
}

/// What identifies a process this controller started, for a reader that may find the PID reused.
fn started(proc_root: &Path, pid: u32) -> Vec<(&'static str, Value)> {
    let ticks = stat_of(proc_root, i64::from(pid))
        .ok()
        .flatten()
        .map_or(Value::Null, |stat| int(stat.start_ticks));
    vec![("pid", int(pid)), ("start_ticks", ticks)]
}

/// When a process that started `start_ticks` after boot started, to the millisecond.
fn started_at(proc_root: &Path, start_ticks: i64) -> String {
    let uptime = fs::read_to_string(proc_root.join("uptime"))
        .ok()
        .and_then(|found| {
            found
                .split_whitespace()
                .next()
                .and_then(|field| field.parse::<f64>().ok())
        });
    let ticks = nix::unistd::sysconf(nix::unistd::SysconfVar::CLK_TCK)
        .ok()
        .flatten()
        .filter(|ticks| *ticks > 0);
    let (Some(uptime), Some(per_second)) = (uptime, ticks) else {
        return "at an unknown time".to_string();
    };
    let micros = now_micros() - ((uptime - start_ticks as f64 / per_second as f64) * 1e6) as i64;
    let millis = micros.div_euclid(1000);
    let seconds = u64::try_from(millis.div_euclid(1000)).unwrap_or(0);
    let stamp = utc_stamp(seconds);
    format!(
        "{}.{:03}+00:00",
        stamp.trim_end_matches('Z'),
        millis.rem_euclid(1000)
    )
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty()
        && haystack
            .windows(needle.len())
            .any(|window| window == needle)
}

struct Reading {
    found: Vec<Value>,
    unreadable: Vec<(Stat, String)>,
    wrapper: &'static str,
}

/// One reading of the process table: the processes that belong to the launch, and those that could not be
/// inspected.
fn scan(
    evidence: &Value,
    attempt_dir: Option<&Path>,
    effects: &Effects<'_>,
) -> io::Result<Reading> {
    let proc_root = &effects.proc_root;
    let mut needles: Vec<Vec<u8>> = Vec::new();
    let marker = get(evidence, "marker");
    if marker.truthy() {
        needles.push(format!("{MARKER}={}", marker.py_str()).into_bytes());
    }
    if let Some(directory) = attempt_dir {
        needles.push(directory.as_os_str().as_bytes().to_vec());
        // The reviewer's home, temporary directory and working directory are canonical paths, which a work root
        // with a symbolic link or `..` in it does not spell the same way.
        if let Ok(canonical) = fs::canonicalize(directory)
            && canonical != directory
        {
            needles.push(canonical.into_os_string().into_vec());
        }
    }
    let recorded = integer(get(evidence, "pid")).filter(|pid| *pid != 0);
    let wrapper = match recorded {
        Some(pid) => stat_of(proc_root, pid)?,
        None => None,
    };
    let recorded_start = integer(get(evidence, "start_ticks"));
    let reused = wrapper
        .as_ref()
        .is_some_and(|stat| Some(stat.start_ticks) != recorded_start);
    // Nothing the launch started is older than its wrapper, or than the controller when the wrapper was never
    // recorded.
    let since = [get(evidence, "start_ticks"), get(evidence, "since_ticks")]
        .into_iter()
        .find_map(|value| integer(value).filter(|ticks| *ticks != 0))
        .unwrap_or(0);
    // Launch records written before process groups name the launch's session instead.
    let group = integer(get(evidence, "process_group"))
        .filter(|group| *group != 0)
        .map(|group| (group, false))
        .or_else(|| {
            integer(get(evidence, "session"))
                .filter(|session| *session != 0)
                .map(|session| (session, true))
        });
    // A PID stays allocated while any process uses it as its process group, so a live wrapper with its recorded
    // start time, or a live member that carries the marker or names the attempt directory, shows the group is the
    // launch's own. Without either, the group's number may have been reused by an unrelated group after every
    // process of the launch ended.
    let wrapper_holds_group = wrapper.is_some() && !reused;
    let in_group = |stat: &Stat| {
        !reused
            && group.is_some_and(|(group, session)| {
                (if session {
                    stat.session
                } else {
                    stat.process_group
                }) == group
            })
    };
    let group_reason = || {
        let (group, session) = group.unwrap_or_default();
        let what = if session { "session" } else { "process group" };
        format!("{what} {group}")
    };
    let mut table: BTreeMap<i64, Stat> = BTreeMap::new();
    let mut ours: BTreeMap<i64, String> = BTreeMap::new();
    let mut unreadable = Vec::new();
    let mut group_confirmed = wrapper_holds_group;
    let mut unconfirmed_members: Vec<i64> = Vec::new();
    let mut unreadable_members: Vec<(Stat, String, bool)> = Vec::new();
    for entry in fs::read_dir(proc_root)? {
        let entry = entry?;
        let Some(pid) = entry
            .file_name()
            .to_str()
            .filter(|name| !name.is_empty() && name.bytes().all(|byte| byte.is_ascii_digit()))
            .and_then(|name| name.parse::<i64>().ok())
        else {
            continue;
        };
        let Some(stat) = stat_of(proc_root, pid)? else {
            continue;
        };
        if stat.start_ticks >= since {
            let member = in_group(&stat);
            if recorded == Some(pid) && !reused {
                ours.insert(pid, "the recorded PID and start time".into());
            } else if member && wrapper_holds_group {
                ours.insert(pid, group_reason());
            } else if !stat.ended() && !needles.is_empty() {
                let directory = entry.path();
                let held = fs::read(directory.join("environ")).and_then(|environ| {
                    let command = fs::read(directory.join("cmdline"))?;
                    let cwd = fs::read_link(directory.join("cwd"))?;
                    Ok([environ, command, cwd.into_os_string().into_vec()].join(&0))
                });
                match held {
                    Ok(held) => {
                        if needles.iter().any(|needle| contains(&held, needle)) {
                            ours.insert(pid, "the launch marker or the attempt directory".into());
                            group_confirmed |= member;
                        } else if member {
                            unconfirmed_members.push(pid);
                        }
                    }
                    Err(error) if gone(&error) => continue,
                    Err(error) => {
                        // Another user's process cannot be read and is not this launch's; an unreadable one of ours
                        // might be.
                        let mine = fs::metadata(&directory)
                            .is_ok_and(|found| effects.uid == 0 || found.uid() == effects.uid);
                        if mine || member {
                            let line =
                                pysemantics::file_failure(&error, directory.as_os_str().as_bytes())
                                    .line();
                            if member {
                                unreadable_members.push((stat, line, mine));
                            } else {
                                unreadable.push((stat, line));
                            }
                        }
                        continue;
                    }
                }
            } else if member {
                unconfirmed_members.push(pid);
            }
        }
        table.insert(pid, stat);
    }
    if group_confirmed {
        for pid in unconfirmed_members {
            ours.insert(pid, group_reason());
        }
        for (stat, _, _) in unreadable_members {
            ours.insert(stat.pid, group_reason());
            table.insert(stat.pid, stat);
        }
    } else {
        unreadable.extend(
            unreadable_members
                .into_iter()
                .filter(|(_, _, mine)| *mine)
                .map(|(stat, line, _)| (stat, line)),
        );
    }
    let mut grew = true;
    while grew {
        grew = false;
        for (pid, stat) in &table {
            if !ours.contains_key(pid) && ours.contains_key(&stat.ppid) {
                ours.insert(*pid, format!("child of {}", stat.ppid));
                grew = true;
            }
        }
    }
    let expected: Option<Vec<String>> = get(evidence, "command")
        .as_array()
        .map(|parts| parts.iter().map(Value::py_str).collect());
    let mut found = Vec::new();
    for (pid, why) in &ours {
        let Some(stat) = table.get(pid) else {
            continue;
        };
        let command = command_of(proc_root, *pid);
        // Only the head of a command line is kept: its tail can hold a prompt or a secret.
        let head: String = command
            .iter()
            .take(3)
            .cloned()
            .collect::<Vec<_>>()
            .join(" ")
            .chars()
            .take(160)
            .collect();
        let mut row = merged(
            &stat.value(),
            vec![("matched_by", text(why)), ("command", text(&head))],
        );
        if recorded == Some(*pid) {
            put(
                &mut row,
                "is_expected_command",
                Value::Bool(expected.as_ref() == Some(&command)),
            );
        }
        found.push(row);
    }
    let wrapper = match (recorded, &wrapper) {
        (None, _) => "not recorded",
        (Some(_), None) => "gone",
        (Some(_), Some(_)) if reused => "reused by another process",
        (Some(_), Some(stat)) if stat.ended() => "a zombie",
        _ => "running",
    };
    Ok(Reading {
        found,
        unreadable,
        wrapper,
    })
}

/// What this process can see of a recorded launch: `running`, `absent` or `unknown`.
fn observe(evidence: Option<&Value>, attempt_dir: Option<&Path>, effects: &Effects<'_>) -> Value {
    let here = identity(effects);
    // To the microsecond: the filer refuses an observation dated before the attempt's last output.
    let seen = object(vec![
        ("observed_at", text(&usage::utc_isoformat(now_micros()))),
        ("observer", here.clone()),
    ]);
    let unknown = |seen: &Value, reason: String| {
        merged(
            seen,
            vec![("status", text("unknown")), ("reason", text(&reason))],
        )
    };
    let Some(evidence) = evidence.filter(|evidence| evidence.truthy()) else {
        return unknown(
            &seen,
            "no launch record names this claim, so nothing says where its processes ran".into(),
        );
    };
    let field = |value: &Value, key: &str| get(value, key).clone();
    if !field(&here, "boot_id").truthy() || matches!(field(&here, "pid_namespace"), Value::Null) {
        return unknown(&seen, "this host exposes no process table to read".into());
    }
    let host = field(&here, "host");
    if !field(evidence, "host").py_eq(&host)
        || !field(evidence, "machine_id_sha256").py_eq(&field(&here, "machine_id_sha256"))
    {
        return unknown(
            &seen,
            format!(
                "the launch ran on host {}; observe it there",
                field(evidence, "host").py_str()
            ),
        );
    }
    let boot = field(&here, "boot_id");
    if !field(evidence, "boot_id").truthy() {
        return unknown(
            &seen,
            "the launch record names no boot, so nothing says whether the host has restarted since"
                .into(),
        );
    }
    if !field(evidence, "boot_id").py_eq(&boot) {
        return merged(
            &seen,
            vec![
                ("status", text("absent")),
                ("processes", Value::Array(Vec::new())),
                (
                    "verified_by",
                    text(&format!(
                        "bench-tools queue on {}: the boot id is {}, not the {} the launch ran under, and no \
                         process outlives the boot it started in",
                        host.py_str(),
                        boot.py_str(),
                        field(evidence, "boot_id").py_str()
                    )),
                ),
            ],
        );
    }
    let namespace = field(&here, "pid_namespace");
    if !field(evidence, "pid_namespace").py_eq(&namespace) {
        return unknown(
            &seen,
            format!(
                "the launch ran in PID namespace {}, which this process cannot see into",
                field(evidence, "pid_namespace").py_str()
            ),
        );
    }
    let uid = field(evidence, "uid");
    if effects.uid != 0 && integer(&uid) != Some(i64::from(effects.uid)) {
        return unknown(
            &seen,
            format!(
                "the launch ran as uid {}, whose processes uid {} cannot inspect",
                uid.py_str(),
                effects.uid
            ),
        );
    }
    let began = Instant::now();
    let mut readings = 0;
    let mut clean = 0;
    let (reading, alive, waited) = loop {
        let reading = match scan(evidence, attempt_dir, effects) {
            Ok(reading) => reading,
            Err(error) => {
                return unknown(
                    &seen,
                    format!("the process table could not be read: {error}"),
                );
            }
        };
        readings += 1;
        let alive: Vec<i64> = reading
            .found
            .iter()
            .filter(|row| !matches!(get(row, "state").py_str().as_str(), "Z" | "X"))
            .filter_map(|row| integer(get(row, "pid")))
            .collect();
        let waited = began.elapsed().as_secs_f64();
        clean = if reading.unreadable.is_empty() {
            clean + 1
        } else {
            0
        };
        // A process that forks and exits between two directory reads is missed by one reading, not by two in a row.
        if !alive.is_empty()
            || clean == 2
            || (!reading.unreadable.is_empty() && waited >= effects.unreadable_wait_seconds)
        {
            break (reading, alive, waited);
        }
        (effects.nap)(effects.scan_gap_seconds);
    };
    let seen = merged(
        &seen,
        vec![
            ("processes", Value::Array(reading.found)),
            ("wrapper", text(reading.wrapper)),
        ],
    );
    if !alive.is_empty() {
        let pids: Vec<String> = alive.iter().map(ToString::to_string).collect();
        return merged(
            &seen,
            vec![
                ("status", text("running")),
                (
                    "reason",
                    text(&format!(
                        "live processes of the launch: [{}]",
                        pids.join(", ")
                    )),
                ),
            ],
        );
    }
    if !reading.unreadable.is_empty() {
        let named: Vec<String> = reading
            .unreadable
            .iter()
            .map(|(stat, error)| {
                format!(
                    "{} (pid {}, state {}, started {}): {error}",
                    stat.name,
                    stat.pid,
                    stat.state,
                    started_at(&effects.proc_root, stat.start_ticks)
                )
            })
            .collect();
        return unknown(
            &seen,
            format!(
                "reading {readings} of the process table, {waited:.1} s after the first, could not inspect \
                 processes of this user: {}",
                named.join("; ")
            ),
        );
    }
    let group = if has(evidence, "process_group") {
        format!(
            "process group {}",
            field(evidence, "process_group").py_str()
        )
    } else {
        format!("session {}", field(evidence, "session").py_str())
    };
    merged(
        &seen,
        vec![
            ("status", text("absent")),
            (
                "verified_by",
                text(&format!(
                    "bench-tools queue read the process table of {} (boot {}, PID namespace {}) {readings} times as \
                     uid {}: no live process carried the launch marker, named the attempt directory, belonged to \
                     {group}, or descended from one that did; the recorded PID {} was {}",
                    host.py_str(),
                    boot.py_str(),
                    namespace.py_str(),
                    effects.uid,
                    field(evidence, "pid").py_str(),
                    reading.wrapper
                )),
            ),
        ],
    )
}

fn status_of(seen: &Value) -> String {
    get(seen, "status").py_str()
}

// --- records ---------------------------------------------------------------------------------

fn emit(queue: &Queue, event: Vec<(&str, Value)>) -> Q<()> {
    let mut pairs = vec![("at", text(&utc_now()))];
    pairs.extend(event);
    let line = pyjson::dumps(&object(pairs), &Dumps::default());
    fs::create_dir_all(&queue.state).map_err(os_failure(&queue.state))?;
    append(&queue.state.join("controller.jsonl"), &line)?;
    println!("{line}");
    Ok(())
}

fn launches(queue: &Queue) -> Q<Vec<Value>> {
    let directory = queue.state.join("launches");
    listed(&directory, "launch-", ".json")?
        .iter()
        .map(|name| read_json(&directory.join(name)).map_err(Failure::from))
        .collect()
}

/// The launch whose child made this claim, or None when no record can be tied to it.
fn launch_for(records: &[Value], run: &Run, attempt: &str) -> Option<usize> {
    let marker = run.claimed.get(attempt).map_or(Value::Null, |claim| {
        get(claim, "queue_launch_marker").clone()
    });
    records
        .iter()
        .enumerate()
        .rev()
        .find_map(|(index, record)| {
            let state = get(record, "state").py_str();
            if get(record, "run_id").py_str() != run.id || state == "recovered" {
                return None;
            }
            let matched = if marker.truthy() {
                get(get(record, "process"), "marker").py_eq(&marker)
            } else {
                state == "ended" && get(record, "attempt_id").py_eq(&text(attempt))
            };
            matched.then_some(index)
        })
}

fn launch_path(queue: &Queue, record: &Value) -> PathBuf {
    queue
        .state
        .join("launches")
        .join(format!("{}.json", get(record, "launch_id").py_str()))
}

fn close(
    queue: &Queue,
    record: &mut Value,
    outcome: &str,
    attempt: Option<&str>,
    seen: &Value,
) -> Q<()> {
    let attempt_value = attempt.map_or(Value::Null, text);
    for (key, value) in [
        ("state", text("recovered")),
        ("outcome", text(outcome)),
        ("attempt_id", attempt_value.clone()),
        ("recovered_at", text(&utc_now())),
        ("observation", seen.clone()),
    ] {
        put(record, key, value);
    }
    write(&launch_path(queue, record), record)?;
    emit(
        queue,
        vec![
            ("recovered", get(record, "launch_id").clone()),
            ("run", get(record, "run_id").clone()),
            ("attempt", attempt_value),
            ("outcome", text(outcome)),
        ],
    )
}

/// One controller invocation: its record, its clock and its last meter reading.
struct Session<'q, 'e> {
    queue: &'q Queue,
    effects: &'q Effects<'e>,
    launcher: Launcher,
    usage: Option<Value>,
    usage_read: f64,
    beat: f64,
    id: String,
    path: PathBuf,
    record: Value,
}

impl<'q, 'e> Session<'q, 'e> {
    fn new(queue: &'q Queue, action: &str, effects: &'q Effects<'e>) -> Q<Self> {
        let beat = (effects.clock)();
        let id = format!("{}-{}", compact(&stamp(beat)), std::process::id());
        let path = queue.state.join("invocations").join(format!("{id}.json"));
        let own = std::process::id();
        let mut process = identity(effects);
        for (key, value) in started(&effects.proc_root, own) {
            put(&mut process, key, value);
        }
        put(
            &mut process,
            "command",
            texts(&command_of(&effects.proc_root, i64::from(own))),
        );
        let record = object(vec![
            ("invocation_id", text(&id)),
            ("queue_id", text(&queue.id)),
            ("action", text(action)),
            ("started_at", text(&stamp(beat))),
            ("heartbeat_at", text(&stamp(beat))),
            ("ended_at", Value::Null),
            ("outcome", Value::Null),
            ("process", process),
        ]);
        write(&path, &record)?;
        Ok(Self {
            queue,
            effects,
            launcher: Launcher::default(),
            usage: None,
            usage_read: 0.0,
            beat,
            id,
            path,
            record,
        })
    }

    fn end(&mut self, outcome: &str) -> Q<()> {
        put(
            &mut self.record,
            "ended_at",
            text(&stamp((self.effects.clock)())),
        );
        put(&mut self.record, "outcome", text(outcome));
        write(&self.path, &self.record)
    }

    /// Keep the heartbeat and the status cadence while a review runs; neither may stop the controller.
    fn tick(&mut self) -> Q<()> {
        let moment = (self.effects.clock)();
        let kept = (|| {
            if moment - self.beat >= HEARTBEAT_SECONDS {
                self.beat = moment;
                put(&mut self.record, "heartbeat_at", text(&stamp(moment)));
                write(&self.path, &self.record)?;
            }
            snapshot(self.queue, moment, true, self.effects).map(|_| ())
        })();
        match kept {
            Err(Failure::Input(why) | Failure::Refused(why)) => {
                emit(self.queue, vec![("status_failed", text(&why))])
            }
            Err(Failure::Blocked(reasons)) => {
                emit(self.queue, vec![("status_failed", texts(&reasons))])
            }
            Ok(()) => Ok(()),
        }
    }

    /// The reason the meters stop the queue, from a reading no older than an hour. A sign-in that an earlier
    /// reading shows about to expire is read again, since a person may have renewed it.
    fn quota(&mut self) -> Q<Option<String>> {
        let moment = (self.effects.clock)();
        let stale = match &self.usage {
            None => true,
            Some(usage) => {
                moment - self.usage_read > QUOTA_MAX_AGE_SECONDS
                    || (self.usage_read != moment
                        && subscription_meters::sign_in_expiring(usage, moment))
            }
        };
        if stale {
            let usage = read_meters(self.queue, &self.launcher.env, self.effects)?;
            self.usage_read = moment;
            write(&self.queue.state.join("meters.json"), &usage)?;
            emit(self.queue, vec![("meters", usage.clone())])?;
            self.usage = Some(usage);
        }
        let usage = self.usage.as_ref().unwrap_or(&Value::Null);
        subscription_meters::quota_stop(usage, self.queue.quota_stop, moment, "review")
            .map_err(Failure::Input)
    }

    fn quota_note(&self) -> String {
        let at = self
            .usage
            .as_ref()
            .map_or(Value::Null, |usage| get(usage, "at").clone());
        format!(
            "Subscription meters read at {} by bench-tools queue for queue {}; the reading is in {}.",
            at.py_str(),
            self.queue.id,
            self.queue.state.join("meters.json").display()
        )
    }
}

// --- quota and spend -------------------------------------------------------------------------

/// A reading that says how much of each pinned client's quota is used, or no launch.
fn read_meters(queue: &Queue, env: &[(String, String)], effects: &Effects<'_>) -> Q<Value> {
    let pinned = queue.pinned();
    match (effects.meters)(&pinned, env)
        .and_then(|usage| subscription_meters::checked(&pinned, usage))
    {
        Ok(usage) => Ok(usage),
        Err(Unreadable {
            message,
            sign_in: true,
        }) => input(message),
        Err(Unreadable { message, .. }) => blocked(format!(
            "the subscription meters could not be read, and no launch starts on a reading older than \
             {QUOTA_MAX_AGE_SECONDS:.0} s: {message}"
        )),
    }
}

#[derive(Default)]
struct Spent {
    ceiling: f64,
    total: f64,
    attempts: f64,
    charges: f64,
    in_flight_reserved: f64,
    unknown_attempts: u64,
    unknown_reserved: f64,
}

/// Each client's spend over all its runs, as `run-cell` counts each run's.
fn spend(queue: &Queue, runs: &[Run]) -> Q<Vec<(String, Spent)>> {
    let mut totals: Vec<(String, Spent)> = queue
        .ceilings
        .iter()
        .map(|(client, ceiling)| {
            (
                client.clone(),
                Spent {
                    ceiling: *ceiling,
                    ..Spent::default()
                },
            )
        })
        .collect();
    for run in runs {
        let client = client_of(run)?;
        let Some((_, row)) = totals.iter_mut().find(|(name, _)| name == client) else {
            return input(format!(
                "{} runs on {client}, which the queue gives no ceiling",
                run.id
            ));
        };
        let spent = run.spend()?;
        row.total = usage::round_to(row.total + spent.total, 4);
        row.attempts = usage::round_to(row.attempts + spent.attempts, 4);
        row.charges = usage::round_to(row.charges + spent.charges, 4);
        row.in_flight_reserved =
            usage::round_to(row.in_flight_reserved + spent.in_flight_reserved, 4);
        for (attempt, record) in &run.filed {
            if matches!(get(get(record, "usage"), "priced_total_usd"), Value::Null) {
                row.unknown_attempts += 1;
                row.unknown_reserved =
                    usage::round_to(row.unknown_reserved + run.reserved(attempt)?, 4);
            }
        }
    }
    Ok(totals)
}

fn spend_value(totals: &[(String, Spent)]) -> Value {
    Value::Object(
        totals
            .iter()
            .map(|(client, row)| {
                (
                    client.clone(),
                    object(vec![
                        ("ceiling", Value::Float(row.ceiling)),
                        ("total", Value::Float(row.total)),
                        ("attempts", Value::Float(row.attempts)),
                        ("charges", Value::Float(row.charges)),
                        ("in_flight_reserved", Value::Float(row.in_flight_reserved)),
                        ("unknown_attempts", int(row.unknown_attempts)),
                        ("unknown_reserved", Value::Float(row.unknown_reserved)),
                    ]),
                )
            })
            .collect(),
    )
}

fn fits(queue: &Queue, runs: &[Run], run: &Run, arm: &str) -> Q<()> {
    let client = client_of(run)?;
    let totals = spend(queue, runs)?;
    let Some((_, row)) = totals.iter().find(|(name, _)| name == client) else {
        return input(format!(
            "{} runs on {client}, which the queue gives no ceiling",
            run.id
        ));
    };
    let bound = run.attempt_bound(arm)?;
    if row.total + bound > row.ceiling + 1e-9 {
        return blocked(format!(
            "{client} spend {:.2} + bound {bound:.2} exceeds the queue ceiling {:.2}",
            row.total, row.ceiling
        ));
    }
    Ok(())
}

// --- what blocks a launch --------------------------------------------------------------------

fn disposition(record: &Value) -> String {
    get(record, "disposition").py_str()
}

fn is_valid(record: &Value) -> bool {
    disposition(record) == "valid completed"
}

fn arm_of(record: &Value) -> String {
    get(get(record, "cell"), "arm").py_str()
}

/// Filed valid attempts whose clone or cache remains, or whose cleanup receipt is missing.
fn unpruned(run: &Run) -> Q<Vec<String>> {
    let mut left = Vec::new();
    for (attempt, record) in &run.filed {
        let workspace = run.work.join(attempt);
        if !is_valid(record) || !exists(&workspace)? {
            continue;
        }
        let receipt = workspace.join("workspace-pruned.json");
        if !is_file(&receipt)?
            || !matches!(get(&read_json(&receipt)?, "applied"), Value::Bool(true))
            || exists(&workspace.join("clone"))?
            || exists(&workspace.join("clone-cache"))?
        {
            left.push(attempt.clone());
        }
    }
    Ok(left)
}

struct Diagnosis {
    cause: String,
    reason: String,
    path: PathBuf,
}

fn diagnosis(run: &Run, attempt: &str) -> Q<Option<Diagnosis>> {
    let directory = run.dir.join("deviations");
    let mut found = Vec::new();
    for name in listed(&directory, "", ".json")? {
        let path = directory.join(name);
        let Ok(row) = fs::read(&path)
            .map_err(|_| ())
            .and_then(|raw| pytext::decode_utf8(&raw).map_err(|_| ()))
            .and_then(|raw| pyjson::loads(&raw, Duplicates::KeepLast).map_err(|_| ()))
        else {
            continue;
        };
        if row.as_object().is_some()
            && get(&row, "predecessor").py_eq(&text(attempt))
            && has(&row, "cause")
        {
            found.push((row, path));
        }
    }
    if found.len() > 1 {
        return blocked(format!(
            "{}/{attempt} has {} diagnoses in deviations/; keep one",
            run.id,
            found.len()
        ));
    }
    let Some((row, path)) = found.pop() else {
        return Ok(None);
    };
    let cause = get(&row, "cause").as_str().unwrap_or("").to_string();
    let reason = get(&row, "reason");
    let reason = if reason.truthy() {
        reason.py_str()
    } else {
        String::new()
    };
    if !known_cause(&cause) || pytext::strip(&reason).is_empty() {
        return blocked(format!(
            "{}: a diagnosis gives a reason and a cause among {CAUSES}",
            path.display()
        ));
    }
    Ok(Some(Diagnosis {
        cause,
        reason,
        path,
    }))
}

/// Whether the filed attempt captured a model response: the client and the model had worked before it stopped.
fn answered(run: &Run, attempt: &str) -> Q<bool> {
    let requests = run
        .dir
        .join("attempts")
        .join(attempt)
        .join("usage-requests.jsonl");
    Ok(is_file(&requests)?
        && String::from_utf8_lossy(&fs::read(&requests).map_err(os_failure(&requests))?)
            .lines()
            .any(|line| !pytext::strip(line).is_empty()))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Verdict {
    Undiagnosed,
    Kept,
    StopsArm,
    Replace,
}

impl Verdict {
    fn name(self) -> &'static str {
        match self {
            Self::Undiagnosed => "undiagnosed",
            Self::Kept => "kept",
            Self::StopsArm => "stops-arm",
            Self::Replace => "replace",
        }
    }
}

/// What a failed attempt with no successor means for the queue.
fn verdict(run: &Run, attempt: &str) -> Q<(Verdict, String)> {
    let record = &run.filed[attempt];
    let Some(found) = diagnosis(run, attempt)? else {
        return Ok((
            Verdict::Undiagnosed,
            format!(
                "{} has no diagnosis in deviations/",
                get(record, "disposition").py_repr()
            ),
        ));
    };
    let arm = arm_of(record);
    let proven = answered(run, attempt)?
        || run
            .filed
            .values()
            .any(|row| arm_of(row) == arm && is_valid(row));
    if let Some(what) = kept(&found.cause) {
        return Ok((
            Verdict::Kept,
            format!("{what} is the cell's result, not a rerun opportunity"),
        ));
    }
    if found.cause == "setup-rejection" || (found.cause == "transient-capacity" && !proven) {
        return Ok((
            Verdict::StopsArm,
            format!("{arm} was rejected before any call of it succeeded, so the arm stops"),
        ));
    }
    Ok((
        Verdict::Replace,
        format!(
            "diagnosed as {}; replace it before the queue goes on",
            found.cause
        ),
    ))
}

struct Failed {
    run: String,
    attempt: String,
    arm: String,
    disposition: String,
    verdict: Verdict,
    detail: String,
}

impl Failed {
    fn value(&self) -> Value {
        object(vec![
            ("run", text(&self.run)),
            ("attempt", text(&self.attempt)),
            ("arm", text(&self.arm)),
            ("disposition", text(&self.disposition)),
            ("verdict", text(self.verdict.name())),
            ("detail", text(&self.detail)),
        ])
    }
}

/// The failed attempts with no successor, each with its verdict.
fn failures(run: &Run) -> Q<Vec<Failed>> {
    let predecessors: Vec<&Value> = run
        .filed
        .values()
        .chain(run.claimed.values())
        .map(|record| get(record, "predecessor"))
        .collect();
    let mut rows = Vec::new();
    for (attempt, record) in &run.filed {
        if is_valid(record)
            || predecessors
                .iter()
                .any(|predecessor| predecessor.py_eq(&text(attempt)))
        {
            continue;
        }
        let (verdict, detail) = verdict(run, attempt)?;
        rows.push(Failed {
            run: run.id.clone(),
            attempt: attempt.clone(),
            arm: arm_of(record),
            disposition: disposition(record),
            verdict,
            detail,
        });
    }
    Ok(rows)
}

type Stopped = BTreeMap<String, BTreeMap<String, String>>;

/// The first cell of the sealed order with no attempt whose arm has not stopped.
fn next_cell(run: &Run, stopped: &Stopped) -> Q<Option<String>> {
    let attempted: BTreeSet<String> = run
        .attempt_ids()
        .iter()
        .map(|attempt| run.cell_of(attempt))
        .collect();
    for key in get(&run.manifest, "sealed_order").as_array().unwrap_or(&[]) {
        let key = key.py_str();
        if attempted.contains(&key) {
            continue;
        }
        let arm = get(&run_cell::parse_key(&key)?, "arm").py_str();
        if !stopped
            .get(&run.id)
            .is_some_and(|arms| arms.contains_key(&arm))
        {
            return Ok(Some(key));
        }
    }
    Ok(None)
}

struct Ran {
    code: i32,
    output: String,
}

fn exit_code(status: std::process::ExitStatus) -> i32 {
    status
        .code()
        .unwrap_or_else(|| -status.signal().unwrap_or(0))
}

fn runner_command(launcher: &Launcher, run: &Run, arguments: &[String]) -> Vec<String> {
    let mut command = launcher.runner.clone();
    command.extend([
        "--run".to_string(),
        display(&run.dir),
        "--work".to_string(),
        display(&run.work),
    ]);
    command.extend(arguments.iter().cloned());
    command
}

fn cell(launcher: &Launcher, run: &Run, arguments: &[String], effects: &Effects<'_>) -> Q<Ran> {
    let command = runner_command(launcher, run, arguments);
    let done = Command::new(&command[0])
        .args(&command[1..])
        .current_dir(&effects.repo)
        .env_clear()
        .envs(launcher.env.iter().map(|(key, value)| (key, value)))
        .stdin(Stdio::null())
        .output()
        .map_err(|error| Failure::Input(format!("cannot run {}: {error}", command[0])))?;
    Ok(Ran {
        code: exit_code(done.status),
        output: format!(
            "{}{}",
            String::from_utf8_lossy(&done.stdout),
            String::from_utf8_lossy(&done.stderr)
        ),
    })
}

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(ToString::to_string).collect()
}

/// File or release one unfiled claim whose processes were observed gone, and say what was done.
fn settle(
    queue: &Queue,
    launcher: &Launcher,
    run: &Run,
    attempt: &str,
    seen: &Value,
    effects: &Effects<'_>,
) -> Q<String> {
    let directory = run.work.join(attempt);
    let observed = get(seen, "observed_at").py_str();
    let digits: String = observed
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == 'T')
        .collect();
    let evidence = queue
        .state
        .join("recoveries")
        .join(format!("{}-{attempt}-{digits}.json", run.id));
    write(
        &evidence,
        &object(vec![
            ("run_id", text(&run.id)),
            ("attempt_id", text(attempt)),
            ("observed_at", text(&observed)),
            ("status", text("absent")),
            ("verified_by", get(seen, "verified_by").clone()),
            ("observation", seen.clone()),
            (
                "claim",
                run.claimed.get(attempt).cloned().unwrap_or(Value::Null),
            ),
        ]),
    )?;
    let dispatched = directory.join("dispatch.txt");
    let ended = is_file(&dispatched)?
        && ENDED.is_match(&String::from_utf8_lossy(
            &fs::read(&dispatched).map_err(os_failure(&dispatched))?,
        ));
    let shown = display(&evidence);
    let (action, arguments) = if !exists(&directory.join("timing.json"))? {
        (
            "released: no reviewer had started",
            strings(&["--release", attempt, "--interrupted", &shown]),
        )
    } else if ended {
        (
            "filed: the dispatch had ended",
            strings(&["--file", attempt]),
        )
    } else {
        (
            "filed as interrupted",
            strings(&[
                "--file",
                attempt,
                "--interrupted",
                &shown,
                "--reason",
                "the controller stopped and the dispatch never returned; its processes were observed gone",
            ]),
        )
    };
    let done = cell(launcher, run, &arguments, effects)?;
    let after = Run::open(&run.dir, &run.work)?;
    if after.in_flight().iter().any(|claimed| claimed == attempt) {
        return blocked(format!(
            "{}/{attempt}: run-cell {} exit {}: {}",
            run.id,
            arguments[..2].join(" "),
            done.code,
            tail(pytext::strip(&done.output), 600)
        ));
    }
    Ok(action.to_string())
}

struct Assessed {
    blocked: Vec<String>,
    failed: Vec<Failed>,
    runs: Vec<Run>,
}

fn run_named<'r>(runs: &'r [Run], record: &Value) -> Q<&'r Run> {
    let id = get(record, "run_id").py_str();
    runs.iter().find(|run| run.id == id).map_or_else(
        || {
            input(format!(
                "{} names run {id}, which is not in the queue",
                get(record, "launch_id").py_str()
            ))
        },
        Ok,
    )
}

/// What stands between the queue and a new launch. With `act`, settle what verified absence allows first.
fn assess(queue: &Queue, launcher: &Launcher, act: bool, effects: &Effects<'_>) -> Q<Assessed> {
    let mut blocked = Vec::new();
    let mut records = launches(queue)?;
    let runs = queue.runs()?;
    for record in &mut records {
        let state = get(record, "state").py_str();
        if state != "intent" && state != "spawned" {
            continue;
        }
        let run = run_named(&runs, record)?;
        let before = get(record, "attempts_before").clone();
        let new: Vec<String> = run
            .attempt_ids()
            .into_iter()
            .filter(|attempt| {
                !before
                    .as_array()
                    .unwrap_or(&[])
                    .iter()
                    .any(|old| old.py_eq(&text(attempt)))
            })
            .collect();
        let in_flight = run.in_flight();
        if new.iter().any(|attempt| in_flight.contains(attempt)) {
            continue;
        }
        let seen = observe(
            Some(get(record, "process")),
            new.first().map(|attempt| run.work.join(attempt)).as_deref(),
            effects,
        );
        let launch = get(record, "launch_id").py_str();
        if status_of(&seen) != "absent" {
            blocked.push(format!(
                "{launch} of {} was never recorded as ended and its processes are {}: {}",
                run.id,
                status_of(&seen),
                get(&seen, "reason").py_str()
            ));
        } else if act {
            let outcome = if new.is_empty() {
                "no claim was made"
            } else {
                "filed before the controller stopped"
            };
            close(
                queue,
                record,
                outcome,
                new.first().map(String::as_str),
                &seen,
            )?;
        } else {
            blocked.push(format!(
                "{launch} of {} was never recorded as ended; its processes are gone, so recover or run closes it",
                run.id
            ));
        }
    }
    let mut unfiled = BTreeSet::new();
    for run in &runs {
        for attempt in run.in_flight() {
            let index = launch_for(&records, run, &attempt);
            if let Some(index) = index {
                unfiled.insert(get(&records[index], "launch_id").py_str());
            }
            let seen = observe(
                index.map(|index| get(&records[index], "process")),
                Some(&run.work.join(&attempt)),
                effects,
            );
            if status_of(&seen) != "absent" {
                blocked.push(format!(
                    "{}/{attempt} is claimed and not filed, and its processes are {}: {}",
                    run.id,
                    status_of(&seen),
                    get(&seen, "reason").py_str()
                ));
            } else if act {
                let outcome = settle(queue, launcher, run, &attempt, &seen, effects)?;
                match index {
                    Some(index) => {
                        close(queue, &mut records[index], &outcome, Some(&attempt), &seen)?
                    }
                    None => emit(
                        queue,
                        vec![
                            ("recovered", Value::Null),
                            ("run", text(&run.id)),
                            ("attempt", text(&attempt)),
                            ("outcome", text(&outcome)),
                        ],
                    )?,
                }
            } else {
                blocked.push(format!(
                    "{}/{attempt} is claimed and not filed; its processes are gone, so recover or run settles it",
                    run.id
                ));
            }
        }
    }
    for record in &mut records {
        let launch = get(record, "launch_id").py_str();
        if get(record, "state").py_str() != "ended"
            || has(record, "observation")
            || unfiled.contains(&launch)
        {
            continue;
        }
        let run = run_named(&runs, record)?;
        let attempt = get(record, "attempt_id");
        let directory = attempt.truthy().then(|| run.work.join(attempt.py_str()));
        let seen = observe(Some(get(record, "process")), directory.as_deref(), effects);
        if status_of(&seen) != "absent" {
            blocked.push(format!(
                "{launch} of {} ended, and its processes are {}: {}",
                run.id,
                status_of(&seen),
                get(&seen, "reason").py_str()
            ));
        } else if act {
            put(record, "observation", seen);
            write(&launch_path(queue, record), record)?;
        }
    }
    if !blocked.is_empty() {
        return Ok(Assessed {
            blocked,
            failed: Vec::new(),
            runs,
        });
    }
    let current = queue.runs()?;
    let mut failed = Vec::new();
    for run in &current {
        for attempt in unpruned(run)? {
            let pruned = if act {
                run_cell::filing(&run.work.join(&attempt), &|| {}).and_then(|_held| {
                    let target = get(get(&run.filed[&attempt], "cell"), "target").py_str();
                    run_cell::prune(run, effects.prune, &attempt, &target)
                })
            } else {
                Err(run_cell::Failure::Input(format!(
                    "{attempt}: recover or run prunes it when it is eligible"
                )))
            };
            match pruned {
                Ok(()) => {}
                Err(run_cell::Failure::Input(error)) => {
                    blocked.push(format!("{}: cleanup incomplete: {error}", run.id));
                }
                Err(refused) => return Err(refused.into()),
            }
        }
        let over = run.over_reservation()?;
        if !over.is_empty() {
            blocked.push(format!(
                "{}: {} used more than was reserved; nothing in the queue launches until charges.jsonl carries a \
                 line with \"reconciles\" naming each",
                run.id,
                over.join(", ")
            ));
        }
        failed.extend(
            failures(run)?
                .into_iter()
                .filter(|row| row.verdict != Verdict::Kept),
        );
    }
    Ok(Assessed {
        blocked,
        failed,
        runs: current,
    })
}

/// Split the failures into the reasons the queue waits and the arms that stopped, by run.
fn standing(failed: &[Failed]) -> (Vec<String>, Stopped) {
    let mut waits = Vec::new();
    let mut stopped = Stopped::new();
    for row in failed {
        if row.verdict == Verdict::StopsArm {
            stopped
                .entry(row.run.clone())
                .or_default()
                .insert(row.arm.clone(), format!("{}: {}", row.attempt, row.detail));
        } else {
            waits.push(format!("{}/{}: {}", row.run, row.attempt, row.detail));
        }
    }
    (waits, stopped)
}

fn stopped_value(stopped: &Stopped) -> Value {
    Value::Object(
        stopped
            .iter()
            .map(|(run, arms)| {
                (
                    run.clone(),
                    Value::Object(
                        arms.iter()
                            .map(|(arm, why)| (arm.clone(), text(why)))
                            .collect(),
                    ),
                )
            })
            .collect(),
    )
}

// --- launching -------------------------------------------------------------------------------

/// Run one `run-cell` dispatch as a recorded child, and return the run afterwards and the filed record.
fn launch(
    session: &mut Session<'_, '_>,
    run: &Run,
    action: &str,
    arguments: &[String],
) -> Q<(Run, Value)> {
    let queue = session.queue;
    let effects = session.effects;
    let directory = queue.state.join("launches");
    fs::create_dir_all(&directory).map_err(os_failure(&directory))?;
    let used = listed(&directory, "launch-", "")?
        .iter()
        .filter_map(|name| {
            name.split('-')
                .nth(1)
                .and_then(|rest| rest.split('.').next())
                .and_then(|digits| digits.parse::<u64>().ok())
        })
        .max()
        .unwrap_or(0);
    let launch_id = format!("launch-{:04}", used + 1);
    let marker = token();
    let command = runner_command(&session.launcher, run, arguments);
    let mut process = identity(effects);
    for (key, value) in [
        ("marker", text(&marker)),
        ("pid", Value::Null),
        ("start_ticks", Value::Null),
        ("process_group", Value::Null),
        ("command", texts(&command)),
        (
            "since_ticks",
            get(get(&session.record, "process"), "start_ticks").clone(),
        ),
    ] {
        put(&mut process, key, value);
    }
    let before = run.attempt_ids();
    let mut record = object(vec![
        ("launch_id", text(&launch_id)),
        ("queue_id", text(&queue.id)),
        ("invocation_id", text(&session.id)),
        ("run_id", text(&run.id)),
        ("action", text(action)),
        ("attempts_before", texts(&before)),
        ("started_at", text(&utc_now())),
        ("state", text("intent")),
        ("attempt_id", Value::Null),
        ("runtimes", queue.runtimes_value()),
        ("process", process),
    ]);
    let path = directory.join(format!("{launch_id}.json"));
    write(&path, &record)?;
    let output_path = directory.join(format!("{launch_id}.output.txt"));
    let output = File::create(&output_path).map_err(os_failure(&output_path))?;
    let errors = output.try_clone().map_err(os_failure(&output_path))?;
    // A process group of its own, so that what the launch leaves behind can be told from this controller's other
    // children.
    let mut child = Command::new(&command[0])
        .args(&command[1..])
        .current_dir(&effects.repo)
        .env_clear()
        .envs(session.launcher.env.iter().map(|(key, value)| (key, value)))
        .env(MARKER, &marker)
        .stdin(Stdio::null())
        .stdout(output)
        .stderr(errors)
        .process_group(0)
        .spawn()
        .map_err(|error| Failure::Input(format!("cannot start {}: {error}", command[0])))?;
    let pid = child.id();
    if let Value::Object(fields) = &mut record
        && let Some(process) = fields.get_mut("process")
    {
        for (key, value) in started(&effects.proc_root, pid) {
            put(process, key, value);
        }
        put(process, "process_group", int(pid));
    }
    put(&mut record, "state", text("spawned"));
    write(&path, &record)?;
    emit(
        queue,
        vec![
            ("launched", text(&launch_id)),
            ("run", text(&run.id)),
            ("action", text(action)),
            ("pid", int(pid)),
        ],
    )?;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                session.tick()?;
                (effects.sleep)(effects.poll_seconds);
            }
            Err(error) => return input(format!("cannot wait for {launch_id}: {error}")),
        }
    };
    let code = exit_code(status);
    let after = Run::open(&run.dir, &run.work)?;
    let new: Vec<String> = after
        .attempt_ids()
        .into_iter()
        .filter(|attempt| !before.contains(attempt))
        .collect();
    let attempt = match new.as_slice() {
        [only] => Some(only.clone()),
        _ => None,
    };
    put(&mut record, "state", text("ended"));
    put(&mut record, "exit", int(code));
    put(&mut record, "ended_at", text(&utc_now()));
    put(
        &mut record,
        "attempt_id",
        attempt.as_deref().map_or(Value::Null, text),
    );
    write(&path, &record)?;
    let filed = attempt
        .as_ref()
        .and_then(|attempt| after.filed.get(attempt))
        .cloned();
    let raw = fs::read(&output_path).map_err(os_failure(&output_path))?;
    let output = tail(pytext::strip(&String::from_utf8_lossy(&raw)), 1500).to_string();
    emit(
        queue,
        vec![
            ("ended", text(&launch_id)),
            ("run", text(&run.id)),
            ("exit", int(code)),
            ("attempt", attempt.as_deref().map_or(Value::Null, text)),
            (
                "disposition",
                filed
                    .as_ref()
                    .map_or(Value::Null, |row| get(row, "disposition").clone()),
            ),
            ("output", text(&output)),
        ],
    )?;
    match filed {
        Some(filed) if code == 0 => Ok((after, filed)),
        filed => blocked(format!(
            "{}: run-cell exit {code}{} ({launch_id}): {}",
            run.id,
            if filed.is_none() {
                " and filed no attempt"
            } else {
                ""
            },
            tail(&output, 600)
        )),
    }
}

fn run_queue(session: &mut Session<'_, '_>, count: Option<u64>) -> Q<u8> {
    let queue = session.queue;
    let effects = session.effects;
    let mut launched = 0;
    while count.is_none_or(|count| launched < count) {
        session.launcher = verify(queue, &queue.runs()?, true, effects)?;
        let assessed = assess(queue, &session.launcher, true, effects)?;
        let (waits, stopped) = standing(&assessed.failed);
        if !assessed.blocked.is_empty() || !waits.is_empty() {
            return Err(Failure::Blocked([assessed.blocked, waits].concat()));
        }
        let runs = assessed.runs;
        let mut pending = Vec::new();
        for run in &runs {
            if next_cell(run, &stopped)?.is_some() {
                pending.push(run);
            }
        }
        if pending.is_empty() {
            let status = runs
                .iter()
                .map(|run| run_cell::status(run).map_err(Failure::from))
                .collect::<Q<Vec<Value>>>()?;
            emit(
                queue,
                vec![
                    ("completed", Value::Bool(true)),
                    ("stopped_arms", stopped_value(&stopped)),
                    ("status", Value::Array(status)),
                ],
            )?;
            return Ok(0);
        }
        if let Some(stop) = session.quota()? {
            emit(queue, vec![("stopped", text(&stop))])?;
            return Ok(0);
        }
        // Every run files one valid review before any run goes on to its second.
        let run = pending
            .iter()
            .find(|run| !run.filed.values().any(is_valid))
            .unwrap_or(&pending[0]);
        let key = next_cell(run, &stopped)?.unwrap_or_default();
        fits(
            queue,
            &runs,
            run,
            &get(&run_cell::parse_key(&key)?, "arm").py_str(),
        )?;
        let note = session.quota_note();
        let (_, filed) = launch(
            session,
            run,
            "next",
            &strings(&["--cell", &key, "--quota", &note]),
        )?;
        launched += 1;
        if !is_valid(&filed) {
            return blocked(format!(
                "{}: the review filed as {}; save a diagnosis before the queue goes on",
                run.id,
                get(&filed, "disposition").py_repr()
            ));
        }
    }
    Ok(0)
}

fn replace(session: &mut Session<'_, '_>, run_id: &str, attempt: &str) -> Q<u8> {
    let queue = session.queue;
    let effects = session.effects;
    session.launcher = verify(queue, &queue.runs()?, true, effects)?;
    let assessed = assess(queue, &session.launcher, true, effects)?;
    let runs = assessed.runs;
    let Some(run) = runs.iter().find(|run| run.id == run_id) else {
        return input(format!("{run_id} is not a run of queue {}", queue.id));
    };
    if !assessed.blocked.is_empty() {
        return Err(Failure::Blocked(assessed.blocked));
    }
    let Some(record) = run.filed.get(attempt).filter(|record| !is_valid(record)) else {
        return blocked(format!(
            "{}/{attempt} is not a filed failure; a valid attempt is never replaced",
            run.id
        ));
    };
    let successor = run
        .claimed
        .iter()
        .chain(run.filed.iter())
        .find(|(_, row)| get(row, "predecessor").py_eq(&text(attempt)))
        .map(|(other, _)| other.clone());
    if let Some(successor) = successor {
        return blocked(format!(
            "{}/{attempt} already has the replacement {successor}",
            run.id
        ));
    }
    let (kind, detail) = verdict(run, attempt)?;
    if kind != Verdict::Replace {
        return blocked(format!("{}/{attempt}: {detail}", run.id));
    }
    let Some(found) = diagnosis(run, attempt)? else {
        return blocked(format!(
            "{}/{attempt} has no diagnosis in deviations/",
            run.id
        ));
    };
    if let Some(stop) = session.quota()? {
        return blocked(stop);
    }
    fits(queue, &runs, run, &arm_of(record))?;
    let saved = found.path.strip_prefix(&run.dir).unwrap_or(&found.path);
    let reason = format!(
        "{} (diagnosis {}, sha256 {})",
        found.reason,
        saved.display(),
        sha256_file(&found.path)?
    );
    let mut arguments = strings(&[
        "--replace",
        attempt,
        "--reason",
        &reason,
        "--quota",
        &session.quota_note(),
    ]);
    if replaced(&found.cause) == Some(true) {
        arguments.push("--stopped-by-harness".into());
    }
    let (_, filed) = launch(session, run, "replace", &arguments)?;
    if !is_valid(&filed) {
        return blocked(format!(
            "{}: the replacement filed as {}; save a diagnosis before the queue goes on",
            run.id,
            get(&filed, "disposition").py_repr()
        ));
    }
    Ok(0)
}

// --- status ----------------------------------------------------------------------------------

fn report(queue: &Queue, moment: f64, effects: &Effects<'_>) -> Q<Value> {
    let runs = queue.runs()?;
    let records = launches(queue)?;
    let mut rows = Vec::new();
    let mut claims = Vec::new();
    let mut planned = 0;
    let mut valid = 0;
    for run in &runs {
        let mut row = run_cell::status(run)?;
        put(&mut row, "cleanup_incomplete", texts(&unpruned(run)?));
        let unresolved = match failures(run) {
            Ok(rows) => Value::Array(rows.iter().map(Failed::value).collect()),
            Err(Failure::Blocked(reasons)) => texts(&reasons),
            Err(other) => return Err(other),
        };
        put(&mut row, "unresolved_failures", unresolved);
        planned += integer(get(&row, "planned_cells")).unwrap_or(0);
        valid += integer(get(get(&row, "filed"), "valid completed")).unwrap_or(0);
        rows.push(row);
        for attempt in run.in_flight() {
            let index = launch_for(&records, run, &attempt);
            claims.push(object(vec![
                ("run", text(&run.id)),
                ("attempt", text(&attempt)),
                (
                    "launch",
                    index.map_or(Value::Null, |index| {
                        get(&records[index], "launch_id").clone()
                    }),
                ),
                (
                    "process",
                    observe(
                        index.map(|index| get(&records[index], "process")),
                        Some(&run.work.join(&attempt)),
                        effects,
                    ),
                ),
            ]));
        }
    }
    let invocations = listed(&queue.state.join("invocations"), "", ".json")?;
    let controller = match invocations.last() {
        None => Value::Null,
        Some(name) => {
            let last = read_json(&queue.state.join("invocations").join(name))?;
            let beat = moment_of(&get(&last, "heartbeat_at").py_str()).unwrap_or(moment);
            let ended = get(&last, "ended_at");
            let age = (moment - beat).round_ties_even() as i64;
            object(vec![
                ("invocation", get(&last, "invocation_id").clone()),
                ("action", get(&last, "action").clone()),
                ("ended_at", ended.clone()),
                ("outcome", get(&last, "outcome").clone()),
                ("heartbeat_at", get(&last, "heartbeat_at").clone()),
                ("heartbeat_age_seconds", int(age)),
                (
                    "process",
                    if ended.truthy() {
                        Value::Null
                    } else {
                        observe(Some(get(&last, "process")), None, effects)
                    },
                ),
            ])
        }
    };
    let status = queue.state.join("status");
    let saved = listed(&status, "", ".json")?;
    let mut acknowledged = BTreeSet::new();
    let receipts = status.join("acknowledged.jsonl");
    if is_file(&receipts)? {
        let raw = fs::read(&receipts).map_err(os_failure(&receipts))?;
        for line in String::from_utf8_lossy(&raw).lines() {
            if pytext::strip(line).is_empty() {
                continue;
            }
            let row = pyjson::loads(line, Duplicates::KeepLast)
                .map_err(|error| Failure::Input(format!("{}: {error}", receipts.display())))?;
            acknowledged.insert(get(&row, "snapshot").py_str());
        }
    }
    let quota = queue.state.join("meters.json");
    let claimed_launches: Vec<Value> = claims
        .iter()
        .map(|claim| get(claim, "launch").clone())
        .collect();
    let open: Vec<Value> = records
        .iter()
        .filter(|record| {
            let state = get(record, "state").py_str();
            (state == "intent" || state == "spawned")
                && !claimed_launches
                    .iter()
                    .any(|launch| launch.py_eq(get(record, "launch_id")))
        })
        .map(|record| {
            object(vec![
                ("launch", get(record, "launch_id").clone()),
                ("run", get(record, "run_id").clone()),
                (
                    "process",
                    observe(Some(get(record, "process")), None, effects),
                ),
            ])
        })
        .collect();
    let unacknowledged: Vec<&String> = saved
        .iter()
        .filter(|name| !acknowledged.contains(*name))
        .collect();
    Ok(object(vec![
        ("at", text(&stamp(moment))),
        ("queue_id", text(&queue.id)),
        ("planned", int(planned)),
        ("valid", int(valid)),
        ("remaining", int(planned - valid)),
        ("runs", Value::Array(rows)),
        ("spend_usd", spend_value(&spend(queue, &runs)?)),
        ("unfiled_claims", Value::Array(claims)),
        ("open_launches", Value::Array(open)),
        ("controller", controller),
        (
            "quota",
            if is_file(&quota)? {
                read_json(&quota)?
            } else {
                Value::Null
            },
        ),
        (
            "snapshots",
            object(vec![
                ("saved", int(saved.len())),
                (
                    "unacknowledged",
                    Value::Array(unacknowledged.iter().map(|name| text(name)).collect()),
                ),
            ]),
        ),
        (
            "delivery",
            text(
                "a snapshot is a local file; nothing here sends it, and a delivery is recorded with --acknowledge",
            ),
        ),
    ]))
}

/// Save the status report. With `due_only`, only when the newest saved one is 30 minutes old.
fn snapshot(
    queue: &Queue,
    moment: f64,
    due_only: bool,
    effects: &Effects<'_>,
) -> Q<Option<PathBuf>> {
    let directory = queue.state.join("status");
    let saved = listed(&directory, "", ".json")?;
    if due_only && let Some(newest) = saved.last() {
        let at = get(&read_json(&directory.join(newest))?, "at").py_str();
        let newest = moment_of(&at).ok_or_else(|| {
            Failure::Input(format!("{newest}: {} is not a time", pytext::repr_str(&at)))
        })?;
        if moment - newest < SNAPSHOT_SECONDS {
            return Ok(None);
        }
    }
    let name = format!("{}.json", compact(&stamp(moment)));
    let path = directory.join(&name);
    write(&path, &report(queue, moment, effects)?)?;
    emit(queue, vec![("snapshot", text(&name))])?;
    Ok(Some(path))
}

fn acknowledge(queue: &Queue, name: &str, note: Option<&str>) -> Q<()> {
    let directory = queue.state.join("status");
    if !directory.join(name).is_file() {
        return input(format!("no saved snapshot {name}"));
    }
    let note = note.unwrap_or("");
    if pytext::strip(note).is_empty() {
        return input("--acknowledge needs --note saying how the snapshot was delivered");
    }
    let line = pyjson::dumps(
        &object(vec![
            ("snapshot", text(name)),
            ("acknowledged_at", text(&utc_now())),
            ("note", text(note)),
        ]),
        &Dumps::default(),
    );
    append(&directory.join("acknowledged.jsonl"), &line)
}

// --- commands --------------------------------------------------------------------------------

fn check(queue: &Queue, effects: &Effects<'_>) -> Q<Outcome> {
    let _lock = serial(queue)?;
    let launcher = verify(queue, &queue.runs()?, false, effects)?;
    let assessed = assess(queue, &launcher, false, effects)?;
    let (waits, stopped) = standing(&assessed.failed);
    let mut blocked = [assessed.blocked, waits].concat();
    if blocked.is_empty() {
        for run in &assessed.runs {
            if let Some(key) = next_cell(run, &stopped)? {
                let done = cell(
                    &launcher,
                    run,
                    &strings(&["--cell", &key, "--dry-run"]),
                    effects,
                )?;
                if done.code != 0 {
                    blocked.push(format!(
                        "{}: {}",
                        run.id,
                        tail(pytext::strip(&done.output), 600)
                    ));
                }
            }
        }
    }
    let usage = read_meters(queue, &launcher.env, effects)?;
    let stop =
        subscription_meters::quota_stop(&usage, queue.quota_stop, (effects.clock)(), "review")
            .map_err(Failure::Input)?;
    let printed = object(vec![
        ("ready", Value::Bool(blocked.is_empty())),
        ("blocked", texts(&blocked)),
        ("stopped_arms", stopped_value(&stopped)),
        ("quota_stop", stop.as_deref().map_or(Value::Null, text)),
        ("meters", usage),
        ("spend_usd", spend_value(&spend(queue, &assessed.runs)?)),
    ]);
    Ok(Outcome {
        code: u8::from(!blocked.is_empty()),
        printed: Some(printed),
    })
}

fn detach(queue: &Queue, count: Option<u64>, effects: &Effects<'_>) -> Q<Outcome> {
    if let Some(problem) = volatile(&queue.state, &effects.proc_root) {
        return blocked(format!(
            "the state directory is not persistent storage: {problem}"
        ));
    }
    let suffix: String = token().chars().take(8).collect();
    let log = queue.state.join("logs").join(format!(
        "run-{}-{suffix}.log",
        compact(&stamp((effects.clock)()))
    ));
    let logs = queue.state.join("logs");
    fs::create_dir_all(&logs).map_err(os_failure(&logs))?;
    let mut command = vec![
        display(&effects.executable),
        "queue".to_string(),
        "run".to_string(),
        "--queue".to_string(),
        display(&queue.path),
    ];
    if let Some(count) = count {
        command.extend(["--count".to_string(), count.to_string()]);
    }
    let output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&log)
        .map_err(os_failure(&log))?;
    let errors = output.try_clone().map_err(os_failure(&log))?;
    let child = Command::new(&command[0])
        .args(&command[1..])
        .current_dir(&effects.repo)
        .stdin(Stdio::null())
        .stdout(output)
        .stderr(errors)
        .process_group(0)
        .spawn()
        .map_err(|error| Failure::Input(format!("cannot start {}: {error}", command[0])))?;
    let mut detached = identity(effects);
    for (key, value) in started(&effects.proc_root, child.id()) {
        put(&mut detached, key, value);
    }
    put(&mut detached, "command", texts(&command));
    Ok(Outcome {
        code: 0,
        printed: Some(object(vec![
            ("detached", detached),
            ("log", text(&display(&log))),
            (
                "survives",
                text("the launching shell; not a host or sandbox restart"),
            ),
        ])),
    })
}

/// Run `work` as the one controller of the queue, with its invocation recorded from start to end.
fn controlled(
    queue: &Queue,
    action: &str,
    effects: &Effects<'_>,
    work: impl FnOnce(&mut Session<'_, '_>) -> Q<u8>,
) -> Q<Outcome> {
    let _lock = serial(queue)?;
    if let Some(problem) = volatile(&queue.state, &effects.proc_root) {
        return blocked(format!(
            "the state directory is not persistent storage: {problem}"
        ));
    }
    let mut session = Session::new(queue, action, effects)?;
    match work(&mut session) {
        Ok(code) => {
            session.end(&format!("exit {code}"))?;
            Ok(Outcome {
                code,
                printed: None,
            })
        }
        Err(Failure::Blocked(reasons)) => {
            let emitted = emit(queue, vec![("blocked", texts(&reasons))]);
            session.end("blocked")?;
            emitted?;
            Ok(Outcome {
                code: 1,
                printed: None,
            })
        }
        Err(other) => {
            session.end("failed")?;
            Err(other)
        }
    }
}

fn recover(session: &mut Session<'_, '_>) -> Q<u8> {
    let queue = session.queue;
    session.launcher = verify(queue, &queue.runs()?, true, session.effects)?;
    let assessed = assess(queue, &session.launcher, true, session.effects)?;
    let (waits, stopped) = standing(&assessed.failed);
    emit(
        queue,
        vec![
            ("settled", Value::Bool(assessed.blocked.is_empty())),
            ("blocked", texts(&assessed.blocked)),
            ("waiting_on_diagnosis_or_replacement", texts(&waits)),
            ("stopped_arms", stopped_value(&stopped)),
        ],
    )?;
    Ok(u8::from(!assessed.blocked.is_empty()))
}

#[derive(Clone)]
pub enum Action {
    Check,
    Run {
        count: Option<u64>,
        detach: bool,
    },
    Status {
        snapshot: bool,
        acknowledge: Option<(String, Option<String>)>,
    },
    Recover,
    Replace {
        run: String,
        attempt: String,
    },
}

/// What a command prints at its end, indented, and its exit code; a controller's events are printed as they happen.
pub struct Outcome {
    pub code: u8,
    pub printed: Option<Value>,
}

pub fn command(queue: &Path, action: Action, effects: &Effects<'_>) -> Result<Outcome, Failure> {
    let queue = Queue::open(queue, effects)?;
    match action {
        Action::Check => check(&queue, effects),
        Action::Status {
            snapshot: save,
            acknowledge: receipt,
        } => {
            if let Some((name, note)) = receipt {
                acknowledge(&queue, &name, note.as_deref())?;
            } else if save {
                snapshot(&queue, (effects.clock)(), false, effects)?;
            }
            Ok(Outcome {
                code: 0,
                printed: Some(report(&queue, (effects.clock)(), effects)?),
            })
        }
        Action::Run {
            count,
            detach: true,
        } => detach(&queue, count, effects),
        Action::Run {
            count,
            detach: false,
        } => controlled(&queue, "run", effects, |session| run_queue(session, count)),
        Action::Recover => controlled(&queue, "recover", effects, recover),
        Action::Replace { run, attempt } => controlled(&queue, "replace", effects, |session| {
            replace(session, &run, &attempt)
        }),
    }
}

#[cfg(test)]
mod tests;
