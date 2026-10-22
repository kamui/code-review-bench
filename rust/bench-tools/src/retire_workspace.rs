//! `bench-tools retire-workspace`: remove a fully captured, closed workspace during an operator maintenance window.
//!
//! The closure plan, manifest, inventory, cold restoration receipt and every closure evidence file must be published
//! at a commit reachable from the shared branch. The workspace must match its saved inventory exactly, every file in
//! it must be captured in the published manifest, no controlling reservation may depend on it, no process may use
//! it, and the published packages must restore. Archive presence alone never closes an investigation.
//!
//! A dry run writes the receipt. Applying writes it as `removal-started`, with the workspace's device and inode, then
//! removes each file only while it is still the one the inventory hashed, and finally writes
//! `<receipt>.completed.json`. A retry after any failure rechecks the published closure, accounting and activity and
//! finishes the same removal; a file the inventory does not name is never removed.
//!
//! Exit codes: 0 eligible or retired; 1 refused; 2 an I/O or subprocess failure. Linux `/proc` is required.

use crate::evidence::{self, Error, Store, remote};
use crate::evidence_inventory;
use crate::pyfs;
use crate::pyjson::{self, Duplicates, Object, Value};
use crate::pytext::{self, PyPath};
use crate::removal::{self, Directory, Entry, Hook, Identity, Kind, Point};
use crate::schema::Validator;
use crate::sha256::{self, Sha256};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::io::Read;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::process::{Command, Stdio};

/// `bench/schema/evidence-retirement.schema.json`, which a cargo test keeps byte-identical.
pub const SCHEMA: &str = include_str!("retire_workspace/evidence-retirement.schema.json");

const STARTED: &str = "removal-started";
const RECEIPT_REFUSAL: &str =
    "receipt must be a new path outside the workspace and must not traverse it";
const SKIPPED: [&[u8]; 4] = [b".git", b"node_modules", b"clone", b"clone-cache"];

type R<T> = Result<T, Error>;

fn refuse<T>(message: impl Into<String>) -> R<T> {
    Err(Error::Violation(message.into()))
}

fn failed(message: impl Into<String>) -> Error {
    Error::Io(message.into())
}

fn os_failure(error: &std::io::Error, path: &[u8]) -> Error {
    failed(pytext::os_error(error, path))
}

/// How the command reports a failure: a refusal on stdout, anything else on stderr.
pub fn report(error: &Error) -> String {
    match error {
        Error::Violation(message) => format!("retirement refused: {message}"),
        Error::Io(message) => format!("retirement: {message}"),
    }
}

/// What the canonical repository holds. Cargo tests substitute a fake; the binary always runs `gh`.
pub trait Forge {
    /// `gh api ROUTE`, parsed.
    fn get(&mut self, route: &str) -> R<Value>;
}

impl Forge for evidence::GitHub {
    fn get(&mut self, route: &str) -> R<Value> {
        remote::api(&["api", route])
    }
}

/// What retirement acts through.
pub struct Effects<'a> {
    /// Verifies the published packages.
    pub store: Store<'a>,
    pub forge: &'a mut dyn Forge,
    /// The processes using the workspace; the binary passes [`active_users`].
    pub active: &'a dyn Fn(&PyPath) -> R<Vec<u32>>,
    pub hook: Hook<'a>,
}

pub struct Request<'a> {
    /// The checkout holding the published plan.
    pub root: &'a [u8],
    /// The plan's repository-relative path.
    pub plan: &'a str,
    pub commit: &'a str,
    pub branch: &'a str,
    pub receipt: &'a [u8],
    pub apply: bool,
}

static NULL: Value = Value::Null;

fn at<'a>(value: &'a Value, key: &str) -> &'a Value {
    value
        .as_object()
        .and_then(|object| object.get(key))
        .unwrap_or(&NULL)
}

fn text_at(value: &Value, key: &str) -> R<String> {
    at(value, key)
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| Error::Violation(format!("KeyError: {}", pytext::repr_str(key))))
}

fn is_count(value: &Value, expected: u64) -> bool {
    matches!(value, Value::Int(digits) if *digits == expected.to_string())
}

fn count(value: &Value) -> u64 {
    match value {
        Value::Int(digits) => digits.parse().unwrap_or(u64::MAX),
        _ => u64::MAX,
    }
}

fn integer(value: impl ToString) -> Value {
    Value::Int(value.to_string())
}

fn text(value: &str) -> Value {
    Value::Str(value.to_string())
}

fn inventory_error(error: evidence_inventory::Error) -> Error {
    match error {
        evidence_inventory::Error::Violation(message) => {
            Error::Violation(String::from_utf8_lossy(&message).into_owned())
        }
        evidence_inventory::Error::Io(message) => Error::Io(message),
    }
}

fn absolute(path: &[u8]) -> R<PyPath> {
    evidence_inventory::absolute(path).map_err(inventory_error)
}

