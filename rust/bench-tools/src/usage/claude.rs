//! `usage claude`: sum and price the billed requests of Claude Code transcripts, and check the sums against the
//! session totals the CLI records.
//!
//! The harness writes one line per content block of a response, and every line repeats the response's `usage`, the
//! output count growing to its final value on the last line. Lines sharing a `requestId` are one request: each count
//! is the largest any line records, and tool calls are the distinct `tool_use` blocks. A line without a `requestId` is
//! its own request. Lines the harness wrote itself, with `isApiErrorMessage` or the `<synthetic>` model, bill nothing.
//!
//! `cache_creation_input_tokens` is the authoritative cache-write total. The `cache_creation` split prices it per
//! tier; a missing split leaves the whole total of unknown tier, a short split leaves the shortfall, and a split above
//! the total leaves the whole total. `cost` prices unknown-tier writes at the five-minute multiplier, and
//! `cost_bounds` at the lower and higher of the two.
//!
//! A request is complete once a line records its `stop_reason`. A subagent's transcript can stop before its final
//! request's last line; its parent's completion receipt then holds that request's usage. The receipt replaces the
//! partial output count only when it is the agent's sole completed receipt for one request, its input counts and text
//! match the final request, and the transcript records nothing after that request. The repaired sums are then checked
//! against the last `cost-state` record of each session, per model and token category.

use super::{
    Count, Outcome, checked_sum, count, general, int, round_to, thousands, timestamp_micros,
    utc_isoformat,
};
use crate::pyjson::{self, Duplicates, Object, Value};
use crate::pytext;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt::Write as _;
use std::os::unix::ffi::OsStrExt;

const PROGRAM: &str = "bench-tools usage";
const WIDTH: usize = 18;
const NO_PATH: &str = "(no path)";
const TIMING_EVENTS: [&str; 3] = ["root_dispatched_at", "payload_validated_at", "completed_at"];
const FALLBACK: &str = "unknown-tier cache writes are priced at cache_write_mult (5m) in cost; cost_bounds price them \
                        at the lower and the higher of the two multipliers";
/// The counts a request must record, as `usage` names them, and the session-total key and category of each.
const REQUIRED: [(&str, &str, &str); 4] = [
    ("input_tokens", "inputTokens", "input"),
    (
        "cache_creation_input_tokens",
        "cacheCreationInputTokens",
        "cache_write",
    ),
    (
        "cache_read_input_tokens",
        "cacheReadInputTokens",
        "cache_read",
    ),
    ("output_tokens", "outputTokens", "output"),
];
const CAUSES: [(&str, &str); 3] = [
    ("no_breakdown", "without a tier breakdown"),
    ("residual", "whose breakdown is short of the total"),
    (
        "inconsistent",
        "whose breakdown exceeds the total (inconsistent usage)",
    ),
];

#[derive(Clone, Copy, Debug)]
pub struct Prices {
    pub input: f64,
    pub output: f64,
    pub write_5m_mult: f64,
    pub write_1h_mult: f64,
    pub read_mult: f64,
}

pub enum Output {
    Text,
    Json,
    Row(String),
}

pub struct Options {
    pub transcripts: Vec<Vec<u8>>,
    /// Transcripts that are not metered but may hold completion receipts or session totals, such as a parent's.
    pub receipts: Vec<Vec<u8>>,
    pub prices: Prices,
    pub report: Option<Vec<u8>>,
    pub timing: Option<Vec<u8>>,
    pub by_kind: bool,
    pub output: Output,
}

pub fn run(options: &Options) -> Outcome {
    match meter(options) {
        Ok(outcome) => outcome,
        Err(message) => Outcome::failure(2, &format!("{PROGRAM}: {message}")),
    }
}

fn meter(options: &Options) -> Result<Outcome, String> {
    let timing = match &options.timing {
        Some(path) => read_timing(path).map_err(|message| {
            format!(
                "cannot read timing {}: {message}",
                pytext::stderr_text(path)
            )
        })?,
        None => Timing::default(),
    };
    let mut transcripts = options
        .transcripts
        .iter()
        .map(|path| read_transcript(path))
        .collect::<Result<Vec<_>, _>>()?;
    let report_tokens = match &options.report {
        Some(path) => Some(report_tokens(path)?),
        None => None,
    };
    let mut records = Records::default();
    let mut scanned = HashSet::new();
    for transcript in &transcripts {
        if scanned.insert(identity(&transcript.path)) {
            records.scan(&pytext::stderr_text(&transcript.path), &transcript.lines);
        }
    }
    for path in &options.receipts {
        if scanned.insert(identity(path)) {
            records.scan(&pytext::stderr_text(path), &parse_lines(path)?);
        }
    }
    let completeness = reconcile(&mut transcripts, &records)?;
    let usages = transcripts
        .iter()
        .map(Usage::of)
        .collect::<Result<Vec<_>, _>>()?;
    let mut total = Usage::named("TOTAL");
    for usage in &usages {
        total.absorb(usage)?;
    }
    let prices = &options.prices;
    let stdout = match &options.output {
        Output::Row(label) => format!("{}\n", row_line(&total, label, prices, report_tokens)),
        Output::Json => {
            let mut data = Object::default();
            data.insert("prices".into(), Value::Object(prices_fields(prices)));
            let entries = usages
                .iter()
                .zip(&transcripts)
                .map(|(usage, transcript)| {
                    let mut entry = usage.fields();
                    entry.insert("path".into(), Value::os_text(&transcript.path));
                    add_costs(&mut entry, usage, prices);
                    if options.by_kind {
                        entry.insert("by_kind".into(), usage.by_kind());
                    }
                    Value::Object(entry)
                })
                .collect();
            data.insert("transcripts".into(), Value::Array(entries));
            data.insert("timing".into(), Value::Object(timing.fields()));
            let mut entry = total.fields();
            entry.insert(
                "agent_span_sum_seconds".into(),
                Value::Float(round_to(total.wall_seconds, 3)),
            );
            add_costs(&mut entry, &total, prices);
            if options.by_kind {
                entry.insert("by_kind".into(), total.by_kind());
            }
            if let Some(tokens) = report_tokens {
                let shaped_output = total.output.saturating_sub(tokens);
                let shaped = total.cost_summary(prices, shaped_output);
                entry.insert("report_tokens".into(), int(tokens));
                entry.insert("production_shaped_output".into(), int(shaped_output));
                entry.insert(
                    "production_shaped_cost".into(),
                    Value::Float(round_to(shaped.estimate, 6)),
                );
                let bounds = [
                    ("low".to_string(), Value::Float(round_to(shaped.low, 6))),
                    ("high".to_string(), Value::Float(round_to(shaped.high, 6))),
                ];
                entry.insert(
                    "production_shaped_cost_bounds".into(),
                    Value::Object(bounds.into_iter().collect()),
                );
            }
            data.insert("total".into(), Value::Object(entry));
            data.insert("completeness".into(), Value::Object(completeness.fields()));
            let options = pyjson::Dumps {
                indent: Some(2),
                ..pyjson::Dumps::default()
            };
            format!("{}\n", pyjson::dumps(&Value::Object(data), &options))
        }
        Output::Text => {
            let mut lines = Vec::new();
            for (index, usage) in usages.iter().enumerate() {
                if index > 0 {
                    lines.push(String::new());
                }
                lines.extend(block_lines(usage, prices, None, options.by_kind));
            }
            lines.push(String::new());
            lines.extend(block_lines(&total, prices, report_tokens, options.by_kind));
            lines.push(String::new());
            lines.extend(timing.lines());
            lines.iter().fold(String::new(), |mut out, line| {
                let _ = writeln!(out, "{line}");
                out
            })
        }
    };
    Ok(Outcome::success(stdout.into_bytes()))
}

