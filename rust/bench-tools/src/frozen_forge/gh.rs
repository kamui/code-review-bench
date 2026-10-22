//! The stand-in `gh`: the pull request's view, diff and checks, and `gh api` reads, answered from the capture.

use super::{Ran, Served, compact, get, graphql, text};
use crate::pyjson::{self, Duplicates, Object, Value};
use std::fmt::Write as _;
use std::io::Write as _;
use std::path::Path;
use std::process::{Command, Stdio};

const PULL_FIELDS: [&str; 30] = [
    "additions",
    "author",
    "baseRefName",
    "baseRefOid",
    "body",
    "changedFiles",
    "closed",
    "closedAt",
    "closingIssuesReferences",
    "comments",
    "commits",
    "createdAt",
    "deletions",
    "files",
    "headRefName",
    "headRefOid",
    "headRepository",
    "headRepositoryOwner",
    "id",
    "isCrossRepository",
    "isDraft",
    "labels",
    "maintainerCanModify",
    "merged",
    "mergedAt",
    "number",
    "reviews",
    "state",
    "title",
    "url",
];
const WRITES: [&str; 12] = [
    "checkout",
    "close",
    "comment",
    "create",
    "edit",
    "lock",
    "merge",
    "ready",
    "reopen",
    "review",
    "unlock",
    "update-branch",
];
const REST_PAGE: usize = 30;

fn fail(message: impl Into<String>) -> Ran {
    Ran {
        stdout: Vec::new(),
        stderr: format!("{}\n", message.into()),
        code: 1,
    }
}

fn printed(stdout: impl Into<Vec<u8>>) -> Ran {
    Ran {
        stdout: stdout.into(),
        stderr: String::new(),
        code: 0,
    }
}

fn unsupported(argv: &[String]) -> Ran {
    fail(format!(
        "gh {}: not answered by the frozen forge. It serves `gh pr view`, `gh pr diff`, `gh pr checks`, `gh api graphql` and `gh api repos/{{owner}}/{{repo}}/pulls/<n>/comments` for this task's pull request as it stood at the cutoff.",
        argv.join(" ")
    ))
}

fn refused(what: &str) -> Ran {
    fail(format!(
        "{what}: refused. The frozen forge is read-only: it serves the pull request as it stood at the cutoff and accepts no writes."
    ))
}

/// Flags and positional words, with each flag's value; a flag in `switches` takes none.
struct Parsed {
    flags: Vec<(String, Option<String>)>,
    positional: Vec<String>,
}

fn parse(words: &[String], switches: &[&str], short: &[(char, &str)]) -> Result<Parsed, String> {
    let mut parsed = Parsed {
        flags: Vec::new(),
        positional: Vec::new(),
    };
    let mut at = 0;
    while at < words.len() {
        let word = &words[at];
        at += 1;
        let (name, inline) = if let Some(long) = word.strip_prefix("--") {
            if long.is_empty() {
                parsed.positional.extend(words[at..].iter().cloned());
                break;
            }
            match long.split_once('=') {
                Some((name, value)) => (name.to_string(), Some(value.to_string())),
                None => (long.to_string(), None),
            }
        } else if let Some(letters) = word.strip_prefix('-').filter(|rest| !rest.is_empty()) {
            let mut chars = letters.chars();
            let letter = chars.next().unwrap_or('-');
            let name = short
                .iter()
                .find(|(key, _)| *key == letter)
                .map(|(_, long)| (*long).to_string())
                .ok_or_else(|| format!("unknown shorthand flag: '{letter}' in {word}"))?;
            let rest: String = chars.collect();
            (
                name,
                (!rest.is_empty()).then_some(rest.trim_start_matches('=').to_string()),
            )
        } else {
            parsed.positional.push(word.clone());
            continue;
        };
        if switches.contains(&name.as_str()) {
            parsed.flags.push((name, inline));
            continue;
        }
        let value = match inline {
            Some(value) => value,
            None => {
                let value = words
                    .get(at)
                    .cloned()
                    .ok_or_else(|| format!("flag needs an argument: --{name}"))?;
                at += 1;
                value
            }
        };
        parsed.flags.push((name, Some(value)));
    }
    Ok(parsed)
}

impl Parsed {
    fn value(&self, name: &str) -> Option<&str> {
        self.flags
            .iter()
            .rev()
            .find(|(flag, _)| flag == name)
            .and_then(|(_, value)| value.as_deref())
    }

