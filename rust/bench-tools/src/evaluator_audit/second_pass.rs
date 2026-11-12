//! `bench-tools evaluator-audit second-pass`: the second grader's pass over an evaluator audit round's sampled
//! batches, under an authorization that pins the plan, the grader, a dollar cap and a quota stop.
//!
//! A committed config holds what differs between rounds. Every step of a batch is a documented bench-tools command:
//! `evaluator-audit claims`, `grade prepare --inventory`, `grade check-prepared --dispatching`,
//! `grade dispatch --max-budget-usd`, `evaluator-audit second` and `regrade archive-attempt`. `grade map` is never
//! run, so no grade changes.

pub mod launch;

use crate::grade::client::Kind;
use crate::normalize_review::utc_stamp;
use crate::pyjson::{Object, Value};
use crate::regrade::{
    self, Authorization, QuotaGate, Usd, checked, create_new, digest, entries, exists, field,
    hex_id, indented, list, optional, read_json, say, shown, text,
};
use crate::subscription_meters::{Pinned, Unreadable};
use crate::{child, runtime_store};
use std::collections::{HashMap, HashSet, VecDeque};
use std::ffi::{OsStr, OsString};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, mpsc};

type R<T> = Result<T, String>;

pub const EXIT_FAILED: u8 = 1;
pub const EXIT_REFUSED: u8 = 2;
pub const EXIT_BUDGET: u8 = 3;
pub const EXIT_QUOTA: u8 = 4;
const CONFIG_CONTRACT: &str = "evaluator-audit-second-pass-config/v1";
const PLAN_CONTRACT: &str = "evaluator-audit-second-pass-plan/v1";
const PREFLIGHT_CONTRACT: &str = "evaluator-audit-second-pass-preflight/v1";
const DEFAULT_QUOTA_STOP: f64 = 90.0;
const METER_WORK: &str = "second-pass session";
/// The variable `grade dispatch` passes on to claude; the second grader runs without fast mode.
pub const FAST_MODE_OFF: &str = "CLAUDE_CODE_DISABLE_FAST_MODE";

/// What a pass does outside its own files, beside what a regrade queue does.
pub trait Effects: regrade::Effects {
    /// Append `$ bench-tools ARGS` to `log`, then run `bench-tools ARGS` from the repository with its stdout and
    /// stderr appended there, stdin closed and `path` as `PATH`; the step's exit code.
    fn tool(&self, args: &[OsString], log: &Path, path: &OsStr) -> std::io::Result<i32>;
    /// Run `bench-tools ARGS` from the repository with its stdout and stderr appended to `log`; the exit code.
    fn driver(&self, args: &[OsString], log: &Path) -> std::io::Result<i32>;
    /// What `bench-tools runtime pin` prints.
    fn runtime_pin(&self) -> R<Value>;
    fn sleep(&self, seconds: f64);
    /// When the stored Claude sign-in expires, in seconds since 1970; only its `expiresAt` is read.
    fn sign_in_expires(&self) -> Option<f64>;
    /// When this host booted, in seconds since 1970.
    fn booted_at(&self) -> f64;
}

fn joined(args: &[OsString]) -> String {
    args.iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join(" ")
}

impl Effects for regrade::Host {
    fn tool(&self, args: &[OsString], log: &Path, path: &OsStr) -> std::io::Result<i32> {
        let mut output = std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(log)?;
        writeln!(output, "$ bench-tools {}", joined(args))?;
        let status = std::process::Command::new(&self.executable)
            .args(args)
            .current_dir(&self.root)
            .stdin(std::process::Stdio::null())
            .stdout(output.try_clone()?)
            .stderr(output)
            .env("PATH", path)
            .env_remove(regrade::HANDED_OVER)
            .status()?;
        Ok(child::exit_code(status))
    }

    fn driver(&self, args: &[OsString], log: &Path) -> std::io::Result<i32> {
        let output = std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(log)?;
        let status = std::process::Command::new(&self.executable)
            .args(args)
            .current_dir(&self.root)
            .stdin(std::process::Stdio::null())
            .stdout(output.try_clone()?)
            .stderr(output)
            .env_remove(regrade::HANDED_OVER)
            .status()?;
        Ok(child::exit_code(status))
    }

    fn runtime_pin(&self) -> R<Value> {
        runtime_store::pin(&runtime_store::default_store()).map_err(|failure| match failure {
            runtime_store::Failure::Refused(message) | runtime_store::Failure::Input(message) => {
                format!("bench-tools runtime pin failed: {message}")
            }
        })
    }

    fn sleep(&self, seconds: f64) {
        std::thread::sleep(std::time::Duration::from_secs_f64(seconds.max(0.0)));
    }

    fn sign_in_expires(&self) -> Option<f64> {
        let home = std::env::var_os("HOME")?;
        let stored = read_json(&Path::new(&home).join(".claude/.credentials.json")).ok()?;
        number(optional(optional(&stored, "claudeAiOauth")?, "expiresAt")?).map(|ms| ms / 1000.0)
    }

    fn booted_at(&self) -> f64 {
        std::fs::read_to_string("/proc/stat")
            .ok()
            .and_then(|stat| {
                stat.lines()
                    .find_map(|line| line.strip_prefix("btime "))
                    .and_then(|seconds| seconds.trim().parse().ok())
            })
            .unwrap_or(0.0)
    }
}

pub(crate) fn number(value: &Value) -> Option<f64> {
    match value {
        Value::Int(digits) => digits.parse().ok(),
        Value::Float(number) => Some(*number),
        _ => None,
    }
}

fn count(value: &Value, key: &str) -> R<usize> {
    match field(value, key)? {
        Value::Int(digits) => digits.parse().ok(),
        _ => None,
    }
    .ok_or_else(|| format!("field '{key}' is not a count"))
}

fn real(value: &Value, key: &str) -> R<f64> {
    number(field(value, key)?).ok_or_else(|| format!("field '{key}' is not a number"))
}

fn string(text: impl Into<String>) -> Value {
    Value::Str(text.into())
}

fn int(number: usize) -> Value {
    Value::Int(number.to_string())
}

fn object(pairs: Vec<(&str, Value)>) -> Value {
    let mut object = Object::default();
    for (key, value) in pairs {
        object.insert(key.to_string(), value);
    }
    Value::Object(object)
}

/// `round(value, places)` as Python rounds a float.
pub(crate) fn rounded(value: f64, places: usize) -> f64 {
    format!("{value:.places$}").parse().unwrap_or(value)
}

fn utc(seconds: f64) -> String {
    utc_stamp(seconds.max(0.0) as u64)
}

/// `{"path": relative, "sha256": ...}` of a file in the repository.
fn pin(root: &Path, relative: &str) -> R<Value> {
    Ok(object(vec![
        ("path", string(relative)),
        ("sha256", string(digest(&root.join(relative))?)),
    ]))
}

fn write_json(path: &Path, value: &Value) -> R<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("cannot create {}: {error}", shown(parent)))?;
    }
    create_new(path, &indented(value))
}

/// Write `value` beside `path` and move it into place.
fn replace(path: &Path, value: &Value) -> R<()> {
    let mut staged = path.as_os_str().to_os_string();
    staged.push(".tmp");
    std::fs::write(&staged, indented(value))
        .and_then(|()| std::fs::rename(&staged, path))
        .map_err(|error| format!("cannot save {}: {error}", shown(path)))
}

fn relative(root: &Path, path: &Path) -> String {
    shown(path.strip_prefix(root).unwrap_or(path))
}

fn free_bytes(root: &Path) -> u64 {
    nix::sys::statvfs::statvfs(root).map_or(0, |stats| {
        #[allow(
            clippy::useless_conversion,
            reason = "statvfs reports 32-bit counts on macOS"
        )]
        let free = u64::from(stats.blocks_available()) * u64::from(stats.fragment_size());
        free
    })
}

/// The launcher's limits: how many batches the trial and each checkpoint measure, how many batches each driver run
/// takes, and the retries a failure known to pass gets.
pub struct LaunchPolicy {
    pub trial_batches: usize,
    pub checkpoint_batches: usize,
    pub chunk_batches: usize,
    pub workers: usize,
    pub week_points_limit: f64,
    pub reset_margin_seconds: f64,
    pub sign_in_margin_seconds: f64,
    pub max_retries_per_batch: usize,
    pub max_retries: usize,
    pub meter_failures_before_stop: usize,
}

