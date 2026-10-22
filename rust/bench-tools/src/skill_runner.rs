//! Run one frozen skill review in a fresh Codex `exec` or Claude Code print-mode session.
//!
//! The run supplies `inputs/skill-pin.json`, the frozen tree under `inputs/skill/`, `inputs/invocation.md`,
//! `inputs/runner.json` and `inputs/shared-policy.md`. The attempt directory must already exist and must not hold a
//! `home`. The runner refuses unless the run's `runtimes.frozen.json` pins this bench-tools, checks the run, arm,
//! skill, target and clone pins, writes the attempt's `input.md`, creates a clean home before copying credentials,
//! starts one new root session, and keeps its raw output, transcripts, usage, session evidence and native skill
//! artifacts. It never resumes a session, and normalizes the native report with this bench-tools. When
//! `runner.json` sets `cross_model_review_mode` to `off`, the runner writes that setting to CE's local config in the
//! clone and excludes the file from Git before it records the clone's tree identity.
//!
//! Codex runs with project documents and apps off and every skill but the frozen one disabled. Claude runs in
//! `--safe-mode`, which disables CLAUDE.md discovery, installed skills, plugins, hooks, MCP servers and custom
//! agents, so the frozen skill is read by path from the attempt's private work directory.
//! `CLAUDE_CODE_SUBAGENT_MODEL` pins every Agent call to the arm's model, and the transcripts are then checked
//! request by request. Both environments are built from scratch, so the orchestrating session's variables never
//! reach the reviewer.
//!
//! Exit codes: 0 the session completed and every observed reviewer context used the pinned model and effort; 1 the
//! session completed with a pin or policy violation; 2 an input is missing, a setup command fails, or the reviewer
//! stops.

use crate::clean_context;
use crate::execution_policy;
use crate::file_attempt;
use crate::network_gate;
use crate::normalize_review::utc_now;
use crate::packet_selection;
use crate::pyjson::{self, Dumps, Duplicates, Object, Value};
use crate::pytext::{self, PyPath};
use crate::review_isolation::{self, resolve};
use crate::sha256;
use crate::usage;
use std::ffi::OsStr;
use std::fmt::Write as _;
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

type R<T> = Result<T, String>;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Client {
    Codex,
    Claude,
}

impl Client {
    fn kind(self) -> &'static str {
        match self {
            Self::Codex => "codex-skill",
            Self::Claude => "claude-skill",
        }
    }
}

pub struct Arguments {
    pub run: PathBuf,
    pub attempt_dir: PathBuf,
    pub clone: PathBuf,
    pub packet: PathBuf,
    pub arm: PathBuf,
    pub target: String,
    /// Defaults to the checkout's `bench/rates.current.json`.
    pub rates: Option<PathBuf>,
}

/// What the runner reports: a summary line on stdout, violations on stderr, and the exit code.
pub struct Outcome {
    pub stdout: String,
    pub stderr: String,
    pub code: u8,
}

// bwrap-v1 shows the reviewer the system read-only, its own attempt directory read-write, the pinned client and
// toolchains read-only, and a private /tmp. Every home directory, Windows mount and shared scratch area is replaced
// by an empty tmpfs, so other attempts, reference answers and host caches are unreachable. The network stays shared
// because target tests may use it.
pub const SANDBOX_HIDDEN: [&str; 7] = [
    "/home",
    "/mnt",
    "/media",
    "/srv",
    "/Docker",
    "/var/tmp",
    "/run/user",
];

const CLAUDE_TOOLS_ALLOWED: &str = "Bash,Read,Write,Edit,Glob,Grep,Agent";
// The skill's cross-model peer would send the review to another client and model. Only an executed program counts:
// the skill's own reports and briefs name the peer script when recording that it did not run.
const PEER_PROGRAMS: [&str; 4] = [
    "codex",
    "claude",
    "cross-model-adversarial-review.sh",
    "peer-job-runner.py",
];
// CE reads `cross_model_review_mode` from this file at the repository root before its own `config.yaml`.
const CE_LOCAL_CONFIG: &str = ".compound-engineering/config.local.yaml";
const COMMAND_PREFIXES: [&str; 11] = [
    "bash", "sh", "zsh", "exec", "nohup", "env", "command", "time", "setsid", "python", "python3",
];

// --- values and files ------------------------------------------------------------------------

fn get<'a>(value: &'a Value, key: &str) -> &'a Value {
    static NULL: Value = Value::Null;
    value
        .as_object()
        .and_then(|object| object.get(key))
        .unwrap_or(&NULL)
}

fn text(value: &str) -> Value {
    Value::Str(value.to_string())
}

fn int(value: impl ToString) -> Value {
    Value::Int(value.to_string())
}

fn path_value(path: &Path) -> Value {
    text(&path.to_string_lossy())
}

fn object(pairs: Vec<(&str, Value)>) -> Value {
    Value::Object(
        pairs
            .into_iter()
            .map(|(key, value)| (key.to_string(), value))
            .collect::<Object>(),
    )
}

fn texts(values: &[String]) -> Value {
    Value::Array(values.iter().map(|value| text(value)).collect())
}

fn compact(value: &Value) -> String {
    pyjson::dumps(value, &Dumps::default())
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

fn failed(path: &Path) -> impl Fn(std::io::Error) -> String + '_ {
    move |error| pytext::os_error(&error, path.as_os_str().as_bytes())
}

fn read_bytes(path: &Path) -> R<Vec<u8>> {
    fs::read(path).map_err(failed(path))
}

/// `Path.read_text(encoding="utf-8")`: strict UTF-8 with universal newlines.
fn read_text(path: &Path) -> R<String> {
    let text = pytext::decode_utf8(&read_bytes(path)?)
        .map_err(|error| format!("{}: {error}", path.display()))?;
    Ok(pytext::universal_newlines(&text))
}

fn read_json(path: &Path) -> R<Value> {
    let parsed = read_text(path).and_then(|text| {
        pyjson::loads(&text, Duplicates::KeepLast).map_err(|error| error.to_string())
    });
    parsed.map_err(|error| format!("cannot read {}: {error}", path.display()))
}

fn write(path: &Path, content: &str) -> R<()> {
    fs::write(path, content).map_err(failed(path))
}

fn append(path: &Path, content: &str) -> R<()> {
    let mut existing = read_text(path)?;
    existing.push_str(content);
    write(path, &existing)
}

fn sha_of(bytes: &[u8]) -> String {
    sha256::hex_digest(bytes)
}

fn rstrip(text: &str) -> &str {
    text.trim_end_matches(pytext::is_space)
}

fn chmod(path: &Path, mode: u32) -> R<()> {
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).map_err(failed(path))
}

fn mkdir(path: &Path) -> R<()> {
    match fs::create_dir(path) {
        Err(error) if error.kind() != std::io::ErrorKind::AlreadyExists => Err(failed(path)(error)),
        _ => Ok(()),
    }
}

fn mkdirs(path: &Path) -> R<()> {
    fs::create_dir_all(path).map_err(failed(path))
}

/// `shutil.copy2` without the timestamps, which nothing reads.
fn copy(source: &Path, destination: &Path) -> R<()> {
    fs::copy(source, destination)
        .map(|_| ())
        .map_err(|error| format!("{} -> {}: {error}", source.display(), destination.display()))
}

/// `shutil.copytree` onto a new destination.
fn copy_tree(source: &Path, destination: &Path) -> R<()> {
    fs::create_dir(destination).map_err(failed(destination))?;
    for entry in fs::read_dir(source).map_err(failed(source))? {
        let entry = entry.map_err(failed(source))?;
        let path = entry.path();
        let target = destination.join(entry.file_name());
        if entry.file_type().map_err(failed(&path))?.is_dir() {
            copy_tree(&path, &target)?;
        } else {
            copy(&path, &target)?;
        }
    }
    let mode = fs::metadata(source).map_err(failed(source))?.permissions();
    fs::set_permissions(destination, mode).map_err(failed(destination))
}

/// Everything under `root`, as `Path.rglob("*")` lists it: links are listed but never entered. Sorted as Python
/// sorts paths, by component.
fn descendants(root: &Path) -> Vec<PathBuf> {
    fn walk(directory: &Path, found: &mut Vec<PathBuf>) {
        let Ok(entries) = fs::read_dir(directory) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let is_dir = entry.file_type().is_ok_and(|kind| kind.is_dir());
            found.push(path.clone());
            if is_dir {
                walk(&path, found);
            }
        }
    }
    let mut found = Vec::new();
    walk(root, &mut found);
    found.sort();
    found
}

fn named(path: &Path, prefix: &str, suffix: &str) -> bool {
    path.file_name()
        .and_then(OsStr::to_str)
        .is_some_and(|name| {
            name.len() >= prefix.len() + suffix.len()
                && name.starts_with(prefix)
                && name.ends_with(suffix)
        })
}

/// The files under `root`, with their size and SHA-256, as `{path, bytes, sha256}` rows.
fn file_rows(root: &Path) -> R<Vec<Value>> {
    let mut rows = Vec::new();
    for path in descendants(root) {
        if path.is_file() {
            let content = read_bytes(&path)?;
            let relative = path.strip_prefix(root).unwrap_or(&path);
            rows.push(object(vec![
                ("path", path_value(relative)),
                ("bytes", int(content.len())),
                ("sha256", text(&sha_of(&content))),
            ]));
        }
    }
    Ok(rows)
}

/// The skill tree's identity: SHA-256 over each file's relative path, a NUL and its content, in path order.
pub fn skill_tree_hash(root: &Path) -> R<(String, Vec<Value>)> {
    let all = descendants(root);
    let files: Vec<&PathBuf> = all.iter().filter(|path| path.is_file()).collect();
    if files.is_empty() || all.iter().any(|path| path.is_symlink()) {
        return Err(format!(
            "skill tree is empty or contains symlinks: {}",
            root.display()
        ));
    }
    let mut digest = sha256::Sha256::new();
    let mut rows = Vec::new();
    for path in files {
        let relative = path.strip_prefix(root).unwrap_or(path);
        let content = read_bytes(path)?;
        digest.update(relative.as_os_str().as_bytes());
        digest.update(b"\0");
        digest.update(&content);
        rows.push(object(vec![
            ("path", path_value(relative)),
            ("bytes", int(content.len())),
            ("sha256", text(&sha_of(&content))),
        ]));
    }
    Ok((sha256::hex(&digest.finalize()), rows))
}

// --- processes -------------------------------------------------------------------------------

pub struct Ran {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

fn exit_code(status: ExitStatus) -> i32 {
    use std::os::unix::process::ExitStatusExt;
    status
        .code()
        .unwrap_or_else(|| -status.signal().unwrap_or(0))
}

fn decoded(bytes: &[u8]) -> String {
    pytext::universal_newlines(&String::from_utf8_lossy(bytes))
}

fn described(argv: &[String]) -> String {
    argv.join(" ")
}

/// Run `argv` and capture its output, with `env` replacing the environment when given.
pub fn run(argv: &[String], env: Option<&[(String, String)]>) -> R<Ran> {
    let mut command = Command::new(&argv[0]);
    command.args(&argv[1..]).stdin(Stdio::null());
    if let Some(env) = env {
        command.env_clear().envs(env.iter().cloned());
    }
    let done = command
        .output()
        .map_err(|error| format!("command failed: {}: {error}", described(argv)))?;
    Ok(Ran {
        code: exit_code(done.status),
        stdout: decoded(&done.stdout),
        stderr: decoded(&done.stderr),
    })
}

fn git(clone: &Path, arguments: &[&str]) -> R<String> {
    let done = Command::new("git")
        .arg("-C")
        .arg(clone)
        .args(arguments)
        .stdin(Stdio::null())
        .output()
        .map_err(|error| format!("git: {error}"))?;
    if !done.status.success() {
        return Err(format!(
            "git -C {} {}: {}",
            clone.display(),
            arguments.join(" "),
            pytext::strip(&decoded(&done.stderr))
        ));
    }
    Ok(pytext::strip(&decoded(&done.stdout)).to_string())
}

/// The clone's commit and working-tree status, hashed.
fn tree_identity(clone: &Path) -> R<String> {
    let state = format!(
        "{}\n{}\n",
        git(clone, &["rev-parse", "HEAD"])?,
        git(clone, &["status", "--porcelain", "--untracked-files=all"])?
    );
    Ok(sha_of(state.as_bytes()))
}

/// Turn CE's cross-model peer off in the clone when `runner.json` asks for it. Git's exclude file hides the
/// config, so the tree identity and the clean-clone checks see the pinned tree.
fn configure_cross_model_review(clone: &Path, runner: &Value) -> R<()> {
    match get(runner, "cross_model_review_mode") {
        Value::Null => return Ok(()),
        mode if mode.as_str() == Some("off") => {}
        _ => return Err("cross_model_review_mode must be 'off' when set".to_string()),
    }
    let config = clone.join(CE_LOCAL_CONFIG);
    if config.symlink_metadata().is_ok() {
        return Err(format!("refusing to replace the clone's {CE_LOCAL_CONFIG}"));
    }
    if let Some(parent) = config.parent() {
        mkdirs(parent)?;
    }
    write(&config, "cross_model_review_mode: off\n")?;
    let exclude = clone.join(git(clone, &["rev-parse", "--git-path", "info/exclude"])?);
    if let Some(parent) = exclude.parent() {
        mkdirs(parent)?;
    }
    let mut excluded = if exclude.exists() {
        read_text(&exclude)?
    } else {
        String::new()
    };
    if !excluded.is_empty() && !excluded.ends_with('\n') {
        excluded.push('\n');
    }
    let _ = writeln!(excluded, "/{CE_LOCAL_CONFIG}");
    write(&exclude, &excluded)?;
    let status = git(
        clone,
        &[
            "status",
            "--porcelain",
            "--untracked-files=all",
            "--",
            CE_LOCAL_CONFIG,
        ],
    )?;
    if !status.is_empty() {
        return Err(format!("{CE_LOCAL_CONFIG} left the clone's status unclean"));
    }
    Ok(())
}

pub fn mise_paths() -> R<String> {
    if review_isolation::which("mise").is_none() {
        return Ok(String::new());
    }
    let resolved = run(&["mise".to_string(), "bin-paths".to_string()], None)?;
    if resolved.code != 0 {
        return Ok(String::new());
    }
    Ok(pytext::splitlines(&resolved.stdout)
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(":"))
}

fn search_path(runtime_path: &str) -> String {
    let inherited = std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin".to_string());
    if runtime_path.is_empty() {
        inherited
    } else {
        format!("{runtime_path}:{inherited}")
    }
}

fn source_home() -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
}