/// One request's counts as its lines record them: the largest of each, or absent or invalid.
#[derive(Default)]
struct Pending {
    id: Option<String>,
    lines: u64,
    counts: [Count; 4],
    five: Count,
    hour: Count,
    breakdown: bool,
    thinking: Count,
    tool_ids: HashSet<String>,
    unnamed_tools: u64,
    model: Option<String>,
    finalized: bool,
    last_line: usize,
    texts: Vec<String>,
}

struct Request {
    id: Option<String>,
    lines: u64,
    input: u64,
    cache_write: u64,
    cache_read: u64,
    output: u64,
    five: u64,
    hour: u64,
    breakdown: bool,
    thinking: u64,
    tool_calls: u64,
    model: Option<String>,
    finalized: bool,
    last_line: usize,
    texts: Vec<String>,
}

#[derive(Clone, Default)]
struct Part {
    count: u64,
    chars: u64,
    tokens: i64,
}

struct Transcript {
    path: Vec<u8>,
    lines: Vec<(usize, Object)>,
    requests: Vec<Request>,
    models: Vec<String>,
    first: Option<i64>,
    last: Option<i64>,
    kind_chars: BTreeMap<String, u64>,
    write_chars: BTreeMap<String, Part>,
}

/// The JSON objects of a transcript's lines with their line numbers; a line that is not JSON makes the file unusable.
fn parse_lines(path: &[u8]) -> Result<Vec<(usize, Object)>, String> {
    let shown = pytext::stderr_text(path);
    let bytes = std::fs::read(std::ffi::OsStr::from_bytes(path))
        .map_err(|error| format!("cannot read {shown}: {}", pytext::strerror(&error)))?;
    let text = pytext::decode_utf8(&bytes)
        .map(|text| pytext::universal_newlines(&text))
        .map_err(|message| format!("cannot read {shown}: {message}"))?;
    let mut lines = Vec::new();
    for (index, line) in text.split_inclusive('\n').enumerate() {
        if pytext::strip(line).is_empty() {
            continue;
        }
        match pyjson::loads(line, Duplicates::KeepLast) {
            Ok(Value::Object(record)) => lines.push((index + 1, record)),
            Ok(_) => {}
            Err(_) => return Err(format!("{shown}:{}: malformed JSON", index + 1)),
        }
    }
    Ok(lines)
}

/// The file a path names, so one file read under two paths is scanned once.
fn identity(path: &[u8]) -> Vec<u8> {
    std::fs::canonicalize(std::ffi::OsStr::from_bytes(path)).map_or_else(
        |_| path.to_vec(),
        |real| real.as_os_str().as_bytes().to_vec(),
    )
}

fn widest(old: Count, new: Count) -> Count {
    match (old, new) {
        (Count::Invalid, _) | (_, Count::Invalid) => Count::Invalid,
        (Count::Known(a), Count::Known(b)) => Count::Known(a.max(b)),
        (Count::Known(a), Count::Absent) | (Count::Absent, Count::Known(a)) => Count::Known(a),
        (Count::Absent, Count::Absent) => Count::Absent,
    }
}

fn read_transcript(path: &[u8]) -> Result<Transcript, String> {
    let shown = pytext::stderr_text(path);
    let lines = parse_lines(path)?;
    let mut pending: Vec<Pending> = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();
    let mut transcript = Transcript {
        path: path.to_vec(),
        lines: Vec::new(),
        requests: Vec::new(),
        models: Vec::new(),
        first: None,
        last: None,
        kind_chars: BTreeMap::new(),
        write_chars: BTreeMap::new(),
    };
    let mut seen_blocks: HashSet<(usize, String)> = HashSet::new();
    for (number, record) in &lines {
        if record.get("type").and_then(Value::as_str) != Some("assistant") {
            continue;
        }
        let Some(message) = record.get("message").and_then(Value::as_object) else {
            continue;
        };
        let Some(used) = message.get("usage").and_then(Value::as_object) else {
            continue;
        };
        let model = message.get("model").and_then(Value::as_str);
        if record.get("isApiErrorMessage").is_some_and(Value::truthy)
            || model == Some("<synthetic>")
        {
            continue;
        }
        let at = match record
            .get("requestId")
            .and_then(Value::as_str)
            .filter(|id| !id.is_empty())
        {
            Some(id) => *index.entry(id.to_string()).or_insert_with(|| {
                pending.push(Pending {
                    id: Some(id.to_string()),
                    ..Pending::default()
                });
                pending.len() - 1
            }),
            None => {
                pending.push(Pending::default());
                pending.len() - 1
            }
        };
        let request = &mut pending[at];
        request.lines += 1;
        request.last_line = *number;
        for (slot, (field, _, _)) in request.counts.iter_mut().zip(REQUIRED) {
            *slot = widest(*slot, count(used.get(field)));
        }
        if let Some(tiers) = used.get("cache_creation").and_then(Value::as_object) {
            let five = tiers.get("ephemeral_5m_input_tokens");
            let hour = tiers.get("ephemeral_1h_input_tokens");
            let present = |value: Option<&Value>| !matches!(value, None | Some(Value::Null));
            if present(five) || present(hour) {
                request.breakdown = true;
                request.five = widest(request.five, count(five));
                request.hour = widest(request.hour, count(hour));
            }
        }
        if let Some(details) = used.get("output_tokens_details").and_then(Value::as_object) {
            request.thinking = widest(request.thinking, count(details.get("thinking_tokens")));
        }
        if !matches!(message.get("stop_reason"), None | Some(Value::Null)) {
            request.finalized = true;
        }
        if let Some(model) = model.filter(|model| !model.is_empty()) {
            request.model.get_or_insert_with(|| model.to_string());
            if !transcript.models.iter().any(|known| known == model) {
                transcript.models.push(model.to_string());
            }
        }
        for item in message
            .get("content")
            .and_then(Value::as_array)
            .unwrap_or_default()
        {
            let Some(item) = item.as_object() else {
                continue;
            };
            let kind = item.get("type").and_then(Value::as_str);
            if kind == Some("tool_use") {
                match item
                    .get("id")
                    .and_then(Value::as_str)
                    .filter(|id| !id.is_empty())
                {
                    Some(id) => {
                        request.tool_ids.insert(id.to_string());
                    }
                    None => request.unnamed_tools += 1,
                }
            }
            if kind == Some("text")
                && let Some(text) = item.get("text").and_then(Value::as_str)
                && !request.texts.iter().any(|known| known == text)
            {
                request.texts.push(text.to_string());
            }
            if seen_blocks.insert((at, block_identity(item))) {
                add_block_chars(
                    item,
                    &mut transcript.kind_chars,
                    &mut transcript.write_chars,
                );
            }
        }
        if let Some(time) = record
            .get("timestamp")
            .and_then(Value::as_str)
            .and_then(timestamp_micros)
        {
            transcript.first = Some(transcript.first.map_or(time, |old| old.min(time)));
            transcript.last = Some(transcript.last.map_or(time, |old| old.max(time)));
        }
    }
    if pending.is_empty() {
        return Err(format!("no billed assistant turns in {shown}"));
    }
    for (number, request) in pending.into_iter().enumerate() {
        let location = format!("{shown}: request {}", number + 1);
        let [input, cache_write, cache_read, output] = request.counts;
        let required = |value: Count, field: &str| match value {
            Count::Known(value) => Ok(value),
            Count::Absent => Err(format!("{location} missing {field}")),
            Count::Invalid => Err(format!("{location} has invalid {field}")),
        };
        let optional = |value: Count, field: &str| match value {
            Count::Known(value) => Ok(value),
            Count::Absent => Ok(0),
            Count::Invalid => Err(format!("{location} has invalid {field}")),
        };
        transcript.requests.push(Request {
            input: required(input, REQUIRED[0].0)?,
            cache_write: required(cache_write, REQUIRED[1].0)?,
            cache_read: required(cache_read, REQUIRED[2].0)?,
            output: required(output, REQUIRED[3].0)?,
            five: optional(request.five, "ephemeral_5m_input_tokens")?,
            hour: optional(request.hour, "ephemeral_1h_input_tokens")?,
            thinking: optional(request.thinking, "thinking_tokens")?,
            breakdown: request.breakdown,
            tool_calls: checked_sum(request.tool_ids.len() as u64, request.unnamed_tools)?,
            id: request.id,
            lines: request.lines,
            model: request.model,
            finalized: request.finalized,
            last_line: request.last_line,
            texts: request.texts,
        });
    }
    transcript.lines = lines;
    Ok(transcript)
}