/// A round's second-pass config: everything that differs between rounds.
pub struct Config {
    /// The config file, relative to the repository.
    pub path: String,
    value: Value,
    pub round: String,
    /// The plan `plan` writes and the later steps read, relative to the repository.
    pub plan: String,
    regrades: Vec<String>,
    pub state_directory: String,
    archive_root: String,
    cache_root: String,
    plain_queue: String,
    queues: Vec<(String, String)>,
    client: String,
    model: String,
    first_grader: String,
    budget_policy: String,
    pub launch: LaunchPolicy,
}

impl Config {
    pub fn load(root: &Path, path: &Path) -> R<Config> {
        let resolved = path
            .canonicalize()
            .map_err(|error| format!("cannot resolve {}: {error}", shown(path)))?;
        let inside = resolved
            .strip_prefix(root)
            .map_err(|_| format!("the config {} is outside the repository", shown(path)))?;
        let value = read_json(&resolved)?;
        if optional(&value, "contract").and_then(Value::as_str) != Some(CONFIG_CONTRACT) {
            return Err(format!("{} is not a {CONFIG_CONTRACT}", shown(path)));
        }
        let strings = |key: &str| -> R<Vec<String>> {
            list(&value, key)?
                .iter()
                .map(|item| {
                    item.as_str()
                        .map(str::to_string)
                        .ok_or_else(|| format!("field '{key}' holds a value that is not a string"))
                })
                .collect()
        };
        let queues = field(&value, "cacheReplacementQueues")?
            .as_object()
            .ok_or("field 'cacheReplacementQueues' is not an object")?
            .iter()
            .map(|(manifest, queue)| {
                queue
                    .as_str()
                    .map(|queue| (manifest.clone(), queue.to_string()))
                    .ok_or_else(|| format!("the queue of {manifest} is not a string"))
            })
            .collect::<R<_>>()?;
        let grader = field(&value, "grader")?;
        let launch = field(&value, "launch")?;
        let launch = LaunchPolicy {
            trial_batches: count(launch, "trialBatches")?,
            checkpoint_batches: count(launch, "checkpointBatches")?,
            chunk_batches: count(launch, "chunkBatches")?,
            workers: count(launch, "workers")?,
            week_points_limit: real(launch, "weekPointsLimit")?,
            reset_margin_seconds: real(launch, "resetMarginSeconds")?,
            sign_in_margin_seconds: real(launch, "signInMarginSeconds")?,
            max_retries_per_batch: count(launch, "maxRetriesPerBatch")?,
            max_retries: count(launch, "maxRetries")?,
            meter_failures_before_stop: count(launch, "meterFailuresBeforeStop")?,
        };
        Ok(Config {
            path: shown(inside),
            round: text(&value, "round")?.to_string(),
            plan: text(&value, "plan")?.to_string(),
            regrades: strings("regrades")?,
            state_directory: text(&value, "stateDirectory")?.to_string(),
            archive_root: text(&value, "archiveRoot")?.to_string(),
            cache_root: text(&value, "cacheRoot")?.to_string(),
            plain_queue: text(&value, "plainQueue")?.to_string(),
            queues,
            client: text(grader, "client")?.to_string(),
            model: text(grader, "model")?.to_string(),
            first_grader: text(&value, "firstGrader")?.to_string(),
            budget_policy: text(&value, "budgetPolicy")?.to_string(),
            launch,
            value,
        })
    }
}

/// A sampled batch as the plan orders it.
#[derive(Clone, Debug)]
pub(crate) struct Batch {
    pub(crate) run: String,
    pub(crate) target: String,
    cache: Option<String>,
    pub(crate) first_grade_usd: Option<f64>,
}

impl Batch {
    pub(crate) fn of(value: &Value) -> R<Batch> {
        Ok(Batch {
            run: text(value, "run")?.to_string(),
            target: text(value, "target")?.to_string(),
            cache: optional(value, "cacheReplacements")
                .and_then(Value::as_str)
                .map(str::to_string),
            first_grade_usd: optional(value, "firstGradeUsd").and_then(number),
        })
    }

    pub(crate) fn id(&self) -> String {
        format!("{}/{}", self.run, self.target)
    }

    pub(crate) fn run_name(&self) -> &str {
        Path::new(&self.run)
            .file_name()
            .and_then(OsStr::to_str)
            .unwrap_or(&self.run)
    }

    /// `<root>/<run name>/<target>`, where a batch's attempts or assessments are kept.
    pub(crate) fn under(&self, root: &Path) -> PathBuf {
        root.join(self.run_name()).join(&self.target)
    }
}

/// Whether the batch has a saved second assessment.
pub(crate) fn saved(record_root: &Path, batch: &Batch) -> R<bool> {
    for assessment in entries(&batch.under(&record_root.join("second")))? {
        let named = assessment
            .file_name()
            .and_then(OsStr::to_str)
            .is_some_and(|name| name.starts_with("assessment-"));
        if named && exists(&assessment.join("receipt.json"))? {
            return Ok(true);
        }
    }
    Ok(false)
}

/// A batch's numbered attempts in `<queue>/attempts`, in order.
pub(crate) fn attempts(queue: &Path, batch: &Batch) -> R<(PathBuf, Vec<PathBuf>)> {
    let directory = batch.under(&queue.join("attempts"));
    let mut found = Vec::new();
    for path in entries(&directory)? {
        let Some(name) = path.file_name().and_then(OsStr::to_str) else {
            continue;
        };
        if let Some(number) = name.strip_prefix("attempt-") {
            let number: u64 = number
                .parse()
                .map_err(|_| format!("{} is not a numbered attempt", shown(&path)))?;
            found.push((number, path));
        }
    }
    found.sort();
    Ok((directory, found.into_iter().map(|(_, path)| path).collect()))
}

/// A plan `plan` wrote, once its sample and cache replacement manifests still hash to their pins.
struct Plan {
    batches: Vec<Batch>,
    sample: Value,
    grades: Value,
    grades_path: String,
    record_root: PathBuf,
    queue: PathBuf,
    workspaces: PathBuf,
    archive_root: String,
    cache_root: PathBuf,
    manifests: HashMap<String, String>,
}

fn load_plan(root: &Path, config: &Config, relative: &str) -> R<Plan> {
    let value = read_json(&root.join(relative))?;
    if optional(&value, "contract").and_then(Value::as_str) != Some(PLAN_CONTRACT)
        || optional(&value, "round").and_then(Value::as_str) != Some(config.round.as_str())
    {
        return Err(format!(
            "{relative} is not a second-pass plan of {}",
            config.round
        ));
    }
    let sample = field(&value, "sample")?.clone();
    checked(root, &sample)?;
    let mut manifests = HashMap::new();
    for (name, manifest) in field(&value, "cacheReplacements")?
        .as_object()
        .ok_or("field 'cacheReplacements' is not an object")?
        .iter()
    {
        checked(root, manifest)?;
        manifests.insert(name.clone(), text(manifest, "path")?.to_string());
    }
    let grades = field(&value, "grades")?.clone();
    Ok(Plan {
        batches: list(&value, "order")?
            .iter()
            .map(Batch::of)
            .collect::<R<_>>()?,
        grades_path: text(&grades, "path")?.to_string(),
        sample,
        grades,
        record_root: root.join(text(&value, "recordRoot")?),
        queue: root.join(text(&value, "queueDirectory")?),
        workspaces: root.join(text(&value, "workspaceRoot")?),
        archive_root: text(&value, "archiveRoot")?.to_string(),
        cache_root: root.join(text(&value, "cacheRoot")?),
        manifests,
    })
}

impl Plan {
    fn replacement_args(&self, batch: &Batch) -> R<Vec<OsString>> {
        let mut args: Vec<OsString> = vec!["--cache-root".into(), self.cache_root.clone().into()];
        if let Some(name) = &batch.cache {
            let manifest = self
                .manifests
                .get(name)
                .ok_or_else(|| format!("the plan pins no {name} cache replacement manifest"))?;
            args.extend(["--cache-replacements".into(), manifest.into()]);
        }
        Ok(args)
    }
}

// plan

