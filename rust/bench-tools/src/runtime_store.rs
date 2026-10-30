//! The runtime store: build, retain, select and run pinned `bench-tools` executables, so a Rust-backed operation
//! is as attributable as a frozen Python one. `bench/tools/bench-tools` is the launcher every command goes through;
//! docs/python-migration/runtime.md describes the whole contract.
//!
//! The store (`BENCH_RUNTIME_STORE`, else `~/.t3/bench-runtime`) keeps each built executable under
//! `bench-tools/<sha256>/` beside its `receipt.json`. Entries are never overwritten or removed, so every retained
//! version stays runnable offline for rollback. `selected.json` names the version new operations take; `select`
//! and `rollback` rewrite it, append to `selections.jsonl` and move `launcher` to the selected entry when it has
//! these commands. `bench/tools/bench-tools` runs `launcher` to resolve every other command, so a rollback to a
//! version from before them still runs, and an entry that was only installed or restored never handles a command.
//!
//! `install` builds the crate's Git tree at `HEAD` from a snapshot inside a private Cargo home that holds no
//! configuration, with the toolchain `rust-toolchain.toml` pins, `cargo build --locked --offline --release` for this
//! host and only [`BUILD_ENVIRONMENT`] passed through, and refuses what a receipt could not restore.
//!
//! `exec` and `resolve` take the first of: `BENCH_RUST_BENCH_TOOLS` with a matching `BENCH_RUST_BENCH_TOOLS_SHA256`,
//! as a queue launch sets them; with `--run`, the run's `runtimes.frozen.json` pin as its runtime deviations moved
//! it, refusing a run that pins none;
//! otherwise the selected version, only while its receipt's crate tree is the crate tree at `HEAD`. There is no
//! fallback to another version, to `cargo run` or to an unreceipted `target/` build.
//!
//! Exit codes: 0 done; 1 a check refused; 2 an input could not be read or a subprocess failed. Once `exec` starts
//! the executable, the exit code is that program's.

use crate::grade::runtime::{self, Checked, DEVIATION, NAME};
use crate::pyjson::{self, Dumps, Duplicates, Object, Value};
use crate::{normalize_review, run_cell, sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::{Read, Write};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub const STORE_VARIABLE: &str = "BENCH_RUNTIME_STORE";
pub const INSTALL: &str = "bench/tools/bench-tools runtime install --select";
/// The crate `install` builds, relative to the checkout.
pub const CRATE: &str = "rust/bench-tools";
const LAUNCHER: &str = "launcher";
/// The only variables a build sees. Any other variable a crate reads is absent, so it reads the same everywhere.
pub const BUILD_ENVIRONMENT: [&str; 6] = [
    "PATH",
    "HOME",
    "CARGO_HOME",
    "RUSTUP_HOME",
    "RUSTUP_TOOLCHAIN",
    "TMPDIR",
];
/// Cargo reads configuration from every ancestor of its working directory; this one has no ancestor but itself.
const CARGO_CWD: &str = "/";

#[derive(Debug)]
pub enum Failure {
    /// A check refused the runtime; exit code 1.
    Refused(String),
    /// An input is missing or a subprocess failed; exit code 2.
    Input(String),
}

pub type R<T> = Result<T, Failure>;

fn refused<T>(message: impl Into<String>) -> R<T> {
    Err(Failure::Refused(message.into()))
}

fn input<T>(message: impl Into<String>) -> R<T> {
    Err(Failure::Input(message.into()))
}

fn shown(path: &Path) -> String {
    path.display().to_string()
}

fn os_failure(path: &Path) -> impl Fn(std::io::Error) -> Failure + '_ {
    move |error| Failure::Input(format!("{}: {error}", shown(path)))
}

fn checked(failure: Checked) -> Failure {
    match failure {
        Checked::Refused(why) => Failure::Refused(why),
        Checked::Input(why) => Failure::Input(why),
    }
}

fn now() -> String {
    normalize_review::utc_stamp(run_cell::now_seconds())
}

/// `json.dumps(value, indent=2, sort_keys=True)` and a newline, as every record here is written.
pub fn dumps(value: &Value) -> String {
    let options = Dumps {
        indent: Some(2),
        sort_keys: true,
        ..Dumps::default()
    };
    pyjson::dumps(value, &options) + "\n"
}

fn object(entries: Vec<(&str, Value)>) -> Value {
    let mut object = Object::default();
    for (key, value) in entries {
        object.insert(key.to_string(), value);
    }
    Value::Object(object)
}

fn text(value: impl Into<String>) -> Value {
    Value::Str(value.into())
}

fn field<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    path.iter()
        .try_fold(value, |value, key| value.as_object()?.get(key))
}

fn field_text<'a>(value: &'a Value, path: &[&str]) -> Option<&'a str> {
    field(value, path).and_then(Value::as_str)
}

fn is_sha256(text: &str) -> bool {
    text.len() == 64
        && text
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

pub fn sha256_file(path: &Path) -> R<String> {
    let mut file = fs::File::open(path).map_err(os_failure(path))?;
    let mut digest = sha256::Sha256::new();
    let mut block = vec![0; 1 << 20];
    loop {
        let read = file.read(&mut block).map_err(os_failure(path))?;
        if read == 0 {
            return Ok(sha256::hex(&digest.finalize()));
        }
        digest.update(&block[..read]);
    }
}

/// A unique sibling name for a file written in one step.
fn partial_name(path: &Path, label: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos());
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    path.with_file_name(format!(".{name}.{}.{nanos}.{label}", std::process::id()))
}

/// Write `path` in one step; with `exclusive`, refuse a path that exists.
fn publish(path: &Path, content: &str, exclusive: bool) -> R<()> {
    let partial = partial_name(path, "partial");
    let written = (|| {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&partial)?;
        file.write_all(content.as_bytes())?;
        file.sync_all()
    })();
    let outcome = written.map_err(os_failure(&partial)).and_then(|()| {
        if exclusive {
            fs::hard_link(&partial, path).map_err(|error| {
                if error.kind() == std::io::ErrorKind::AlreadyExists {
                    Failure::Refused(format!(
                        "{} exists; a record is never overwritten",
                        shown(path)
                    ))
                } else {
                    os_failure(path)(error)
                }
            })
        } else {
            fs::rename(&partial, path).map_err(os_failure(path))
        }
    });
    let _ = fs::remove_file(&partial);
    outcome
}

