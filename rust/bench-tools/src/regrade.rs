//! `bench-tools regrade run`: grade a pinned current queue of saved reviews with bounded concurrency and an explicit
//! spending policy.
//!
//! The queue runs only in the `bench-tools` its authorization pins under `runtimes`: another installed runtime
//! hands the run to the pinned one. One controller holds the queue's `controller.lock` and alone writes status,
//! reservations, settlements and current grades. Each worker thread waits on one reserved `grade dispatch` in its
//! own neutral workspace.

use crate::current_grading;
use crate::grade::runtime;
use crate::pyjson::{self, Duplicates, Object, Value};
use crate::pytext::{self, PyPath};
use crate::subscription_meters::{self, Pinned, Unreadable};
use crate::{child, clean_context, grading_reports, sha256};
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

pub const QUOTA_STOPPED: &str = "quota-stopped";
const QUOTA_STOP_PERCENT: f64 = 95.0;
const QUOTA_MAX_AGE_SECONDS: f64 = 300.0;
const DEFAULT_TIMEOUT_SECONDS: u64 = 900;
const CONTRACT: &str = "current-reconciliation/v1";
const HANDED_OVER: &str = "BENCH_REGRADE_PINNED_RUNTIME";

type R<T> = Result<T, String>;

/// An exact non-negative dollar amount in units of 10^-24 USD, so sums and the cap compare as decimals do.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Usd(i128);

const PLACES: u32 = 24;
const UNIT: i128 = 10i128.pow(PLACES);

impl Usd {
    pub const ONE: Usd = Usd(UNIT);
    const CENT: i128 = UNIT / 100;

    fn parse(text: &str) -> Option<Usd> {
        let (whole, fraction) = text.split_once('.').unwrap_or((text, ""));
        let digits = |part: &str| part.bytes().all(|byte| byte.is_ascii_digit());
        if whole.is_empty()
            || !digits(whole)
            || !digits(fraction)
            || fraction.len() > PLACES as usize
        {
            return None;
        }
        let whole: i128 = whole.parse().ok()?;
        let padded = format!("{fraction:0<width$}", width = PLACES as usize);
        let fraction: i128 = padded.parse().ok()?;
        whole.checked_mul(UNIT)?.checked_add(fraction).map(Usd)
    }

    /// `money(value)`: a JSON number that is a finite amount of at least zero.
    pub fn of(value: &Value) -> R<Usd> {
        let text = match value {
            Value::Int(digits) => digits.clone(),
            Value::Float(number) if number.is_finite() => format!("{number}"),
            _ => String::new(),
        };
        let unsigned = text.strip_prefix('-').unwrap_or(&text);
        Usd::parse(unsigned)
            .filter(|amount| amount.0 == 0 || !text.starts_with('-'))
            .ok_or_else(|| format!("invalid USD amount: {}", value.py_str()))
    }

    /// The amount in dollars and cents, as `grade dispatch --max-budget-usd` receives an allowance.
    fn cents(self) -> String {
        let cents = self.floor_cents().0 / Self::CENT;
        format!("{}.{:02}", cents / 100, cents % 100)
    }

    fn floor_cents(self) -> Usd {
        Usd(self.0 - self.0.rem_euclid(Self::CENT))
    }

    pub fn as_f64(self) -> f64 {
        self.to_string().parse().unwrap_or(f64::NAN)
    }

    fn json(self) -> Value {
        Value::Float(self.as_f64())
    }

    fn times(self, count: usize) -> Usd {
        Usd(self.0 * count as i128)
    }

    fn plus(self, other: Usd) -> Usd {
        Usd(self.0 + other.0)
    }

    fn minus(self, other: Usd) -> Usd {
        Usd(self.0 - other.0)
    }
}

impl fmt::Display for Usd {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sign = if self.0 < 0 { "-" } else { "" };
        let magnitude = self.0.unsigned_abs();
        let unit = UNIT.unsigned_abs();
        let fraction = format!("{:0width$}", magnitude % unit, width = PLACES as usize);
        let fraction = fraction.trim_end_matches('0');
        let fraction = if fraction.is_empty() { "0" } else { fraction };
        write!(out, "{sign}{}.{fraction}", magnitude / unit)
    }
}

fn sum(amounts: &[Usd]) -> Usd {
    amounts
        .iter()
        .fold(Usd::default(), |total, amount| total.plus(*amount))
}

/// The allowance a batch of `items` review comments would like: 1.50 USD plus 0.06 USD a comment, within 3 to 6 USD.
pub fn desired(items: usize) -> Usd {
    let wanted = Usd(UNIT * 3 / 2).plus(Usd(UNIT * 6 / 100).times(items));
    wanted.clamp(Usd::ONE.times(3), Usd::ONE.times(6))
}

/// What the cap allows another reservation once `used` is spent or held, keeping one dollar of headroom; `None`
/// when less than a dollar would be left.
pub fn allowance(cap: Usd, used: Usd, items: usize) -> Option<Usd> {
    let remaining = cap.minus(used).minus(Usd::ONE);
    (remaining >= Usd::ONE).then(|| remaining.min(desired(items)).floor_cents())
}

fn reserved_total(outstanding: &[Option<Usd>]) -> Option<Usd> {
    outstanding
        .iter()
        .copied()
        .collect::<Option<Vec<Usd>>>()
        .map(|known| sum(&known))
}

fn field<'v>(value: &'v Value, key: &str) -> R<&'v Value> {
    value
        .as_object()
        .and_then(|object| object.get(key))
        .ok_or_else(|| format!("missing field {}", pytext::repr_str(key)))
}

fn optional<'v>(value: &'v Value, key: &str) -> Option<&'v Value> {
    value
        .as_object()
        .and_then(|object| object.get(key))
        .filter(|found| !matches!(found, Value::Null))
}

fn text<'v>(value: &'v Value, key: &str) -> R<&'v str> {
    field(value, key)?
        .as_str()
        .ok_or_else(|| format!("field {} is not a string", pytext::repr_str(key)))
}

fn list<'v>(value: &'v Value, key: &str) -> R<&'v [Value]> {
    field(value, key)?
        .as_array()
        .ok_or_else(|| format!("field {} is not a list", pytext::repr_str(key)))
}

fn count(value: &Value, key: &str) -> R<usize> {
    match field(value, key)? {
        Value::Int(digits) => digits.parse().ok(),
        _ => None,
    }
    .ok_or_else(|| format!("field {} is not a count", pytext::repr_str(key)))
}

fn string(text: impl Into<String>) -> Value {
    Value::Str(text.into())
}

fn int(number: usize) -> Value {
    Value::Int(number.to_string())
}

fn shown(path: &Path) -> String {
    pytext::stderr_text(pytext::os_bytes(path.as_os_str()))
}

fn path_value(path: &Path) -> Value {
    string(shown(path))
}

fn read_json(path: &Path) -> R<Value> {
    let raw =
        std::fs::read(path).map_err(|error| format!("cannot read {}: {error}", shown(path)))?;
    let decoded = String::from_utf8(raw).map_err(|_| format!("{} is not UTF-8", shown(path)))?;
    pyjson::loads(&decoded, Duplicates::KeepLast)
        .map_err(|error| format!("{} is not JSON: {error}", shown(path)))
}

fn digest(path: &Path) -> R<String> {
    std::fs::read(path)
        .map(|raw| sha256::hex_digest(&raw))
        .map_err(|error| format!("cannot read {}: {error}", shown(path)))
}

fn exists(path: &Path) -> R<bool> {
    path.try_exists()
        .map_err(|error| format!("cannot inspect {}: {error}", shown(path)))
}