fn resolved(path: &PyPath) -> PyPath {
    PyPath::new(&pyfs::resolve(path.as_bytes()))
}

fn inventory(workspace: &PyPath) -> R<Value> {
    evidence_inventory::inventory(workspace.as_bytes(), true).map_err(inventory_error)
}

fn is_relative_to(path: &PyPath, base: &PyPath) -> bool {
    evidence_inventory::is_relative_to(path, base)
}

/// Whether `path` is a link; a path that cannot be inspected is an error.
fn is_symlink(path: &PyPath) -> R<bool> {
    match removal::lstat(&path.to_path_buf()) {
        Ok(found) => Ok(matches!(found, Some((_, Kind::Symlink)))),
        Err(_) if missing(path) => Ok(false),
        Err(message) => Err(failed(message)),
    }
}

/// A path whose parent is not a directory names nothing, as `pathlib` treats it.
fn missing(path: &PyPath) -> bool {
    std::fs::symlink_metadata(path.to_path_buf()).is_err_and(|error| {
        error.kind() == std::io::ErrorKind::NotFound
            || error.raw_os_error() == Some(nix::errno::Errno::ENOTDIR as i32)
    })
}

/// Whether `path` is a directory, following links; a dangling link or a missing path is not.
fn is_dir(path: &PyPath) -> R<bool> {
    Ok(followed(path)?.is_some_and(|metadata| metadata.is_dir()))
}

/// What `path` names after following links, `None` when nothing does; a path that cannot be searched is an error.
fn followed(path: &PyPath) -> R<Option<std::fs::Metadata>> {
    match std::fs::metadata(path.to_path_buf()) {
        Ok(metadata) => Ok(Some(metadata)),
        Err(error)
            if error.kind() == std::io::ErrorKind::NotFound
                || [nix::errno::Errno::ENOTDIR, nix::errno::Errno::ELOOP]
                    .iter()
                    .any(|errno| error.raw_os_error() == Some(*errno as i32)) =>
        {
            Ok(None)
        }
        Err(error) => Err(os_failure(&error, path.as_bytes())),
    }
}

/// The names in `directory`; a directory that cannot be listed is an error, never an empty one.
fn names(directory: &PyPath) -> R<Vec<Vec<u8>>> {
    let mut found = Vec::new();
    for entry in std::fs::read_dir(directory.to_path_buf())
        .map_err(|error| os_failure(&error, directory.as_bytes()))?
    {
        let entry = entry.map_err(|error| os_failure(&error, directory.as_bytes()))?;
        found.push(pytext::os_bytes(&entry.file_name()).to_vec());
    }
    Ok(found)
}

/// `git -C directory ARGS`, stripped; a failing command is an error.
fn git(directory: &PyPath, arguments: &[&str]) -> R<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(directory.to_path_buf())
        .args(arguments)
        .stdin(Stdio::null())
        .stderr(Stdio::inherit())
        .output()
        .map_err(|error| os_failure(&error, b"git"))?;
    if !output.status.success() {
        let shown = directory.display();
        let command: Vec<&str> = ["git", "-C", &shown]
            .into_iter()
            .chain(arguments.iter().copied())
            .collect();
        return Err(failed(remote::called_process_error(
            &command,
            output.status,
        )));
    }
    let stdout = pytext::decode_utf8(&output.stdout).map_err(failed)?;
    Ok(pytext::strip(&stdout).to_string())
}

/// The closure plan, after its schema.
struct Plan {
    kind: String,
    workspace: String,
    manifest: String,
    inventory: String,
    restoration_receipt: String,
    capture_prefix: String,
    accounting_roots: Vec<String>,
    evidence: Value,
    classifications: Value,
}

impl Plan {
    fn read(path: &PyPath) -> R<Self> {
        let plan = evidence::read(path)?;
        let schema = pyjson::loads(SCHEMA, Duplicates::KeepLast)
            .map_err(|error| failed(format!("retirement schema: {error}")))?;
        let errors = Validator::default()
            .validate(&schema, &plan)
            .map_err(|error| Error::Violation(error.0))?;
        if !errors.is_empty() {
            return refuse(format!("invalid closure: {}", errors.join("; ")));
        }
        Ok(Self {
            kind: text_at(&plan, "kind")?,
            workspace: text_at(&plan, "workspace")?,
            manifest: text_at(&plan, "manifest")?,
            inventory: text_at(&plan, "inventory")?,
            restoration_receipt: text_at(&plan, "restoration_receipt")?,
            capture_prefix: text_at(&plan, "capture_prefix")?,
            accounting_roots: at(&plan, "accounting_roots")
                .as_array()
                .unwrap_or_default()
                .iter()
                .filter_map(|root| root.as_str().map(str::to_string))
                .collect(),
            evidence: at(&plan, "evidence").clone(),
            classifications: at(&plan, "classifications").clone(),
        })
    }
}