fn read_json(path: &Path) -> R<Value> {
    let raw = match fs::read(path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return refused(format!("{} is missing", shown(path)));
        }
        Err(error) => return refused(format!("{} cannot be read: {error}", shown(path))),
    };
    let parsed = std::str::from_utf8(&raw)
        .map_err(|error| error.to_string())
        .and_then(|text| {
            pyjson::loads(text, Duplicates::KeepLast).map_err(|error| format!("{error:?}"))
        });
    match parsed {
        Ok(value) if value.as_object().is_some() => Ok(value),
        Ok(_) => refused(format!("{} is not a JSON object", shown(path))),
        Err(error) => refused(format!("{} cannot be read: {error}", shown(path))),
    }
}

/// `Path(path).expanduser().resolve()`.
fn located(path: &Path) -> PathBuf {
    let raw = path.as_os_str().as_bytes();
    let expanded = match (raw.strip_prefix(b"~"), std::env::var_os("HOME")) {
        (Some(rest), Some(home)) if rest.is_empty() || rest.starts_with(b"/") => {
            let mut joined = home.as_bytes().to_vec();
            joined.extend_from_slice(rest);
            joined
        }
        _ => raw.to_vec(),
    };
    PathBuf::from(OsStr::from_bytes(&crate::pyfs::resolve(&expanded)))
}

/// `BENCH_RUNTIME_STORE`, else `~/.t3/bench-runtime`, resolved.
pub fn default_store() -> PathBuf {
    match std::env::var_os(STORE_VARIABLE).filter(|store| !store.is_empty()) {
        Some(store) => located(Path::new(&store)),
        None => located(Path::new("~/.t3/bench-runtime")),
    }
}

/// The variable a queue launch puts the pinned executable's path in; its SHA-256 is in `<variable>_SHA256`.
pub fn variable() -> String {
    format!("BENCH_RUST_{}", NAME.to_uppercase().replace('-', "_"))
}

fn entry(store: &Path, sha: &str) -> R<PathBuf> {
    if !is_sha256(sha) {
        return input(format!("{sha:?} is not a SHA-256"));
    }
    Ok(store.join(NAME).join(sha))
}

/// One writer of the store's entries and selection at a time; a second waits.
fn locked(store: &Path) -> R<nix::fcntl::Flock<fs::File>> {
    let directory = store.join(NAME);
    fs::create_dir_all(&directory).map_err(os_failure(&directory))?;
    let path = directory.join(".lock");
    let file = fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(&path)
        .map_err(os_failure(&path))?;
    nix::fcntl::Flock::lock(file, nix::fcntl::FlockArg::LockExclusive)
        .map_err(|(_, error)| Failure::Input(format!("{}: {error}", shown(&path))))
}

/// The pin of a retained entry: its executable's and its receipt's SHA-256.
fn pin_of(directory: &Path) -> R<Value> {
    let receipt_path = directory.join("receipt.json");
    let receipt = read_json(&receipt_path)?;
    let Some(sha) = field_text(&receipt, &["artifact", "sha256"]) else {
        return refused(format!(
            "{} names no artifact SHA-256",
            shown(&receipt_path)
        ));
    };
    Ok(object(vec![
        ("receipt_sha256", text(sha256_file(&receipt_path)?)),
        ("sha256", text(sha)),
    ]))
}

/// The pinned executable once it and its receipt hash to `pin` and, with `here`, it suits this host.
fn check(store: &Path, pin: &Value, here: bool) -> R<runtime::Runtime> {
    let part = |key: &str| field_text(pin, &[key]).unwrap_or("");
    runtime::check(store, NAME, part("sha256"), part("receipt_sha256"), here).map_err(checked)
}

// --- running git and cargo --------------------------------------------------------------------------------------

type Environment = BTreeMap<String, OsString>;

fn run(command: &[&OsStr], cwd: &Path, env: Option<&Environment>) -> R<String> {
    let mut process = Command::new(command[0]);
    process
        .args(&command[1..])
        .current_dir(cwd)
        .stdin(Stdio::null());
    if let Some(env) = env {
        process.env_clear().envs(env);
    }
    let done = process.output().map_err(|error| {
        Failure::Input(format!(
            "{} cannot run: {error}",
            command[0].to_string_lossy()
        ))
    })?;
    if !done.status.success() {
        let stderr = String::from_utf8_lossy(&done.stderr);
        let stderr = stderr.trim();
        let tail = &stderr[stderr.floor_char_boundary(stderr.len().saturating_sub(2000))..];
        let joined: Vec<String> = command
            .iter()
            .map(|part| part.to_string_lossy().into_owned())
            .collect();
        return input(format!(
            "{} exited {}: {tail}",
            joined.join(" "),
            done.status.code().unwrap_or(-1)
        ));
    }
    Ok(String::from_utf8_lossy(&done.stdout).into_owned())
}

fn git(checkout: &Path, arguments: &[&str]) -> R<String> {
    let mut command: Vec<&OsStr> = vec![OsStr::new("git"), OsStr::new("-C"), checkout.as_os_str()];
    command.extend(arguments.iter().map(OsStr::new));
    run(&command, Path::new(CARGO_CWD), None)
}

/// The checkout that holds the current directory.
pub fn current_checkout() -> R<PathBuf> {
    let here = std::env::current_dir()
        .map_err(|error| Failure::Input(format!("the current directory: {error}")))?;
    let top = git(&here, &["rev-parse", "--show-toplevel"])?;
    Ok(PathBuf::from(top.trim()))
}

/// Run an offline cargo command. A registry dependency missing from the host's Cargo cache names the fetch step,
/// which is the only one that needs the network.
fn offline_cargo(command: &[&OsStr], env: &Environment) -> R<String> {
    run(command, Path::new(CARGO_CWD), Some(env)).map_err(|failure| match failure {
        Failure::Input(message)
            if message
                .split_once(" exited ")
                .is_some_and(|(_, rest)| rest.contains("offline")) =>
        {
            Failure::Input(format!(
                "{message}\nrun `cargo fetch --locked` in {CRATE} to fetch its registry dependencies into the \
                 Cargo cache, then install again"
            ))
        }
        other => other,
    })
}

// --- building ---------------------------------------------------------------------------------------------------

/// The passed-through variables; a relative directory would resolve against cargo's directory, not this one.
fn build_environment() -> R<Environment> {
    let env: Environment = BUILD_ENVIRONMENT
        .iter()
        .filter_map(|name| std::env::var_os(name).map(|value| ((*name).to_string(), value)))
        .collect();
    let relative: Vec<&str> = ["CARGO_HOME", "HOME", "RUSTUP_HOME", "TMPDIR"]
        .into_iter()
        .filter(|name| {
            env.get(*name)
                .is_some_and(|value| !Path::new(value).is_absolute())
        })
        .collect();
    if !relative.is_empty() {
        return refused(format!(
            "{} must be absolute for a build whose configuration a receipt can pin",
            relative.join(", ")
        ));
    }
    Ok(env)
}