fn dumps(value: &Value) -> String {
    pyjson::dumps(value, &pyjson::Dumps::default())
}

fn indented(value: &Value) -> String {
    let options = pyjson::Dumps {
        indent: Some(2),
        ..pyjson::Dumps::default()
    };
    pyjson::dumps(value, &options) + "\n"
}

fn create_new(path: &Path, contents: &str) -> R<()> {
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .and_then(|mut file| file.write_all(contents.as_bytes()))
        .map_err(|error| format!("cannot create {}: {error}", shown(path)))
}

fn entries(directory: &Path) -> R<Vec<PathBuf>> {
    let failed = |error: std::io::Error| format!("cannot list {}: {error}", shown(directory));
    match std::fs::read_dir(directory) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => Err(failed(error)),
        Ok(listing) => listing
            .map(|entry| entry.map(|entry| entry.path()).map_err(failed))
            .collect(),
    }
}

fn attempt_number(path: &Path) -> Option<u64> {
    path.file_name()?
        .to_str()?
        .strip_prefix("attempt-")?
        .parse()
        .ok()
}

fn is_attempt(path: &Path) -> bool {
    path.file_name()
        .and_then(OsStr::to_str)
        .is_some_and(|name| name.starts_with("attempt-"))
}

/// Every `batches/*/*/attempt-*/reservation.json`, sorted.
fn reservations(directory: &Path) -> R<Vec<PathBuf>> {
    let mut found = Vec::new();
    for run in entries(&directory.join("batches"))? {
        for target in entries(&run)? {
            for attempt in entries(&target)?
                .into_iter()
                .filter(|path| is_attempt(path))
            {
                let reservation = attempt.join("reservation.json");
                if exists(&reservation)? {
                    found.push(reservation);
                }
            }
        }
    }
    found.sort_by(|left, right| {
        left.as_os_str()
            .as_encoded_bytes()
            .cmp(right.as_os_str().as_encoded_bytes())
    });
    Ok(found)
}

/// The path a `work` pointer leads to, whether or not its workspace exists yet.
fn resolved(pointer: &Path) -> R<PathBuf> {
    match pointer.canonicalize() {
        Ok(path) => Ok(path),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound && pointer.is_symlink() => {
            let target = std::fs::read_link(pointer)
                .map_err(|error| format!("cannot read {}: {error}", shown(pointer)))?;
            Ok(pointer.parent().unwrap_or(Path::new("")).join(target))
        }
        Err(error) => Err(format!("cannot resolve {}: {error}", shown(pointer))),
    }
}

fn unused_log(path: &Path) -> R<PathBuf> {
    let stem = path
        .file_stem()
        .and_then(OsStr::to_str)
        .unwrap_or_default()
        .to_string();
    let mut candidate = path.to_path_buf();
    let mut retry = 1;
    while exists(&candidate)? {
        candidate = path.with_file_name(format!("{stem}.retry-{retry}.log"));
        retry += 1;
    }
    Ok(candidate)
}

fn hex_id() -> R<String> {
    Ok(clean_context::uuid4()?.replace('-', ""))
}

fn tail(path: &Path, characters: usize) -> String {
    let text = String::from_utf8_lossy(&std::fs::read(path).unwrap_or_default()).into_owned();
    let skip = text.chars().count().saturating_sub(characters);
    text.chars().skip(skip).collect()
}

fn say(line: &str) {
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{line}");
    let _ = out.flush();
}

fn iso_now(seconds: f64) -> String {
    crate::normalize_review::utc_stamp(seconds.max(0.0) as u64).replace('Z', "+00:00")
}

/// What a queue run does outside its own files: run its grading steps, read the current record and the
/// subscription meters, find the installed client, and tell the time.
pub trait Effects: Sync {
    /// Run `bench-tools ARGS` from the repository with stdout and stderr in the new file `log`, stdin closed and
    /// `path` as `PATH`; the step's exit code.
    fn step(&self, args: &[OsString], log: &Path, path: &OsStr) -> std::io::Result<i32>;
    /// The `current-grading fingerprints` rows, once `current-grading check` passes.
    fn fingerprints(&self) -> R<Value>;
    fn meters(&self, clients: &[Pinned], path: &OsStr) -> Result<Value, Unreadable>;
    /// The client executable on `PATH`, with links resolved.
    fn installed_client(&self, name: &str) -> R<PathBuf>;
    fn client_version(&self, executable: &Path) -> R<String>;
    /// Seconds since the epoch.
    fn now(&self) -> f64;
    /// Seconds since this run started, on a clock that never goes back.
    fn elapsed(&self) -> f64;
}

/// The effects of a real run, whose steps run `executable`.
pub struct Host {
    pub root: PathBuf,
    pub executable: PathBuf,
    pub started: std::time::Instant,
}

impl Effects for Host {
    fn step(&self, args: &[OsString], log: &Path, path: &OsStr) -> std::io::Result<i32> {
        let output = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(log)?;
        let status = std::process::Command::new(&self.executable)
            .args(args)
            .current_dir(&self.root)
            .stdin(std::process::Stdio::null())
            .stdout(output.try_clone()?)
            .stderr(output)
            .env("PATH", path)
            .env_remove(HANDED_OVER)
            .status()?;
        Ok(child::exit_code(status))
    }

    fn fingerprints(&self) -> R<Value> {
        let failed = |failure: current_grading::Failure| {
            let message = match failure {
                current_grading::Failure::Inconsistent(message) => {
                    String::from_utf8_lossy(&message).into_owned()
                }
                current_grading::Failure::Input(message)
                | current_grading::Failure::Crash(message) => crate::pysemantics::relabel(&message),
            };
            format!("current evidence: {message}")
        };
        let root =
            current_grading::Root::new(pytext::os_bytes(self.root.as_os_str())).map_err(failed)?;
        let current = current_grading::CURRENT.as_bytes();
        let (selected, documents) =
            current_grading::load_current(&root, current, true).map_err(failed)?;
        current_grading::coverage_status(&selected, &documents).map_err(failed)?;
        current_grading::fingerprints(&root, current).map_err(failed)
    }

    fn meters(&self, clients: &[Pinned], path: &OsStr) -> Result<Value, Unreadable> {
        let mut env: Vec<(String, String)> = crate::run_cell::environment()
            .into_iter()
            .filter(|(name, _)| name != "PATH")
            .collect();
        env.push(("PATH".into(), path.to_string_lossy().into_owned()));
        subscription_meters::read(clients, &env)
    }

    fn installed_client(&self, name: &str) -> R<PathBuf> {
        use std::os::unix::fs::PermissionsExt;
        let path = std::env::var_os("PATH").unwrap_or_default();
        std::env::split_paths(&path)
            .map(|directory| directory.join(name))
            .find(|candidate| {
                std::fs::metadata(candidate)
                    .is_ok_and(|found| found.is_file() && found.permissions().mode() & 0o111 != 0)
            })
            .and_then(|found| found.canonicalize().ok())
            .ok_or_else(|| format!("no {name} client is installed"))
    }

    fn client_version(&self, executable: &Path) -> R<String> {
        let refused = || format!("cannot read the version of {}", shown(executable));
        let output = child::run(
            std::process::Command::new(executable).arg("--version"),
            Duration::from_secs(15),
        )
        .map_err(|_| refused())?;
        let version = regex::Regex::new(r"\d+\.\d+\.\d+").map_err(|error| error.to_string())?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        match (output.code, version.find(&stdout)) {
            (Some(0), Some(found)) => Ok(found.as_str().to_string()),
            _ => Err(refused()),
        }
    }

