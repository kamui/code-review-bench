//! The grader clients: their preflight, the probes that run each against a local fake API before a paid session,
//! the Codex settings a grading session runs under, and the Codex transcript evidence a dispatch records.

use crate::grading_tools::{self, Host, Refusal, Tools};
use crate::pyjson::{self, Dumps, Duplicates, Object, Value};
use crate::{clean_context, review_isolation, sha256, usage};
use regex::Regex;
use std::fmt::Write as _;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub const CATALOG: &str = "bench/harness/codex-grading-models.v1.json";
pub const CATALOG_CLI_VERSION: &str = "0.160.0";
const PROBE_MODEL: &str = "gpt-6-luna";
const NAMESPACE: &str = "mcp__grading";
/// The native helpers Codex always offers beside an MCP server, sorted.
pub const RESOURCE_TOOLS: [&str; 3] = [
    "list_mcp_resource_templates",
    "list_mcp_resources",
    "read_mcp_resource",
];
const DISABLED_FEATURES: [&str; 11] = [
    "shell_tool",
    "unified_exec",
    "multi_agent",
    "view_image",
    "browser_use",
    "computer_use",
    "image_generation",
    "goals",
    "sleep_tool",
    "skill_search",
    "skill_mcp_dependency_install",
];
const CLAUDE_MARKERS: [&str; 2] = [
    "AMBIENT-GRADING-PROBE-MARKER",
    "ANCESTOR-GRADING-PROBE-MARKER",
];

type S<T> = Result<T, String>;

fn text(value: &str) -> Value {
    Value::Str(value.to_string())
}

fn object(pairs: Vec<(&str, Value)>) -> Value {
    Value::Object(
        pairs
            .into_iter()
            .map(|(key, value)| (key.to_string(), value))
            .collect(),
    )
}

fn strings<'a>(items: impl IntoIterator<Item = &'a str>) -> Value {
    Value::Array(items.into_iter().map(text).collect())
}

fn path_text(path: &Path) -> Value {
    Value::Str(path.to_string_lossy().into_owned())
}

fn json(value: &Value) -> String {
    pyjson::dumps(value, &Dumps::default())
}

fn indented(value: &Value) -> String {
    pyjson::dumps(
        value,
        &Dumps {
            indent: Some(2),
            ..Dumps::default()
        },
    )
}

fn parse(raw: &[u8]) -> S<Value> {
    let text = std::str::from_utf8(raw).map_err(|error| error.to_string())?;
    pyjson::loads(text, Duplicates::KeepLast).map_err(|error| error.to_string())
}

fn get<'v>(value: &'v Value, key: &str) -> Option<&'v Value> {
    value.as_object().and_then(|object| object.get(key))
}

fn get_str<'v>(value: &'v Value, key: &str) -> Option<&'v str> {
    get(value, key).and_then(Value::as_str)
}

fn require(condition: bool, message: &str) -> S<()> {
    if condition {
        Ok(())
    } else {
        Err(message.to_string())
    }
}

fn io(path: &Path) -> impl Fn(std::io::Error) -> String + '_ {
    move |error| format!("{}: {error}", path.display())
}

fn write(path: &Path, contents: impl AsRef<[u8]>) -> S<()> {
    std::fs::write(path, contents).map_err(io(path))
}

fn read(path: &Path) -> S<Vec<u8>> {
    std::fs::read(path).map_err(io(path))
}

fn mkdir(path: &Path) -> S<()> {
    std::fs::create_dir_all(path).map_err(io(path))
}

/// `os.path.expanduser("~")`.
pub fn user_home() -> PathBuf {
    std::env::var_os("HOME").map_or_else(grading_tools::account_home, PathBuf::from)
}

/// A private directory removed with everything in it when dropped.
struct Scratch(PathBuf);

impl Scratch {
    fn new(prefix: &str) -> S<Self> {
        let temporary = std::env::temp_dir();
        loop {
            let candidate = temporary.join(crate::evidence::random_name(prefix));
            match std::fs::DirBuilder::new().mode(0o700).create(&candidate) {
                Ok(()) => return Ok(Self(review_isolation::resolve(&candidate))),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(io(&candidate)(error)),
            }
        }
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// What a fake API answers one request with: its content type and body, or `None` for 404.
type Respond = dyn Fn(&str, &Value, usize) -> Option<(&'static str, Vec<u8>)> + Send + Sync;

/// A loopback HTTP server that records each POSTed JSON body and answers it with `respond`.
struct FakeApi {
    port: u16,
    stop: Arc<AtomicBool>,
    worker: Option<std::thread::JoinHandle<()>>,
    requests: Arc<Mutex<Vec<Value>>>,
}

impl FakeApi {
    fn start(respond: Box<Respond>) -> S<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").map_err(|error| error.to_string())?;
        let port = listener
            .local_addr()
            .map_err(|error| error.to_string())?
            .port();
        listener
            .set_nonblocking(true)
            .map_err(|error| error.to_string())?;
        let stop = Arc::new(AtomicBool::new(false));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let respond: Arc<Respond> = Arc::from(respond);
        let worker = {
            let (stop, requests) = (stop.clone(), requests.clone());
            std::thread::spawn(move || {
                while !stop.load(Ordering::SeqCst) {
                    match listener.accept() {
                        Ok((stream, _)) => {
                            let (requests, respond) = (requests.clone(), respond.clone());
                            std::thread::spawn(move || {
                                let _ = answer(stream, &requests, respond.as_ref());
                            });
                        }
                        Err(_) => std::thread::sleep(Duration::from_millis(10)),
                    }
                }
            })
        };
        Ok(Self {
            port,
            stop,
            worker: Some(worker),
            requests,
        })
    }

    fn stop(&mut self) -> Vec<Value> {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        self.requests
            .lock()
            .map(|found| found.clone())
            .unwrap_or_default()
    }
}

impl Drop for FakeApi {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Answer one request on `stream` and close it, as an HTTP/1.0 server does.
fn answer(
    stream: TcpStream,
    requests: &Mutex<Vec<Value>>,
    respond: &Respond,
) -> std::io::Result<()> {
    stream.set_nonblocking(false)?;
    stream.set_read_timeout(Some(Duration::from_secs(30)))?;
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let mut parts = line.split_whitespace();
    let (method, path) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""));
    let mut length = 0usize;
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header)? == 0 || header.trim().is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':')
            && name.trim().eq_ignore_ascii_case("content-length")
        {
            length = value.trim().parse().unwrap_or(0);
        }
    }
    let mut body = vec![0; length];
    reader.read_exact(&mut body)?;
    let reply = if method != "POST" {
        None
    } else if let Ok(value) = parse(&body) {
        let index = {
            let mut found = requests
                .lock()
                .map_err(|_| std::io::Error::other("poisoned"))?;
            found.push(value.clone());
            found.len()
        };
        respond(path, &value, index).map(|reply| ("200 OK", reply))
    } else {
        Some(("400 Bad Request", ("text/plain", Vec::new())))
    };
    let (status, (mime, payload)) = reply.unwrap_or(if method == "POST" {
        ("404 Not Found", ("text/plain", Vec::new()))
    } else {
        ("501 Not Implemented", ("text/plain", Vec::new()))
    });
    let mut stream = stream;
    stream.write_all(
        format!(
            "HTTP/1.0 {status}\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            payload.len()
        )
        .as_bytes(),
    )?;
    stream.write_all(&payload)?;
    stream.flush()
}