fn temporary_directory(parent: &Path, prefix: &str) -> R<PathBuf> {
    let template = parent.join(format!("{prefix}XXXXXX"));
    nix::unistd::mkdtemp(&template)
        .map_err(|error| Failure::Input(format!("{}: {error}", shown(&template))))
}

/// A Cargo home only this build reads, linking the host's registry and Git caches and the locks that guard them.
/// A missing lock is linked anyway, so cargo creates it in the host home and other cargo processes share it.
fn private_home(scratch: &Path, host: &Environment) -> R<PathBuf> {
    let shared = match (host.get("CARGO_HOME"), host.get("HOME")) {
        (Some(home), _) => PathBuf::from(home),
        (None, Some(home)) => Path::new(home).join(".cargo"),
        (None, None) => located(Path::new("~/.cargo")),
    };
    let home = temporary_directory(scratch, "cargo-home-")?;
    let caches: Vec<&str> = ["registry", "git"]
        .into_iter()
        .filter(|name| fs::symlink_metadata(shared.join(name)).is_ok())
        .collect();
    let locks: &[&str] = if caches.is_empty() {
        &[]
    } else {
        &[".package-cache", ".package-cache-mutate"]
    };
    for name in caches.iter().chain(locks) {
        std::os::unix::fs::symlink(shared.join(name), home.join(name))
            .map_err(os_failure(&home.join(name)))?;
    }
    fs::canonicalize(&home).map_err(os_failure(&home))
}

/// The build environment: `host` with the private Cargo home, and `PATH` led by the bin directory of the toolchain
/// that rustup or another proxy selects for the snapshot, chosen once here.
fn isolated_environment(source: &Path, host: &Environment) -> R<Environment> {
    let sysroot = run(
        &[
            OsStr::new("rustc"),
            OsStr::new("--print"),
            OsStr::new("sysroot"),
        ],
        source,
        Some(host),
    )?;
    let tools = Path::new(sysroot.trim()).join("bin");
    let missing: Vec<&str> = ["cargo", "rustc"]
        .into_iter()
        .filter(|name| !tools.join(name).is_file())
        .collect();
    if !missing.is_empty() {
        return input(format!(
            "the selected toolchain {} has no {}",
            shown(tools.parent().unwrap_or(&tools)),
            missing.join(" or ")
        ));
    }
    let mut env = host.clone();
    let home = source.parent().unwrap_or(source);
    env.insert("CARGO_HOME".into(), home.as_os_str().to_owned());
    let mut path = tools.into_os_string();
    if let Some(host_path) = host.get("PATH").filter(|path| !path.is_empty()) {
        path.push(":");
        path.push(host_path);
    }
    env.insert("PATH".into(), path);
    Ok(env)
}

/// Write the crate's Git tree into `destination`, so later edits to the checkout cannot reach the build; returns
/// the files written, resolved. Refuses a tracked symbolic link or submodule, whose target the tree does not hold.
fn snapshot(checkout: &Path, tree: &str, destination: &Path) -> R<BTreeSet<PathBuf>> {
    let listing = git(checkout, &["ls-tree", "-r", "-z", tree])?;
    let entries: Vec<(&str, &str)> = listing
        .split('\0')
        .filter(|row| !row.is_empty())
        .filter_map(|row| row.split_once('\t'))
        .collect();
    let others: Vec<String> = entries
        .iter()
        .filter(|(meta, _)| !meta.starts_with("100644 blob ") && !meta.starts_with("100755 blob "))
        .map(|(_, name)| format!("{CRATE}/{name}"))
        .collect();
    if !others.is_empty() {
        return refused(format!(
            "{CRATE} tracks symbolic links or submodules, whose targets its receipt cannot pin: {}",
            others.join(", ")
        ));
    }
    let mut written = BTreeSet::new();
    for (meta, name) in entries {
        let mut parts = meta.split(' ');
        let (mode, oid) = (parts.next().unwrap_or(""), parts.nth(1).unwrap_or(""));
        let blob = Command::new("git")
            .arg("-C")
            .arg(checkout)
            .args(["cat-file", "blob", oid])
            .stdin(Stdio::null())
            .output()
            .map_err(|error| Failure::Input(format!("git cannot run: {error}")))?;
        if !blob.status.success() {
            return input(format!(
                "git cat-file blob {oid} exited {}: {}",
                blob.status.code().unwrap_or(-1),
                String::from_utf8_lossy(&blob.stderr).trim()
            ));
        }
        let path = destination.join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(os_failure(parent))?;
        }
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(os_failure(&path))?;
        file.write_all(&blob.stdout).map_err(os_failure(&path))?;
        let bits = if mode == "100755" { 0o755 } else { 0o644 };
        fs::set_permissions(&path, fs::Permissions::from_mode(bits)).map_err(os_failure(&path))?;
        written.insert(fs::canonicalize(&path).map_err(os_failure(&path))?);
    }
    Ok(written)
}

/// Cargo configuration files the build would read from [`CARGO_CWD`], or that the crate tracks and the build would
/// ignore. The private Cargo home holds none.
fn cargo_configs(source: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = [Path::new(CARGO_CWD), source]
        .into_iter()
        .flat_map(|directory| {
            ["config", "config.toml"].map(|name| directory.join(".cargo").join(name))
        })
        .filter(|path| path.is_file())
        .collect();
    found.sort();
    found
}

/// The `channel` of `rust-toolchain.toml`'s `[toolchain]` table.
fn toolchain_channel(text: &str) -> Option<String> {
    let mut table = String::new();
    for line in text.lines().map(str::trim) {
        if let Some(header) = line.strip_prefix('[') {
            table = header.trim_end_matches(']').trim().to_string();
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if table == "toolchain" && key.trim() == "channel" {
            let value = value.trim();
            return value
                .strip_prefix('"')
                .and_then(|value| value.strip_suffix('"'))
                .map(str::to_string);
        }
    }
    None
}

/// The `[patch]` and `[replace]` tables of a manifest, as headers or as root-level dotted keys.
fn source_overrides(manifest: &str) -> Vec<&'static str> {
    let mut found = BTreeSet::new();
    let mut root = true;
    for line in manifest.lines().map(str::trim) {
        let key = if let Some(header) = line.strip_prefix('[') {
            root = false;
            header.trim_start_matches('[').trim()
        } else if root {
            line
        } else {
            continue;
        };
        for name in ["patch", "replace"] {
            let rest = key.strip_prefix(name);
            if rest
                .is_some_and(|rest| rest.is_empty() || rest.starts_with(['.', ']', '=', ' ', '\t']))
            {
                found.insert(name);
            }
        }
    }
    found.into_iter().collect()
}