    fn has(&self, name: &str) -> bool {
        self.flags.iter().any(|(flag, _)| flag == name)
    }

    fn values(&self, name: &str) -> Vec<&str> {
        self.flags
            .iter()
            .filter(|(flag, _)| flag == name)
            .filter_map(|(_, value)| value.as_deref())
            .collect()
    }

    fn only(&self, known: &[&str]) -> Result<(), String> {
        match self
            .flags
            .iter()
            .find(|(flag, _)| !known.contains(&flag.as_str()))
        {
            Some((flag, _)) => Err(format!("--{flag} is not answered by the frozen forge")),
            None => Ok(()),
        }
    }
}

pub fn run(served: &Served, cwd: &Path, argv: &[String]) -> Ran {
    let words: Vec<&str> = argv.iter().map(String::as_str).collect();
    match words.as_slice() {
        ["--version" | "version", ..] => printed(format!(
            "gh (frozen forge stand-in for {}#{})\n",
            served.capture.repository, served.capture.number
        )),
        ["auth", "status", ..] => printed(format!(
            "github.com\n  - frozen forge stand-in: read-only answers about {}#{} as it stood at the cutoff\n",
            served.capture.repository, served.capture.number
        )),
        ["pr", "view", ..] => answer(pr_view(served, cwd, &argv[2..])),
        ["pr", "diff", ..] => answer(pr_diff(served, cwd, &argv[2..])),
        ["pr", "checks", ..] => answer(pr_checks(served, cwd, &argv[2..])),
        ["pr", write, ..] if WRITES.contains(write) => refused(&format!("gh pr {write}")),
        [
            "issue" | "repo" | "release" | "gist" | "label" | "workflow" | "run" | "secret"
            | "variable",
            write,
            ..,
        ] if matches!(
            *write,
            "create"
                | "edit"
                | "comment"
                | "close"
                | "delete"
                | "reopen"
                | "fork"
                | "rename"
                | "set"
                | "transfer"
                | "lock"
                | "pin"
        ) =>
        {
            refused(&format!("gh {} {write}", words[0]))
        }
        ["api", ..] => answer(api(served, cwd, &argv[1..])),
        _ => unsupported(argv),
    }
}

fn answer(result: Result<Ran, String>) -> Ran {
    result.unwrap_or_else(fail)
}

fn repository_flag(served: &Served, parsed: &Parsed) -> Result<(), String> {
    match parsed.value("repo") {
        Some(slug) if !slug.eq_ignore_ascii_case(&served.capture.repository) => Err(format!(
            "{slug} is not in the frozen forge; it serves only {}",
            served.capture.repository
        )),
        _ => Ok(()),
    }
}

fn not_found(selector: &str) -> String {
    format!("no pull requests found for branch \"{selector}\"")
}

fn config(cwd: &Path, key: &str) -> Option<String> {
    let done = Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(["config", "--get", key])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    done.status
        .success()
        .then(|| String::from_utf8_lossy(&done.stdout).trim().to_string())
}

/// Check that `selector` names the served pull request, as `gh pr` resolves a number, URL or branch.
fn select(served: &Served, cwd: &Path, selector: Option<&str>) -> Result<(), String> {
    let number = served.capture.number;
    let head_ref = served.field("head_ref");
    let branch = match selector {
        None => {
            let done = Command::new("git")
                .arg("-C")
                .arg(cwd)
                .args(["symbolic-ref", "--quiet", "--short", "HEAD"])
                .stdin(Stdio::null())
                .stderr(Stdio::null())
                .output()
                .map_err(|error| format!("cannot run git: {error}"))?;
            if !done.status.success() {
                return Err(
                    "could not determine the current branch: not on a branch in a git repository"
                        .to_string(),
                );
            }
            String::from_utf8_lossy(&done.stdout).trim().to_string()
        }
        Some(selector) => {
            let digits = selector.strip_prefix('#').unwrap_or(selector);
            if !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit()) {
                return if digits.parse() == Ok(number) {
                    Ok(())
                } else {
                    Err(format!(
                        "GraphQL: Could not resolve to a PullRequest with the number of {digits}. (repository.pullRequest)"
                    ))
                };
            }
            if let Some(rest) = selector
                .strip_prefix("https://github.com/")
                .or_else(|| selector.strip_prefix("http://github.com/"))
            {
                let parts: Vec<&str> = rest.split('/').collect();
                let slug = parts
                    .get(..2)
                    .map(|parts| parts.join("/"))
                    .unwrap_or_default();
                if parts.get(2) != Some(&"pull")
                    || !slug.eq_ignore_ascii_case(&served.capture.repository)
                {
                    return Err(format!(
                        "{selector} is not a pull request of {}",
                        served.capture.repository
                    ));
                }
                return select(served, cwd, parts.get(3).copied());
            }
            selector.to_string()
        }
    };
    let head_owner = served.field("head_owner");
    let named = branch == head_ref
        || branch.split_once(':').is_some_and(|(owner, name)| {
            name == head_ref && owner.eq_ignore_ascii_case(&head_owner)
        });
    let tracked = config(cwd, &format!("branch.{branch}.merge")).is_some_and(|merge| {
        merge == format!("refs/pull/{number}/head") || merge == format!("refs/heads/{head_ref}")
    });
    if named || tracked {
        Ok(())
    } else {
        Err(not_found(&branch))
    }
}

