//! Build, check and clone the truncated mirror of a benchmark target, and build and restore its dependency cache.
//!
//! `mirror` creates `<cache-root>/mirrors/<id>.git` from a staging clone holding the full upstream history:
//! `review-head` at the target's `head` and the local base branch at its `merge_base`. It then checks that every
//! `negative_shas` entry is absent, that nothing newer than the head is reachable from `review-head`, and that the
//! diff identity of `<base>..review-head` is the target's `diff_manifest_sha256`. `check` repeats those checks on
//! an existing mirror, and `clone` makes one attempt's working clone, repeating the leak and identity checks.
//!
//! `provisioning.cache` in `target.json` describes the dependency cache: `kind`, `build` (run once, online, in a
//! scratch clone at the head), `post_clone` (run offline in every attempt clone, which must stay clean), `env`
//! and `smoke` (named commands at the `head`, the `base` or `both`). Commands run through `sh -c` with
//! `{cache}`, `{clone}`, `{work}` and `{cache_root}` substituted. `cache` archives the built cache to
//! `<cache-root>/archives/<id>.tar.gz` and prints its `dependency_identity` entry. `prepare` clones, checks the
//! archive against the hash `target.json` records, restores it into `<out>-cache` and runs the post-clone steps.
//! `smoke` measures provisioning and the smoke checks and writes `smoke.json`.
//!
//! `prepare` refuses unless the output's filesystem keeps `BENCH_DISK_RESERVE_GIB` (default 20) free beyond what
//! it estimates it will write, and preparations sharing a cache root run one at a time. `--cache-replacements`
//! replaces a deleted frozen archive's expected hash in memory only.
//!
//! Exit codes: 0 done; 1 a check failed, one line per failure on stdout; 2 an input cannot be read, a pinned
//! revision is missing or a git command failed.

use crate::grade::provision::{apply_replacement, remove_tree};
use crate::pyjson::{self, Dumps, Object, Value};
use crate::pytext::{self, PyPath};
use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const DEFAULT_RESERVE_GIB: f64 = 20.0;
const GIB: f64 = (1u64 << 30) as f64;
const BLOCK: u64 = 4096;

type P<T> = Result<T, String>;

/// What a subcommand printed and its exit code; an error is exit 2.
pub struct Outcome {
    pub stdout: String,
    pub code: u8,
}

impl Outcome {
    fn lines(lines: &[String], code: u8) -> Self {
        Self {
            stdout: lines.iter().map(|line| format!("{line}\n")).collect(),
            code,
        }
    }
}

pub struct Options {
    pub target: PathBuf,
    pub cache_root: PathBuf,
    pub cache_replacements: Option<PathBuf>,
}

/// `~/.t3/bench-cache`.
pub fn default_cache_root() -> PathBuf {
    let home = std::env::var_os("HOME").unwrap_or_default();
    Path::new(&home).join(".t3/bench-cache")
}

fn indented(value: &Value, ensure_ascii: bool) -> String {
    pyjson::dumps(
        value,
        &Dumps {
            ensure_ascii,
            indent: Some(2),
            ..Dumps::default()
        },
    )
}

fn text(value: &str) -> Value {
    Value::Str(value.to_string())
}

fn object(entries: Vec<(&str, Value)>) -> Value {
    Value::Object(
        entries
            .into_iter()
            .map(|(key, value)| (key.to_string(), value))
            .collect(),
    )
}

fn os_failure(path: &Path) -> impl Fn(std::io::Error) -> String + '_ {
    move |error| pytext::os_error(&error, path.as_os_str().as_bytes())
}

fn git(arguments: &[&OsStr]) -> P<String> {
    let done = Command::new("git")
        .args(arguments)
        .stdin(Stdio::null())
        .output()
        .map_err(|error| format!("cannot run git: {}", pytext::os_error(&error, b"git")))?;
    if !done.status.success() {
        let command: Vec<String> = std::iter::once("git".to_string())
            .chain(
                arguments
                    .iter()
                    .map(|part| part.to_string_lossy().into_owned()),
            )
            .collect();
        return Err(format!(
            "command failed: {}\n{}",
            command.join(" "),
            String::from_utf8_lossy(&done.stderr).trim()
        ));
    }
    String::from_utf8(done.stdout).map_err(|_| "git printed output that is not UTF-8".to_string())
}

fn git_in(repo: &Path, arguments: &[&str]) -> P<String> {
    let mut all: Vec<&OsStr> = vec![OsStr::new("-C"), repo.as_os_str()];
    all.extend(arguments.iter().map(OsStr::new));
    git(&all)
}

fn has_object(repo: &Path, sha: &str) -> bool {
    Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["cat-file", "-e", sha])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

/// The fields of `target.json` provisioning reads.
pub struct Target {
    pub value: Value,
    pub id: String,
    pub head: String,
    pub merge_base: String,
    pub base_branch: String,
    path: PathBuf,
}

fn field<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    value.as_object().and_then(|object| object.get(key))
}

fn string_field(value: &Value, key: &str) -> String {
    field(value, key).map_or_else(String::new, Value::py_str)
}