/// The list price of the batch's latest first grade by the config's first grader, which the launcher scales its
/// trial by.
fn first_grade_usd(root: &Path, config: &Config, run: &str, target: &str) -> R<Value> {
    let batch = Batch {
        run: run.to_string(),
        target: target.to_string(),
        cache: None,
        first_grade_usd: None,
    };
    let mut latest: Option<(String, Value)> = None;
    for assessment in entries(&batch.under(&root.join("bench/grading/current/assessments")))? {
        let named = assessment
            .file_name()
            .and_then(OsStr::to_str)
            .is_some_and(|name| name.starts_with("assessment-"));
        let receipt = assessment.join("receipt.json");
        if !named || !exists(&receipt)? {
            continue;
        }
        let provenance = field(&read_json(&receipt)?, "provenance")?.clone();
        if optional(&provenance, "model").and_then(Value::as_str)
            != Some(config.first_grader.as_str())
        {
            continue;
        }
        let at = text(&provenance, "dispatched_at")?.to_string();
        if latest.as_ref().is_none_or(|(seen, _)| at > *seen) {
            let high = field(field(&provenance, "usage")?, "high")?.clone();
            latest = Some((at, high));
        }
    }
    Ok(latest.map_or(Value::Null, |(_, high)| high))
}

/// Order the sample's batches, each with the cache replacement manifest of the regrade queue that graded it; the
/// line to print.
pub fn plan(root: &Path, config: &Config, out: &Path) -> R<String> {
    let sample_path = format!("bench/grading/current/audit/{}/sample.json", config.round);
    let sample = read_json(&root.join(&sample_path))?;
    let mut authorizations = Vec::new();
    for folder in &config.regrades {
        for path in entries(&root.join(folder))? {
            let name = path.file_name().and_then(OsStr::to_str).unwrap_or("");
            if name.len() >= "authorization..json".len()
                && name.starts_with("authorization.")
                && name.ends_with(".json")
            {
                authorizations.push(path);
            }
        }
    }
    authorizations.sort_by(|left, right| {
        let parts = |path: &Path| {
            path.components()
                .map(|part| part.as_os_str().as_encoded_bytes().to_vec())
                .collect::<Vec<_>>()
        };
        parts(left).cmp(&parts(right))
    });
    let mut manifests_of: HashMap<(String, String), HashSet<Option<String>>> = HashMap::new();
    for path in authorizations {
        let authorization = read_json(&path)?;
        let Some(execution) = authorization
            .as_object()
            .and_then(|object| object.get("executionPlan"))
        else {
            continue;
        };
        let manifest = optional(&authorization, "cacheReplacements")
            .filter(|manifest| manifest.truthy())
            .map(|manifest| text(manifest, "path").map(str::to_string))
            .transpose()?;
        for entry in list(&read_json(&root.join(text(execution, "path")?))?, "order")? {
            manifests_of
                .entry((
                    text(entry, "run")?.to_string(),
                    text(entry, "target")?.to_string(),
                ))
                .or_default()
                .insert(manifest.clone());
        }
    }
    let mut manifests = Object::default();
    let mut order = Vec::new();
    let mut units = 0;
    for batch in list(&sample, "batches")? {
        let (run, target) = (text(batch, "run")?, text(batch, "target")?);
        let found = manifests_of
            .get(&(run.to_string(), target.to_string()))
            .cloned()
            .unwrap_or_default();
        if found.len() != 1 {
            let mut named: Vec<String> = found
                .iter()
                .map(|manifest| manifest.clone().unwrap_or_else(|| "None".into()))
                .collect();
            named.sort();
            let named = if named.is_empty() {
                "no".to_string()
            } else {
                format!("{named:?}")
            };
            return Err(format!(
                "{run}/{target} was regraded under {named} cache manifest; expected exactly one"
            ));
        }
        let manifest = found.into_iter().next().flatten();
        let queue = match &manifest {
            None => config.plain_queue.clone(),
            Some(path) => config
                .queues
                .iter()
                .find(|(known, _)| known == path)
                .map(|(_, queue)| queue.clone())
                .ok_or_else(|| format!("the config names no queue for the manifest {path}"))?,
        };
        if let Some(path) = &manifest {
            manifests.insert(queue.clone(), pin(root, path)?);
        }
        units += count(batch, "units")?;
        order.push(object(vec![
            ("run", string(run)),
            ("target", string(target)),
            ("units", field(batch, "units")?.clone()),
            ("queue", string(&queue)),
            (
                "cacheReplacements",
                manifest.as_ref().map_or(Value::Null, |_| string(&queue)),
            ),
            ("firstGradeUsd", first_grade_usd(root, config, run, target)?),
        ]));
    }
    let planned = order.len();
    let value = object(vec![
        ("contract", string(PLAN_CONTRACT)),
        ("round", string(&config.round)),
        ("sample", pin(root, &sample_path)?),
        ("grades", field(&sample, "grades")?.clone()),
        (
            "recordRoot",
            string(format!("bench/grading/current/audit/{}", config.round)),
        ),
        (
            "queueDirectory",
            string(format!("{}/queue", config.state_directory)),
        ),
        (
            "workspaceRoot",
            string(format!("{}/workspaces", config.state_directory)),
        ),
        ("archiveRoot", string(&config.archive_root)),
        ("cacheRoot", string(&config.cache_root)),
        ("cacheReplacements", Value::Object(manifests)),
        ("order", Value::Array(order)),
    ]);
    write_json(out, &value)?;
    Ok(format!(
        "planned {planned} batches, {units} units: {}",
        shown(out)
    ))
}

// preflight

/// The first `name` on `PATH` that is an executable file, as `shutil.which` finds it.
fn which(name: &str) -> Option<PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|directory| directory.join(name))
        .find(|candidate| {
            std::fs::metadata(candidate)
                .is_ok_and(|found| found.is_file() && found.permissions().mode() & 0o111 != 0)
        })
}

fn disk_bytes(path: &Path) -> Value {
    std::process::Command::new("du")
        .arg("-sb")
        .arg(path)
        .output()
        .ok()
        .and_then(|output| {
            String::from_utf8_lossy(&output.stdout)
                .split_whitespace()
                .next()
                .map(str::to_string)
        })
        .map_or(Value::Null, Value::Int)
}

fn remove_tree(path: &Path) {
    let _ = std::process::Command::new("chmod")
        .args(["-R", "u+w"])
        .arg(path)
        .status();
    let _ = std::fs::remove_dir_all(path);
}