fn login(record: &Value) -> Value {
    match get(record, "login") {
        Value::Null => Value::Null,
        login => Value::Object(Object::from_iter([("login".to_string(), login.clone())])),
    }
}

fn nodes(value: &Value) -> &[Value] {
    get(value, "nodes").as_array().unwrap_or_default()
}

fn pick(record: &Value, keys: &[&str]) -> Value {
    Value::Object(
        keys.iter()
            .map(|key| {
                let value = get(record, key);
                let value = if *key == "author" {
                    login(value)
                } else {
                    value.clone()
                };
                ((*key).to_string(), value)
            })
            .collect(),
    )
}

/// The changed files of the pull request: path, line counts and change type, from the frozen upstream copy.
fn files(served: &Served) -> Result<Value, String> {
    let range = [served.field("merge_base"), served.field("head")];
    let numstat = served.origin_git(&[
        "diff",
        "--no-ext-diff",
        "-M",
        "--numstat",
        "-z",
        &range[0],
        &range[1],
    ])?;
    let status = served.origin_git(&[
        "diff",
        "--no-ext-diff",
        "-M",
        "--name-status",
        "-z",
        &range[0],
        &range[1],
    ])?;
    let words = |raw: &[u8]| -> Vec<String> {
        raw.split(|byte| *byte == 0)
            .filter(|word| !word.is_empty())
            .map(|word| String::from_utf8_lossy(word).into_owned())
            .collect()
    };
    let mut kinds = Vec::new();
    let status = words(&status);
    let mut at = 0;
    while at < status.len() {
        let code = status[at].chars().next().unwrap_or('M');
        let paths = if matches!(code, 'R' | 'C') { 2 } else { 1 };
        kinds.push(match code {
            'A' => "ADDED",
            'D' => "DELETED",
            'R' => "RENAMED",
            'C' => "COPIED",
            'T' => "CHANGED",
            _ => "MODIFIED",
        });
        at += 1 + paths;
    }
    let numstat = words(&numstat);
    let mut out = Vec::new();
    let mut at = 0;
    while at < numstat.len() {
        let fields: Vec<&str> = numstat[at].splitn(3, '\t').collect();
        at += 1;
        let path = match fields.get(2) {
            Some(path) if !path.is_empty() => (*path).to_string(),
            _ => {
                at += 2;
                numstat.get(at - 1).cloned().unwrap_or_default()
            }
        };
        let count = |index: usize| {
            Value::Int(
                fields
                    .get(index)
                    .and_then(|word| word.parse::<u64>().ok())
                    .unwrap_or(0)
                    .to_string(),
            )
        };
        out.push(Value::Object(Object::from_iter([
            ("additions".to_string(), count(0)),
            (
                "changeType".to_string(),
                text(kinds.get(out.len()).copied().unwrap_or("MODIFIED")),
            ),
            ("deletions".to_string(), count(1)),
            ("path".to_string(), text(&path)),
        ])));
    }
    Ok(Value::Array(out))
}