/// `urllib.parse.quote(name, safe="/")`.
fn quote_path(name: &str) -> String {
    name.split('/')
        .map(remote::quote)
        .collect::<Vec<_>>()
        .join("/")
}

fn reachable(forge: &mut dyn Forge, repository: &str, commit: &str, branch: &str) -> R<bool> {
    let comparison = forge.get(&format!(
        "repos/{repository}/compare/{commit}...{}",
        remote::quote(branch)
    ))?;
    Ok(matches!(
        at(&comparison, "status").as_str(),
        Some("ahead" | "identical")
    ))
}

/// The confined file `name`, refused unless the canonical repository holds these exact bytes at `commit`.
fn shared_file(
    root: &PyPath,
    name: &str,
    repository: &str,
    commit: &str,
    forge: &mut dyn Forge,
) -> R<PyPath> {
    let path = evidence::confined(root, name)?;
    let response = forge.get(&format!(
        "repos/{repository}/contents/{}?ref={commit}",
        quote_path(name)
    ))?;
    let size = std::fs::metadata(path.to_path_buf())
        .map_err(|error| os_failure(&error, path.as_bytes()))?
        .len();
    let (_, blob) =
        evidence_inventory::identities(path.as_bytes(), size).map_err(inventory_error)?;
    if at(&response, "type").as_str() != Some("file")
        || at(&response, "sha").as_str() != Some(blob.as_str())
        || !is_count(at(&response, "size"), size)
    {
        return refuse(format!("file is not published at {commit}: {name}"));
    }
    Ok(path)
}

/// What the published closure establishes, none of which depends on the workspace's current contents.
struct Published {
    plan: Plan,
    plan_sha256: String,
    manifest_value: Value,
    manifest: evidence::Manifest,
    snapshot: Value,
    workspace: PyPath,
    roots: Vec<PyPath>,
}

fn published(root: &PyPath, request: &Request<'_>, forge: &mut dyn Forge) -> R<Published> {
    let plan_path = evidence::confined(root, request.plan)?;
    let plan = Plan::read(&plan_path)?;
    let commit = request.commit;
    if commit.len() != 40
        || !commit
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        || request.branch.is_empty()
    {
        return refuse("pin a pushed commit and its shared branch");
    }
    let manifest_path = evidence::confined(root, &plan.manifest)?;
    let manifest_value = evidence::read(&manifest_path)?;
    let manifest = evidence::validate(&manifest_value, true)?;
    let repository = manifest.repository.clone();
    if !reachable(forge, &repository, commit, request.branch)? {
        return refuse("commit is not reachable from the shared branch");
    }
    for name in [
        request.plan,
        &plan.manifest,
        &plan.inventory,
        &plan.restoration_receipt,
    ] {
        shared_file(root, name, &repository, commit, forge)?;
    }
    let receipt = evidence::read(&evidence::confined(root, &plan.restoration_receipt)?)?;
    if at(&receipt, "manifest_sha256").as_str() != Some(&evidence::digest(&manifest_path)?)
        || at(&receipt, "status").as_str() != Some("verified")
    {
        return refuse("cold restoration receipt does not match the published manifest");
    }
    let mut required = vec![
        "inputs",
        "execution",
        "accounting",
        "diagnostics",
        "disposition",
        "lineage",
    ];
    if plan.kind == "grading" {
        required.push("grading");
    }
    if required
        .iter()
        .any(|category| at(&plan.evidence, category).as_array().is_none())
    {
        return refuse("closure omits required evidence categories");
    }
    for category in required {
        let references = at(&plan.evidence, category).as_array().unwrap_or_default();
        if references.is_empty() {
            return refuse(format!("closure lacks {category} evidence"));
        }
        for reference in references {
            let name = text_at(reference, "path")?;
            let path = shared_file(root, &name, &repository, commit, forge)?;
            if evidence::digest(&path)? != text_at(reference, "sha256")? {
                return refuse(format!("closure evidence changed: {name}"));
            }
        }
    }
    let workspace = absolute(plan.workspace.as_bytes())?;
    if resolved(&workspace).as_bytes() != workspace.as_bytes() {
        return refuse(
            "workspace path must be canonical; capture its inventory again without aliases or parent traversals",
        );
    }
    let home = std::env::var_os("HOME").map(|home| PyPath::new(pytext::os_bytes(&home)));
    let mut ancestry = std::iter::once(workspace.clone()).chain(evidence::parents(&workspace));
    if workspace.as_bytes() == root.as_bytes()
        || is_relative_to(root, &workspace)
        || home.is_some_and(|home| home.as_bytes() == workspace.as_bytes())
        || evidence::parents(&workspace).len() < 3
        || ancestry.try_fold(false, |found, path| {
            Ok::<_, Error>(found || is_symlink(&path)?)
        })?
    {
        return refuse("unsafe workspace root");
    }
    let snapshot = evidence::read(&evidence::confined(root, &plan.inventory)?)?;
    if at(&snapshot, "root").os_bytes() != Some(workspace.as_bytes()) {
        return refuse("inventory names another workspace");
    }
    let mut roots = vec![root.clone()];
    for path in &plan.accounting_roots {
        roots.push(absolute(path.as_bytes())?);
    }
    Ok(Published {
        plan,
        plan_sha256: evidence::digest(&plan_path)?,
        manifest_value,
        manifest,
        snapshot,
        workspace,
        roots,
    })
}