/// Read `<target_dir>/target.json`, applying a replacement manifest's archive hash in memory.
pub fn load_target(target_dir: &Path, replacements: Option<&Path>) -> P<Target> {
    let path = target_dir.join("target.json");
    let raw = std::fs::read_to_string(&path)
        .map_err(|error| format!("{}: {}", path.display(), os_failure(&path)(error)))?;
    let mut value = pyjson::loads(&raw, pyjson::Duplicates::KeepLast)
        .map_err(|error| format!("{}: {error}", path.display()))?;
    for key in [
        "id",
        "head",
        "merge_base",
        "negative_shas",
        "diff_manifest_sha256",
        "local_base_branch",
    ] {
        if field(&value, key).is_none() {
            return Err(format!("{}: missing {key}", path.display()));
        }
    }
    if let Some(manifest) = replacements {
        let as_py = |path: &Path| PyPath::new(path.as_os_str().as_bytes()).to_owned();
        apply_replacement(&mut value, &as_py(&path), &as_py(manifest)).map_err(|failure| {
            match failure {
                crate::current_grading::Failure::Inconsistent(bytes) => {
                    String::from_utf8_lossy(&bytes).into_owned()
                }
                crate::current_grading::Failure::Input(message)
                | crate::current_grading::Failure::Crash(message) => message,
            }
        })??;
    }
    Ok(Target {
        id: string_field(&value, "id"),
        head: string_field(&value, "head"),
        merge_base: string_field(&value, "merge_base"),
        base_branch: string_field(&value, "local_base_branch"),
        value,
        path,
    })
}

struct Smoke {
    name: String,
    command: String,
    revision: String,
}

/// `provisioning.cache` over the empty cache's defaults.
struct Cache {
    kind: String,
    build: Vec<String>,
    post_clone: Vec<String>,
    env: Vec<(String, String)>,
    smoke: Vec<Smoke>,
}

fn strings(value: Option<&Value>, name: &str) -> P<Vec<String>> {
    match value {
        None => Ok(Vec::new()),
        Some(Value::Array(items)) => items
            .iter()
            .map(|item| {
                item.as_str()
                    .map(str::to_string)
                    .ok_or_else(|| format!("provisioning.cache.{name} holds a non-string"))
            })
            .collect(),
        Some(_) => Err(format!("provisioning.cache.{name} is not a list")),
    }
}

impl Cache {
    fn of(target: &Target) -> P<Self> {
        let cache = field(&target.value, "provisioning")
            .and_then(|provisioning| field(provisioning, "cache"))
            .filter(|cache| cache.truthy());
        let get = |key: &str| cache.and_then(|cache| field(cache, key));
        let env = match get("env") {
            None => Vec::new(),
            Some(Value::Object(env)) => env
                .iter()
                .map(|(key, value)| (key.clone(), value.py_str()))
                .collect(),
            Some(_) => return Err("provisioning.cache.env is not an object".to_string()),
        };
        let mut smoke = Vec::new();
        if let Some(items) = get("smoke") {
            for item in items
                .as_array()
                .ok_or("provisioning.cache.smoke is not a list")?
            {
                smoke.push(Smoke {
                    name: string_field(item, "name"),
                    command: string_field(item, "command"),
                    revision: field(item, "revision")
                        .map_or_else(|| "head".to_string(), Value::py_str),
                });
            }
        }
        Ok(Self {
            kind: get("kind").map_or_else(|| "none".to_string(), Value::py_str),
            build: strings(get("build"), "build")?,
            post_clone: strings(get("post_clone"), "post_clone")?,
            env,
            smoke,
        })
    }

    fn archive_name(&self) -> String {
        format!("cache archive ({})", self.kind)
    }
}

fn now() -> String {
    crate::normalize_review::utc_now()
}

fn iso_utc(seconds: i64) -> String {
    crate::normalize_review::utc_stamp(u64::try_from(seconds).unwrap_or(0)).replace('Z', "+00:00")
}

/// The failure lines of the leak, history and diff-identity checks; empty when everything holds.
pub(crate) fn checks(
    repo: &Path,
    target: &Target,
    base_ref: &str,
    head_ref: &str,
    date_check: bool,
) -> Vec<String> {
    let mut failures = Vec::new();
    for entry in field(&target.value, "negative_shas")
        .and_then(Value::as_array)
        .unwrap_or_default()
    {
        let sha = string_field(entry, "sha");
        if has_object(repo, &sha) {
            let reason = field(entry, "reason")
                .map_or_else(|| "no reason recorded".to_string(), Value::py_str);
            failures.push(format!("leak: negative sha {sha} is present ({reason})"));
        }
    }
    if date_check {
        // Unix timestamps, not ISO strings: upstream histories carry malformed offsets (psf/requests has a 2011
        // commit stamped +518:00) that no ISO parser accepts, and git itself compares by %ct.
        let stamps = || -> P<(i64, i64)> {
            let parse = |word: &str| {
                word.parse::<i64>()
                    .map_err(|_| format!("invalid commit timestamp {word:?}"))
            };
            let all = git_in(repo, &["log", "--format=%ct", head_ref])?
                .split_whitespace()
                .map(parse)
                .collect::<P<Vec<i64>>>()?;
            let head = parse(git_in(repo, &["log", "-1", "--format=%ct", head_ref])?.trim())?;
            Ok((all.into_iter().max().unwrap_or(head), head))
        };
        match stamps() {
            Err(error) => failures.push(format!("history: {error}")),
            Ok((newest, head)) if newest != head => failures.push(format!(
                "history: newest reachable commit {} is later than the head's {}",
                iso_utc(newest),
                iso_utc(head)
            )),
            Ok(_) => {}
        }
    }
    match crate::diff_identity::identity(
        repo.as_os_str(),
        OsStr::new(base_ref),
        OsStr::new(head_ref),
    ) {
        Err(error) => failures.push(format!("diff identity: {}", error.0)),
        Ok((_, digest)) => {
            let expected = string_field(&target.value, "diff_manifest_sha256");
            if digest != expected {
                failures.push(format!(
                    "diff identity mismatch: expected {expected}, computed {digest}"
                ));
            }
        }
    }
    failures
}

pub(crate) fn mirror_path(cache_root: &Path, target: &Target) -> PathBuf {
    cache_root
        .join("mirrors")
        .join(format!("{}.git", target.id))
}

fn cache_path(cache_root: &Path, target: &Target) -> PathBuf {
    cache_root.join("caches").join(&target.id)
}