fn pull_field(served: &Served, name: &str) -> Result<Value, String> {
    let pull = &served.capture.pull;
    Ok(match name {
        "author" | "headRepositoryOwner" => login(get(pull, name)),
        "headRepository" => match get(pull, name) {
            Value::Null => Value::Null,
            repository => {
                let slug = get(repository, "nameWithOwner").py_str();
                Value::Object(Object::from_iter([
                    (
                        "name".to_string(),
                        text(slug.split_once('/').map_or(slug.as_str(), |(_, name)| name)),
                    ),
                    ("nameWithOwner".to_string(), text(&slug)),
                ]))
            }
        },
        "labels" => Value::Array(
            nodes(get(pull, "labels"))
                .iter()
                .map(|label| pick(label, &["color", "description", "name"]))
                .collect(),
        ),
        "comments" => Value::Array(
            nodes(get(pull, "comments"))
                .iter()
                .map(|comment| {
                    pick(
                        comment,
                        &[
                            "author",
                            "authorAssociation",
                            "body",
                            "createdAt",
                            "id",
                            "url",
                        ],
                    )
                })
                .collect(),
        ),
        "reviews" => Value::Array(
            nodes(get(pull, "reviews"))
                .iter()
                .map(|review| {
                    pick(
                        review,
                        &[
                            "author",
                            "authorAssociation",
                            "body",
                            "commit",
                            "id",
                            "state",
                            "submittedAt",
                        ],
                    )
                })
                .collect(),
        ),
        "commits" => Value::Array(
            nodes(get(pull, "commits"))
                .iter()
                .map(|node| {
                    let commit = get(node, "commit");
                    let author = get(commit, "author");
                    Value::Object(Object::from_iter([
                        (
                            "authoredDate".to_string(),
                            get(commit, "authoredDate").clone(),
                        ),
                        (
                            "authors".to_string(),
                            Value::Array(vec![Value::Object(Object::from_iter([
                                ("email".to_string(), get(author, "email").clone()),
                                (
                                    "login".to_string(),
                                    get(get(author, "user"), "login").clone(),
                                ),
                                ("name".to_string(), get(author, "name").clone()),
                            ]))]),
                        ),
                        (
                            "committedDate".to_string(),
                            get(commit, "committedDate").clone(),
                        ),
                        (
                            "messageBody".to_string(),
                            get(commit, "messageBody").clone(),
                        ),
                        (
                            "messageHeadline".to_string(),
                            get(commit, "messageHeadline").clone(),
                        ),
                        ("oid".to_string(), get(commit, "oid").clone()),
                    ]))
                })
                .collect(),
        ),
        "closingIssuesReferences" => Value::Array(
            nodes(get(pull, "closingIssuesReferences"))
                .iter()
                .map(|issue| {
                    let slug = get(get(issue, "repository"), "nameWithOwner")
                        .as_str()
                        .map_or_else(|| served.capture.repository.clone(), str::to_string);
                    let (owner, name) = slug.split_once('/').unwrap_or((slug.as_str(), ""));
                    Value::Object(Object::from_iter([
                        ("id".to_string(), get(issue, "id").clone()),
                        ("number".to_string(), get(issue, "number").clone()),
                        (
                            "repository".to_string(),
                            Value::Object(Object::from_iter([
                                ("name".to_string(), text(name)),
                                (
                                    "owner".to_string(),
                                    Value::Object(Object::from_iter([(
                                        "login".to_string(),
                                        text(owner),
                                    )])),
                                ),
                            ])),
                        ),
                        ("url".to_string(), get(issue, "url").clone()),
                    ]))
                })
                .collect(),
        ),
        "files" => files(served)?,
        _ => get(pull, name).clone(),
    })
}

fn json_output(served: &Served, value: &Value, jq: Option<&str>) -> Result<Ran, String> {
    let body = compact(value);
    match jq {
        None => Ok(printed(format!("{body}\n"))),
        Some(filter) => run_jq(served, &body, filter),
    }
}

/// Apply `filter` as `gh --jq` does: strings print raw, null prints an empty line, other values print compact.
fn run_jq(served: &Served, body: &str, filter: &str) -> Result<Ran, String> {
    let mut child = Command::new(served.field("jq"))
        .args(["-c", filter])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("cannot run jq: {error}"))?;
    child
        .stdin
        .take()
        .ok_or("cannot write to jq")?
        .write_all(body.as_bytes())
        .map_err(|error| format!("cannot write to jq: {error}"))?;
    let done = child
        .wait_with_output()
        .map_err(|error| format!("jq failed: {error}"))?;
    if !done.status.success() {
        return Err(String::from_utf8_lossy(&done.stderr).trim().to_string());
    }
    let mut out = String::new();
    for line in String::from_utf8_lossy(&done.stdout).lines() {
        match pyjson::loads(line, Duplicates::KeepLast) {
            Ok(Value::Str(raw)) => out.push_str(&raw),
            Ok(Value::Null) => {}
            _ => out.push_str(line),
        }
        out.push('\n');
    }
    Ok(printed(out))
}

