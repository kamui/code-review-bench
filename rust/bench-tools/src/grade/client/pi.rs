//! The Pi grader client: its preflight and efforts, the agent directory and command line a grading session runs
//! under, the probe that runs it against a local fake OpenRouter API before a paid session, the pinned package a
//! queue runs, and the session evidence a dispatch records.

use super::{
    FakeApi, S, Scratch, content_text, exit_code_zero, get, get_str, git_markers, io, json, mkdir,
    object, parse, path_text, pinned_version, price, probe_workspace, read, read_json, require,
    strings, text, write, written,
};
use crate::grading_tools;
use crate::pyjson::{Object, Value};
use crate::{review_isolation, sha256, usage};
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

pub const NAME: &str = "pi";
/// The only provider whose request shape the probe's fake API answers.
const PROVIDER: &str = "openrouter";
/// The grading `run` tool allows a command five minutes; Pi's default MCP request timeout is one minute.
const MCP_TIMEOUT_SECONDS: u64 = 600;
const MARKER: &str = "PI-GRADING-PROBE-MARKER";
/// Where each planted marker sits, by the label it carries.
const MARKED: [&str; 6] = [
    "ANCESTOR",
    "PROJECT",
    "USER",
    "PROJECT-SYSTEM",
    "PROMPT",
    "SKILL",
];
/// What a pinned queue copies of the package: its manifest, which names the version, and the self-contained bundle.
const PINNED: [&str; 2] = ["package.json", "dist/bundle"];
pub const LAUNCHER: &str = "dist/bundle/cli.js";

/// The grading tools as Pi names an MCP server's tools, sorted.
pub fn tool_names() -> Vec<String> {
    let mut names: Vec<String> = grading_tools::TOOLS
        .iter()
        .map(|name| format!("mcp__grading__{name}"))
        .collect();
    names.sort();
    names
}

/// The model id Pi is given as `provider/id`, split, for the one provider grading supports.
pub fn split(model: &str) -> S<(&str, &str)> {
    match model.split_once('/') {
        Some((PROVIDER, id)) if !id.is_empty() => Ok((PROVIDER, id)),
        _ => Err(format!(
            "Pi grading supports {PROVIDER} models named {PROVIDER}/<id>, not {model}"
        )),
    }
}

pub fn agent_directory(home: &Path) -> PathBuf {
    home.join(".pi/agent")
}

pub fn sessions(home: &Path) -> PathBuf {
    agent_directory(home).join("sessions")
}

/// Write the agent directory's `mcp.json` and `settings.json`: the grading server, declared to the model directly,
/// and no codemode, cache warming, telemetry or project trust.
pub fn configure(agent: &Path, command: &Path, arguments: &[String]) -> S<()> {
    mkdir(agent)?;
    let server = object(vec![
        ("command", path_text(command)),
        ("args", strings(arguments.iter().map(String::as_str))),
        ("exposure", text("direct")),
        ("timeout", Value::Int(MCP_TIMEOUT_SECONDS.to_string())),
    ]);
    write(
        &agent.join("mcp.json"),
        json(&object(vec![
            ("mcpServers", object(vec![("grading", server)])),
            ("autoEnableCodemode", Value::Bool(false)),
        ])),
    )?;
    write(
        &agent.join("settings.json"),
        r#"{"defaultProjectTrust": "never", "cacheWarming": "off", "enableInstallTelemetry": false, "quietStartup": true}"#,
    )
}

/// A Pi process's environment: nothing inherited but `PATH`, the fresh home and agent directory, and no network
/// activity beyond the model calls.
pub fn environment(command: &mut Command, home: &Path, tmp: &Path) {
    command
        .env_clear()
        .env(
            "PATH",
            std::env::var_os("PATH").unwrap_or_else(|| "/usr/bin:/bin".into()),
        )
        .env("LANG", "C.UTF-8")
        .env("HOME", home)
        .env("PI_CODING_AGENT_DIR", agent_directory(home))
        .env("TMPDIR", tmp)
        .env("PI_OFFLINE", "1")
        .env("PI_SKIP_VERSION_CHECK", "1")
        .env("PI_TELEMETRY", "0");
}