fn archive_path(cache_root: &Path, target: &Target) -> PathBuf {
    cache_root
        .join("archives")
        .join(format!("{}.tar.gz", target.id))
}

fn remove(path: &Path) -> P<()> {
    remove_tree(path.as_os_str().as_bytes())
}

fn make_directories(path: &Path) -> P<()> {
    std::fs::create_dir_all(path).map_err(os_failure(path))
}

/// Clone the mirror into `out` with the attempt layout and return the check failures; the clone is removed when
/// any fail.
fn make_clone(target: &Target, cache_root: &Path, out: &Path) -> P<Vec<String>> {
    let mirror = mirror_path(cache_root, target);
    if !mirror.is_dir() {
        return Err(format!("no mirror at {}", mirror.display()));
    }
    if out.exists() {
        return Err(format!(
            "clone target exists: {} (attempt directories are never reused)",
            out.display()
        ));
    }
    git(&[
        OsStr::new("clone"),
        OsStr::new("-q"),
        mirror.as_os_str(),
        out.as_os_str(),
    ])?;
    git_in(out, &["checkout", "-q", "--detach", &target.head])?;
    git_in(
        out,
        &[
            "branch",
            "-q",
            "-f",
            &target.base_branch,
            &target.merge_base,
        ],
    )?;
    git_in(out, &["checkout", "-q", "-B", "review-head", &target.head])?;
    let mut failures = Vec::new();
    if git_in(out, &["rev-parse", "HEAD"])?.trim() != target.head {
        failures.push("head mismatch after checkout".to_string());
    }
    failures.extend(checks(
        out,
        target,
        &target.base_branch,
        "review-head",
        false,
    ));
    if !failures.is_empty() {
        std::fs::remove_dir_all(out).map_err(os_failure(out))?;
    }
    Ok(failures)
}

struct Substitutions {
    cache: PathBuf,
    cache_root: PathBuf,
    clone: PathBuf,
    work: PathBuf,
}

impl Substitutions {
    fn render(&self, command: &str) -> String {
        let mut text = command.to_string();
        for (key, value) in [
            ("cache", &self.cache),
            ("clone", &self.clone),
            ("work", &self.work),
            ("cache_root", &self.cache_root),
        ] {
            text = text.replace(&format!("{{{key}}}"), &value.to_string_lossy());
        }
        text
    }

    fn environment(&self, cache: &Cache) -> Vec<(String, String)> {
        cache
            .env
            .iter()
            .map(|(key, value)| (key.clone(), self.render(value)))
            .collect()
    }
}

struct Ran {
    code: i64,
    duration: f64,
    output: String,
    stdout: String,
}

fn universal_newlines(raw: &[u8]) -> String {
    String::from_utf8_lossy(raw)
        .replace("\r\n", "\n")
        .replace('\r', "\n")
}

/// Run one command through `sh -c` with `environment` added to this process's.
fn shell(command: &str, cwd: &Path, environment: &[(String, String)]) -> P<Ran> {
    use std::os::unix::process::ExitStatusExt;
    let started = std::time::Instant::now();
    let done = Command::new("sh")
        .args(["-c", command])
        .current_dir(cwd)
        .envs(environment.iter().map(|(key, value)| (key, value)))
        .stdin(Stdio::null())
        .output()
        .map_err(|error| format!("cannot run sh: {}", pytext::os_error(&error, b"sh")))?;
    let stdout = universal_newlines(&done.stdout);
    Ok(Ran {
        code: done
            .status
            .code()
            .map_or_else(|| -i64::from(done.status.signal().unwrap_or(0)), i64::from),
        duration: crate::usage::round_to(started.elapsed().as_secs_f64(), 2),
        output: stdout.clone() + &universal_newlines(&done.stderr),
        stdout,
    })
}

fn platform_record() -> String {
    let probe = |program: &str, arguments: &[&str]| {
        Command::new(program)
            .args(arguments)
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()
            .ok()
            .map(|done| String::from_utf8_lossy(&done.stdout).trim().to_string())
            .filter(|out| !out.is_empty())
    };
    let mut parts = vec![probe("uname", &["-srm"]).unwrap_or_else(|| "unknown".to_string())];
    for (program, argument) in [
        ("python3", "--version"),
        ("node", "--version"),
        ("go", "version"),
        ("zsh", "--version"),
        ("git", "--version"),
    ] {
        if let Some(out) = probe(program, &[argument]) {
            let out = out.replace("go version go", "go ").replace("version ", "");
            parts.push(out.lines().next().unwrap_or_default().to_string());
        }
    }
    parts.join("; ")
}

fn tail(output: &str, lines: usize) -> String {
    let kept: Vec<&str> = output
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();
    kept[kept.len().saturating_sub(lines)..].join("\n")
}

