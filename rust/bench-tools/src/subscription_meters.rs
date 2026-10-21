//! Read the pinned clients' subscription meters and say when a reading stops a queue. No model is called.

use crate::normalize_review::utc_now;
use crate::pyjson::{self, Dumps, Duplicates, Object, Value};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

pub const SIGN_IN_MARGIN_SECONDS: f64 = 1800.0;
const PROVIDER_SECONDS: u64 = 30;

/// A client whose meters are read: its executable and the version its pin names.
pub struct Pinned {
    pub name: String,
    pub path: PathBuf,
    pub version: String,
}

/// No reading says how much of each pinned client's quota is used. `sign_in` marks a stored Claude sign-in that is
/// missing or is not JSON.
#[derive(Debug)]
pub struct Unreadable {
    pub message: String,
    pub sign_in: bool,
}

fn unreadable(message: impl Into<String>) -> Unreadable {
    Unreadable {
        message: message.into(),
        sign_in: false,
    }
}

fn get<'a>(value: &'a Value, key: &str) -> &'a Value {
    static NULL: Value = Value::Null;
    value
        .as_object()
        .and_then(|object| object.get(key))
        .unwrap_or(&NULL)
}

/// `value` as the JSON object a provider should have sent; one it left out or sent empty is empty.
fn json_object<'a>(value: &'a Value, what: &str) -> Result<Option<&'a Object>, String> {
    if !value.truthy() {
        return Ok(None);
    }
    value
        .as_object()
        .map(Some)
        .ok_or_else(|| format!("{what} is not an object"))
}

fn member<'a>(value: &'a Value, key: &str, what: &str) -> Result<&'a Value, String> {
    static NULL: Value = Value::Null;
    Ok(json_object(value, what)?
        .and_then(|object| object.get(key))
        .unwrap_or(&NULL))
}

fn stored_sign_in(home: &std::path::Path) -> Result<Value, Unreadable> {
    let path = home.join(".claude/.credentials.json");
    std::fs::read_to_string(&path)
        .map_err(|error| error.to_string())
        .and_then(|text| {
            pyjson::loads(&text, Duplicates::KeepLast).map_err(|error| error.to_string())
        })
        .map_err(|error| Unreadable {
            message: format!("cannot read {}: {error}", path.display()),
            sign_in: true,
        })
}

fn claude(pinned: &Pinned, env: &[(String, String)], usage: &mut Object) -> Result<(), Unreadable> {
    let home = env
        .iter()
        .find(|(name, _)| name == "HOME")
        .map(|(_, value)| PathBuf::from(value))
        .ok_or_else(|| {
            unreadable("HOME is not set, so the stored Claude sign-in cannot be found")
        })?;
    let credentials = stored_sign_in(&home)?;
    let oauth = get(&credentials, "claudeAiOauth");
    let token = get(oauth, "accessToken")
        .as_str()
        .ok_or_else(|| unreadable("the stored Claude sign-in has no claudeAiOauth.accessToken"))?;
    let version = pinned.version.split_whitespace().next().unwrap_or("");
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(PROVIDER_SECONDS)))
        .http_status_as_error(false)
        .build()
        .into();
    let mut response = agent
        .get("https://api.anthropic.com/api/oauth/usage")
        .header("Authorization", format!("Bearer {token}"))
        .header("anthropic-beta", "oauth-2025-04-20")
        .header("User-Agent", format!("claude-code/{version}"))
        .call()
        .map_err(|error| unreadable(error.to_string()))?;
    let status = response.status();
    if !status.is_success() {
        return Err(unreadable(format!(
            "HTTP Error {}: {}",
            status.as_u16(),
            status.canonical_reason().unwrap_or_default()
        )));
    }
    let body = response
        .body_mut()
        .read_to_string()
        .map_err(|error| unreadable(error.to_string()))?;
    let reading = pyjson::loads(&body, Duplicates::KeepLast)
        .map_err(|error| unreadable(error.to_string()))?;
    json_object(&reading, "the Claude usage response").map_err(unreadable)?;
    usage.insert(
        "claude".into(),
        Value::Object(
            ["five_hour", "seven_day"]
                .into_iter()
                .map(|key| (key.to_string(), get(&reading, key).clone()))
                .collect(),
        ),
    );
    usage.insert(
        "claude_sign_in_expires_at".into(),
        get(oauth, "expiresAt").clone(),
    );
    Ok(())
}