/// The options that keep every ambient resource out: built-in tools, extensions other than MCP, context files,
/// skills, prompt templates, themes and project configuration.
fn confined() -> Vec<OsString> {
    [
        "--no-extensions",
        "--extension",
        "builtin:mcp",
        "--no-builtin-tools",
        "--no-context-files",
        "--no-skills",
        "--no-prompt-templates",
        "--no-themes",
        "--no-approve",
        "--offline",
    ]
    .into_iter()
    .map(OsString::from)
    .collect()
}

/// A grading session's options: one prompt from stdin, JSON events on stdout, the six grading tools alone and a
/// new session saved under `sessions`.
pub fn session_arguments(
    model: &str,
    effort: Option<&str>,
    sessions: &Path,
    session_id: &str,
) -> Vec<OsString> {
    let mut arguments: Vec<OsString> = ["--mode", "json", "--print", "--model", model]
        .into_iter()
        .map(OsString::from)
        .collect();
    if let Some(effort) = effort {
        arguments.extend(["--thinking".into(), effort.into()]);
    }
    arguments.extend(confined());
    arguments.extend(["--tools".into(), tool_names().join(",").into()]);
    arguments.extend([
        "--session-dir".into(),
        sessions.as_os_str().to_os_string(),
        "--session-id".into(),
        session_id.into(),
    ]);
    arguments
}

/// `models.json` sending the provider's requests to `base_url` with a key that only the fake API accepts.
fn local_provider(agent: &Path, base_url: Option<&str>) -> S<()> {
    let mut provider = vec![("apiKey", text("local-probe-only"))];
    if let Some(base_url) = base_url {
        provider.push(("baseUrl", text(base_url)));
    }
    write(
        &agent.join("models.json"),
        json(&object(vec![(
            "providers",
            object(vec![(PROVIDER, object(provider))]),
        )])),
    )
}

/// Check the Pi sign-in for the model's provider and the client version, returning the version.
pub fn preflight(expected: &str, home: &Path, model: &str) -> S<String> {
    let (provider, _) = split(model)?;
    let auth = read_json(&agent_directory(home).join("auth.json")).unwrap_or(Value::Null);
    let entry = get(&auth, provider).cloned().unwrap_or(Value::Null);
    let signed_in = get_str(&entry, "type") == Some("api_key")
        && get_str(&entry, "key").is_some_and(|key| !key.is_empty());
    if !signed_in {
        return Err(format!(
            "Pi {provider} credential material is absent or invalid; credentials were not logged"
        ));
    }
    pinned_version(NAME, "Pi", expected)
}

/// The thinking levels the installed Pi offers `model`, read over its RPC mode with a placeholder key; no model is
/// called.
pub fn efforts(model: &str) -> S<Vec<String>> {
    split(model)?;
    let scratch = Scratch::new("pi-efforts-")?;
    let home = scratch.0.join("home");
    let tmp = scratch.0.join("tmp");
    mkdir(&tmp)?;
    let agent = agent_directory(&home);
    mkdir(&agent)?;
    local_provider(&agent, None)?;
    let mut command = Command::new(NAME);
    command
        .args(["--mode", "rpc", "--no-session", "--model", model])
        .args(confined())
        .current_dir(&scratch.0);
    environment(&mut command, &home, &tmp);
    let ran = crate::child::run_input(
        &mut command,
        Some(b"{\"type\": \"get_available_thinking_levels\", \"id\": \"efforts\"}\n"),
        Duration::from_secs(30),
    )
    .map_err(|error| format!("cannot run pi: {error}"))?;
    let answered = String::from_utf8_lossy(&ran.stdout)
        .lines()
        .filter_map(|line| parse(line.as_bytes()).ok())
        .find(|line| get_str(line, "id") == Some("efforts"));
    let levels = answered
        .filter(|answer| get(answer, "success").is_some_and(Value::truthy))
        .and_then(|answer| get(&answer, "data").cloned())
        .and_then(|data| get(&data, "levels").cloned());
    match levels {
        Some(Value::Array(levels)) => Ok(levels
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect()),
        _ => Err(format!("Pi lists no thinking levels for {model}")),
    }
}

