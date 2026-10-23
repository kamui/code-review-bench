//! `usage codex`: sum and price the billed requests of Codex CLI rollout logs.
//!
//! `codex review` and `codex exec` write one rollout JSONL per thread under
//! `$CODEX_HOME/sessions/YYYY/MM/DD/rollout-<stamp>-<thread id>.jsonl`. A review runs in a child thread whose
//! `session_meta` names its `parent_thread_id`, so `--sessions-dir` with `--session` gathers the root thread and every
//! descendant. Each billed API request is one `token_usage_record`; repeated records of one response count once, the
//! last one winning. OpenAI's `input_tokens` includes cached and cache-write tokens, which are billed instead of the
//! ordinary input rate, not on top of it.

use super::{Count, Outcome, checked_sum, count, int, thousands};
use crate::pyjson::{self, Duplicates, Object, Value};
use crate::pytext::{self, PyPath};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::io::BufRead;

const PROGRAM: &str = "bench-tools usage";

/// The response-item payload types that are tool calls.
pub const TOOL_TYPES: [&str; 3] = ["function_call", "custom_tool_call", "local_shell_call"];

#[derive(Clone, Copy, Debug)]
pub struct LongContext {
    /// A request whose `input_tokens` exceeds this is priced at the long-context rates.
    pub above: u64,
    pub input_mult: f64,
    pub output_mult: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct Prices {
    pub input: f64,
    pub output: f64,
    pub cached_mult: f64,
    pub write_mult: f64,
    pub long_context: Option<LongContext>,
}

pub enum Source {
    Paths(Vec<Vec<u8>>),
    Session { sessions_dir: Vec<u8>, root: String },
}

pub enum Output {
    Text,
    Json,
    Row(String),
}

pub struct Options {
    pub source: Source,
    pub prices: Prices,
    pub report: Option<Vec<u8>>,
    pub output: Output,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum ResponseId {
    Native(String),
    Ordinal(String),
}

struct Usage {
    input: Count,
    cached: Count,
    write: Count,
    output: Count,
    reasoning: Count,
}

struct Request {
    usage: Usage,
    tool_calls: u64,
}

struct Rollout {
    path: PyPath,
    requests: Vec<Request>,
    models: BTreeSet<String>,
    first: Option<i64>,
    last: Option<i64>,
}

#[derive(Default)]
struct Summary {
    turns: u64,
    tool_calls: u64,
    text_only_turns: u64,
    input: u64,
    cache_write: u64,
    cache_read: u64,
    output: u64,
    thinking: u64,
    models: BTreeSet<String>,
    wall_seconds: Option<u64>,
    cost: f64,
}

pub fn run(options: &Options) -> Outcome {
    match meter(options) {
        Ok(outcome) => outcome,
        Err(message) => Outcome::failure(2, &format!("{PROGRAM}: {message}")),
    }
}

fn meter(options: &Options) -> Result<Outcome, String> {
    let paths = match &options.source {
        Source::Paths(paths) => paths.iter().map(|raw| PyPath::new(raw)).collect(),
        Source::Session { sessions_dir, root } => gather(&PyPath::new(sessions_dir), root)?,
    };
    let rollouts = paths
        .into_iter()
        .map(read_rollout)
        .collect::<Result<Vec<_>, _>>()?;
    let report_bytes = match &options.report {
        Some(raw) => Some(
            std::fs::metadata(PyPath::new(raw).to_path_buf().as_path())
                .map(|metadata| metadata.len())
                .map_err(|error| pytext::os_error(&error, raw))?,
        ),
        None => None,
    };
    let mut total = Summary {
        wall_seconds: Some(0),
        ..Summary::default()
    };
    let mut threads = Vec::new();
    for rollout in &rollouts {
        let part = summarize(rollout, &options.prices, &mut total.cost)?;
        add(&mut total, &part)?;
        threads.push((rollout, part));
    }
    if total.turns == 0 {
        return Ok(Outcome {
            code: 1,
            stdout: b"no billed request found in the given rollouts\n".to_vec(),
            stderr: Vec::new(),
        });
    }
    let stdout = match &options.output {
        Output::Row(label) => format!(
            "{}\n",
            row(label, &total, report_bytes, options.prices.output)
        )
        .into_bytes(),
        Output::Json => {
            let mut data = Object::default();
            data.insert(
                "threads".into(),
                Value::Array(
                    threads
                        .iter()
                        .map(|(rollout, part)| {
                            let mut entry = Object::default();
                            entry.insert("path".into(), Value::os_text(rollout.path.as_bytes()));
                            thread_fields(&mut entry, part);
                            Value::Object(entry)
                        })
                        .collect(),
                ),
            );
            data.insert("total".into(), Value::Object(total_fields(&total)));
            data.insert(
                "prices".into(),
                Value::Object(prices_fields(&options.prices)),
            );
            data.insert("report_bytes".into(), report_bytes.map_or(Value::Null, int));
            let text = pyjson::dumps(
                &Value::Object(data),
                &pyjson::Dumps {
                    indent: Some(2),
                    ..pyjson::Dumps::default()
                },
            );
            format!("{text}\n").into_bytes()
        }
        Output::Text => {
            let mut out = Vec::new();
            for (rollout, part) in &threads {
                out.extend_from_slice(rollout.path.as_bytes());
                out.extend_from_slice(text_line(part).as_bytes());
            }
            out.extend_from_slice(b"TOTAL");
            out.extend_from_slice(text_line(&total).as_bytes());
            out
        }
    };
    Ok(Outcome::success(stdout))
}

fn thread_fields(entry: &mut Object, part: &Summary) {
    for (name, value) in counts(part) {
        entry.insert(name.into(), int(value));
    }
    entry.insert("models".into(), models(part));
    entry.insert(
        "wall_seconds".into(),
        part.wall_seconds.map_or(Value::Null, int),
    );
    entry.insert("cost".into(), Value::Float(part.cost));
}

fn total_fields(total: &Summary) -> Object {
    let mut entry: Object = counts(total)
        .into_iter()
        .map(|(name, value)| (name.to_string(), int(value)))
        .collect();
    entry.insert("cost".into(), Value::Float(total.cost));
    entry.insert("models".into(), models(total));
    entry.insert("wall_seconds".into(), int(total.wall_seconds.unwrap_or(0)));
    entry
}

fn counts(part: &Summary) -> [(&'static str, u64); 8] {
    [
        ("turns", part.turns),
        ("tool_calls", part.tool_calls),
        ("text_only_turns", part.text_only_turns),
        ("input", part.input),
        ("cache_write", part.cache_write),
        ("cache_read", part.cache_read),
        ("output", part.output),
        ("thinking", part.thinking),
    ]
}

fn models(part: &Summary) -> Value {
    Value::Array(
        part.models
            .iter()
            .map(|model| Value::Str(model.clone()))
            .collect(),
    )
}

fn prices_fields(prices: &Prices) -> Object {
    let mut entry = Object::default();
    entry.insert("input".into(), Value::Float(prices.input));
    entry.insert("output".into(), Value::Float(prices.output));
    entry.insert("cached_mult".into(), Value::Float(prices.cached_mult));
    entry.insert("cache_write_mult".into(), Value::Float(prices.write_mult));
    if let Some(long) = prices.long_context {
        let mut rule = Object::default();
        rule.insert("above_input_tokens".into(), int(long.above));
        rule.insert("input_mult".into(), Value::Float(long.input_mult));
        rule.insert("output_mult".into(), Value::Float(long.output_mult));
        entry.insert("long_context".into(), Value::Object(rule));
    }
    entry
}

fn text_line(part: &Summary) -> String {
    format!(
        ": turns={} tool_calls={} input={} cache_read={} cache_write={} output={} thinking={} wall={} cost=${:.4} models={}\n",
        part.turns,
        part.tool_calls,
        part.input,
        part.cache_read,
        part.cache_write,
        part.output,
        part.thinking,
        hms(part.wall_seconds),
        part.cost,
        part.models.iter().cloned().collect::<Vec<_>>().join(","),
    )
}

fn hms(seconds: Option<u64>) -> String {
    seconds.map_or_else(
        || "—".to_string(),
        |seconds| {
            format!(
                "{}:{:02}:{:02}",
                seconds / 3600,
                seconds % 3600 / 60,
                seconds % 60
            )
        },
    )
}

fn row(label: &str, total: &Summary, report_bytes: Option<u64>, out_price: f64) -> String {
    let model = if total.models.is_empty() {
        "—".to_string()
    } else {
        total.models.iter().cloned().collect::<Vec<_>>().join(", ")
    };
    let estimate = report_bytes.map(|bytes| (bytes / 4).min(total.output));
    let shaped = estimate.map(|estimate| total.cost - estimate as f64 * out_price / 1_000_000.0);
    super::table_row([
        label.to_string(),
        model,
        total.turns.to_string(),
        total.tool_calls.to_string(),
        total.text_only_turns.to_string(),
        thousands(total.input),
        thousands(total.cache_write),
        thousands(total.cache_read),
        thousands(total.output),
        thousands(total.thinking),
        hms(total.wall_seconds),
        format!("{:.2}", total.cost),
        estimate.map_or_else(|| "—".to_string(), thousands),
        shaped.map_or_else(|| "—".to_string(), |shaped| format!("**{shaped:.2}**")),
    ])
}

/// The root thread's rollout and every descendant's, walking children depth-first.
fn gather(sessions_dir: &PyPath, root: &str) -> Result<Vec<PyPath>, String> {
    let mut found = Vec::new();
    walk(&sessions_dir.to_path_buf(), &mut found);
    found.sort_by_cached_key(|path: &Vec<u8>| pytext::path_order(path));
    let mut threads: Vec<(String, PyPath, Option<String>)> = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();
    for raw in found {
        let path = sessions_dir.join(&raw);
        let Some((thread, parent)) = thread_identity(&path) else {
            continue;
        };
        match index.get(&thread) {
            Some(&at) => threads[at] = (thread, path, parent),
            None => {
                index.insert(thread.clone(), threads.len());
                threads.push((thread, path, parent));
            }
        }
    }
    let Some(&root_at) = index.get(root) else {
        return Err(format!(
            "root thread {root} not found under {}",
            sessions_dir.display()
        ));
    };
    let mut selected = Vec::new();
    let mut visited = HashSet::new();
    let mut frontier = vec![root_at];
    while let Some(current) = frontier.pop() {
        if !visited.insert(current) {
            continue;
        }
        selected.push(threads[current].1.clone());
        let name = &threads[current].0;
        frontier.extend(
            threads
                .iter()
                .enumerate()
                .filter(|(_, (_, _, parent))| parent.as_ref() == Some(name))
                .map(|(at, _)| at),
        );
    }
    Ok(selected)
}

/// Paths below `dir`, relative to it, of regular files named `rollout-*.jsonl`.
fn walk(dir: &std::path::Path, found: &mut Vec<Vec<u8>>) {
    use std::os::unix::ffi::OsStrExt;
    let mut stack = vec![(dir.to_path_buf(), Vec::new())];
    while let Some((at, prefix)) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&at) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let mut relative = prefix.clone();
            if !relative.is_empty() {
                relative.push(b'/');
            }
            relative.extend_from_slice(name.as_bytes());
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_dir() {
                stack.push((entry.path(), relative));
            } else if entry.path().is_file()
                && name.as_bytes().starts_with(b"rollout-")
                && name.as_bytes().ends_with(b".jsonl")
            {
                found.push(relative);
            }
        }
    }
}