fn git_markers(root: &Path, branch: &str, subject: &str) -> S<()> {
    for arguments in [
        vec!["init", "-q", "-b", branch],
        vec!["commit", "-q", "--allow-empty", "-m", subject],
    ] {
        let done = Command::new("git")
            .arg("-C")
            .arg(root)
            .args([
                "-c",
                "user.name=probe",
                "-c",
                "user.email=probe@example.invalid",
            ])
            .args(arguments)
            .output()
            .map_err(|error| format!("git: {error}"))?;
        require(
            done.status.success(),
            "git could not set up the probe repository",
        )?;
    }
    Ok(())
}

fn sandbox_probe(work: &Path, policy: Value, bench_tools: &Path) -> S<Value> {
    let tools = Tools {
        work: work.to_path_buf(),
        policy,
        protected: Vec::new(),
        bench_tools: Some(bench_tools.to_path_buf()),
        host: Host::current(),
    };
    grading_tools::probe(&tools).map_err(|refusal| match refusal {
        Refusal::Denied(message) => message,
        Refusal::Failed => "the grading sandbox could not be probed".to_string(),
    })
}

fn probe_policy() -> Value {
    object(vec![
        ("test_kind", text("none")),
        ("private_go", Value::Bool(false)),
        ("go_flags", text("-mod=readonly")),
        ("once", Value::Bool(false)),
    ])
}