/// One server-sent chat completion for request `index`: a call to `exercise`'s grading tool, or the closing text.
/// Its usage carries the cost OpenRouter reports.
fn reply(id: &str, index: usize, exercise: Option<&(String, Value)>) -> Vec<u8> {
    let delta = match exercise {
        Some((name, arguments)) => object(vec![
            ("role", text("assistant")),
            (
                "tool_calls",
                Value::Array(vec![object(vec![
                    ("index", Value::Int("0".into())),
                    ("id", Value::Str(format!("probe-{name}"))),
                    ("type", text("function")),
                    (
                        "function",
                        object(vec![
                            ("name", Value::Str(format!("mcp__grading__{name}"))),
                            ("arguments", Value::Str(json(arguments))),
                        ]),
                    ),
                ])]),
            ),
        ]),
        None => object(vec![
            ("role", text("assistant")),
            ("content", text("probe complete")),
        ]),
    };
    let finish = if exercise.is_some() {
        "tool_calls"
    } else {
        "stop"
    };
    let chunk = |choice: Value, usage: Option<Value>| {
        let mut entries = vec![
            ("id", Value::Str(format!("gen-probe-{index}"))),
            ("object", text("chat.completion.chunk")),
            ("created", Value::Int("1".into())),
            ("model", text(id)),
            ("choices", Value::Array(vec![choice])),
        ];
        if let Some(usage) = usage {
            entries.push(("usage", usage));
        }
        object(entries)
    };
    let one = || Value::Int("1".into());
    let chunks = [
        chunk(
            object(vec![
                ("index", Value::Int("0".into())),
                ("delta", delta),
                ("finish_reason", Value::Null),
            ]),
            None,
        ),
        chunk(
            object(vec![
                ("index", Value::Int("0".into())),
                ("delta", Value::Object(Object::default())),
                ("finish_reason", text(finish)),
            ]),
            Some(object(vec![
                ("prompt_tokens", one()),
                ("completion_tokens", one()),
                ("total_tokens", Value::Int("2".into())),
                ("cost", Value::Float(0.0)),
                ("is_byok", Value::Bool(false)),
            ])),
        ),
    ];
    let mut payload = chunks.iter().fold(String::new(), |mut out, chunk| {
        let _ = write!(out, "data: {}\n\n", json(chunk));
        out
    });
    payload.push_str("data: [DONE]\n\n");
    payload.into_bytes()
}

/// Recognizable instructions wherever Pi could load ambient context from: the agent directory, the start directory
/// and its ancestors, and project `.pi` resources. Pi always reads the agent directory's `SYSTEM.md` and
/// `APPEND_SYSTEM.md`, so none is planted there: a session's agent directory is new and holds only what dispatch
/// writes.
fn plant_markers(root: &Path, start: &Path, agent: &Path) -> S<()> {
    let marked = |label: &str| format!("{label}-{MARKER}");
    for (directory, label) in [(root, "ANCESTOR"), (start, "PROJECT"), (agent, "USER")] {
        write(&directory.join("AGENTS.md"), marked(label))?;
        write(&directory.join("CLAUDE.md"), marked(label))?;
    }
    let project = start.join(".pi");
    mkdir(&project.join("prompts"))?;
    write(&project.join("SYSTEM.md"), marked("PROJECT-SYSTEM"))?;
    write(&project.join("APPEND_SYSTEM.md"), marked("PROJECT-SYSTEM"))?;
    write(&project.join("prompts/probe.md"), marked("PROMPT"))?;
    for skills in [
        start.join(".pi/skills/probe"),
        start.join(".agents/skills/probe"),
        agent.join("skills/probe"),
    ] {
        mkdir(&skills)?;
        write(
            &skills.join("SKILL.md"),
            format!("---\nname: probe\ndescription: {}\n---\n", marked("SKILL")),
        )?;
    }
    git_markers(
        root,
        "BRANCH-PI-GIT-PROBE-MARKER",
        "SUBJECT-PI-GIT-PROBE-MARKER",
    )?;
    write(&root.join("UNTRACKED-PI-GIT-PROBE-MARKER"), "")
}

/// The labels of the planted markers `context` holds.
fn leaked(context: &str) -> Vec<&'static str> {
    MARKED
        .into_iter()
        .filter(|label| context.contains(&format!("{label}-{MARKER}")))
        .collect()
}