/// The no-charge preflight: every planned batch's claim list is written and its workspace prepared and checked for
/// dispatch, and its input fingerprint must match its grade. The exit code: 0 when every batch passed, else 1.
pub fn preflight(
    effects: &dyn Effects,
    root: &Path,
    config: &Config,
    cli_version: &str,
    out: &Path,
) -> R<u8> {
    let plan = load_plan(root, config, &config.plan)?;
    let scratch = root.join(&config.state_directory).join("preflight");
    if exists(&scratch)? {
        return Err(format!(
            "{} exists; remove it after inspecting it",
            shown(&scratch)
        ));
    }
    std::fs::create_dir_all(&scratch)
        .map_err(|error| format!("cannot create {}: {error}", shown(&scratch)))?;
    let installed = which("claude").ok_or("no claude client is installed")?;
    let resolved = installed
        .canonicalize()
        .map_err(|error| format!("cannot resolve {}: {error}", shown(&installed)))?;
    let inherited = std::env::var_os("PATH").unwrap_or_default();
    let mut record = Object::default();
    for (key, value) in [
        ("contract", string(PREFLIGHT_CONTRACT)),
        ("round", string(&config.round)),
        ("startedAt", string(utc(effects.now()))),
        ("plan", pin(root, &config.plan)?),
        ("sample", plan.sample.clone()),
        ("grades", plan.grades.clone()),
        ("runtimes", effects.runtime_pin()?),
        (
            "client",
            object(vec![
                ("path", string(shown(&installed))),
                ("resolved", string(shown(&resolved))),
                ("version", string(cli_version)),
                ("sha256", string(digest(&resolved)?)),
            ]),
        ),
        (
            "diskReserveGiB",
            string(
                std::env::var("BENCH_DISK_RESERVE_GIB")
                    .unwrap_or_else(|_| "unset (provision default 20)".into()),
            ),
        ),
        ("freeBytesBefore", Value::Int(free_bytes(root).to_string())),
    ] {
        record.insert(key.into(), value);
    }
    let log = scratch.join("preflight.log");
    let mut steps = Vec::new();
    let mut step = |name: &str, args: Vec<OsString>, extra: Vec<(&str, Value)>| -> R<i32> {
        let code = effects
            .tool(&args, &log, &inherited)
            .map_err(|error| format!("cannot run bench-tools {name}: {error}"))?;
        let mut entry = vec![
            ("step", string(name)),
            ("exit", Value::Int(code.to_string())),
        ];
        entry.extend(extra);
        steps.push(object(entry));
        say(&format!("{name}: exit {code}"));
        Ok(code)
    };
    step(
        "grading probe-claude",
        vec!["grading".into(), "probe-claude".into()],
        vec![],
    )?;
    let meters = effects.meters(
        &[Pinned {
            name: "claude".into(),
            path: resolved.clone(),
            version: cli_version.to_string(),
        }],
        &inherited,
    );
    record.insert(
        "meters".into(),
        meters.unwrap_or_else(|error: Unreadable| {
            object(vec![(
                "unreadable",
                string(format!("cannot read the Claude meters: {}", error.message)),
            )])
        }),
    );
    /// A run and its cache replacement queue, which share one `grade preflight`.
    type Group = (String, Option<String>);
    let mut groups: Vec<(Group, Vec<String>)> = Vec::new();
    for batch in &plan.batches {
        let key = (batch.run.clone(), batch.cache.clone());
        match groups.iter_mut().find(|(seen, _)| *seen == key) {
            Some((_, targets)) => targets.push(batch.target.clone()),
            None => groups.push((key, vec![batch.target.clone()])),
        }
    }
    for ((run, replacement), targets) in groups {
        let mut common: Vec<OsString> = vec![
            "--run".into(),
            run.clone().into(),
            "--work-root".into(),
            scratch.join("would-work").into(),
            "--key-root".into(),
            scratch.join("would-keys").into(),
            "--cache-root".into(),
            plan.cache_root.clone().into(),
        ];
        for target in &targets {
            common.extend(["--target".into(), target.into()]);
        }
        if let Some(replacement) = &replacement {
            let manifest = plan.manifests.get(replacement).ok_or_else(|| {
                format!("the plan pins no {replacement} cache replacement manifest")
            })?;
            common.extend(["--cache-replacements".into(), manifest.into()]);
        }
        let named = || {
            vec![
                ("run", string(&run)),
                (
                    "targets",
                    Value::Array(targets.iter().map(string).collect()),
                ),
            ]
        };
        let mut inputs: Vec<OsString> = vec!["grade".into(), "preflight-inputs".into()];
        inputs.extend(common.iter().cloned());
        step("grade preflight-inputs", inputs, named())?;
        let mut dispatch: Vec<OsString> = vec!["grade".into(), "preflight".into()];
        dispatch.extend(common);
        dispatch.extend([
            "--model".into(),
            config.model.clone().into(),
            "--expected-cli-version".into(),
            cli_version.into(),
        ]);
        step("grade preflight", dispatch, named())?;
    }
    let mut graded = HashMap::new();
    for row in list(&read_json(&root.join(&plan.grades_path))?, "batches")? {
        graded.insert(
            (
                text(row, "run")?.to_string(),
                text(row, "target")?.to_string(),
            ),
            field(row, "input_fingerprint")?.clone(),
        );
    }
    let mut batches = Vec::new();
    let mut all_passed = true;
    for (index, batch) in plan.batches.iter().enumerate() {
        let started = effects.elapsed();
        let directory = batch.under(&scratch.join("batches"));
        std::fs::create_dir_all(directory.parent().unwrap_or(&scratch))
            .and_then(|()| std::fs::create_dir(&directory))
            .map_err(|error| format!("cannot create {}: {error}", shown(&directory)))?;
        let work = scratch.join("workspaces").join(hex_id()?);
        let claims = directory.join("claims.json");
        let key = directory.join("key.json");
        let batch_log = directory.join("preflight.log");
        let mut entry = Object::default();
        entry.insert("run".into(), string(&batch.run));
        entry.insert("target".into(), string(&batch.target));
        let mut codes = Object::default();
        let run = |args: Vec<OsString>| -> R<i32> {
            effects
                .tool(&args, &batch_log, &inherited)
                .map_err(|error| format!("cannot run bench-tools: {error}"))
        };
        let code = run(vec![
            "evaluator-audit".into(),
            "claims".into(),
            "--run".into(),
            batch.run.clone().into(),
            "--target".into(),
            batch.target.clone().into(),
            "--out".into(),
            claims.clone().into(),
        ])?;
        codes.insert("claims".into(), Value::Int(code.to_string()));
        let mut prepared_code = None;
        if code == 0 {
            entry.insert("claimsSha256".into(), string(digest(&claims)?));
            let mut listed = 0;
            for items in read_json(&claims)?
                .as_object()
                .map(|object| {
                    object
                        .iter()
                        .map(|(_, value)| value.clone())
                        .collect::<Vec<Value>>()
                })
                .unwrap_or_else(Vec::new)
            {
                for (_, claim) in items.as_object().map(Object::iter).into_iter().flatten() {
                    listed += list(claim, "ids")?.len();
                }
            }
            entry.insert("claims".into(), int(listed));
            let mut prepare: Vec<OsString> = vec![
                "grade".into(),
                "prepare".into(),
                "--run".into(),
                batch.run.clone().into(),
                "--target".into(),
                batch.target.clone().into(),
                "--work".into(),
                work.clone().into(),
                "--key".into(),
                key.clone().into(),
                "--inventory".into(),
                claims.clone().into(),
            ];
            prepare.extend(plan.replacement_args(batch)?);
            let code = run(prepare)?;
            codes.insert("prepare".into(), Value::Int(code.to_string()));
            prepared_code = Some(code);
        }
        let mut matches = false;
        let mut safety = Value::Null;
        if prepared_code == Some(0) {
            let code = run(vec![
                "grade".into(),
                "check-prepared".into(),
                "--work".into(),
                work.clone().into(),
                "--key".into(),
                key.clone().into(),
                "--dispatching".into(),
            ])?;
            codes.insert("check-prepared".into(), Value::Int(code.to_string()));
            let prepared = read_json(&key)?;
            let inputs = read_json(&work.join("validator/inputs.json"))?;
            let fingerprint = field(&prepared, "input_fingerprint")?.clone();
            matches = graded
                .get(&(batch.run.clone(), batch.target.clone()))
                .is_some_and(|graded| graded.py_eq(&fingerprint));
            safety = inputs
                .as_object()
                .and_then(|object| object.get("safety_policy"))
                .cloned()
                .unwrap_or(Value::Null);
            entry.insert("inputFingerprint".into(), fingerprint);
            entry.insert("fingerprintMatchesGrade".into(), Value::Bool(matches));
            entry.insert("safetyPolicy".into(), safety.clone());
            entry.insert("reviews".into(), int(list(&prepared, "reviews")?.len()));
            entry.insert("workspaceBytes".into(), disk_bytes(&work));
        }
        let passed = codes.len() == 3
            && codes.iter().all(|(_, code)| code.py_str() == "0")
            && matches
            && safety.as_str() == Some("no-safety/v1");
        entry.insert("exits".into(), Value::Object(codes));
        entry.insert("passed".into(), Value::Bool(passed));
        entry.insert(
            "seconds".into(),
            Value::Float(rounded(effects.elapsed() - started, 1)),
        );
        if passed {
            remove_tree(&work);
            std::fs::remove_dir_all(&directory)
                .map_err(|error| format!("cannot remove {}: {error}", shown(&directory)))?;
        } else {
            entry.insert(
                "keptForInspection".into(),
                object(vec![
                    ("work", string(relative(root, &work))),
                    ("batch", string(relative(root, &directory))),
                ]),
            );
        }
        all_passed &= passed;
        let seconds = entry.get("seconds").map(Value::py_str).unwrap_or_default();
        batches.push(Value::Object(entry));
        say(&format!(
            "[{}/{}] {}: {} ({seconds}s)",
            index + 1,
            plan.batches.len(),
            batch.id(),
            if passed { "passed" } else { "FAILED" }
        ));
    }
    let steps_passed = steps
        .iter()
        .all(|step| optional(step, "exit").map(Value::py_str).as_deref() == Some("0"));
    let total = batches.len();
    let failed = batches
        .iter()
        .filter(|batch| !optional(batch, "passed").is_some_and(Value::truthy))
        .count();
    let passed = steps_passed && all_passed;
    record.insert("steps".into(), Value::Array(steps));
    record.insert("batches".into(), Value::Array(batches));
    record.insert("completedAt".into(), string(utc(effects.now())));
    record.insert(
        "freeBytesAfter".into(),
        Value::Int(free_bytes(root).to_string()),
    );
    record.insert("passed".into(), Value::Bool(passed));
    write_json(out, &Value::Object(record))?;
    say(&format!(
        "preflight {}: {} of {total} batches; record {}",
        if passed { "passed" } else { "FAILED" },
        total - failed,
        shown(out)
    ));
    if passed {
        std::fs::remove_dir_all(&scratch)
            .map_err(|error| format!("cannot remove {}: {error}", shown(&scratch)))?;
    }
    Ok(if passed { 0 } else { EXIT_FAILED })
}