fn sha256_file(path: &Path) -> P<String> {
    use std::io::Read;
    let mut file = std::fs::File::open(path).map_err(os_failure(path))?;
    let mut digest = crate::sha256::Sha256::new();
    let mut buffer = vec![0u8; 1 << 20];
    loop {
        let read = file.read(&mut buffer).map_err(os_failure(path))?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(crate::sha256::hex(&digest.finalize()))
}

fn write_file(path: &Path, content: &str) -> P<()> {
    std::fs::write(path, content).map_err(os_failure(path))
}

fn absolute(path: &Path) -> P<PathBuf> {
    std::path::absolute(path).map_err(os_failure(path))
}

fn short(sha: &str) -> &str {
    &sha[..sha.len().min(12)]
}

pub fn mirror(options: &Options, staging: &Path) -> P<Outcome> {
    let target = load_target(&options.target, None)?;
    let staging = absolute(staging)?;
    for sha in [&target.head, &target.merge_base] {
        if !has_object(&staging, sha) {
            return Err(format!("staging clone {} lacks {sha}", staging.display()));
        }
    }
    let base = &target.base_branch;
    let path = mirror_path(&options.cache_root, &target);
    if path.exists() {
        std::fs::remove_dir_all(&path).map_err(os_failure(&path))?;
    }
    let parent = path.parent().unwrap_or(Path::new("/"));
    make_directories(parent)?;
    git(&[
        OsStr::new("init"),
        OsStr::new("-q"),
        OsStr::new("--bare"),
        path.as_os_str(),
    ])?;
    let head_ref = format!("{}:refs/heads/review-head", target.head);
    let base_ref = format!("{}:refs/heads/{base}", target.merge_base);
    git(&[
        OsStr::new("-C"),
        staging.as_os_str(),
        OsStr::new("push"),
        OsStr::new("-q"),
        path.as_os_str(),
        OsStr::new(&head_ref),
        OsStr::new(&base_ref),
    ])?;
    git_in(
        &path,
        &["symbolic-ref", "HEAD", &format!("refs/heads/{base}")],
    )?;
    let failures = checks(&path, &target, base, "review-head", true);
    if !failures.is_empty() {
        std::fs::remove_dir_all(&path).map_err(os_failure(&path))?;
        return Ok(Outcome::lines(&failures, 1));
    }
    let negative: Vec<Value> = field(&target.value, "negative_shas")
        .and_then(Value::as_array)
        .unwrap_or_default()
        .iter()
        .map(|entry| field(entry, "sha").cloned().unwrap_or(Value::Null))
        .collect();
    let mut refs = Object::default();
    refs.insert("review-head".to_string(), text(&target.head));
    refs.insert(base.clone(), text(&target.merge_base));
    let record = object(vec![
        ("target", text(&target.id)),
        ("built_at", text(&now())),
        ("staging", text(&staging.to_string_lossy())),
        ("mirror", text(&path.to_string_lossy())),
        ("refs", Value::Object(refs)),
        ("negative_shas_checked", Value::Array(negative.clone())),
        (
            "diff_manifest_sha256",
            field(&target.value, "diff_manifest_sha256")
                .cloned()
                .unwrap_or(Value::Null),
        ),
        ("ok", Value::Bool(true)),
        ("failures", Value::Array(Vec::new())),
    ]);
    let receipt = parent.join(format!("{}.json", target.id));
    write_file(&receipt, &(indented(&record, true) + "\n"))?;
    Ok(Outcome::lines(
        &[format!(
            "mirror {}: review-head={} {base}={}; {} negative sha(s) absent; diff identity verified",
            path.display(),
            short(&target.head),
            short(&target.merge_base),
            negative.len()
        )],
        0,
    ))
}

pub fn check(options: &Options) -> P<Outcome> {
    let target = load_target(&options.target, options.cache_replacements.as_deref())?;
    let path = mirror_path(&options.cache_root, &target);
    if !path.is_dir() {
        return Err(format!("no mirror at {}", path.display()));
    }
    for (name, sha) in [
        ("review-head", &target.head),
        (target.base_branch.as_str(), &target.merge_base),
    ] {
        let Ok(actual) = git_in(
            &path,
            &["rev-parse", "--verify", &format!("refs/heads/{name}")],
        ) else {
            return Ok(Outcome::lines(
                &[format!("ref: {name} is missing from {}", path.display())],
                1,
            ));
        };
        if actual.trim() != sha {
            return Ok(Outcome::lines(
                &[format!("ref: {name} is {}, expected {sha}", actual.trim())],
                1,
            ));
        }
    }
    let failures = checks(&path, &target, &target.base_branch, "review-head", true);
    if failures.is_empty() {
        return Ok(Outcome::lines(
            &[format!("mirror {}: ok", path.display())],
            0,
        ));
    }
    Ok(Outcome::lines(&failures, 1))
}

pub fn clone(options: &Options, out: &Path) -> P<Outcome> {
    let target = load_target(&options.target, None)?;
    let out = absolute(out)?;
    let failures = make_clone(&target, &options.cache_root, &out)?;
    if !failures.is_empty() {
        return Ok(Outcome::lines(&failures, 1));
    }
    Ok(Outcome::lines(
        &[format!(
            "clone {}: review-head checked out at {}, {} at {}",
            out.display(),
            short(&target.head),
            target.base_branch,
            short(&target.merge_base)
        )],
        0,
    ))
}

fn step(command: &str, ran: &Ran) -> Vec<(&'static str, Value)> {
    vec![
        ("command", text(command)),
        ("exit_code", Value::Int(ran.code.to_string())),
        ("duration_seconds", Value::Float(ran.duration)),
    ]
}

pub fn cache(options: &Options) -> P<Outcome> {
    let target = load_target(&options.target, None)?;
    let config = Cache::of(&target)?;
    let cache_root = &options.cache_root;
    let cache = cache_path(cache_root, &target);
    let scratch = cache_root
        .join("scratch")
        .join(format!("{}-cache", target.id));
    let work = PathBuf::from(format!("{}-work", scratch.display()));
    for path in [&cache, &scratch, &work] {
        remove(path)?;
    }
    make_directories(&cache)?;
    make_directories(&work)?;
    let failures = make_clone(&target, cache_root, &scratch)?;
    if !failures.is_empty() {
        remove(&cache)?;
        return Ok(Outcome::lines(&failures, 1));
    }
    let built = build_cache(options, &target, &config, &cache, &scratch, &work);
    remove(&scratch)?;
    remove(&work)?;
    let record = match built? {
        Ok(record) => record,
        Err(lines) => {
            remove(&cache)?;
            return Ok(Outcome::lines(&lines, 1));
        }
    };
    let receipt = cache
        .parent()
        .unwrap_or(Path::new("/"))
        .join(format!("{}.json", target.id));
    write_file(&receipt, &(indented(&record, true) + "\n"))?;
    let steps = field(&record, "build")
        .and_then(Value::as_array)
        .map_or(0, <[Value]>::len);
    let lines = match field(&record, "archive").filter(|archive| archive.truthy()) {
        Some(archive) => {
            let home = std::env::var("HOME").unwrap_or_default();
            let location = string_field(archive, "path");
            let location = if home.is_empty() {
                location
            } else {
                location.replacen(&home, "~", 1)
            };
            let entry = object(vec![
                ("name", text(&config.archive_name())),
                (
                    "sha256",
                    field(archive, "sha256").cloned().unwrap_or(Value::Null),
                ),
                ("location", text(&location)),
            ]);
            vec![
                format!(
                    "cache {}: {steps} build step(s), archive {} bytes",
                    cache.display(),
                    string_field(archive, "bytes")
                ),
                format!(
                    "dependency_identity entry: {}",
                    pyjson::dumps(&entry, &Dumps::default())
                ),
            ]
        }
        None => vec![format!(
            "cache {}: nothing to build (kind {})",
            cache.display(),
            config.kind
        )],
    };
    Ok(Outcome::lines(&lines, 0))
}

/// Run the build steps and archive the cache: the cache record, or the failure lines.
fn build_cache(
    options: &Options,
    target: &Target,
    config: &Cache,
    cache: &Path,
    scratch: &Path,
    work: &Path,
) -> P<Result<Value, Vec<String>>> {
    let substitutions = Substitutions {
        cache: cache.to_path_buf(),
        cache_root: options.cache_root.clone(),
        clone: scratch.to_path_buf(),
        work: work.to_path_buf(),
    };
    let environment = substitutions.environment(config);
    let started = now();
    let mut steps = Vec::new();
    for command in &config.build {
        let rendered = substitutions.render(command);
        let ran = shell(&rendered, scratch, &environment)?;
        let mut entry = step(command, &ran);
        entry.push(("output_tail", text(&tail(&ran.output, 20))));
        steps.push(object(entry));
        if ran.code != 0 {
            return Ok(Err(vec![
                format!("build step failed (exit {}): {rendered}", ran.code),
                tail(&ran.output, 20),
            ]));
        }
    }
    let mut archive_record = Value::Null;
    if !config.build.is_empty() {
        let archive = archive_path(&options.cache_root, target);
        make_directories(archive.parent().unwrap_or(Path::new("/")))?;
        let command = format!(
            "tar -C {} -czf {} {}",
            crate::shlex::quote(&cache.parent().unwrap_or(Path::new("/")).to_string_lossy()),
            crate::shlex::quote(&archive.to_string_lossy()),
            crate::shlex::quote(&target.id)
        );
        let ran = shell(&command, scratch, &environment)?;
        if ran.code != 0 {
            return Ok(Err(vec![
                format!("archive failed (exit {})", ran.code),
                tail(&ran.output, 20),
            ]));
        }
        let bytes = std::fs::metadata(&archive)
            .map_err(os_failure(&archive))?
            .len();
        archive_record = object(vec![
            ("path", text(&archive.to_string_lossy())),
            ("sha256", text(&sha256_file(&archive)?)),
            ("bytes", Value::Int(bytes.to_string())),
            ("duration_seconds", Value::Float(ran.duration)),
        ]);
    }
    Ok(Ok(object(vec![
        ("target", text(&target.id)),
        ("target_sha256", text(&sha256_file(&target.path)?)),
        ("kind", text(&config.kind)),
        ("built_at", text(&started)),
        ("finished_at", text(&now())),
        ("platform", text(&platform_record())),
        ("cache", text(&cache.to_string_lossy())),
        ("build", Value::Array(steps)),
        ("archive", archive_record),
    ])))
}

fn dangling_links(root: &Path, directory: &Path, found: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_symlink() {
            if !path.exists() {
                let relative = path.strip_prefix(root).unwrap_or(&path);
                found.push(relative.to_string_lossy().into_owned());
            }
        } else if kind.is_dir() {
            dangling_links(root, &path, found);
        }
    }
}