struct Server {
    child: Child,
    lines: mpsc::Receiver<String>,
}

impl Server {
    fn send(&mut self, row: &Value) -> Result<(), String> {
        let stdin = self.child.stdin.as_mut().ok_or("app-server has no input")?;
        let mut line = pyjson::dumps(row, &Dumps::default());
        line.push('\n');
        stdin
            .write_all(line.as_bytes())
            .and_then(|()| stdin.flush())
            .map_err(|error| error.to_string())
    }

    fn receive(&self, identifier: i64) -> Result<Value, String> {
        let deadline = Instant::now() + Duration::from_secs(PROVIDER_SECONDS);
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            let line = match self.lines.recv_timeout(left) {
                Ok(line) => line,
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    return Err("app-server usage timeout".into());
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err("app-server closed before returning usage".into());
                }
            };
            let row =
                pyjson::loads(&line, Duplicates::KeepLast).map_err(|error| error.to_string())?;
            let Some(object) = json_object(&row, "an app-server message")? else {
                continue;
            };
            if object
                .get("id")
                .is_some_and(|id| id.py_eq(&Value::Int(identifier.to_string())))
            {
                if object.get("error").is_some() {
                    return Err("app-server could not return usage".into());
                }
                return object
                    .get("result")
                    .cloned()
                    .ok_or_else(|| "'result'".to_string());
            }
        }
    }

    fn end(mut self) -> Result<(), String> {
        let pid = nix::unistd::Pid::from_raw(i32::try_from(self.child.id()).unwrap_or(i32::MAX));
        let _ = nix::sys::signal::kill(pid, nix::sys::signal::Signal::SIGTERM);
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            if self
                .child
                .try_wait()
                .map_err(|error| error.to_string())?
                .is_some()
            {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
        Err("app-server did not exit within 10 seconds of the reading".into())
    }
}

