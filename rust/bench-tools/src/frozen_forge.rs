//! Serve a task's pull request to a reviewer as the forge showed it at the task's cutoff.
//!
//! `setup` gives one attempt a frozen forge under `<attempt>/forge/`: the task's pinned `graphql.json` and
//! `issues.json` (never `provenance.json`, which records later values), a stand-in `gh` in `forge/bin`, and
//! `forge/origin.git`, a copy of the truncated mirror shaped as the upstream repository stood at the cutoff:
//! the base branch at the merge base, `refs/pull/<n>/head` at the reviewed head, and the head branch when the
//! pull request is not from a fork. The copy borrows its objects from the clone, which holds exactly the
//! mirror's. The clone's `origin` then points at that copy, its remote-tracking refs are
//! fetched from it, `review-head` tracks `refs/pull/<n>/head` as `gh pr checkout` leaves it, and the local base
//! branch tracks the upstream base. The clone's `.git/FETCH_HEAD` points into the attempt's work directory, so a
//! fetch from `origin` works where the clone is read-only, as under `claude-strict-v2` and in Codex's sandbox. A
//! pre-receive hook refuses every push. `forge.json` in the attempt records the pins and the shape.
//!
//! `gh` answers the calls the frozen skills and built-in reviews make (see `bench/frozen-forge-inventory.v1.json`)
//! from the capture and the copy; a write or any other call fails with a message that names the frozen forge.
//! Every `gh` call, and every upload-pack that serves a fetch from `origin`, appends one line to
//! `<attempt>/forge-log/calls.jsonl`; a call whose line cannot be written fails.
//!
//! `probe` checks a target's frozen forge without a model: it clones the mirror into a scratch attempt, sets up
//! the forge, and runs each inventoried call of an arm kind in that kind's environment.

pub mod gh;
pub mod graphql;
pub mod probe;

use crate::build_packet::{
    BASE_MOVED_BEFORE_MERGE, MERGE_BASE_ROW, MERGED_ROW, NOT_MERGED, STATE_ROW, THREAD_RESOLVED,
    THREAD_STATE_UNKNOWN, THREAD_UNRESOLVED,
};
use crate::provision::{self, Target};
use crate::pyjson::{self, Dumps, Duplicates, Value};
use crate::{sha256, shlex};
use std::fs;
use std::io::Write as _;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub const SERVED: [&str; 2] = ["graphql.json", "issues.json"];
pub const RECEIPT: &str = "forge.json";
pub const LOG: &str = "forge-log/calls.jsonl";
const CONFIG: &str = "serve.json";

type R<T> = Result<T, String>;

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

fn object<const N: usize>(entries: [(&str, Value); N]) -> Value {
    Value::Object(
        entries
            .into_iter()
            .map(|(key, value)| (key.to_string(), value))
            .collect(),
    )
}

fn without(value: &Value, key: &str) -> Value {
    match value {
        Value::Object(record) => Value::Object(
            record
                .iter()
                .filter(|(name, _)| name.as_str() != key)
                .map(|(name, child)| (name.clone(), child.clone()))
                .collect(),
        ),
        other => other.clone(),
    }
}

