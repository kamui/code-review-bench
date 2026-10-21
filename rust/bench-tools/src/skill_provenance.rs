//! A skill's release metadata and verified runtime source, recorded before a run freezes, and the check that a
//! frozen arm's record still matches its pin.

use crate::pyjson::{self, Dumps, Object, Value};
use crate::{pysemantics, pyvalue, sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::process::Command;

const EXCLUDED_PARTS: [&str; 5] = ["tests", "test", "history", "licenses", "__pycache__"];
const EXCLUDED_NAMES: [&str; 7] = [
    "design.md",
    "changelog.md",
    "history.md",
    "readme.md",
    "license",
    "license.md",
    "third_party_notices.md",
];

pub struct Request<'a> {
    pub repository: &'a Path,
    pub revision: &'a str,
    pub skill_path: &'a str,
    pub frozen: Option<&'a Path>,
    pub benchmark_tree: Option<&'a str>,
    pub release: Option<&'a Value>,
    pub exclusions: &'a [String],
    pub inclusions: &'a [String],
}

fn git(repository: &Path, arguments: &[&str]) -> Result<Vec<u8>, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repository)
        .args(arguments)
        .output()
        .map_err(|error| format!("git: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "git {} failed: {}",
            arguments.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(output.stdout)
}

fn git_text(repository: &Path, arguments: &[&str]) -> Result<String, String> {
    git(repository, arguments).map(|bytes| String::from_utf8_lossy(&bytes).trim().to_string())
}

fn glob(path: &str, pattern: &str) -> bool {
    let points = |text: &str| text.chars().map(u32::from).collect::<Vec<_>>();
    pyvalue::fnmatchcase(&points(path), &points(pattern)).unwrap_or(false)
}

/// Whether a skill file, by its path inside the skill, is part of what the skill runs on: an inclusion glob
/// always counts; documentation, history, licences, tests and exclusion globs never do.
pub fn runtime_file(path: &str, exclusions: &[String], inclusions: &[String]) -> bool {
    if inclusions.iter().any(|pattern| glob(path, pattern)) {
        return true;
    }
    let parts: Vec<&str> = path.split('/').filter(|part| !part.is_empty()).collect();
    let name = parts
        .last()
        .map_or_else(String::new, |name| name.to_lowercase());
    !(parts
        .iter()
        .any(|part| EXCLUDED_PARTS.contains(&part.to_lowercase().as_str()))
        || EXCLUDED_NAMES.contains(&name.as_str())
        || name.starts_with("test_")
        || name.contains(".test.")
        || name.contains(".spec.")
        || exclusions.iter().any(|pattern| glob(path, pattern)))
}

fn prefix(skill_path: &str) -> String {
    format!("{}/", skill_path.trim_end_matches('/'))
}

fn source_files(
    repository: &Path,
    revision: &str,
    request: &Request<'_>,
) -> Result<BTreeMap<String, Vec<u8>>, String> {
    let prefix = prefix(request.skill_path);
    let listed = git(
        repository,
        &[
            "ls-tree",
            "-r",
            "--name-only",
            revision,
            "--",
            request.skill_path,
        ],
    )?;
    let mut files = BTreeMap::new();
    for path in String::from_utf8_lossy(&listed).lines() {
        let relative = path.strip_prefix(&prefix).unwrap_or(path);
        if runtime_file(relative, request.exclusions, request.inclusions) {
            let content = git(repository, &["show", &format!("{revision}:{path}")])?;
            files.insert(relative.to_string(), content);
        }
    }
    Ok(files)
}

fn frozen_files(
    root: &Path,
    directory: &Path,
    request: &Request<'_>,
    files: &mut BTreeMap<String, Vec<u8>>,
) -> Result<(), String> {
    let entries = std::fs::read_dir(directory)
        .map_err(|error| format!("{}: {error}", directory.display()))?;
    for entry in entries {
        let path = entry
            .map_err(|error| format!("{}: {error}", directory.display()))?
            .path();
        if path.is_dir() && !path.is_symlink() {
            frozen_files(root, &path, request, files)?;
        } else if path.is_file() {
            let relative = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .into_owned();
            if runtime_file(&relative, request.exclusions, request.inclusions) {
                let content =
                    std::fs::read(&path).map_err(|error| format!("{}: {error}", path.display()))?;
                files.insert(relative, content);
            }
        }
    }
    Ok(())
}

/// The newest first-parent commit that touched a runtime file, as `(instant, commit, timestamp)`.
fn latest_runtime_change(
    request: &Request<'_>,
    commit: &str,
) -> Result<Option<(i64, String, String)>, String> {
    let prefix = prefix(request.skill_path);
    let history = git(
        request.repository,
        &[
            "log",
            "--diff-merges=first-parent",
            "--format=commit %H %cI",
            "--name-only",
            commit,
            "--",
            request.skill_path,
        ],
    )?;
    let mut latest: Option<(i64, String, String)> = None;
    let mut current: Option<(String, String)> = None;
    for line in String::from_utf8_lossy(&history).lines() {
        if let Some(rest) = line.strip_prefix("commit ") {
            let mut words = rest.split_whitespace();
            current = Some((
                words.next().unwrap_or_default().to_string(),
                words.next().unwrap_or_default().to_string(),
            ));
        } else if let (Some(relative), Some((changed, timestamp))) =
            (line.strip_prefix(&prefix), &current)
            && runtime_file(relative, request.exclusions, request.inclusions)
        {
            let instant = pysemantics::fromisoformat(timestamp)
                .map(|(instant, _)| instant)
                .ok_or_else(|| format!("git log gave an invalid commit date {timestamp:?}"))?;
            if latest
                .as_ref()
                .is_none_or(|(newest, _, _)| instant > *newest)
            {
                latest = Some((instant, changed.clone(), timestamp.clone()));
            }
        }
    }
    Ok(latest)
}

fn declared_version(skill: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(skill);
    let parts: Vec<&str> = text.splitn(3, "---").collect();
    if parts.len() != 3 || !parts[0].trim().is_empty() {
        return None;
    }
    let pattern = regex::Regex::new(r##"(?m)^version:[ \t]*['"]?([^ \t\r\n'"#]+)"##).ok()?;
    let version = pattern.captures(parts[1])?.get(1)?.as_str();
    (!["null", "~"].contains(&version.to_lowercase().as_str())).then(|| version.to_string())
}

fn string(text: &str) -> Value {
    Value::Str(text.to_string())
}

fn optional(text: Option<&str>) -> Value {
    text.map_or(Value::Null, string)
}

fn object(members: Vec<(&str, Value)>) -> Value {
    Value::Object(
        members
            .into_iter()
            .map(|(key, value)| (key.to_string(), value))
            .collect::<Object>(),
    )
}

fn release_metadata(
    request: &Request<'_>,
    release: &Value,
    files: &BTreeMap<String, Vec<u8>>,
    version: Option<String>,
) -> Result<(Value, Option<String>, String), String> {
    let field = |key: &str| match release {
        Value::Object(release) => release.get(key),
        _ => None,
    };
    let text = |key: &str| match field(key) {
        Some(Value::Str(text)) => Ok(text.clone()),
        _ => Err(format!("the release has no {key}")),
    };
    let tag = text("tag_name")?;
    let timestamp = match field("published_at") {
        Some(Value::Str(text)) if !text.is_empty() => Some(text.clone()),
        _ => None,
    };
    let draft = !matches!(
        field("draft"),
        None | Some(Value::Null | Value::Bool(false))
    );
    let zoned = timestamp.as_deref().is_some_and(|stamp| {
        pysemantics::fromisoformat(&stamp.replace('Z', "+00:00")).is_some_and(|(_, zoned)| zoned)
    });
    let (Some(timestamp), false, true) = (timestamp, draft, zoned) else {
        return Err("release must have a published timestamp with a timezone".to_string());
    };
    if source_files(request.repository, &tag, request)? != *files {
        return Err("published release runtime differs from the pinned skill".to_string());
    }
    let released = regex::Regex::new(r"(?:^|[-/])v?(\d+\.\d+\.\d+(?:-[\w.]+)?)$")
        .ok()
        .and_then(|pattern| pattern.captures(&tag))
        .and_then(|found| found.get(1))
        .map(|found| found.as_str().to_string());
    if let (Some(version), Some(released)) = (&version, &released)
        && version != released
    {
        return Err("declared skill version differs from release version".to_string());
    }
    let metadata = object(vec![
        ("tag", string(&tag)),
        ("timestamp", string(&timestamp)),
        ("url", string(&text("html_url")?)),
    ]);
    Ok((metadata, version.or(released), timestamp))
}

/// The provenance record of the skill at `request.skill_path` in `request.revision`.
pub fn collect(request: &Request<'_>) -> Result<Value, String> {
    let repository = request.repository;
    if git_text(repository, &["rev-parse", "--is-shallow-repository"])? == "true" {
        return Err("fetch complete history before collecting skill provenance".to_string());
    }
    let commit = git_text(
        repository,
        &["rev-parse", &format!("{}^{{commit}}", request.revision)],
    )?;
    let tree = git_text(
        repository,
        &["rev-parse", &format!("{commit}:{}", request.skill_path)],
    )?;
    let files = source_files(repository, &commit, request)?;
    let Some(skill) = files.get("SKILL.md") else {
        return Err("skill source must contain SKILL.md".to_string());
    };
    if let Some(frozen) = request.frozen {
        let mut copied = BTreeMap::new();
        frozen_files(frozen, frozen, request, &mut copied)?;
        let paths: BTreeSet<&String> = files.keys().chain(copied.keys()).collect();
        let mismatches: Vec<&str> = paths
            .into_iter()
            .filter(|path| files.get(*path) != copied.get(*path))
            .map(String::as_str)
            .collect();
        if !mismatches.is_empty() {
            return Err(format!(
                "frozen runtime differs from upstream: {}",
                mismatches.join(", ")
            ));
        }
    }
    let Some((_, changed, changed_at)) = latest_runtime_change(request, &commit)? else {
        return Err("no runtime-file commit found; fetch the complete skill history".to_string());
    };
    let mut version = declared_version(skill);
    let mut release = Value::Null;
    let mut dated = changed_at.clone();
    if let Some(given) = request.release {
        let (metadata, released, timestamp) = release_metadata(request, given, &files, version)?;
        (release, version, dated) = (metadata, released, timestamp);
    }
    let url = git_text(repository, &["remote", "get-url", "origin"])?;
    let url = url.strip_suffix(".git").unwrap_or(&url);
    let date: String = dated.chars().take(10).collect();
    let date_source = if matches!(release, Value::Null) {
        "commit"
    } else {
        "release"
    };
    let runtime_files = files
        .iter()
        .map(|(path, content)| {
            object(vec![
                ("path", string(path)),
                ("sha256", string(&sha256::hex_digest(content))),
            ])
        })
        .collect();
    let patterns = |patterns: &[String]| Value::Array(patterns.iter().map(|p| string(p)).collect());
    Ok(object(vec![
        ("skill_path", string(request.skill_path)),
        ("source_repository", string(url)),
        ("source_commit", string(&commit)),
        ("source_tree", string(&tree)),
        (
            "benchmark_tree",
            string(request.benchmark_tree.unwrap_or(&tree)),
        ),
        ("version", optional(version.as_deref())),
        ("release", release),
        (
            "last_runtime_change",
            object(vec![
                ("commit", string(&changed)),
                ("timestamp", string(&changed_at)),
            ]),
        ),
        ("date", string(&date)),
        ("date_source", string(date_source)),
        ("runtime_files", Value::Array(runtime_files)),
        ("excluded_patterns", patterns(request.exclusions)),
        ("included_patterns", patterns(request.inclusions)),
    ]))
}

fn member<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    match value {
        Value::Object(object) => object.get(key),
        _ => None,
    }
}

fn text_of(value: Option<&Value>) -> Option<&str> {
    match value {
        Some(Value::Str(text)) => Some(text),
        _ => None,
    }
}

/// The record's version and date, as the explorer labels the skill.
pub fn label(record: &Value) -> String {
    ["version", "date"]
        .iter()
        .filter_map(|key| text_of(member(record, key)).filter(|text| !text.is_empty()))
        .collect::<Vec<_>>()
        .join(" · ")
}

/// Write the record to a new `out`, and return the line the command prints: its path, hash and label.
pub fn write(out: &Path, record: &Value) -> Result<String, String> {
    use std::io::Write;
    let failed = |error: std::io::Error| format!("{}: {error}", out.display());
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent).map_err(failed)?;
    }
    let text = format!(
        "{}\n",
        pyjson::dumps(
            record,
            &Dumps {
                indent: Some(2),
                ..Dumps::default()
            }
        )
    );
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(out)
        .and_then(|mut file| file.write_all(text.as_bytes()))
        .map_err(failed)?;
    let summary = object(vec![
        ("path", string(&out.to_string_lossy())),
        ("sha256", string(&sha256::hex_digest(text.as_bytes()))),
        ("label", string(&label(record))),
    ]);
    Ok(pyjson::dumps(&summary, &Dumps::default()))
}