/// Run the installed `pi` against a local fake OpenRouter API that calls each grading tool once, from a start
/// directory inside a repository planted with ambient instructions, and check that every request offered only the
/// grading tools for `model`, carried no ambient context, and that every tool completed. No model is called.
pub fn probe(bench_tools: &Path, model: &str) -> S<Value> {
    let (_, id) = split(model)?;
    let executable = review_isolation::which(NAME).ok_or("Pi client is not installed")?;
    let own = review_isolation::own_executable()?;
    let scratch = Scratch::new("pi-grading-probe-")?;
    let root = &scratch.0;
    let (work, start, home) = (root.join("work"), root.join("start"), root.join("home"));
    let (tmp, agent) = (root.join("tmp"), agent_directory(&home));
    for directory in [&work, &start, &tmp, &agent] {
        mkdir(directory)?;
    }
    plant_markers(root, &start, &agent)?;
    let policy = root.join("policy.json");
    let exercises: Vec<(String, Value)> = probe_workspace(&work, &policy, bench_tools)?
        .into_iter()
        .map(|(name, input)| (name.to_string(), input))
        .collect();
    configure(
        &agent,
        &own,
        &[
            "grading".to_string(),
            "tools".to_string(),
            "--work".to_string(),
            work.to_string_lossy().into_owned(),
            "--policy".to_string(),
            policy.to_string_lossy().into_owned(),
            "--bench-tools".to_string(),
            review_isolation::resolve(bench_tools)
                .to_string_lossy()
                .into_owned(),
        ],
    )?;
    let replies = exercises.clone();
    let served = id.to_string();
    let mut api = FakeApi::start(Box::new(move |path, _, index| {
        path.ends_with("/chat/completions").then(|| {
            (
                "text/event-stream",
                reply(&served, index, replies.get(index - 1)),
            )
        })
    }))?;
    local_provider(
        &agent,
        Some(&format!("http://127.0.0.1:{}/api/v1", api.port)),
    )?;
    let session_id = "pi-grading-probe";
    let ran = (|| {
        let mut version = Command::new(&executable);
        version.arg("--version");
        environment(&mut version, &home, &tmp);
        let version = crate::child::run(&mut version, Duration::from_secs(15))
            .map_err(|error| format!("cannot run pi: {error}"))?;
        let mut session = Command::new(&executable);
        session
            .args(session_arguments(model, None, &sessions(&home), session_id))
            .current_dir(&start);
        environment(&mut session, &home, &tmp);
        let session = crate::child::run_input(
            &mut session,
            Some(b"Exercise every supplied grading tool once."),
            Duration::from_secs(60),
        )
        .map_err(|error| format!("cannot run pi: {error}"))?;
        Ok::<_, String>((version, session))
    })();
    let requests = api.stop();
    let (version, session) = ran?;
    require(
        version.code == Some(0),
        "cannot check the Pi client version",
    )?;
    require(
        session.code.is_some(),
        "Pi compatibility probe timed out after 60 s",
    )?;
    require(session.code == Some(0), "Pi compatibility probe failed")?;
    require(
        requests.len() == exercises.len() + 1,
        "client did not exercise every grading tool",
    )?;
    let expected = tool_names();
    for request in &requests {
        let mut offered: Vec<String> = get(request, "tools")
            .and_then(Value::as_array)
            .unwrap_or_default()
            .iter()
            .filter_map(|tool| get(tool, "function").and_then(|function| get_str(function, "name")))
            .map(str::to_string)
            .collect();
        offered.sort();
        require(offered == expected, "unexpected native or MCP tools")?;
        require(
            get_str(request, "model") == Some(id),
            "client requested an unexpected model",
        )?;
    }
    let context = json(&Value::Array(requests.clone()));
    let leaked = leaked(&context);
    require(
        leaked.is_empty(),
        &format!("ambient context was loaded: {}", leaked.join(", ")),
    )?;
    require(
        !context.contains("PI-GIT-PROBE-MARKER"),
        "the enclosing repository's git state was sent to the model",
    )?;
    require(
        context.contains("focused inspection"),
        "grading inspection did not complete",
    )?;
    let saved = session_files(&sessions(&home))?;
    require(
        saved.len() == 1,
        "client did not save exactly one fresh grading session",
    )?;
    let raw = String::from_utf8_lossy(&read(&saved[0])?).into_owned();
    require(
        !raw.contains(MARKER),
        "ambient context was loaded into the saved session",
    )?;
    let mut results = Object::default();
    let mut declared = Vec::new();
    for line in raw.lines() {
        let entry = parse(line.as_bytes())?;
        let message = get(&entry, "message").cloned().unwrap_or(Value::Null);
        match get_str(&message, "role") {
            Some("system") => declared.extend(
                get(&message, "toolsAdded")
                    .and_then(Value::as_array)
                    .unwrap_or_default()
                    .iter()
                    .filter_map(|tool| get_str(tool, "name"))
                    .map(str::to_string),
            ),
            Some("toolResult") => {
                if let Some(id) = get_str(&message, "toolCallId") {
                    results.insert(id.to_string(), message.clone());
                }
            }
            _ => {}
        }
    }
    declared.sort();
    require(
        declared == expected,
        "the saved session declared other tools",
    )?;
    for (name, _) in &exercises {
        let result = results
            .get(&format!("probe-{name}"))
            .ok_or_else(|| format!("grading tool result missing: {name}"))?;
        require(
            !get(result, "isError").is_some_and(Value::truthy),
            &format!("grading tool returned an error: {name}"),
        )?;
    }
    for name in ["run", "validate"] {
        let content = results
            .get(&format!("probe-{name}"))
            .and_then(|result| get(result, "content"))
            .map_or_else(String::new, |content| content_text(content, ""));
        require(
            exit_code_zero(&content),
            &format!("grading tool failed: {name}"),
        )?;
    }
    written(&work)?;
    Ok(object(vec![
        ("client", text(NAME)),
        (
            "cli_version",
            Value::Str(String::from_utf8_lossy(&version.stdout).trim().to_string()),
        ),
        ("model", text(model)),
        ("tools", strings(expected.iter().map(String::as_str))),
        ("ambient_markers_absent", Value::Bool(true)),
        ("saved_session_markers_absent", Value::Bool(true)),
        ("inspection_completed", Value::Bool(true)),
        ("all_tools_completed", Value::Bool(true)),
        ("fresh_home", Value::Bool(true)),
        ("budget_enforced", Value::Bool(false)),
        ("paid_calls", Value::Int("0".into())),
    ]))
}