/// What makes a content block itself, so a block the harness re-emits on a later line of one request counts once: a
/// tool call's `id`, or else the whole block.
fn block_identity(item: &Object) -> String {
    match item.get("id").and_then(Value::as_str) {
        Some(id)
            if !id.is_empty() && item.get("type").and_then(Value::as_str) == Some("tool_use") =>
        {
            format!("tool_use {id}")
        }
        _ => pyjson::dumps(
            &Value::Object(item.clone()),
            &pyjson::Dumps {
                ensure_ascii: false,
                sort_keys: true,
                ..pyjson::Dumps::default()
            },
        ),
    }
}

/// Adds a block's characters to its kind, and for a file write, again under the path written.
fn add_block_chars(
    item: &Object,
    kinds: &mut BTreeMap<String, u64>,
    writes: &mut BTreeMap<String, Part>,
) {
    match item.get("type").and_then(Value::as_str) {
        Some("text") => {
            if let Some(text) = item.get("text").and_then(Value::as_str) {
                *kinds.entry("text".into()).or_default() += text.chars().count() as u64;
            }
        }
        Some("tool_use") => {
            let name = item
                .get("name")
                .and_then(Value::as_str)
                .filter(|name| !name.is_empty())
                .unwrap_or("unknown");
            let empty = Object::default();
            let data = item
                .get("input")
                .and_then(Value::as_object)
                .unwrap_or(&empty);
            let chars = pyjson::dumps(&Value::Object(data.clone()), &pyjson::Dumps::default())
                .chars()
                .count() as u64;
            *kinds.entry(format!("tool:{name}")).or_default() += chars;
            let path = match name {
                "Write" => Some(
                    data.get("file_path")
                        .and_then(Value::as_str)
                        .filter(|path| !path.is_empty())
                        .unwrap_or(NO_PATH)
                        .to_string(),
                ),
                "Bash" => data
                    .get("command")
                    .and_then(Value::as_str)
                    .filter(|command| command.contains("<<"))
                    .map(|command| redirect_target(command).unwrap_or_else(|| NO_PATH.to_string())),
                _ => None,
            };
            if let Some(path) = path {
                let entry = writes.entry(path).or_default();
                entry.count += 1;
                entry.chars += chars;
            }
        }
        _ => {}
    }
}

/// The first redirect target in a heredoc command: `> path`, `>> path`, `tee path` or `tee -a path`, the path
/// optionally quoted, as `(?:>>?|tee(?: -a)?)\s*['"]?([^\s'"|;&]+)` finds it.
fn redirect_target(command: &str) -> Option<String> {
    let chars: Vec<char> = command.chars().collect();
    let starts = |at: usize, prefix: &str| {
        prefix
            .chars()
            .enumerate()
            .all(|(offset, c)| chars.get(at + offset) == Some(&c))
    };
    let in_path = |c: char| !pytext::is_space(c) && !"'\"|;&".contains(c);
    for start in 0..chars.len() {
        for prefix in [">>", ">", "tee -a", "tee"] {
            if !starts(start, prefix) {
                continue;
            }
            let mut at = start + prefix.chars().count();
            while chars.get(at).is_some_and(|c| pytext::is_space(*c)) {
                at += 1;
            }
            if chars.get(at).is_some_and(|c| *c == '\'' || *c == '"') {
                at += 1;
            }
            let end = (at..chars.len())
                .find(|&index| !in_path(chars[index]))
                .unwrap_or(chars.len());
            if end > at {
                return Some(chars[at..end].iter().collect());
            }
        }
    }
    None
}

/// `round(chars * visible / total_chars)`, rounding half to even.
fn token_share(chars: u64, total_chars: u64, visible: u64) -> i64 {
    if total_chars == 0 || visible == 0 {
        return 0;
    }
    let product = u128::from(chars) * u128::from(visible);
    let (quotient, remainder) = (
        product / u128::from(total_chars),
        product % u128::from(total_chars),
    );
    let twice = remainder * 2;
    let up =
        twice > u128::from(total_chars) || (twice == u128::from(total_chars) && quotient % 2 == 1);
    i64::try_from(quotient + u128::from(up)).unwrap_or(i64::MAX)
}

/// Splits `visible` tokens across the kinds by characters; the largest kind takes the rounding remainder.
fn allocate(kind_chars: &BTreeMap<String, u64>, visible: u64) -> BTreeMap<String, Part> {
    let total: u64 = kind_chars.values().sum();
    let mut parts: BTreeMap<String, Part> = kind_chars
        .iter()
        .map(|(name, chars)| {
            let part = Part {
                count: 0,
                chars: *chars,
                tokens: token_share(*chars, total, visible),
            };
            (name.clone(), part)
        })
        .collect();
    if total == 0 || visible == 0 {
        return parts;
    }
    let shared: i64 = parts.values().map(|part| part.tokens).sum();
    if let Some((largest, _)) = kind_chars.iter().max_by(|a, b| (a.1, a.0).cmp(&(b.1, b.0)))
        && let Some(part) = parts.get_mut(largest)
    {
        part.tokens += i64::try_from(visible).unwrap_or(i64::MAX) - shared;
    }
    parts
}

#[derive(Default)]
struct Receipt {
    location: String,
    status: Option<String>,
    usage: Object,
    text: String,
}

#[derive(Default)]
struct Records {
    receipts: HashMap<String, Vec<Receipt>>,
    /// The last session totals each session records, in first-seen order.
    sessions: Vec<(String, Object)>,
}