/// Run the reviewer with the prompt on stdin and its streams in the attempt, killed after `timeout` seconds;
/// returns its exit code, 124 on a timeout.
fn review(
    command: &[String],
    attempt: &Path,
    input: &Path,
    clone: &Path,
    env: &[(String, String)],
    timeout: u64,
) -> R<i32> {
    let open = |path: &Path| fs::File::create(path).map_err(failed(path));
    let mut child = Command::new(&command[0])
        .args(&command[1..])
        .stdin(fs::File::open(input).map_err(failed(input))?)
        .stdout(open(&attempt.join("stdout.jsonl"))?)
        .stderr(open(&attempt.join("stderr.txt"))?)
        .current_dir(clone)
        .env_clear()
        .envs(env.iter().cloned())
        .spawn()
        .map_err(|error| format!("{}: {error}", command[0]))?;
    let deadline = Instant::now().checked_add(Duration::from_secs(timeout));
    loop {
        if let Some(status) = child.try_wait().map_err(|error| error.to_string())? {
            return Ok(exit_code(status));
        }
        if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            let _ = child.kill();
            let _ = child.wait();
            return Ok(124);
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

fn timeout_seconds(runner: &Value) -> R<u64> {
    let value = match get(runner, "timeout_seconds") {
        Value::Null => return Ok(5400),
        Value::Int(digits) => digits.parse::<i64>().ok(),
        Value::Float(number) if number.is_finite() => Some(number.trunc() as i64),
        Value::Str(digits) => pytext::strip(digits).parse::<i64>().ok(),
        _ => None,
    };
    value
        .map(|seconds| u64::try_from(seconds).unwrap_or(0))
        .ok_or_else(|| "timeout_seconds must be an integer".to_string())
}

fn number(value: &Value, key: &str) -> R<f64> {
    match get(value, key) {
        Value::Int(digits) => digits.parse().map_err(|_| format!("'{key}'")),
        Value::Float(number) => Ok(*number),
        Value::Null => Err(format!("'{key}'")),
        other => Err(format!("{key} is a {}", other.type_name())),
    }
}

fn ratio(rate: &Value, key: &str) -> R<String> {
    let input = number(rate, "input")?;
    if input == 0.0 {
        return Err("float division by zero".to_string());
    }
    Ok(usage::general(number(rate, key)? / input))
}

/// The newest dated rate for `model` in the rates file.
fn rates_for(path: &Path, model: &str) -> R<Value> {
    let catalog = read_json(path)?;
    let mut matches: Vec<&Value> = get(&catalog, "rates")
        .as_array()
        .unwrap_or_default()
        .iter()
        .filter(|row| get(row, "model").as_str() == Some(model))
        .collect();
    matches.sort_by(|a, b| {
        let key = |row: &Value| get(row, "as_of").as_str().unwrap_or("").to_string();
        key(a).cmp(&key(b))
    });
    matches.last().map(|row| (*row).clone()).ok_or_else(|| {
        format!(
            "no dated rate entry for requested model {} in {}",
            pytext::repr_str(model),
            path.display()
        )
    })
}

// --- the frozen run --------------------------------------------------------------------------

struct Prepared {
    run_dir: PathBuf,
    attempt: PathBuf,
    clone: PathBuf,
    entry: Value,
    model: String,
    effort: String,
    skill_source: PathBuf,
    tree_hash: String,
    runner: Value,
    cache: PathBuf,
    work: PathBuf,
    artifact_root: PathBuf,
    skill_name: String,
    input: PathBuf,
    prompt: String,
    tools: PathBuf,
    target_dir: PathBuf,
    packet: PathBuf,
    policy: u64,
}

fn strict(path: &Path) -> R<PathBuf> {
    path.canonicalize().map_err(failed(path))
}

fn escapes(relative: &Path) -> bool {
    relative.is_absolute()
        || relative
            .components()
            .any(|part| matches!(part, std::path::Component::ParentDir))
}

fn field<'a>(value: &'a Value, key: &str) -> R<&'a str> {
    get(value, key)
        .as_str()
        .ok_or_else(|| format!("'{key}' is missing or not a string"))
}

fn render_run_policy(manifest: &Value, bench: &Path, target_dir: &Path, clone: &Path) -> R<String> {
    let policy = get(manifest, "execution_policy");
    let (allowance, unavailable) =
        execution_policy::task_texts(bench.parent().unwrap_or(bench), manifest, target_dir)?;
    Ok(format!(
        "## Shared execution policy\n\n{}\n\n**Execution allowance.** {} {}\n\n**Unavailable.** {}\n\nRepository path: `{}`.\n",
        pytext::strip(field(policy, "branch_layout")?),
        pytext::strip(field(policy, "allowance")?),
        pytext::strip(&allowance),
        pytext::strip(&unavailable),
        clone.display()
    ))
}

fn check_inputs(run_dir: &Path) -> R<()> {
    let inputs = run_dir.join("inputs");
    let pin = read_json(&inputs.join("input-pin.json"))?;
    let rows = get(&pin, "files").as_array().unwrap_or_default();
    let mut pinned: Vec<&str> = rows
        .iter()
        .map(|row| get(row, "path").as_str().unwrap_or(""))
        .collect();
    pinned.sort_unstable();
    pinned.dedup();
    if pinned != ["invocation.md", "runner.json", "shared-policy.md"] {
        return Err(
            "input pin must cover exactly ['invocation.md', 'runner.json', 'shared-policy.md']"
                .to_string(),
        );
    }
    let inputs_resolved = resolve(&inputs);
    for row in rows {
        let name = field(row, "path")?;
        let relative = Path::new(name);
        if escapes(relative) {
            return Err(format!(
                "invalid pinned input path: {}",
                pytext::repr_str(name)
            ));
        }
        let pinned_path = strict(&inputs.join(relative))?;
        if !pinned_path.starts_with(&inputs_resolved) {
            return Err(format!(
                "pinned input escapes inputs directory: {}",
                pytext::repr_str(name)
            ));
        }
        let data = read_bytes(&pinned_path)?;
        let bytes = get(row, "bytes");
        if !bytes.py_eq(&int(data.len())) || get(row, "sha256").as_str() != Some(&sha_of(&data)) {
            return Err(format!("frozen run input changed: {name}"));
        }
    }
    Ok(())
}

/// Check the frozen run, arm, skill, target and clone pins; write the attempt's `input.md`.
fn prepare_attempt(arguments: &Arguments, client: Client) -> R<Prepared> {
    let run_dir = strict(&arguments.run)?;
    file_attempt::check_pin(&run_dir.to_string_lossy(), "launch")?;
    let tools = review_isolation::own_executable()?;
    let attempt = strict(&arguments.attempt_dir)?;
    let clone = strict(&arguments.clone)?;
    let packet_path = strict(&arguments.packet)?;
    let arm_path = strict(&arguments.arm)?;
    let bench = bench_dir(&run_dir);
    let manifest = read_json(&run_dir.join("manifest.json"))?;
    if !get(&manifest, "frozen_at").truthy() || !get(&manifest, "freeze_commit").truthy() {
        return Err("run manifest is not frozen".to_string());
    }
    let arm = read_json(&arm_path)?;
    let arm_id = get(&arm, "id");
    let entries: Vec<&Value> = get(&manifest, "arms")
        .as_array()
        .unwrap_or_default()
        .iter()
        .filter(|entry| get(entry, "id").py_eq(arm_id))
        .collect();
    if entries.len() != 1 {
        return Err(format!(
            "run manifest must contain exactly one {} arm entry",
            arm_id.py_repr()
        ));
    }
    let entry = entries[0].clone();
    if get(&arm, "kind").as_str() != Some(client.kind()) {
        return Err(format!("this runner requires a {} arm", client.kind()));
    }
    let (model, effort) = match (get(&arm, "model").as_str(), get(&arm, "effort").as_str()) {
        (Some(model), Some(effort)) if !model.is_empty() && !effort.is_empty() => {
            (model.to_string(), effort.to_string())
        }
        _ => return Err("the arm must pin both model and reasoning effort".to_string()),
    };
    if get(&entry, "arm_file_sha256").as_str() != Some(&sha_of(&read_bytes(&arm_path)?)) {
        return Err("arm file hash differs from the frozen run manifest".to_string());
    }
    let inputs = run_dir.join("inputs");
    let pin = read_json(&inputs.join("skill-pin.json"))?;
    let skill_source = inputs.join("skill");
    let (tree_hash, rows) = skill_tree_hash(&skill_source)?;
    if get(&pin, "tree_hash").as_str() != Some(&tree_hash)
        || get(&entry, "resolved_skill_tree").as_str() != Some(&tree_hash)
    {
        return Err("frozen skill tree hash does not match its pin or run manifest".to_string());
    }
    if !Value::Array(rows).py_eq(get(&pin, "files")) {
        return Err("frozen skill files do not match skill-pin.json".to_string());
    }
    check_inputs(&run_dir)?;
    let invocation = read_text(&inputs.join("invocation.md"))?;
    let runner = read_json(&inputs.join("runner.json"))?;
    let policy = execution_policy::check(&manifest)?;
    let recorded = execution_policy::runner_version(&runner)?;
    if recorded != policy {
        return Err(format!(
            "runner.json records execution policy {recorded} but the run manifest's is {policy}"
        ));
    }
    if policy == execution_policy::OPEN_NETWORK
        && get(&runner, "sandbox").as_str() != Some("bwrap-v2")
    {
        return Err(format!(
            "execution policy {policy} needs runner.json sandbox bwrap-v2, which gates the network"
        ));
    }
    let target_id = arguments.target.as_str();
    let cohort: Vec<&Value> = get(&manifest, "cohort")
        .as_array()
        .unwrap_or_default()
        .iter()
        .filter(|row| get(row, "target").as_str() == Some(target_id))
        .collect();
    if cohort.len() != 1 {
        return Err(format!(
            "target {} is not uniquely in the run cohort",
            pytext::repr_str(target_id)
        ));
    }
    let target_dir = bench.join("targets").join(target_id);
    let target = read_json(&target_dir.join("target.json"))?;
    let py = |path: &Path| PyPath::new(path.as_os_str().as_bytes());
    let selected = packet_selection::select(
        &py(&run_dir),
        &manifest,
        &py(&target_dir),
        &py(bench.parent().unwrap_or(&bench)),
    )
    .map_err(|error| {
        let message = error
            .caught()
            .map_or_else(|| error.describe(), str::to_string);
        format!("packet selection failed: {message}")
    })?;
    if get(cohort[0], "packet_sha256").as_str() != Some(&sha_of(&read_bytes(&packet_path)?)) {
        return Err("PR packet differs from the frozen cohort hash".to_string());
    }
    let replaced = manifest
        .as_object()
        .is_some_and(|object| object.get("packet_replacements").is_some());
    if replaced && packet_path != resolve(&selected.to_path_buf()) {
        return Err("--packet differs from the frozen packet selection".to_string());
    }
    let merge_base = field(&target, "merge_base")?;
    let head = field(&target, "head")?;
    if git(&clone, &["rev-parse", "main"])? != merge_base
        || git(&clone, &["rev-parse", "review-head"])? != head
    {
        return Err("clone does not have the pinned main and review-head commits".to_string());
    }
    let sibling = |suffix: &str| {
        let mut name = clone.as_os_str().to_os_string();
        name.push(suffix);
        PathBuf::from(name)
    };
    let (cache, work) = (sibling("-cache"), sibling("-work"));
    mkdir(&cache)?;
    mkdir(&work)?;
    let relative = Path::new(field(&runner, "artifact_root")?);
    if escapes(relative) {
        return Err("artifact_root must be relative to the private work directory".to_string());
    }
    let artifact_root = resolve(&work.join(relative));
    if !artifact_root.starts_with(resolve(&work)) {
        return Err("artifact_root resolves outside the private work directory".to_string());
    }
    if let Some(parent) = artifact_root.parent() {
        mkdirs(parent)?;
    }
    if artifact_root.exists() {
        return Err(format!(
            "refusing reused native artifact directory: {}",
            artifact_root.display()
        ));
    }
    let packet = read_text(&packet_path)?;
    let skill_name = field(&pin, "skill_id")?.to_string();
    let skill_path = work.join("frozen-skill").join("SKILL.md");
    let invocation_text = invocation
        .replace("{REPORT_ROOT}", &artifact_root.to_string_lossy())
        .replace("{SKILL_PATH}", &skill_path.to_string_lossy())
        .replace("{TARGET}", target_id)
        .replace("{BASE_SHA}", merge_base)
        .replace("{HEAD_SHA}", head)
        .replace("{MODEL}", &model)
        .replace("{EFFORT}", &effort)
        .replace("{PACKET}", rstrip(&packet));
    let shared_policy = read_text(&inputs.join("shared-policy.md"))?;
    let task = if invocation.contains("{PACKET}") {
        String::new()
    } else {
        format!("## Review task\n\n{}\n\n", rstrip(&packet))
    };
    let prompt = format!(
        "{}\n\n{task}{}\n## Selected skill invocation\n\n{}\n\n## Frozen skill: {skill_name}\n\nRead and follow the frozen skill at `{}`. Load reference files only when the skill workflow calls for them. Do not load another skill.\n",
        rstrip(&shared_policy),
        render_run_policy(&manifest, &bench, &target_dir, &clone)?,
        pytext::strip(&invocation_text),
        skill_path.display()
    );
    let input = attempt.join("input.md");
    write(&input, &prompt)?;
    configure_cross_model_review(&clone, &runner)?;
    write(
        &attempt.join("tree-before.txt"),
        &format!("{}\n", tree_identity(&clone)?),
    )?;
    Ok(Prepared {
        run_dir,
        attempt,
        clone,
        entry,
        model,
        effort,
        skill_source,
        tree_hash,
        runner,
        cache,
        work,
        artifact_root,
        skill_name,
        input,
        prompt,
        tools,
        target_dir,
        packet: packet_path,
        policy,
    })
}

/// The checkout's `bench` directory, two levels above the run.
fn bench_dir(run_dir: &Path) -> PathBuf {
    PathBuf::from(file_attempt::bench_default(&run_dir.to_string_lossy(), ""))
}

/// Copy the frozen skill to the attempt's private work directory, where the prompt names it.
fn copy_frozen_skill(prepared: &Prepared) -> R<PathBuf> {
    let skill_work = prepared.work.join("frozen-skill");
    if skill_work.exists() {
        return Err(format!(
            "refusing reused skill workspace: {}",
            skill_work.display()
        ));
    }
    copy_tree(&prepared.skill_source, &skill_work)?;
    if skill_tree_hash(&skill_work)?.0 != prepared.tree_hash {
        return Err("attempt-local skill copy differs from the frozen skill tree".to_string());
    }
    Ok(skill_work)
}

fn make_read_only(root: &Path) -> R<()> {
    for path in descendants(root) {
        chmod(&path, if path.is_dir() { 0o555 } else { 0o444 })?;
    }
    chmod(root, 0o555)
}

/// The pinned client, refused unless it hashes to the frozen runner configuration.
fn pinned_client(runner: &Value, name: &str, label: &str) -> R<(PathBuf, String)> {
    let configured = get(runner, &format!("{name}_executable"))
        .as_str()
        .ok_or_else(|| {
            format!("{label} executable is not pinned to an available absolute path: '{name}_executable'")
        })?;
    let executable = strict(Path::new(configured)).map_err(|error| {
        format!("{label} executable is not pinned to an available absolute path: {error}")
    })?;
    let hash = sha_of(&read_bytes(&executable)?);
    if get(runner, &format!("{name}_executable_sha256")).as_str() != Some(&hash) {
        return Err(format!(
            "{label} executable hash differs from the frozen runner configuration"
        ));
    }
    Ok((executable, hash))
}

fn sandbox_gate(profile: &str, prepared: &Prepared) -> R<Option<network_gate::Gate>> {
    if profile == "bwrap-v2" {
        network_gate::Gate::start(&prepared.attempt, &prepared.tools).map(Some)
    } else {
        Ok(None)
    }
}