/// The thread id and parent thread id the first line's `session_meta` names, or `None` to skip the file.
fn thread_identity(path: &PyPath) -> Option<(String, Option<String>)> {
    let file = std::fs::File::open(path.to_path_buf()).ok()?;
    let mut head = Vec::new();
    std::io::BufReader::new(file)
        .read_until(b'\n', &mut head)
        .ok()?;
    let head = match head.iter().position(|byte| *byte == b'\r') {
        Some(end) => &head[..end],
        None => &head[..],
    };
    let record = pyjson::loads(std::str::from_utf8(head).ok()?, Duplicates::KeepLast).ok()?;
    let payload = match record.as_object()?.get("payload") {
        Some(value) if value.truthy() => value.as_object()?,
        _ => return None,
    };
    let text = |name: &str| {
        payload
            .get(name)
            .and_then(Value::as_str)
            .filter(|text| !text.is_empty())
            .map(str::to_string)
    };
    let thread = text("id").or_else(|| text("session_id"))?;
    Some((thread, text("parent_thread_id")))
}

fn read_rollout(path: PyPath) -> Result<Rollout, String> {
    let text = pytext::read_text(&path).map_err(|error| match error {
        pytext::ReadError::Os(message) => message,
        pytext::ReadError::Decode(message) => format!("{}: {message}", path.display()),
    })?;
    let mut rollout = Rollout {
        path,
        requests: Vec::new(),
        models: BTreeSet::new(),
        first: None,
        last: None,
    };
    let shown = rollout.path.display();
    let mut identities: HashMap<ResponseId, usize> = HashMap::new();
    let mut pending_tools = 0_u64;
    for (index, line) in text.split_inclusive('\n').enumerate() {
        if pytext::strip(line).is_empty() {
            continue;
        }
        let location = format!("{shown}:{}", index + 1);
        let record = match pyjson::loads(line, Duplicates::KeepLast) {
            Ok(Value::Object(record)) => record,
            Ok(_) => return Err(format!("{location}: rollout line is not an object")),
            Err(pyjson::ParseError::Syntax { message, .. }) => {
                return Err(format!("{location}: malformed JSON ({message})"));
            }
            Err(error) => return Err(format!("{location}: malformed JSON ({error})")),
        };
        let kind = record.get("type").and_then(Value::as_str);
        if !matches!(
            kind,
            Some("turn_context" | "response_item" | "token_usage_record")
        ) {
            continue;
        }
        let empty = Object::default();
        let payload = match record.get("payload") {
            Some(value) if value.truthy() => value
                .as_object()
                .ok_or_else(|| format!("{location}: payload is not an object"))?,
            _ => &empty,
        };
        match kind {
            Some("turn_context") => match payload.get("model") {
                Some(Value::Str(model)) if !model.is_empty() => {
                    rollout.models.insert(model.clone());
                }
                None | Some(Value::Null | Value::Str(_)) => {}
                Some(_) => return Err(format!("{location}: model must be a string")),
            },
            Some("response_item") => {
                if payload
                    .get("type")
                    .and_then(Value::as_str)
                    .is_some_and(|kind| TOOL_TYPES.contains(&kind))
                {
                    pending_tools = checked_sum(pending_tools, 1)?;
                }
            }
            _ => {
                let used = payload
                    .get("usage")
                    .and_then(Value::as_object)
                    .ok_or_else(|| {
                        format!("{location}: token_usage_record without usage object")
                    })?;
                let usage = Usage {
                    input: count(used.get("input_tokens")),
                    cached: count(used.get("cached_input_tokens")),
                    write: count(used.get("cache_write_input_tokens")),
                    output: count(used.get("output_tokens")),
                    reasoning: count(used.get("reasoning_output_tokens")),
                };
                let identity = response_id(payload, record.get("ordinal"), &location)?;
                match identities.get(&identity) {
                    Some(&at) => rollout.requests[at].usage = usage,
                    None => {
                        identities.insert(identity, rollout.requests.len());
                        rollout.requests.push(Request {
                            usage,
                            tool_calls: pending_tools,
                        });
                        pending_tools = 0;
                    }
                }
                if let Some(time) = record
                    .get("timestamp")
                    .and_then(Value::as_str)
                    .and_then(super::timestamp_micros)
                {
                    rollout.first = Some(rollout.first.map_or(time, |old| old.min(time)));
                    rollout.last = Some(rollout.last.map_or(time, |old| old.max(time)));
                }
            }
        }
    }
    Ok(rollout)
}