/// Every `*.jsonl` under `directory`, sorted; none when it does not exist.
pub fn session_files(directory: &Path) -> S<Vec<PathBuf>> {
    fn walk(directory: &Path, found: &mut Vec<PathBuf>) -> S<()> {
        for entry in std::fs::read_dir(directory).map_err(io(directory))? {
            let entry = entry.map_err(io(directory))?;
            let path = entry.path();
            if entry.file_type().map_err(io(&path))?.is_dir() {
                walk(&path, found)?;
            } else if path
                .extension()
                .is_some_and(|extension| extension == "jsonl")
            {
                found.push(path);
            }
        }
        Ok(())
    }
    let mut found = Vec::new();
    if std::fs::symlink_metadata(directory).is_ok() {
        walk(directory, &mut found)?;
    }
    found.sort();
    Ok(found)
}

#[derive(Default)]
struct Tokens {
    input: f64,
    output: f64,
    cache_read: f64,
    cache_write: f64,
    cache_write_1h: f64,
}

impl Tokens {
    fn add(&mut self, usage: &Value) {
        let count = |field: &str| price(usage, field).unwrap_or(0.0);
        self.input += count("input");
        self.output += count("output");
        self.cache_read += count("cacheRead");
        self.cache_write += count("cacheWrite");
        self.cache_write_1h += count("cacheWrite1h");
    }

    fn priced(&self, rate: &Value) -> S<f64> {
        let short = self.cache_write - self.cache_write_1h;
        Ok((self.input * price(rate, "input")?
            + self.output * price(rate, "output")?
            + self.cache_read * price(rate, "cache_read")?
            + short * price(rate, "cache_write_5m")?
            + self.cache_write_1h * price(rate, "cache_write_1h")?)
            / 1e6)
    }
}

/// What one saved Pi session shows: its models, tools and usage.
#[derive(Default)]
struct Session {
    id: Option<String>,
    forked: bool,
    models: BTreeSet<String>,
    first_declared: Option<Vec<String>>,
    problems: BTreeSet<String>,
    tokens: Tokens,
    billed: usize,
    reported: f64,
    unreported: usize,
}