fn pr_view(served: &Served, cwd: &Path, words: &[String]) -> Result<Ran, String> {
    let parsed = parse(
        words,
        &["comments", "web"],
        &[
            ('q', "jq"),
            ('R', "repo"),
            ('c', "comments"),
            ('w', "web"),
            ('t', "template"),
        ],
    )?;
    if parsed.has("web") {
        return Ok(refused("gh pr view --web"));
    }
    parsed.only(&["json", "jq", "repo"])?;
    repository_flag(served, &parsed)?;
    if parsed.positional.len() > 1 {
        return Err(format!(
            "accepts at most 1 arg(s), received {}",
            parsed.positional.len()
        ));
    }
    select(served, cwd, parsed.positional.first().map(String::as_str))?;
    let Some(fields) = parsed.value("json") else {
        if parsed.has("jq") {
            return Err("cannot use `--jq` without specifying `--json`".to_string());
        }
        return Ok(printed(view_text(served)));
    };
    let mut wanted: Vec<&str> = fields
        .split(',')
        .map(str::trim)
        .filter(|field| !field.is_empty())
        .collect();
    if wanted.is_empty() {
        return Err(format!(
            "Specify one or more comma-separated fields for `--json`; the frozen forge serves: {}",
            PULL_FIELDS.join(", ")
        ));
    }
    if let Some(unknown) = wanted.iter().find(|field| !PULL_FIELDS.contains(field)) {
        return Err(format!(
            "the field {unknown} is not in the frozen capture; the frozen forge serves: {}",
            PULL_FIELDS.join(", ")
        ));
    }
    wanted.sort_unstable();
    wanted.dedup();
    let mut out = Object::default();
    for field in wanted {
        out.insert(field.to_string(), pull_field(served, field)?);
    }
    json_output(served, &Value::Object(out), parsed.value("jq"))
}

fn view_text(served: &Served) -> String {
    let pull = &served.capture.pull;
    let labels: Vec<String> = nodes(get(pull, "labels"))
        .iter()
        .map(|label| get(label, "name").py_str())
        .collect();
    format!(
        "title:\t{}\nstate:\t{}\nauthor:\t{}\nlabels:\t{}\nnumber:\t{}\nurl:\t{}\nadditions:\t{}\ndeletions:\t{}\n--\n{}\n",
        get(pull, "title").py_str(),
        get(pull, "state").py_str(),
        get(get(pull, "author"), "login").py_str(),
        labels.join(", "),
        get(pull, "number").py_str(),
        get(pull, "url").py_str(),
        get(pull, "additions").py_str(),
        get(pull, "deletions").py_str(),
        get(pull, "body").py_str(),
    )
}

fn pr_diff(served: &Served, cwd: &Path, words: &[String]) -> Result<Ran, String> {
    let parsed = parse(words, &["name-only", "patch", "web"], &[('R', "repo")])?;
    if parsed.has("web") {
        return Ok(refused("gh pr diff --web"));
    }
    parsed.only(&["color", "name-only", "repo"])?;
    repository_flag(served, &parsed)?;
    if parsed.positional.len() > 1 {
        return Err(format!(
            "accepts at most 1 arg(s), received {}",
            parsed.positional.len()
        ));
    }
    select(served, cwd, parsed.positional.first().map(String::as_str))?;
    let color = match parsed.value("color").unwrap_or("auto") {
        "always" => "--color=always",
        "never" | "auto" => "--no-color",
        other => return Err(format!("invalid argument \"{other}\" for \"--color\" flag")),
    };
    let (base, head) = (served.field("merge_base"), served.field("head"));
    let mut arguments = vec!["diff", "--no-ext-diff", "-M", color];
    if parsed.has("name-only") {
        arguments.push("--name-only");
    }
    arguments.extend([base.as_str(), head.as_str()]);
    Ok(printed(served.origin_git(&arguments)?))
}

fn duration(start: &Value, end: &Value) -> String {
    let moment = |value: &Value| value.as_str().and_then(crate::build_packet::instant);
    let Some(elapsed) = moment(start)
        .zip(moment(end))
        .map(|(start, end)| start.seconds_until(end))
        .filter(|elapsed| *elapsed >= 0)
    else {
        return "0".to_string();
    };
    let (hours, minutes, rest) = (elapsed / 3600, elapsed % 3600 / 60, elapsed % 60);
    match (hours, minutes) {
        (0, 0) => format!("{rest}s"),
        (0, _) => format!("{minutes}m{rest}s"),
        _ => format!("{hours}h{minutes}m{rest}s"),
    }
}