    fn now(&self) -> f64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0.0, |elapsed| elapsed.as_secs_f64())
    }

    fn elapsed(&self) -> f64 {
        self.started.elapsed().as_secs_f64()
    }
}

/// A read authorization whose runner deviations still hash to their pins.
pub struct Authorization {
    pub path: PathBuf,
    pub sha256: String,
    pub value: Value,
}

/// `ROOT / ref["path"]`, once it still hashes to `ref["sha256"]`.
fn checked(root: &Path, reference: &Value) -> R<PathBuf> {
    let relative = text(reference, "path")?;
    let path = root.join(relative);
    if digest(&path)? != text(reference, "sha256")? {
        return Err(format!("pinned input changed: {relative}"));
    }
    Ok(path)
}

pub fn authorize(root: &Path, path: &Path) -> R<Authorization> {
    let value = read_json(path)?;
    if let Some(deviations) = optional(&value, "runnerDeviations") {
        for reference in deviations
            .as_array()
            .ok_or("field 'runnerDeviations' is not a list")?
        {
            checked(root, reference)?;
        }
    }
    Ok(Authorization {
        path: path.to_path_buf(),
        sha256: digest(path)?,
        value,
    })
}

/// The executable the authorization pins under `runtimes`, from the store that holds `running`.
pub fn pinned_runtime(authorization: &Value, running: &Path) -> R<PathBuf> {
    let pin = optional(authorization, "runtimes")
        .and_then(|runtimes| optional(runtimes, runtime::NAME))
        .filter(|pin| pin.as_object().is_some())
        .ok_or(
            "authorization pins no bench-tools runtime; add the object `bench/tools/bench-tools runtime pin` prints \
             as `runtimes` in a new authorization version",
        )?;
    let unavailable =
        |why: String| format!("the authorization's pinned bench-tools is unavailable: {why}");
    let store = running
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .ok_or_else(|| unavailable(format!("{} is not in a runtime store", shown(running))))?;
    let pinned = |name: &str| optional(pin, name).and_then(Value::as_str).unwrap_or("");
    match runtime::check(
        store,
        runtime::NAME,
        pinned("sha256"),
        pinned("receipt_sha256"),
        true,
    ) {
        Ok(found) => Ok(found.executable),
        Err(runtime::Checked::Refused(why) | runtime::Checked::Input(why)) => Err(unavailable(why)),
    }
}

/// What a `regrade run` was asked to do.
pub struct Options {
    pub root: PathBuf,
    pub authorization: PathBuf,
    pub directory: PathBuf,
    pub limit: Option<usize>,
    pub workers: usize,
    pub expected_cli_version: Option<String>,
}

/// `regrade run`: the exit code, after `regrading stopped: <error>` on stderr for a run refused or stopped by an
/// error. A runtime other than the pinned one hands the run to it and never returns.
pub fn run(options: &Options) -> u8 {
    match start(options) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("regrading stopped: {error}");
            2
        }
    }
}

fn start(options: &Options) -> R<u8> {
    if options.workers < 1 {
        return Err("--workers must be at least 1".into());
    }
    let root = options
        .root
        .canonicalize()
        .map_err(|error| format!("cannot resolve {}: {error}", shown(&options.root)))?;
    let authorization = authorize(&root, &options.authorization)?;
    let running = std::env::current_exe()
        .and_then(|path| path.canonicalize())
        .map_err(|error| format!("cannot locate the running bench-tools: {error}"))?;
    let pinned = pinned_runtime(&authorization.value, &running)?;
    let pinned = pinned
        .canonicalize()
        .map_err(|error| format!("cannot resolve {}: {error}", shown(&pinned)))?;
    if pinned != running {
        if std::env::var_os(HANDED_OVER).is_some() {
            return Err(format!(
                "the pinned bench-tools {} did not start as itself",
                shown(&pinned)
            ));
        }
        use std::os::unix::process::CommandExt;
        let error = std::process::Command::new(&pinned)
            .args(std::env::args_os().skip(1))
            .env(HANDED_OVER, "1")
            .exec();
        return Err(format!(
            "cannot start the pinned bench-tools {}: {error}",
            shown(&pinned)
        ));
    }
    std::fs::create_dir_all(&options.directory)
        .map_err(|error| format!("cannot create {}: {error}", shown(&options.directory)))?;
    let directory = options
        .directory
        .canonicalize()
        .map_err(|error| format!("cannot resolve {}: {error}", shown(&options.directory)))?;
    let lock = std::fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(directory.join("controller.lock"))
        .map_err(|error| format!("cannot open the controller lock: {error}"))?;
    let _held = nix::fcntl::Flock::lock(lock, nix::fcntl::FlockArg::LockExclusiveNonblock)
        .map_err(|(_, error)| {
            format!(
                "another controller holds {}: {error}",
                shown(&directory.join("controller.lock"))
            )
        })?;
    let host = Host {
        root: root.clone(),
        executable: running,
        started: std::time::Instant::now(),
    };
    execute(
        &host,
        &Run {
            root,
            directory,
            authorization,
            limit: options.limit,
            workers: options.workers,
            expected_cli_version: options.expected_cli_version.clone(),
        },
    )
}

/// A locked queue directory and the authorization it runs under.
pub struct Run {
    pub root: PathBuf,
    pub directory: PathBuf,
    pub authorization: Authorization,
    pub limit: Option<usize>,
    pub workers: usize,
    pub expected_cli_version: Option<String>,
}

fn authorized_cap(authorization: &Value) -> R<Option<Usd>> {
    let cap = field(authorization, "budgetCapUsd")?;
    let codex = text(field(authorization, "grader")?, "model")?.starts_with("gpt-");
    let unbounded =
        optional(authorization, "budgetPolicy").and_then(Value::as_str) == Some("codex-unbounded");
    if matches!(cap, Value::Null) {
        if !codex || !unbounded {
            return Err(
                "a null budget cap requires a Codex grader and budgetPolicy codex-unbounded".into(),
            );
        }
        return Ok(None);
    }
    if unbounded {
        return Err("budgetPolicy codex-unbounded requires a null budget cap".into());
    }
    if codex {
        return Err("bounded Codex dispatch requires a verified dollar hard cap".into());
    }
    Usd::of(cap).map(Some)
}

fn quota_stop_percent(authorization: &Value) -> R<f64> {
    let percent = match authorization
        .as_object()
        .and_then(|object| object.get("quotaStopPercent"))
    {
        None => Some(QUOTA_STOP_PERCENT),
        Some(Value::Int(digits)) => digits.parse().ok(),
        Some(Value::Float(number)) => Some(*number),
        Some(_) => None,
    };
    percent
        .filter(|percent| *percent > 0.0 && *percent <= 100.0)
        .ok_or_else(|| "quotaStopPercent must be a number above 0 and at most 100".into())
}

/// The grader an authorization names.
struct Grader {
    model: String,
    effort: String,
    cli_version: Option<String>,
    timeout: u64,
}

impl Grader {
    fn of(authorization: &Value) -> R<Grader> {
        let grader = field(authorization, "grader")?;
        let timeout = match grader
            .as_object()
            .and_then(|object| object.get("timeoutSeconds"))
        {
            None => Some(DEFAULT_TIMEOUT_SECONDS),
            Some(Value::Int(digits)) => digits.parse().ok().filter(|seconds| *seconds > 0),
            Some(_) => None,
        }
        .ok_or("grader.timeoutSeconds must be a positive integer")?;
        Ok(Grader {
            model: text(grader, "model")?.to_string(),
            effort: text(grader, "effort")?.to_string(),
            cli_version: optional(grader, "cliVersion")
                .and_then(Value::as_str)
                .filter(|version| !version.is_empty())
                .map(str::to_string),
            timeout,
        })
    }