impl Session {
    fn read(&mut self, raw: &str, allowed: &[String]) -> S<()> {
        let outside = |name: &str| !allowed.iter().any(|tool| tool == name);
        for line in raw.lines() {
            let entry = parse(line.as_bytes())?;
            match get_str(&entry, "type") {
                Some("session") => {
                    self.id = get_str(&entry, "id").map(str::to_string);
                    self.forked = get(&entry, "parentSession").is_some_and(Value::truthy);
                }
                Some("message") => {
                    let message = get(&entry, "message").cloned().unwrap_or(Value::Null);
                    match get_str(&message, "role") {
                        Some("system") => {
                            let added: Vec<String> = get(&message, "toolsAdded")
                                .and_then(Value::as_array)
                                .unwrap_or_default()
                                .iter()
                                .map(|tool| get_str(tool, "name").unwrap_or("").to_string())
                                .collect();
                            if added.iter().any(|name| outside(name)) {
                                self.problems.insert(
                                    "the session declared a tool outside the grading MCP catalog"
                                        .to_string(),
                                );
                            }
                            if self.first_declared.is_none() {
                                let mut sorted = added;
                                sorted.sort();
                                self.first_declared = Some(sorted);
                            }
                        }
                        Some("assistant") => {
                            self.answered(&message);
                            for block in get(&message, "content")
                                .and_then(Value::as_array)
                                .unwrap_or_default()
                            {
                                if get_str(block, "type") == Some("toolCall")
                                    && outside(get_str(block, "name").unwrap_or(""))
                                {
                                    self.problems.insert(
                                        "grader invoked a tool outside the grading MCP catalog"
                                            .to_string(),
                                    );
                                }
                            }
                        }
                        Some("toolResult")
                            if outside(get_str(&message, "toolName").unwrap_or("")) =>
                        {
                            self.problems.insert(
                                "grader invoked a tool outside the grading MCP catalog".to_string(),
                            );
                        }
                        _ => {}
                    }
                }
                Some("usage") => self.answered(&entry),
                Some(_) => {
                    if let Some(usage) =
                        get(&entry, "usage").filter(|usage| usage.as_object().is_some())
                    {
                        self.charge(usage);
                    }
                }
                None => {}
            }
        }
        Ok(())
    }

    /// A model response or usage record: its `provider/model` and its usage.
    fn answered(&mut self, record: &Value) {
        // Pi records the model the provider served in `responseModel`, only when it differs from the requested one.
        let served = get_str(record, "responseModel").or_else(|| get_str(record, "model"));
        if let (Some(provider), Some(model)) = (get_str(record, "provider"), served) {
            self.models.insert(format!("{provider}/{model}"));
        }
        if let Some(usage) = get(record, "usage") {
            self.charge(usage);
        }
    }

    fn charge(&mut self, usage: &Value) {
        self.tokens.add(usage);
        if price(usage, "totalTokens").unwrap_or(0.0) <= 0.0 {
            return;
        }
        self.billed += 1;
        let cost = get(usage, "cost").cloned().unwrap_or(Value::Null);
        if get(&cost, "totalFromProvider").is_some_and(Value::truthy)
            && let Ok(total) = price(&cost, "total")
        {
            self.reported += total;
        } else {
            self.unreported += 1;
        }
    }
}

/// The Pi session's id, models, subagents, tool audit and priced usage from its saved session files. The priced
/// total is the rate entry's; the upper bound is the larger of it and what OpenRouter reported it charged, unknown
/// when a billed response carries no reported cost.
pub fn evidence(sessions: &Path, session_id: &str, rate: &Value) -> S<Value> {
    let allowed = tool_names();
    let paths = session_files(sessions)?;
    let mut roots = Vec::new();
    let mut others = 0usize;
    let mut problems = BTreeSet::new();
    let mut whole = Session::default();
    for path in &paths {
        let mut session = Session::default();
        let raw = String::from_utf8(read(path)?).map_err(|error| error.to_string())?;
        session.read(&raw, &allowed)?;
        if session.id.as_deref() == Some(session_id) && !session.forked {
            roots.push(session.first_declared.clone());
        } else {
            others += 1;
        }
        problems.extend(session.problems);
        whole.models.extend(session.models);
        whole.tokens.input += session.tokens.input;
        whole.tokens.output += session.tokens.output;
        whole.tokens.cache_read += session.tokens.cache_read;
        whole.tokens.cache_write += session.tokens.cache_write;
        whole.tokens.cache_write_1h += session.tokens.cache_write_1h;
        whole.billed += session.billed;
        whole.reported += session.reported;
        whole.unreported += session.unreported;
    }
    match roots.as_slice() {
        [declared] if declared.as_ref() == Some(&allowed) => {}
        [_] => {
            problems.insert("the session did not declare exactly the grading tools".to_string());
        }
        _ => {
            problems.insert("expected exactly one Pi root session".to_string());
        }
    }
    let (priced, reported, high) = if whole.billed == 0 {
        problems.insert("no billed Pi model response found".to_string());
        (Value::Null, Value::Null, Value::Null)
    } else {
        let priced = usage::round_to(whole.tokens.priced(rate)?, 6);
        let reported = usage::round_to(whole.reported, 6);
        if whole.unreported == 0 {
            (
                Value::Float(priced),
                Value::Float(reported),
                Value::Float(priced.max(reported)),
            )
        } else {
            (Value::Float(priced), Value::Null, Value::Null)
        }
    };
    let session = if roots.len() == 1 {
        text(session_id)
    } else {
        Value::Null
    };
    Ok(object(vec![
        ("session_id", session),
        (
            "models_observed",
            strings(whole.models.iter().map(String::as_str)),
        ),
        ("subagents", Value::Int(others.to_string())),
        (
            "audit_violations",
            strings(problems.iter().map(String::as_str)),
        ),
        (
            "usage",
            object(vec![
                ("priced_total_usd", priced.clone()),
                ("low", priced),
                ("high", high),
                ("billing", text("api-dollars")),
                ("requests", Value::Int(whole.billed.to_string())),
                (
                    "unreported_requests",
                    Value::Int(whole.unreported.to_string()),
                ),
                ("provider_reported_usd", reported),
                ("rate", rate.clone()),
            ]),
        ),
    ]))
}

