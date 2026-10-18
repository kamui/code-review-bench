#![forbid(unsafe_code)]

use chrono::{DateTime, Datelike, NaiveDate, NaiveDateTime, Timelike};
use serde::Serialize;
use serde_json::{Map, Value};
use std::collections::{BTreeSet, HashMap};
use std::io::BufRead;

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    Io,
    MalformedJson,
    InvalidInput,
    MissingCount,
    CacheBounds,
    Overflow,
}

#[derive(Debug, Serialize)]
pub struct UsageError {
    pub kind: ErrorKind,
    pub detail: String,
}

impl UsageError {
    pub fn new(kind: ErrorKind, detail: impl Into<String>) -> Self {
        Self {
            kind,
            detail: detail.into(),
        }
    }
}

impl std::fmt::Display for UsageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.detail)
    }
}

impl std::error::Error for UsageError {}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct SessionId(String);

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum ResponseId {
    Native(String),
    Ordinal(String),
}

#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct TokenCount(u64);

impl TokenCount {
    fn add(self, other: Self) -> Result<Self, UsageError> {
        self.0
            .checked_add(other.0)
            .map(Self)
            .ok_or_else(|| UsageError::new(ErrorKind::Overflow, "token count exceeds u64"))
    }
}

#[derive(Debug)]
enum Accounting {
    Known(TokenCount),
    Unknown,
}

impl Accounting {
    fn require(&self, field: &str, location: &str) -> Result<TokenCount, UsageError> {
        match self {
            Self::Known(count) => Ok(*count),
            Self::Unknown => Err(UsageError::new(
                ErrorKind::MissingCount,
                format!("{location} missing {field}"),
            )),
        }
    }
}

#[derive(Debug)]
struct Usage {
    input: Accounting,
    cached: Accounting,
    write: Accounting,
    output: Accounting,
    reasoning: Accounting,
}

#[derive(Debug)]
struct Request {
    usage: Usage,
    tool_calls: u64,
}

#[derive(Debug)]
pub struct Rollout {
    requests: Vec<Request>,
    models: BTreeSet<String>,
    first: Option<i64>,
    last: Option<i64>,
    timestamp_is_aware: Option<bool>,
    pub session: Option<SessionId>,
    pub parent: Option<SessionId>,
}

#[derive(Clone, Copy, Debug)]
pub struct Rates {
    input: f64,
    output: f64,
    cached: f64,
    write: f64,
}

impl Rates {
    pub fn new(input: f64, output: f64, cached: f64, write: f64) -> Result<Self, UsageError> {
        if [input, output, cached, write]
            .iter()
            .any(|n| !n.is_finite() || *n < 0.0)
        {
            return Err(UsageError::new(
                ErrorKind::InvalidInput,
                "rates must be finite and non-negative",
            ));
        }
        Ok(Self {
            input,
            output,
            cached,
            write,
        })
    }
}

#[derive(Debug, Serialize)]
pub struct Summary {
    pub turns: u64,
    pub tool_calls: u64,
    pub text_only_turns: u64,
    pub input: TokenCount,
    pub cache_write: TokenCount,
    pub cache_read: TokenCount,
    pub output: TokenCount,
    pub thinking: TokenCount,
    pub models: Vec<String>,
    pub wall_seconds: Option<i64>,
    pub cost: f64,
}

fn object<'a>(value: &'a Value, location: &str) -> Result<&'a Map<String, Value>, UsageError> {
    value.as_object().ok_or_else(|| {
        UsageError::new(
            ErrorKind::InvalidInput,
            format!("{location}: expected an object"),
        )
    })
}

fn count(raw: &Map<String, Value>, field: &str, location: &str) -> Result<Accounting, UsageError> {
    match raw.get(field) {
        None | Some(Value::Null) => Ok(Accounting::Unknown),
        Some(value) => value
            .as_u64()
            .map(|n| Accounting::Known(TokenCount(n)))
            .ok_or_else(|| {
                UsageError::new(
                    ErrorKind::InvalidInput,
                    format!("{location}: {field} must be an integer in 0..=u64::MAX"),
                )
            }),
    }
}

fn session_id(raw: Option<&Value>, location: &str) -> Result<Option<SessionId>, UsageError> {
    match raw {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) if !s.is_empty() => Ok(Some(SessionId(s.clone()))),
        _ => Err(UsageError::new(
            ErrorKind::InvalidInput,
            format!("{location}: session identity must be a nonempty string"),
        )),
    }
}