fn display(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn read_json(path: &Path) -> R<Value> {
    let raw = fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    pyjson::loads(&raw, Duplicates::KeepLast)
        .map_err(|error| format!("{}: {error}", path.display()))
}

fn indented(value: &Value) -> String {
    pyjson::dumps(
        value,
        &Dumps {
            indent: Some(2),
            ..Dumps::default()
        },
    ) + "\n"
}

/// Compact JSON as the forge's API writes it.
pub fn compact(value: &Value) -> String {
    pyjson::dumps(
        value,
        &Dumps {
            ensure_ascii: false,
            separators: Some((",".to_string(), ":".to_string())),
            ..Dumps::default()
        },
    )
}

fn sha256_file(path: &Path) -> R<String> {
    Ok(sha256::hex_digest(
        &fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?,
    ))
}

fn write_file(path: &Path, content: &str, mode: u32) -> R<()> {
    fs::write(path, content).map_err(|error| format!("{}: {error}", path.display()))?;
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
        .map_err(|error| format!("{}: {error}", path.display()))
}

fn git(arguments: &[&str]) -> R<String> {
    let done = Command::new("git")
        .args(arguments)
        .stdin(Stdio::null())
        .output()
        .map_err(|error| format!("cannot run git: {error}"))?;
    if !done.status.success() {
        return Err(format!(
            "git {} failed: {}",
            arguments.join(" "),
            String::from_utf8_lossy(&done.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&done.stdout).into_owned())
}

/// A task's pull request and issues at the cutoff, as `graphql.json` and `issues.json` hold them.
pub struct Capture {
    pub repository: String,
    pub number: u64,
    /// The pull request, with the head commit's checks merged into the last of its commits.
    pub pull: Value,
    /// `repository` without its `pullRequest`.
    pub repository_record: Value,
    /// Each captured issue with its repository: the originating issues, then the closing references.
    issues: Vec<(String, Value)>,
}

impl Capture {
    pub fn load(graphql: &Value, issues: &Value) -> R<Self> {
        let repository = get(get(graphql, "data"), "repository");
        let repository_record = without(repository, "pullRequest");
        let pull = get(repository, "pullRequest");
        let slug = get(repository, "nameWithOwner")
            .as_str()
            .ok_or("graphql.json names no repository")?
            .to_string();
        let number = match get(pull, "number") {
            Value::Int(digits) => digits
                .parse()
                .map_err(|_| "graphql.json has an invalid pull request number")?,
            _ => return Err("graphql.json holds no pull request".to_string()),
        };
        let head_checks = get(get(pull, "headCommit"), "nodes")
            .as_array()
            .and_then(<[Value]>::last)
            .map(|node| get(node, "commit").clone());
        let mut pull = without(pull, "headCommit");
        if let Value::Object(record) = &mut pull
            && let (Some(checks), Some(Value::Object(commits))) =
                (head_checks, record.get_mut("commits"))
            && let Some(Value::Array(nodes)) = commits.get_mut("nodes")
            && let Some(Value::Object(last)) = nodes.last_mut()
            && let Some(Value::Object(commit)) = last.get_mut("commit")
            && commit
                .get("oid")
                .is_some_and(|oid| oid.py_eq(get(&checks, "oid")))
            && let Some(checks) = checks.as_object()
        {
            for (key, value) in checks.iter() {
                commit.insert(key.clone(), value.clone());
            }
        }
        let mut found = Vec::new();
        for entry in get(issues, "issues").as_array().unwrap_or_default() {
            found.push((
                get(entry, "repository").py_str(),
                get(entry, "issue").clone(),
            ));
        }
        for issue in get(get(&pull, "closingIssuesReferences"), "nodes")
            .as_array()
            .unwrap_or_default()
        {
            let owner = get(get(issue, "repository"), "nameWithOwner")
                .as_str()
                .map_or_else(|| slug.clone(), str::to_string);
            found.push((owner, issue.clone()));
        }
        Ok(Self {
            repository: slug,
            number,
            pull,
            repository_record,
            issues: found,
        })
    }

    pub fn has_issue_repository(&self, slug: &str) -> bool {
        self.issues
            .iter()
            .any(|(owner, _)| owner.eq_ignore_ascii_case(slug))
    }

    pub fn issue(&self, slug: &str, number: u64) -> Option<&Value> {
        self.issues
            .iter()
            .find(|(owner, issue)| {
                owner.eq_ignore_ascii_case(slug)
                    && get(issue, "number").py_str() == number.to_string()
            })
            .map(|(_, issue)| issue)
    }

    /// The captured record with GraphQL id `id`, and its type.
    pub fn node(&self, id: &str) -> Option<(&Value, Option<&'static str>)> {
        fn walk<'a>(
            value: &'a Value,
            kind: Option<&'static str>,
            id: &str,
        ) -> Option<(&'a Value, Option<&'static str>)> {
            match value {
                Value::Object(record) => {
                    if record.get("id").and_then(Value::as_str) == Some(id) {
                        return Some((value, kind));
                    }
                    record.iter().find_map(|(key, child)| {
                        let below = if key == "nodes" {
                            kind
                        } else {
                            graphql::child_type(kind, key)
                        };
                        walk(child, below, id)
                    })
                }
                Value::Array(items) => items.iter().find_map(|item| walk(item, kind, id)),
                _ => None,
            }
        }
        walk(&self.pull, Some("PullRequest"), id).or_else(|| {
            self.issues
                .iter()
                .find_map(|(_, issue)| walk(issue, Some("Issue"), id))
        })
    }
}

/// What `gh` serves in one attempt: `forge/serve.json` and the capture beside it.
pub struct Served {
    pub root: PathBuf,
    pub capture: Capture,
    pub config: Value,
}

impl Served {
    pub fn open(root: &Path) -> R<Self> {
        let config = read_json(&root.join(CONFIG))?;
        let capture = Capture::load(
            &read_json(&root.join("graphql.json"))?,
            &read_json(&root.join("issues.json"))?,
        )?;
        Ok(Self {
            root: root.to_path_buf(),
            capture,
            config,
        })
    }

    pub fn field(&self, key: &str) -> String {
        get(&self.config, key).py_str()
    }

    pub fn origin(&self) -> PathBuf {
        self.root.join("origin.git")
    }

    pub fn log(&self) -> PathBuf {
        PathBuf::from(self.field("log"))
    }

    /// Run git against the frozen upstream copy.
    pub fn origin_git(&self, arguments: &[&str]) -> R<Vec<u8>> {
        let done = Command::new("git")
            .arg("--git-dir")
            .arg(self.origin())
            .args(arguments)
            .stdin(Stdio::null())
            .output()
            .map_err(|error| format!("cannot run git: {error}"))?;
        if !done.status.success() {
            return Err(format!(
                "git {} failed: {}",
                arguments.join(" "),
                String::from_utf8_lossy(&done.stderr).trim()
            ));
        }
        Ok(done.stdout)
    }
}

/// Append one call to the attempt's forge log.
pub fn log_call(log: &Path, entry: &Value) -> R<()> {
    let mut file = fs::OpenOptions::new()
        .append(true)
        .open(log)
        .map_err(|error| {
            format!(
                "cannot write the frozen forge log {}: {error}",
                log.display()
            )
        })?;
    file.write_all(format!("{}\n", compact(entry)).as_bytes())
        .map_err(|error| {
            format!(
                "cannot write the frozen forge log {}: {error}",
                log.display()
            )
        })
}

/// One log line; a fetch is logged before it is served, so its exit is unknown.
fn call_entry(
    tool: &str,
    argv: &[String],
    code: Option<i32>,
    stdout: &[u8],
    stderr: &str,
) -> Value {
    let cwd = std::env::current_dir().map_or_else(|_| String::new(), |path| display(&path));
    object([
        ("at", text(&crate::normalize_review::utc_now())),
        ("tool", text(tool)),
        (
            "argv",
            Value::Array(argv.iter().map(|word| text(word)).collect()),
        ),
        ("cwd", text(&cwd)),
        (
            "exit",
            code.map_or(Value::Null, |code| Value::Int(code.to_string())),
        ),
        ("stdout_bytes", Value::Int(stdout.len().to_string())),
        ("stdout_sha256", text(&sha256::hex_digest(stdout))),
        ("stderr", text(stderr)),
    ])
}

/// What the stand-in `gh` prints and its exit code.
pub struct Ran {
    pub stdout: Vec<u8>,
    pub stderr: String,
    pub code: u8,
}

/// Answer one `gh` call and log it; a call that cannot be logged prints nothing and fails.
pub fn serve_gh(root: &Path, argv: &[String]) -> Ran {
    let served = match Served::open(root) {
        Ok(served) => served,
        Err(error) => {
            return Ran {
                stdout: Vec::new(),
                stderr: format!("gh (frozen forge): {error}\n"),
                code: 1,
            };
        }
    };
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let answered = gh::run(&served, &cwd, argv);
    let entry = call_entry(
        "gh",
        argv,
        Some(i32::from(answered.code)),
        &answered.stdout,
        &answered.stderr,
    );
    match log_call(&served.log(), &entry) {
        Ok(()) => answered,
        Err(error) => Ran {
            stdout: Vec::new(),
            stderr: format!("gh (frozen forge): {error}\n"),
            code: 1,
        },
    }
}

/// Log a fetch from `origin`, then serve it with git's own upload-pack. Git can exit before its upload-pack does,
/// so the line is written first.
pub fn upload_pack(root: &Path, argv: &[String]) -> R<i32> {
    let served = Served::open(root)?;
    log_call(
        &served.log(),
        &call_entry("git-upload-pack", argv, None, &[], ""),
    )?;
    let status = Command::new("git")
        .arg("upload-pack")
        .args(argv)
        .status()
        .map_err(|error| format!("cannot run git upload-pack: {error}"))?;
    Ok(status.code().unwrap_or(1))
}

pub struct Setup {
    pub target_dir: PathBuf,
    pub attempt: PathBuf,
    pub clone: PathBuf,
    /// The capture manifest, `bench/forge-captures.v1.json`.
    pub captures: PathBuf,
    /// The manifest's SHA-256 the run pinned.
    pub captures_sha256: Option<String>,
    /// The bench-tools the shims run; copied into the forge so a sandbox that hides the host still reaches it.
    pub executable: PathBuf,
    /// The packet the attempt dispatches, which must state what the capture holds.
    pub packet: Option<PathBuf>,
}

const BASE_MOVED_SINCE_BRANCHING: &str = "the base branch moved after the head branched from it";

/// `packet` as the capture holds its pull request: open and unmerged, as every capture's pull request stood at
/// its cutoff, with a moved base branch's note naming no merge, and with each review thread's fetch-time
/// resolution unknown.
pub fn cutoff_packet(packet: &str) -> String {
    packet
        .split('\n')
        .map(|line| {
            if line.starts_with(STATE_ROW) {
                return format!("{STATE_ROW} `OPEN` |");
            }
            if line.starts_with(MERGED_ROW) {
                return format!("{MERGED_ROW} {NOT_MERGED} |");
            }
            if line.starts_with(MERGE_BASE_ROW) {
                return line.replace(BASE_MOVED_BEFORE_MERGE, BASE_MOVED_SINCE_BRANCHING);
            }
            [THREAD_RESOLVED, THREAD_UNRESOLVED]
                .iter()
                .find_map(|state| line.strip_suffix(state))
                .map_or_else(
                    || line.to_string(),
                    |shared| format!("{shared}{THREAD_STATE_UNKNOWN}"),
                )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Refuse a dispatched packet other than the capture's pinned packet rolled back to the cutoff.
fn check_packet(options: &Setup, target: &Target, entry: &Value) -> R<Value> {
    let Some(dispatched) = &options.packet else {
        return Ok(Value::Null);
    };
    let pinned = get(get(entry, "packet"), "path").py_str();
    let cut = options.target_dir.join(
        Path::new(&pinned)
            .file_name()
            .ok_or_else(|| format!("{}: the capture pins no packet", target.id))?,
    );
    if get(get(entry, "packet"), "sha256").as_str() != Some(sha256_file(&cut)?.as_str()) {
        return Err(format!("{}: {pinned} differs from its pin", target.id));
    }
    let read = |path: &Path| {
        fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))
    };
    if read(dispatched)? != cutoff_packet(&read(&cut)?) {
        return Err(format!(
            "{}: the dispatched packet {} does not state what the capture holds; dispatch {pinned} with the pull request open and unmerged, no merge in the merge-base note, and each thread's resolution unknown, as bench/forge-packet-replacements.v1.json pins it",
            target.id,
            dispatched.display()
        ));
    }
    Ok(object([
        ("path", text(&display(dispatched))),
        ("sha256", text(&sha256_file(dispatched)?)),
    ]))
}

/// The pinned capture entry of `target`, checked against `target.json` and the served files.
fn pinned_entry(options: &Setup, target: &Target) -> R<Value> {
    if let Some(expected) = &options.captures_sha256 {
        let actual = sha256_file(&options.captures)?;
        if &actual != expected {
            return Err(format!(
                "{} hashes to {actual}, but the run pins {expected}",
                options.captures.display()
            ));
        }
    }
    let manifest = read_json(&options.captures)?;
    let entry = get(&manifest, "targets")
        .as_array()
        .unwrap_or_default()
        .iter()
        .find(|entry| get(entry, "target").as_str() == Some(target.id.as_str()))
        .cloned()
        .ok_or_else(|| {
            format!(
                "{} pins no capture of {}",
                options.captures.display(),
                target.id
            )
        })?;
    let target_sha = sha256_file(&options.target_dir.join("target.json"))?;
    if get(&entry, "target_sha256").as_str() != Some(target_sha.as_str()) {
        return Err(format!(
            "{}: target.json hashes to {target_sha}, not the capture's pin",
            target.id
        ));
    }
    for name in SERVED {
        let path = options.target_dir.join("forge").join(name);
        let actual = sha256_file(&path)?;
        if get(get(&entry, "files"), name).as_str() != Some(actual.as_str()) {
            return Err(format!("{}: {name} differs from its pin", target.id));
        }
    }
    Ok(entry)
}

fn shim(executable: &Path, command: &str, root: &Path) -> String {
    format!(
        "#!/bin/sh\nexec {} frozen-forge {command} --forge {} -- \"$@\"\n",
        shlex::quote(&display(executable)),
        shlex::quote(&display(root)),
    )
}

/// Make every file under `path` read-only, keeping directories writable so workspace tools can remove them.
fn read_only(path: &Path) -> R<()> {
    let metadata =
        fs::symlink_metadata(path).map_err(|error| format!("{}: {error}", path.display()))?;
    if metadata.is_dir() {
        for entry in fs::read_dir(path).map_err(|error| format!("{}: {error}", path.display()))? {
            read_only(&entry.map_err(|error| error.to_string())?.path())?;
        }
    } else if metadata.is_file() {
        let mode = metadata.permissions().mode() & 0o555;
        fs::set_permissions(path, fs::Permissions::from_mode(mode))
            .map_err(|error| format!("{}: {error}", path.display()))?;
    }
    Ok(())
}

const REFUSE_PUSH: &str =
    "#!/bin/sh\necho 'the frozen forge is read-only: pushes are refused' >&2\nexit 1\n";

/// Give one attempt its frozen forge and point the clone's `origin` at it; returns the receipt it writes.
pub fn setup(options: &Setup) -> R<Value> {
    let target = provision::load_target(&options.target_dir, None)?;
    let entry = pinned_entry(options, &target)?;
    let packet = check_packet(options, &target, &entry)?;
    let capture = Capture::load(
        &read_json(&options.target_dir.join("forge/graphql.json"))?,
        &read_json(&options.target_dir.join("forge/issues.json"))?,
    )?;
    let pull = &capture.pull;
    if get(pull, "headRefOid").as_str() != Some(target.head.as_str()) {
        return Err(format!(
            "{}: the capture's head is not target.json's head",
            target.id
        ));
    }
    let base_ref = get(pull, "baseRefName").py_str();
    let head_ref = get(pull, "headRefName").py_str();
    let cross = matches!(get(pull, "isCrossRepository"), Value::Bool(true));
    let number = capture.number;
    let pull_ref = format!("refs/pull/{number}/head");
    let root = options.attempt.join("forge");
    if root.exists() {
        return Err(format!(
            "{} exists: attempt directories are never reused",
            root.display()
        ));
    }
    for directory in [
        root.join("bin"),
        root.join("libexec"),
        options.attempt.join("forge-log"),
    ] {
        fs::create_dir_all(&directory)
            .map_err(|error| format!("{}: {error}", directory.display()))?;
    }
    let log = options.attempt.join(LOG);
    fs::write(&log, "").map_err(|error| format!("{}: {error}", log.display()))?;

    let origin = root.join("origin.git");
    let origin_text = display(&origin);
    let clone = display(&options.clone);
    let in_clone = |arguments: &[&str]| -> R<String> {
        let mut all = vec!["-C", clone.as_str()];
        all.extend_from_slice(arguments);
        git(&all)
    };
    // The clone holds exactly the mirror's objects, so the copy borrows them instead of duplicating a history.
    let objects = in_clone(&["rev-parse", "--path-format=absolute", "--git-common-dir"])?;
    git(&["init", "-q", "--bare", &origin_text])?;
    write_file(
        &origin.join("objects/info/alternates"),
        &format!("{}/objects\n", objects.trim()),
        0o644,
    )?;
    let mut refs = vec![
        (pull_ref.clone(), target.head.clone()),
        (format!("refs/heads/{base_ref}"), target.merge_base.clone()),
    ];
    if !cross {
        refs.push((format!("refs/heads/{head_ref}"), target.head.clone()));
    }
    for (name, sha) in &refs {
        git(&["--git-dir", &origin_text, "update-ref", name, sha])?;
    }
    git(&[
        "--git-dir",
        &origin_text,
        "symbolic-ref",
        "HEAD",
        &format!("refs/heads/{base_ref}"),
    ])?;
    let failures = provision::checks(
        &origin,
        &target,
        &format!("refs/heads/{base_ref}"),
        &pull_ref,
        true,
    );
    if !failures.is_empty() {
        return Err(format!(
            "the frozen origin failed its checks: {}",
            failures.join("; ")
        ));
    }
    write_file(&origin.join("hooks/pre-receive"), REFUSE_PUSH, 0o755)?;

    for name in SERVED {
        let path = root.join(name);
        fs::copy(options.target_dir.join("forge").join(name), &path)
            .map_err(|error| format!("{}: {error}", path.display()))?;
    }
    let executable = root.join("bench-tools");
    fs::copy(&options.executable, &executable)
        .map_err(|error| format!("{}: {error}", executable.display()))?;
    let jq = crate::review_isolation::which("jq")
        .ok_or("jq is not installed; the stand-in gh needs it for --jq")?;
    let config = object([
        ("schema_version", Value::Int("1".to_string())),
        ("target", text(&target.id)),
        ("repository", text(&capture.repository)),
        ("pull_request", Value::Int(number.to_string())),
        ("cutoff", get(&entry, "cutoff").clone()),
        ("head", text(&target.head)),
        ("merge_base", text(&target.merge_base)),
        ("base_ref", text(&base_ref)),
        ("head_ref", text(&head_ref)),
        (
            "head_owner",
            get(get(pull, "headRepositoryOwner"), "login").clone(),
        ),
        ("cross_repository", Value::Bool(cross)),
        ("log", text(&display(&log))),
        ("jq", text(&display(&jq))),
    ]);
    write_file(&root.join(CONFIG), &indented(&config), 0o444)?;
    let gh = root.join("bin/gh");
    write_file(&gh, &shim(&executable, "gh", &root), 0o555)?;
    let upload = root.join("libexec/upload-pack");
    write_file(&upload, &shim(&executable, "upload-pack", &root), 0o555)?;

    in_clone(&["remote", "set-url", "origin", &origin_text])?;
    for name in in_clone(&["for-each-ref", "--format=%(refname)", "refs/remotes/origin"])?.lines() {
        in_clone(&["update-ref", "-d", name])?;
    }
    in_clone(&["fetch", "-q", "--no-tags", "origin"])?;
    in_clone(&["remote", "set-head", "origin", &base_ref])?;
    in_clone(&["config", "branch.review-head.remote", "origin"])?;
    in_clone(&["config", "branch.review-head.merge", &pull_ref])?;
    in_clone(&[
        "config",
        &format!("branch.{}.remote", target.base_branch),
        "origin",
    ])?;
    in_clone(&[
        "config",
        &format!("branch.{}.merge", target.base_branch),
        &format!("refs/heads/{base_ref}"),
    ])?;
    in_clone(&[
        "config",
        "remote.origin.uploadpack",
        &shlex::quote(&display(&upload)),
    ])?;
    read_only(&origin)?;
    read_only(&root)?;
    crate::review_isolation::relocate_fetch_head(&options.clone)?;

    let origin_refs: Vec<Value> = refs
        .iter()
        .map(|(name, sha)| object([("ref", text(name)), ("sha", text(sha))]))
        .collect();
    let receipt = object([
        ("schema_version", Value::Int("1".to_string())),
        ("target", text(&target.id)),
        ("repository", text(&capture.repository)),
        ("pull_request", Value::Int(number.to_string())),
        ("cutoff", get(&entry, "cutoff").clone()),
        (
            "captures",
            object([
                ("path", text(&display(&options.captures))),
                ("sha256", text(&sha256_file(&options.captures)?)),
            ]),
        ),
        (
            "served",
            object(SERVED.map(|name| (name, get(get(&entry, "files"), name).clone()))),
        ),
        ("packet", packet),
        ("forge", text(&display(&root))),
        ("bench_tools_sha256", text(&sha256_file(&executable)?)),
        ("jq", text(&display(&jq))),
        (
            "origin",
            object([
                ("path", text(&origin_text)),
                ("refs", Value::Array(origin_refs)),
                ("head", text(&format!("refs/heads/{base_ref}"))),
                (
                    "head_note",
                    text(
                        "origin's HEAD names the pull request's base branch; the capture does not record the repository's default branch",
                    ),
                ),
            ]),
        ),
        (
            "clone",
            object([
                ("review-head", text(&format!("tracks origin {pull_ref}"))),
                (
                    target.base_branch.as_str(),
                    text(&format!("tracks origin refs/heads/{base_ref}")),
                ),
            ]),
        ),
        ("log", text(&display(&log))),
    ]);
    write_file(&options.attempt.join(RECEIPT), &indented(&receipt), 0o644)?;
    Ok(receipt)
}

#[cfg(test)]
mod tests;