/// Start `profile`'s sandbox for a client installed under `client_dir`: its gate, its `bwrap` prefix and its
/// receipt, written to `sandbox.json` with the run's execution policy. bwrap-v2 shows only uv's managed Pythons,
/// which virtualenvs link to: the rest of uv's data directory holds the credentials `uv auth login` saves.
fn start_sandbox(
    prepared: &Prepared,
    profile: &str,
    client_dir: &Path,
    aliases: &[String],
) -> R<(Vec<String>, Value, Option<network_gate::Gate>)> {
    let source = source_home();
    let uv = source.join(".local/share/uv");
    let readonly = [
        client_dir.to_path_buf(),
        source.join(".local/share/mise"),
        if profile == "bwrap-v1" {
            uv
        } else {
            uv.join("python")
        },
        source.join(".bun"),
    ];
    let gate = sandbox_gate(profile, prepared)?;
    let (prefix, mut receipt) = sandbox_command(
        profile,
        &prepared.attempt,
        &prepared.clone,
        &readonly,
        aliases,
        gate.as_ref(),
    )?;
    if let Value::Object(fields) = &mut receipt {
        fields.insert("execution_policy".to_string(), int(prepared.policy));
    }
    write(&prepared.attempt.join("sandbox.json"), &indented(&receipt))?;
    Ok((prefix, receipt, gate))
}

fn sandbox_profile(runner: &Value) -> Option<String> {
    let configured = get(runner, "sandbox");
    if configured.truthy() {
        return Some(configured.py_str());
    }
    std::env::var("BENCH_SANDBOX")
        .ok()
        .filter(|profile| !profile.is_empty())
}