/// The restore step, if one ran, and the failures.
type Restored = (Option<(String, Ran)>, Vec<String>);

/// Check the archive against the hash `target.json` records and extract it into `dest`.
fn restore_cache(target: &Target, config: &Cache, cache_root: &Path, dest: &Path) -> P<Restored> {
    let name = config.archive_name();
    let recorded: Vec<Value> = field(&target.value, "provisioning")
        .and_then(|provisioning| field(provisioning, "dependency_identity"))
        .and_then(Value::as_array)
        .unwrap_or_default()
        .iter()
        .filter(|entry| field(entry, "name").and_then(Value::as_str) == Some(name.as_str()))
        .map(|entry| field(entry, "sha256").cloned().unwrap_or(Value::Null))
        .collect();
    let archive = archive_path(cache_root, target);
    if recorded.len() != 1 {
        return Ok((
            None,
            vec![format!(
                "target.json records {} '{name}' entries in provisioning.dependency_identity, expected 1",
                recorded.len()
            )],
        ));
    }
    if !archive.is_file() {
        return Ok((
            None,
            vec![format!("no cache archive at {}", archive.display())],
        ));
    }
    let actual = sha256_file(&archive)?;
    if recorded[0].as_str() != Some(actual.as_str()) {
        return Ok((
            None,
            vec![format!(
                "cache archive {} has sha256 {actual}; target.json records {}",
                archive.display(),
                recorded[0].py_str()
            )],
        ));
    }
    if dest.exists() {
        return Err(format!(
            "cache directory exists: {} (attempt directories are never reused)",
            dest.display()
        ));
    }
    make_directories(dest)?;
    let ran = shell(
        &format!(
            "tar -C {} --strip-components=1 -xzf {}",
            crate::shlex::quote(&dest.to_string_lossy()),
            crate::shlex::quote(&archive.to_string_lossy())
        ),
        dest,
        &[],
    )?;
    if ran.code != 0 {
        let failure = format!(
            "cache restore failed (exit {}): {}\n{}",
            ran.code,
            archive.display(),
            tail(&ran.output, 10)
        );
        return Ok((
            Some((format!("restore cache archive sha256:{actual}"), ran)),
            vec![failure],
        ));
    }
    let restore = (format!("restore cache archive sha256:{actual}"), ran);
    // A relative link out of the cache resolves only at the build directory's depth.
    let mut dangling = Vec::new();
    dangling_links(dest, dest, &mut dangling);
    dangling.sort();
    if let Some(first) = dangling.first() {
        return Ok((
            Some(restore),
            vec![format!(
                "restored cache has {} dangling symlink(s), first {first}: the archive only works at the build directory's path",
                dangling.len()
            )],
        ));
    }
    Ok((Some(restore), Vec::new()))
}