/// Check a frozen arm's `skill_provenance` pin: the record sits inside the run, still has its frozen hash and
/// names the arm's benchmark tree. An arm without a pin passes.
pub fn verify_pin(run: &Path, arm: &Value) -> Result<(), String> {
    let Some(pin) = member(arm, "skill_provenance").filter(|pin| !matches!(pin, Value::Null))
    else {
        return Ok(());
    };
    let (Some(relative), Some(sha)) =
        (text_of(member(pin, "path")), text_of(member(pin, "sha256")))
    else {
        return Err("skill provenance pin must name a path and a sha256".to_string());
    };
    let run = crate::review_isolation::resolve(run);
    let path = crate::review_isolation::resolve(&run.join(relative));
    if !path.starts_with(&run) {
        return Err("skill provenance must be inside the run directory".to_string());
    }
    let bytes = std::fs::read(&path).map_err(|error| format!("{}: {error}", path.display()))?;
    if sha256::hex_digest(&bytes) != sha {
        return Err("skill provenance no longer matches its frozen hash".to_string());
    }
    let record = pyjson::loads(
        &String::from_utf8_lossy(&bytes),
        pyjson::Duplicates::KeepLast,
    )
    .map_err(|error| format!("{}: {error}", path.display()))?;
    let same = match (
        member(&record, "benchmark_tree"),
        member(arm, "resolved_skill_tree"),
    ) {
        (Some(recorded), Some(resolved)) => recorded.py_eq(resolved),
        _ => false,
    };
    if !same {
        return Err("skill provenance identifies a different benchmark tree".to_string());
    }
    Ok(())
}