fn pr_checks(served: &Served, cwd: &Path, words: &[String]) -> Result<Ran, String> {
    let parsed = parse(
        words,
        &["watch", "web", "required", "fail-fast"],
        &[('R', "repo"), ('w', "web")],
    )?;
    if parsed.has("web") {
        return Ok(refused("gh pr checks --web"));
    }
    parsed.only(&["repo"])?;
    repository_flag(served, &parsed)?;
    select(served, cwd, parsed.positional.first().map(String::as_str))?;
    let head = nodes(get(&served.capture.pull, "commits"))
        .last()
        .map(|node| get(node, "commit"))
        .cloned()
        .unwrap_or(Value::Null);
    let mut rows: Vec<(String, &str, String, String, String)> = Vec::new();
    for suite in nodes(get(&head, "checkSuites")) {
        for run in nodes(get(suite, "checkRuns")) {
            let bucket = if get(run, "status").as_str() != Some("COMPLETED") {
                "pending"
            } else {
                match get(run, "conclusion").as_str() {
                    Some("SUCCESS") => "pass",
                    Some("SKIPPED" | "NEUTRAL") => "skipping",
                    Some("CANCELLED") => "cancel",
                    _ => "fail",
                }
            };
            rows.push((
                get(run, "name").py_str(),
                bucket,
                duration(get(run, "startedAt"), get(run, "completedAt")),
                get(run, "detailsUrl").as_str().unwrap_or("").to_string(),
                get(run, "title").as_str().unwrap_or("").to_string(),
            ));
        }
    }
    for context in get(get(&head, "status"), "contexts")
        .as_array()
        .unwrap_or_default()
    {
        let bucket = match get(context, "state").as_str() {
            Some("SUCCESS") => "pass",
            Some("PENDING" | "EXPECTED") => "pending",
            _ => "fail",
        };
        rows.push((
            get(context, "context").py_str(),
            bucket,
            "0".to_string(),
            get(context, "targetUrl").as_str().unwrap_or("").to_string(),
            get(context, "description")
                .as_str()
                .unwrap_or("")
                .to_string(),
        ));
    }
    if rows.is_empty() {
        return Err(format!(
            "no checks reported on the '{}' branch",
            served.field("head_ref")
        ));
    }
    rows.sort_by(|left, right| left.0.cmp(&right.0));
    let out: String = rows.iter().fold(
        String::new(),
        |mut out, (name, bucket, elapsed, url, description)| {
            let _ = writeln!(out, "{name}\t{bucket}\t{elapsed}\t{url}\t{description}");
            out
        },
    );
    let code = if rows.iter().any(|row| matches!(row.1, "fail" | "cancel")) {
        1
    } else if rows.iter().any(|row| row.1 == "pending") {
        8
    } else {
        0
    };
    Ok(Ran {
        stdout: out.into_bytes(),
        stderr: String::new(),
        code,
    })
}

/// `gh api` fills `{owner}`, `{repo}` and `{branch}` from the current repository in the endpoint and in `-F` values.
fn filled(served: &Served, cwd: &Path, raw: &str) -> String {
    let (owner, name) = served
        .capture
        .repository
        .split_once('/')
        .unwrap_or_default();
    let raw = raw.replace("{owner}", owner).replace("{repo}", name);
    if !raw.contains("{branch}") {
        return raw;
    }
    let branch = Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(["symbolic-ref", "--short", "HEAD"])
        .output()
        .map(|done| String::from_utf8_lossy(&done.stdout).trim().to_string())
        .unwrap_or_default();
    raw.replace("{branch}", &branch)
}

fn typed(served: &Served, cwd: &Path, raw: &str) -> Result<Value, String> {
    Ok(match raw {
        "true" => Value::Bool(true),
        "false" => Value::Bool(false),
        "null" => Value::Null,
        _ if raw.parse::<i64>().is_ok() => Value::Int(raw.to_string()),
        _ => match raw.strip_prefix('@') {
            Some("-") => {
                return Err(
                    "reading a field from stdin is not supported by the frozen forge".to_string(),
                );
            }
            Some(path) => Value::Str(
                std::fs::read_to_string(path).map_err(|error| format!("{path}: {error}"))?,
            ),
            None => Value::Str(filled(served, cwd, raw)),
        },
    })
}