fn blocks(size: u64) -> u64 {
    size.div_ceil(BLOCK) * BLOCK
}

fn allocated(directory: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return 0;
    };
    entries
        .flatten()
        .map(|entry| match std::fs::symlink_metadata(entry.path()) {
            Ok(metadata) if metadata.is_dir() => allocated(&entry.path()),
            Ok(metadata) => metadata.blocks() * 512,
            Err(_) => 0,
        })
        .sum()
}

fn archive_entries(archive: &Path) -> std::io::Result<u64> {
    let file = std::fs::File::open(archive)?;
    let mut bundle = tar::Archive::new(flate2::read::GzDecoder::new(file));
    let mut needed = 0;
    for entry in bundle.entries()? {
        let entry = entry?;
        needed += if entry.header().entry_type().is_file() {
            blocks(entry.size())
        } else {
            BLOCK
        };
    }
    Ok(needed)
}

/// Bytes `prepare` writes before its post-clone steps: the mirror's objects (a clone on another filesystem copies
/// them), the checked-out tree and the extracted cache archive.
fn space_needed(target: &Target, config: &Cache, cache_root: &Path) -> P<u64> {
    let mirror = mirror_path(cache_root, target);
    let mut needed = allocated(&mirror);
    for line in git_in(&mirror, &["ls-tree", "-r", "-l", &target.head])?.lines() {
        let size = line.split_whitespace().nth(3).unwrap_or_default();
        needed += size.parse::<u64>().map_or(BLOCK, blocks);
    }
    let archive = archive_path(cache_root, target);
    if !config.build.is_empty() && archive.is_file() {
        // restore_cache refuses an archive that tar cannot read.
        needed += archive_entries(&archive).unwrap_or(0);
    }
    Ok(needed)
}

fn space_refusal(free: u64, needed: u64, reserve: u64, out: &Path) -> Vec<String> {
    if free.saturating_sub(needed) >= reserve && free >= needed {
        return Vec::new();
    }
    let gib = |bytes: u64| bytes as f64 / GIB;
    vec![format!(
        "disk space: {} needs about {:.1} GiB and a {:.1} GiB reserve, but its filesystem has {:.1} GiB free; nothing was cloned",
        out.display(),
        gib(needed),
        gib(reserve),
        gib(free)
    )]
}

fn free_space(path: &Path) -> P<u64> {
    let stats =
        nix::sys::statvfs::statvfs(path).map_err(|error| format!("{}: {error}", path.display()))?;
    #[allow(
        clippy::useless_conversion,
        reason = "statvfs reports 32-bit counts on macOS"
    )]
    Ok(u64::from(stats.blocks_available()) * u64::from(stats.fragment_size()))
}

fn space_failures(
    target: &Target,
    config: &Cache,
    cache_root: &Path,
    out: &Path,
) -> P<Vec<String>> {
    let raw = std::env::var("BENCH_DISK_RESERVE_GIB").ok();
    let reserve = match &raw {
        None => DEFAULT_RESERVE_GIB,
        Some(raw) => raw.trim().parse::<f64>().unwrap_or(-1.0),
    };
    if !(0.0..f64::INFINITY).contains(&reserve) {
        return Err(format!(
            "BENCH_DISK_RESERVE_GIB is not a non-negative number: {}",
            text(raw.as_deref().unwrap_or_default()).py_repr()
        ));
    }
    if !mirror_path(cache_root, target).is_dir() {
        return Ok(Vec::new());
    }
    let mut existing = absolute(out)?;
    while !existing.is_dir() {
        existing = existing
            .parent()
            .map_or_else(|| PathBuf::from("/"), Path::to_path_buf);
    }
    let reserve = (reserve * GIB) as u64;
    Ok(space_refusal(
        free_space(&existing)?,
        space_needed(target, config, cache_root)?,
        reserve,
        out,
    ))
}

/// The provisioning record `prepare` prints.
struct Prepared {
    recipe: String,
    duration: f64,
    tree_clean_after: Option<bool>,
    steps: Vec<Value>,
    failures: Vec<String>,
}

impl Prepared {
    fn value(&self) -> Value {
        object(vec![
            ("recipe", text(&self.recipe)),
            ("duration_seconds", Value::Float(self.duration)),
            (
                "tree_clean_after",
                self.tree_clean_after.map_or(Value::Null, Value::Bool),
            ),
            ("steps", Value::Array(self.steps.clone())),
            (
                "failures",
                Value::Array(self.failures.iter().map(|line| text(line)).collect()),
            ),
        ])
    }
}