impl Records {
    fn scan(&mut self, shown: &str, lines: &[(usize, Object)]) {
        for (number, record) in lines {
            if let Some(result) = record.get("toolUseResult").and_then(Value::as_object)
                && let Some(agent) = result.get("agentId").and_then(Value::as_str)
                && let Some(usage) = result.get("usage").and_then(Value::as_object)
            {
                let text = match result.get("content") {
                    Some(Value::Str(text)) => text.clone(),
                    Some(Value::Array(blocks)) => blocks
                        .iter()
                        .filter_map(Value::as_object)
                        .filter(|block| block.get("type").and_then(Value::as_str) == Some("text"))
                        .filter_map(|block| block.get("text").and_then(Value::as_str))
                        .collect(),
                    _ => String::new(),
                };
                self.receipts
                    .entry(agent.to_string())
                    .or_default()
                    .push(Receipt {
                        location: format!("{shown}:{number}"),
                        status: result
                            .get("status")
                            .and_then(Value::as_str)
                            .map(str::to_string),
                        usage: usage.clone(),
                        text,
                    });
            }
            if record.get("type").and_then(Value::as_str) == Some("cost-state")
                && let Some(session) = record.get("sessionId").and_then(Value::as_str)
                && let Some(models) = record.get("modelUsage").and_then(Value::as_object)
            {
                match self.sessions.iter_mut().find(|(known, _)| known == session) {
                    Some(entry) => entry.1 = models.clone(),
                    None => self.sessions.push((session.to_string(), models.clone())),
                }
            }
        }
    }
}

struct Repair {
    transcript: Vec<u8>,
    agent: String,
    request: Option<String>,
    receipt: String,
    from: u64,
    to: u64,
}

struct Gap {
    model: String,
    category: &'static str,
    session: u64,
    metered: u64,
}

#[derive(Default)]
struct Completeness {
    unfinalized: Vec<(Vec<u8>, Option<String>, usize)>,
    repairs: Vec<Repair>,
    refusals: Vec<(Vec<u8>, String, String)>,
    gaps: Vec<Gap>,
    problems: Vec<String>,
}

/// Repairs each subagent's unfinished final request from its sole matching receipt, then lists what still keeps the
/// sums from matching the session totals.
fn reconcile(transcripts: &mut [Transcript], records: &Records) -> Result<Completeness, String> {
    let mut found = Completeness::default();
    for transcript in transcripts.iter_mut() {
        let Some(agent) = agent_id(&transcript.path) else {
            continue;
        };
        let continues_after = |line: usize| {
            transcript.lines.iter().any(|(number, record)| {
                *number > line
                    && matches!(
                        record.get("type").and_then(Value::as_str),
                        Some("user" | "assistant")
                    )
            })
        };
        let Some(request) = transcript.requests.last() else {
            continue;
        };
        if request.finalized {
            continue;
        }
        let later = continues_after(request.last_line);
        let receipts = records.receipts.get(&agent).map_or(&[][..], Vec::as_slice);
        match repair(request, receipts, later) {
            Ok((receipt, output, thinking)) => {
                found.repairs.push(Repair {
                    transcript: transcript.path.clone(),
                    agent,
                    request: request.id.clone(),
                    receipt: receipt.location.clone(),
                    from: request.output,
                    to: output,
                });
                let request = transcript
                    .requests
                    .last_mut()
                    .expect("the final request was just read");
                request.output = output;
                request.thinking = request.thinking.max(thinking);
                request.finalized = true;
            }
            Err(reason) => found
                .refusals
                .push((transcript.path.clone(), agent, reason)),
        }
    }
    let mut metered: BTreeMap<String, [u64; 4]> = BTreeMap::new();
    for transcript in transcripts.iter() {
        for request in &transcript.requests {
            if !request.finalized {
                found.problems.push(format!(
                    "{}: request {} (line {}) has no final response",
                    pytext::stderr_text(&transcript.path),
                    request.id.as_deref().unwrap_or("without an id"),
                    request.last_line
                ));
                found.unfinalized.push((
                    transcript.path.clone(),
                    request.id.clone(),
                    request.last_line,
                ));
            }
            let sums = metered
                .entry(request.model.clone().unwrap_or_else(|| "unknown".into()))
                .or_default();
            for (sum, value) in sums.iter_mut().zip([
                request.input,
                request.cache_write,
                request.cache_read,
                request.output,
            ]) {
                *sum = checked_sum(*sum, value)?;
            }
        }
    }
    if records.sessions.is_empty() {
        found
            .problems
            .push("no session totals to reconcile against".into());
        return Ok(found);
    }
    let mut session: BTreeMap<String, [u64; 4]> = BTreeMap::new();
    for (id, models) in &records.sessions {
        for (model, totals) in models.iter() {
            let sums = session.entry(model.clone()).or_default();
            for (sum, (_, key, _)) in sums.iter_mut().zip(REQUIRED) {
                let value = match count(totals.as_object().and_then(|totals| totals.get(key))) {
                    Count::Known(value) => value,
                    Count::Absent => 0,
                    Count::Invalid => {
                        found
                            .problems
                            .push(format!("session {id} records an invalid {key} for {model}"));
                        0
                    }
                };
                *sum = checked_sum(*sum, value)?;
            }
        }
    }
    let models: BTreeSet<&String> = metered.keys().chain(session.keys()).collect();
    for model in models {
        let ours = metered.get(model).copied().unwrap_or_default();
        let theirs = session.get(model).copied().unwrap_or_default();
        for ((metered, session), (_, _, category)) in ours.into_iter().zip(theirs).zip(REQUIRED) {
            if metered != session {
                found.problems.push(format!(
                    "{model} {category}: the session records {}, the requests {}",
                    thousands(session),
                    thousands(metered)
                ));
                found.gaps.push(Gap {
                    model: model.clone(),
                    category,
                    session,
                    metered,
                });
            }
        }
    }
    Ok(found)
}

/// The agent id of a subagent transcript named `agent-<id>.jsonl`.
fn agent_id(path: &[u8]) -> Option<String> {
    let name = path.rsplit(|byte| *byte == b'/').next()?;
    let id = name.strip_prefix(b"agent-")?.strip_suffix(b".jsonl")?;
    std::str::from_utf8(id)
        .ok()
        .filter(|id| !id.is_empty())
        .map(str::to_string)
}

/// The receipt that completes `request`, with its output and thinking counts, or why none does.
fn repair<'a>(
    request: &Request,
    receipts: &'a [Receipt],
    later: bool,
) -> Result<(&'a Receipt, u64, u64), String> {
    let [receipt] = receipts else {
        return Err(format!(
            "found {} completion receipts for the agent",
            receipts.len()
        ));
    };
    if receipt.status.as_deref() != Some("completed") {
        return Err(format!(
            "receipt status is {}",
            receipt.status.as_deref().unwrap_or("missing")
        ));
    }
    let usage = &receipt.usage;
    if let Some(iterations) = usage.get("iterations")
        && iterations.as_array().is_none_or(|list| list.len() != 1)
    {
        let covered = iterations.as_array().map_or_else(
            || "an unknown number of".into(),
            |list| list.len().to_string(),
        );
        return Err(format!("receipt covers {covered} requests"));
    }
    let known = |value: Option<&Value>, absent: Option<u64>| match count(value) {
        Count::Known(value) => Some(value),
        Count::Absent => absent,
        Count::Invalid => None,
    };
    let tiers = usage.get("cache_creation").and_then(Value::as_object);
    let tier = |name: &str| known(tiers.and_then(|tiers| tiers.get(name)), Some(0));
    let inputs = [
        known(usage.get("input_tokens"), None),
        known(usage.get("cache_creation_input_tokens"), None),
        known(usage.get("cache_read_input_tokens"), None),
        tier("ephemeral_5m_input_tokens"),
        tier("ephemeral_1h_input_tokens"),
    ];
    let ours = [
        request.input,
        request.cache_write,
        request.cache_read,
        request.five,
        request.hour,
    ];
    if inputs
        .iter()
        .zip(ours)
        .any(|(theirs, ours)| *theirs != Some(ours))
    {
        return Err("receipt input counts differ from the final request".into());
    }
    if receipt.text.is_empty() || receipt.text != request.texts.concat() {
        return Err("receipt text differs from the final request".into());
    }
    if later {
        return Err("the transcript continues after the final request".into());
    }
    let output = known(usage.get("output_tokens"), None).ok_or("receipt output is not a count")?;
    if output < request.output {
        return Err(format!(
            "receipt output {output} is below the recorded {}",
            request.output
        ));
    }
    let thinking = usage
        .get("output_tokens_details")
        .and_then(Value::as_object)
        .and_then(|details| known(details.get("thinking_tokens"), Some(0)))
        .unwrap_or(0);
    Ok((receipt, output, thinking))
}