fn api(served: &Served, cwd: &Path, words: &[String]) -> Result<Ran, String> {
    let parsed = parse(
        words,
        &["paginate", "silent", "include", "verbose", "slurp"],
        &[
            ('f', "raw-field"),
            ('F', "field"),
            ('X', "method"),
            ('q', "jq"),
            ('H', "header"),
            ('i', "include"),
            ('p', "preview"),
            ('t', "template"),
        ],
    )?;
    parsed.only(&[
        "raw-field",
        "field",
        "method",
        "jq",
        "header",
        "paginate",
        "silent",
        "input",
        "hostname",
        "cache",
        "preview",
    ])?;
    if let Some(host) = parsed
        .value("hostname")
        .filter(|host| *host != "github.com")
    {
        return Err(format!("{host} is not in the frozen forge"));
    }
    let [endpoint] = parsed.positional.as_slice() else {
        return Err("gh api needs exactly one endpoint".to_string());
    };
    let mut fields = Object::default();
    for raw in parsed.values("raw-field") {
        let (key, value) = raw
            .split_once('=')
            .ok_or_else(|| format!("field {raw} has no '='"))?;
        fields.insert(key.to_string(), text(value));
    }
    for raw in parsed.values("field") {
        let (key, value) = raw
            .split_once('=')
            .ok_or_else(|| format!("field {raw} has no '='"))?;
        fields.insert(key.to_string(), typed(served, cwd, value)?);
    }
    let method = parsed.value("method").map(str::to_ascii_uppercase);
    let ran = if endpoint == "graphql" {
        if method.as_deref().is_some_and(|method| method != "POST") {
            return Err("graphql takes POST".to_string());
        }
        if parsed.has("paginate") {
            return Err("--paginate on graphql is not answered by the frozen forge".to_string());
        }
        graphql_call(served, &parsed, fields)?
    } else {
        let method = method.unwrap_or_else(|| {
            if fields.is_empty() && !parsed.has("input") {
                "GET".to_string()
            } else {
                "POST".to_string()
            }
        });
        if method != "GET" {
            return Ok(refused(&format!("gh api --method {method} {endpoint}")));
        }
        rest(served, cwd, endpoint, &fields, &parsed)?
    };
    if parsed.has("silent") {
        return Ok(Ran {
            stdout: Vec::new(),
            ..ran
        });
    }
    Ok(ran)
}

fn graphql_call(served: &Served, parsed: &Parsed, mut fields: Object) -> Result<Ran, String> {
    if let Some(input) = parsed.value("input") {
        let body = pyjson::loads(
            &std::fs::read_to_string(input).map_err(|error| format!("{input}: {error}"))?,
            Duplicates::KeepLast,
        )
        .map_err(|error| format!("{input}: {error}"))?;
        if let Some(query) = get(&body, "query").as_str() {
            fields.insert("query".to_string(), text(query));
        }
        if let Some(variables) = get(&body, "variables").as_object() {
            for (key, value) in variables.iter() {
                fields.insert(key.clone(), value.clone());
            }
        }
    }
    let query = fields
        .get("query")
        .and_then(Value::as_str)
        .ok_or("gh api graphql needs a query field")?
        .to_string();
    let variables: Object = fields
        .iter()
        .filter(|(key, _)| key.as_str() != "query" && key.as_str() != "operationName")
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();
    let (response, failed) = graphql::answer(&served.capture, &query, &variables)?;
    let mut ran = json_output(served, &response, parsed.value("jq"))?;
    if failed {
        let messages: Vec<String> = get(&response, "errors")
            .as_array()
            .unwrap_or_default()
            .iter()
            .map(|error| format!("gh: {}\n", get(error, "message").py_str()))
            .collect();
        ran.stderr = messages.concat();
        ran.code = 1;
    }
    Ok(ran)
}