    fn client(&self) -> &'static str {
        if self.model.starts_with("gpt-") {
            "codex"
        } else {
            "claude"
        }
    }
}

/// `(version, PATH)` of the client this queue grades with. The first run of a queue copies the client installed
/// then into the queue directory, and every run executes that copy, so an update of the installed client during or
/// between runs changes no batch's client.
fn pin_client(
    effects: &dyn Effects,
    directory: &Path,
    grader: &Grader,
    expected: Option<&str>,
) -> R<(String, OsString)> {
    use std::os::unix::fs::PermissionsExt;
    let name = grader.client();
    let record = directory.join("client.json");
    let copy = directory.join("client-bin").join(name);
    let pinned = if exists(&record)? {
        read_json(&record)?
    } else {
        let source = effects.installed_client(name)?;
        let bin = directory.join("client-bin");
        std::fs::create_dir_all(&bin)
            .map_err(|error| format!("cannot create {}: {error}", shown(&bin)))?;
        let staged = bin.join(format!("{name}.copying"));
        std::fs::copy(&source, &staged)
            .and_then(|_| std::fs::set_permissions(&staged, std::fs::Permissions::from_mode(0o555)))
            .and_then(|()| std::fs::rename(&staged, &copy))
            .map_err(|error| format!("cannot copy {} into the queue: {error}", shown(&source)))?;
        let pinned: Object = [
            ("client".to_string(), string(name)),
            (
                "version".to_string(),
                string(effects.client_version(&copy)?),
            ),
            ("source".to_string(), path_value(&source)),
            ("sha256".to_string(), string(digest(&copy)?)),
            ("pinnedAt".to_string(), string(iso_now(effects.now()))),
        ]
        .into_iter()
        .collect();
        let pinned = Value::Object(pinned);
        create_new(&record, &indented(&pinned))?;
        pinned
    };
    let version = text(&pinned, "version")?.to_string();
    let intact = !copy.is_symlink()
        && copy.is_file()
        && digest(&copy).ok().as_deref() == Some(text(&pinned, "sha256")?);
    if !intact {
        return Err(format!(
            "the queue's copy of {name} {version} at {} is missing or changed; restore it, or remove {} to pin the \
             installed client for the batches left",
            shown(&copy),
            shown(&record)
        ));
    }
    if let Some(wanted) = expected.or(grader.cli_version.as_deref())
        && wanted != version
    {
        return Err(format!(
            "the queue's pinned {name} is {version}, not the requested {wanted}"
        ));
    }
    let inherited = std::env::var_os("PATH").unwrap_or_default();
    let mut path = copy
        .parent()
        .unwrap_or(directory)
        .as_os_str()
        .to_os_string();
    path.push(":");
    path.push(inherited);
    Ok((version, path))
}

/// An execution-plan path inside the repository.
fn planned_root(root: &Path, relative: &str) -> R<PathBuf> {
    let path = Path::new(relative);
    if path.is_absolute()
        || path
            .components()
            .any(|part| part == std::path::Component::ParentDir)
    {
        return Err(format!(
            "execution plan path leaves the repository: {relative}"
        ));
    }
    Ok(root.join(path))
}

type Batch = (String, String);

fn batch_of(value: &Value) -> R<Batch> {
    Ok((
        text(value, "run")?.to_string(),
        text(value, "target")?.to_string(),
    ))
}

fn run_name(run: &str) -> &str {
    Path::new(run)
        .file_name()
        .and_then(OsStr::to_str)
        .unwrap_or(run)
}

/// What `launch` did with the row at the head of the queue.
enum Launched {
    /// The row left the queue, with the dispatch to start when it reserved one.
    Left(Option<Job>),
    /// The row stays for an active reservation to settle or for the plan's quota.
    Waits,
}

struct Job {
    attempt: PathBuf,
    args: Vec<OsString>,
    log: PathBuf,
}

/// A row's reservation (`None` when the cap allows none, or for an unbounded queue), beside the ledger it was
/// quoted from and the row's review comments.
struct Quote {
    budget: Option<Usd>,
    settled: Usd,
    outstanding: Vec<Option<Usd>>,
    items: usize,
}

struct Stop {
    state: &'static str,
    reason: String,
    code: u8,
}

fn exit(code: i32) -> u8 {
    u8::try_from(code)
        .ok()
        .filter(|code| *code != 0)
        .unwrap_or(1)
}

struct Controller<'e> {
    effects: &'e dyn Effects,
    root: PathBuf,
    directory: PathBuf,
    authorization: &'e Authorization,
    plan: Value,
    cap: Option<Usd>,
    stop_percent: f64,
    grader: Grader,
    cli_version: String,
    client_path: OsString,
    clients: Vec<Pinned>,
    workspaces: PathBuf,
    archives: PathBuf,
    groups: HashMap<Batch, Vec<Value>>,
    planned: HashMap<Batch, String>,
    context: Vec<OsString>,
    rows: Vec<Object>,
    previous: Vec<Value>,
    invocation: Object,
    active: BTreeMap<PathBuf, usize>,
    queue: VecDeque<usize>,
    stopped: Option<Stop>,
    completed: usize,
    read_at: f64,
}

/// Grade the queue the authorization pins; the exit code: 0 once every batch is mapped or the limit is reached,
/// 1 or a step's code after a failure, 2 after an error, 3 at the budget and 4 at the plan's quota.
pub fn execute(effects: &dyn Effects, run: &Run) -> R<u8> {
    let mut controller = Controller::new(effects, run)?;
    controller.resume(run.limit)?;
    controller.drive(run.limit, run.workers)
}