/// The workspace as the inventory saw it: no development work, every file captured, and for a linked worktree its
/// head pushed. Returns a worktree's Git common directory.
fn unchanged(
    published: &Published,
    request: &Request<'_>,
    forge: &mut dyn Forge,
) -> R<Option<PyPath>> {
    let workspace = &published.workspace;
    let current = inventory(workspace)?;
    let head = at(&current, "head").as_str().map(str::to_string);
    if removal::lstat(&workspace.join(b".git").to_path_buf())
        .map_err(failed)?
        .is_some()
        && head.is_none()
    {
        return refuse("workspace Git identity cannot be verified");
    }
    let entries = at(&current, "entries").as_array().unwrap_or_default();
    if !at(&current, "missing_tracked")
        .as_array()
        .unwrap_or_default()
        .is_empty()
        || entries
            .iter()
            .any(|entry| at(entry, "class").as_str() == Some("development"))
    {
        return refuse("modified development work blocks retirement");
    }
    let common = if published.plan.kind == "worktree" {
        let Some(head) = head else {
            return refuse("worktree has uncommitted work");
        };
        if !git(
            workspace,
            &["status", "--porcelain", "--untracked-files=all"],
        )?
        .is_empty()
        {
            return refuse("worktree has uncommitted work");
        }
        let common = git(workspace, &["rev-parse", "--git-common-dir"])?;
        let common = if common.starts_with('/') {
            PyPath::new(common.as_bytes())
        } else {
            workspace.join(common.as_bytes())
        };
        let common = resolved(&common);
        if is_relative_to(&common, workspace) {
            return refuse("only linked worktrees can be retired");
        }
        if !reachable(forge, &published.manifest.repository, &head, request.branch)? {
            return refuse("worktree has unpushed commits");
        }
        Some(common)
    } else if head.is_some() {
        return refuse("Git worktrees require kind worktree");
    } else {
        None
    };
    for clone in nested_clones(entries) {
        let clone = workspace.join(&clone);
        if !git(&clone, &["status", "--porcelain", "--untracked-files=all"])?.is_empty() {
            return refuse(format!("modified nested clone: {}", clone.display()));
        }
    }
    capture_check(
        &published.snapshot,
        &current,
        &published.manifest,
        &published.plan.capture_prefix,
        published.plan.kind == "worktree",
        &published.plan.classifications,
    )?;
    Ok(common)
}

/// Each directory below the top that holds a `.git`, outside any `.git` directory.
fn nested_clones(entries: &[Value]) -> BTreeSet<Vec<u8>> {
    let mut clones = BTreeSet::new();
    for entry in entries {
        let Some(path) = at(entry, "path").os_bytes() else {
            continue;
        };
        let parts: Vec<&[u8]> = path.split(|byte| *byte == b'/').collect();
        if let Some(index) = parts.iter().position(|part| *part == b".git")
            && index > 0
        {
            clones.insert(parts[..index].join(&b'/'));
        }
    }
    clones
}

fn capture_check(
    snapshot: &Value,
    current: &Value,
    manifest: &evidence::Manifest,
    prefix: &str,
    worktree: bool,
    classifications: &Value,
) -> R<()> {
    if !snapshot.py_eq(current) {
        return refuse("workspace changed since the complete inventory was captured");
    }
    evidence::logical_path(prefix)?;
    let members: HashMap<&str, &evidence::Member> = manifest
        .packages
        .iter()
        .flat_map(|package| &package.members)
        .map(|member| (member.path.as_str(), member))
        .collect();
    for entry in at(current, "entries").as_array().unwrap_or_default() {
        let name = at(entry, "path").py_str();
        let class = at(entry, "class").as_str().unwrap_or_default();
        if worktree && (class == "tracked-identical" || name == ".git") {
            continue;
        }
        if at(entry, "type").as_str() != Some("file") {
            return refuse(format!("uncaptured non-file blocks retirement: {name}"));
        }
        let sha = at(entry, "sha256").as_str();
        if class == "unknown" {
            let decision = at(classifications, &name);
            if !matches!(
                at(decision, "class").as_str(),
                Some("local-evidence" | "reproducibility")
            ) || !at(decision, "reason").truthy()
                || at(decision, "sha256").as_str() != sha
            {
                return refuse(format!("unknown file blocks retirement: {name}"));
            }
        }
        if class == "account-state" {
            continue;
        }
        let captured = members
            .get(format!("{prefix}/{name}").as_str())
            .is_some_and(|member| {
                Some(member.sha256.as_str()) == sha
                    && is_count(at(entry, "bytes"), member.bytes)
                    && is_count(at(entry, "mode"), u64::from(member.mode))
            });
        if !captured {
            return refuse(format!("uncaptured file blocks retirement: {name}"));
        }
    }
    Ok(())
}