impl Completeness {
    fn fields(&self) -> Object {
        let path = |path: &Vec<u8>| Value::os_text(path);
        let request = |id: &Option<String>| id.clone().map_or(Value::Null, Value::Str);
        let object = |pairs: Vec<(&str, Value)>| {
            Value::Object(
                pairs
                    .into_iter()
                    .map(|(key, value)| (key.to_string(), value))
                    .collect(),
            )
        };
        let signed = |session: u64, metered: u64| {
            let gap = i128::from(session) - i128::from(metered);
            Value::Int(gap.to_string())
        };
        let mut data = Object::default();
        data.insert("complete".into(), Value::Bool(self.problems.is_empty()));
        data.insert(
            "unfinalized".into(),
            Value::Array(
                self.unfinalized
                    .iter()
                    .map(|(transcript, id, line)| {
                        object(vec![
                            ("transcript", path(transcript)),
                            ("request", request(id)),
                            ("line", int(*line as u64)),
                        ])
                    })
                    .collect(),
            ),
        );
        data.insert(
            "repairs".into(),
            Value::Array(
                self.repairs
                    .iter()
                    .map(|repair| {
                        object(vec![
                            ("transcript", path(&repair.transcript)),
                            ("agent", Value::Str(repair.agent.clone())),
                            ("request", request(&repair.request)),
                            ("receipt", Value::Str(repair.receipt.clone())),
                            ("output_from", int(repair.from)),
                            ("output_to", int(repair.to)),
                        ])
                    })
                    .collect(),
            ),
        );
        data.insert(
            "refusals".into(),
            Value::Array(
                self.refusals
                    .iter()
                    .map(|(transcript, agent, reason)| {
                        object(vec![
                            ("transcript", path(transcript)),
                            ("agent", Value::Str(agent.clone())),
                            ("reason", Value::Str(reason.clone())),
                        ])
                    })
                    .collect(),
            ),
        );
        data.insert(
            "gaps".into(),
            Value::Array(
                self.gaps
                    .iter()
                    .map(|gap| {
                        object(vec![
                            ("model", Value::Str(gap.model.clone())),
                            ("category", Value::Str(gap.category.into())),
                            ("session", int(gap.session)),
                            ("metered", int(gap.metered)),
                            ("gap", signed(gap.session, gap.metered)),
                        ])
                    })
                    .collect(),
            ),
        );
        data.insert(
            "problems".into(),
            Value::Array(self.problems.iter().cloned().map(Value::Str).collect()),
        );
        data
    }
}

#[derive(Clone, Default)]
struct Usage {
    label: String,
    turns: u64,
    lines: u64,
    tool_calls: u64,
    text_only_turns: u64,
    input: u64,
    cache_write: u64,
    cache_write_5m: u64,
    cache_write_1h: u64,
    cache_read: u64,
    output: u64,
    thinking: u64,
    /// Unknown-tier tokens and turns per cause, in [`CAUSES`] order.
    unknown: [(u64, u64); 3],
    models: Vec<String>,
    wall_seconds: f64,
    chars: u64,
    kinds: BTreeMap<String, Part>,
    writes: BTreeMap<String, Part>,
}

struct CostSummary {
    estimate: f64,
    low: f64,
    high: f64,
    unknown: u64,
    assumption: String,
}

fn plus(sum: &mut u64, value: u64) -> Result<(), String> {
    *sum = checked_sum(*sum, value)?;
    Ok(())
}

impl Usage {
    fn named(label: &str) -> Self {
        Self {
            label: label.into(),
            ..Self::default()
        }
    }

    fn of(transcript: &Transcript) -> Result<Self, String> {
        let name = transcript
            .path
            .rsplit(|byte| *byte == b'/')
            .next()
            .unwrap_or_default();
        let mut usage = Self::named(&pytext::stderr_text(name));
        usage.models.clone_from(&transcript.models);
        for request in &transcript.requests {
            usage.turns += 1;
            plus(&mut usage.lines, request.lines)?;
            plus(&mut usage.tool_calls, request.tool_calls)?;
            usage.text_only_turns += u64::from(request.tool_calls == 0);
            plus(&mut usage.input, request.input)?;
            plus(&mut usage.cache_write, request.cache_write)?;
            plus(&mut usage.cache_read, request.cache_read)?;
            plus(&mut usage.output, request.output)?;
            plus(&mut usage.thinking, request.thinking)?;
            usage.classify_cache_write(request)?;
        }
        if let (Some(first), Some(last)) = (transcript.first, transcript.last) {
            usage.wall_seconds = (last - first) as f64 / 1_000_000.0;
        }
        let visible = usage.visible();
        usage.chars = transcript.kind_chars.values().sum();
        usage.kinds = allocate(&transcript.kind_chars, visible);
        usage.writes = transcript
            .write_chars
            .iter()
            .map(|(path, part)| {
                let tokens = token_share(part.chars, usage.chars, visible);
                (
                    path.clone(),
                    Part {
                        tokens,
                        ..part.clone()
                    },
                )
            })
            .collect();
        Ok(usage)
    }

    /// Sorts one request's cache writes into known tiers and unknown-tier tokens by cause.
    fn classify_cache_write(&mut self, request: &Request) -> Result<(), String> {
        let total = request.cache_write;
        let (five, hour) = (request.five, request.hour);
        let cause = if !request.breakdown {
            (total > 0).then_some((0, total))
        } else if checked_sum(five, hour).is_err() || five + hour > total {
            Some((2, total))
        } else {
            plus(&mut self.cache_write_5m, five)?;
            plus(&mut self.cache_write_1h, hour)?;
            let residual = total - five - hour;
            (residual > 0).then_some((1, residual))
        };
        if let Some((cause, tokens)) = cause {
            plus(&mut self.unknown[cause].0, tokens)?;
            self.unknown[cause].1 += 1;
        }
        Ok(())
    }

    fn cache_write_unknown(&self) -> u64 {
        self.unknown.iter().map(|(tokens, _)| tokens).sum()
    }

    fn visible(&self) -> u64 {
        self.output.saturating_sub(self.thinking)
    }

    fn absorb(&mut self, other: &Self) -> Result<(), String> {
        for (sum, value) in [
            (&mut self.turns, other.turns),
            (&mut self.lines, other.lines),
            (&mut self.tool_calls, other.tool_calls),
            (&mut self.text_only_turns, other.text_only_turns),
            (&mut self.input, other.input),
            (&mut self.cache_write, other.cache_write),
            (&mut self.cache_write_5m, other.cache_write_5m),
            (&mut self.cache_write_1h, other.cache_write_1h),
            (&mut self.cache_read, other.cache_read),
            (&mut self.output, other.output),
            (&mut self.thinking, other.thinking),
            (&mut self.chars, other.chars),
        ] {
            plus(sum, value)?;
        }
        for (mine, theirs) in self.unknown.iter_mut().zip(other.unknown) {
            plus(&mut mine.0, theirs.0)?;
            plus(&mut mine.1, theirs.1)?;
        }
        for model in &other.models {
            if !self.models.contains(model) {
                self.models.push(model.clone());
            }
        }
        self.wall_seconds += other.wall_seconds;
        for (mine, theirs) in [
            (&mut self.kinds, &other.kinds),
            (&mut self.writes, &other.writes),
        ] {
            for (name, part) in theirs {
                let entry = mine.entry(name.clone()).or_default();
                entry.count += part.count;
                entry.chars += part.chars;
                entry.tokens += part.tokens;
            }
        }
        Ok(())
    }