fn rest(
    served: &Served,
    cwd: &Path,
    endpoint: &str,
    fields: &Object,
    parsed: &Parsed,
) -> Result<Ran, String> {
    let path = filled(
        served,
        cwd,
        endpoint
            .trim_start_matches("https://api.github.com")
            .trim_start_matches('/'),
    );
    let (path, query) = path
        .split_once('?')
        .map_or((path.as_str(), ""), |(path, query)| (path, query));
    let mut parameters: Vec<(String, String)> = query
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect();
    parameters.extend(
        fields
            .iter()
            .map(|(key, value)| (key.clone(), value.py_str())),
    );
    let parameter = |key: &str| {
        parameters
            .iter()
            .rev()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value.clone())
    };
    let parts: Vec<&str> = path.split('/').collect();
    let number = served.capture.number.to_string();
    let records = match parts.as_slice() {
        ["repos", repo_owner, repo_name, "pulls", pull, "comments"]
            if format!("{repo_owner}/{repo_name}")
                .eq_ignore_ascii_case(&served.capture.repository) =>
        {
            if *pull != number {
                return Err(format!(
                    "gh: Not Found (HTTP 404): {path} is not in the frozen forge"
                ));
            }
            review_comments(served)
        }
        _ => {
            return Err(format!(
                "gh api {path}: not answered by the frozen forge; it serves `gh api graphql` and repos/{}/pulls/{number}/comments",
                served.capture.repository
            ));
        }
    };
    let per_page = parameter("per_page")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(REST_PAGE)
        .clamp(1, 100);
    let first_page = parameter("page")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(1)
        .max(1);
    let pages: Vec<&[Value]> = records.chunks(per_page).collect();
    let selected: Vec<&[Value]> = if parsed.has("paginate") {
        pages.iter().skip(first_page - 1).copied().collect()
    } else {
        vec![pages.get(first_page - 1).copied().unwrap_or(&[])]
    };
    let mut out = Ran {
        stdout: Vec::new(),
        stderr: String::new(),
        code: 0,
    };
    for page in if selected.is_empty() {
        vec![&[][..]]
    } else {
        selected
    } {
        let ran = json_output(served, &Value::Array(page.to_vec()), parsed.value("jq"))?;
        out.stdout.extend(ran.stdout);
    }
    Ok(out)
}

/// The pull request's review comments in the REST shape, oldest first.
fn review_comments(served: &Served) -> Vec<Value> {
    let int = |value: &Value| match value {
        Value::Str(digits)
            if digits.bytes().all(|byte| byte.is_ascii_digit()) && !digits.is_empty() =>
        {
            Value::Int(digits.clone())
        }
        other => other.clone(),
    };
    let mut out: Vec<(u64, Value)> = Vec::new();
    for thread in nodes(get(&served.capture.pull, "reviewThreads")) {
        for comment in nodes(get(thread, "comments")) {
            let id = get(comment, "fullDatabaseId").py_str();
            let mut record = Object::default();
            let mut put = |key: &str, value: Value| record.insert(key.to_string(), value);
            put(
                "url",
                text(&format!(
                    "https://api.github.com/repos/{}/pulls/comments/{id}",
                    served.capture.repository
                )),
            );
            put(
                "pull_request_review_id",
                int(get(get(comment, "pullRequestReview"), "fullDatabaseId")),
            );
            put("id", int(get(comment, "fullDatabaseId")));
            put("node_id", get(comment, "id").clone());
            put("diff_hunk", get(comment, "diffHunk").clone());
            put("path", get(comment, "path").clone());
            put("position", get(comment, "position").clone());
            put(
                "original_position",
                get(comment, "originalPosition").clone(),
            );
            put("commit_id", get(get(comment, "commit"), "oid").clone());
            put(
                "original_commit_id",
                get(get(comment, "originalCommit"), "oid").clone(),
            );
            put("user", login(get(comment, "author")));
            put("body", get(comment, "body").clone());
            put("created_at", get(comment, "createdAt").clone());
            put("updated_at", Value::Null);
            put("html_url", get(comment, "url").clone());
            put(
                "author_association",
                get(comment, "authorAssociation").clone(),
            );
            put("start_line", get(comment, "startLine").clone());
            put(
                "original_start_line",
                get(comment, "originalStartLine").clone(),
            );
            put("start_side", get(thread, "startDiffSide").clone());
            put("line", get(comment, "line").clone());
            put("original_line", get(comment, "originalLine").clone());
            put("side", get(thread, "diffSide").clone());
            put(
                "subject_type",
                text(&get(comment, "subjectType").py_str().to_ascii_lowercase()),
            );
            if let reply @ Value::Str(_) = get(get(comment, "replyTo"), "fullDatabaseId") {
                put("in_reply_to_id", int(reply));
            }
            out.push((id.parse().unwrap_or(u64::MAX), Value::Object(record)));
        }
    }
    out.sort_by_key(|(id, _)| *id);
    out.into_iter().map(|(_, record)| record).collect()
}