/// The local files cargo's dep-info lists for a build; registry and Git dependencies are pinned by Cargo.lock.
fn local_inputs(dep_info: &Path) -> R<Vec<PathBuf>> {
    let content = fs::read_to_string(dep_info).map_err(|error| {
        Failure::Input(format!(
            "cargo wrote no dep-info at {}: {error}",
            shown(dep_info)
        ))
    })?;
    let mut paths = Vec::new();
    for line in content.lines() {
        if line.starts_with('#') {
            continue;
        }
        let Some((_, inputs)) = line.split_once(": ") else {
            continue;
        };
        let mut current = String::new();
        let mut characters = inputs.trim().chars().peekable();
        while let Some(character) = characters.next() {
            match character {
                '\\' if characters.peek() == Some(&' ') => {
                    current.push(' ');
                    characters.next();
                }
                ' ' => paths.push(std::mem::take(&mut current)),
                other => current.push(other),
            }
        }
        paths.push(current);
    }
    Ok(paths
        .into_iter()
        .filter(|path| !path.is_empty())
        .map(|path| located(Path::new(&path)))
        .collect())
}

fn files_under(directory: &Path, found: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        match entry.file_type() {
            Ok(kind) if kind.is_dir() => files_under(&entry.path(), found),
            Ok(kind) if kind.is_file() => found.push(entry.path()),
            _ => {}
        }
    }
}

/// The environment variables rustc's dep-info or a build script's `rerun-if-env-changed` says the build read.
fn consumed_variables(target_dir: &Path) -> BTreeSet<String> {
    let mut files = Vec::new();
    files_under(target_dir, &mut files);
    let mut names = BTreeSet::new();
    for path in files {
        let is_dep_info = path.extension().is_some_and(|extension| extension == "d");
        let is_build_output = path.file_name().is_some_and(|name| name == "output")
            && path
                .parent()
                .and_then(Path::parent)
                .and_then(Path::file_name)
                .is_some_and(|name| name == "build");
        if !is_dep_info && !is_build_output {
            continue;
        }
        let Ok(raw) = fs::read(&path) else { continue };
        for line in String::from_utf8_lossy(&raw).lines() {
            if is_dep_info && let Some(rest) = line.strip_prefix("# env-dep:") {
                names.insert(rest.split('=').next().unwrap_or("").to_string());
            }
            if is_build_output {
                let declared = line
                    .strip_prefix("cargo:rerun-if-env-changed=")
                    .or_else(|| line.strip_prefix("cargo::rerun-if-env-changed="));
                if let Some(name) = declared {
                    names.insert(name.trim().to_string());
                }
            }
        }
    }
    names
}

/// `origin`'s URL without credentials, when it is a network URL.
fn origin(checkout: &Path) -> Value {
    let url = git(checkout, &["remote", "get-url", "origin"]).unwrap_or_default();
    let url = url.trim();
    let url = match url.split_once("://") {
        Some((scheme, rest)) => match rest.split_once('/') {
            Some((authority, path)) => {
                let host = authority
                    .rsplit_once('@')
                    .map_or(authority, |(_, host)| host);
                format!("{scheme}://{host}/{path}")
            }
            None => url.to_string(),
        },
        None => url.to_string(),
    };
    let network = ["http://", "https://", "ssh://", "git://"]
        .iter()
        .any(|scheme| url.starts_with(scheme))
        || url.split_once(':').is_some_and(|(user_host, _)| {
            user_host.split_once('@').is_some_and(|(user, host)| {
                !user.is_empty()
                    && !host.is_empty()
                    && user_host
                        .chars()
                        .all(|c| c.is_alphanumeric() || "_.-@".contains(c))
            })
        });
    if network { text(url) } else { Value::Null }
}

fn cargo_metadata(env: &Environment, manifest: &Path) -> R<Value> {
    let output = offline_cargo(
        &[
            OsStr::new("cargo"),
            OsStr::new("metadata"),
            OsStr::new("--offline"),
            OsStr::new("--locked"),
            OsStr::new("--all-features"),
            OsStr::new("--format-version"),
            OsStr::new("1"),
            OsStr::new("--manifest-path"),
            manifest.as_os_str(),
        ],
        env,
    )?;
    pyjson::loads(&output, Duplicates::KeepLast)
        .map_err(|error| Failure::Input(format!("cargo metadata printed no JSON: {error:?}")))
}