// authorize

/// Write the round's authorization from its config, once the owner's approval is filed as the ruling receipt the
/// config pins; the line to print. Without that receipt it writes nothing.
pub fn authorize(effects: &dyn Effects, root: &Path, config: &Config) -> R<String> {
    let block = field(&config.value, "authorization")?;
    let approval = optional(block, "approval").ok_or_else(|| {
        format!(
            "no owner approval of round {}'s second grader and spend is filed; save the approval as a ruling receipt \
             and set authorization.approval in {} first",
            config.round, config.path
        )
    })?;
    let receipt = text(approval, "receipt")?;
    let words = text(approval, "text")?;
    let receipt_path = root.join(receipt);
    if !exists(&receipt_path)? || digest(&receipt_path)? != text(approval, "sha256")? {
        return Err(format!(
            "the approval receipt {receipt} is missing or is not the one authorization.approval pins"
        ));
    }
    let filed = std::fs::read_to_string(&receipt_path)
        .map_err(|error| format!("cannot read {receipt}: {error}"))?;
    if !filed.contains(words) || !filed.contains(&config.round) {
        return Err(format!(
            "the approval receipt {receipt} does not hold the owner's words {words:?} about round {}",
            config.round
        ));
    }
    let out = root.join(text(block, "path")?);
    if exists(&out)? {
        return Err(format!(
            "{} exists; an authorization is written once",
            shown(&out)
        ));
    }
    let plan = read_json(&root.join(&config.plan))?;
    let batches = list(&plan, "order")?.len();
    let cap = field(approval, "capUsd")?.clone();
    let cap_usd = Usd::of(&cap)?;
    let consent = object(vec![
        ("speaker", string("user")),
        ("text", string(words)),
        ("context", field(approval, "context")?.clone()),
        ("ref", pin(root, receipt)?),
    ]);
    let grader = field(block, "grader")?;
    let mut deviations = vec![pin(root, &config.path)?];
    for path in list(block, "pinned")? {
        deviations.push(pin(
            root,
            path.as_str().ok_or("a pinned path is not a string")?,
        )?);
    }
    deviations.push(pin(root, receipt)?);
    let quota = field(block, "quotaReceipt")?;
    let policy = &config.launch;
    let value = object(vec![
        ("schemaVersion", int(1)),
        ("scope", field(block, "scope")?.clone()),
        ("budgetPolicy", string(&config.budget_policy)),
        ("budgetCapUsd", cap),
        ("maxBudgetUsd", field(block, "maxBudgetUsd")?.clone()),
        ("budgetReceipt", consent.clone()),
        (
            "grader",
            object(vec![
                ("client", string(&config.client)),
                ("model", string(&config.model)),
                ("effort", field(grader, "effort")?.clone()),
                ("cliVersion", field(grader, "cliVersion")?.clone()),
                ("timeoutSeconds", field(grader, "timeoutSeconds")?.clone()),
                ("selection", consent),
                ("fastMode", field(grader, "fastMode")?.clone()),
                ("client_pin", field(grader, "client_pin")?.clone()),
            ]),
        ),
        ("plan", pin(root, &config.plan)?),
        ("preflight", pin(root, text(block, "preflight")?)?),
        ("runtimes", effects.runtime_pin()?),
        ("runnerDeviations", Value::Array(deviations)),
        (
            "quotaStopPercent",
            field(block, "quotaStopPercent")?.clone(),
        ),
        (
            "quotaReceipt",
            object(vec![
                ("speaker", string("user")),
                ("text", field(quota, "text")?.clone()),
                ("ref", pin(root, text(quota, "path")?)?),
            ]),
        ),
        (
            "runner",
            object(vec![
                ("command", string("bench-tools evaluator-audit second-pass")),
                (
                    "workspaceNames",
                    string(
                        "random: each attempt's workspace is a new uuid4 hex name under the plan's workspaceRoot",
                    ),
                ),
                ("retriesPerBatch", int(policy.max_retries_per_batch)),
                ("maxRetries", int(policy.max_retries)),
            ]),
        ),
    ]);
    write_json(&out, &value)?;
    Ok(format!("cap ${:.2}, {batches} batches", cap_usd.as_f64()))
}

// run

/// A pass over the plan an authorization pins.
pub struct Run {
    pub root: PathBuf,
    pub config: Config,
    pub authorization: Authorization,
    pub workers: usize,
    pub limit: Option<usize>,
}

enum State {
    Saved,
    New,
    /// A good receipt awaits saving.
    Resave(PathBuf),
    Unresolved(PathBuf),
}

struct Queue<'e> {
    effects: &'e dyn Effects,
    root: PathBuf,
    plan: Plan,
    budget_policy: String,
    client: String,
    cap: Usd,
    reserve: Usd,
    reserve_value: Value,
    model: String,
    effort: String,
    timeout: String,
    version: String,
    client_path: OsString,
    status_path: PathBuf,
    status: Mutex<Object>,
    active: Mutex<HashSet<PathBuf>>,
    saving: Mutex<()>,
    gate: Mutex<QuotaGate>,
    workers: usize,
    limit: Option<usize>,
}

fn receipt(attempt: &Path) -> R<Option<Value>> {
    let path = attempt.join("work/dispatch.json");
    if exists(&path)? {
        read_json(&path).map(Some)
    } else {
        Ok(None)
    }
}

fn priced(receipt: Option<&Value>) -> Option<f64> {
    receipt
        .and_then(|receipt| optional(receipt, "usage"))
        .and_then(|usage| optional(usage, "high"))
        .and_then(number)
}

fn good(receipt: Option<&Value>) -> bool {
    receipt.is_some_and(|receipt| {
        optional(receipt, "exit_code").and_then(number) == Some(0.0)
            && optional(receipt, "verdicts_present").is_some_and(Value::truthy)
            && !optional(receipt, "audit_violations").is_some_and(Value::truthy)
            && priced(Some(receipt)).is_some()
    })
}

fn usd(amount: f64) -> R<Usd> {
    Usd::of(&Value::Float(amount))
}