/// Whether `path`, or any path a link along it names, lies in `workspace`.
fn path_depends_on(path: &PyPath, workspace: &PyPath) -> R<bool> {
    let mut pending = vec![absolute(path.as_bytes())?];
    let mut seen = HashSet::new();
    while let Some(path) = pending.pop() {
        if !seen.insert(path.as_bytes().to_vec()) {
            continue;
        }
        if seen.len() > 256 {
            return refuse("accounting path has too many link dependencies");
        }
        for current in std::iter::once(path.clone()).chain(evidence::parents(&path)) {
            if is_relative_to(&current, workspace) || is_relative_to(&resolved(&current), workspace)
            {
                return Ok(true);
            }
            if is_symlink(&current)? {
                let target = std::fs::read_link(current.to_path_buf())
                    .map_err(|error| os_failure(&error, current.as_bytes()))?;
                let target = pytext::os_bytes(target.as_os_str());
                pending.push(if target.starts_with(b"/") {
                    PyPath::new(target)
                } else {
                    evidence::parent(&current).join(target)
                });
            }
        }
    }
    Ok(false)
}

/// Every reservation below `root`: `batches/*/*/attempt-*/reservation.json`, following links, and any other one
/// outside Git metadata and rebuildable directories, without following links.
fn reservations(root: &PyPath, found: &mut Vec<PyPath>) -> R<()> {
    let batches = root.join(b"batches");
    if is_dir(&batches)? {
        for run in names(&batches)? {
            let run = batches.join(&run);
            if !is_dir(&run)? {
                continue;
            }
            for target in names(&run)? {
                let target = run.join(&target);
                if !is_dir(&target)? {
                    continue;
                }
                for attempt in names(&target)? {
                    let reservation = target.join(&attempt).join(b"reservation.json");
                    if attempt.starts_with(b"attempt-") && followed(&reservation)?.is_some() {
                        found.push(reservation);
                    }
                }
            }
        }
    }
    let mut pending = vec![root.clone()];
    while let Some(directory) = pending.pop() {
        for name in names(&directory)? {
            let path = directory.join(&name);
            if is_dir(&path)? {
                if !SKIPPED.contains(&name.as_slice()) && !is_symlink(&path)? {
                    pending.push(path);
                }
            } else if name == b"reservation.json" {
                found.push(path);
            }
        }
    }
    Ok(())
}

/// The reservations whose settlement still reads the workspace's bytes.
fn accounting_dependencies(workspace: &PyPath, roots: &[PyPath]) -> R<Vec<PyPath>> {
    let mut found = Vec::new();
    for root in roots {
        if !is_dir(root)? {
            return refuse(format!("accounting root missing: {}", root.display()));
        }
        reservations(root, &mut found)?;
    }
    found.sort_by_cached_key(|path| pytext::path_order(path.as_bytes()));
    found.dedup_by(|a, b| a.as_bytes() == b.as_bytes());
    let mut blockers = Vec::new();
    for reservation in found {
        let attempt = evidence::parent(&reservation);
        let work = attempt.join(b"work");
        let resolution = attempt.join(b"budget-resolution.json");
        let mut inputs = vec![
            reservation.clone(),
            work.clone(),
            work.join(b"dispatch.json"),
            work.join(b"home"),
            resolution.clone(),
        ];
        if resolution.is_file() {
            let record = evidence::read(&resolution)?;
            let Some(references) = at(&record, "evidence").as_array() else {
                return refuse(format!(
                    "budget resolution names no evidence: {}",
                    resolution.display()
                ));
            };
            for reference in references {
                let path = text_at(reference, "path")?;
                if !path.starts_with('/') {
                    return refuse(format!(
                        "relative accounting proof needs its original controller checkout: {}",
                        resolution.display()
                    ));
                }
                inputs.push(PyPath::new(path.as_bytes()));
            }
        }
        let mut depends = is_relative_to(workspace, &resolved(&work));
        for input in &inputs {
            depends = depends || path_depends_on(input, workspace)?;
        }
        if depends {
            blockers.push(reservation);
        }
    }
    Ok(blockers)
}