/// [`prepare_locked`] under the cache root's lock, which serializes preparations.
fn prepare(target: &Target, cache_root: &Path, out: &Path, base: bool) -> P<Prepared> {
    let path = cache_root.join("prepare.lock");
    let file = std::fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(&path)
        .map_err(|error| {
            format!(
                "cannot lock {}: {}",
                path.display(),
                os_failure(&path)(error)
            )
        })?;
    let _lock = nix::fcntl::Flock::lock(file, nix::fcntl::FlockArg::LockExclusive)
        .map_err(|(_, error)| format!("cannot lock {}: {error}", path.display()))?;
    prepare_locked(target, cache_root, out, base)
}

/// Check the free space, clone (checking out the base branch for `base`), restore the cache archive into
/// `<out>-cache` and run the post-clone steps.
fn prepare_locked(target: &Target, cache_root: &Path, out: &Path, base: bool) -> P<Prepared> {
    let config = Cache::of(target)?;
    let mut failures = space_failures(target, &config, cache_root, out)?;
    if failures.is_empty() {
        failures = make_clone(target, cache_root, out)?;
    }
    let mut record = Prepared {
        recipe: if config.post_clone.is_empty() {
            "none".to_string()
        } else {
            config.post_clone.join("; ")
        },
        duration: 0.0,
        tree_clean_after: None,
        steps: Vec::new(),
        failures,
    };
    if !record.failures.is_empty() {
        return Ok(record);
    }
    if base {
        git_in(out, &["checkout", "-q", &target.base_branch])?;
    }
    let suffixed = |suffix: &str| PathBuf::from(format!("{}{suffix}", out.display()));
    let substitutions = Substitutions {
        cache: suffixed("-cache"),
        cache_root: cache_root.to_path_buf(),
        clone: out.to_path_buf(),
        work: suffixed("-work"),
    };
    make_directories(&substitutions.work)?;
    let environment = substitutions.environment(&config);
    let mut total = 0.0;
    if !config.build.is_empty() {
        let (restore, failures) = restore_cache(target, &config, cache_root, &substitutions.cache)?;
        if let Some((command, ran)) = restore {
            total += ran.duration;
            record.steps.push(object(step(&command, &ran)));
            record.recipe = std::iter::once(command)
                .chain(config.post_clone.iter().cloned())
                .collect::<Vec<_>>()
                .join("; ");
        }
        if !failures.is_empty() {
            record.failures.extend(failures);
            record.duration = crate::usage::round_to(total, 2);
            return Ok(record);
        }
    }
    for command in &config.post_clone {
        let ran = shell(&substitutions.render(command), out, &environment)?;
        total += ran.duration;
        record.steps.push(object(step(command, &ran)));
        if ran.code != 0 {
            record.failures.push(format!(
                "post-clone step failed (exit {}): {command}\n{}",
                ran.code,
                tail(&ran.output, 10)
            ));
            break;
        }
    }
    record.duration = crate::usage::round_to(total, 2);
    if record.failures.is_empty() {
        let status = git_in(out, &["status", "--porcelain"])?;
        let dirty = status.trim();
        record.tree_clean_after = Some(dirty.is_empty());
        if !dirty.is_empty() {
            let first: Vec<&str> = dirty.lines().take(5).collect();
            record.failures.push(format!(
                "post-clone provisioning dirtied the tracked tree:\n{}",
                first.join("\n")
            ));
        }
    }
    Ok(record)
}

pub fn prepare_command(options: &Options, out: &Path) -> P<Outcome> {
    let target = load_target(&options.target, options.cache_replacements.as_deref())?;
    let record = prepare(&target, &options.cache_root, &absolute(out)?, false)?;
    Ok(Outcome {
        stdout: indented(&record.value(), true) + "\n",
        code: u8::from(!record.failures.is_empty()),
    })
}

fn smoke_checks(config: &Cache, cache_root: &Path, revisions: &[(&str, PathBuf)]) -> P<Vec<Value>> {
    let mut checks = Vec::new();
    for item in &config.smoke {
        for (revision, clone) in revisions
            .iter()
            .filter(|(revision, _)| item.revision == *revision || item.revision == "both")
        {
            let suffixed = |suffix: &str| PathBuf::from(format!("{}{suffix}", clone.display()));
            let substitutions = Substitutions {
                cache: suffixed("-cache"),
                cache_root: cache_root.to_path_buf(),
                clone: clone.clone(),
                work: suffixed("-work"),
            };
            let ran = shell(
                &substitutions.render(&item.command),
                clone,
                &substitutions.environment(config),
            )?;
            // The result line of a test runner is on stdout; stderr carries warnings that would hide it.
            let last = Some(tail(&ran.stdout, 1))
                .filter(|line| !line.is_empty())
                .unwrap_or_else(|| tail(&ran.output, 1));
            let last: String = last.chars().take(200).collect();
            checks.push(object(vec![
                ("name", text(&item.name)),
                ("command", text(&item.command)),
                ("revision", text(revision)),
                ("exit_code", Value::Int(ran.code.to_string())),
                ("duration_seconds", Value::Float(ran.duration)),
                (
                    "summary",
                    text(if last.is_empty() {
                        "(no output)"
                    } else {
                        &last
                    }),
                ),
            ]));
        }
    }
    Ok(checks)
}