fn response_id(
    payload: &Map<String, Value>,
    ordinal: Option<&Value>,
    location: &str,
) -> Result<ResponseId, UsageError> {
    match payload.get("response_id") {
        Some(Value::String(s)) if !s.is_empty() => return Ok(ResponseId::Native(s.clone())),
        None | Some(Value::Null) | Some(Value::String(_)) => {}
        _ => {
            return Err(UsageError::new(
                ErrorKind::InvalidInput,
                format!("{location}: response_id must be a string"),
            ));
        }
    }
    let text = match ordinal {
        None | Some(Value::Null) => "None".to_owned(),
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) if !n.to_string().contains(['.', 'e', 'E']) => n.to_string(),
        _ => {
            return Err(UsageError::new(
                ErrorKind::InvalidInput,
                format!("{location}: ordinal must be a string or integer"),
            ));
        }
    };
    Ok(ResponseId::Ordinal(text))
}

fn stamp(raw: Option<&Value>) -> Option<(i64, bool)> {
    let text = raw?.as_str()?;
    let aware = DateTime::parse_from_rfc3339(text).ok().or_else(|| {
        ["%Y%m%dT%H%M%S%.f%#z", "%Y-%m-%dT%H:%M:%S%.f%#z"]
            .iter()
            .find_map(|format| DateTime::parse_from_str(text, format).ok())
    });
    if let Some(time) = aware {
        if !(1..=9999).contains(&time.year()) || time.nanosecond() >= 1_000_000_000 {
            return None;
        }
        return Some((time.timestamp_micros(), true));
    }
    let naive = [
        "%Y-%m-%dT%H:%M:%S%.f",
        "%Y%m%dT%H%M%S%.f",
        "%Y-%m-%d %H:%M:%S%.f",
    ]
    .iter()
    .find_map(|format| NaiveDateTime::parse_from_str(text, format).ok())
    .or_else(|| {
        ["%Y-%m-%d", "%Y%m%d"].iter().find_map(|format| {
            NaiveDate::parse_from_str(text, format)
                .ok()?
                .and_hms_opt(0, 0, 0)
        })
    })?;
    if !(1..=9999).contains(&naive.year()) || naive.nanosecond() >= 1_000_000_000 {
        return None;
    }
    Some((naive.and_utc().timestamp_micros(), false))
}

pub fn read_rollout(reader: impl BufRead, path: &str) -> Result<Rollout, UsageError> {
    let mut rollout = Rollout {
        requests: Vec::new(),
        models: BTreeSet::new(),
        first: None,
        last: None,
        timestamp_is_aware: None,
        session: None,
        parent: None,
    };
    let mut identities = HashMap::new();
    let mut pending_tools = 0_u64;
    let mut have_meta = false;
    for (index, line) in reader.split(b'\n').enumerate() {
        let line = line.map_err(|e| UsageError::new(ErrorKind::Io, format!("{path}: {e}")))?;
        if line.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        let location = format!("{path}:{}", index + 1);
        let raw: Value = serde_json::from_slice(&line).map_err(|e| {
            UsageError::new(
                ErrorKind::MalformedJson,
                format!(
                    "{location}: malformed JSON ({})",
                    if e.is_eof() {
                        "Expecting value"
                    } else {
                        "invalid syntax"
                    }
                ),
            )
        })?;
        let record = object(&raw, &location)?;
        let kind = record.get("type").and_then(Value::as_str);
        let empty = Map::new();
        let payload = match record.get("payload") {
            None | Some(Value::Null) => &empty,
            Some(value) => object(value, &location)?,
        };
        if !matches!(
            kind,
            Some("session_meta" | "turn_context" | "response_item" | "token_usage_record")
        ) {
            continue;
        }
        match kind {
            Some("session_meta") => {
                let native = session_id(payload.get("id"), &location)?;
                let legacy = session_id(payload.get("session_id"), &location)?;
                let parent = session_id(payload.get("parent_thread_id"), &location)?;
                if !have_meta && !payload.is_empty() {
                    rollout.session = native.or(legacy);
                    rollout.parent = parent;
                    have_meta = true;
                }
            }
            Some("turn_context") => {
                if let Some(value) = payload.get("model") {
                    match value {
                        Value::String(model) => {
                            if !model.is_empty() {
                                rollout.models.insert(model.clone());
                            }
                        }
                        Value::Null => {}
                        _ => {
                            return Err(UsageError::new(
                                ErrorKind::InvalidInput,
                                format!("{location}: model must be a string"),
                            ));
                        }
                    }
                }
            }
            Some("response_item")
                if matches!(
                    payload.get("type").and_then(Value::as_str),
                    Some("function_call" | "custom_tool_call" | "local_shell_call")
                ) =>
            {
                pending_tools = pending_tools.checked_add(1).ok_or_else(|| {
                    UsageError::new(ErrorKind::Overflow, "tool count exceeds u64")
                })?;
            }
            Some("token_usage_record") => {
                let raw_usage =
                    payload
                        .get("usage")
                        .and_then(Value::as_object)
                        .ok_or_else(|| {
                            UsageError::new(
                                ErrorKind::InvalidInput,
                                format!("{location}: token_usage_record without usage object"),
                            )
                        })?;
                let usage = Usage {
                    input: count(raw_usage, "input_tokens", &location)?,
                    cached: count(raw_usage, "cached_input_tokens", &location)?,
                    write: count(raw_usage, "cache_write_input_tokens", &location)?,
                    output: count(raw_usage, "output_tokens", &location)?,
                    reasoning: count(raw_usage, "reasoning_output_tokens", &location)?,
                };
                let identity = response_id(payload, record.get("ordinal"), &location)?;
                if let Some(&i) = identities.get(&identity) {
                    let request: &mut Request = &mut rollout.requests[i];
                    request.usage = usage;
                } else {
                    identities.insert(identity, rollout.requests.len());
                    rollout.requests.push(Request {
                        usage,
                        tool_calls: pending_tools,
                    });
                    pending_tools = 0;
                }
                if let Some((time, aware)) = stamp(record.get("timestamp")) {
                    if rollout
                        .timestamp_is_aware
                        .is_some_and(|previous| previous != aware)
                    {
                        return Err(UsageError::new(
                            ErrorKind::InvalidInput,
                            format!("{location}: incompatible timestamps"),
                        ));
                    }
                    rollout.timestamp_is_aware = Some(aware);
                    rollout.first = Some(rollout.first.map_or(time, |old| old.min(time)));
                    rollout.last = Some(rollout.last.map_or(time, |old| old.max(time)));
                }
            }
            _ => {}
        }
    }
    Ok(rollout)
}