impl<'e> Controller<'e> {
    fn new(effects: &'e dyn Effects, run: &'e Run) -> R<Self> {
        let root = run.root.clone();
        let authorization = &run.authorization;
        let value = &authorization.value;
        let plan = read_json(&checked(&root, field(value, "sourcePlan")?)?)?;
        let execution = read_json(&checked(&root, field(value, "executionPlan")?)?)?;
        if optional(&plan, "contract").and_then(Value::as_str) != Some(CONTRACT) {
            return Err(
                "authorization pins a plan of another grading contract; plan the current queue again".into(),
            );
        }
        let cap = authorized_cap(value)?;
        let stop_percent = quota_stop_percent(value)?;
        let grader = Grader::of(value)?;
        let (cli_version, client_path) = pin_client(
            effects,
            &run.directory,
            &grader,
            run.expected_cli_version.as_deref(),
        )?;
        let clients = vec![Pinned {
            name: grader.client().to_string(),
            path: run.directory.join("client-bin").join(grader.client()),
            version: cli_version.clone(),
        }];
        let workspaces = planned_root(&root, text(&execution, "workspaceRoot")?)?;
        let archives = planned_root(&root, text(&execution, "archiveRoot")?)?;
        let mut groups: HashMap<Batch, Vec<Value>> = HashMap::new();
        for review in list(&plan, "reviews")? {
            groups
                .entry(batch_of(review)?)
                .or_default()
                .push(review.clone());
        }
        let mut planned = HashMap::new();
        let mut plan_order = Vec::new();
        let mut current = Vec::new();
        for batch in list(&plan, "batches")? {
            let key = batch_of(batch)?;
            if optional(batch, "state").and_then(Value::as_str) == Some("current") {
                current.push(key.clone());
            }
            planned.insert(key.clone(), text(batch, "inputFingerprint")?.to_string());
            plan_order.push(key);
        }
        let ordered = list(&execution, "order")?
            .iter()
            .map(batch_of)
            .collect::<R<Vec<_>>>()?;
        let unique: HashSet<&Batch> = ordered.iter().collect();
        let awaiting: HashSet<&Batch> = plan_order
            .iter()
            .filter(|key| !current.contains(key))
            .collect();
        if unique.len() != ordered.len() || unique != awaiting {
            return Err(
                "execution plan order must name every batch awaiting grading exactly once".into(),
            );
        }
        let fingerprints = input_fingerprints(effects, &plan_order, &current)?;
        let changed: Vec<String> = plan_order
            .iter()
            .filter(|key| fingerprints.get(*key) != planned.get(*key))
            .map(|(run, target)| format!("{run}/{target}"))
            .collect();
        if !changed.is_empty() {
            return Err(format!(
                "relevant inputs changed since the plan; plan and authorize the queue again: {}",
                changed.join(", ")
            ));
        }
        let mut context: Vec<OsString> = vec!["--root".into(), root.clone().into()];
        if let Some(reference) = optional(value, "claimEvidence") {
            context.extend(["--claim-evidence".into(), checked(&root, reference)?.into()]);
        }
        if let Some(reference) = optional(value, "cacheReplacements") {
            context.extend([
                "--cache-replacements".into(),
                checked(&root, reference)?.into(),
            ]);
        }
        if let Some(cache) = optional(&execution, "cacheRoot") {
            let cache = cache.as_str().ok_or("field 'cacheRoot' is not a string")?;
            context.extend(["--cache-root".into(), planned_root(&root, cache)?.into()]);
        }
        let status = run.directory.join("status.json");
        let previous = if exists(&status)? {
            read_json(&status)?
        } else {
            Value::Object(Object::default())
        };
        let mut saved: HashMap<Batch, Object> = HashMap::new();
        for row in optional(&previous, "batches")
            .and_then(Value::as_array)
            .unwrap_or_default()
        {
            if let Some(object) = row.as_object() {
                saved.insert(batch_of(row)?, object.clone());
            }
        }
        let rows = ordered
            .iter()
            .map(|key| {
                saved.remove(key).unwrap_or_else(|| {
                    [
                        ("run".to_string(), string(&key.0)),
                        ("target".to_string(), string(&key.1)),
                        (
                            "reviews".to_string(),
                            int(groups.get(key).map_or(0, Vec::len)),
                        ),
                        ("state".to_string(), string("pending")),
                    ]
                    .into_iter()
                    .collect()
                })
            })
            .collect();
        let invocation = [
            ("startedAt".to_string(), string(iso_now(effects.now()))),
            ("workers".to_string(), int(run.workers)),
            ("peakActive".to_string(), int(0)),
            ("cliVersion".to_string(), string(&cli_version)),
        ]
        .into_iter()
        .collect();
        Ok(Controller {
            effects,
            root,
            directory: run.directory.clone(),
            authorization,
            previous: optional(&previous, "invocations")
                .and_then(Value::as_array)
                .unwrap_or_default()
                .to_vec(),
            plan,
            cap,
            stop_percent,
            grader,
            cli_version,
            client_path,
            clients,
            workspaces,
            archives,
            groups,
            planned,
            context,
            rows,
            invocation,
            active: BTreeMap::new(),
            queue: VecDeque::new(),
            stopped: None,
            completed: 0,
            read_at: 0.0,
        })
    }

    fn row_text(&self, index: usize, key: &str) -> String {
        self.rows[index]
            .get(key)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    }

    fn batch(&self, index: usize) -> Batch {
        (self.row_text(index, "run"), self.row_text(index, "target"))
    }

    fn label(&self, index: usize) -> String {
        let (run, target) = self.batch(index);
        format!("{run}/{target}")
    }

    fn set(&mut self, index: usize, key: &str, value: Value) {
        self.rows[index].insert(key.to_string(), value);
    }

    fn relative(&self, attempt: &Path) -> R<String> {
        attempt.strip_prefix(&self.root).map(shown).map_err(|_| {
            format!(
                "{} is not inside the repository {}",
                shown(attempt),
                shown(&self.root)
            )
        })
    }

    fn latest_attempt(&self, index: usize) -> R<PathBuf> {
        let (run, target) = self.batch(index);
        let batch = self
            .directory
            .join("batches")
            .join(run_name(&run))
            .join(target);
        let mut attempts = Vec::new();
        for path in entries(&batch)?.into_iter().filter(|path| is_attempt(path)) {
            let number = attempt_number(&path)
                .ok_or_else(|| format!("{} is not a numbered attempt", shown(&path)))?;
            attempts.push((number, path));
        }
        attempts.sort();
        Ok(attempts
            .pop()
            .map_or_else(|| batch.join("attempt-1"), |(_, path)| path))
    }

    /// `(settled upper charges, outstanding reservations)`; an unbounded reservation has no known amount. A
    /// reservation stays outstanding until a priced receipt or a zero-charge proof settles it; active attempts are
    /// still writing their receipts. A proof needs no possible provider call: a legacy receipt observing no model, or
    /// an intact workspace with no receipt and no `home`, which `grade dispatch` creates only after its last gate
    /// before the paid call.
    fn ledger(&self) -> R<(Usd, Vec<Option<Usd>>)> {
        let mut settled = Usd::default();
        let mut outstanding = Vec::new();
        for reservation in reservations(&self.directory)? {
            let attempt = reservation
                .parent()
                .unwrap_or(&self.directory)
                .to_path_buf();
            let active = self.active.contains_key(&attempt);
            let receipt = attempt.join("work/dispatch.json");
            let record = if !active && exists(&receipt)? {
                Some(read_json(&receipt)?)
            } else {
                None
            };
            let resolution = attempt.join("budget-resolution.json");
            let high = record
                .as_ref()
                .map(|record| field(record, "usage"))
                .transpose()?
                .and_then(|usage| optional(usage, "high"));
            if let Some(high) = high {
                settled = settled.plus(Usd::of(high)?);
            } else if !active && exists(&resolution)? {
                let proof = read_json(&resolution)?;
                for reference in list(&proof, "evidence")? {
                    checked(&self.root, reference)?;
                }
                let work = attempt.join("work");
                let started = match &record {
                    Some(record) => {
                        field(record, "models_observed")?.truthy()
                            || match optional(record, "provider_call_possible") {
                                Some(possible) => possible.truthy(),
                                None => {
                                    optional(record, "budget_policy").and_then(Value::as_str)
                                        == Some("codex-unbounded")
                                }
                            }
                    }
                    None => !work.is_dir() || exists(&work.join("home"))?,
                };
                if started || Usd::of(field(&proof, "chargeUpperUsd")?)? != Usd::default() {
                    return Err(format!("invalid zero-charge proof: {}", shown(&resolution)));
                }
            } else {
                let reserved = read_json(&reservation)?;
                outstanding.push(match field(&reserved, "maxBudgetUsd")? {
                    Value::Null => None,
                    budget => Some(Usd::of(budget)?),
                });
            }
        }
        Ok((settled, outstanding))
    }