/// Grade the batches of the plan the authorization pins that have no saved second assessment; the exit code: 0 when
/// complete or at the limit, 1 after a failure, 3 at the budget and 4 at the quota.
pub fn execute(effects: &dyn Effects, run: &Run) -> R<u8> {
    let root = &run.root;
    let config = &run.config;
    let authorization = &run.authorization.value;
    if optional(authorization, "schemaVersion")
        .map(Value::py_str)
        .as_deref()
        != Some("1")
        || optional(authorization, "budgetPolicy").and_then(Value::as_str)
            != Some(config.budget_policy.as_str())
    {
        return Err(format!(
            "the authorization is not a second-pass authorization (budgetPolicy {})",
            config.budget_policy
        ));
    }
    let cap = Usd::of(field(authorization, "budgetCapUsd")?)?;
    let reserve_value = field(authorization, "maxBudgetUsd")?;
    let reserve = Usd::of(reserve_value)?;
    if cap == Usd::default() || reserve == Usd::default() {
        return Err("budgetCapUsd and maxBudgetUsd must be positive".into());
    }
    let grader = field(authorization, "grader")?;
    if optional(grader, "client").and_then(Value::as_str) != Some(config.client.as_str())
        || optional(grader, "model").and_then(Value::as_str) != Some(config.model.as_str())
    {
        return Err(format!(
            "the authorization's grader is not {} {}",
            config.client, config.model
        ));
    }
    let stop_percent = regrade::quota_stop_percent(authorization, DEFAULT_QUOTA_STOP)?;
    checked(root, field(field(authorization, "budgetReceipt")?, "ref")?)?;
    let plan_pin = field(authorization, "plan")?;
    checked(root, plan_pin)?;
    let plan = load_plan(root, config, text(plan_pin, "path")?)?;
    let directory = plan.queue.clone();
    std::fs::create_dir_all(&directory)
        .map_err(|error| format!("cannot create {}: {error}", shown(&directory)))?;
    let lock = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(directory.join("driver.lock"))
        .map_err(|error| format!("cannot open the driver lock: {error}"))?;
    let _held = nix::fcntl::Flock::lock(lock, nix::fcntl::FlockArg::LockExclusiveNonblock)
        .map_err(|_| {
            format!(
                "another driver holds {}",
                shown(&directory.join("driver.lock"))
            )
        })?;
    let wanted = text(grader, "cliVersion")?;
    let kind = Kind::of(&config.model);
    let (version, client_path) = regrade::pin_client(effects, &directory, kind, Some(wanted))?;
    let status_path = directory.join("status.json");
    let status = if exists(&status_path)? {
        match read_json(&status_path)? {
            Value::Object(object) => object,
            _ => return Err(format!("{} is not an object", shown(&status_path))),
        }
    } else {
        let mut fresh = Object::default();
        fresh.insert("batches".into(), Value::Object(Object::default()));
        fresh.insert("invocations".into(), Value::Array(Vec::new()));
        fresh
    };
    let clients = vec![Pinned {
        name: kind.name().to_string(),
        path: directory.join("client-bin").join(kind.name()),
        version: version.clone(),
    }];
    let queue = Queue {
        effects,
        root: root.clone(),
        budget_policy: config.budget_policy.clone(),
        client: config.client.clone(),
        cap,
        reserve,
        reserve_value: Value::Float(number(reserve_value).unwrap_or(f64::NAN)),
        model: config.model.clone(),
        effort: text(grader, "effort")?.to_string(),
        timeout: field(grader, "timeoutSeconds")?.py_str(),
        version,
        client_path,
        status_path,
        status: Mutex::new(status),
        active: Mutex::new(HashSet::new()),
        saving: Mutex::new(()),
        gate: Mutex::new(QuotaGate::new(clients, stop_percent, METER_WORK)),
        workers: run.workers,
        limit: run.limit,
        plan,
    };
    queue.run()
}