fn response_id(
    payload: &Object,
    ordinal: Option<&Value>,
    location: &str,
) -> Result<ResponseId, String> {
    match payload.get("response_id") {
        Some(Value::Str(id)) if !id.is_empty() => return Ok(ResponseId::Native(id.clone())),
        None | Some(Value::Null | Value::Str(_)) => {}
        Some(_) => return Err(format!("{location}: response_id must be a string")),
    }
    match ordinal {
        None | Some(Value::Null) => Ok(ResponseId::Ordinal("None".into())),
        Some(Value::Str(text)) => Ok(ResponseId::Ordinal(text.clone())),
        Some(Value::Int(digits)) => Ok(ResponseId::Ordinal(digits.clone())),
        Some(_) => Err(format!("{location}: ordinal must be a string or integer")),
    }
}

/// One thread's sums. In long-context pricing each request is priced on its own and also added to `running_total`,
/// so the total sums request costs in file order; otherwise the thread's sums are priced once.
fn summarize(
    rollout: &Rollout,
    prices: &Prices,
    running_total: &mut f64,
) -> Result<Summary, String> {
    let shown = rollout.path.display();
    let mut part = Summary {
        turns: rollout.requests.len() as u64,
        models: rollout.models.clone(),
        wall_seconds: match (rollout.first, rollout.last) {
            (Some(first), Some(last)) => {
                Some(u64::try_from((last - first) / 1_000_000).unwrap_or(0))
            }
            _ => None,
        },
        ..Summary::default()
    };
    let mut request_costs = Vec::new();
    for (index, request) in rollout.requests.iter().enumerate() {
        let location = format!("{shown}: request {}", index + 1);
        let used = &request.usage;
        let input = require(&used.input, "input_tokens", &location)?;
        let cached = require(&used.cached, "cached_input_tokens", &location)?;
        let write = require(&used.write, "cache_write_input_tokens", &location)?;
        let output = require(&used.output, "output_tokens", &location)?;
        let reasoning = require(&used.reasoning, "reasoning_output_tokens", &location)?;
        let ordinary = checked_sum(cached, write)
            .ok()
            .and_then(|subsets| input.checked_sub(subsets))
            .ok_or_else(|| {
                format!("{location} has cached+cache_write ({cached}+{write}) above input_tokens ({input})")
            })?;
        part.input = checked_sum(part.input, ordinary)?;
        part.cache_read = checked_sum(part.cache_read, cached)?;
        part.cache_write = checked_sum(part.cache_write, write)?;
        part.output = checked_sum(part.output, output)?;
        part.thinking = checked_sum(part.thinking, reasoning)?;
        part.tool_calls = checked_sum(part.tool_calls, request.tool_calls)?;
        part.text_only_turns += u64::from(request.tool_calls == 0);
        if let Some(long) = prices.long_context {
            let (input_mult, output_mult) = if input > long.above {
                (long.input_mult, long.output_mult)
            } else {
                (1.0, 1.0)
            };
            request_costs.push(price(
                [ordinary, cached, write, output],
                prices.input * input_mult,
                prices.output * output_mult,
                prices,
            ));
        }
    }
    if prices.long_context.is_some() {
        for cost in request_costs {
            part.cost += cost;
            *running_total += cost;
        }
    } else {
        part.cost = price(
            [part.input, part.cache_read, part.cache_write, part.output],
            prices.input,
            prices.output,
            prices,
        );
        *running_total += part.cost;
    }
    if !part.cost.is_finite() {
        return Err(format!("{shown}: cost must be finite"));
    }
    Ok(part)
}