/// Decode the octal escapes `/proc/self/mountinfo` writes for spaces, tabs, newlines and backslashes.
fn unescape_mount(field: &str) -> String {
    let bytes = field.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        let octal = bytes
            .get(index + 1..index + 4)
            .filter(|digits| digits.iter().all(|digit| (b'0'..=b'7').contains(digit)));
        match (bytes[index], octal) {
            (b'\\', Some(digits)) => {
                let value = digits
                    .iter()
                    .fold(0u32, |value, digit| value * 8 + u32::from(digit - b'0'));
                out.push(u8::try_from(value).unwrap_or(b'?'));
                index += 4;
            }
            (b'\\', None) if bytes.get(index + 1) == Some(&b'\\') => {
                out.push(b'\\');
                index += 2;
            }
            (byte, _) => {
                out.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn joined(base: &Path, relative: &Path) -> PathBuf {
    if relative.as_os_str().is_empty() {
        base.to_path_buf()
    } else {
        base.join(relative)
    }
}

/// The paths under a hidden directory where another mount of the attempt's filesystem shows the attempt, as WSL's
/// `/mnt/wslg/distro` shows the whole distribution. Codex's own Linux sandbox binds its scratch directory at every
/// path the mount table gives it; the table still lists a mount the tmpfs covers, so that path has to exist or no
/// reviewer command starts.
pub fn hidden_aliases(attempt: &Path, hidden: &[String], mountinfo: &str) -> Vec<String> {
    let mounts: Vec<(String, PathBuf, PathBuf)> = mountinfo
        .lines()
        .filter_map(|line| {
            let fields: Vec<&str> = line.split_whitespace().collect();
            (fields.len() > 4).then(|| {
                (
                    fields[2].to_string(),
                    PathBuf::from(unescape_mount(fields[3])),
                    PathBuf::from(unescape_mount(fields[4])),
                )
            })
        })
        .collect();
    let mut holding: Option<&(String, PathBuf, PathBuf)> = None;
    for mount in mounts.iter().filter(|mount| attempt.starts_with(&mount.2)) {
        if holding.is_none_or(|best| mount.2.components().count() > best.2.components().count()) {
            holding = Some(mount);
        }
    }
    let Some((device, root, point)) = holding else {
        return Vec::new();
    };
    let inner = joined(root, attempt.strip_prefix(point).unwrap_or(attempt));
    let mut aliases: Vec<String> = mounts
        .iter()
        .filter(|(number, base, other)| {
            number == device
                && inner.starts_with(base)
                && hidden.iter().any(|path| other.starts_with(path))
        })
        .map(|(_, base, other)| {
            joined(other, inner.strip_prefix(base).unwrap_or(&inner))
                .to_string_lossy()
                .into_owned()
        })
        .filter(|alias| alias.as_str() != attempt.to_string_lossy())
        .collect();
    aliases.sort();
    aliases.dedup();
    aliases
}

fn hidden_directories() -> Vec<String> {
    SANDBOX_HIDDEN
        .iter()
        .filter(|path| Path::new(path).is_dir())
        .map(ToString::to_string)
        .collect()
}

/// The `bwrap` prefix for `profile` and its receipt. bwrap-v2 is bwrap-v1 in its own network namespace whose only
/// exit is `gate`, with all of `/run` hidden, since WSL interop, systemd-resolved and Tailscale reach the host
/// through sockets there.
pub fn sandbox_command(
    profile: &str,
    attempt: &Path,
    clone: &Path,
    readonly: &[PathBuf],
    aliases: &[String],
    gate: Option<&network_gate::Gate>,
) -> R<(Vec<String>, Value)> {
    let gate = match (profile, gate) {
        ("bwrap-v1", _) => None,
        ("bwrap-v2", Some(gate)) => Some(gate),
        ("bwrap-v2", None) => return Err("bwrap-v2 needs a network gate".to_string()),
        _ => {
            return Err(format!(
                "unknown sandbox profile {}",
                pytext::repr_str(profile)
            ));
        }
    };
    let bwrap = review_isolation::which("bwrap")
        .ok_or_else(|| "bwrap is not installed".to_string())?
        .to_string_lossy()
        .into_owned();
    let mut prefix: Vec<String> = [
        bwrap.as_str(),
        "--ro-bind",
        "/",
        "/",
        "--dev",
        "/dev",
        "--proc",
        "/proc",
        "--unshare-pid",
        "--unshare-ipc",
        "--die-with-parent",
    ]
    .map(String::from)
    .to_vec();
    let mut hidden = hidden_directories();
    if gate.is_some() {
        prefix.push("--unshare-net".to_string());
        hidden.retain(|path| !path.starts_with("/run/"));
        hidden.push("/run".to_string());
    }
    for path in &hidden {
        prefix.extend(["--tmpfs".to_string(), path.clone()]);
    }
    // WSL links /etc/resolv.conf into /mnt/wsl, which is hidden; target tests need DNS.
    let resolver = resolve(Path::new("/etc/resolv.conf"));
    let tools = gate.map(|gate| gate.tools().to_path_buf());
    let mut shown: Vec<String> = readonly
        .iter()
        .chain(std::iter::once(&resolver))
        .chain(tools.iter())
        .filter(|path| path.exists())
        .map(|path| path.to_string_lossy().into_owned())
        .collect();
    shown.sort();
    shown.dedup();
    for path in &shown {
        prefix.extend(["--ro-bind".to_string(), path.clone(), path.clone()]);
    }
    let attempt_text = attempt.to_string_lossy().into_owned();
    let readwrite: Vec<String> = std::iter::once(attempt_text.clone())
        .chain(aliases.iter().cloned())
        .collect();
    for path in &readwrite {
        prefix.extend(["--bind".to_string(), attempt_text.clone(), path.clone()]);
    }
    let private_tmp = attempt.join("tmp").to_string_lossy().into_owned();
    prefix.extend([
        "--bind".to_string(),
        private_tmp.clone(),
        "/tmp".to_string(),
    ]);
    if let Some(gate) = gate {
        let covered: Vec<String> = hidden.iter().cloned().chain(["/tmp".to_string()]).collect();
        let reshown: Vec<String> = shown.iter().chain(&readwrite).cloned().collect();
        prefix.extend(gate.namespace_arguments(&readwrite, &covered, &reshown)?);
    }
    prefix.extend([
        "--chdir".to_string(),
        clone.to_string_lossy().into_owned(),
        "--".to_string(),
    ]);
    let mut namespaces: Vec<String> = vec!["mount".into(), "pid".into(), "ipc".into()];
    if let Some(gate) = gate {
        prefix.extend(gate.bridge_command());
        namespaces.push("network".into());
    }
    let receipt = object(vec![
        ("profile", text(profile)),
        ("bwrap", text(&bwrap)),
        ("hidden", texts(&hidden)),
        ("readonly", texts(&shown)),
        ("readwrite", texts(&readwrite)),
        ("private_tmp", text(&private_tmp)),
        (
            "network",
            text(if gate.is_some() { "gated" } else { "shared" }),
        ),
        ("namespaces", texts(&namespaces)),
        (
            "gate",
            gate.map_or(Value::Null, network_gate::Gate::receipt),
        ),
        ("prefix", texts(&prefix)),
    ]);
    Ok((prefix, receipt))
}

fn version_number(text: &str) -> Option<String> {
    regex::Regex::new(r"\d+\.\d+\.\d+")
        .ok()?
        .find(text)
        .map(|found| found.as_str().to_string())
}

fn timing(started: &str) -> String {
    indented(&object(vec![
        ("completion_mode", text("skill-artifacts")),
        ("root_dispatched_at", text(started)),
        ("payload_validated_at", Value::Null),
        ("completed_at", Value::Null),
    ]))
}

fn elapsed(started: Instant) -> String {
    pyjson::float_repr(usage::round_to(started.elapsed().as_secs_f64(), 3))
}

fn native_artifacts(prepared: &Prepared) -> R<Vec<Value>> {
    let rows = if prepared.artifact_root.is_dir() {
        file_rows(&prepared.artifact_root)?
    } else {
        Vec::new()
    };
    write(
        &prepared.attempt.join("native-artifacts.json"),
        &indented(&object(vec![
            ("root", path_value(&prepared.artifact_root)),
            ("files", Value::Array(rows.clone())),
        ])),
    )?;
    Ok(rows)
}

fn reports_named(prepared: &Prepared, rows: &[Value], name: &Value) -> Vec<PathBuf> {
    rows.iter()
        .filter_map(|row| get(row, "path").as_str())
        .filter(|path| {
            Some(
                Path::new(path)
                    .file_name()
                    .and_then(OsStr::to_str)
                    .unwrap_or(""),
            ) == name.as_str()
        })
        .map(|path| prepared.artifact_root.join(path))
        .collect()
}

fn tool_command(prepared: &Prepared, arguments: &[&str]) -> Vec<String> {
    std::iter::once(prepared.tools.to_string_lossy().into_owned())
        .chain(arguments.iter().map(ToString::to_string))
        .collect()
}

fn normalize(prepared: &Prepared, command: &[String], violations: &mut Vec<String>) -> R<()> {
    let normalized = run(command, None)?;
    write(
        &prepared.attempt.join("normalization.txt"),
        &format!("{}{}", normalized.stdout, normalized.stderr),
    )?;
    if normalized.code != 0 {
        violations.push(format!(
            "native report normalization exited {}",
            normalized.code
        ));
    }
    Ok(())
}

/// The audit of an attempt run under `sandbox` (its receipt, or null when unsandboxed).
fn audit_command(prepared: &Prepared, sandbox: &Value, network: bool) -> Vec<String> {
    let attempt = prepared.attempt.to_string_lossy();
    let clone = prepared.clone.to_string_lossy();
    let mut command = tool_command(
        prepared,
        &[
            "attempt-audit",
            "--arm",
            "",
            "--attempt-dir",
            &attempt,
            "--clone",
            &clone,
        ],
    );
    if !matches!(sandbox, Value::Null) {
        command.extend([
            "--mount-sandbox".to_string(),
            prepared
                .attempt
                .join("sandbox.json")
                .to_string_lossy()
                .into_owned(),
        ]);
    }
    if network {
        command.push("--allow-network".to_string());
    }
    if get(sandbox, "network").as_str() == Some("gated") {
        command.extend([
            "--network-gate".to_string(),
            prepared
                .attempt
                .join("network/gate.json")
                .to_string_lossy()
                .into_owned(),
            "--target".to_string(),
            prepared.target_dir.to_string_lossy().into_owned(),
            "--packet".to_string(),
            prepared.packet.to_string_lossy().into_owned(),
        ]);
    }
    command
}

fn finish(
    prepared: &Prepared,
    ended: &str,
    exit: i32,
    rows: &[Value],
    observed: Value,
    violations: Vec<String>,
    summary: String,
) -> R<Outcome> {
    let attempt = &prepared.attempt;
    let mut timing = read_json(&attempt.join("timing.json"))?;
    let stamp = |set: bool| if set { text(ended) } else { Value::Null };
    if let Value::Object(fields) = &mut timing {
        fields.insert("completed_at".to_string(), stamp(violations.is_empty()));
        fields.insert("payload_validated_at".to_string(), stamp(!rows.is_empty()));
    }
    write(&attempt.join("timing.json"), &indented(&timing))?;
    write(&attempt.join("skill-attempt.json"), &indented(&observed))?;
    if violations.is_empty() {
        return Ok(Outcome {
            stdout: format!("{summary}\n"),
            stderr: String::new(),
            code: 0,
        });
    }
    write(
        &attempt.join("stop.json"),
        &indented(&object(vec![
            ("stopped_at", text(ended)),
            ("exit_code", int(exit)),
            ("reason", text(&violations.join("; "))),
        ])),
    )?;
    Ok(Outcome {
        stdout: String::new(),
        stderr: format!("{}\n", violations.join("\n")),
        code: if exit == 0 { 1 } else { 2 },
    })
}

fn cost_summary(id: &str, rows: usize, usage: &Value) -> R<String> {
    let cost = number(get(usage, "total"), "cost")?;
    Ok(format!("{id}: {rows} native artifact(s); ${cost:.6}"))
}

/// Run the client's skill attempt.
pub fn launch(client: Client, arguments: &Arguments) -> R<Outcome> {
    let prepared = prepare_attempt(arguments, client)?;
    let rates = arguments
        .rates
        .clone()
        .unwrap_or_else(|| bench_dir(&prepared.run_dir).join("rates.current.json"));
    match client {
        Client::Codex => codex::launch(&prepared, &rates),
        Client::Claude => claude::launch(&prepared, &rates),
    }
}

mod codex {
    use super::*;

    struct Rollouts {
        root: String,
        contexts: Vec<Value>,
        lineage: Vec<Value>,
        spawns: Vec<Value>,
        paths: Vec<PathBuf>,
    }

    fn records(path: &Path) -> R<Vec<Value>> {
        let content = read_text(path)
            .map_err(|error| format!("cannot read rollout {}: {error}", path.display()))?;
        let mut records = Vec::new();
        for (number, line) in pytext::splitlines(&content).into_iter().enumerate() {
            if pytext::strip(line).is_empty() {
                continue;
            }
            records.push(pyjson::loads(line, Duplicates::KeepLast).map_err(|error| {
                format!(
                    "{}:{}: invalid rollout JSON: {error}",
                    path.display(),
                    number + 1
                )
            })?);
        }
        Ok(records)
    }

    fn or<'a>(first: &'a Value, second: &'a Value) -> &'a Value {
        if first.truthy() { first } else { second }
    }

    fn arguments_of(payload: &Value) -> Value {
        let fallback = text("{}");
        let raw = or(
            or(get(payload, "arguments"), get(payload, "input")),
            &fallback,
        );
        let parsed = match raw {
            Value::Str(json) => pyjson::loads(json, Duplicates::KeepLast).unwrap_or(Value::Null),
            other => other.clone(),
        };
        if parsed.as_object().is_some() {
            parsed
        } else {
            Value::Object(Object::default())
        }
    }

    /// The rollouts under `sessions`: one fresh root, every child descended from it through a spawn call that forked
    /// no context and kept the pinned model and effort, and a turn context for every session.
    fn load_rollouts(sessions: &Path, model: &str, effort: &str) -> R<Rollouts> {
        let paths: Vec<PathBuf> = descendants(sessions)
            .into_iter()
            .filter(|path| named(path, "rollout-", ".jsonl"))
            .collect();
        let (mut roots, mut contexts, mut lineage, mut spawns) =
            (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        for path in &paths {
            let mut meta: Option<Value> = None;
            for record in records(path)? {
                let kind = get(&record, "type").as_str().unwrap_or("");
                let meta_id = || {
                    meta.as_ref()
                        .map_or(Value::Null, |meta| get(meta, "id").clone())
                };
                if kind == "session_meta" && meta.is_none() {
                    meta = Some(
                        or(get(&record, "payload"), &Value::Object(Object::default())).clone(),
                    );
                } else if kind == "turn_context" {
                    let payload = get(&record, "payload");
                    contexts.push(object(vec![
                        ("session_id", meta_id()),
                        ("model", get(payload, "model").clone()),
                        ("effort", get(payload, "effort").clone()),
                        (
                            "sandbox",
                            get(get(payload, "sandbox_policy"), "type").clone(),
                        ),
                    ]));
                } else if kind == "response_item" || kind == "function_call" {
                    let payload = or(get(&record, "payload"), &record);
                    let call_type = get(payload, "type").as_str().unwrap_or("");
                    let name = get(payload, "name");
                    let spawn = matches!(name.as_str(), Some("spawn_agent" | "spawn_agents"));
                    if (call_type == "function_call"
                        || call_type == "custom_tool_call"
                        || name.truthy())
                        && spawn
                    {
                        let arguments = arguments_of(payload);
                        let effort = arguments
                            .as_object()
                            .and_then(|fields| fields.get("reasoning_effort"))
                            .unwrap_or_else(|| get(&arguments, "effort"));
                        let mut keys: Vec<String> = arguments
                            .as_object()
                            .map(|fields| fields.iter().map(|(key, _)| key.clone()).collect())
                            .unwrap_or_default();
                        keys.sort();
                        spawns.push(object(vec![
                            ("session_id", meta_id()),
                            ("name", name.clone()),
                            ("fork_turns", get(&arguments, "fork_turns").clone()),
                            ("fork_context", get(&arguments, "fork_context").clone()),
                            ("model", get(&arguments, "model").clone()),
                            ("reasoning_effort", effort.clone()),
                            ("argument_keys", texts(&keys)),
                        ]));
                    }
                }
            }
            if let Some(meta) = meta.filter(Value::truthy) {
                let id = or(get(&meta, "id"), get(&meta, "session_id")).clone();
                lineage.push(object(vec![
                    ("session_id", id.clone()),
                    ("parent_thread_id", get(&meta, "parent_thread_id").clone()),
                    ("thread_source", get(&meta, "thread_source").clone()),
                    ("originator", get(&meta, "originator").clone()),
                ]));
                if !get(&meta, "parent_thread_id").truthy() {
                    roots.push(id);
                }
            }
        }
        if roots.len() != 1 {
            return Err(format!(
                "expected one fresh root session, found {}",
                roots.len()
            ));
        }
        let root = roots[0].clone();
        let key = |value: &Value| value.py_repr();
        let mut ids: Vec<String> = lineage
            .iter()
            .map(|item| key(get(item, "session_id")))
            .collect();
        let null = lineage
            .iter()
            .any(|item| matches!(get(item, "session_id"), Value::Null));
        ids.sort();
        ids.dedup();
        if null || ids.len() != lineage.len() {
            return Err("session lineage has missing or duplicate session IDs".to_string());
        }
        let by_id = |id: &Value| {
            lineage
                .iter()
                .find(|item| get(item, "session_id").py_eq(id))
        };
        for item in &lineage {
            let session = get(item, "session_id");
            let mut seen: Vec<String> = Vec::new();
            let mut current = item;
            while get(current, "parent_thread_id").truthy() {
                let parent = get(current, "parent_thread_id");
                let next = by_id(parent);
                if seen.contains(&key(parent)) || next.is_none() {
                    return Err(format!(
                        "session {} has a cyclic or unrecorded parent {}",
                        session.py_str(),
                        parent.py_str()
                    ));
                }
                seen.push(key(parent));
                current = next.unwrap_or(current);
            }
            if !get(current, "session_id").py_eq(&root) {
                return Err(format!(
                    "session {} does not descend from root {}",
                    session.py_str(),
                    root.py_str()
                ));
            }
        }
        let mut with_context: Vec<String> = contexts
            .iter()
            .map(|item| get(item, "session_id"))
            .filter(|id| id.truthy())
            .map(key)
            .collect();
        with_context.sort();
        with_context.dedup();
        if with_context != ids {
            return Err(
                "every root and child session must have a recorded turn context".to_string(),
            );
        }
        for child in lineage
            .iter()
            .filter(|item| get(item, "parent_thread_id").truthy())
        {
            let child_id = get(child, "session_id").py_str();
            let parent = get(child, "parent_thread_id");
            let calls: Vec<&Value> = spawns
                .iter()
                .filter(|call| get(call, "session_id").py_eq(parent))
                .collect();
            if calls.is_empty() {
                return Err(format!(
                    "child session {child_id} has no logged spawn call in its parent"
                ));
            }
            if calls.iter().any(|call| {
                get(call, "fork_turns").as_str() != Some("none")
                    && !matches!(get(call, "fork_context"), Value::Bool(false))
            }) {
                return Err(format!(
                    "child session {child_id} was not spawned with fork_turns=none"
                ));
            }
            let allowed = |value: &Value, pinned: &str| {
                matches!(value, Value::Null) || value.as_str() == Some(pinned)
            };
            if calls.iter().any(|call| {
                !allowed(get(call, "model"), model)
                    || !allowed(get(call, "reasoning_effort"), effort)
            }) {
                return Err(format!(
                    "child session {child_id} spawn overrode the pinned model or effort"
                ));
            }
        }
        Ok(Rollouts {
            root: root.py_str(),
            contexts,
            lineage,
            spawns,
            paths,
        })
    }

    pub(super) fn launch(prepared: &Prepared, rates_path: &Path) -> R<Outcome> {
        let (attempt, clone) = (&prepared.attempt, &prepared.clone);
        let (model, effort) = (prepared.model.as_str(), prepared.effort.as_str());
        let receipt = clean_context::prepare(attempt)?;
        let home = PathBuf::from(field(&receipt, "home")?);
        let codex_home = home.join(".codex");
        let skill_home = home.join(".agents/skills").join(&prepared.skill_name);
        mkdirs(skill_home.parent().unwrap_or(&home))?;
        copy_tree(&prepared.skill_source, &skill_home)?;
        make_read_only(&skill_home)?;
        let config = clean_context::configure_codex(&home, clone, Some(&skill_home), None)?;
        copy(&config, &attempt.join("codex-config.toml"))?;
        let config_hash = sha_of(&read_bytes(&config)?);
        let skill_work = copy_frozen_skill(prepared)?;
        let credentials = source_home().join(".codex/auth.json");
        if !credentials.is_file() {
            return Err(format!(
                "Codex credentials are missing at {}",
                credentials.display()
            ));
        }
        mkdirs(&codex_home)?;
        let temporary = attempt.join("tmp");
        mkdir(&temporary)?;
        let runtime_path = mise_paths()?;
        let mut env: Vec<(String, String)> = vec![
            ("PATH".into(), search_path(&runtime_path)),
            ("HOME".into(), home.to_string_lossy().into_owned()),
            (
                "CODEX_HOME".into(),
                codex_home.to_string_lossy().into_owned(),
            ),
            ("TMPDIR".into(), temporary.to_string_lossy().into_owned()),
            ("LANG".into(), "C.UTF-8".into()),
            ("LC_ALL".into(), "C.UTF-8".into()),
        ];
        let artifact_env = get(&prepared.runner, "artifact_root_env");
        if artifact_env.truthy() {
            env.push((
                artifact_env.py_str(),
                prepared.artifact_root.to_string_lossy().into_owned(),
            ));
        }
        let (executable, executable_hash) = pinned_client(&prepared.runner, "codex", "Codex")?;
        let path = |path: &Path| path.to_string_lossy().into_owned();
        let (cache, work) = (path(&prepared.cache), path(&prepared.work));
        let mut command: Vec<String> = vec![
            path(&executable),
            "exec".into(),
            "--json".into(),
            "--output-last-message".into(),
            path(&attempt.join("last-message.txt")),
            "--model".into(),
            model.into(),
            "--sandbox".into(),
            "workspace-write".into(),
            "--add-dir".into(),
            cache.clone(),
            "--add-dir".into(),
            work.clone(),
            "--strict-config".into(),
            "--cd".into(),
            path(clone),
        ];
        for setting in [
            format!("model_reasoning_effort=\"{effort}\""),
            format!("review_model=\"{model}\""),
            "sandbox_mode=\"workspace-write\"".into(),
            "approval_policy=\"never\"".into(),
            "sandbox_workspace_write.network_access=true".into(),
            "project_doc_max_bytes=0".into(),
            "project_doc_fallback_filenames=[]".into(),
            "features.apps=false".into(),
            "apps._default.enabled=false".into(),
            format!(
                "sandbox_workspace_write.writable_roots={}",
                compact(&texts(&[cache, work]))
            ),
        ] {
            command.extend(["-c".to_string(), setting]);
        }
        let mut sandbox = Value::Null;
        let mut gate = None;
        let network = get(&prepared.runner, "network_allowed").truthy()
            || std::env::var("BENCH_NETWORK_ALLOWED").as_deref() == Ok("1");
        if let Some(profile) = sandbox_profile(&prepared.runner) {
            let client_dir = executable
                .parent()
                .and_then(Path::parent)
                .unwrap_or(Path::new("/"));
            let mountinfo = read_text(Path::new("/proc/self/mountinfo"))?;
            let aliases = hidden_aliases(attempt, &hidden_directories(), &mountinfo);
            let (prefix, receipt, started) =
                start_sandbox(prepared, &profile, client_dir, &aliases)?;
            command.splice(0..0, prefix);
            sandbox = receipt;
            gate = started;
        }
        let timeout = timeout_seconds(&prepared.runner)?;
        let auth = codex_home.join("auth.json");
        copy(&credentials, &auth)?;
        chmod(&auth, 0o600)?;
        let mut started = Instant::now();
        let launched = (|| -> R<(String, i32)> {
            let version = run(&[path(&executable), "--version".into()], Some(&env))?;
            if version.code != 0 {
                return Err(format!(
                    "codex --version failed: {}",
                    pytext::strip(&version.stderr)
                ));
            }
            let version_text = pytext::strip(&version.stdout).to_string();
            let cli_version = regex::Regex::new(r"codex-cli\s+(\d+\.\d+\.\d+)")
                .ok()
                .and_then(|pattern| pattern.captures(&version_text))
                .and_then(|found| found.get(1))
                .map_or(version_text.clone(), |found| found.as_str().to_string());
            let frozen = get(&prepared.entry, "expected_cli_version")
                .as_str()
                .unwrap_or("");
            if version_number(frozen).is_some_and(|expected| expected != cli_version) {
                return Err(format!(
                    "Codex CLI {cli_version} differs from frozen {frozen}"
                ));
            }
            write(&attempt.join("timing.json"), &timing(&utc_now()))?;
            write(
                &attempt.join("dispatch.txt"),
                &format!(
                    "codex-cli {cli_version}\nmodel={model} effort={effort}\nskill={} skill_tree={}\ncontext_id={}\nruntime_path={}\ncodex_executable={}\ncodex_executable_sha256={executable_hash}\nconfig_sha256={config_hash}\n",
                    prepared.skill_name,
                    prepared.tree_hash,
                    get(&receipt, "context_id").py_str(),
                    if runtime_path.is_empty() {
                        "<inherited>"
                    } else {
                        &runtime_path
                    },
                    executable.display()
                ),
            )?;
            started = Instant::now();
            let exit = review(&command, attempt, &prepared.input, clone, &env, timeout)?;
            Ok((cli_version, exit))
        })();
        let _ = fs::remove_file(&auth);
        let stopped = gate.map(network_gate::Gate::stop).transpose();
        let (cli_version, exit) = launched?;
        stopped?;
        let ended = utc_now();
        append(
            &attempt.join("dispatch.txt"),
            &format!("elapsed_seconds={}\nexit={exit}\n", elapsed(started)),
        )?;
        let sessions = codex_home.join("sessions");
        let mut rollout_paths: Vec<PathBuf> = descendants(&sessions)
            .into_iter()
            .filter(|path| named(path, "rollout-", ".jsonl"))
            .collect();
        write(
            &attempt.join("tree-after.txt"),
            &format!("{}\n", tree_identity(clone)?),
        )?;
        let mut rollouts = Rollouts {
            root: String::new(),
            contexts: Vec::new(),
            lineage: Vec::new(),
            spawns: Vec::new(),
            paths: Vec::new(),
        };
        let mut root: Option<String> = None;
        match load_rollouts(&sessions, model, effort) {
            Ok(loaded) => {
                root = Some(loaded.root.clone());
                rollout_paths.clone_from(&loaded.paths);
                rollouts = loaded;
            }
            Err(error) => write(&attempt.join("rollout-error.txt"), &format!("{error}\n"))?,
        }
        let mut usage = Value::Null;
        let metered = (|| -> R<()> {
            let rate = rates_for(&resolve(rates_path), model)?;
            let Some(root) = &root else {
                return Ok(());
            };
            let prices = format!(
                "{},{}",
                get(&rate, "input").py_str(),
                get(&rate, "output").py_str()
            );
            let command = tool_command(
                prepared,
                &[
                    "usage",
                    "codex",
                    "--sessions-dir",
                    &sessions.to_string_lossy(),
                    "--session",
                    root,
                    "--prices",
                    &prices,
                    "--cached-mult",
                    &ratio(&rate, "cache_read")?,
                    "--cache-write-mult",
                    &ratio(&rate, "cache_write_5m")?,
                    "--json",
                ],
            );
            let done = run(&command, None)?;
            write(
                &attempt.join("usage.txt"),
                &format!("{}{}", done.stdout, done.stderr),
            )?;
            if done.code == 0 {
                usage = pyjson::loads(&done.stdout, Duplicates::KeepLast)
                    .map_err(|error| error.to_string())?;
            }
            Ok(())
        })();
        if let Err(error) = metered {
            write(&attempt.join("usage-error.txt"), &format!("{error}\n"))?;
        }
        let complete = !matches!(usage, Value::Null);
        let rows = native_artifacts(prepared)?;
        let mut violations = Vec::new();
        if exit != 0 {
            violations.push(format!("codex exec exit {exit}"));
        }
        let mut models: Vec<String> = Vec::new();
        let mut efforts: Vec<String> = Vec::new();
        let mut missing = 0;
        for context in &rollouts.contexts {
            let (m, e) = (get(context, "model"), get(context, "effort"));
            if m.truthy() {
                models.push(m.py_str());
            }
            if e.truthy() {
                efforts.push(e.py_str());
            }
            if !m.truthy() || !e.truthy() {
                missing += 1;
            }
        }
        for list in [&mut models, &mut efforts] {
            list.sort();
            list.dedup();
        }
        let listed = |values: &[String]| {
            if values.is_empty() {
                "none".to_string()
            } else {
                texts(values).py_str()
            }
        };
        if missing > 0 {
            violations.push(format!(
                "{missing} reviewer turn context(s) lack explicit model or effort"
            ));
        }
        if models != [model] {
            violations.push(format!(
                "observed models {} do not match {model}",
                listed(&models)
            ));
        }
        if efforts != [effort] {
            violations.push(format!(
                "observed efforts {} do not match {effort}",
                listed(&efforts)
            ));
        }
        if !complete {
            violations.push("Codex usage was not completely metered".to_string());
        }
        if complete {
            let metered_models = get(get(&usage, "total"), "models");
            if !metered_models.py_eq(&texts(&[model.to_string()])) {
                violations.push(format!(
                    "metered models {} do not match {model}",
                    metered_models.py_str()
                ));
            }
        }
        if skill_tree_hash(&skill_home)?.0 != prepared.tree_hash
            || skill_tree_hash(&skill_work)?.0 != prepared.tree_hash
        {
            violations.push("frozen skill tree changed during the attempt".to_string());
        }
        if tree_identity(clone)? != pytext::strip(&read_text(&attempt.join("tree-before.txt"))?) {
            violations.push("review clone tree identity changed".to_string());
        }
        if rows.is_empty() {
            violations.push("skill produced no native report artifacts".to_string());
        }
        if !rollout_paths.is_empty() {
            let mut command = audit_command(prepared, &sandbox, network);
            command[3] = "codex-skill".to_string();
            let audited = run(&command, None)?;
            write(
                &attempt.join("audit.txt"),
                &format!("{}{}", audited.stdout, audited.stderr),
            )?;
            if audited.code != 0 && audited.code != 1 {
                violations.push(format!("attempt audit failed with exit {}", audited.code));
            }
            match read_json(&attempt.join("audit.json")) {
                Ok(audit) => {
                    let found = get(&audit, "violations");
                    if found.truthy() {
                        for issue in found.as_array().unwrap_or_default() {
                            violations.push(format!("audit: {}", issue.py_str()));
                        }
                    } else if audited.code != 0 {
                        violations.push(format!(
                            "attempt audit exited {} without a violation list",
                            audited.code
                        ));
                    }
                }
                Err(error) => {
                    violations.push(format!(
                        "attempt audit produced no usable audit.json: {error}"
                    ));
                }
            }
        } else if !attempt.join("audit.json").exists() {
            violations.push("attempt audit skipped because no root session was found".to_string());
        }
        if !rows.is_empty() {
            let normalizer = get(&prepared.runner, "normalizer");
            let out = attempt.join("normalized.json");
            let (artifacts, clone_text, out) = (
                prepared.artifact_root.to_string_lossy().into_owned(),
                clone.to_string_lossy().into_owned(),
                out.to_string_lossy().into_owned(),
            );
            match get(normalizer, "kind").as_str() {
                Some("thermo") => {
                    let command = tool_command(
                        prepared,
                        &[
                            "normalize-thermo",
                            "--artifact-root",
                            &artifacts,
                            "--clone",
                            &clone_text,
                            "--out",
                            &out,
                        ],
                    );
                    normalize(prepared, &command, &mut violations)?;
                }
                Some("codex-skill") => {
                    let expected = get(normalizer, "native_file");
                    let reports = reports_named(prepared, &rows, expected);
                    if reports.len() == 1 {
                        let report = reports[0].to_string_lossy();
                        let command = tool_command(
                            prepared,
                            &[
                                "normalize-review",
                                "--arm",
                                "codex-skill",
                                "--native-review",
                                &report,
                                "--clone",
                                &clone_text,
                                "--out",
                                &out,
                            ],
                        );
                        normalize(prepared, &command, &mut violations)?;
                    } else {
                        violations.push(format!(
                            "expected one {} report, found {}",
                            expected.py_repr(),
                            reports.len()
                        ));
                    }
                }
                _ => violations.push(format!(
                    "unknown or missing normalizer kind: {}",
                    get(normalizer, "kind").py_repr()
                )),
            }
        }
        let root_value = root.as_deref().map_or(Value::Null, text);
        let observed = object(vec![
            ("root_session_id", root_value),
            ("contexts", Value::Array(rollouts.contexts)),
            ("models", texts(&models)),
            ("efforts", texts(&efforts)),
            ("cli_version", text(&cli_version)),
            ("skill_name", text(&prepared.skill_name)),
            ("skill_tree_sha256", text(&prepared.tree_hash)),
            ("prompt_sha256", text(&sha_of(prepared.prompt.as_bytes()))),
            ("clean_context", receipt),
            ("usage", usage.clone()),
            (
                "usage_status",
                text(if complete { "complete" } else { "incomplete" }),
            ),
            ("session_lineage", Value::Array(rollouts.lineage)),
            ("spawn_calls", Value::Array(rollouts.spawns)),
            ("codex_config_sha256", text(&config_hash)),
            ("sandbox", sandbox),
            ("native_artifacts", Value::Array(rows.clone())),
            ("violations", texts(&violations)),
        ]);
        let summary = if violations.is_empty() {
            cost_summary(root.as_deref().unwrap_or(""), rows.len(), &usage)?
        } else {
            String::new()
        };
        finish(prepared, &ended, exit, &rows, observed, violations, summary)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn rollout(path: &Path, id: &str, parent: Option<&str>, spawn: Option<&str>) {
            let mut lines = vec![
                format!(
                    r#"{{"type": "session_meta", "payload": {{"id": "{id}", "parent_thread_id": {}}}}}"#,
                    parent.map_or("null".to_string(), |parent| format!("\"{parent}\""))
                ),
                r#"{"type": "turn_context", "payload": {"model": "gpt-6-luna", "effort": "high"}}"#
                    .to_string(),
            ];
            if let Some(arguments) = spawn {
                lines.push(format!(
                    r#"{{"type": "function_call", "name": "spawn_agent", "arguments": {}}}"#,
                    compact(&text(arguments))
                ));
            }
            fs::write(path, lines.join("\n") + "\n").unwrap();
        }

        fn scratch() -> PathBuf {
            let directory = std::env::temp_dir().join(crate::evidence::random_name("rollouts-"));
            fs::create_dir(&directory).unwrap();
            directory
        }

        #[test]
        fn a_lone_root_session_is_accepted() {
            let root = scratch();
            rollout(&root.join("rollout-root.jsonl"), "root", None, None);
            let loaded = load_rollouts(&root, "gpt-6-luna", "high").unwrap();
            assert_eq!(loaded.root, "root");
            assert_eq!(loaded.contexts.len(), 1);
            assert!(matches!(
                get(&loaded.lineage[0], "parent_thread_id"),
                Value::Null
            ));
            assert!(loaded.spawns.is_empty());
            assert_eq!(loaded.paths.len(), 1);
            fs::remove_dir_all(root).unwrap();
        }

        #[test]
        fn a_child_must_descend_from_the_root_through_a_fresh_fork() {
            let root = scratch();
            rollout(
                &root.join("rollout-root.jsonl"),
                "root",
                None,
                Some(r#"{"fork_turns": "none"}"#),
            );
            rollout(
                &root.join("rollout-child.jsonl"),
                "child",
                Some("root"),
                None,
            );
            let loaded = load_rollouts(&root, "gpt-6-luna", "high").unwrap();
            assert_eq!(loaded.lineage.len(), 2);
            assert_eq!(get(&loaded.spawns[0], "fork_turns").as_str(), Some("none"));
            rollout(
                &root.join("rollout-root.jsonl"),
                "root",
                None,
                Some(r#"{"fork_turns": "all"}"#),
            );
            let error = load_rollouts(&root, "gpt-6-luna", "high").err().unwrap();
            assert!(error.contains("fork_turns=none"), "{error}");
            rollout(
                &root.join("rollout-root.jsonl"),
                "root",
                None,
                Some(r#"{"fork_turns": "none", "model": "gpt-other"}"#),
            );
            let error = load_rollouts(&root, "gpt-6-luna", "high").err().unwrap();
            assert!(error.contains("overrode the pinned model"), "{error}");
            fs::remove_dir_all(root).unwrap();
        }

        #[test]
        fn a_session_without_a_turn_context_is_rejected() {
            let root = scratch();
            fs::write(
                root.join("rollout-root.jsonl"),
                "{\"type\": \"session_meta\", \"payload\": {\"id\": \"root\"}}\n",
            )
            .unwrap();
            let error = load_rollouts(&root, "gpt-6-luna", "high").err().unwrap();
            assert!(error.contains("turn context"), "{error}");
            fs::remove_dir_all(root).unwrap();
        }
    }
}

mod claude {
    use super::*;

    fn strip_heredocs(command: &str) -> String {
        let opener = fancy_regex::Regex::new(r#"<<-?\s*(['"]?)([A-Za-z_]\w*)\1"#)
            .unwrap_or_else(|_| unreachable!("the heredoc pattern is valid"));
        let mut kept = Vec::new();
        let mut terminator: Option<String> = None;
        for line in command.split('\n') {
            if let Some(end) = &terminator {
                if pytext::strip(line) == end {
                    terminator = None;
                }
                continue;
            }
            kept.push(line);
            if let Ok(Some(found)) = opener.captures(line) {
                terminator = found.get(2).map(|name| name.as_str().to_string());
            }
        }
        kept.join("\n")
    }

    /// The command segments that execute the skill's cross-model peer.
    pub(super) fn peer_invocations(command: &str) -> Vec<String> {
        let separators = regex::Regex::new(r"\n|;|&&|\|\||\||&|\$\(|`")
            .unwrap_or_else(|_| unreachable!("the separator pattern is valid"));
        let assignment = regex::Regex::new(r"^[A-Za-z_]\w*=")
            .unwrap_or_else(|_| unreachable!("the assignment pattern is valid"));
        let stripped = strip_heredocs(command);
        let mut found = Vec::new();
        for segment in separators.split(&stripped) {
            let mut words = crate::shlex::split_commented(segment).unwrap_or_else(|_| {
                segment
                    .split_whitespace()
                    .map(ToString::to_string)
                    .collect()
            });
            while words.first().is_some_and(|word| {
                assignment.is_match(word)
                    || COMMAND_PREFIXES.contains(&word.as_str())
                    || word.starts_with('-')
                    || word == "("
                    || word == "{"
            }) {
                words.remove(0);
            }
            if words.first().is_some_and(|word| word == "timeout") {
                words = words
                    .into_iter()
                    .skip(1)
                    .filter(|word| !word.starts_with('-'))
                    .skip(1)
                    .collect();
            }
            if let Some(program) = words.first() {
                let name = program.rsplit('/').next().unwrap_or(program);
                if PEER_PROGRAMS.contains(&name) {
                    found.push(pytext::strip(segment).to_string());
                }
            }
        }
        found
    }

    fn jsonl(path: &Path) -> R<Vec<Value>> {
        let content = read_text(path)?;
        let mut records = Vec::new();
        for (number, line) in pytext::splitlines(&content).into_iter().enumerate() {
            if pytext::strip(line).is_empty() {
                continue;
            }
            records.push(pyjson::loads(line, Duplicates::KeepLast).map_err(|error| {
                format!(
                    "{}:{}: invalid transcript JSON: {error}",
                    path.display(),
                    number + 1
                )
            })?);
        }
        Ok(records)
    }

    fn text_of(content: &Value) -> String {
        match content {
            Value::Str(text) => text.clone(),
            Value::Array(blocks) => blocks
                .iter()
                .filter_map(|block| get(block, "text").as_str())
                .collect(),
            _ => String::new(),
        }
    }

    fn children(directory: &Path) -> Vec<PathBuf> {
        let mut found: Vec<PathBuf> = fs::read_dir(directory)
            .map(|entries| entries.flatten().map(|entry| entry.path()).collect())
            .unwrap_or_default();
        found.sort();
        found
    }

    pub(super) struct Transcripts {
        pub roots: Vec<PathBuf>,
        pub subs: Vec<PathBuf>,
        pub contexts: Vec<Value>,
        pub agent_calls: Vec<Value>,
    }

    fn distinct(values: Vec<Value>) -> Vec<Value> {
        let mut kept: Vec<Value> = Vec::new();
        for value in values {
            if !kept.iter().any(|seen| seen.py_eq(&value)) {
                kept.push(value);
            }
        }
        kept.sort_by_key(Value::py_str);
        kept
    }

    /// Check the lineage and collect each transcript's models, efforts and Agent calls.
    pub(super) fn load_transcripts(home: &Path, session: &str, prompt: &str) -> R<Transcripts> {
        let projects = home.join(".claude/projects");
        let mut roots = Vec::new();
        let mut subs = Vec::new();
        for project in children(&projects).into_iter().filter(|path| path.is_dir()) {
            roots.extend(
                children(&project)
                    .into_iter()
                    .filter(|path| path.is_file() && named(path, "", ".jsonl")),
            );
            for owner in children(&project).into_iter().filter(|path| path.is_dir()) {
                subs.extend(
                    children(&owner.join("subagents"))
                        .into_iter()
                        .filter(|path| named(path, "agent-", ".jsonl")),
                );
            }
        }
        roots.sort();
        subs.sort();
        let stem = |path: &Path| {
            path.file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
        };
        if roots.len() != 1 || stem(&roots[0]).as_deref() != Some(session) {
            let names: Vec<String> = roots
                .iter()
                .map(|path| {
                    path.file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned()
                })
                .collect();
            return Err(format!(
                "expected one root transcript {session}.jsonl, found {}",
                texts(&names).py_repr()
            ));
        }
        let strays: Vec<String> = subs
            .iter()
            .filter(|path| {
                path.parent()
                    .and_then(Path::parent)
                    .and_then(Path::file_name)
                    .and_then(OsStr::to_str)
                    != Some(session)
            })
            .map(|path| path.to_string_lossy().into_owned())
            .collect();
        if !strays.is_empty() {
            return Err(format!(
                "subagent transcripts outside the root session: {}",
                texts(&strays).py_repr()
            ));
        }
        let root_records = jsonl(&roots[0])?;
        let first = root_records
            .iter()
            .find(|record| get(record, "type").as_str() == Some("user"));
        if first.is_none_or(|record| {
            pytext::strip(&text_of(get(get(record, "message"), "content"))) != pytext::strip(prompt)
        }) {
            return Err(
                "the root session's first user message is not the runner's prompt".to_string(),
            );
        }
        let mut contexts = Vec::new();
        let mut agent_calls = Vec::new();
        for (index, path) in roots.iter().chain(subs.iter()).enumerate() {
            let records = if index == 0 {
                root_records.clone()
            } else {
                jsonl(path)?
            };
            let (mut models, mut efforts, mut requests) = (Vec::new(), Vec::new(), 0usize);
            for record in &records {
                let message = get(record, "message");
                if get(record, "type").as_str() != Some("assistant")
                    || get(message, "model").as_str() == Some("<synthetic>")
                    || get(record, "isApiErrorMessage").truthy()
                {
                    continue;
                }
                requests += 1;
                models.push(get(message, "model").clone());
                efforts.push(get(record, "effort").clone());
                for block in get(message, "content").as_array().unwrap_or_default() {
                    let name = get(block, "name").as_str();
                    if get(block, "type").as_str() == Some("tool_use")
                        && matches!(name, Some("Agent" | "Task"))
                    {
                        let input = get(block, "input");
                        agent_calls.push(object(vec![
                            (
                                "transcript",
                                text(&path.file_name().unwrap_or_default().to_string_lossy()),
                            ),
                            ("tool_use_id", get(block, "id").clone()),
                            ("subagent_type", get(input, "subagent_type").clone()),
                            ("model", get(input, "model").clone()),
                            ("run_in_background", get(input, "run_in_background").clone()),
                            ("description", get(input, "description").clone()),
                        ]));
                    }
                }
            }
            contexts.push(object(vec![
                (
                    "transcript",
                    path_value(path.strip_prefix(&projects).unwrap_or(path)),
                ),
                ("root", Value::Bool(index == 0)),
                ("assistant_records", int(requests)),
                ("models", Value::Array(distinct(models))),
                ("efforts", Value::Array(distinct(efforts))),
            ]));
        }
        Ok(Transcripts {
            roots,
            subs,
            contexts,
            agent_calls,
        })
    }

    pub(super) fn policy_violations(
        observed: &Transcripts,
        model: &str,
        effort: &str,
    ) -> Vec<String> {
        let mut violations = Vec::new();
        let only = |value: &Value, pinned: &str| {
            value
                .as_array()
                .is_some_and(|items| items.len() == 1 && items[0].as_str() == Some(pinned))
        };
        for context in &observed.contexts {
            let (models, efforts) = (get(context, "models"), get(context, "efforts"));
            if get(context, "assistant_records").truthy()
                && (!only(models, model) || !only(efforts, effort))
            {
                violations.push(format!(
                    "{}: models {} efforts {}, arm requires {model} at {effort}",
                    get(context, "transcript").py_str(),
                    models.py_str(),
                    efforts.py_str()
                ));
            }
        }
        if !observed
            .contexts
            .first()
            .is_some_and(|root| get(root, "assistant_records").truthy())
        {
            violations.push("the root session recorded no model request".to_string());
        }
        for call in &observed.agent_calls {
            let kind = get(call, "subagent_type");
            let kind = if kind.truthy() {
                kind.py_str()
            } else {
                String::new()
            };
            if kind.to_lowercase().contains("fork") {
                violations.push(format!(
                    "Agent call {} forked the parent context",
                    get(call, "tool_use_id").py_str()
                ));
            }
        }
        violations
    }

    fn final_result(stdout: &Path) -> R<Value> {
        let mut result = Value::Object(Object::default());
        if stdout.is_file() {
            for line in pytext::splitlines(&read_text(stdout)?) {
                let Ok(record) = pyjson::loads(line, Duplicates::KeepLast) else {
                    continue;
                };
                if get(&record, "type").as_str() == Some("result") {
                    result = object(
                        [
                            "subtype",
                            "is_error",
                            "num_turns",
                            "total_cost_usd",
                            "duration_ms",
                            "session_id",
                        ]
                        .into_iter()
                        .map(|key| (key, get(&record, key).clone()))
                        .collect(),
                    );
                }
            }
        }
        Ok(result)
    }

    pub(super) fn launch(prepared: &Prepared, rates_path: &Path) -> R<Outcome> {
        let (attempt, clone) = (&prepared.attempt, &prepared.clone);
        let (model, effort) = (prepared.model.as_str(), prepared.effort.as_str());
        let (executable, executable_hash) = pinned_client(&prepared.runner, "claude", "Claude")?;
        let source = source_home();
        let credentials = source.join(".claude/.credentials.json");
        if !credentials.is_file() {
            return Err(format!(
                "Claude credentials are missing at {}",
                credentials.display()
            ));
        }
        let receipt = clean_context::prepare(attempt)?;
        let home = PathBuf::from(field(&receipt, "home")?);
        let skill_work = copy_frozen_skill(prepared)?;
        make_read_only(&skill_work)?;
        let temporary = attempt.join("tmp");
        mkdir(&temporary)?;
        let runtime_path = mise_paths()?;
        let path = |path: &Path| path.to_string_lossy().into_owned();
        let mut env: Vec<(String, String)> = vec![
            ("PATH".into(), search_path(&runtime_path)),
            ("HOME".into(), path(&home)),
            ("TMPDIR".into(), path(&temporary)),
            ("CLAUDE_CODE_TMPDIR".into(), path(&temporary)),
            ("LANG".into(), "C.UTF-8".into()),
            ("LC_ALL".into(), "C.UTF-8".into()),
            ("CLAUDE_CODE_PRINT_BG_WAIT_CEILING_MS".into(), "0".into()),
            ("CLAUDE_CODE_SUBAGENT_MODEL".into(), model.into()),
        ];
        let artifact_env = get(&prepared.runner, "artifact_root_env");
        if artifact_env.truthy() {
            env.push((artifact_env.py_str(), path(&prepared.artifact_root)));
        }
        let session = clean_context::uuid4()?;
        let budget = match std::env::var("ATTEMPT_BUDGET_USD") {
            Ok(budget) if !budget.is_empty() => budget,
            _ => match get(&prepared.runner, "budget_usd") {
                Value::Null => return Err("'budget_usd'".to_string()),
                budget => budget.py_str(),
            },
        };
        let mut command: Vec<String> = [
            path(&executable).as_str(),
            "-p",
            "--safe-mode",
            "--session-id",
            &session,
            "--model",
            model,
            "--effort",
            effort,
            "--max-budget-usd",
            &budget,
            "--allowedTools",
            CLAUDE_TOOLS_ALLOWED,
            "--add-dir",
            &path(&prepared.cache),
            "--add-dir",
            &path(&prepared.work),
            "--add-dir",
            &path(&temporary),
            "--strict-mcp-config",
            "--output-format",
            "stream-json",
            "--verbose",
        ]
        .map(String::from)
        .to_vec();
        let mut sandbox = Value::Null;
        let mut gate = None;
        if let Some(profile) = sandbox_profile(&prepared.runner) {
            let client_dir = executable.parent().unwrap_or(Path::new("/"));
            let (prefix, receipt, started) = start_sandbox(prepared, &profile, client_dir, &[])?;
            command.splice(0..0, prefix);
            sandbox = receipt;
            gate = started;
        }
        let timeout = timeout_seconds(&prepared.runner)?;
        mkdirs(&home.join(".claude"))?;
        let auth = home.join(".claude/.credentials.json");
        clean_context::claude_account(&source.join(".claude.json"), &home)?;
        copy(&credentials, &auth)?;
        chmod(&auth, 0o600)?;
        let mut started = Instant::now();
        let launched = (|| -> R<(String, i32)> {
            let version = run(&[path(&executable), "--version".into()], Some(&env))?;
            if version.code != 0 {
                return Err(format!(
                    "claude --version failed: {}",
                    pytext::strip(&version.stderr)
                ));
            }
            let cli_version =
                pytext::strip(version.stdout.split(" (").next().unwrap_or("")).to_string();
            let frozen = get(&prepared.entry, "expected_cli_version");
            if version_number(frozen.as_str().unwrap_or(""))
                .is_none_or(|expected| expected != cli_version)
            {
                return Err(format!(
                    "Claude CLI {cli_version} differs from frozen {}",
                    frozen.py_str()
                ));
            }
            write(&attempt.join("session-id.txt"), &format!("{session}\n"))?;
            write(&attempt.join("timing.json"), &timing(&utc_now()))?;
            write(
                &attempt.join("dispatch.txt"),
                &format!(
                    "claude-code {cli_version}\nmodel={model} effort={effort} session={session}\nskill={} skill_tree={}\ncontext_id={}\nruntime_path={}\nclaude_executable={}\nclaude_executable_sha256={executable_hash}\nbudget_usd={budget}\ncommand={}\n",
                    prepared.skill_name,
                    prepared.tree_hash,
                    get(&receipt, "context_id").py_str(),
                    if runtime_path.is_empty() {
                        "<inherited>"
                    } else {
                        &runtime_path
                    },
                    executable.display(),
                    compact(&texts(&command))
                ),
            )?;
            started = Instant::now();
            let exit = review(&command, attempt, &prepared.input, clone, &env, timeout)?;
            Ok((cli_version, exit))
        })();
        let _ = fs::remove_file(&auth);
        let stopped = gate.map(network_gate::Gate::stop).transpose();
        let (cli_version, exit) = launched?;
        stopped?;
        let ended = utc_now();
        append(
            &attempt.join("dispatch.txt"),
            &format!("elapsed_seconds={}\nexit={exit}\n", elapsed(started)),
        )?;
        write(
            &attempt.join("tree-after.txt"),
            &format!("{}\n", tree_identity(clone)?),
        )?;
        let mut violations = if exit == 0 {
            Vec::new()
        } else {
            vec![format!("claude exit {exit}")]
        };
        let observed = match load_transcripts(&home, &session, &prepared.prompt) {
            Ok(observed) => {
                violations.extend(policy_violations(&observed, model, effort));
                observed
            }
            Err(error) => {
                violations.push(error);
                Transcripts {
                    roots: Vec::new(),
                    subs: Vec::new(),
                    contexts: Vec::new(),
                    agent_calls: Vec::new(),
                }
            }
        };
        let mut usage = Value::Null;
        let metered = (|| -> R<()> {
            let rate = rates_for(&resolve(rates_path), model)?;
            let paths: Vec<String> = observed
                .roots
                .iter()
                .chain(observed.subs.iter())
                .map(|path| path.to_string_lossy().into_owned())
                .collect();
            if paths.is_empty() {
                return Ok(());
            }
            let prices = format!(
                "{},{}",
                get(&rate, "input").py_str(),
                get(&rate, "output").py_str()
            );
            let mut command = tool_command(prepared, &["usage", "claude"]);
            command.extend(paths);
            for argument in [
                "--prices".to_string(),
                prices,
                "--cache-read-mult".to_string(),
                ratio(&rate, "cache_read")?,
                "--cache-write-mult".to_string(),
                ratio(&rate, "cache_write_5m")?,
                "--cache-write-1h-mult".to_string(),
                ratio(&rate, "cache_write_1h")?,
                "--json".to_string(),
            ] {
                command.push(argument);
            }
            let done = run(&command, None)?;
            write(
                &attempt.join("usage.txt"),
                &format!("{}{}", done.stdout, done.stderr),
            )?;
            if done.code == 0 {
                usage = pyjson::loads(&done.stdout, Duplicates::KeepLast)
                    .map_err(|error| error.to_string())?;
            }
            Ok(())
        })();
        if let Err(error) = metered {
            write(&attempt.join("usage-error.txt"), &format!("{error}\n"))?;
        }
        let complete = !matches!(usage, Value::Null);
        if !complete {
            violations.push("Claude usage was not completely metered".to_string());
        }
        let rows = native_artifacts(prepared)?;
        if rows.is_empty() {
            violations.push("skill produced no native report artifacts".to_string());
        }
        if home.join(".codex").exists() {
            violations
                .push("a Codex client state directory appeared in the fresh HOME".to_string());
        }
        if skill_tree_hash(&skill_work)?.0 != prepared.tree_hash {
            violations.push("frozen skill tree changed during the attempt".to_string());
        }
        if tree_identity(clone)? != pytext::strip(&read_text(&attempt.join("tree-before.txt"))?) {
            violations.push("review clone tree identity changed".to_string());
        }
        if !observed.roots.is_empty() {
            let network = get(&prepared.runner, "network_allowed").truthy();
            let mut command = audit_command(prepared, &sandbox, network);
            command[3] = "claude-skill".to_string();
            let audited = run(&command, None)?;
            write(
                &attempt.join("audit.txt"),
                &format!("{}{}", audited.stdout, audited.stderr),
            )?;
            match read_json(&attempt.join("audit.json")) {
                Ok(audit) => {
                    let found = get(&audit, "violations");
                    for issue in found.as_array().unwrap_or_default() {
                        violations.push(format!("audit: {}", issue.py_str()));
                    }
                    if (audited.code != 0 && audited.code != 1)
                        || (audited.code != 0 && !found.truthy())
                    {
                        violations.push(format!("attempt audit exited {}", audited.code));
                    }
                    for command in get(&audit, "commands").as_array().unwrap_or_default() {
                        for segment in peer_invocations(command.as_str().unwrap_or("")) {
                            let shown: String = segment.chars().take(200).collect();
                            violations.push(format!("cross-model peer command: {shown}"));
                        }
                    }
                }
                Err(error) => {
                    violations.push(format!(
                        "attempt audit produced no usable audit.json: {error}"
                    ));
                }
            }
        }
        if !rows.is_empty() {
            let normalizer = get(&prepared.runner, "normalizer");
            let expected = get(normalizer, "native_file");
            let reports = reports_named(prepared, &rows, expected);
            let (artifacts, clone_text, out) = (
                path(&prepared.artifact_root),
                path(clone),
                path(&attempt.join("normalized.json")),
            );
            let kind = get(normalizer, "kind");
            if kind.as_str() == Some("thermo") {
                let command = tool_command(
                    prepared,
                    &[
                        "normalize-thermo",
                        "--artifact-root",
                        &artifacts,
                        "--clone",
                        &clone_text,
                        "--out",
                        &out,
                        "--arm",
                        "claude-skill",
                    ],
                );
                normalize(prepared, &command, &mut violations)?;
            } else if kind.as_str() != Some("native-review") {
                violations.push(format!(
                    "unknown or missing normalizer kind: {}",
                    kind.py_repr()
                ));
            } else if reports.len() != 1 {
                violations.push(format!(
                    "expected one {} report, found {}",
                    expected.py_repr(),
                    reports.len()
                ));
            } else {
                let report = path(&reports[0]);
                let command = tool_command(
                    prepared,
                    &[
                        "normalize-review",
                        "--arm",
                        "claude-skill",
                        "--native-review",
                        &report,
                        "--clone",
                        &clone_text,
                        "--out",
                        &out,
                    ],
                );
                normalize(prepared, &command, &mut violations)?;
            }
        }
        let mut models = Vec::new();
        let mut efforts = Vec::new();
        for context in &observed.contexts {
            models.extend(
                get(context, "models")
                    .as_array()
                    .unwrap_or_default()
                    .iter()
                    .cloned(),
            );
            efforts.extend(
                get(context, "efforts")
                    .as_array()
                    .unwrap_or_default()
                    .iter()
                    .cloned(),
            );
        }
        let record = object(vec![
            ("root_session_id", text(&session)),
            ("contexts", Value::Array(observed.contexts.clone())),
            ("models", Value::Array(distinct(models))),
            ("efforts", Value::Array(distinct(efforts))),
            ("cli_version", text(&cli_version)),
            ("skill_name", text(&prepared.skill_name)),
            ("skill_tree_sha256", text(&prepared.tree_hash)),
            ("prompt_sha256", text(&sha_of(prepared.prompt.as_bytes()))),
            ("clean_context", receipt),
            ("usage", usage.clone()),
            (
                "usage_status",
                text(if complete { "complete" } else { "incomplete" }),
            ),
            ("agent_calls", Value::Array(observed.agent_calls.clone())),
            ("result", final_result(&attempt.join("stdout.jsonl"))?),
            ("sandbox", sandbox),
            ("native_artifacts", Value::Array(rows.clone())),
            ("violations", texts(&violations)),
        ]);
        let summary = if violations.is_empty() {
            cost_summary(&session, rows.len(), &usage)?
        } else {
            String::new()
        };
        finish(prepared, &ended, exit, &rows, record, violations, summary)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        const SESSION: &str = "11111111-2222-4333-8444-555555555555";
        const PROMPT: &str = "Review the frozen task.";

        fn assistant(model: &str, effort: &str, content: &str) -> String {
            format!(
                r#"{{"type": "assistant", "effort": {effort}, "requestId": "req", "message": {{"model": "{model}", "content": {content}}}}}"#
            )
        }

        fn ok(model: &str, effort: &str) -> String {
            assistant(
                model,
                &format!("\"{effort}\""),
                r#"[{"type": "text", "text": "ok"}]"#,
            )
        }

        struct Home(PathBuf);

        impl Home {
            fn new() -> Self {
                let directory =
                    std::env::temp_dir().join(crate::evidence::random_name("transcripts-"));
                fs::create_dir(&directory).unwrap();
                Self(directory)
            }

            fn project(&self) -> PathBuf {
                self.0.join(".claude/projects/-clone")
            }

            fn write(path: &Path, records: &[String]) {
                fs::create_dir_all(path.parent().unwrap()).unwrap();
                fs::write(path, records.join("\n") + "\n").unwrap();
            }

            fn root(&self, prompt: &str, records: &[String]) {
                let mut all = vec![format!(
                    r#"{{"type": "user", "message": {{"content": {}}}}}"#,
                    compact(&text(prompt))
                )];
                all.extend_from_slice(records);
                Self::write(&self.project().join(format!("{SESSION}.jsonl")), &all);
            }

            fn sub(&self, name: &str, records: &[String]) {
                let mut all =
                    vec![r#"{"type": "user", "message": {"content": "brief"}}"#.to_string()];
                all.extend_from_slice(records);
                Self::write(
                    &self
                        .project()
                        .join(SESSION)
                        .join("subagents")
                        .join(format!("agent-{name}.jsonl")),
                    &all,
                );
            }

            fn violations(&self) -> Vec<String> {
                policy_violations(
                    &load_transcripts(&self.0, SESSION, PROMPT).unwrap(),
                    "claude-sonnet-5-5",
                    "high",
                )
            }
        }

        impl Drop for Home {
            fn drop(&mut self) {
                let _ = fs::remove_dir_all(&self.0);
            }
        }

        #[test]
        fn a_pinned_root_and_a_fresh_subagent_pass() {
            let home = Home::new();
            let call = r#"[{"type": "tool_use", "id": "t1", "name": "Agent", "input": {"description": "correctness", "prompt": "x"}}]"#;
            home.root(PROMPT, &[assistant("claude-sonnet-5-5", "\"high\"", call)]);
            home.sub("a1", &[ok("claude-sonnet-5-5", "high")]);
            let observed = load_transcripts(&home.0, SESSION, PROMPT).unwrap();
            assert_eq!(observed.subs.len(), 1);
            assert!(matches!(
                get(&observed.agent_calls[0], "subagent_type"),
                Value::Null
            ));
            assert_eq!(home.violations(), [] as [String; 0]);
        }

        #[test]
        fn a_subagent_on_another_model_or_effort_is_a_violation() {
            let home = Home::new();
            home.root(PROMPT, &[ok("claude-sonnet-5-5", "high")]);
            home.sub("a1", &[ok("claude-haiku-4-5-20251001", "high")]);
            home.sub("a2", &[ok("claude-sonnet-5-5", "medium")]);
            assert_eq!(home.violations().len(), 2);
        }

        #[test]
        fn synthetic_notices_are_not_model_requests() {
            let home = Home::new();
            home.root(
                PROMPT,
                &[
                    ok("claude-sonnet-5-5", "high"),
                    assistant("<synthetic>", "null", "[]"),
                ],
            );
            assert_eq!(home.violations(), [] as [String; 0]);
        }

        #[test]
        fn a_forked_agent_is_a_violation() {
            let home = Home::new();
            let call = r#"[{"type": "tool_use", "id": "t1", "name": "Agent", "input": {"subagent_type": "fork", "prompt": "x"}}]"#;
            home.root(PROMPT, &[assistant("claude-sonnet-5-5", "\"high\"", call)]);
            assert!(home.violations()[0].contains("forked the parent context"));
        }

        #[test]
        fn inherited_history_and_a_second_root_are_rejected() {
            let home = Home::new();
            home.root(
                &format!("earlier conversation\n\n{PROMPT}"),
                &[ok("claude-sonnet-5-5", "high")],
            );
            let error = load_transcripts(&home.0, SESSION, PROMPT).err().unwrap();
            assert!(error.contains("first user message"), "{error}");
            home.root(PROMPT, &[ok("claude-sonnet-5-5", "high")]);
            Home::write(
                &home
                    .project()
                    .join("99999999-2222-4333-8444-555555555555.jsonl"),
                &[ok("claude-sonnet-5-5", "high")],
            );
            let error = load_transcripts(&home.0, SESSION, PROMPT).err().unwrap();
            assert!(error.contains("one root transcript"), "{error}");
        }

        #[test]
        fn executed_peer_programs_are_detected() {
            for command in [
                r#"bash "$SKILL_DIR/scripts/cross-model-adversarial-review.sh" start x"#,
                "cd /x && codex exec -",
                "CROSS_MODEL_HOST_HARNESS=claude timeout 600 claude -p hi",
                "python3 scripts/peer-job-runner.py start",
                "out=$(codex exec -)",
            ] {
                assert!(!peer_invocations(command).is_empty(), "{command}");
            }
        }

        #[test]
        fn mentions_of_the_peer_are_not_invocations() {
            let report = "cd /run; python3 - <<'E'\nnotes = ['cross-model-adversarial-review.sh not run']\nE\ncat > brief.json <<'EOF'\n{\"constraints\": [\"do not run codex\"]}\nEOF";
            assert_eq!(peer_invocations(report), [] as [String; 0]);
            assert_eq!(
                peer_invocations("rg -n cross-model-adversarial-review.sh references"),
                [] as [String; 0]
            );
            assert_eq!(
                peer_invocations("echo 'claude code harness' > note.txt"),
                [] as [String; 0]
            );
            assert_eq!(peer_invocations("ls # codex exec -"), [] as [String; 0]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MOUNTS: &str = "29 1 8:48 / / rw - ext4 /dev/sdd rw\n\
        40 29 0:38 / /mnt/wslg ro - tmpfs none rw\n\
        41 40 8:48 / /mnt/wslg/distro ro - ext4 /dev/sdd rw\n\
        42 29 8:48 /home/u/other /mnt/elsewhere rw - ext4 /dev/sdd rw\n\
        43 29 8:16 / /mnt/c rw - 9p drvfs rw\n";

    fn hidden(paths: &[&str]) -> Vec<String> {
        paths.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn the_attempt_is_bound_where_a_hidden_mount_of_its_filesystem_shows_it() {
        let attempt = Path::new("/home/u/runs/att-001");
        let aliases = hidden_aliases(attempt, &hidden(&["/home", "/mnt"]), MOUNTS);
        assert_eq!(aliases, ["/mnt/wslg/distro/home/u/runs/att-001"]);
        if review_isolation::which("bwrap").is_none() {
            return;
        }
        let (prefix, receipt) = sandbox_command(
            "bwrap-v1",
            attempt,
            &attempt.join("clone"),
            &[],
            &aliases,
            None,
        )
        .unwrap();
        let bind = prefix.iter().position(|part| *part == aliases[0]).unwrap();
        assert_eq!(
            prefix[bind - 2..=bind],
            ["--bind", "/home/u/runs/att-001", aliases[0].as_str()]
        );
        if Path::new("/mnt").is_dir() {
            assert!(prefix.iter().position(|part| part == "/mnt").unwrap() < bind);
        }
        assert!(get(&receipt, "readwrite").py_eq(&texts(&[
            "/home/u/runs/att-001".to_string(),
            aliases[0].clone()
        ])));
    }

    #[test]
    fn an_attempt_on_a_separate_home_mount_is_found_by_its_path_in_the_filesystem() {
        let mounts = "29 1 8:1 / / rw - ext4 /dev/sda rw\n30 29 8:2 / /home rw - ext4 /dev/sdb rw\n\
            31 29 8:2 /u /srv/u rw - ext4 /dev/sdb rw\n";
        assert_eq!(
            hidden_aliases(
                Path::new("/home/u/runs/att-001"),
                &hidden(&["/home", "/srv"]),
                mounts
            ),
            ["/srv/u/runs/att-001"]
        );
        assert_eq!(
            hidden_aliases(
                Path::new("/home/u/runs/att-001"),
                &hidden(&["/home", "/mnt"]),
                "29 1 8:48 / / rw - ext4 /dev/sdd rw\n"
            ),
            [] as [String; 0]
        );
        assert_eq!(unescape_mount(r"/a\040b\\c"), r"/a b\c");
    }

    #[test]
    fn bwrap_hides_homes_before_binding_the_attempt_and_a_private_tmp() {
        if review_isolation::which("bwrap").is_none() {
            return;
        }
        let attempt = std::env::temp_dir().join(crate::evidence::random_name("sandbox-"));
        fs::create_dir(&attempt).unwrap();
        let (prefix, receipt) = sandbox_command(
            "bwrap-v1",
            &attempt,
            &attempt.join("clone"),
            &[attempt.join("missing")],
            &[],
            None,
        )
        .unwrap();
        fs::remove_dir(&attempt).unwrap();
        let attempt_text = attempt.to_string_lossy().into_owned();
        assert_eq!(prefix.last().map(String::as_str), Some("--"));
        assert!(
            get(&receipt, "hidden")
                .as_array()
                .unwrap()
                .iter()
                .any(|path| path.as_str() == Some("/home"))
        );
        assert!(!get(&receipt, "readonly").py_str().contains("missing"));
        let position = |needle: &str| prefix.iter().position(|part| part == needle).unwrap();
        assert!(position("/home") < position(&attempt_text));
        let tmp = position("/tmp");
        assert_eq!(
            prefix[tmp - 2..=tmp],
            ["--bind", &format!("{attempt_text}/tmp"), "/tmp"]
        );
        assert!(!prefix.iter().any(|part| part == "--unshare-net"));
    }

    #[test]
    fn bwrap_v2_cuts_the_network_and_runs_the_reviewer_behind_the_gate() {
        if review_isolation::which("bwrap").is_none() {
            return;
        }
        let attempt = std::env::temp_dir().join(crate::evidence::random_name("sandbox-"));
        fs::create_dir(&attempt).unwrap();
        let clone = attempt.join("clone");
        let tools = std::env::current_exe().unwrap();
        let tools_text = tools.to_string_lossy().into_owned();
        assert!(sandbox_command("bwrap-v2", &attempt, &clone, &[], &[], None).is_err());
        let gate = network_gate::Gate::start(&attempt, &tools).unwrap();
        let alias = "/mnt/wslg/distro/attempt".to_string();
        let (prefix, receipt) = sandbox_command(
            "bwrap-v2",
            &attempt,
            &clone,
            &[],
            std::slice::from_ref(&alias),
            Some(&gate),
        )
        .unwrap();
        let gate_dir = gate.dir().to_string_lossy().into_owned();
        gate.stop().unwrap();
        fs::remove_dir_all(&attempt).unwrap();
        let attempt_text = attempt.to_string_lossy().into_owned();
        let network = format!("{attempt_text}/network");
        let at = |window: &[&str]| {
            prefix
                .windows(window.len())
                .position(|found| found == window)
                .unwrap_or_else(|| panic!("{window:?} not in {prefix:?}"))
        };
        at(&["--unshare-net"]);
        at(&["--seccomp"]);
        assert!(at(&["--tmpfs", "/run"]) < at(&["--ro-bind", &tools_text, &tools_text]));
        assert!(!prefix.iter().any(|part| part == "/run/user"));
        assert!(
            at(&["--bind", &attempt_text, &alias])
                < at(&["--ro-bind", &network, &format!("{alias}/network")])
        );
        assert!(
            at(&["--bind", &attempt_text, &attempt_text]) < at(&["--ro-bind", &network, &network])
        );
        let shown_gate = at(&["--ro-bind", &gate_dir, network_gate::INNER_DIR]);
        assert!(shown_gate > at(&["--tmpfs", "/run"]));
        at(&["--unsetenv", "HTTP_PROXY"]);
        assert!(
            !prefix
                .windows(2)
                .any(|pair| pair == ["--setenv", "HTTP_PROXY"])
        );
        let command = at(&["--", &tools_text, "network-gate", "bridge"]);
        assert_eq!(prefix.last().map(String::as_str), Some("--"));
        assert!(command > at(&["--chdir"]));
        assert_eq!(get(&receipt, "network").as_str(), Some("gated"));
        assert!(get(&receipt, "namespaces").py_str().contains("network"));
        assert_eq!(
            get(get(&receipt, "gate"), "policy").as_str(),
            Some(network_gate::POLICY)
        );
    }

    #[test]
    fn the_audit_of_a_gated_attempt_applies_the_gated_rules() {
        let prepared = Prepared {
            run_dir: PathBuf::from("/r"),
            attempt: PathBuf::from("/a"),
            clone: PathBuf::from("/c"),
            entry: Value::Null,
            model: String::new(),
            effort: String::new(),
            skill_source: PathBuf::new(),
            tree_hash: String::new(),
            runner: Value::Null,
            cache: PathBuf::new(),
            work: PathBuf::new(),
            artifact_root: PathBuf::new(),
            skill_name: String::new(),
            input: PathBuf::new(),
            prompt: String::new(),
            tools: PathBuf::from("/bench-tools"),
            target_dir: PathBuf::from("/bench/targets/t-1"),
            packet: PathBuf::from("/bench/targets/t-1/packet.v2.md"),
            policy: 1,
        };
        let receipt = |network: &str| object(vec![("network", text(network))]);
        let gated = audit_command(&prepared, &receipt("gated"), false);
        assert!(gated.ends_with(&[
            "--network-gate".to_string(),
            "/a/network/gate.json".to_string(),
            "--target".to_string(),
            "/bench/targets/t-1".to_string(),
            "--packet".to_string(),
            "/bench/targets/t-1/packet.v2.md".to_string(),
        ]));
        for sandbox in [receipt("shared"), Value::Null] {
            let command = audit_command(&prepared, &sandbox, true);
            assert!(!command.iter().any(|part| part == "--network-gate"));
            assert_eq!(command.last().map(String::as_str), Some("--allow-network"));
        }
    }

    #[test]
    fn an_unknown_sandbox_profile_is_refused() {
        let error = sandbox_command(
            "none",
            Path::new("/x"),
            Path::new("/x/clone"),
            &[],
            &[],
            None,
        )
        .err()
        .unwrap();
        assert!(error.contains("unknown sandbox profile"), "{error}");
    }

    #[test]
    fn a_non_finite_timeout_is_refused_and_a_huge_one_never_expires() {
        let runner = |timeout: &str| {
            pyjson::loads(
                &format!("{{\"timeout_seconds\": {timeout}}}"),
                Duplicates::KeepLast,
            )
            .unwrap()
        };
        for refused in ["NaN", "Infinity", "-Infinity"] {
            assert_eq!(
                timeout_seconds(&runner(refused)).unwrap_err(),
                "timeout_seconds must be an integer"
            );
        }
        assert_eq!(timeout_seconds(&runner("90.9")).unwrap(), 90);
        let huge = timeout_seconds(&runner("1e300")).unwrap();
        let root = std::env::temp_dir().join(crate::evidence::random_name("timeout-"));
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("input"), "").unwrap();
        let command = ["/usr/bin/true".to_string()];
        assert_eq!(
            review(&command, &root, &root.join("input"), &root, &[], huge).unwrap(),
            0
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn the_skill_tree_hash_orders_paths_by_component_and_refuses_links() {
        let root = std::env::temp_dir().join(crate::evidence::random_name("skill-"));
        fs::create_dir_all(root.join("a")).unwrap();
        fs::write(root.join("a/b"), "1").unwrap();
        fs::write(root.join("a-c"), "2").unwrap();
        let (hash, rows) = skill_tree_hash(&root).unwrap();
        let paths: Vec<&str> = rows
            .iter()
            .filter_map(|row| get(row, "path").as_str())
            .collect();
        assert_eq!(paths, ["a/b", "a-c"]);
        assert_eq!(hash, sha_of(b"a/b\x001a-c\x002"));
        std::os::unix::fs::symlink("a/b", root.join("link")).unwrap();
        assert!(skill_tree_hash(&root).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    fn git_in(directory: &Path, arguments: &[&str]) -> String {
        let output = Command::new("git")
            .arg("-C")
            .arg(directory)
            .args([
                "-c",
                "user.name=Fixture",
                "-c",
                "user.email=fixture@example.test",
            ])
            .args(arguments)
            .output()
            .unwrap();
        assert!(output.status.success(), "git {arguments:?}");
        decoded(&output.stdout).trim().to_string()
    }

    fn put(path: &Path, content: impl AsRef<[u8]>) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    fn digest(path: &Path) -> String {
        sha_of(&fs::read(path).unwrap())
    }

    const DIFF: &str = "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd";

    /// A frozen skill run whose one target has a re-cut packet, a source repository for its clones, and the
    /// running test executable pinned as the run's bench-tools.
    struct Recut {
        root: PathBuf,
        run: PathBuf,
        target: PathBuf,
        source: PathBuf,
        merge_base: String,
        head: String,
    }

    impl Drop for Recut {
        fn drop(&mut self) {
            let parent = self.root.parent().unwrap().to_path_buf();
            let _ = fs::remove_dir_all(parent);
        }
    }

    impl Recut {
        fn new() -> Self {
            let parent =
                resolve(&std::env::temp_dir().join(crate::evidence::random_name("recut-")));
            let (root, source) = (parent.join("root"), parent.join("source"));
            fs::create_dir_all(&source).unwrap();
            git_in(&source, &["init", "-q", "-b", "trunk"]);
            let commit = |content: &str| {
                fs::write(source.join("f.txt"), content).unwrap();
                git_in(&source, &["add", "-A"]);
                git_in(&source, &["commit", "-qm", content]);
                git_in(&source, &["rev-parse", "HEAD"])
            };
            let merge_base = commit("base");
            let head = commit("head");
            let target = root.join("bench/targets/t1");
            put(&target.join("packet.md"), "# Original packet\n\n");
            put(&target.join("packet.v2.md"), "# Re-cut packet\r\n\n\n");
            put(
                &target.join("target.json"),
                format!(
                    r#"{{"id": "t1", "packet_sha256": "{}", "diff_manifest_sha256": "{DIFF}", "merge_base": "{merge_base}", "head": "{head}", "provisioning": {{"allowance": "Run focused tests.", "unavailable": "network"}}}}"#,
                    sha_of(b"# Original packet\n\n")
                ),
            );
            put(
                &root.join("docs/packet-replacements.v1.json"),
                format!(
                    r#"{{"schema_version": 1, "targets": [{{"target": "t1", "target_sha256": "{}", "original": {{"path": "bench/targets/t1/packet.md", "sha256": "{}"}}, "replacement": {{"path": "bench/targets/t1/packet.v2.md", "sha256": "{}"}}}}]}}"#,
                    digest(&target.join("target.json")),
                    digest(&target.join("packet.md")),
                    digest(&target.join("packet.v2.md"))
                ),
            );
            let run = root.join("bench/runs/2026-01-01-test");
            let executable = review_isolation::own_executable().unwrap();
            put(
                &run.join("runtimes.frozen.json"),
                format!(
                    r#"{{"bench-tools": {{"sha256": "{}"}}}}"#,
                    digest(&executable)
                ),
            );
            let inputs = run.join("inputs");
            put(&inputs.join("skill/SKILL.md"), "Review the code.\n");
            put(&inputs.join("shared-policy.md"), "Policy\n");
            put(
                &inputs.join("runner.json"),
                r#"{"artifact_root": "report"}"#,
            );
            git_in(&root, &["init", "-q"]);
            Self {
                root,
                run,
                target,
                source,
                merge_base,
                head,
            }
        }

        /// Freeze the run for a `client` arm with `invocation`, with or without the packet replacement.
        fn freeze(&self, client: Client, invocation: &str, replaced: bool) {
            self.freeze_under(client, invocation, replaced, "", "");
        }

        /// As `freeze`, with `policy` (`"version": N, ` or empty) leading the execution policy and `pins` (each
        /// `, "<pin>": {...}`) added to the manifest.
        fn freeze_under(
            &self,
            client: Client,
            invocation: &str,
            replaced: bool,
            policy: &str,
            pins: &str,
        ) {
            let arm = self.root.join("bench/arms/test.json");
            put(
                &arm,
                format!(
                    r#"{{"id": "test", "kind": "{}", "model": "test-model", "effort": "high"}}"#,
                    client.kind()
                ),
            );
            let inputs = self.run.join("inputs");
            put(&inputs.join("invocation.md"), invocation);
            let (tree, files) = skill_tree_hash(&inputs.join("skill")).unwrap();
            put(
                &inputs.join("skill-pin.json"),
                compact(&object(vec![
                    ("tree_hash", text(&tree)),
                    ("files", Value::Array(files)),
                    ("skill_id", text("fixture")),
                ])),
            );
            let pinned: Vec<Value> = ["invocation.md", "runner.json", "shared-policy.md"]
                .iter()
                .map(|name| {
                    let path = inputs.join(name);
                    object(vec![
                        ("path", text(name)),
                        ("bytes", int(fs::metadata(&path).unwrap().len())),
                        ("sha256", text(&digest(&path))),
                    ])
                })
                .collect();
            put(
                &inputs.join("input-pin.json"),
                compact(&object(vec![("files", Value::Array(pinned))])),
            );
            let packet = if replaced {
                "packet.v2.md"
            } else {
                "packet.md"
            };
            let replacement = if replaced {
                format!(
                    r#", "packet_replacements": {{"path": "docs/packet-replacements.v1.json", "sha256": "{}"}}"#,
                    digest(&self.root.join("docs/packet-replacements.v1.json"))
                )
            } else {
                String::new()
            };
            let manifest = |frozen: &str| {
                format!(
                    r#"{{"run_id": "2026-01-01-test", "arms": [{{"id": "test", "arm_file_sha256": "{}", "resolved_skill_tree": "{tree}"}}], "cohort": [{{"target": "t1", "packet_sha256": "{}", "diff_manifest_sha256": "{DIFF}"}}]{replacement}{pins}, "execution_policy": {{{policy}"allowance": "Five minutes.", "branch_layout": "`main` is the merge-base."}}{frozen}}}"#,
                    digest(&arm),
                    digest(&self.target.join(packet))
                )
            };
            put(&self.run.join("manifest.json"), manifest(""));
            git_in(&self.root, &["add", "."]);
            git_in(&self.root, &["commit", "-qm", "Freeze fixture"]);
            let commit = git_in(&self.root, &["rev-parse", "HEAD"]);
            put(
                &self.run.join("manifest.json"),
                manifest(&format!(
                    r#", "frozen_at": "2026-01-01T00:00:00Z", "freeze_commit": "{commit}""#
                )),
            );
        }

        /// A fresh attempt, at the same path each time, and clone launched with `packet`; the prompt the runner wrote, or its refusal.
        fn prompt(&self, client: Client, packet: &str) -> R<String> {
            let attempt = self.root.parent().unwrap().join("attempt");
            let _ = fs::remove_dir_all(&attempt);
            let clone = attempt.join("clone");
            fs::create_dir_all(&attempt).unwrap();
            git_in(
                &attempt,
                &["clone", "-q", &self.source.to_string_lossy(), "clone"],
            );
            git_in(&clone, &["branch", "main", &self.merge_base]);
            git_in(&clone, &["checkout", "-q", "-b", "review-head", &self.head]);
            let arguments = Arguments {
                run: self.run.clone(),
                attempt_dir: attempt,
                clone,
                packet: self.target.join(packet),
                arm: self.root.join("bench/arms/test.json"),
                target: "t1".to_string(),
                rates: None,
            };
            prepare_attempt(&arguments, client).map(|prepared| prepared.prompt)
        }
    }

    #[test]
    fn a_recut_prompt_differs_from_the_original_one_only_in_its_packet_text() {
        let fixture = Recut::new();
        for client in [Client::Codex, Client::Claude] {
            for invocation in [
                "Review {PACKET}\n",
                "The review task section above holds the packet.\n",
            ] {
                fixture.freeze(client, invocation, true);
                let recut = fixture.prompt(client, "packet.v2.md").unwrap();
                fixture.freeze(client, invocation, false);
                let original = fixture.prompt(client, "packet.md").unwrap();
                assert_eq!(
                    recut,
                    original.replace("# Original packet", "# Re-cut packet")
                );
                assert_eq!(original.matches("# Original packet").count(), 1);
                if !invocation.contains("{PACKET}") {
                    assert!(
                        recut.starts_with("Policy\n\n## Review task\n\n# Re-cut packet\n\n"),
                        "{recut}"
                    );
                }
            }
        }
    }

    #[test]
    fn both_skill_runners_refuse_the_original_packet_for_a_recut_run() {
        let fixture = Recut::new();
        for client in [Client::Codex, Client::Claude] {
            fixture.freeze(client, "Review {PACKET}\n", true);
            let error = fixture.prompt(client, "packet.md").unwrap_err();
            assert!(error.contains("PR packet differs"), "{error}");
        }
    }

    #[test]
    fn a_runner_that_turns_the_cross_model_peer_off_configures_ce_and_keeps_the_clone_clean() {
        let fixture = Recut::new();
        let runner = fixture.run.join("inputs/runner.json");
        let clone = fixture.root.parent().unwrap().join("attempt/clone");
        let config = clone.join(CE_LOCAL_CONFIG);
        for client in [Client::Codex, Client::Claude] {
            fixture.freeze(client, "Review {PACKET}\n", false);
            fixture.prompt(client, "packet.md").unwrap();
            assert!(!config.exists());

            put(
                &runner,
                r#"{"artifact_root": "report", "cross_model_review_mode": "off"}"#,
            );
            fixture.freeze(client, "Review {PACKET}\n", false);
            fixture.prompt(client, "packet.md").unwrap();
            assert_eq!(
                fs::read_to_string(&config).unwrap(),
                "cross_model_review_mode: off\n"
            );
            assert_eq!(
                git_in(&clone, &["status", "--porcelain", "--untracked-files=all"]),
                ""
            );
            let clean = sha_of(format!("{}\n\n", fixture.head).as_bytes());
            let before = fs::read_to_string(clone.with_file_name("tree-before.txt")).unwrap();
            assert_eq!(before, format!("{clean}\n"));

            put(
                &runner,
                r#"{"artifact_root": "report", "cross_model_review_mode": "auto"}"#,
            );
            fixture.freeze(client, "Review {PACKET}\n", false);
            let error = fixture.prompt(client, "packet.md").unwrap_err();
            assert!(
                error.contains("cross_model_review_mode must be 'off'"),
                "{error}"
            );
            put(&runner, r#"{"artifact_root": "report"}"#);
        }
    }

    #[test]
    fn an_open_network_run_needs_its_pins_a_matching_runner_and_bwrap_v2() {
        let fixture = Recut::new();
        let texts = fixture.root.join("docs/task-texts.json");
        put(
            &texts,
            format!(
                r#"{{"targets": [{{"target": "t1", "target_sha256": "{}", "allowance": "Run anything.", "unavailable": "None."}}]}}"#,
                digest(&fixture.target.join("target.json"))
            ),
        );
        let pin = |path: &str, sha: &str| format!(r#"{{"path": "{path}", "sha256": "{sha}"}}"#);
        let forge = format!(
            r#", "forge_captures": {}"#,
            pin("docs/none.json", &"0".repeat(64))
        );
        let pins = format!(
            r#"{forge}, "task_text_replacements": {}"#,
            pin("docs/task-texts.json", &digest(&texts))
        );
        let runner = fixture.run.join("inputs/runner.json");
        let invocation = "Use `{MODEL}` at `{EFFORT}`.\n";
        let open = r#""version": 2, "#;
        for client in [Client::Codex, Client::Claude] {
            put(
                &runner,
                r#"{"artifact_root": "report", "sandbox": "bwrap-v2"}"#,
            );
            fixture.freeze_under(client, invocation, true, open, &forge);
            let error = fixture.prompt(client, "packet.v2.md").unwrap_err();
            assert!(
                error.ends_with("needs the manifest pins task_text_replacements"),
                "{error}"
            );

            fixture.freeze_under(client, invocation, true, open, &pins);
            let error = fixture.prompt(client, "packet.v2.md").unwrap_err();
            assert_eq!(
                error,
                "runner.json records execution policy 1 but the run manifest's is 2"
            );

            put(
                &runner,
                r#"{"artifact_root": "report", "sandbox": "bwrap-v1", "execution_policy": 2}"#,
            );
            fixture.freeze_under(client, invocation, true, open, &pins);
            let error = fixture.prompt(client, "packet.v2.md").unwrap_err();
            assert!(
                error.contains("needs runner.json sandbox bwrap-v2"),
                "{error}"
            );

            put(
                &runner,
                r#"{"artifact_root": "report", "sandbox": "bwrap-v2", "execution_policy": 2}"#,
            );
            fixture.freeze_under(client, invocation, true, open, &pins);
            let prompt = fixture.prompt(client, "packet.v2.md").unwrap();
            assert!(
                prompt.contains("**Execution allowance.** Five minutes. Run anything."),
                "{prompt}"
            );
            assert!(prompt.contains("**Unavailable.** None."), "{prompt}");
            assert!(prompt.contains("Use `test-model` at `high`."), "{prompt}");

            fixture.freeze_under(client, invocation, true, "", &pins);
            let error = fixture.prompt(client, "packet.v2.md").unwrap_err();
            assert_eq!(
                error,
                "runner.json records execution policy 2 but the run manifest's is 1"
            );
            put(&runner, r#"{"artifact_root": "report"}"#);
        }
    }

    #[test]
    fn bwrap_v2_shows_only_uv_managed_pythons_and_records_the_policy() {
        if review_isolation::which("bwrap").is_none() {
            return;
        }
        let fixture = Recut::new();
        fixture.freeze(Client::Claude, "Review {PACKET}\n", false);
        let home = source_home();
        let uv = home.join(".local/share/uv");
        for (profile, shown, hidden) in [
            ("bwrap-v1", uv.clone(), None),
            ("bwrap-v2", uv.join("python"), Some(uv.join("credentials"))),
        ] {
            let attempt = fixture
                .root
                .parent()
                .unwrap()
                .join(format!("attempt-{profile}"));
            fs::create_dir_all(attempt.join("tmp")).unwrap();
            let prepared = Prepared {
                run_dir: fixture.run.clone(),
                attempt: attempt.clone(),
                clone: attempt.clone(),
                entry: Value::Null,
                model: String::new(),
                effort: String::new(),
                skill_source: PathBuf::new(),
                tree_hash: String::new(),
                runner: Value::Null,
                cache: PathBuf::new(),
                work: PathBuf::new(),
                artifact_root: PathBuf::new(),
                skill_name: String::new(),
                input: PathBuf::new(),
                prompt: String::new(),
                tools: review_isolation::own_executable().unwrap(),
                target_dir: fixture.target.clone(),
                packet: PathBuf::new(),
                policy: 2,
            };
            let (_, receipt, gate) =
                start_sandbox(&prepared, profile, Path::new("/usr"), &[]).unwrap();
            if let Some(gate) = gate {
                gate.stop().unwrap();
            }
            let saved = read_json(&attempt.join("sandbox.json")).unwrap();
            assert!(saved.py_eq(&receipt));
            assert!(get(&saved, "execution_policy").py_eq(&int(2)));
            let readonly = get(&saved, "readonly").py_str();
            if uv.join("python").is_dir() {
                assert!(
                    readonly.contains(&format!("'{}'", shown.display())),
                    "{readonly}"
                );
            }
            if let Some(hidden) = hidden {
                assert!(
                    !readonly.contains(&format!("'{}'", uv.display())),
                    "{readonly}"
                );
                assert!(!readonly.contains(&hidden.to_string_lossy().into_owned()));
            }
        }
    }
}