/// The processes of this user, other than this one, whose working directory, open files or command line name a
/// path in `workspace`.
pub fn active_users(workspace: &PyPath) -> R<Vec<u32>> {
    let proc = std::path::Path::new("/proc");
    if !proc.join("self/fd").is_dir() {
        return refuse("cannot establish inactivity without Linux /proc");
    }
    let own = std::process::id();
    let uid = std::fs::metadata(proc.join("self"))
        .map_err(|error| os_failure(&error, b"/proc/self"))?
        .uid();
    let mut active = BTreeSet::new();
    for entry in std::fs::read_dir(proc).map_err(|error| os_failure(&error, b"/proc"))? {
        let entry = entry.map_err(|error| os_failure(&error, b"/proc"))?;
        let Some(pid) = entry
            .file_name()
            .to_str()
            .filter(|name| name.bytes().all(|byte| byte.is_ascii_digit()))
            .and_then(|name| name.parse::<u32>().ok())
        else {
            continue;
        };
        if pid == own {
            continue;
        }
        match uses(&entry.path(), workspace, uid) {
            Ok(true) => {
                active.insert(pid);
            }
            Ok(false) => {}
            Err(error)
                if error.kind() == std::io::ErrorKind::NotFound
                    || error.raw_os_error() == Some(nix::errno::Errno::ESRCH as i32) => {}
            Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
                return refuse(format!("cannot inspect process {pid}"));
            }
            Err(error) => {
                return Err(os_failure(
                    &error,
                    pytext::os_bytes(entry.path().as_os_str()),
                ));
            }
        }
    }
    Ok(active.into_iter().collect())
}