struct Described {
    identity: Vec<(&'static str, Value)>,
    bin: String,
    target: String,
    tracked: BTreeSet<PathBuf>,
    env: Environment,
}

/// The committed source and the toolchain a build would use, the files of the crate's snapshot, and the build's
/// environment. Writes the crate tree at `HEAD` into `source`, directly inside the private Cargo home, then checks
/// and describes that snapshot, so the receipt names exactly what cargo compiles.
fn describe(checkout: &Path, source: &Path, host: &Environment) -> R<Described> {
    let dirty = git(
        checkout,
        &[
            "status",
            "--porcelain",
            "--untracked-files=all",
            "--",
            CRATE,
        ],
    )?;
    if !dirty.trim().is_empty() {
        return refused(format!(
            "{CRATE} has uncommitted or untracked files:\n{}",
            dirty.trim_end()
        ));
    }
    let revision = git(checkout, &["rev-parse", "HEAD"])?.trim().to_string();
    let tree = git(checkout, &["rev-parse", &format!("{revision}:{CRATE}")])?
        .trim()
        .to_string();
    let tracked = snapshot(checkout, &tree, source)?;
    let configs = cargo_configs(source);
    if !configs.is_empty() {
        let names: Vec<String> = configs.iter().map(|path| shown(path)).collect();
        return refused(format!(
            "cargo configuration a receipt cannot restore would apply: {}",
            names.join(", ")
        ));
    }
    let untracked: Vec<&str> = ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml"]
        .into_iter()
        .filter(|name| {
            fs::canonicalize(source.join(name)).map_or(true, |path| !tracked.contains(&path))
        })
        .collect();
    if !untracked.is_empty() {
        return refused(format!(
            "{CRATE} does not track {}, so its crate tree does not hold them",
            untracked.join(", ")
        ));
    }
    let read = |name: &str| fs::read_to_string(source.join(name)).unwrap_or_default();
    let Some(channel) = toolchain_channel(&read("rust-toolchain.toml")) else {
        return input(format!(
            "{CRATE}/rust-toolchain.toml needs a [toolchain] channel"
        ));
    };
    let overrides = source_overrides(&read("Cargo.toml"));
    if !overrides.is_empty() {
        return refused(format!(
            "{CRATE}/Cargo.toml has [{}] sections, whose sources its receipt cannot pin",
            overrides.join("], [")
        ));
    }
    let env = isolated_environment(source, host)?;
    for directory in [checkout.join(CRATE), source.to_path_buf()] {
        let manifest = directory.join("Cargo.toml");
        let workspace = run(
            &[
                OsStr::new("cargo"),
                OsStr::new("locate-project"),
                OsStr::new("--workspace"),
                OsStr::new("--message-format"),
                OsStr::new("plain"),
                OsStr::new("--offline"),
                OsStr::new("--manifest-path"),
                manifest.as_os_str(),
            ],
            Path::new(CARGO_CWD),
            Some(&env),
        )?;
        let workspace = PathBuf::from(workspace.trim());
        if located(&workspace) != located(&manifest) {
            return refused(format!(
                "{CRATE} belongs to the Cargo workspace {}, whose manifest and lockfile its receipt cannot pin",
                shown(&workspace)
            ));
        }
    }
    let manifest = source.join("Cargo.toml");
    let metadata = cargo_metadata(&env, &manifest)?;
    let packages = field(&metadata, &["packages"])
        .and_then(Value::as_array)
        .unwrap_or_default();
    let manifest_of = |package: &Value| {
        field_text(package, &["manifest_path"]).map(|path| located(Path::new(path)))
    };
    let local: Vec<String> = packages
        .iter()
        .filter(|package| matches!(field(package, &["source"]), None | Some(Value::Null)))
        .filter_map(manifest_of)
        .filter(|path| !tracked.contains(path))
        .map(|path| shown(&path))
        .collect();
    if !local.is_empty() {
        return refused(format!(
            "{CRATE} depends on local packages whose manifests it does not track: {}",
            local.join(", ")
        ));
    }
    let own = located(&manifest);
    let Some(package) = packages
        .iter()
        .find(|package| manifest_of(package).as_ref() == Some(&own))
    else {
        return input(format!("cargo metadata does not list {CRATE}'s package"));
    };
    let name = field_text(package, &["name"]).unwrap_or("").to_string();
    if name != NAME {
        return refused(format!("{CRATE} is the package {name}, not {NAME}"));
    }
    let bin = field(package, &["targets"])
        .and_then(Value::as_array)
        .unwrap_or_default()
        .iter()
        .find(|target| {
            field(target, &["kind"])
                .and_then(Value::as_array)
                .is_some_and(|kinds| kinds.iter().any(|kind| kind.as_str() == Some("bin")))
        })
        .and_then(|target| field_text(target, &["name"]))
        .unwrap_or(NAME)
        .to_string();
    let rustc_text = run(
        &[OsStr::new("rustc"), OsStr::new("-vV")],
        Path::new(CARGO_CWD),
        Some(&env),
    )?;
    let mut rustc = Object::default();
    for line in rustc_text.lines() {
        if let Some((key, value)) = line.split_once(": ") {
            rustc.insert(key.to_string(), text(value));
        }
    }
    let release = rustc
        .get("release")
        .and_then(Value::as_str)
        .unwrap_or("None");
    if release != channel {
        return refused(format!(
            "the active rustc is {release}; {CRATE}/rust-toolchain.toml pins {channel}"
        ));
    }
    let host_triple = rustc.get("host").and_then(Value::as_str).unwrap_or("None");
    let Some(target) = runtime::host_target().filter(|target| *target == host_triple) else {
        return refused(format!(
            "this host ({host_triple}) is not a supported Rust runtime target ({})",
            runtime::SUPPORTED_TARGETS.join(", ")
        ));
    };
    let cargo_version = run(
        &[OsStr::new("cargo"), OsStr::new("-V")],
        Path::new(CARGO_CWD),
        Some(&env),
    )?;
    let (libc, libc_version) = runtime::host_libc();
    let optional = |value: Option<String>| value.map_or(Value::Null, text);
    let identity = vec![
        (
            "crate",
            object(vec![
                ("name", text(&name)),
                (
                    "version",
                    field(package, &["version"]).cloned().unwrap_or(Value::Null),
                ),
                ("path", text(CRATE)),
                ("bin", text(&bin)),
            ]),
        ),
        (
            "source",
            object(vec![
                ("repository", origin(checkout)),
                ("revision", text(&revision)),
                ("tree", text(&tree)),
                (
                    "cargo_toml_sha256",
                    text(sha256_file(&source.join("Cargo.toml"))?),
                ),
                (
                    "cargo_lock_sha256",
                    text(sha256_file(&source.join("Cargo.lock"))?),
                ),
                (
                    "toolchain_file_sha256",
                    text(sha256_file(&source.join("rust-toolchain.toml"))?),
                ),
            ]),
        ),
        (
            "toolchain",
            object(vec![
                ("channel", text(&channel)),
                ("rustc", Value::Object(rustc)),
                ("cargo", text(cargo_version.trim())),
            ]),
        ),
        ("target", text(target)),
        (
            "libc",
            object(vec![
                ("name", optional(libc)),
                ("version", optional(libc_version)),
            ]),
        ),
    ];
    Ok(Described {
        identity,
        bin,
        target: target.to_string(),
        tracked,
        env,
    })
}

/// Build the crate's binary from a snapshot of its committed tree in `scratch`; returns the executable and its
/// receipt, without retaining it.
fn build(checkout: &Path, scratch: &Path, host: &Environment) -> R<(PathBuf, Value)> {
    let home = private_home(scratch, host)?;
    let source = home.join("source");
    let target_dir = scratch.join("target");
    let Described {
        identity,
        bin,
        target,
        tracked,
        mut env,
    } = describe(checkout, &source, host)?;
    // The snapshot lies inside the Cargo home, and rustc applies the last matching prefix.
    let flags = [
        format!("--remap-path-prefix={}=/cargo", shown(&home)),
        format!("--remap-path-prefix={}=/source", shown(&source)),
    ];
    let command = [
        "cargo",
        "build",
        "--locked",
        "--offline",
        "--release",
        "--target",
        &target,
        "--bin",
        &bin,
    ];
    let passed: Vec<Value> = env.keys().map(text).collect();
    env.insert(
        "CARGO_TARGET_DIR".into(),
        target_dir.clone().into_os_string(),
    );
    env.insert("CARGO_ENCODED_RUSTFLAGS".into(), flags.join("\x1f").into());
    let manifest = source.join("Cargo.toml");
    let mut invocation: Vec<&OsStr> = command.iter().map(OsStr::new).collect();
    invocation.extend([OsStr::new("--manifest-path"), manifest.as_os_str()]);
    offline_cargo(&invocation, &env)?;
    let executable = target_dir.join(&target).join("release").join(&bin);
    let read: Vec<String> = consumed_variables(&target_dir)
        .into_iter()
        .filter(|name| env.contains_key(name))
        .collect();
    if !read.is_empty() {
        return refused(format!(
            "the build read host environment variables its receipt cannot pin: {}",
            read.join(", ")
        ));
    }
    let built = located(&target_dir);
    let mut dep_info = executable.clone().into_os_string();
    dep_info.push(".d");
    let unpinned: Vec<String> = local_inputs(Path::new(&dep_info))?
        .into_iter()
        .filter(|path| !path.starts_with(&built) && !tracked.contains(path))
        .map(|path| shown(&path))
        .collect();
    if !unpinned.is_empty() {
        return refused(format!(
            "the build read local files that are not tracked in {CRATE}, which its receipt cannot pin: {}",
            unpinned.join(", ")
        ));
    }
    let size = fs::metadata(&executable)
        .map_err(os_failure(&executable))?
        .len();
    let mut receipt = vec![("schema", text(runtime::RECEIPT))];
    receipt.extend(identity);
    receipt.extend([
        (
            "build",
            object(vec![
                (
                    "command",
                    Value::Array(command.iter().map(|part| text(*part)).collect()),
                ),
                (
                    "rustflags",
                    Value::Array(vec![
                        text("--remap-path-prefix=<cargo home>=/cargo"),
                        text("--remap-path-prefix=<crate>=/source"),
                    ]),
                ),
                ("environment", Value::Array(passed)),
                ("cargo_config_files", Value::Array(Vec::new())),
            ]),
        ),
        (
            "artifact",
            object(vec![
                ("file", text(&bin)),
                ("sha256", text(sha256_file(&executable)?)),
                ("size", Value::Int(size.to_string())),
            ]),
        ),
        ("built_at", text(now())),
    ]);
    Ok((executable, object(receipt)))
}

// --- retaining --------------------------------------------------------------------------------------------------

fn same_build(left: &Value, right: &Value) -> bool {
    [
        "crate",
        "source",
        "toolchain",
        "target",
        "build",
        "artifact",
    ]
    .iter()
    .all(|key| {
        let side = |value: &Value| field(value, &[key]).cloned().unwrap_or(Value::Null);
        side(left).py_eq(&side(right))
    })
}

/// Place an executable and its receipt in the store; an existing entry is kept only when it records the same build.
/// Returns the entry and whether it was added.
fn retain(store: &Path, executable: &Path, receipt: &Value) -> R<(PathBuf, bool)> {
    if let Some(problem) = runtime::receipt_problem(receipt) {
        return refused(format!("the receipt is incomplete: {problem}"));
    }
    let name = field_text(receipt, &["crate", "name"]).unwrap_or("");
    if name != NAME {
        return refused(format!("the receipt describes {name}, not {NAME}"));
    }
    let digest = field_text(receipt, &["artifact", "sha256"]).unwrap_or("");
    let file = field_text(receipt, &["artifact", "file"]).unwrap_or("");
    if sha256_file(executable)? != digest {
        return refused(format!(
            "{} does not hash to the receipt's {digest}",
            shown(executable)
        ));
    }
    let _lock = locked(store)?;
    let target = entry(store, digest)?;
    if fs::symlink_metadata(&target).is_ok() {
        let kept = read_json(&target.join("receipt.json"))?;
        if !same_build(&kept, receipt) {
            return refused(format!(
                "{} already retains this executable with another receipt; entries are never replaced",
                shown(&target)
            ));
        }
        check(store, &pin_of(&target)?, false)?;
        return Ok((target, false));
    }
    let staging = partial_name(&store.join(NAME).join("entry"), "partial");
    fs::create_dir(&staging).map_err(os_failure(&staging))?;
    let placed = (|| {
        let copy = staging.join(file);
        fs::copy(executable, &copy).map_err(os_failure(&copy))?;
        fs::set_permissions(&copy, fs::Permissions::from_mode(0o555)).map_err(os_failure(&copy))?;
        let receipt_path = staging.join("receipt.json");
        publish(&receipt_path, &dumps(receipt), false)?;
        fs::set_permissions(&receipt_path, fs::Permissions::from_mode(0o444))
            .map_err(os_failure(&receipt_path))?;
        fs::rename(&staging, &target).map_err(os_failure(&target))
    })();
    if placed.is_err() {
        let _ = fs::remove_dir_all(&staging);
    }
    placed?;
    check(store, &pin_of(&target)?, false)?;
    Ok((target, true))
}

/// Point `launcher` at the selected entry when its executable runs these commands, so `bench/tools/bench-tools`
/// resolves with it. A selected entry from before them keeps the previous launcher, which resolves to it. An entry
/// that is only retained never becomes the launcher, so it never handles a frozen run's commands.
fn adopt_launcher(store: &Path, directory: &Path, file: &str) -> R<bool> {
    let executable = directory.join(file);
    let capable = Command::new(&executable)
        .args(["runtime", "--help"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success());
    if !capable {
        return Ok(false);
    }
    let sha = directory.file_name().unwrap_or_default();
    let link = store.join(NAME).join(LAUNCHER);
    let partial = partial_name(&link, "link");
    std::os::unix::fs::symlink(Path::new(sha).join(file), &partial)
        .map_err(os_failure(&partial))?;
    fs::rename(&partial, &link).map_err(|error| {
        let _ = fs::remove_file(&partial);
        os_failure(&link)(error)
    })?;
    Ok(true)
}

// --- selection --------------------------------------------------------------------------------------------------

fn selected(store: &Path) -> R<Option<Value>> {
    let path = store.join(NAME).join("selected.json");
    if path.is_file() {
        read_json(&path).map(Some)
    } else {
        Ok(None)
    }
}

/// Record `pin` as the selection and move the launcher to it; the caller holds the store's lock.
fn publish_selection(store: &Path, pin: &Value, action: &str) -> R<Value> {
    let found = check(store, pin, true)?;
    let directory = entry(store, field_text(pin, &["sha256"]).unwrap_or(""))?;
    let file = field_text(&found.receipt, &["artifact", "file"]).unwrap_or(NAME);
    let row = record_selection(store, pin, action)?;
    adopt_launcher(store, &directory, file)?;
    Ok(row)
}

fn record_selection(store: &Path, pin: &Value, action: &str) -> R<Value> {
    let current = selected(store)?;
    let previous = current
        .as_ref()
        .and_then(|current| field(current, &["pin"]))
        .cloned()
        .unwrap_or(Value::Null);
    if previous.py_eq(pin)
        && let Some(current) = current
    {
        return Ok(current);
    }
    let row = object(vec![
        ("action", text(action)),
        ("at", text(now())),
        ("pin", pin.clone()),
        ("previous", previous),
    ]);
    let history_path = store.join(NAME).join("selections.jsonl");
    let line = pyjson::dumps(
        &row,
        &Dumps {
            sort_keys: true,
            ..Dumps::default()
        },
    ) + "\n";
    let mut history = fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(&history_path)
        .map_err(os_failure(&history_path))?;
    history
        .write_all(line.as_bytes())
        .and_then(|()| history.sync_all())
        .map_err(os_failure(&history_path))?;
    publish(&store.join(NAME).join("selected.json"), &dumps(&row), false)?;
    Ok(row)
}

/// Make `sha` the version new operations take. Operations holding a pin are unaffected.
pub fn select(store: &Path, sha: &str) -> R<Value> {
    let _lock = locked(store)?;
    publish_selection(store, &pin_of(&entry(store, sha)?)?, "select")
}

/// Select the version the current selection replaced, reading and writing under one lock.
pub fn rollback(store: &Path) -> R<Value> {
    let _lock = locked(store)?;
    let previous = selected(store)?
        .and_then(|current| field(&current, &["previous"]).cloned())
        .filter(Value::truthy);
    let Some(previous) = previous else {
        return refused(format!(
            "{NAME} has no previously selected version to return to"
        ));
    };
    publish_selection(store, &previous, "rollback")
}

pub fn current_pin(store: &Path) -> R<Value> {
    let Some(current) = selected(store)? else {
        return refused(format!(
            "{NAME} has no selected version in {}",
            shown(store)
        ));
    };
    let pin = field(&current, &["pin"]).cloned().unwrap_or(Value::Null);
    check(store, &pin, true)?;
    Ok(pin)
}

// --- resolving --------------------------------------------------------------------------------------------------

/// The executable an operation runs: the launch's `BENCH_RUST_BENCH_TOOLS` with its matching SHA-256; else the pin in
/// `run`'s `runtimes.frozen.json`; else the selected version, only while its crate tree is the one at `checkout`'s
/// `HEAD`. Refuses anything else rather than falling back to another version or implementation.
pub fn resolve(store: &Path, checkout: &Path, run: Option<&Path>) -> R<PathBuf> {
    let name = variable();
    let path = std::env::var_os(&name);
    let digest = std::env::var_os(format!("{name}_SHA256"));
    if path.is_some() || digest.is_some() {
        let (Some(path), Some(digest)) = (path.filter(|path| !path.is_empty()), digest) else {
            return refused(format!("{name} and {name}_SHA256 must be set together"));
        };
        let executable = PathBuf::from(&path);
        let regular = fs::symlink_metadata(&executable).is_ok_and(|found| found.is_file());
        if !executable.is_absolute() || !regular {
            return refused(format!(
                "{name} is {}, not the absolute path of an executable file",
                shown(&executable)
            ));
        }
        if sha256_file(&executable)?.as_str() != digest {
            return refused(format!(
                "{} does not hash to {name}_SHA256 {}",
                shown(&executable),
                digest.to_string_lossy()
            ));
        }
        if !nix::unistd::access(&executable, nix::unistd::AccessFlags::X_OK).is_ok() {
            return refused(format!("{} is not executable", shown(&executable)));
        }
        return Ok(executable);
    }
    if let Some(run) = run {
        return for_run(store, run);
    }
    let pin = current_pin(store).map_err(|failure| match failure {
        Failure::Refused(why) => Failure::Refused(format!("{why}; install it with: {INSTALL}")),
        other => other,
    })?;
    let found = check(store, &pin, true)?;
    let path = field_text(&found.receipt, &["crate", "path"]).unwrap_or("");
    let built = field_text(&found.receipt, &["source", "tree"]).unwrap_or("");
    let tree = runtime::crate_tree(checkout, "HEAD", path);
    if tree.as_deref() != Some(built) {
        return refused(format!(
            "the selected {NAME} was built from crate tree {built}; HEAD has {}; install it with: {INSTALL}",
            tree.unwrap_or_else(|| format!("no crate {path}"))
        ));
    }
    Ok(found.executable)
}

/// A run's own operation runs its `runtimes.frozen.json` pin, as its runtime deviations moved it, checked as a queue
/// checks it, and never the selection. A run that pins none was frozen for the Python implementation, so it is
/// refused with the commit to run it from.
fn for_run(store: &Path, run: &Path) -> R<PathBuf> {
    let pins = runtime::run_pins(run).map_err(Failure::Input)?.pins;
    let Some(pin) = field(&pins, &[NAME]) else {
        let manifest = run.join("manifest.json");
        let commit = if manifest.is_file() {
            read_json(&manifest)
                .ok()
                .and_then(|manifest| field_text(&manifest, &["freeze_commit"]).map(str::to_string))
        } else {
            None
        };
        return refused(format!(
            "run {} pins no {NAME} runtime; run it from a checkout of its freeze commit {}",
            run.file_name().unwrap_or_default().to_string_lossy(),
            commit
                .filter(|commit| !commit.is_empty())
                .as_deref()
                .unwrap_or("unknown")
        ));
    };
    Ok(check(store, pin, true)?.executable)
}

// --- deviations -------------------------------------------------------------------------------------------------

pub struct Notes<'a> {
    pub reason: &'a str,
    pub scope: &'a str,
    pub validation: &'a str,
    pub accounting: &'a str,
}

/// Record a runtime change with both identities. The run's operations then take the new pin: they read
/// `runtimes.frozen.json` through [`runtime::run_pins`], which follows these records.
pub fn deviation(store: &Path, run: &Path, old: &str, new: &str, notes: &Notes<'_>) -> R<PathBuf> {
    let statements = [
        ("accounting", notes.accounting),
        ("reason", notes.reason),
        ("scope", notes.scope),
        ("validation", notes.validation),
    ];
    let blank: Vec<&str> = statements
        .iter()
        .filter(|(_, value)| value.trim().is_empty())
        .map(|(key, _)| *key)
        .collect();
    if !blank.is_empty() {
        return input(format!("a runtime deviation states {}", blank.join(", ")));
    }
    if old == new {
        return input("a runtime deviation names two different versions");
    }
    if !run.join("manifest.json").is_file() {
        return input(format!("{} is not a run directory", shown(run)));
    }
    let current = runtime::run_pins(run).map_err(Failure::Input)?.pins;
    let current = field_text(&current, &[NAME, "sha256"]).unwrap_or("none");
    if current != old {
        return input(format!(
            "run {} now pins {NAME} {current}, not {old}",
            shown(run)
        ));
    }
    let mut record = vec![
        ("schema", text(DEVIATION)),
        ("version", Value::Int("1".into())),
        ("runtime", text(NAME)),
        ("recorded_at", text(now())),
    ];
    for (side, digest) in [("old", old), ("new", new)] {
        let directory = entry(store, digest)?;
        record.push((
            side,
            object(vec![
                ("pin", pin_of(&directory)?),
                ("receipt", read_json(&directory.join("receipt.json"))?),
            ]),
        ));
    }
    record.extend(statements.map(|(key, value)| (key, text(value))));
    let path = run.join("deviations").join(format!(
        "runtime-{NAME}-{}-to-{}.v1.json",
        &old[..12],
        &new[..12]
    ));
    let parent = run.join("deviations");
    if !parent.is_dir() {
        fs::create_dir(&parent).map_err(os_failure(&parent))?;
    }
    publish(&path, &dumps(&object(record)), true)?;
    Ok(path)
}

// --- commands ---------------------------------------------------------------------------------------------------

/// `install`: build the checkout's crate, retain it and, with `select`, make it the selection and the launcher.
pub fn install(store: &Path, checkout: &Path, select_it: bool) -> R<Value> {
    let host = build_environment()?;
    let scratch = temporary_directory(&std::env::temp_dir(), "rust-runtime-build-")?;
    let built = build(checkout, &scratch, &host).and_then(|(executable, receipt)| {
        retain(store, &executable, &receipt).map(|retained| (retained, receipt))
    });
    let _ = fs::remove_dir_all(&scratch);
    let ((directory, added), receipt) = built?;
    let pin = pin_of(&directory)?;
    let mut result = vec![
        ("retained", text(shown(&directory))),
        ("added", Value::Bool(added)),
        ("pin", pin.clone()),
    ];
    if select_it {
        let sha = field_text(&pin, &["sha256"]).unwrap_or("");
        result.push(("selected", select(store, sha)?));
    }
    let file = field_text(&receipt, &["artifact", "file"]).unwrap_or(NAME);
    let launcher = directory.file_name().is_some_and(|sha| {
        fs::read_link(store.join(NAME).join(LAUNCHER))
            .is_ok_and(|target| target == Path::new(sha).join(file))
    });
    result.push(("launcher", Value::Bool(launcher)));
    Ok(object(result))
}

/// `restore`: retain an executable from elsewhere, such as another store or a rebuild, only when it hashes to the
/// receipt.
pub fn restore(store: &Path, receipt: &Path, executable: &Path) -> R<Value> {
    let receipt = read_json(receipt)?;
    let (directory, added) = retain(store, executable, &receipt)?;
    Ok(object(vec![
        ("retained", text(shown(&directory))),
        ("added", Value::Bool(added)),
        ("pin", pin_of(&directory)?),
    ]))
}

pub fn verify(store: &Path, sha: &str) -> R<Value> {
    let directory = entry(store, sha)?;
    let pin = pin_of(&directory)?;
    let found = check(store, &pin, true)?;
    Ok(object(vec![
        ("verified", text(shown(&found.executable))),
        ("pin", pin),
    ]))
}

pub fn pin(store: &Path) -> R<Value> {
    Ok(object(vec![(NAME, current_pin(store)?)]))
}

pub fn list(store: &Path) -> R<Value> {
    let directory = store.join(NAME);
    let mut versions: Vec<String> = fs::read_dir(&directory)
        .map(|entries| {
            entries
                .flatten()
                .filter_map(|entry| entry.file_name().into_string().ok())
                .filter(|name| is_sha256(name))
                .collect()
        })
        .unwrap_or_default();
    versions.sort();
    Ok(object(vec![
        (
            "versions",
            Value::Array(versions.into_iter().map(text).collect()),
        ),
        ("selected", selected(store)?.unwrap_or(Value::Null)),
    ]))
}

/// `exec`: replace this process with the resolved executable, so its exit code and output are the command's.
pub fn exec(store: &Path, checkout: &Path, run: Option<&Path>, arguments: &[OsString]) -> Failure {
    use std::os::unix::process::CommandExt;
    match resolve(store, checkout, run) {
        Ok(executable) => {
            let error = Command::new(&executable).args(arguments).exec();
            Failure::Input(format!("{} cannot run: {error}", shown(&executable)))
        }
        Err(failure) => failure,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_toolchain_channel_is_read_from_its_table() {
        let file = "[toolchain]\nchannel = \"1.99.1\"\nprofile = \"minimal\"\n";
        assert_eq!(toolchain_channel(file).as_deref(), Some("1.99.1"));
        assert_eq!(toolchain_channel("[other]\nchannel = \"1\"\n"), None);
    }

    #[test]
    fn patch_and_replace_are_found_as_tables_or_root_keys() {
        assert_eq!(
            source_overrides(
                "[package]\nname = \"x\"\n[patch.crates-io]\nfoo = { path = \"f\" }\n"
            ),
            ["patch"]
        );
        assert_eq!(
            source_overrides("replace = {}\n[package]\npatches = 1\n"),
            ["replace"]
        );
        assert_eq!(
            source_overrides("[package]\nname = \"patch\"\n[dependencies]\npatchy = \"1\"\n"),
            [] as [&str; 0]
        );
    }

    #[test]
    fn dep_info_paths_keep_escaped_spaces() {
        let directory = std::env::temp_dir().join(format!("dep-info-{}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        let file = directory.join("x.d");
        fs::write(
            &file,
            "/a/out: /src/a\\ b.rs /src/c.rs\n# env-dep:X=1\n\n/src/a\\ b.rs:\n",
        )
        .unwrap();
        let found = local_inputs(&file).unwrap();
        fs::remove_dir_all(&directory).unwrap();
        assert_eq!(
            found,
            [PathBuf::from("/src/a b.rs"), PathBuf::from("/src/c.rs")]
        );
    }
}