    fn save(&mut self, state: &str, reason: Option<&str>) -> R<()> {
        let (state, reason) = match &self.stopped {
            Some(stop) => (stop.state, Some(stop.reason.as_str())),
            None => (state, reason),
        };
        let wall = (self.effects.elapsed() * 1000.0).round() / 1000.0;
        self.invocation
            .insert("wallSeconds".into(), Value::Float(wall));
        let authorization = &self.authorization.value;
        let grader = field(authorization, "grader")?;
        let mut invocations = self.previous.clone();
        invocations.push(Value::Object(self.invocation.clone()));
        let mut document: Object = [
            ("schemaVersion".to_string(), int(2)),
            ("state".to_string(), string(state)),
            ("reason".to_string(), reason.map_or(Value::Null, string)),
            (
                "budgetCapUsd".to_string(),
                field(authorization, "budgetCapUsd")?.clone(),
            ),
            ("model".to_string(), field(grader, "model")?.clone()),
            ("effort".to_string(), field(grader, "effort")?.clone()),
            ("plannedBatches".to_string(), int(self.rows.len())),
            (
                "plannedReviews".to_string(),
                int(list(&self.plan, "reviews")?.len()),
            ),
            (
                "batches".to_string(),
                Value::Array(self.rows.iter().cloned().map(Value::Object).collect()),
            ),
            ("invocations".to_string(), Value::Array(invocations)),
        ]
        .into_iter()
        .collect();
        let spending = match self.ledger() {
            Ok((settled, outstanding)) => [
                settled.json(),
                reserved_total(&outstanding).map_or(Value::Null, Usd::json),
                int(outstanding.len()),
                int(outstanding.iter().filter(|amount| amount.is_none()).count()),
            ],
            Err(_) => [Value::Null, Value::Null, Value::Null, Value::Null],
        };
        for (name, value) in [
            "spentUpperUsd",
            "reservedUsd",
            "outstandingReservations",
            "unknownReservations",
        ]
        .into_iter()
        .zip(spending)
        {
            document.insert(name.into(), value);
        }
        let temporary = self.directory.join("status.tmp");
        std::fs::write(&temporary, indented(&Value::Object(document)))
            .and_then(|()| std::fs::rename(&temporary, self.directory.join("status.json")))
            .map_err(|error| format!("cannot save the queue status: {error}"))
    }

    fn save_running(&mut self) -> R<()> {
        self.save("running", None)
    }

    /// The first stop stands, except that a quota stop yields to a later failure or exhausted cap.
    fn block(&mut self, state: &'static str, reason: String, code: u8) -> R<()> {
        if self
            .stopped
            .as_ref()
            .is_none_or(|stop| stop.state == QUOTA_STOPPED)
        {
            self.stopped = Some(Stop {
                state,
                reason,
                code,
            });
        }
        self.save_running()
    }

    /// Stops the queue as failed when `outcome` is an error.
    fn guarded<T>(&mut self, place: &str, outcome: R<T>) -> R<Option<T>> {
        match outcome {
            Ok(value) => Ok(Some(value)),
            Err(error) => {
                self.block("failed", format!("{place}: {error}"), 2)?;
                Ok(None)
            }
        }
    }

    fn verdict(&self, usage: &Value, moment: f64) -> Result<(Option<String>, bool), Unreadable> {
        let usage = subscription_meters::checked(&self.clients, usage.clone())?;
        let stop =
            subscription_meters::quota_stop(&usage, self.stop_percent, moment, "grading session")
                .map_err(|message| Unreadable {
                message,
                sign_in: false,
            })?;
        Ok((stop, subscription_meters::sign_in_expiring(&usage, moment)))
    }

    /// The reason the plan's meters stop new launches, from a reading no older than five minutes. A sign-in that
    /// the reading shows about to expire is read again, since a person may have renewed it.
    fn quota(&mut self) -> Option<String> {
        let moment = self.effects.now();
        let outcome = (|| {
            let mut verdict = None;
            if let Some(usage) = self.invocation.get("meters")
                && moment - self.read_at <= QUOTA_MAX_AGE_SECONDS
            {
                verdict = Some(self.verdict(usage, moment)?);
            }
            match verdict {
                Some((stop, false)) => Ok(stop),
                _ => {
                    let usage = self.effects.meters(&self.clients, &self.client_path)?;
                    let (stop, _) = self.verdict(&usage, moment)?;
                    self.invocation.insert("meters".into(), usage);
                    self.read_at = moment;
                    Ok(stop)
                }
            }
        })();
        outcome.unwrap_or_else(|error: Unreadable| {
            Some(format!(
                "The subscription meters could not be read: {}",
                error.message
            ))
        })
    }

    fn within_quota(&mut self) -> R<bool> {
        match self.quota() {
            Some(reason) => {
                say(&format!("Stopped at the plan's quota: {reason}"));
                self.block(QUOTA_STOPPED, reason, 4)?;
                Ok(false)
            }
            None => Ok(true),
        }
    }

    fn reviews(&self, index: usize) -> &[Value] {
        self.groups
            .get(&self.batch(index))
            .map_or(&[], Vec::as_slice)
    }

    fn check_inputs(&self, index: usize) -> R<()> {
        if let Some(reference) = optional(&self.authorization.value, "cacheReplacements") {
            checked(&self.root, reference)?;
        }
        for review in self.reviews(index) {
            checked(&self.root, field(review, "review")?)?;
            checked(&self.root, field(review, "record")?)?;
        }
        Ok(())
    }

    fn step(&self, args: Vec<OsString>, log: &Path) -> R<i32> {
        self.effects
            .step(&args, log, &self.client_path)
            .map_err(|error| {
                format!(
                    "cannot run bench-tools {}: {error}",
                    args[..2].join(OsStr::new(" ")).to_string_lossy()
                )
            })
    }

    fn settle(&mut self, index: usize, attempt: &Path, code: i32) -> R<()> {
        let (run, target) = self.batch(index);
        let work = resolved(&attempt.join("work"))?;
        let receipt = attempt.join("work/dispatch.json");
        let workspace = self.relative(attempt)?;
        self.set(index, "workspace", string(workspace));
        if code != 0 {
            say(&tail(&attempt.join("dispatch.log"), 1800));
        }
        if !exists(&receipt)? {
            self.set(index, "state", string("unsettled"));
            return self.block(
                "failed",
                format!(
                    "Reserved attempt has no receipt and is never dispatched again: {}",
                    shown(attempt)
                ),
                if code == 0 { 1 } else { exit(code) },
            );
        }
        let record = read_json(&receipt)?;
        let failed = code != 0
            || !matches!(field(&record, "exit_code")?, Value::Int(digits) if digits == "0")
            || !field(&record, "verdicts_present")?.truthy()
            || field(&record, "audit_violations")?.truthy();
        if failed {
            self.set(index, "state", string("dispatch-failed"));
            return self.block(
                "failed",
                format!("Paid attempt requires investigation: {}", shown(attempt)),
                if code == 0 { 1 } else { exit(code) },
            );
        }
        let (settled, outstanding) = self.ledger()?;
        if let Some(cap) = self.cap {
            let held = outstanding
                .iter()
                .copied()
                .collect::<Option<Vec<_>>>()
                .ok_or("an unbounded reservation is outstanding under a dollar cap")?;
            if settled.plus(sum(&held)) > cap {
                self.set(index, "state", string("budget-stopped"));
                return self.block(
                    "budget-stopped",
                    "Metered usage reached the total cap".into(),
                    3,
                );
            }
        }
        let context = read_json(&work.join("clean-context.json"))?;
        let identity = [
            ("sessionId", field(&record, "session_id")?.clone()),
            ("contextId", field(&context, "context_id")?.clone()),
        ];
        for (name, value) in &identity {
            let shared = self.rows.iter().enumerate().any(|(other, row)| {
                other != index
                    && row
                        .get(name)
                        .is_some_and(|seen| dumps(seen) == dumps(value))
            });
            if shared {
                return Err(format!(
                    "{name} {} is not unique to {}",
                    value.py_str(),
                    shown(attempt)
                ));
            }
        }
        let map_log = unused_log(&attempt.join("map.log"))?;
        let code = self.step(
            vec![
                "grade".into(),
                "map".into(),
                "--root".into(),
                self.root.clone().into(),
                "--work".into(),
                work.clone().into(),
                "--key".into(),
                attempt.join("key.json").into(),
            ],
            &map_log,
        )?;
        if code != 0 {
            self.set(index, "state", string("mapping-failed"));
            say(&tail(&map_log, 2400));
            return self.block(
                "failed",
                format!("Mapping failed for {run}/{target}"),
                exit(code),
            );
        }
        let evidence = self.archive(attempt)?;
        let fingerprint = self
            .planned
            .get(&(run.clone(), target.clone()))
            .cloned()
            .unwrap_or_default();
        self.set(index, "state", string("mapped"));
        self.set(index, "inputFingerprint", string(fingerprint));
        self.set(
            index,
            "costUpperUsd",
            field(field(&record, "usage")?, "high")?.clone(),
        );
        self.set(index, "dispatchSha256", string(digest(&receipt)?));
        self.set(index, "evidence", evidence);
        for (name, value) in identity {
            self.set(index, name, value);
        }
        self.completed += 1;
        self.save_running()?;
        let spending = match self.cap {
            None => format!(
                "known priced usage ${settled}; {} unsettled sessions",
                outstanding.len()
            ),
            Some(_) => format!("cumulative ${settled}"),
        };
        let reviews = self.rows[index]
            .get("reviews")
            .map(Value::py_str)
            .unwrap_or_default();
        say(&format!(
            "Mapped {run}/{target}; {reviews} reviews; {spending}"
        ));
        Ok(())
    }