impl Queue<'_> {
    fn tool(&self, args: Vec<OsString>, log: &Path, path: &OsStr) -> R<i32> {
        self.effects.tool(&args, log, path).map_err(|error| {
            format!(
                "cannot run bench-tools {}: {error}",
                joined(&args[..2.min(args.len())])
            )
        })
    }

    fn saved(&self, batch: &Batch) -> R<bool> {
        saved(&self.plan.record_root, batch)
    }

    fn classify(&self, batch: &Batch) -> R<State> {
        if self.saved(batch)? {
            return Ok(State::Saved);
        }
        let (_, attempts) = attempts(&self.plan.queue, batch)?;
        let Some(latest) = attempts.last() else {
            return Ok(State::New);
        };
        if exists(&latest.join("disposition.json"))? {
            return Ok(State::New);
        }
        if good(receipt(latest)?.as_ref()) {
            return Ok(State::Resave(latest.clone()));
        }
        Ok(State::Unresolved(latest.clone()))
    }

    /// The attempt's settled charge, or its reservation while no priced receipt settles it. An attempt that is not
    /// running and never made the workspace's `home`, which `grade dispatch` creates only after its last gate before
    /// the paid call, holds nothing.
    fn charge(&self, attempt: &Path) -> R<(Usd, Usd)> {
        let reservation = attempt.join("reservation.json");
        if !exists(&reservation)? {
            return Ok((Usd::default(), Usd::default()));
        }
        if let Some(price) = priced(receipt(attempt)?.as_ref()) {
            return Ok((usd(price)?, Usd::default()));
        }
        let running = self.active.lock().unwrap().contains(attempt);
        if !exists(&attempt.join("work/home"))?
            && !exists(&attempt.join("work/dispatch.json"))?
            && !running
        {
            return Ok((Usd::default(), Usd::default()));
        }
        Ok((
            Usd::default(),
            Usd::of(field(&read_json(&reservation)?, "maxBudgetUsd")?)?,
        ))
    }

    fn budget(&self) -> R<(Usd, Usd, usize)> {
        let (mut settled, mut outstanding, mut count) = (Usd::default(), Usd::default(), 0);
        for batch in &self.plan.batches {
            for attempt in attempts(&self.plan.queue, batch)?.1 {
                let (spent, reserved) = self.charge(&attempt)?;
                settled = settled.plus(spent);
                if reserved != Usd::default() {
                    outstanding = outstanding.plus(reserved);
                    count += 1;
                }
            }
        }
        Ok((settled, outstanding, count))
    }

    /// Whether the cap still funds another reservation, with a dollar of headroom for it and for each one held.
    fn fits(&self) -> R<bool> {
        let (settled, outstanding, count) = self.budget()?;
        Ok(settled
            .plus(outstanding)
            .plus(Usd::ONE.times(count))
            .plus(self.reserve)
            .plus(Usd::ONE)
            <= self.cap)
    }

    fn quota(&self) -> Option<String> {
        self.gate
            .lock()
            .unwrap()
            .stop(self.effects, &self.client_path)
    }

    fn save_status(&self, state: &str, reason: Option<&str>) -> R<()> {
        let (settled, outstanding, count) = self.budget()?;
        let mut saved = 0;
        for batch in &self.plan.batches {
            saved += usize::from(self.saved(batch)?);
        }
        let mut status = self.status.lock().unwrap();
        for (key, value) in [
            ("state", string(state)),
            ("reason", reason.map_or(Value::Null, string)),
            ("updatedAt", string(utc(self.effects.now()))),
            ("budgetCapUsd", Value::Float(self.cap.as_f64())),
            ("spentUpperUsd", Value::Float(rounded(settled.as_f64(), 6))),
            (
                "reservedUsd",
                Value::Float(rounded(outstanding.as_f64(), 6)),
            ),
            ("outstandingReservations", int(count)),
            (
                "client",
                object(vec![(self.client.as_str(), string(&self.version))]),
            ),
            ("saved", int(saved)),
            ("planned", int(self.plan.batches.len())),
        ] {
            status.insert(key.into(), value);
        }
        replace(&self.status_path, &Value::Object(status.clone()))
    }

    fn note(
        &self,
        batch: &Batch,
        state: &str,
        attempt: Option<&Path>,
        extra: Vec<(&str, Value)>,
    ) -> R<()> {
        let mut entry = vec![
            ("state", string(state)),
            ("at", string(utc(self.effects.now()))),
        ];
        entry.extend(extra);
        if let Some(attempt) = attempt {
            entry.push(("attempt", string(relative(&self.root, attempt))));
        }
        let mut status = self.status.lock().unwrap();
        let batches = match status.get_mut("batches") {
            Some(Value::Object(batches)) => batches,
            _ => return Err("the status has no batches object".into()),
        };
        batches.insert(batch.id(), object(entry));
        replace(&self.status_path, &Value::Object(status.clone()))
    }

    fn usage_high(attempt: &Path) -> R<Value> {
        Ok(priced(receipt(attempt)?.as_ref()).map_or(Value::Null, Value::Float))
    }

    /// Save the second assessment from a good receipt, then archive the attempt.
    fn finish(&self, attempt: &Path) -> R<String> {
        let work = regrade::resolved(&attempt.join("work"))?;
        let key = attempt.join("key.json");
        let log = attempt.join("audit.log");
        let code = {
            let _saving = self.saving.lock().unwrap();
            self.tool(
                vec![
                    "evaluator-audit".into(),
                    "second".into(),
                    "--work".into(),
                    work.into(),
                    "--key".into(),
                    key.into(),
                ],
                &log,
                &self.client_path,
            )?
        };
        if code != 0 {
            return Ok("second-failed".into());
        }
        let inherited = std::env::var_os("PATH").unwrap_or_default();
        let code = self.tool(
            vec![
                "regrade".into(),
                "archive-attempt".into(),
                attempt.into(),
                self.plan.archive_root.clone().into(),
                "--root".into(),
                self.root.clone().into(),
            ],
            &log,
            &inherited,
        )?;
        Ok(if code == 0 { "saved" } else { "archive-failed" }.into())
    }

    fn grade(&self, batch: &Batch, attempt: &Path) -> R<String> {
        let work = regrade::resolved(&attempt.join("work"))?;
        let key = attempt.join("key.json");
        let log = attempt.join("audit.log");
        let claims = attempt.join("claims.json");
        let mut prepare: Vec<OsString> = vec![
            "grade".into(),
            "prepare".into(),
            "--run".into(),
            batch.run.clone().into(),
            "--target".into(),
            batch.target.clone().into(),
            "--work".into(),
            work.clone().into(),
            "--key".into(),
            key.clone().into(),
            "--inventory".into(),
            claims.clone().into(),
        ];
        prepare.extend(self.plan.replacement_args(batch)?);
        let steps: [(&str, Vec<OsString>); 4] = [
            (
                "claims",
                vec![
                    "evaluator-audit".into(),
                    "claims".into(),
                    "--run".into(),
                    batch.run.clone().into(),
                    "--target".into(),
                    batch.target.clone().into(),
                    "--out".into(),
                    claims.into(),
                ],
            ),
            ("prepare", prepare),
            (
                "check-prepared",
                vec![
                    "grade".into(),
                    "check-prepared".into(),
                    "--work".into(),
                    work.clone().into(),
                    "--key".into(),
                    key.clone().into(),
                    "--dispatching".into(),
                ],
            ),
            (
                "dispatch",
                vec![
                    "grade".into(),
                    "dispatch".into(),
                    "--work".into(),
                    work.into(),
                    "--key".into(),
                    key.into(),
                    "--model".into(),
                    self.model.clone().into(),
                    "--effort".into(),
                    self.effort.clone().into(),
                    "--expected-cli-version".into(),
                    self.version.clone().into(),
                    "--max-budget-usd".into(),
                    self.reserve_value.py_str().into(),
                    "--timeout".into(),
                    self.timeout.clone().into(),
                ],
            ),
        ];
        for (name, args) in steps {
            self.note(batch, name, Some(attempt), vec![])?;
            if self.tool(args, &log, &self.client_path)? != 0 {
                return Ok(format!("{name}-failed"));
            }
        }
        if !good(receipt(attempt)?.as_ref()) {
            return Ok("receipt-needs-investigation".into());
        }
        self.note(
            batch,
            "second",
            Some(attempt),
            vec![("usageHigh", Self::usage_high(attempt)?)],
        )?;
        self.finish(attempt)
    }

    fn new_attempt(&self, batch: &Batch) -> R<PathBuf> {
        let (directory, attempts) = attempts(&self.plan.queue, batch)?;
        let number = attempts
            .last()
            .and_then(|path| regrade::attempt_number(path))
            .map_or(1, |number| number + 1);
        let attempt = directory.join(format!("attempt-{number}"));
        std::fs::create_dir_all(&directory)
            .and_then(|()| std::fs::create_dir(&attempt))
            .map_err(|error| format!("cannot create {}: {error}", shown(&attempt)))?;
        let workspace = self.plan.workspaces.join(hex_id()?);
        std::fs::create_dir_all(&self.plan.workspaces)
            .map_err(|error| format!("cannot create {}: {error}", shown(&self.plan.workspaces)))?;
        std::os::unix::fs::symlink(&workspace, attempt.join("work"))
            .map_err(|error| format!("cannot link {}: {error}", shown(&attempt.join("work"))))?;
        let reservation = object(vec![
            ("batch", string(batch.id())),
            ("maxBudgetUsd", self.reserve_value.clone()),
            ("budgetPolicy", string(&self.budget_policy)),
            ("at", string(utc(self.effects.now()))),
            (
                "note",
                string(
                    "the session's --max-budget-usd, held at its maximum until a priced receipt settles it",
                ),
            ),
        ]);
        create_new(&attempt.join("reservation.json"), &indented(&reservation))?;
        Ok(attempt)
    }

    fn start_invocation(&self, invocation: Value) {
        if let Some(Value::Array(invocations)) = self.status.lock().unwrap().get_mut("invocations")
        {
            invocations.push(invocation);
        }
    }

    fn update_invocation(&self, update: Vec<(&str, Value)>) {
        if let Some(Value::Array(invocations)) = self.status.lock().unwrap().get_mut("invocations")
            && let Some(Value::Object(last)) = invocations.last_mut()
        {
            for (key, value) in update {
                last.insert(key.into(), value);
            }
        }
    }

    fn run(&self) -> R<u8> {
        let started = self.effects.elapsed();
        self.start_invocation(object(vec![
            ("startedAt", string(utc(self.effects.now()))),
            ("workers", int(self.workers)),
            ("limit", self.limit.map_or(Value::Null, int)),
            ("peakActive", int(0)),
        ]));
        let mut pending = VecDeque::new();
        for batch in &self.plan.batches {
            match self.classify(batch)? {
                State::Saved => {}
                State::New => pending.push_back(batch.clone()),
                State::Unresolved(attempt) => {
                    self.save_status(
                        "failed",
                        Some(&format!(
                            "{}: {} has no saved second assessment; inspect it and write disposition.json to grade \
                             the batch again",
                            batch.id(),
                            relative(&self.root, &attempt)
                        )),
                    )?;
                    return Ok(EXIT_FAILED);
                }
                State::Resave(attempt) => {
                    let outcome = self.finish(&attempt)?;
                    self.note(batch, &outcome, Some(&attempt), vec![])?;
                    if outcome != "saved" {
                        self.save_status("failed", Some(&format!("{}: {outcome}", batch.id())))?;
                        return Ok(EXIT_FAILED);
                    }
                }
            }
        }
        self.save_status("running", None)?;
        let mut state = "running";
        let mut reason: Option<String> = None;
        let mut exit = 0;
        let mut launched = 0;
        let mut running = 0;
        let mut peak = 0;
        let mut error = None;
        let (sender, finished) = mpsc::channel::<(Batch, PathBuf, R<String>)>();
        std::thread::scope(|scope| -> R<()> {
            loop {
                while state == "running"
                    && error.is_none()
                    && running < self.workers
                    && self.limit.is_none_or(|limit| launched < limit)
                    && let Some(batch) = pending.front().cloned()
                {
                    if let Err(refused) = regrade::pin_client(
                        self.effects,
                        &self.plan.queue,
                        Kind::of(&self.model),
                        Some(&self.version),
                    ) {
                        error = Some(refused);
                        break;
                    }
                    if !self.fits()? {
                        if running > 0 {
                            break;
                        }
                        (state, exit) = ("budget-stopped", EXIT_BUDGET);
                        reason = Some("the cap cannot fund the next batch's reservation".into());
                        break;
                    }
                    if let Some(stop) = self.quota() {
                        (state, exit) = ("quota-stopped", EXIT_QUOTA);
                        reason = Some(stop);
                        break;
                    }
                    pending.pop_front();
                    let attempt = self.new_attempt(&batch)?;
                    self.active.lock().unwrap().insert(attempt.clone());
                    let sender = sender.clone();
                    let job = (batch, attempt);
                    scope.spawn(move || {
                        let outcome = self.grade(&job.0, &job.1);
                        let _ = sender.send((job.0, job.1, outcome));
                    });
                    launched += 1;
                    running += 1;
                    peak = peak.max(running);
                    self.update_invocation(vec![("peakActive", int(peak))]);
                    self.save_status(state, reason.as_deref())?;
                }
                if running == 0 {
                    return Ok(());
                }
                let first = finished.recv().map_err(|error| error.to_string())?;
                let mut done = vec![first];
                done.extend(finished.try_iter());
                for (batch, attempt, outcome) in done {
                    running -= 1;
                    self.active.lock().unwrap().remove(&attempt);
                    let outcome = outcome.unwrap_or_else(|error| format!("error: {error}"));
                    self.note(
                        &batch,
                        &outcome,
                        Some(&attempt),
                        vec![("usageHigh", Self::usage_high(&attempt)?)],
                    )?;
                    if outcome != "saved" {
                        (state, exit) = ("failed", EXIT_FAILED);
                        reason = Some(format!("{}: {outcome}", batch.id()));
                    }
                }
                let (settled, outstanding, _) = self.budget()?;
                if settled.plus(outstanding) > self.cap && exit != EXIT_FAILED {
                    (state, exit) = ("budget-stopped", EXIT_BUDGET);
                    reason = Some("settled charges exceed the cap".into());
                }
                self.save_status(state, reason.as_deref())?;
            }
        })?;
        if let Some(error) = error {
            return Err(error);
        }
        if state == "running" {
            let mut left = false;
            for batch in &self.plan.batches {
                left |= !self.saved(batch)?;
            }
            state = if left { "limit-reached" } else { "complete" };
        }
        let meters = self.gate.lock().unwrap().reading.clone();
        self.update_invocation(vec![
            ("endedAt", string(utc(self.effects.now()))),
            (
                "wallSeconds",
                Value::Float(rounded(self.effects.elapsed() - started, 1)),
            ),
            ("meters", meters.unwrap_or(Value::Null)),
            ("exit", int(usize::from(exit))),
        ]);
        self.save_status(state, reason.as_deref())?;
        let status = self.status.lock().unwrap().clone();
        let shown_number = |key: &str| status.get(key).and_then(number).unwrap_or(0.0);
        say(&format!(
            "{state}: {} of {} batches saved; spent ${:.2} of ${:.2}, reserved ${:.2}{}",
            status.get("saved").map(Value::py_str).unwrap_or_default(),
            status.get("planned").map(Value::py_str).unwrap_or_default(),
            shown_number("spentUpperUsd"),
            self.cap.as_f64(),
            shown_number("reservedUsd"),
            reason.map_or(String::new(), |reason| format!("; {reason}"))
        ));
        Ok(exit)
    }
}