fn uses(process: &std::path::Path, workspace: &PyPath, uid: u32) -> std::io::Result<bool> {
    if std::fs::metadata(process)?.uid() != uid {
        return Ok(false);
    }
    let mut links = vec![process.join("cwd")];
    for descriptor in std::fs::read_dir(process.join("fd"))? {
        links.push(descriptor?.path());
    }
    for link in links {
        match std::fs::read_link(&link) {
            Ok(target) => {
                let target = PyPath::new(pytext::os_bytes(target.as_os_str()));
                if target.as_bytes().starts_with(b"/") && is_relative_to(&target, workspace) {
                    return Ok(true);
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    let command = std::fs::read(process.join("cmdline"))?;
    Ok(command
        .windows(workspace.as_bytes().len())
        .any(|window| window == workspace.as_bytes()))
}

/// The closure's settlement: no reservation depends on the workspace, no process uses it, and every package
/// restores from the remote.
fn settled(published: &Published, effects: &mut Effects<'_>) -> R<()> {
    if published.plan.accounting_roots.is_empty() {
        return refuse("inventory all controlling accounting queues explicitly");
    }
    let blockers = accounting_dependencies(&published.workspace, &published.roots)?;
    if !blockers.is_empty() {
        let shown: Vec<String> = blockers
            .iter()
            .map(|path| pytext::repr_str(&path.display()))
            .collect();
        return refuse(format!(
            "workspace-dependent accounting: [{}]",
            shown.join(", ")
        ));
    }
    idle(published, effects, "workspace is active")?;
    evidence::verify_remote(&mut effects.store, &published.manifest_value)
}

fn idle(published: &Published, effects: &Effects<'_>, message: &str) -> R<()> {
    let pids = (effects.active)(&published.workspace)?;
    if pids.is_empty() {
        return Ok(());
    }
    let shown: Vec<String> = pids.iter().map(u32::to_string).collect();
    refuse(format!("{message}: [{}]", shown.join(", ")))
}

fn same_as_snapshot(published: &Published, effects: &Effects<'_>, message: &str) -> R<()> {
    if !inventory(&published.workspace)?.py_eq(&published.snapshot) {
        return refuse(message);
    }
    idle(published, effects, message)
}

/// The files the inventory hashed, by path.
fn hashed_files(snapshot: &Value) -> HashMap<Vec<u8>, (String, u64, u64)> {
    at(snapshot, "entries")
        .as_array()
        .unwrap_or_default()
        .iter()
        .filter(|entry| at(entry, "type").as_str() == Some("file"))
        .filter_map(|entry| {
            Some((
                at(entry, "path").os_bytes()?.to_vec(),
                (
                    at(entry, "sha256").as_str()?.to_string(),
                    count(at(entry, "bytes")),
                    count(at(entry, "mode")),
                ),
            ))
        })
        .collect()
}

fn hash(mut file: &std::fs::File) -> std::io::Result<String> {
    let mut sha = Sha256::new();
    let mut buffer = vec![0u8; 1024 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            return Ok(sha256::hex(&sha.finalize()));
        }
        sha.update(&buffer[..read]);
    }
}

/// Refuse an entry unless it is a file the inventory hashed, with the same bytes and mode.
fn captured(
    files: &HashMap<Vec<u8>, (String, u64, u64)>,
    relative: &[u8],
    entry: Entry<'_>,
) -> Result<(), String> {
    let shown = String::from_utf8_lossy(relative);
    let Some((sha, bytes, mode)) = files.get(relative) else {
        return Err(format!(
            "{shown} is not in the saved inventory; it is never removed"
        ));
    };
    let Entry::File(file) = entry else {
        return Err(format!("{shown} is no longer a regular file"));
    };
    let metadata = file.metadata().map_err(|error| error.to_string())?;
    if metadata.len() != *bytes
        || u64::from(metadata.mode() & 0o7777) != *mode
        || hash(file).map_err(|error| error.to_string())? != *sha
    {
        return Err(format!("{shown} changed since the inventory was saved"));
    }
    Ok(())
}

/// The workspace's device and inode, as the started receipt records them.
fn identity_fields(identity: Identity) -> [(&'static str, Value); 2] {
    [
        ("workspace_device", integer(identity.device)),
        ("workspace_inode", integer(identity.inode)),
    ]
}

/// Remove the workspace, which must still be `identity` when it is present, then a worktree's Git registration.
fn remove(
    published: &Published,
    identity: Identity,
    common: Option<&PyPath>,
    hook: Hook<'_>,
) -> R<()> {
    let workspace = &published.workspace;
    let interrupted = |error: std::io::Error| failed(error.to_string());
    hook(Point::BeforeRemoval).map_err(interrupted)?;
    match removal::lstat(&workspace.to_path_buf()).map_err(failed)? {
        None => {}
        Some((found, Kind::Directory)) if found == identity => {
            let parent = evidence::parent(workspace);
            let Some((parent_identity, Kind::Directory)) =
                removal::lstat(&parent.to_path_buf()).map_err(failed)?
            else {
                return Err(failed(format!("{} is not a directory", parent.display())));
            };
            let directory =
                Directory::open(&parent.to_path_buf(), parent_identity).map_err(failed)?;
            let name = &workspace.as_bytes()[parent.as_bytes().len()..];
            let name = name.strip_prefix(b"/").unwrap_or(name);
            let files = hashed_files(&published.snapshot);
            directory
                .remove_tree(
                    name,
                    identity,
                    &mut |relative, entry| captured(&files, relative, entry),
                    hook,
                )
                .map_err(failed)?;
        }
        Some(_) => return refuse("workspace changed after retirement started"),
    }
    if let Some(common) = common {
        let listed = git(common, &["worktree", "list", "--porcelain"])?;
        let registered = format!("worktree {}", workspace.display());
        if listed.lines().any(|line| line == registered) {
            git(
                common,
                &["worktree", "remove", "--force", &workspace.display()],
            )?;
        }
    }
    hook(Point::BeforeReceipt).map_err(interrupted)
}

fn lock(directory: &PyPath) -> R<nix::fcntl::Flock<std::fs::File>> {
    let path = directory.join(b".evidence-retirement.lock");
    let file = std::fs::OpenOptions::new()
        .append(true)
        .create(true)
        .custom_flags(nix::fcntl::OFlag::O_NOFOLLOW.bits())
        .open(path.to_path_buf())
        .map_err(|error| os_failure(&error, path.as_bytes()))?;
    nix::fcntl::Flock::lock(file, nix::fcntl::FlockArg::LockExclusiveNonblock).map_err(
        |(_, errno)| {
            os_failure(
                &std::io::Error::from_raw_os_error(errno as i32),
                path.as_bytes(),
            )
        },
    )
}

fn free_bytes(path: &PyPath) -> R<u64> {
    removal::free_bytes(&path.to_path_buf()).map_err(failed)
}

fn summary(published: &Published, request: &Request<'_>, effects: &Effects<'_>) -> R<Object> {
    let allocated: u64 = at(&published.snapshot, "entries")
        .as_array()
        .unwrap_or_default()
        .iter()
        .map(|entry| count(at(entry, "allocated_bytes")))
        .sum();
    Ok([
        ("schema_version", integer(1)),
        ("workspace", Value::os_text(published.workspace.as_bytes())),
        ("shared_commit", text(request.commit)),
        ("plan_sha256", text(&published.plan_sha256)),
        ("applied", Value::Bool(false)),
        (
            "packages",
            Value::Array(
                published
                    .manifest
                    .packages
                    .iter()
                    .map(|package| text(&package.sha256))
                    .collect(),
            ),
        ),
        ("blocked_paths", Value::Array(Vec::new())),
        ("at", text(&(effects.store.now)())),
        ("inventoried_allocated_bytes", integer(allocated)),
        (
            "free_bytes_before",
            integer(free_bytes(&published.workspace)?),
        ),
    ]
    .into_iter()
    .map(|(key, value)| (key.to_string(), value))
    .collect())
}

/// The completed receipt: the started one without its removal state, applied, with the space it freed.
fn finish(published: &Published, started: &Value, completed: &PyPath) -> R<Value> {
    let mut result: Object = started
        .as_object()
        .into_iter()
        .flat_map(Object::iter)
        .filter(|(key, _)| {
            !matches!(
                key.as_str(),
                "state" | "workspace_device" | "workspace_inode" | "git_common_dir"
            )
        })
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();
    result.insert("applied".to_string(), Value::Bool(true));
    let before = i128::from(count(at(started, "free_bytes_before")));
    let after = free_bytes(&evidence::parent(&published.workspace))?;
    result.insert("free_bytes_after".to_string(), integer(after));
    result.insert(
        "free_bytes_delta".to_string(),
        integer(i128::from(after) - before),
    );
    let result = Value::Object(result);
    evidence::write_new(&mut evidence::System, completed, &result)?;
    Ok(result)
}

fn completed_path(receipt: &PyPath) -> PyPath {
    PyPath::new(&[receipt.as_bytes(), b".completed.json"].concat())
}

/// Retire the workspace a published closure plan names, or report that it could be retired.
pub fn retire(request: &Request<'_>, effects: &mut Effects<'_>) -> R<Value> {
    let root = resolved(&absolute(request.root)?);
    let requested = absolute(request.receipt)?;
    let receipt = resolved(&requested);
    if removal::lstat(&receipt.to_path_buf())
        .map_err(failed)?
        .is_some()
    {
        return resume(&root, request, &requested, &receipt, effects);
    }
    let published = published(&root, request, effects.forge)?;
    let common = unchanged(&published, request, effects.forge)?;
    settled(&published, effects)?;
    same_as_snapshot(
        &published,
        effects,
        "workspace changed or became active during verification",
    )?;
    if path_depends_on(&requested, &published.workspace)? {
        return refuse(RECEIPT_REFUSAL);
    }
    let Some((identity, Kind::Directory)) =
        removal::lstat(&published.workspace.to_path_buf()).map_err(failed)?
    else {
        return refuse("workspace changed during verification");
    };
    let mut result = summary(&published, request, effects)?;
    if !request.apply {
        let result = Value::Object(result);
        evidence::write_new(&mut evidence::System, &receipt, &result)?;
        return Ok(result);
    }
    // The operator maintenance window covers clients that do not share the runner lock.
    let _lock = lock(&evidence::parent(&published.workspace))?;
    same_as_snapshot(&published, effects, "workspace changed before removal")?;
    result.insert("state".to_string(), text(STARTED));
    for (key, value) in identity_fields(identity) {
        result.insert(key.to_string(), value);
    }
    if let Some(common) = &common {
        result.insert(
            "git_common_dir".to_string(),
            Value::os_text(common.as_bytes()),
        );
    }
    let started = Value::Object(result);
    evidence::write_new(&mut evidence::System, &receipt, &started)?;
    remove(&published, identity, common.as_ref(), effects.hook)?;
    finish(&published, &started, &completed_path(&receipt))
}

/// Finish a retirement whose started receipt is saved, or return its completed receipt.
fn resume(
    root: &PyPath,
    request: &Request<'_>,
    requested: &PyPath,
    receipt: &PyPath,
    effects: &mut Effects<'_>,
) -> R<Value> {
    let started = evidence::read(receipt)?;
    let published = published(root, request, effects.forge)?;
    let matches = |record: &Value| {
        at(record, "workspace").os_bytes() == Some(published.workspace.as_bytes())
            && at(record, "plan_sha256").as_str() == Some(&published.plan_sha256)
            && at(record, "shared_commit").as_str() == Some(request.commit)
    };
    if at(&started, "state").as_str() != Some(STARTED)
        || !matches(&started)
        || path_depends_on(requested, &published.workspace)?
    {
        return refuse(RECEIPT_REFUSAL);
    }
    let completed = completed_path(receipt);
    if removal::lstat(&completed.to_path_buf())
        .map_err(failed)?
        .is_some()
    {
        let done = evidence::read(&completed)?;
        if !matches(&done) || !matches!(at(&done, "applied"), Value::Bool(true)) {
            return refuse(format!(
                "{} is not this retirement's receipt",
                completed.display()
            ));
        }
        return Ok(done);
    }
    if !request.apply {
        return refuse("retirement already started; rerun with --apply to finish it");
    }
    settled(&published, effects)?;
    let identity = Identity {
        device: count(at(&started, "workspace_device")),
        inode: count(at(&started, "workspace_inode")),
    };
    let common = at(&started, "git_common_dir").os_bytes().map(PyPath::new);
    if (published.plan.kind == "worktree") != common.is_some() {
        return refuse(RECEIPT_REFUSAL);
    }
    let _lock = lock(&evidence::parent(&published.workspace))?;
    idle(&published, effects, "workspace is active")?;
    remove(&published, identity, common.as_ref(), effects.hook)?;
    finish(&published, &started, &completed)
}

#[cfg(test)]
mod tests;