    fn archive(&self, attempt: &Path) -> R<Value> {
        let bytes = |path: &Path| PyPath::new(pytext::os_bytes(path.as_os_str()));
        grading_reports::archive_attempt(
            &bytes(&self.root),
            &bytes(attempt),
            &bytes(&self.archives),
        )
        .map_err(|failure| format!("archiving {} failed: {}", shown(attempt), failure.line()))
    }

    fn preflight(&mut self) -> R<()> {
        let mut runs: Vec<(String, Vec<String>)> = Vec::new();
        for index in self.queue.clone() {
            self.check_inputs(index)?;
            let (run, target) = self.batch(index);
            match runs.iter_mut().find(|(name, _)| *name == run) {
                Some((_, targets)) => targets.push(target),
                None => runs.push((run, vec![target])),
            }
        }
        let logs = self.directory.join("preflight");
        std::fs::create_dir_all(&logs)
            .map_err(|error| format!("cannot create {}: {error}", shown(&logs)))?;
        for (run, targets) in runs {
            let scratch = self.workspaces.join(hex_id()?);
            let mut args: Vec<OsString> = vec![
                "grade".into(),
                "preflight".into(),
                "--run".into(),
                run.clone().into(),
                "--work-root".into(),
                scratch.join("work").into(),
                "--key-root".into(),
                scratch.join("keys").into(),
                "--model".into(),
                self.grader.model.clone().into(),
                "--expected-cli-version".into(),
                self.cli_version.clone().into(),
            ];
            args.extend(self.context.iter().cloned());
            for target in targets {
                args.extend(["--target".into(), target.into()]);
            }
            if self.cap.is_none() {
                args.push("--allow-unbounded-codex".into());
            }
            let log = unused_log(&logs.join(format!("{}.log", run_name(&run))))?;
            let code = self.step(args, &log)?;
            if code != 0 {
                return self.block(
                    "failed",
                    format!("Queue preflight failed for {run}"),
                    exit(code),
                );
            }
        }
        Ok(())
    }

    /// What the cap allows the row now.
    fn quote(&self, index: usize) -> R<Quote> {
        let (settled, outstanding) = self.ledger()?;
        let mut items = 0;
        for review in self.reviews(index) {
            items += count(review, "items")?;
        }
        let budget = match self.cap {
            None => None,
            Some(cap) => {
                let held = outstanding
                    .iter()
                    .copied()
                    .collect::<Option<Vec<_>>>()
                    .ok_or("an unbounded reservation is outstanding under a dollar cap")?;
                let used = settled.plus(sum(&held)).plus(Usd::ONE.times(held.len()));
                allowance(cap, used, items)
            }
        };
        Ok(Quote {
            budget,
            settled,
            outstanding,
            items,
        })
    }

    /// False, with the queue stopped for its budget, when the cap allows a row no reservation.
    fn fundable(&mut self, budget: Option<Usd>, settled: Usd) -> R<bool> {
        let Some(cap) = self.cap.filter(|_| budget.is_none()) else {
            return Ok(true);
        };
        say(&format!(
            "Stopped at ${settled}: total authorization ${cap}"
        ));
        self.block(
            "budget-stopped",
            "Insufficient reserved budget for another batch".into(),
            3,
        )?;
        Ok(false)
    }

    fn workspace(&self, attempt: &Path) -> R<PathBuf> {
        let pointer = attempt.join("work");
        if !exists(&pointer)? && !pointer.is_symlink() {
            std::os::unix::fs::symlink(self.workspaces.join(hex_id()?), &pointer)
                .map_err(|error| format!("cannot create {}: {error}", shown(&pointer)))?;
        }
        resolved(&pointer)
    }

    fn dispatch_arguments(&self, work: &Path, key: &Path, budget: Option<Usd>) -> R<Vec<OsString>> {
        let mut args: Vec<OsString> = vec![
            "grade".into(),
            "dispatch".into(),
            "--work".into(),
            work.into(),
            "--key".into(),
            key.into(),
            "--model".into(),
            self.grader.model.clone().into(),
            "--effort".into(),
            self.grader.effort.clone().into(),
            "--expected-cli-version".into(),
            self.cli_version.clone().into(),
        ];
        match budget {
            None if !self.grader.model.starts_with("gpt-") => {
                return Err("unbounded dispatch requires a Codex grader".into());
            }
            None => args.push("--allow-unbounded-codex".into()),
            Some(budget) => args.extend(["--max-budget-usd".into(), budget.cents().into()]),
        }
        args.extend(["--timeout".into(), self.grader.timeout.to_string().into()]);
        Ok(args)
    }