pub fn summarize(rollout: &Rollout, path: &str, rates: Rates) -> Result<Summary, UsageError> {
    let mut result = Summary {
        turns: rollout.requests.len() as u64,
        tool_calls: 0,
        text_only_turns: 0,
        input: TokenCount(0),
        cache_write: TokenCount(0),
        cache_read: TokenCount(0),
        output: TokenCount(0),
        thinking: TokenCount(0),
        models: rollout.models.iter().cloned().collect(),
        wall_seconds: match (rollout.first, rollout.last) {
            (Some(first), Some(last)) => Some(
                last.checked_sub(first).ok_or_else(|| {
                    UsageError::new(ErrorKind::Overflow, "timestamp span exceeds i64")
                })? / 1_000_000,
            ),
            _ => None,
        },
        cost: 0.0,
    };
    for (i, request) in rollout.requests.iter().enumerate() {
        let location = format!("{path}: request {}", i + 1);
        let used = &request.usage;
        let input = used.input.require("input_tokens", &location)?;
        let cached = used.cached.require("cached_input_tokens", &location)?;
        let write = used.write.require("cache_write_input_tokens", &location)?;
        let output = used.output.require("output_tokens", &location)?;
        let reasoning = used
            .reasoning
            .require("reasoning_output_tokens", &location)?;
        let subsets = cached.add(write)?;
        let ordinary = input.0.checked_sub(subsets.0).ok_or_else(|| {
            UsageError::new(
                ErrorKind::CacheBounds,
                format!(
                    "{location} has cached+cache_write ({}+{}) above input_tokens ({})",
                    cached.0, write.0, input.0
                ),
            )
        })?;
        result.input = result.input.add(TokenCount(ordinary))?;
        result.cache_read = result.cache_read.add(cached)?;
        result.cache_write = result.cache_write.add(write)?;
        result.output = result.output.add(output)?;
        result.thinking = result.thinking.add(reasoning)?;
        result.tool_calls = result
            .tool_calls
            .checked_add(request.tool_calls)
            .ok_or_else(|| UsageError::new(ErrorKind::Overflow, "tool count exceeds u64"))?;
        result.text_only_turns += u64::from(request.tool_calls == 0);
    }
    result.cost = (result.input.0 as f64 * rates.input
        + result.cache_read.0 as f64 * rates.input * rates.cached
        + result.cache_write.0 as f64 * rates.input * rates.write
        + result.output.0 as f64 * rates.output)
        / 1_000_000.0;
    if !result.cost.is_finite() {
        return Err(UsageError::new(ErrorKind::Overflow, "cost must be finite"));
    }
    Ok(result)
}