    fn cost(&self, prices: &Prices, output: u64, unknown_mult: f64) -> f64 {
        (self.input as f64 * prices.input
            + self.cache_write_5m as f64 * prices.input * prices.write_5m_mult
            + self.cache_write_1h as f64 * prices.input * prices.write_1h_mult
            + self.cache_write_unknown() as f64 * prices.input * unknown_mult
            + self.cache_read as f64 * prices.input * prices.read_mult
            + output as f64 * prices.output)
            / 1_000_000.0
    }

    fn cost_summary(&self, prices: &Prices, output: u64) -> CostSummary {
        let unknown = self.cache_write_unknown();
        let (low, high) = unknown_bounds(prices);
        let assumption = if unknown == 0 {
            "all cache-write tiers known".to_string()
        } else {
            format!(
                "{} unknown-tier cache-write tokens priced at the 5m multiplier ×{} in the estimate and at ×{} / ×{} \
                 in the bounds",
                thousands(unknown),
                general(prices.write_5m_mult),
                general(low),
                general(high)
            )
        };
        CostSummary {
            estimate: self.cost(prices, output, prices.write_5m_mult),
            low: self.cost(prices, output, low),
            high: self.cost(prices, output, high),
            unknown,
            assumption,
        }
    }

    fn unknown_causes(&self) -> String {
        CAUSES
            .iter()
            .zip(self.unknown)
            .filter(|(_, (tokens, turns))| *tokens != 0 || *turns != 0)
            .map(|((cause, reading), (tokens, turns))| {
                let noun = if turns == 1 { "turn" } else { "turns" };
                let what = if *cause == "residual" {
                    format!("{} residual", thousands(tokens))
                } else {
                    thousands(tokens)
                };
                format!("{what} in {} {noun} {reading}", thousands(turns))
            })
            .collect::<Vec<_>>()
            .join("; ")
    }

    fn ordered(table: &BTreeMap<String, Part>) -> Vec<(&String, &Part)> {
        let mut parts: Vec<_> = table.iter().collect();
        parts.sort_by(|a, b| b.1.chars.cmp(&a.1.chars).then_with(|| a.0.cmp(b.0)));
        parts
    }

    fn by_kind(&self) -> Value {
        let section = |table: &BTreeMap<String, Part>, counted: bool| {
            Value::Object(
                Self::ordered(table)
                    .into_iter()
                    .map(|(name, part)| {
                        let mut entry = Object::default();
                        if counted {
                            entry.insert("count".into(), int(part.count));
                        }
                        entry.insert("chars".into(), int(part.chars));
                        entry.insert("tokens".into(), Value::Int(part.tokens.to_string()));
                        (name.clone(), Value::Object(entry))
                    })
                    .collect(),
            )
        };
        let mut data = Object::default();
        data.insert("thinking".into(), int(self.thinking));
        data.insert("visible".into(), int(self.visible()));
        data.insert("chars".into(), int(self.chars));
        data.insert("kinds".into(), section(&self.kinds, false));
        data.insert("writes".into(), section(&self.writes, true));
        Value::Object(data)
    }

    fn fields(&self) -> Object {
        let mut data = Object::default();
        data.insert("label".into(), Value::Str(self.label.clone()));
        for (name, value) in [
            ("turns", self.turns),
            ("lines", self.lines),
            ("tool_calls", self.tool_calls),
            ("text_only_turns", self.text_only_turns),
            ("input", self.input),
            ("cache_write", self.cache_write),
            ("cache_write_5m", self.cache_write_5m),
            ("cache_write_1h", self.cache_write_1h),
            ("cache_write_unknown", self.cache_write_unknown()),
            ("cache_read", self.cache_read),
            ("output", self.output),
            ("thinking", self.thinking),
        ] {
            data.insert(name.into(), int(value));
        }
        let causes = CAUSES
            .iter()
            .zip(self.unknown)
            .map(|((cause, _), (tokens, turns))| {
                let entry = [
                    ("tokens".to_string(), int(tokens)),
                    ("turns".to_string(), int(turns)),
                ];
                (
                    (*cause).to_string(),
                    Value::Object(entry.into_iter().collect()),
                )
            })
            .collect();
        data.insert("cache_write_unknown_by_cause".into(), Value::Object(causes));
        data.insert(
            "models".into(),
            Value::Array(self.models.iter().cloned().map(Value::Str).collect()),
        );
        data.insert("wall".into(), Value::Str(wall_text(self.wall_seconds)));
        data.insert(
            "wall_seconds".into(),
            Value::Float(round_to(self.wall_seconds, 3)),
        );
        data
    }
}

fn unknown_bounds(prices: &Prices) -> (f64, f64) {
    (
        prices.write_5m_mult.min(prices.write_1h_mult),
        prices.write_5m_mult.max(prices.write_1h_mult),
    )
}

fn add_costs(entry: &mut Object, usage: &Usage, prices: &Prices) {
    let summary = usage.cost_summary(prices, usage.output);
    entry.insert("cost".into(), Value::Float(round_to(summary.estimate, 6)));
    let mut bounds = Object::default();
    bounds.insert("low".into(), Value::Float(round_to(summary.low, 6)));
    bounds.insert("high".into(), Value::Float(round_to(summary.high, 6)));
    bounds.insert("unknown_tier_tokens".into(), int(summary.unknown));
    bounds.insert("tiers_known".into(), Value::Bool(summary.unknown == 0));
    bounds.insert("assumption".into(), Value::Str(summary.assumption));
    entry.insert("cost_bounds".into(), Value::Object(bounds));
}

fn prices_fields(prices: &Prices) -> Object {
    let mut data = Object::default();
    data.insert("input".into(), Value::Float(prices.input));
    data.insert("output".into(), Value::Float(prices.output));
    data.insert(
        "cache_write_mult".into(),
        Value::Float(prices.write_5m_mult),
    );
    data.insert(
        "cache_write_1h_mult".into(),
        Value::Float(prices.write_1h_mult),
    );
    data.insert("cache_read_mult".into(), Value::Float(prices.read_mult));
    data.insert("unknown_tier_fallback".into(), Value::Str(FALLBACK.into()));
    data
}

/// `H:MM:SS` of the seconds rounded half to even.
fn wall_text(seconds: f64) -> String {
    let total = seconds.round_ties_even() as i64;
    let (hours, rest) = (total.div_euclid(3600), total.rem_euclid(3600));
    format!("{hours}:{:02}:{:02}", rest / 60, rest % 60)
}

/// The report's bytes ÷ 4, rounded half to even: its estimated output tokens.
fn report_tokens(path: &[u8]) -> Result<u64, String> {
    let size = std::fs::metadata(std::ffi::OsStr::from_bytes(path))
        .map_err(|error| {
            format!(
                "cannot read {}: {}",
                pytext::stderr_text(path),
                pytext::strerror(&error)
            )
        })?
        .len();
    let (quotient, remainder) = (size / 4, size % 4);
    Ok(quotient + u64::from(remainder > 2 || (remainder == 2 && quotient % 2 == 1)))
}