fn price(
    [ordinary, cached, write, output]: [u64; 4],
    input_price: f64,
    output_price: f64,
    prices: &Prices,
) -> f64 {
    (ordinary as f64 * input_price
        + cached as f64 * input_price * prices.cached_mult
        + write as f64 * input_price * prices.write_mult
        + output as f64 * output_price)
        / 1_000_000.0
}

fn require(value: &Count, field: &str, location: &str) -> Result<u64, String> {
    match value {
        Count::Known(count) => Ok(*count),
        Count::Absent => Err(format!("{location} missing {field}")),
        Count::Invalid => Err(format!("{location} has invalid {field}")),
    }
}

/// Folds one thread into the total; the total's cost is summed separately by [`summarize`].
fn add(total: &mut Summary, part: &Summary) -> Result<(), String> {
    total.turns = checked_sum(total.turns, part.turns)?;
    total.tool_calls = checked_sum(total.tool_calls, part.tool_calls)?;
    total.text_only_turns = checked_sum(total.text_only_turns, part.text_only_turns)?;
    total.input = checked_sum(total.input, part.input)?;
    total.cache_write = checked_sum(total.cache_write, part.cache_write)?;
    total.cache_read = checked_sum(total.cache_read, part.cache_read)?;
    total.output = checked_sum(total.output, part.output)?;
    total.thinking = checked_sum(total.thinking, part.thinking)?;
    total.models.extend(part.models.iter().cloned());
    total.wall_seconds = Some(checked_sum(
        total.wall_seconds.unwrap_or(0),
        part.wall_seconds.unwrap_or(0),
    )?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn row_shapes_the_cost_by_the_report() {
        let total = Summary {
            turns: 2,
            output: 1_000,
            cost: 2.0,
            wall_seconds: Some(3_725),
            ..Summary::default()
        };
        assert_eq!(
            row("run", &total, Some(400), 10_000.0),
            "| run | — | 2 | 0 | 0 | 0 | 0 | 0 | 1,000 | 0 | 1:02:05 | 2.00 | 100 | **1.00** |"
        );
    }
}