    fn launch(&mut self, index: usize) -> R<Launched> {
        let (run, target) = self.batch(index);
        self.check_inputs(index)?;
        let attempt = self.latest_attempt(index)?;
        let Quote {
            budget,
            settled,
            outstanding,
            items,
        } = self.quote(index)?;
        if self.cap.is_some()
            && !self.active.is_empty()
            && budget.is_none_or(|budget| budget < desired(items))
        {
            return Ok(Launched::Waits);
        }
        if !self.fundable(budget, settled)? {
            return Ok(Launched::Left(None));
        }
        if !self.within_quota()? {
            return Ok(Launched::Waits);
        }
        std::fs::create_dir_all(&attempt)
            .map_err(|error| format!("cannot create {}: {error}", shown(&attempt)))?;
        let work = self.workspace(&attempt)?;
        let key = attempt.join("key.json");
        let workspace = self.relative(&attempt)?;
        self.set(index, "workspace", string(workspace));
        if !exists(&key)? {
            let mut prepare: Vec<OsString> = vec![
                "grade".into(),
                "prepare".into(),
                "--run".into(),
                run.clone().into(),
                "--target".into(),
                target.clone().into(),
                "--work".into(),
                work.clone().into(),
                "--key".into(),
                key.clone().into(),
            ];
            prepare.extend(self.context.iter().cloned());
            let code = self.step(prepare, &attempt.join("prepare.log"))?;
            if code != 0 {
                self.set(index, "state", string("prepare-failed"));
                self.block(
                    "failed",
                    format!("Prepare failed for {run}/{target}"),
                    exit(code),
                )?;
                return Ok(Launched::Left(None));
            }
        }
        let prepared = read_json(&key)?;
        if optional(&prepared, "input_fingerprint").and_then(Value::as_str)
            != self.planned.get(&(run, target)).map(String::as_str)
        {
            return Err(format!(
                "prepared inputs differ from the authorized plan: {}",
                shown(&attempt)
            ));
        }
        if let Some(reference) = optional(&self.authorization.value, "cacheReplacements") {
            checked(&self.root, reference)?;
            let consumed = ["runner_deviation", "provisioning", "manifest", "sha256"]
                .iter()
                .try_fold(&prepared, |value, key| optional(value, key))
                .and_then(Value::as_str);
            if consumed != Some(text(reference, "sha256")?) {
                return Err(format!(
                    "prepared cache replacement differs from authorization: {}",
                    shown(&attempt)
                ));
            }
        }
        let args = self.dispatch_arguments(&work, &key, budget)?;
        if digest(&self.authorization.path)? != self.authorization.sha256 {
            return Err("authorization changed before reservation".into());
        }
        if !self.within_quota()? {
            return Ok(Launched::Waits);
        }
        let reservation: Object = [
            (
                "maxBudgetUsd".to_string(),
                budget.map_or(Value::Null, Usd::json),
            ),
            ("spentBeforeUpperUsd".to_string(), settled.json()),
            (
                "reservedBeforeUsd".to_string(),
                reserved_total(&outstanding).map_or(Value::Null, Usd::json),
            ),
            (
                "authorizationSha256".to_string(),
                string(&self.authorization.sha256),
            ),
        ]
        .into_iter()
        .collect();
        create_new(
            &attempt.join("reservation.json"),
            &dumps(&Value::Object(reservation)),
        )?;
        self.set(index, "state", string("dispatching"));
        self.active.insert(attempt.clone(), index);
        let peak = self
            .invocation
            .get("peakActive")
            .map(Value::py_str)
            .and_then(|peak| peak.parse::<usize>().ok())
            .unwrap_or(0)
            .max(self.active.len());
        self.invocation.insert("peakActive".into(), int(peak));
        self.save_running()?;
        Ok(Launched::Left(Some(Job {
            log: attempt.join("dispatch.log"),
            attempt,
            args,
        })))
    }

    fn limited(&self, limit: Option<usize>, extra: usize) -> bool {
        limit.is_some_and(|limit| limit > 0 && self.completed + extra >= limit)
    }

    /// Settle every reserved attempt a stopped run left and queue every row with no reservation.
    fn resume(&mut self, limit: Option<usize>) -> R<()> {
        for reservation in reservations(&self.directory)? {
            let saved = read_json(&reservation)?;
            if text(&saved, "authorizationSha256")? != self.authorization.sha256 {
                return Err(format!(
                    "reserved attempt authorization changed: {}",
                    shown(&reservation)
                ));
            }
        }
        for index in 0..self.rows.len() {
            if self.row_text(index, "state") == "mapped" {
                if self.rows[index].get("evidence").is_none() {
                    let attempt = self.root.join(self.row_text(index, "workspace"));
                    let evidence = self.archive(&attempt)?;
                    self.set(index, "evidence", evidence);
                }
                continue;
            }
            let attempt = self.latest_attempt(index)?;
            if !exists(&attempt.join("reservation.json"))? {
                self.queue.push_back(index);
            } else if !self.limited(limit, 0) {
                let outcome = self.settle(index, &attempt, 0);
                let label = self.label(index);
                self.guarded(&label, outcome)?;
            }
        }
        if self.queue.is_empty() || self.stopped.is_some() || self.limited(limit, 0) {
            return Ok(());
        }
        let quoted = self.quote(self.queue[0]);
        let funded = match quoted {
            Ok(quote) => self.fundable(quote.budget, quote.settled),
            Err(error) => Err(error),
        };
        if self.guarded("Queue budget", funded)? == Some(true) && self.within_quota()? {
            let outcome = self.preflight();
            self.guarded("Queue preflight", outcome)?;
        }
        Ok(())
    }

    fn drive(&mut self, limit: Option<usize>, workers: usize) -> R<u8> {
        let effects = self.effects;
        let (sender, finished) = mpsc::channel::<(PathBuf, std::io::Result<i32>)>();
        std::thread::scope(|scope| -> R<()> {
            loop {
                while let Some(&index) = self.queue.front() {
                    if self.stopped.is_some()
                        || self.active.len() >= workers
                        || self.limited(limit, self.active.len())
                    {
                        break;
                    }
                    let outcome = self.launch(index);
                    let label = self.label(index);
                    match self.guarded(&label, outcome)? {
                        Some(Launched::Waits) => break,
                        Some(Launched::Left(job)) => {
                            self.queue.pop_front();
                            if let Some(job) = job {
                                let sender = sender.clone();
                                let path = self.client_path.clone();
                                scope.spawn(move || {
                                    let code = effects.step(&job.args, &job.log, &path);
                                    let _ = sender.send((job.attempt, code));
                                });
                            }
                        }
                        None => {
                            self.queue.pop_front();
                        }
                    }
                }
                if self.active.is_empty() {
                    return Ok(());
                }
                let first = finished.recv().map_err(|error| error.to_string())?;
                let mut done = vec![first];
                done.extend(finished.try_iter());
                for (attempt, code) in done {
                    let Some(index) = self.active.remove(&attempt) else {
                        continue;
                    };
                    let outcome = code
                        .map_err(|error| format!("dispatch did not run: {error}"))
                        .and_then(|code| self.settle(index, &attempt, code));
                    let label = self.label(index);
                    self.guarded(&label, outcome)?;
                }
            }
        })?;
        if let Some(code) = self.stopped.as_ref().map(|stop| stop.code) {
            self.save_running()?;
            return Ok(code);
        }
        if self.limited(limit, 0) {
            self.save("limited", Some("Requested batch limit reached"))?;
            return Ok(0);
        }
        self.save("mapped", Some("Awaiting the declared evaluator audit"))?;
        Ok(0)
    }
}

/// The current input fingerprint of each planned batch. Stale saved grades are refused here, before any dispatch,
/// because no batch can be mapped beside them.
fn input_fingerprints(
    effects: &dyn Effects,
    batches: &[Batch],
    current: &[Batch],
) -> R<HashMap<Batch, String>> {
    let selected = effects.fingerprints()?;
    let mut rows = HashMap::new();
    for row in selected
        .as_array()
        .ok_or("current fingerprints are not a list")?
    {
        rows.insert(batch_of(row)?, row);
    }
    let unknown: Vec<String> = batches
        .iter()
        .filter(|key| !rows.contains_key(*key))
        .map(|(run, target)| format!("{run}/{target}"))
        .collect();
    if !unknown.is_empty() {
        return Err(format!(
            "planned batch is not selected: {}",
            unknown.join(", ")
        ));
    }
    let missing: Vec<String> = current
        .iter()
        .filter(|key| optional(rows[*key], "state").and_then(Value::as_str) != Some("current"))
        .map(|(run, target)| format!("{run}/{target}"))
        .collect();
    if !missing.is_empty() {
        return Err(format!(
            "planned current batches need grading; plan and authorize the queue again: {}",
            missing.join(", ")
        ));
    }
    batches
        .iter()
        .map(|key| {
            Ok((
                key.clone(),
                text(rows[key], "input_fingerprint")?.to_string(),
            ))
        })
        .collect()
}

#[cfg(test)]
mod tests;