fn block_lines(
    usage: &Usage,
    prices: &Prices,
    shaped_tokens: Option<u64>,
    by_kind: bool,
) -> Vec<String> {
    let line = |label: &str, value: &str, rest: &str| format!("{label:<WIDTH$} {value:>12}{rest}");
    let unknown = usage.cache_write_unknown();
    let mut tiers = format!(
        "5m {}, 1h {}",
        thousands(usage.cache_write_5m),
        thousands(usage.cache_write_1h)
    );
    if unknown > 0 {
        let _ = write!(tiers, ", unknown tier {}", thousands(unknown));
    }
    let mut lines = vec![
        usage.label.clone(),
        line(
            "turns",
            &thousands(usage.turns),
            &format!(
                " (API requests; {} assistant lines)",
                thousands(usage.lines)
            ),
        ),
        line("tool calls", &thousands(usage.tool_calls), ""),
        line("text-only turns", &thousands(usage.text_only_turns), ""),
        line("input", &thousands(usage.input), " tokens (uncached)"),
        line(
            "cache write",
            &thousands(usage.cache_write),
            &format!(" tokens ({tiers})"),
        ),
    ];
    if unknown > 0 {
        lines.push(line(
            "unknown tier",
            &thousands(unknown),
            &format!(" tokens: {}", usage.unknown_causes()),
        ));
    }
    lines.push(line("cache read", &thousands(usage.cache_read), " tokens"));
    lines.push(line(
        "output",
        &thousands(usage.output),
        &format!(" tokens (thinking {})", thousands(usage.thinking)),
    ));
    let models = if usage.models.is_empty() {
        "none recorded".to_string()
    } else {
        usage.models.join(", ")
    };
    lines.push(line("models", &models, ""));
    let (wall_label, wall_note) = if usage.label == "TOTAL" {
        ("agent span sum", " (summed over transcripts; not elapsed)")
    } else {
        ("wall", " (assistant timestamp span)")
    };
    lines.push(line(wall_label, &wall_text(usage.wall_seconds), wall_note));
    let summary = usage.cost_summary(prices, usage.output);
    let tier_note = if summary.unknown == 0 {
        "all cache-write tiers known"
    } else {
        "unknown-tier cache writes priced as 5m"
    };
    lines.push(line(
        "cost",
        &format!("{:.2}", summary.estimate),
        &format!(
            " $ at {}/{} per M, cache write ×{} (5m) ×{} (1h), cache read ×{}; {tier_note}",
            general(prices.input),
            general(prices.output),
            general(prices.write_5m_mult),
            general(prices.write_1h_mult),
            general(prices.read_mult)
        ),
    ));
    if summary.unknown > 0 {
        lines.push(line(
            "cost bounds",
            &format!("{:.2}", summary.low),
            &format!(" – {:.2} $ ({})", summary.high, summary.assumption),
        ));
    }
    if let Some(tokens) = shaped_tokens {
        let shaped_output = usage.output.saturating_sub(tokens);
        let shaped = usage.cost_summary(prices, shaped_output);
        let bounds = if shaped.unknown == 0 {
            String::new()
        } else {
            format!("; bounds {:.2} – {:.2} $", shaped.low, shaped.high)
        };
        lines.push(line(
            "production-shaped",
            &format!("{:.2}", shaped.estimate),
            &format!(
                " $ (output {} after subtracting the report's {} est. tokens){bounds}",
                thousands(shaped_output),
                thousands(tokens)
            ),
        ));
    }
    if by_kind {
        lines.extend(by_kind_lines(usage));
    }
    lines
}

fn by_kind_lines(usage: &Usage) -> Vec<String> {
    let signed = |value: i64| {
        let digits = thousands(value.unsigned_abs());
        if value < 0 {
            format!("-{digits}")
        } else {
            digits
        }
    };
    let inner = WIDTH - 2;
    let mut lines = vec![
        "output by kind".to_string(),
        format!(
            "{:<WIDTH$} {:>12} tokens (from usage)",
            "thinking",
            thousands(usage.thinking)
        ),
        format!(
            "{:<WIDTH$} {:>12} tokens over {} chars",
            "visible",
            thousands(usage.visible()),
            thousands(usage.chars)
        ),
    ];
    for (name, part) in Usage::ordered(&usage.kinds) {
        lines.push(format!(
            "  {name:<inner$} {:>12} tokens {:>9} chars",
            signed(part.tokens),
            thousands(part.chars)
        ));
    }
    let writes = Usage::ordered(&usage.writes);
    lines.push(format!(
        "{:<WIDTH$} {:>12} tokens {:>9} chars in {} writes to {} paths",
        "file writes",
        signed(writes.iter().map(|(_, part)| part.tokens).sum()),
        thousands(writes.iter().map(|(_, part)| part.chars).sum()),
        thousands(writes.iter().map(|(_, part)| part.count).sum()),
        thousands(writes.len() as u64)
    ));
    for (path, part) in writes {
        let again = if part.count > 1 {
            format!(", written {} times", thousands(part.count))
        } else {
            String::new()
        };
        lines.push(format!(
            "  {path:<inner$} {:>12} tokens {:>9} chars{again}",
            signed(part.tokens),
            thousands(part.chars)
        ));
    }
    lines
}

fn cost_cell(summary: &CostSummary, prices: &Prices, bold: bool) -> String {
    let estimate = format!("{:.2}", summary.estimate);
    let mut cell = if bold {
        format!("**{estimate}**")
    } else {
        estimate
    };
    if summary.unknown > 0 {
        let _ = write!(
            cell,
            " (unknown tier at ×{}; {:.2}–{:.2})",
            general(prices.write_5m_mult),
            summary.low,
            summary.high
        );
    }
    cell
}

fn row_line(usage: &Usage, label: &str, prices: &Prices, shaped_tokens: Option<u64>) -> String {
    let unknown = usage.cache_write_unknown();
    let mut cache_write = thousands(usage.cache_write);
    if usage.cache_write_1h > 0 || unknown > 0 {
        let _ = write!(
            cache_write,
            " (5m {}, 1h {}",
            thousands(usage.cache_write_5m),
            thousands(usage.cache_write_1h)
        );
        if unknown > 0 {
            let _ = write!(cache_write, ", unknown {})", thousands(unknown));
        } else {
            cache_write.push(')');
        }
    }
    let (report_output, shaped_cost) = match shaped_tokens {
        Some(tokens) => {
            let shaped = usage.cost_summary(prices, usage.output.saturating_sub(tokens));
            (thousands(tokens), cost_cell(&shaped, prices, true))
        }
        None => ("—".to_string(), "—".to_string()),
    };
    super::table_row([
        label.to_string(),
        if usage.models.is_empty() {
            "—".to_string()
        } else {
            usage.models.join(", ")
        },
        thousands(usage.turns),
        thousands(usage.tool_calls),
        thousands(usage.text_only_turns),
        thousands(usage.input),
        cache_write,
        thousands(usage.cache_read),
        thousands(usage.output),
        thousands(usage.thinking),
        wall_text(usage.wall_seconds),
        cost_cell(&usage.cost_summary(prices, usage.output), prices, false),
        report_output,
        shaped_cost,
    ])
}

/// A run's explicit boundaries from its timing sidecar; never inferred from transcript spans.
#[derive(Default)]
struct Timing {
    mode: Option<String>,
    events: [Option<i64>; 3],
}