pub fn smoke(options: &Options, out: Option<&Path>) -> P<Outcome> {
    let resolve = crate::review_isolation::resolve;
    if options.cache_replacements.is_some() {
        let Some(out) = out else {
            return Err(
                "smoke with --cache-replacements requires --out to preserve the frozen receipt"
                    .to_string(),
            );
        };
        if resolve(out) == resolve(&options.target.join("smoke.json")) {
            return Err(
                "replacement smoke output must differ from the frozen target's smoke.json"
                    .to_string(),
            );
        }
    }
    let target = load_target(&options.target, options.cache_replacements.as_deref())?;
    let config = Cache::of(&target)?;
    let out = match out {
        Some(out) => absolute(out)?,
        None => absolute(&options.target)?.join("smoke.json"),
    };
    let cache_root = &options.cache_root;
    let scratch_root = cache_root.join("scratch");
    let mut revisions = vec![("head", scratch_root.join(format!("{}-smoke", target.id)))];
    if config
        .smoke
        .iter()
        .any(|item| item.revision == "base" || item.revision == "both")
    {
        revisions.push((
            "base",
            scratch_root.join(format!("{}-smoke-base", target.id)),
        ));
    }
    let paths: Vec<PathBuf> = revisions
        .iter()
        .flat_map(|(_, path)| {
            ["", "-work", "-cache"]
                .map(|suffix| PathBuf::from(format!("{}{suffix}", path.display())))
        })
        .collect();
    for path in &paths {
        remove(path)?;
    }
    make_directories(&scratch_root)?;
    let measured_at = now();
    type Measured = Result<(Vec<Prepared>, Vec<Value>), Vec<String>>;
    let measured = (|| -> P<Measured> {
        let mut records = Vec::new();
        for (revision, clone) in &revisions {
            let record = prepare(&target, cache_root, clone, *revision == "base")?;
            if !record.failures.is_empty() {
                return Ok(Err(record
                    .failures
                    .iter()
                    .map(|line| format!("{revision}: {line}"))
                    .collect()));
            }
            records.push(record);
        }
        Ok(Ok((
            records,
            smoke_checks(&config, cache_root, &revisions)?,
        )))
    })();
    for path in &paths {
        remove(path)?;
    }
    let (records, checks) = match measured? {
        Ok(measured) => measured,
        Err(lines) => return Ok(Outcome::lines(&lines, 1)),
    };
    let head = &records[0];
    let previous = std::fs::read_to_string(&out)
        .ok()
        .and_then(|raw| pyjson::loads(&raw, pyjson::Duplicates::KeepLast).ok())
        .filter(|previous| previous.as_object().is_some())
        .unwrap_or_else(|| Value::Object(Object::default()));
    let mut notes = vec![format!(
        "Measured by bench-tools provision smoke on this machine; cache kind {}.{}",
        config.kind,
        if config.build.is_empty() {
            ""
        } else {
            " Each clone restored its own copy of the cache archive after checking its recorded hash."
        }
    )];
    if let Some(replacement) = field(&target.value, "_cache_replacement")
        .and_then(|replacement| field(replacement, "manifest"))
    {
        notes.push(format!(
            "Cache replacement manifest {} sha256:{} selected the archive.",
            string_field(replacement, "path"),
            string_field(replacement, "sha256")
        ));
    }
    if let Some(base) = records.get(1) {
        notes.push(format!(
            "Base-revision checks ran in a second clone with {} (the merge-base) checked out, provisioned the same way in {}s; tracked tree clean afterwards: {}.",
            target.base_branch,
            pyjson::float_repr(base.duration),
            base.tree_clean_after.map_or("none", |clean| if clean { "true" } else { "false" })
        ));
    }
    let source_document = field(&previous, "source_document").filter(|value| value.truthy());
    if field(&previous, "source").and_then(Value::as_str) == Some("recorded")
        && let Some(document) = source_document
    {
        notes.push(format!(
            "The earlier recorded figures were transcribed from {}.",
            document.py_str()
        ));
    } else {
        // A re-measurement keeps the provenance of the figures it replaced.
        notes.extend(
            field(&previous, "notes")
                .and_then(Value::as_array)
                .unwrap_or_default()
                .iter()
                .filter_map(Value::as_str)
                .filter(|note| note.starts_with("The earlier recorded figures"))
                .map(str::to_string),
        );
    }
    let failed: Vec<String> = checks
        .iter()
        .filter(|check| string_field(check, "exit_code") != "0")
        .map(|check| string_field(check, "name"))
        .collect();
    let count = checks.len();
    let mut smoke = object(vec![
        ("schema_version", Value::Int("1".to_string())),
        ("target", text(&target.id)),
        ("source", text("measured")),
        ("measured_at", text(&measured_at)),
        ("platform", text(&platform_record())),
        (
            "provisioning",
            object(vec![
                ("recipe", text(&head.recipe)),
                ("duration_seconds", Value::Float(head.duration)),
                (
                    "tree_clean_after",
                    head.tree_clean_after.map_or(Value::Null, Value::Bool),
                ),
            ]),
        ),
        ("checks", Value::Array(checks)),
        (
            "notes",
            Value::Array(notes.iter().map(|note| text(note)).collect()),
        ),
    ]);
    if let (Value::Object(smoke), Some(mirror)) = (
        &mut smoke,
        field(&previous, "mirror").filter(|mirror| mirror.truthy()),
    ) {
        smoke.insert("mirror".to_string(), mirror.clone());
    }
    write_file(&out, &(indented(&smoke, false) + "\n"))?;
    let failed = if failed.is_empty() {
        String::new()
    } else {
        format!(", non-zero: {}", failed.join(", "))
    };
    Ok(Outcome::lines(
        &[format!(
            "smoke {}: provisioning {}s, {count} check(s){failed}",
            out.display(),
            pyjson::float_repr(head.duration)
        )],
        0,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_space_refusal_keeps_the_reserve_beyond_the_estimate() {
        let gib = 1u64 << 30;
        let out = Path::new("/out");
        assert!(!space_refusal(10 * gib, 6 * gib, 5 * gib, out).is_empty());
        assert!(space_refusal(11 * gib, 6 * gib, 5 * gib, out).is_empty());
        assert!(!space_refusal(gib, 2 * gib, 0, out).is_empty());
    }

    #[test]
    fn a_tail_keeps_the_last_non_blank_lines() {
        assert_eq!(tail("a\n\n b \n  \nc\n", 2), " b \nc");
        assert_eq!(tail("", 1), "");
    }
}