/// The Pi package `cli` belongs to: the nearest directory above it with a `package.json`.
pub fn package_of(cli: &Path) -> S<PathBuf> {
    cli.ancestors()
        .skip(1)
        .find(|directory| directory.join("package.json").is_file())
        .map(Path::to_path_buf)
        .ok_or_else(|| format!("{} is not inside a Pi package", cli.display()))
}

/// Copy the parts of `package` a grading session runs into the new directory `into`.
pub fn pin(package: &Path, into: &Path) -> S<()> {
    fn copy(from: &Path, to: &Path) -> S<()> {
        let kind = std::fs::symlink_metadata(from).map_err(io(from))?;
        if kind.is_dir() {
            std::fs::create_dir(to).map_err(io(to))?;
            let mut entries: Vec<_> = std::fs::read_dir(from)
                .map_err(io(from))?
                .collect::<Result<_, _>>()
                .map_err(io(from))?;
            entries.sort_by_key(std::fs::DirEntry::file_name);
            for entry in entries {
                copy(&entry.path(), &to.join(entry.file_name()))?;
            }
            Ok(())
        } else if kind.is_file() {
            std::fs::copy(from, to).map(|_| ()).map_err(io(from))
        } else {
            Err(format!("{} is not a regular file", from.display()))
        }
    }
    std::fs::create_dir(into).map_err(io(into))?;
    for part in PINNED {
        let target = into.join(part);
        if let Some(parent) = target.parent() {
            mkdir(parent)?;
        }
        copy(&package.join(part), &target)?;
    }
    Ok(())
}

/// One digest of every file under `root`: each relative path, executable bit and content hash, in path order.
pub fn tree_sha256(root: &Path) -> S<String> {
    fn walk(root: &Path, directory: &Path, rows: &mut Vec<String>) -> S<()> {
        use std::os::unix::fs::PermissionsExt;
        let mut entries: Vec<_> = std::fs::read_dir(directory)
            .map_err(io(directory))?
            .collect::<Result<_, _>>()
            .map_err(io(directory))?;
        entries.sort_by_key(std::fs::DirEntry::file_name);
        for entry in entries {
            let path = entry.path();
            let kind = std::fs::symlink_metadata(&path).map_err(io(&path))?;
            if kind.is_dir() {
                walk(root, &path, rows)?;
            } else if kind.is_file() {
                let relative = path.strip_prefix(root).unwrap_or(&path);
                rows.push(format!(
                    "{}\0{}\0{}\n",
                    relative.to_string_lossy(),
                    kind.permissions().mode() & 0o111 != 0,
                    sha256::hex_digest(&read(&path)?)
                ));
            } else {
                return Err(format!("{} is not a regular file", path.display()));
            }
        }
        Ok(())
    }
    let mut rows = Vec::new();
    walk(root, root, &mut rows)?;
    Ok(sha256::hex_digest(rows.concat().as_bytes()))
}