impl Timing {
    fn elapsed(&self) -> [Option<f64>; 2] {
        let root = self.events[0];
        [self.events[1], self.events[2]].map(|end| {
            root.zip(end)
                .map(|(root, end)| (end - root) as f64 / 1_000_000.0)
        })
    }

    fn fields(&self) -> Object {
        let mut data = Object::default();
        data.insert(
            "completion_mode".into(),
            self.mode.clone().map_or(Value::Null, Value::Str),
        );
        for (name, event) in TIMING_EVENTS.iter().zip(self.events) {
            data.insert(
                (*name).into(),
                event.map_or(Value::Null, |micros| Value::Str(utc_isoformat(micros))),
            );
        }
        let [payload, completion] = self.elapsed();
        data.insert(
            "elapsed_to_payload_seconds".into(),
            payload.map_or(Value::Null, Value::Float),
        );
        data.insert(
            "elapsed_to_completion_seconds".into(),
            completion.map_or(Value::Null, Value::Float),
        );
        data
    }

    fn lines(&self) -> Vec<String> {
        let mut lines = vec!["RUN TIMING".to_string()];
        for (name, value) in self.fields().iter() {
            let shown = match value {
                Value::Null => "unavailable".to_string(),
                other => other.py_str(),
            };
            lines.push(format!("{}: {shown}", name.replace('_', " ")));
        }
        lines
    }
}

fn read_timing(path: &[u8]) -> Result<Timing, String> {
    let text = pytext::read_file_text(path).map_err(|error| error.message().to_string())?;
    let data =
        match pyjson::loads(&text, Duplicates::KeepLast).map_err(|error| error.to_string())? {
            Value::Object(data) => data,
            _ => return Err("expected a JSON object".into()),
        };
    let mut unknown: Vec<&String> = data
        .iter()
        .map(|(key, _)| key)
        .filter(|key| key.as_str() != "completion_mode" && !TIMING_EVENTS.contains(&key.as_str()))
        .collect();
    if !unknown.is_empty() {
        unknown.sort();
        let names: Vec<&str> = unknown.iter().map(|key| key.as_str()).collect();
        return Err(format!("unknown timing keys: {}", names.join(", ")));
    }
    let mode = match data.get("completion_mode") {
        Some(Value::Str(mode))
            if ["publication", "result", "render-only"].contains(&mode.as_str()) =>
        {
            mode.clone()
        }
        _ => return Err("completion_mode must be publication, result, or render-only".into()),
    };
    let mut timing = Timing {
        mode: Some(mode),
        events: [None; 3],
    };
    let mut previous: Option<(i64, &str)> = None;
    for (slot, name) in timing.events.iter_mut().zip(TIMING_EVENTS) {
        let value = match data.get(name) {
            None | Some(Value::Null) => continue,
            Some(Value::Str(value)) if zoned_timestamp(value) => value,
            Some(_) => {
                return Err(format!(
                    "{name} must be an ISO-8601 timestamp with timezone"
                ));
            }
        };
        let stamp = timestamp_micros(&six_digit_fraction(value))
            .ok_or_else(|| format!("invalid {name}: {}", pytext::repr_str(value)))?;
        if let Some((before, before_name)) = previous
            && stamp < before
        {
            return Err(format!("{name} precedes {before_name}"));
        }
        *slot = Some(stamp);
        previous = Some((stamp, name));
    }
    Ok(timing)
}

/// `YYYY-MM-DDTHH:MM:SS[.f+](Z|±HH:MM)`.
fn zoned_timestamp(text: &str) -> bool {
    let bytes = text.as_bytes();
    let digits = |range: std::ops::Range<usize>| {
        bytes
            .get(range)
            .is_some_and(|part| part.iter().all(u8::is_ascii_digit))
    };
    let shape = digits(0..4)
        && bytes.get(4) == Some(&b'-')
        && digits(5..7)
        && bytes.get(7) == Some(&b'-')
        && digits(8..10)
        && bytes.get(10) == Some(&b'T')
        && digits(11..13)
        && bytes.get(13) == Some(&b':')
        && digits(14..16)
        && bytes.get(16) == Some(&b':')
        && digits(17..19);
    if !shape {
        return false;
    }
    let mut at = 19;
    if bytes.get(at) == Some(&b'.') {
        let start = at + 1;
        at = start;
        while bytes.get(at).is_some_and(u8::is_ascii_digit) {
            at += 1;
        }
        if at == start {
            return false;
        }
    }
    match &bytes[at..] {
        b"Z" => true,
        [sign, h1, h2, b':', m1, m2] => {
            matches!(sign, b'+' | b'-')
                && matches!((h1, h2), (b'0' | b'1', b'0'..=b'9') | (b'2', b'0'..=b'3'))
                && matches!((m1, m2), (b'0'..=b'5', b'0'..=b'9'))
        }
        _ => false,
    }
}

fn six_digit_fraction(text: &str) -> String {
    let Some(dot) = text.find('.') else {
        return text.to_string();
    };
    let end = text[dot + 1..]
        .find(|c: char| !c.is_ascii_digit())
        .map_or(text.len(), |offset| dot + 1 + offset);
    let digits = &text[dot + 1..end];
    let fraction: String = format!("{digits:0<6}").chars().take(6).collect();
    format!("{}.{fraction}{}", &text[..dot], &text[end..])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redirect_targets_follow_the_pattern() {
        assert_eq!(
            redirect_target("cat > out.md <<'EOF'").as_deref(),
            Some("out.md")
        );
        assert_eq!(redirect_target("cat <<EOF >> 'a b'").as_deref(), Some("a"));
        assert_eq!(
            redirect_target("cat <<EOF | tee -a log.txt").as_deref(),
            Some("log.txt")
        );
        assert_eq!(redirect_target("cat <<EOF | tee x").as_deref(), Some("x"));
        assert_eq!(redirect_target("x >> | y").as_deref(), Some(">"));
        assert_eq!(redirect_target("cat <<EOF"), None);
    }

    #[test]
    fn shares_round_half_to_even_and_sum_to_the_visible_output() {
        assert_eq!(token_share(1, 4, 2), 0);
        assert_eq!(token_share(3, 4, 2), 2);
        let kinds: BTreeMap<String, u64> =
            [("text".to_string(), 1), ("tool:Bash".to_string(), 2)].into();
        let parts = allocate(&kinds, 10);
        assert_eq!(parts["text"].tokens + parts["tool:Bash"].tokens, 10);
        assert_eq!(parts["tool:Bash"].tokens, 7);
    }

    #[test]
    fn timing_timestamps_need_a_zone_and_keep_microseconds() {
        assert!(zoned_timestamp("2026-09-05T12:00:00.250Z"));
        assert!(zoned_timestamp("2026-09-05T08:00:00-04:00"));
        assert!(!zoned_timestamp("2026-09-05T08:00:00"));
        assert!(!zoned_timestamp("2026-09-05T08:00:00+24:00"));
        assert_eq!(
            six_digit_fraction("2026-09-05T12:00:00.1234567Z"),
            "2026-09-05T12:00:00.123456Z"
        );
        assert_eq!(
            six_digit_fraction("2026-09-05T12:00:00.25+01:00"),
            "2026-09-05T12:00:00.250000+01:00"
        );
    }

    #[test]
    fn wall_text_rounds_half_to_even() {
        assert_eq!(wall_text(2.5), "0:00:02");
        assert_eq!(wall_text(3725.0), "1:02:05");
    }
}