fn codex(pinned: &Pinned, env: &[(String, String)], usage: &mut Object) -> Result<(), String> {
    let mut child = Command::new(&pinned.path)
        .arg("app-server")
        .env_clear()
        .envs(env.iter().map(|(name, value)| (name, value)))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("{}: {error}", pinned.path.display()))?;
    let stdout = child.stdout.take().ok_or("app-server has no output")?;
    let (sender, lines) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else { break };
            if sender.send(line).is_err() {
                break;
            }
        }
    });
    let mut server = Server { child, lines };
    let parse =
        |text: &str| pyjson::loads(text, Duplicates::KeepLast).map_err(|error| error.to_string());
    let read = (|| {
        server.send(&parse(
            r#"{"id": 1, "method": "initialize", "params": {"clientInfo": {"name": "bench-meter", "version": "1.0"}}}"#,
        )?)?;
        server.receive(1)?;
        server.send(&parse(r#"{"method": "initialized", "params": {}}"#)?)?;
        server.send(&parse(
            r#"{"id": 2, "method": "account/rateLimits/read", "params": {}}"#,
        )?)?;
        let result = server.receive(2)?;
        json_object(&result, "the app-server usage result")?;
        Ok::<_, String>(Value::Object(
            ["ordinaryUsageAllowed", "rateLimits"]
                .into_iter()
                .map(|key| (key.to_string(), get(&result, key).clone()))
                .collect(),
        ))
    })();
    let ended = server.end();
    usage.insert("codex".into(), read?);
    ended
}

/// The percentages each client's usage windows report; a window that reports no finite number is left out.
pub fn used(usage: &Value) -> Result<(Vec<f64>, Vec<f64>), String> {
    let finite = |value: &Value| match value {
        Value::Int(digits) => digits.parse::<f64>().ok(),
        Value::Float(number) if number.is_finite() => Some(*number),
        _ => None,
    };
    let mut claude = Vec::new();
    if let Some(windows) = json_object(get(usage, "claude"), "the Claude reading")? {
        for (name, window) in windows.iter() {
            claude.extend(finite(member(
                window,
                "utilization",
                &format!("the Claude usage window {name}"),
            )?));
        }
    }
    let limits = member(get(usage, "codex"), "rateLimits", "the ChatGPT reading")?;
    let mut codex = Vec::new();
    for key in ["primary", "secondary"] {
        let window = member(limits, key, "the ChatGPT rate limits")?;
        codex.extend(finite(member(
            window,
            "usedPercent",
            &format!("the ChatGPT usage window {key}"),
        )?));
    }
    Ok((claude, codex))
}

/// A reading of each pinned client's meters, stamped `at`; `env` is the environment the Codex app-server runs in,
/// and its `HOME` holds the stored Claude sign-in.
pub fn read(clients: &[Pinned], env: &[(String, String)]) -> Result<Value, Unreadable> {
    checked(clients, meters(clients, env)?)
}

/// The meters as each pinned client reports them, before any check that they say how much quota is used.
pub fn meters(clients: &[Pinned], env: &[(String, String)]) -> Result<Value, Unreadable> {
    let mut usage = Object::default();
    usage.insert("at".into(), Value::Str(utc_now()));
    for client in clients.iter().filter(|client| client.name == "claude") {
        claude(client, env, &mut usage)?;
    }
    for client in clients.iter().filter(|client| client.name == "codex") {
        codex(client, env, &mut usage).map_err(unreadable)?;
    }
    Ok(Value::Object(usage))
}

/// The reading, once it reports a usage window with a finite number for every pinned client.
pub fn checked(clients: &[Pinned], usage: Value) -> Result<Value, Unreadable> {
    let (claude, codex) = used(&usage).map_err(unreadable)?;
    let silent: Vec<&str> = clients
        .iter()
        .map(|client| client.name.as_str())
        .filter(|name| match *name {
            "claude" => claude.is_empty(),
            "codex" => codex.is_empty(),
            _ => true,
        })
        .collect();
    if !silent.is_empty() {
        return Err(unreadable(format!(
            "no usage window was reported for {}",
            silent.join(", ")
        )));
    }
    Ok(usage)
}

pub fn sign_in_expiring(usage: &Value, moment: f64) -> bool {
    let expires = match get(usage, "claude_sign_in_expires_at") {
        Value::Int(digits) => digits.parse::<f64>().ok(),
        Value::Float(number) => Some(*number),
        _ => None,
    };
    expires.is_some_and(|expires| expires / 1000.0 - moment < SIGN_IN_MARGIN_SECONDS)
}

/// The reason the reading stops the queue; `work` names what must not be running when the sign-in is renewed.
pub fn quota_stop(
    usage: &Value,
    threshold: f64,
    moment: f64,
    work: &str,
) -> Result<Option<String>, String> {
    let (claude, codex) = used(usage)?;
    if claude.iter().any(|percent| *percent >= threshold) {
        return Ok(Some("Claude usage reached the quota stop".into()));
    }
    if sign_in_expiring(usage, moment) {
        return Ok(Some(format!(
            "the stored Claude sign-in expires within half an hour; renew it while no {work} runs"
        )));
    }
    let reading = get(usage, "codex");
    if !matches!(reading, Value::Null)
        && !member(reading, "ordinaryUsageAllowed", "the ChatGPT reading")?.truthy()
    {
        return Ok(Some("ChatGPT ordinary usage is unavailable".into()));
    }
    if codex.iter().any(|percent| *percent >= threshold) {
        return Ok(Some("ChatGPT usage reached the quota stop".into()));
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reading(text: &str) -> Value {
        pyjson::loads(text, Duplicates::KeepLast).unwrap()
    }

    #[test]
    fn used_keeps_each_finite_window_and_refuses_a_window_that_is_not_an_object() {
        let usage = reading(
            r#"{"claude": {"five_hour": {"utilization": 12}, "seven_day": {"utilization": null}, "extra": {}},
                "codex": {"rateLimits": {"primary": {"usedPercent": 40.5}, "secondary": {"usedPercent": true}}}}"#,
        );
        assert_eq!(used(&usage).unwrap(), (vec![12.0], vec![40.5]));
        assert_eq!(
            used(&reading(r#"{"claude": {"five_hour": 3}}"#)).unwrap_err(),
            "the Claude usage window five_hour is not an object"
        );
    }

    #[test]
    fn the_stop_follows_claude_then_the_sign_in_then_chatgpt() {
        let at = |text: &str| quota_stop(&reading(text), 95.0, 1_000_000.0, "review").unwrap();
        assert_eq!(
            at(r#"{"claude": {"five_hour": {"utilization": 95}}, "claude_sign_in_expires_at": 1}"#)
                .as_deref(),
            Some("Claude usage reached the quota stop")
        );
        assert_eq!(
            at(r#"{"claude": {"five_hour": {"utilization": 94.9}}, "claude_sign_in_expires_at": 1000000000}"#).as_deref(),
            Some("the stored Claude sign-in expires within half an hour; renew it while no review runs")
        );
        assert_eq!(
            at(
                r#"{"claude": {"five_hour": {"utilization": 1}}, "claude_sign_in_expires_at": 1801000000}"#
            ),
            None
        );
        assert_eq!(
            at(r#"{"codex": {"ordinaryUsageAllowed": false, "rateLimits": {"primary": {"usedPercent": 1}}}}"#).as_deref(),
            Some("ChatGPT ordinary usage is unavailable")
        );
        assert_eq!(
            at(r#"{"codex": {"ordinaryUsageAllowed": true, "rateLimits": {"secondary": {"usedPercent": 99}}}}"#).as_deref(),
            Some("ChatGPT usage reached the quota stop")
        );
    }

    #[test]
    fn a_missing_sign_in_is_unreadable_before_any_provider_call() {
        let home = std::env::temp_dir().join(format!("meters-home-{}", std::process::id()));
        let pinned = [Pinned {
            name: "claude".into(),
            path: "claude".into(),
            version: "9.9.9 (Claude Code)".into(),
        }];
        let error = read(&pinned, &[("HOME".into(), home.display().to_string())]).unwrap_err();
        assert!(error.sign_in, "{}", error.message);
        assert!(error.message.starts_with(&format!(
            "cannot read {}/.claude/.credentials.json: ",
            home.display()
        )));
    }

    #[test]
    fn a_codex_reading_comes_from_the_app_server_and_a_silent_window_is_unreadable() {
        let root = std::env::temp_dir().join(format!("meters-codex-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let server = root.join("codex");
        let source = root.join("codex.sh");
        // A spawn on another test thread holds a copy of this process's open files until it execs, and Linux will
        // not run a file open for writing (ETXTBSY). Copying in a child keeps every writer of the server out of here.
        let install = |limits: &str| {
            std::fs::write(
                &source,
                format!(
                    "#!/bin/sh\nread line\necho '{{\"id\": 1, \"result\": {{}}}}'\nread line\nread line\n\
                     echo '{{\"method\": \"note\"}}'\necho '{{\"id\": 2, \"result\": {{\"ordinaryUsageAllowed\": true, \"rateLimits\": {limits}}}}}'\n\
                     sleep 30\n"
                ),
            )
            .unwrap();
            assert!(
                Command::new("cp")
                    .arg(&source)
                    .arg(&server)
                    .status()
                    .unwrap()
                    .success()
            );
            std::fs::set_permissions(&server, std::os::unix::fs::PermissionsExt::from_mode(0o755))
                .unwrap();
        };
        let pinned = [Pinned {
            name: "codex".into(),
            path: server.clone(),
            version: "0.1.0".into(),
        }];
        install(r#"{"primary": {"usedPercent": 7}}"#);
        let usage = read(&pinned, &[]).unwrap();
        assert_eq!(used(&usage).unwrap(), (vec![], vec![7.0]));
        install("{}");
        assert_eq!(
            read(&pinned, &[]).unwrap_err().message,
            "no usage window was reported for codex"
        );
        std::fs::remove_dir_all(&root).unwrap();
    }
}