/// The workspace inputs both probes give the tools: a clone to inspect, validator inputs and the exercises.
fn probe_workspace(
    work: &Path,
    policy: &Path,
    bench_tools: &Path,
) -> S<Vec<(&'static str, Value)>> {
    mkdir(&work.join("clone"))?;
    write(&work.join("clone/test.txt"), "focused inspection")?;
    sandbox_probe(work, probe_policy(), bench_tools)?;
    mkdir(&work.join("validator"))?;
    write(
        &work.join("validator/inputs.json"),
        r#"{"families": [], "canonical": {}, "matches": {}, "links": {}, "reviews": {"blind-probe": {"items": []}}}"#,
    )?;
    write(policy, json(&probe_policy()))?;
    let verdicts = r#"{"reviews": {"blind-probe": {"items": {}, "recommendations": [], "remedy_inventory": {"state": "complete", "reason": "The review has no items."}}}, "new_candidates": [], "link_disputes": []}"#;
    let exercise = |raw: &str| pyjson::loads(raw, Duplicates::KeepLast).unwrap_or(Value::Null);
    Ok(vec![
        ("inspect", exercise(r#"{"path": "clone/test.txt"}"#)),
        (
            "run",
            exercise(r#"{"argv": ["cat", "test.txt"], "cwd": "clone"}"#),
        ),
        (
            "write_scratch",
            exercise(r#"{"path": "clone-work/probe.txt", "text": "scratch probe"}"#),
        ),
        ("write_verdicts", object(vec![("text", text(verdicts))])),
        (
            "edit_verdicts",
            exercise(
                r#"{"edits": [{"old": "The review has no items.", "new": "The review holds no items."}]}"#,
            ),
        ),
        ("validate", Value::Object(Object::default())),
    ])
}

fn written(work: &Path) -> S<()> {
    require(
        std::fs::read(work.join("clone-work/probe.txt"))
            .is_ok_and(|found| found == b"scratch probe"),
        "scratch writer failed",
    )?;
    require(
        std::fs::read_to_string(work.join("verdicts.json"))
            .is_ok_and(|found| found.contains("The review holds no items.")),
        "verdict writer or editor failed",
    )
}

/// The text of a tool result's content: a string, or the concatenated text of its blocks.
fn content_text(content: &Value, separator: &str) -> String {
    match content {
        Value::Str(found) => found.clone(),
        Value::Array(blocks) => blocks
            .iter()
            .map(|block| get_str(block, "text").unwrap_or(""))
            .collect::<Vec<_>>()
            .join(separator),
        _ => String::new(),
    }
}

fn exit_code_zero(raw: &str) -> bool {
    pyjson::loads(raw, Duplicates::KeepLast)
        .ok()
        .and_then(|found| get(&found, "exit_code").cloned())
        .is_some_and(|code| matches!(code, Value::Int(digits) if digits == "0"))
}

/// Run the installed `claude` against a local fake API that calls each grading tool once, and check that it
/// offered only those tools, loaded no ambient context and that every tool completed. No model is called.
pub fn probe_claude(bench_tools: &Path) -> S<Value> {
    let executable = review_isolation::which("claude").ok_or("Claude client is not installed")?;
    let own = review_isolation::own_executable()?;
    let scratch = Scratch::new("grading-probe-")?;
    let root = &scratch.0;
    let (work, home) = (root.join("work"), root.join("home"));
    mkdir(&work)?;
    mkdir(&home)?;
    write(&work.join("CLAUDE.md"), CLAUDE_MARKERS[0])?;
    write(&root.join("CLAUDE.md"), CLAUDE_MARKERS[1])?;
    write(
        &home.join(".claude.json"),
        r#"{"hasCompletedOnboarding": true}"#,
    )?;
    git_markers(
        root,
        "BRANCH-GRADING-PROBE-MARKER",
        "SUBJECT-GRADING-PROBE-MARKER",
    )?;
    write(&root.join("UNTRACKED-GRADING-PROBE-MARKER"), "")?;
    let policy = root.join("policy.json");
    let exercises = probe_workspace(&work, &policy, bench_tools)?;
    let config = root.join("mcp.json");
    let server = object(vec![
        ("command", path_text(&own)),
        (
            "args",
            Value::Array(vec![
                text("grading"),
                text("tools"),
                text("--work"),
                path_text(&work),
                text("--policy"),
                path_text(&policy),
                text("--bench-tools"),
                path_text(&review_isolation::resolve(bench_tools)),
            ]),
        ),
    ]);
    write(
        &config,
        json(&object(vec![(
            "mcpServers",
            object(vec![("grading", server)]),
        )])),
    )?;
    let settings = root.join("settings.json");
    write(
        &settings,
        r#"{"claudeMdExcludes": ["**"], "autoMemoryEnabled": false, "disableAllHooks": true}"#,
    )?;
    let responses: Vec<(String, Value)> = exercises
        .iter()
        .map(|(name, input)| (name.to_string(), input.clone()))
        .collect();
    let mut api = FakeApi::start(Box::new(move |_, body, index| {
        let stream = get(body, "stream").is_some_and(Value::truthy);
        Some(claude_reply(responses.get(index - 1), stream))
    }))?;
    let start = clean_context::neutral_directory()?;
    let ran = (|| {
        write(&start.join("CLAUDE.md"), CLAUDE_MARKERS[0])?;
        let mut command = Command::new(&executable);
        command
            .args(["-p", "--restricted", "--tools", "", "--strict-mcp-config"])
            .args(["--setting-sources", "", "--mcp-config"])
            .arg(&config)
            .arg("--settings")
            .arg(&settings)
            .arg("--allowedTools")
            .args(
                exercises
                    .iter()
                    .map(|(name, _)| format!("mcp__grading__{name}")),
            )
            .args([
                "--model",
                "claude-opus-5-5",
                "--max-turns",
                "9",
                "--output-format",
                "json",
            ])
            .current_dir(&start);
        for (name, _) in std::env::vars_os() {
            let shown = name.to_string_lossy();
            if shown.starts_with("ANTHROPIC_") || shown.starts_with("CLAUDE_") {
                command.env_remove(&name);
            }
        }
        command
            .env("HOME", &home)
            .env("CLAUDE_CONFIG_DIR", home.join(".claude"))
            .env("ANTHROPIC_API_KEY", "local-probe-only")
            .env(
                "ANTHROPIC_BASE_URL",
                format!("http://127.0.0.1:{}", api.port),
            )
            .env("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC", "1")
            .env("ENABLE_TOOL_SEARCH", "false");
        crate::child::run_input(
            &mut command,
            Some(b"Exercise the supplied grading tool once."),
            Duration::from_secs(60),
        )
        .map_err(|error| format!("cannot run claude: {error}"))
    })();
    let requests = api.stop();
    let _ = std::fs::remove_dir_all(&start);
    let ran = ran?;
    require(ran.code.is_some(), "client probe timed out after 60 s")?;
    require(ran.code == Some(0), "client probe failed")?;
    require(
        requests.len() > exercises.len(),
        "client probe did not execute every grading tool",
    )?;
    let mut tools: Vec<String> = get(&requests[0], "tools")
        .and_then(Value::as_array)
        .unwrap_or_default()
        .iter()
        .filter_map(|tool| get_str(tool, "name").map(str::to_string))
        .collect();
    tools.sort();
    tools.dedup();
    let mut expected: Vec<String> = grading_tools::TOOLS
        .iter()
        .map(|name| format!("mcp__grading__{name}"))
        .collect();
    expected.sort();
    require(tools == expected, "unexpected native or MCP tools")?;
    let context = json(&Value::Array(requests.clone()));
    require(
        !context.contains(CLAUDE_MARKERS[0]),
        "ambient project context was loaded",
    )?;
    require(
        !context.contains(CLAUDE_MARKERS[1]),
        "ambient ancestor context was loaded",
    )?;
    for marker in ["BRANCH", "SUBJECT", "UNTRACKED"] {
        require(
            !context.contains(&format!("{marker}-GRADING-PROBE-MARKER")),
            "the enclosing repository's git status was loaded",
        )?;
    }
    let last = requests.last().cloned().unwrap_or(Value::Null);
    let mut results = Object::default();
    for message in get(&last, "messages")
        .and_then(Value::as_array)
        .unwrap_or_default()
    {
        for block in get(message, "content")
            .and_then(Value::as_array)
            .unwrap_or_default()
        {
            if get_str(block, "type") == Some("tool_result")
                && let Some(id) = get_str(block, "tool_use_id")
            {
                results.insert(id.to_string(), block.clone());
            }
        }
    }
    for (name, _) in &exercises {
        let result = results.get(&format!("probe-{name}"));
        require(
            result.is_some_and(|block| !get(block, "is_error").is_some_and(Value::truthy)),
            &format!("client grading tool failed: {name}"),
        )?;
    }
    for name in ["run", "validate"] {
        let content = results
            .get(&format!("probe-{name}"))
            .and_then(|block| get(block, "content"))
            .map_or_else(String::new, |content| content_text(content, ""));
        require(
            exit_code_zero(&content),
            &format!("client grading tool returned failure: {name}"),
        )?;
    }
    require(
        context.contains("focused inspection"),
        "grading inspection did not complete",
    )?;
    written(&work)?;
    Ok(object(vec![
        ("tools", strings(expected.iter().map(String::as_str))),
        ("ambient_markers_absent", Value::Bool(true)),
        ("inspection_completed", Value::Bool(true)),
        ("all_tools_completed", Value::Bool(true)),
        ("paid_calls", Value::Int("0".into())),
    ]))
}

/// The Messages API reply to a request: a call to `exercise`'s grading tool, or the closing text.
fn claude_reply(exercise: Option<&(String, Value)>, stream: bool) -> (&'static str, Vec<u8>) {
    let zero = || Value::Int("0".into());
    let one = || Value::Int("1".into());
    let (content, reason) = match exercise {
        Some((name, input)) => (
            object(vec![
                ("type", text("tool_use")),
                ("id", Value::Str(format!("probe-{name}"))),
                ("name", Value::Str(format!("mcp__grading__{name}"))),
                ("input", input.clone()),
            ]),
            "tool_use",
        ),
        None => (
            object(vec![
                ("type", text("text")),
                ("text", text("probe complete")),
            ]),
            "end_turn",
        ),
    };
    let message = |content: Value, reason: Value| {
        object(vec![
            ("id", text("msg_probe")),
            ("type", text("message")),
            ("role", text("assistant")),
            ("model", text("claude-opus-5-5")),
            ("content", content),
            ("stop_reason", reason),
            ("stop_sequence", Value::Null),
            (
                "usage",
                object(vec![("input_tokens", one()), ("output_tokens", one())]),
            ),
        ])
    };
    if !stream {
        let whole = message(Value::Array(vec![content]), text(reason));
        return ("application/json", json(&whole).into_bytes());
    }
    let mut started = content.clone();
    if let (Value::Object(block), Some(_)) = (&mut started, exercise) {
        block.insert("input".into(), Value::Object(Object::default()));
    }
    let mut events = vec![
        object(vec![
            ("type", text("message_start")),
            ("message", message(Value::Array(Vec::new()), Value::Null)),
        ]),
        object(vec![
            ("type", text("content_block_start")),
            ("index", zero()),
            ("content_block", started),
        ]),
    ];
    if let Some((_, input)) = exercise {
        events.push(object(vec![
            ("type", text("content_block_delta")),
            ("index", zero()),
            (
                "delta",
                object(vec![
                    ("type", text("input_json_delta")),
                    ("partial_json", Value::Str(json(input))),
                ]),
            ),
        ]));
    }
    events.extend([
        object(vec![
            ("type", text("content_block_stop")),
            ("index", zero()),
        ]),
        object(vec![
            ("type", text("message_delta")),
            (
                "delta",
                object(vec![
                    ("stop_reason", text(reason)),
                    ("stop_sequence", Value::Null),
                ]),
            ),
            ("usage", object(vec![("output_tokens", one())])),
        ]),
        object(vec![("type", text("message_stop"))]),
    ]);
    let payload: String = events.iter().fold(String::new(), |mut out, event| {
        let _ = write!(
            out,
            "event: {}\ndata: {}\n\n",
            get_str(event, "type").unwrap_or(""),
            json(event)
        );
        out
    });
    ("text/event-stream", payload.into_bytes())
}

/// Write `<work>/home/.codex/config.toml` for a grading session: the clean-context settings, the pinned model
/// catalog, every native tool off and the grading MCP server served by this executable.
pub fn configure_codex(
    checkout: &Path,
    work: &Path,
    bench_tools: &Path,
    protected: &[PathBuf],
    execution_policy: Option<&Path>,
) -> S<PathBuf> {
    let policy = match execution_policy {
        Some(path) => Some(
            String::from_utf8(read(path)?).map_err(|_| format!("{}: not UTF-8", path.display()))?,
        ),
        None => None,
    };
    let config = clean_context::configure_codex(&work.join("home"), work, None, policy.as_deref())?;
    let catalog = work.join("home/.codex/grading-models.json");
    write(&catalog, read(&checkout.join(CATALOG))?)?;
    let quoted = |value: &str| json(&text(value));
    let mut before = vec![
        format!(
            "model_catalog_json = {}",
            quoted(&catalog.to_string_lossy())
        ),
        "web_search = \"disabled\"".to_string(),
        "approval_policy = \"never\"".to_string(),
        "sandbox_mode = \"read-only\"".to_string(),
        "agents.enabled = false".to_string(),
        "features.multi_agent_v2 = false".to_string(),
        "features.code_mode = false".to_string(),
        "tools.experimental_request_user_input.enabled = false".to_string(),
    ];
    before.extend(
        DISABLED_FEATURES
            .iter()
            .map(|name| format!("features.{name} = false")),
    );
    let mut arguments = vec![
        "grading".to_string(),
        "tools".to_string(),
        "--work".to_string(),
        work.to_string_lossy().into_owned(),
        "--policy".to_string(),
        work.join("command-policy.json")
            .to_string_lossy()
            .into_owned(),
        "--bench-tools".to_string(),
        review_isolation::resolve(bench_tools)
            .to_string_lossy()
            .into_owned(),
    ];
    for path in protected {
        arguments.push("--protected".to_string());
        arguments.push(
            review_isolation::resolve(path)
                .to_string_lossy()
                .into_owned(),
        );
    }
    let mut enabled: Vec<&str> = grading_tools::TOOLS.to_vec();
    enabled.sort_unstable();
    let own = review_isolation::own_executable()?;
    let after = [
        "[mcp_servers.grading]".to_string(),
        format!("command = {}", quoted(&own.to_string_lossy())),
        format!(
            "args = {}",
            json(&strings(arguments.iter().map(String::as_str)))
        ),
        "required = true".to_string(),
        "default_tools_approval_mode = \"approve\"".to_string(),
        format!("enabled_tools = {}", json(&strings(enabled))),
    ];
    let middle = String::from_utf8_lossy(&read(&config)?).into_owned();
    write(
        &config,
        format!("{}\n{middle}{}\n", before.join("\n"), after.join("\n")),
    )?;
    Ok(config)
}

/// The Responses API events for request `index`: a call to `exercise`, or the closing message.
fn codex_reply(index: usize, exercise: Option<&(String, Value)>) -> Vec<u8> {
    let item = match exercise {
        Some((name, arguments)) => {
            let mut item = vec![
                ("type", text("function_call")),
                ("id", Value::Str(format!("fc_{index}"))),
                ("call_id", Value::Str(format!("probe-{name}"))),
                ("name", text(name)),
                ("arguments", Value::Str(json(arguments))),
                ("status", text("completed")),
            ];
            if !RESOURCE_TOOLS.contains(&name.as_str()) {
                item.push(("namespace", text(NAMESPACE)));
            }
            object(item)
        }
        None => object(vec![
            ("type", text("message")),
            ("id", Value::Str(format!("msg_{index}"))),
            ("status", text("completed")),
            ("role", text("assistant")),
            (
                "content",
                Value::Array(vec![object(vec![
                    ("type", text("output_text")),
                    ("text", text("probe complete")),
                    ("annotations", Value::Array(Vec::new())),
                ])]),
            ),
        ]),
    };
    let one = || Value::Int("1".into());
    let document = |status: &str, output: Value| {
        object(vec![
            ("id", Value::Str(format!("resp_{index}"))),
            ("object", text("response")),
            ("created_at", one()),
            ("status", text(status)),
            ("model", text(PROBE_MODEL)),
            ("output", output),
            (
                "usage",
                object(vec![
                    ("input_tokens", one()),
                    ("output_tokens", one()),
                    ("total_tokens", Value::Int("2".into())),
                    (
                        "input_tokens_details",
                        object(vec![("cached_tokens", Value::Int("0".into()))]),
                    ),
                    (
                        "output_tokens_details",
                        object(vec![("reasoning_tokens", Value::Int("0".into()))]),
                    ),
                ]),
            ),
        ])
    };
    let zero = || Value::Int("0".into());
    let events = [
        object(vec![
            ("type", text("response.created")),
            (
                "response",
                document("in_progress", Value::Array(Vec::new())),
            ),
        ]),
        object(vec![
            ("type", text("response.output_item.added")),
            ("output_index", zero()),
            ("item", item.clone()),
        ]),
        object(vec![
            ("type", text("response.output_item.done")),
            ("output_index", zero()),
            ("item", item.clone()),
        ]),
        object(vec![
            ("type", text("response.completed")),
            ("response", document("completed", Value::Array(vec![item]))),
        ]),
    ];
    events
        .iter()
        .fold(String::new(), |mut out, event| {
            let _ = write!(out, "data: {}\n\n", json(event));
            out
        })
        .into_bytes()
}

/// Every `rollout-*.jsonl` under `directory`, sorted; none when it does not exist.
pub fn rollouts(directory: &Path) -> S<Vec<PathBuf>> {
    fn walk(directory: &Path, found: &mut Vec<PathBuf>) -> S<()> {
        for entry in std::fs::read_dir(directory).map_err(io(directory))? {
            let entry = entry.map_err(io(directory))?;
            let path = entry.path();
            let kind = entry.file_type().map_err(io(&path))?;
            if kind.is_dir() {
                walk(&path, found)?;
            } else {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if name.starts_with("rollout-") && name.ends_with(".jsonl") {
                    found.push(path);
                }
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

/// Run the installed `codex` against a local fake Responses API that calls each grading tool and native resource
/// helper once, and check its tools, model, context and saved session. No model is called. With `evidence`, a
/// new directory keeps the requests, logs and receipt.
pub fn probe_codex(checkout: &Path, bench_tools: &Path, evidence: Option<&Path>) -> S<Value> {
    let executable = review_isolation::which("codex").ok_or("Codex client is not installed")?;
    let started = Instant::now();
    let scratch = Scratch::new("codex-grading-")?;
    let root = match evidence {
        None => scratch.0.clone(),
        Some(directory) => {
            if let Some(parent) = directory.parent() {
                mkdir(parent)?;
            }
            std::fs::create_dir(directory).map_err(io(directory))?;
            review_isolation::resolve(directory)
        }
    };
    let work = root.join("work");
    std::fs::create_dir(&work).map_err(io(&work))?;
    clean_context::prepare(&work)?;
    let home = work.join("home");
    write(
        &root.join("AGENTS.md"),
        "ANCESTOR-CODEX-GRADING-PROBE-MARKER",
    )?;
    write(
        &work.join("AGENTS.md"),
        "AMBIENT-CODEX-GRADING-PROBE-MARKER",
    )?;
    mkdir(&work.join(".codex"))?;
    write(
        &work.join(".codex/config.toml"),
        "developer_instructions = \"PROJECT-CODEX-GRADING-PROBE-MARKER\"",
    )?;
    mkdir(&work.join(".agents/skills/probe"))?;
    write(
        &work.join(".agents/skills/probe/SKILL.md"),
        "---\nname: probe\ndescription: SKILL-CODEX-GRADING-PROBE-MARKER\n---\n",
    )?;
    git_markers(
        &root,
        "BRANCH-CODEX-GIT-PROBE-MARKER",
        "SUBJECT-CODEX-GIT-PROBE-MARKER",
    )?;
    write(&root.join("UNTRACKED-CODEX-GIT-PROBE-MARKER"), "")?;
    let mut exercises: Vec<(String, Value)> =
        probe_workspace(&work, &work.join("command-policy.json"), bench_tools)?
            .into_iter()
            .map(|(name, input)| (name.to_string(), input))
            .collect();
    let private = root.join("protected.txt");
    write(&private, "PRIVATE-CODEX-GRADING-PROBE-MARKER")?;
    let server = object(vec![("server", text("grading"))]);
    exercises.push(("list_mcp_resources".into(), server.clone()));
    exercises.push(("list_mcp_resource_templates".into(), server));
    exercises.push((
        "read_mcp_resource".into(),
        object(vec![
            ("server", text("grading")),
            (
                "uri",
                Value::Str(format!("file://{}", private.to_string_lossy())),
            ),
        ]),
    ));
    let replies = exercises.clone();
    let mut api = FakeApi::start(Box::new(move |path, _, index| {
        (path == "/v1/responses").then(|| {
            (
                "text/event-stream",
                codex_reply(index, replies.get(index - 1)),
            )
        })
    }))?;
    let config = configure_codex(
        checkout,
        &work,
        bench_tools,
        std::slice::from_ref(&private),
        None,
    )?;
    let body = String::from_utf8_lossy(&read(&config)?).into_owned();
    write(
        &config,
        format!(
            "model_provider = \"grading_probe\"\nmodel = \"{PROBE_MODEL}\"\n{body}\n[model_providers.grading_probe]\nname = \"Local grading compatibility probe\"\nbase_url = \"http://127.0.0.1:{}/v1\"\nenv_key = \"BENCH_CODEX_GRADING_PROBE_KEY\"\nwire_api = \"responses\"\nrequires_openai_auth = false\nsupports_websockets = false\nrequest_max_retries = 0\nstream_max_retries = 0\n",
            api.port
        ),
    )?;
    let environment = |command: &mut Command| {
        command
            .env_clear()
            .env(
                "PATH",
                std::env::var_os("PATH").unwrap_or_else(|| "/usr/bin:/bin".into()),
            )
            .env("LANG", "C.UTF-8")
            .env("HOME", &home)
            .env("CODEX_HOME", home.join(".codex"))
            .env("TMPDIR", work.join("tmp"))
            .env("BENCH_CODEX_GRADING_PROBE_KEY", "local-probe-only");
    };
    let ran = (|| {
        let mut version = Command::new(&executable);
        version.arg("--version");
        environment(&mut version);
        let version = crate::child::run(&mut version, Duration::from_secs(15))
            .map_err(|error| format!("cannot run codex: {error}"))?;
        let mut session = Command::new(&executable);
        session
            .args([
                "exec",
                "--skip-git-repo-check",
                "--ignore-rules",
                "--json",
                "-",
            ])
            .current_dir(&work);
        environment(&mut session);
        let session = crate::child::run_input(
            &mut session,
            Some(b"Exercise every supplied grading tool once."),
            Duration::from_secs(60),
        )
        .map_err(|error| format!("cannot run codex: {error}"))?;
        Ok::<_, String>((version, session))
    })();
    let requests = api.stop();
    let (version, session) = ran?;
    write(
        &root.join("requests.json"),
        indented(&Value::Array(requests.clone())) + "\n",
    )?;
    write(&root.join("stdout.jsonl"), &session.stdout)?;
    write(&root.join("stderr.txt"), &session.stderr)?;
    require(
        version.code == Some(0),
        "cannot check the Codex client version",
    )?;
    require(
        session.code.is_some(),
        "Codex compatibility probe timed out after 60 s",
    )?;
    require(
        session.code == Some(0),
        "Codex compatibility probe failed; inspect stderr.txt",
    )?;
    require(
        requests.len() == exercises.len() + 1,
        "client did not exercise every grading tool and resource helper",
    )?;
    let mut expected: Vec<&str> = grading_tools::TOOLS.to_vec();
    expected.sort_unstable();
    for request in &requests {
        let advertised = get(request, "tools")
            .and_then(Value::as_array)
            .unwrap_or_default();
        let namespaces: Vec<&Value> = advertised
            .iter()
            .filter(|tool| get_str(tool, "type") == Some("namespace"))
            .collect();
        require(
            namespaces.len() == 1 && get_str(namespaces[0], "name") == Some(NAMESPACE),
            "unexpected native tool namespaces",
        )?;
        let mut names: Vec<&str> = get(namespaces[0], "tools")
            .and_then(Value::as_array)
            .unwrap_or_default()
            .iter()
            .filter_map(|tool| get_str(tool, "name"))
            .collect();
        names.sort_unstable();
        names.dedup();
        let mut helpers: Vec<&str> = advertised
            .iter()
            .filter(|tool| get_str(tool, "type") != Some("namespace"))
            .map(|tool| get_str(tool, "name").unwrap_or(""))
            .collect();
        helpers.sort_unstable();
        helpers.dedup();
        require(
            names == expected && helpers == RESOURCE_TOOLS,
            "unexpected native or MCP tools",
        )?;
        require(
            get_str(request, "model") == Some(PROBE_MODEL),
            "client requested an unexpected model",
        )?;
    }
    let context = json(&Value::Array(requests.clone()));
    require(
        !context.contains("CODEX-GRADING-PROBE-MARKER"),
        "ambient context was loaded",
    )?;
    require(
        !context.contains("CODEX-GIT-PROBE-MARKER"),
        "the enclosing repository's git state was sent to the model",
    )?;
    require(
        !context.contains("### Available skills\n- "),
        "ambient skills were loaded",
    )?;
    let saved = rollouts(&home.join(".codex/sessions"))?;
    require(
        saved.len() == 1,
        "client did not save exactly one fresh grading session",
    )?;
    for path in &saved {
        require(
            !String::from_utf8_lossy(&read(path)?).contains("CODEX-GRADING-PROBE-MARKER"),
            "ambient context was loaded into the saved session",
        )?;
    }
    let last = requests.last().cloned().unwrap_or(Value::Null);
    let mut results = Object::default();
    for item in get(&last, "input")
        .and_then(Value::as_array)
        .unwrap_or_default()
    {
        if get_str(item, "type") == Some("function_call_output")
            && let (Some(id), Some(output)) = (get_str(item, "call_id"), get(item, "output"))
        {
            results.insert(id.to_string(), output.clone());
        }
    }
    for (name, _) in &exercises {
        let output = results
            .get(&format!("probe-{name}"))
            .ok_or_else(|| format!("grading tool result missing: {name}"))?;
        let shown = content_text(output, "\n");
        require(
            !shown.contains("approval policy is never") && !shown.contains("unsupported call"),
            &format!("grading tool returned an error: {name}"),
        )?;
    }
    for name in ["run", "validate"] {
        let last_block = results
            .get(&format!("probe-{name}"))
            .and_then(Value::as_array)
            .and_then(<[Value]>::last)
            .and_then(|block| get_str(block, "text"))
            .unwrap_or("");
        require(
            exit_code_zero(last_block),
            &format!("grading tool failed: {name}"),
        )?;
    }
    require(
        results
            .get("probe-read_mcp_resource")
            .is_some_and(|output| {
                json(output)
                    .to_lowercase()
                    .contains("resources/read failed")
            }),
        "native resource reader did not reject the private file",
    )?;
    let audit = String::from_utf8_lossy(&read(&work.join("policy-audit.jsonl"))?).into_owned();
    let mut audited = Vec::new();
    let mut completed = true;
    for line in audit.lines() {
        let row = parse(line.as_bytes())?;
        audited.push(get_str(&row, "tool").unwrap_or("").to_string());
        completed &= get_str(&row, "status") == Some("completed");
    }
    audited.sort();
    audited.dedup();
    require(
        completed && audited == expected,
        "grading policy audit did not confirm all tools",
    )?;
    require(
        context.contains("focused inspection"),
        "inspection did not complete",
    )?;
    written(&work)?;
    let elapsed = (started.elapsed().as_secs_f64() * 1000.0).round() / 1000.0;
    let logs = match evidence {
        Some(_) => Value::Object(
            ["requests.json", "stdout.jsonl", "stderr.txt"]
                .into_iter()
                .map(|name| (name.to_string(), path_text(&root.join(name))))
                .collect(),
        ),
        None => Value::Object(Object::default()),
    };
    let names: Vec<String> = expected
        .iter()
        .map(|name| format!("mcp__grading__{name}"))
        .collect();
    let receipt = object(vec![
        ("client", text("codex")),
        (
            "cli_version",
            Value::Str(String::from_utf8_lossy(&version.stdout).trim().to_string()),
        ),
        ("tools", strings(names.iter().map(String::as_str))),
        ("native_helpers", strings(RESOURCE_TOOLS)),
        ("native_resource_helpers_confined", Value::Bool(true)),
        ("catalog_cli_version", text(CATALOG_CLI_VERSION)),
        (
            "catalog_sha256",
            Value::Str(sha256::hex_digest(&read(&checkout.join(CATALOG))?)),
        ),
        (
            "config_sha256",
            Value::Str(sha256::hex_digest(&read(&config)?)),
        ),
        ("saved_session_markers_absent", Value::Bool(true)),
        ("ambient_markers_absent", Value::Bool(true)),
        ("ambient_skills_absent", Value::Bool(true)),
        ("all_tools_completed", Value::Bool(true)),
        ("inspection_completed", Value::Bool(true)),
        ("fresh_home", Value::Bool(true)),
        ("budget_enforced", Value::Bool(false)),
        ("dispatch_ready", Value::Bool(true)),
        ("paid_calls", Value::Int("0".into())),
        ("elapsed_seconds", Value::Float(elapsed)),
        ("logs", logs),
    ]);
    if evidence.is_some() {
        write(&root.join("receipt.json"), indented(&receipt) + "\n")?;
    }
    Ok(receipt)
}

fn version_of(stdout: &[u8]) -> Option<String> {
    Regex::new(r"\d+\.\d+\.\d+")
        .ok()?
        .find(&String::from_utf8_lossy(stdout))
        .map(|found| found.as_str().to_string())
}

/// The installed client's version, as `<client> --version` reports it, when it is `expected`.
fn pinned_version(client: &str, label: &str, expected: &str) -> S<String> {
    let mut command = Command::new(client);
    command.arg("--version");
    let ran = crate::child::run(&mut command, Duration::from_secs(15));
    let ran = match ran {
        Ok(ran) if ran.code.is_some() => ran,
        _ => return Err(format!("cannot check the pinned {label} client version")),
    };
    match version_of(&ran.stdout) {
        Some(version) if ran.code == Some(0) && version == expected => Ok(version),
        _ => Err(format!(
            "{label} client version differs from pinned {expected}"
        )),
    }
}

fn read_json(path: &Path) -> Option<Value> {
    std::fs::read(path).ok().and_then(|raw| parse(&raw).ok())
}

/// Check the Codex ChatGPT sign-in and the client version, returning the version.
pub fn codex_preflight(expected: &str, home: &Path) -> S<String> {
    let auth = read_json(&home.join(".codex/auth.json")).unwrap_or(Value::Null);
    let signed_in = get_str(&auth, "auth_mode") == Some("chatgpt")
        && get(&auth, "tokens")
            .and_then(|tokens| get_str(tokens, "access_token"))
            .is_some_and(|token| !token.is_empty());
    if !signed_in {
        return Err(
            "Codex ChatGPT credential material is absent or invalid; credentials were not logged"
                .to_string(),
        );
    }
    pinned_version("codex", "Codex", expected)
}

/// Check the Claude sign-in and the client version, returning the version.
pub fn claude_preflight(expected: &str, home: &Path) -> S<String> {
    let credentials = read_json(&home.join(".claude/.credentials.json")).unwrap_or(Value::Null);
    let signed_in = get(&credentials, "claudeAiOauth")
        .filter(|auth| auth.as_object().is_some())
        .and_then(|auth| get(auth, "accessToken"))
        .is_some_and(Value::truthy);
    if !signed_in {
        return Err(
            "Claude credential material is absent or invalid; credentials were not logged"
                .to_string(),
        );
    }
    pinned_version("claude", "Claude", expected)
}

/// The latest dated price entry for `model`: `rates.current.json` for a Codex model, `rates.json` otherwise.
pub fn rate_for(checkout: &Path, model: &str) -> S<Option<Value>> {
    let name = if model.starts_with("gpt-") {
        "rates.current.json"
    } else {
        "rates.json"
    };
    let path = checkout.join("bench").join(name);
    let catalog =
        parse(&read(&path)?).map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    let mut latest: Option<&Value> = None;
    for rate in get(&catalog, "rates")
        .and_then(Value::as_array)
        .unwrap_or_default()
    {
        if get_str(rate, "model") == Some(model)
            && latest.is_none_or(|best| get_str(rate, "as_of") >= get_str(best, "as_of"))
        {
            latest = Some(rate);
        }
    }
    Ok(latest.cloned())
}

/// A number field of a rate entry.
pub fn price(rate: &Value, field: &str) -> S<f64> {
    match get(rate, field) {
        Some(Value::Float(number)) => Ok(*number),
        Some(Value::Int(digits)) => digits.parse().map_err(|_| format!("'{field}'")),
        _ => Err(format!("'{field}'")),
    }
}

/// `f"{a / b:g}"` read back as a float, the multiplier the meters were given on their command line.
pub fn multiplier(numerator: f64, denominator: f64) -> f64 {
    let shown = usage::general(numerator / denominator);
    usage::float(&shown).unwrap_or(numerator / denominator)
}

fn allowed_call(payload: &Value) -> bool {
    let namespace = get(payload, "namespace").and_then(Value::as_str);
    let name = get_str(payload, "name").unwrap_or("");
    let grading = namespace == Some(NAMESPACE) || namespace.is_none();
    let tool = match namespace {
        Some(NAMESPACE) => grading_tools::TOOLS.contains(&name),
        _ => name
            .strip_prefix("mcp__grading__")
            .is_some_and(|name| grading_tools::TOOLS.contains(&name)),
    };
    let helper = RESOURCE_TOOLS.contains(&name) && matches!(namespace, None | Some("functions"));
    get_str(payload, "type") == Some("function_call") && ((tool && grading) || helper)
}

/// The Codex session's root id, models, subagents, tool audit and priced usage from its saved rollouts.
pub fn codex_evidence(work: &Path, rate: &Value) -> S<Value> {
    let paths = rollouts(&work.join("home/.codex/sessions"))?;
    let mut problems = std::collections::BTreeSet::new();
    let mut metas = Vec::new();
    for path in &paths {
        let mut meta = Value::Object(Object::default());
        let raw = String::from_utf8(read(path)?).map_err(|error| error.to_string())?;
        for line in raw.lines() {
            let record = parse(line.as_bytes())?;
            let payload = get(&record, "payload")
                .filter(|payload| payload.truthy())
                .cloned()
                .unwrap_or(Value::Object(Object::default()));
            let kind = get_str(&record, "type");
            if kind == Some("session_meta") && !meta.truthy() {
                meta = payload.clone();
            }
            if kind == Some("response_item")
                && get_str(&payload, "type")
                    .is_some_and(|kind| usage::codex::TOOL_TYPES.contains(&kind))
                && !allowed_call(&payload)
            {
                problems
                    .insert("grader invoked a tool outside the grading MCP catalog".to_string());
            }
        }
        metas.push(meta);
    }
    let roots: Vec<&Value> = metas
        .iter()
        .filter(|meta| !get(meta, "parent_thread_id").is_some_and(Value::truthy))
        .collect();
    if roots.len() != 1 {
        problems.insert("expected exactly one Codex root transcript".to_string());
    }
    let session = match roots.as_slice() {
        [root] => get(root, "id").cloned().unwrap_or(Value::Null),
        _ => Value::Null,
    };
    let input = price(rate, "input")?;
    let prices = usage::codex::Prices {
        input,
        output: price(rate, "output")?,
        cached_mult: multiplier(price(rate, "cache_read")?, input),
        write_mult: multiplier(price(rate, "cache_write_5m")?, input),
        long_context: (get_str(rate, "model") == Some("gpt-6.1-sol")).then_some(
            usage::codex::LongContext {
                above: 272_000,
                input_mult: 2.0,
                output_mult: 1.5,
            },
        ),
    };
    let (models, requests, amount) = if paths.is_empty() {
        (
            Value::Array(Vec::new()),
            Value::Int("0".into()),
            Value::Null,
        )
    } else {
        let metered = usage::codex::run(&usage::codex::Options {
            source: usage::codex::Source::Paths(
                paths
                    .iter()
                    .map(|path| path.as_os_str().as_encoded_bytes().to_vec())
                    .collect(),
            ),
            prices,
            report: None,
            output: usage::codex::Output::Json,
        });
        match metered.code {
            0 => {
                let report = parse(&metered.stdout)?;
                let total = get(&report, "total").cloned().unwrap_or(Value::Null);
                let cost = price(&total, "cost")?;
                (
                    get(&total, "models").cloned().unwrap_or(Value::Null),
                    get(&total, "turns").cloned().unwrap_or(Value::Null),
                    Value::Float(usage::round_to(cost, 6)),
                )
            }
            1 => (
                Value::Array(Vec::new()),
                Value::Int("0".into()),
                Value::Null,
            ),
            _ => return Err(String::from_utf8_lossy(&metered.stderr).trim().to_string()),
        }
    };
    if matches!(amount, Value::Null) {
        problems.insert("no billed Codex request usage found".to_string());
    }
    Ok(object(vec![
        ("session_id", session),
        ("models_observed", models),
        (
            "subagents",
            Value::Int((metas.len() - roots.len()).to_string()),
        ),
        (
            "audit_violations",
            strings(problems.iter().map(String::as_str)),
        ),
        (
            "usage",
            object(vec![
                ("priced_total_usd", amount.clone()),
                ("low", amount.clone()),
                ("high", amount),
                ("billing", text("list-price-equivalent")),
                ("requests", requests),
                ("rate", rate.clone()),
            ]),
        ),
    ]))
}

#[cfg(test)]
mod tests;