/// The queue's saved batches, spend and last invocation, as indented JSON.
pub fn status(root: &Path, config: &Config) -> R<String> {
    let plan = load_plan(root, config, &config.plan)?;
    let mut saved_count = 0;
    let (mut settled, mut outstanding) = (Usd::default(), Usd::default());
    let mut running = Vec::new();
    for batch in &plan.batches {
        saved_count += usize::from(saved(&plan.record_root, batch)?);
        let mut found = attempts(&plan.queue, batch)?.1;
        found.sort();
        for attempt in found {
            let reservation = attempt.join("reservation.json");
            if !exists(&reservation)? {
                continue;
            }
            if let Some(price) = priced(receipt(&attempt)?.as_ref()) {
                settled = settled.plus(usd(price)?);
            } else if exists(&attempt.join("work/home"))?
                || exists(&attempt.join("work/dispatch.json"))?
            {
                outstanding =
                    outstanding.plus(Usd::of(field(&read_json(&reservation)?, "maxBudgetUsd")?)?);
                running.push(string(batch.id()));
            }
        }
    }
    let recorded_path = plan.queue.join("status.json");
    let recorded = if exists(&recorded_path)? {
        read_json(&recorded_path)?
    } else {
        Value::Object(Object::default())
    };
    let client_path = plan.queue.join("client.json");
    let client = if exists(&client_path)? {
        let client = read_json(&client_path)?;
        object(vec![
            ("version", field(&client, "version")?.clone()),
            ("sha256", field(&client, "sha256")?.clone()),
        ])
    } else {
        Value::Null
    };
    let recorded_field = |key: &str| {
        recorded
            .as_object()
            .and_then(|object| object.get(key))
            .cloned()
            .unwrap_or(Value::Null)
    };
    let last = optional(&recorded, "invocations")
        .and_then(Value::as_array)
        .and_then(<[Value]>::last)
        .cloned()
        .unwrap_or(Value::Null);
    let value = object(vec![
        ("saved", int(saved_count)),
        ("planned", int(plan.batches.len())),
        ("state", recorded_field("state")),
        ("reason", recorded_field("reason")),
        ("spentUpperUsd", Value::Float(rounded(settled.as_f64(), 2))),
        (
            "reservedUsd",
            Value::Float(rounded(outstanding.as_f64(), 2)),
        ),
        ("unsettled", Value::Array(running)),
        ("client", client),
        ("lastInvocation", last),
        (
            "freeGiB",
            Value::Float(rounded(free_bytes(root) as f64 / f64::from(1u32 << 30), 1)),
        ),
    ]);
    Ok(indented(&value).trim_end().to_string())
}

// command line

/// What `bench-tools evaluator-audit second-pass` was asked to do.
pub enum Operation {
    Plan {
        out: PathBuf,
    },
    Preflight {
        cli_version: String,
        out: PathBuf,
    },
    Authorize,
    Run {
        authorization: PathBuf,
        workers: usize,
        limit: Option<usize>,
    },
    Status,
    Launch {
        authorization: PathBuf,
        not_before: Option<String>,
    },
    LaunchStatus,
}

/// Run an operation from the repository `root` with the round's `config`; the exit code, after `refused: <reason>` on
/// stderr for a refusal or an error.
pub fn command(root: &Path, config: &Path, operation: Operation) -> u8 {
    match start(root, config, operation) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("refused: {error}");
            EXIT_REFUSED
        }
    }
}

fn require_store() -> R<()> {
    if std::env::var_os(runtime_store::STORE_VARIABLE).is_none_or(|store| store.is_empty()) {
        return Err("set BENCH_RUNTIME_STORE to the runtime store the authorization pins".into());
    }
    Ok(())
}

fn start(root: &Path, config: &Path, operation: Operation) -> R<u8> {
    let root = root
        .canonicalize()
        .map_err(|error| format!("cannot resolve {}: {error}", shown(root)))?;
    if !root.join("bench/grading/current/audits.json").is_file() {
        return Err("run from the repository root".into());
    }
    let host = |executable: PathBuf| regrade::Host {
        root: root.clone(),
        executable,
        started: std::time::Instant::now(),
    };
    let own = || {
        std::env::current_exe()
            .and_then(|path| path.canonicalize())
            .map_err(|error| format!("cannot locate the running bench-tools: {error}"))
    };
    if let Operation::Run { .. } = operation {
        require_store()?;
        if std::env::var_os(FAST_MODE_OFF).as_deref() != Some(OsStr::new("1")) {
            return Err(format!(
                "set {FAST_MODE_OFF}=1: the second grader runs without fast mode"
            ));
        }
    }
    let config = Config::load(&root, config)?;
    match operation {
        Operation::Plan { out } => {
            say(&plan(&root, &config, &out)?);
            Ok(0)
        }
        Operation::Preflight { cli_version, out } => {
            require_store()?;
            preflight(&host(own()?), &root, &config, &cli_version, &out)
        }
        Operation::Authorize => {
            say(&authorize(&host(own()?), &root, &config)?);
            Ok(0)
        }
        Operation::Run {
            authorization,
            workers,
            limit,
        } => {
            if workers < 1 {
                return Err("--workers must be at least 1".into());
            }
            let authorization = regrade::authorize(&root, &root.join(authorization))?;
            let running = regrade::hand_over(&authorization.value)?;
            execute(
                &host(running),
                &Run {
                    root: root.clone(),
                    config,
                    authorization,
                    workers,
                    limit,
                },
            )
        }
        Operation::Status => {
            say(&status(&root, &config)?);
            Ok(0)
        }
        Operation::Launch {
            authorization,
            not_before,
        } => {
            require_store()?;
            let not_before = not_before
                .map(|moment| {
                    launch::timestamp(&moment)
                        .ok_or_else(|| format!("--not-before {moment} is not an ISO time"))
                })
                .transpose()?;
            launch::launch(
                &host(own()?),
                &launch::Options {
                    root: root.clone(),
                    config,
                    authorization,
                    not_before,
                },
            )
        }
        Operation::LaunchStatus => {
            say(&launch::status(&root, &config)?);
            Ok(0)
        }
    }
}
