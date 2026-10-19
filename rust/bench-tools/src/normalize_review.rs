//! Normalize one reviewer's native output into the benchmark's common finding schema, as the retained
//! `bench/tools/normalize_review.py` does, byte for byte.
//!
//! The output is `{"arm", "parse_status", "native_verdict", "verdict_source", "items", "parse_notes"}`, with
//! `native_payload` for skill reports, written as `json.dumps(doc, indent=2)` with a final newline.
//! `parse_status` is `parsed`, `empty` or `unresolved`; an unresolved result is still written, and the
//! command then prints one line and exits 1. `--timing` stamps `payload_validated_at` in the sidecar after a
//! parsed or empty result. An unreadable input exits 2 with its message on stderr.
//!
//! Inputs keep Python's duck typing where the result is a value: a title that is not text is formatted as
//! `str()` formats it. An input whose shape made Python raise a traceback exits 2 here with a message.

use crate::pyjson::{self, Dumps, Duplicates, Object, Value};
use crate::pytext::{self, PyPath};

pub const PROGRAM: &str = "normalize_review.py";

pub const UNRESOLVED: &str = "unresolved: the native input lacks the arm's expected review markers or has a region that did not parse; adjudication required\n";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arm {
    ReviewCode,
    ClaudeBuiltin,
    ClaudeSkill,
    Codex,
    CodexSkill,
}

impl Arm {
    pub const NAMES: [&'static str; 5] = [
        "review-code",
        "claude-builtin",
        "claude-skill",
        "codex",
        "codex-skill",
    ];

    pub fn parse(name: &[u8]) -> Option<Self> {
        Some(match name {
            b"review-code" => Self::ReviewCode,
            b"claude-builtin" => Self::ClaudeBuiltin,
            b"claude-skill" => Self::ClaudeSkill,
            b"codex" => Self::Codex,
            b"codex-skill" => Self::CodexSkill,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::ReviewCode => "review-code",
            Self::ClaudeBuiltin => "claude-builtin",
            Self::ClaudeSkill => "claude-skill",
            Self::Codex => "codex",
            Self::CodexSkill => "codex-skill",
        }
    }
}

/// One normalized item, in the key order Python's `item()` builds.
#[derive(Clone, Debug)]
pub struct Item {
    pub file: Value,
    pub line_start: Value,
    pub line_end: Value,
    pub claim: Value,
    pub consequence: Value,
    pub proposed_fix: Value,
    pub native_priority: Value,
    pub native_action: Value,
    pub native_confidence: Value,
    pub kind: &'static str,
    pub native_fields: Option<Value>,
}

impl Default for Item {
    fn default() -> Self {
        Self {
            file: Value::Null,
            line_start: Value::Null,
            line_end: Value::Null,
            claim: text(""),
            consequence: Value::Null,
            proposed_fix: Value::Null,
            native_priority: Value::Null,
            native_action: Value::Null,
            native_confidence: Value::Null,
            kind: "finding",
            native_fields: None,
        }
    }
}

impl Item {
    fn into_value(self) -> Value {
        let mut pairs = vec![
            ("file", self.file),
            ("line_start", self.line_start),
            ("line_end", self.line_end),
            ("claim", self.claim),
            ("consequence", self.consequence),
            ("proposed_fix", self.proposed_fix),
            ("native_priority", self.native_priority),
            ("native_action", self.native_action),
            ("native_confidence", self.native_confidence),
            ("kind", text(self.kind)),
        ];
        if let Some(fields) = self.native_fields {
            pairs.push(("native_fields", fields));
        }
        object(pairs)
    }
}

#[derive(Clone, Debug)]
pub struct Normalized {
    pub parse_status: &'static str,
    pub native_verdict: Value,
    pub verdict_source: Value,
    pub items: Vec<Item>,
    pub parse_notes: Vec<String>,
    pub native_payload: Option<Value>,
}

impl Normalized {
    /// `{"arm": arm, **doc}`.
    pub fn into_value(self, arm: &str) -> Value {
        let mut pairs = vec![
            ("arm", text(arm)),
            ("parse_status", text(self.parse_status)),
            ("native_verdict", self.native_verdict),
            ("verdict_source", self.verdict_source),
            (
                "items",
                Value::Array(self.items.into_iter().map(Item::into_value).collect()),
            ),
            (
                "parse_notes",
                Value::Array(self.parse_notes.into_iter().map(Value::Str).collect()),
            ),
        ];
        if let Some(payload) = self.native_payload {
            pairs.push(("native_payload", payload));
        }
        object(pairs)
    }
}

pub fn text(value: &str) -> Value {
    Value::Str(value.to_string())
}

pub fn object(pairs: Vec<(&str, Value)>) -> Value {
    Value::Object(
        pairs
            .into_iter()
            .map(|(key, value)| (key.to_string(), value))
            .collect(),
    )
}

/// `mapping.get(key, default)`.
fn get(mapping: &Object, key: &str, default: Value) -> Value {
    mapping.get(key).cloned().unwrap_or(default)
}

/// The elements of `for element in value` when every element is then used as a `dict`.
fn objects<'a>(value: &'a Value, what: &str) -> Result<Vec<&'a Object>, String> {
    match value {
        Value::Array(values) => values
            .iter()
            .map(|element| {
                element
                    .as_object()
                    .ok_or_else(|| format!("{what} holds an element that is not an object"))
            })
            .collect(),
        Value::Str(_) | Value::Object(_) if !value.truthy() => Ok(Vec::new()),
        _ => Err(format!("{what} is not a list of objects")),
    }
}

/// `value or {}` used as a `dict`.
fn mapping_or_empty<'a>(
    value: Option<&'a Value>,
    what: &str,
) -> Result<Option<&'a Object>, String> {
    match value {
        Some(value) if value.truthy() => value
            .as_object()
            .map(Some)
            .ok_or_else(|| format!("{what} is not an object")),
        _ => Ok(None),
    }
}

/// The path as `relpath(path, clone)` returns it: repository-relative when `--clone` is given and the path
/// is absolute.
fn relpath(path: Value, clone: Option<&[u8]>, what: &str) -> Result<Value, String> {
    let Some(clone) = clone.filter(|clone| !clone.is_empty()) else {
        return Ok(path);
    };
    match path {
        Value::Str(absolute) if absolute.starts_with('/') => pytext::relpath(&absolute, clone)
            .map(Value::Str)
            .map_err(|error| format!("cannot read the current directory: {error}")),
        Value::Str(_) => Ok(path),
        _ => Err(format!("{what} is not text")),
    }
}

pub fn from_review_code(composition: &Value, clone: Option<&[u8]>) -> Result<Normalized, String> {
    let composition = composition
        .as_object()
        .ok_or("the composition is not an object")?;
    let mut items = Vec::new();
    let mut notes = Vec::new();
    let findings = get(composition, "findings", Value::Array(Vec::new()));
    for finding in objects(&findings, "findings")? {
        let anchor = mapping_or_empty(finding.get("anchor"), "a finding's anchor")?;
        let path = anchor.map_or(text(""), |anchor| get(anchor, "path", text("")));
        let claim = format!(
            "{}. {}",
            get(finding, "title", text("")).py_str(),
            get(finding, "trigger", text("")).py_str()
        );
        items.push(Item {
            file: relpath(path, clone, "a finding's anchor path")?,
            line_start: anchor.map_or(Value::Null, |anchor| get(anchor, "start_line", Value::Null)),
            line_end: anchor.map_or(Value::Null, |anchor| get(anchor, "end_line", Value::Null)),
            claim: text(pytext::strip(&claim)),
            consequence: get(finding, "impact", Value::Null),
            proposed_fix: get(finding, "change", Value::Null),
            native_priority: get(finding, "priority", Value::Null),
            native_action: get(finding, "action", Value::Null),
            ..Item::default()
        });
    }
    let questions = get(composition, "questions", Value::Array(Vec::new()));
    for question in objects(&questions, "questions")? {
        let anchor = mapping_or_empty(question.get("anchor"), "a question's anchor")?;
        let path = anchor.map_or(text(""), |anchor| get(anchor, "path", text("")));
        items.push(Item {
            file: relpath(path, clone, "a question's anchor path")?,
            claim: get(question, "title", text("")),
            consequence: get(question, "why_it_matters", Value::Null),
            proposed_fix: get(question, "answer", Value::Null),
            kind: "question",
            ..Item::default()
        });
    }
    let observations = get(composition, "observations", Value::Array(Vec::new()));
    for observation in objects(&observations, "observations")? {
        items.push(Item {
            claim: get(observation, "fact", text("")),
            consequence: get(observation, "evidence", Value::Null),
            kind: "observation",
            ..Item::default()
        });
    }
    let summary = mapping_or_empty(composition.get("summary"), "the summary")?;
    let status = summary.map_or(Value::Null, |summary| get(summary, "status", Value::Null));
    let missing = matches!(status, Value::Null);
    if missing {
        notes.push("unresolved: composition has no summary.status".to_string());
    }
    Ok(Normalized {
        parse_status: if missing {
            "unresolved"
        } else if items.is_empty() {
            "empty"
        } else {
            "parsed"
        },
        native_verdict: status,
        verdict_source: text("composition.summary.status"),
        items,
        parse_notes: notes,
        native_payload: None,
    })
}

fn dumps_unicode(value: &Value) -> String {
    pyjson::dumps(
        value,
        &Dumps {
            ensure_ascii: false,
            ..Dumps::default()
        },
    )
}

/// A structured skill report, keeping every native finding field.
pub fn from_codex_skill(review: &Value, clone: Option<&[u8]>) -> Result<Normalized, String> {
    let Some(report) = review.as_object() else {
        return Ok(Normalized {
            parse_status: "unresolved",
            native_verdict: Value::Null,
            verdict_source: Value::Null,
            items: vec![Item {
                claim: Value::Str(dumps_unicode(review)),
                ..Item::default()
            }],
            parse_notes: vec!["unresolved: native skill report is not a JSON object".to_string()],
            native_payload: Some(review.clone()),
        });
    };
    let mut notes = Vec::new();
    let mut items = Vec::new();
    let status = get(report, "status", Value::Null);
    let verdict = get(report, "verdict", Value::Null);
    let findings = match report.get("findings") {
        Some(Value::Array(findings)) => findings.as_slice(),
        _ => {
            notes.push("unresolved: native skill report has no findings array".to_string());
            &[]
        }
    };
    for (index, finding) in findings.iter().enumerate() {
        let number = index + 1;
        let Some(fields) = finding.as_object() else {
            notes.push(format!("unresolved: finding {number} is not an object"));
            items.push(Item {
                claim: Value::Str(dumps_unicode(finding)),
                native_fields: Some(finding.clone()),
                ..Item::default()
            });
            continue;
        };
        let title = get(fields, "title", Value::Null);
        if title
            .as_str()
            .is_none_or(|title| pytext::strip(title).is_empty())
        {
            notes.push(format!(
                "unresolved: finding {number} has no non-empty title"
            ));
        }
        let mut file = get(fields, "file", Value::Null);
        if !matches!(file, Value::Null | Value::Str(_)) {
            notes.push(format!(
                "unresolved: finding {number} file is not a string or null"
            ));
            file = Value::Null;
        }
        let mut line = get(fields, "line", Value::Null);
        let positive =
            matches!(&line, Value::Int(digits) if !digits.starts_with('-') && digits != "0");
        if !matches!(line, Value::Null) && !positive {
            notes.push(format!(
                "unresolved: finding {number} line is not a positive integer or null"
            ));
            line = Value::Null;
        }
        let file = if file.truthy() {
            relpath(file, clone, "a finding's file")?
        } else {
            Value::Null
        };
        let claim = match title {
            Value::Str(title) => Value::Str(title),
            _ => Value::Str(dumps_unicode(finding)),
        };
        items.push(Item {
            file,
            line_start: line.clone(),
            line_end: line,
            claim,
            consequence: get(fields, "why_it_matters", Value::Null),
            proposed_fix: get(fields, "suggested_fix", Value::Null),
            native_priority: get(fields, "severity", Value::Null),
            native_action: get(fields, "autofix_class", Value::Null),
            native_confidence: get(fields, "confidence", Value::Null),
            native_fields: Some(finding.clone()),
            ..Item::default()
        });
    }
    if status.as_str() != Some("complete") {
        notes.push(format!(
            "unresolved: native skill report status is {}, expected 'complete'",
            status.py_repr()
        ));
    }
    if verdict
        .as_str()
        .is_none_or(|verdict| pytext::strip(verdict).is_empty())
    {
        notes.push("unresolved: native skill report has no non-empty verdict".to_string());
    }
    let parse_status = if !notes.is_empty() {
        "unresolved"
    } else if items.is_empty() {
        "empty"
    } else {
        "parsed"
    };
    let (native_verdict, source) = match verdict {
        Value::Str(_) => (verdict, "review.json.verdict"),
        _ => (status, "review.json.status"),
    };
    Ok(Normalized {
        parse_status,
        native_verdict,
        verdict_source: text(source),
        items,
        parse_notes: notes,
        native_payload: Some(review.clone()),
    })
}

/// `list.extend(value or [])`: a list's elements, a text's characters or an object's keys.
fn extended(value: Option<&Value>) -> Result<Vec<Value>, String> {
    match value {
        Some(value) if value.truthy() => match value {
            Value::Array(values) => Ok(values.clone()),
            Value::Str(characters) => Ok(characters
                .chars()
                .map(|c| Value::Str(c.to_string()))
                .collect()),
            Value::Object(object) => Ok(object.iter().map(|(key, _)| text(key)).collect()),
            _ => Err("a ReportFindings call's findings is not a list".to_string()),
        },
        _ => Ok(Vec::new()),
    }
}

/// The bracketed group of the first match of ``` ```(?:json)?\s*(\[.*?\])\s*``` ``` with `re.S`.
pub fn fenced_array(text: &str) -> Option<&str> {
    let bytes = text.as_bytes();
    let after_space = |mut index: usize| {
        while let Some(c) = text[index..]
            .chars()
            .next()
            .filter(|c| pytext::is_space(*c))
        {
            index += c.len_utf8();
        }
        index
    };
    for start in (0..bytes.len()).filter(|index| bytes[*index..].starts_with(b"```")) {
        let mut open = start + 3;
        if text[open..].starts_with("json") {
            open += 4;
        }
        let open = after_space(open);
        if bytes.get(open) != Some(&b'[') {
            continue;
        }
        for close in (open + 1..bytes.len()).filter(|index| bytes[*index] == b']') {
            if text[after_space(close + 1)..].starts_with("```") {
                return Some(&text[open..=close]);
            }
        }
    }
    None
}

pub fn from_claude_builtin(payload: &Value) -> Result<Normalized, String> {
    let payload = payload.as_object().ok_or("the payload is not an object")?;
    let mut items = Vec::new();
    let mut notes = Vec::new();
    let calls = get(payload, "report_findings", Value::Null);
    let mut source = None;
    let mut findings: Vec<Value> = Vec::new();
    let mut raw = false;
    if calls.truthy() {
        source = Some("ReportFindings");
        let Value::Array(calls) = &calls else {
            return Err("report_findings is not a list of objects".to_string());
        };
        for call in calls {
            let call = call
                .as_object()
                .ok_or("report_findings holds an element that is not an object")?;
            findings.extend(extended(call.get("findings"))?);
        }
    } else {
        let final_text = get(payload, "final_text", Value::Null);
        let final_text = match &final_text {
            Value::Str(final_text) => final_text.as_str(),
            value if !value.truthy() => "",
            _ => return Err("final_text is not text".to_string()),
        };
        if let Some(array) = fenced_array(final_text) {
            source = Some("final_text json");
            match pyjson::loads(array, Duplicates::KeepLast) {
                Ok(Value::Array(parsed)) => findings = parsed,
                Ok(_) => return Err("the fenced JSON is not an array".to_string()),
                Err(pyjson::ParseError::Syntax { message, .. }) => {
                    notes.push(format!("unresolved: fenced JSON did not parse: {message}"));
                    items.push(Item {
                        claim: text(array),
                        ..Item::default()
                    });
                    raw = true;
                }
                Err(error) => return Err(format!("the fenced JSON cannot be read: {error}")),
            }
        } else if !pytext::strip(final_text).is_empty() {
            notes.push(
                "unresolved: no fenced JSON array and no ReportFindings call; final text kept raw"
                    .to_string(),
            );
            items.push(Item {
                claim: text(pytext::strip(final_text)),
                ..Item::default()
            });
        }
    }
    for finding in &findings {
        let Some(fields) = finding.as_object() else {
            notes.push("unresolved: non-object finding kept raw".to_string());
            items.push(Item {
                claim: Value::Str(pyjson::dumps(finding, &Dumps::default())),
                ..Item::default()
            });
            raw = true;
            continue;
        };
        items.push(Item {
            file: get(fields, "file", Value::Null),
            line_start: get(fields, "line", Value::Null),
            line_end: get(fields, "line", Value::Null),
            claim: get(fields, "summary", text("")),
            consequence: get(fields, "failure_scenario", Value::Null),
            native_confidence: get(fields, "verdict", Value::Null),
            ..Item::default()
        });
    }
    if source.is_none() && items.is_empty() {
        notes.push("unresolved: no review output found".to_string());
    }
    let verdict = if source.is_some() && findings.is_empty() && notes.is_empty() {
        text("empty-array")
    } else if findings.is_empty() {
        Value::Null
    } else {
        text("findings")
    };
    let parse_status = if source.is_none() || raw {
        "unresolved"
    } else if findings.is_empty() {
        "empty"
    } else {
        "parsed"
    };
    Ok(Normalized {
        parse_status,
        native_verdict: verdict,
        verdict_source: source.map_or(Value::Null, text),
        items,
        parse_notes: notes,
        native_payload: None,
    })
}

/// The `[^...]+` run after `start` that a closing `stop` follows: `\"` for an escaped value, `"` otherwise.
fn quoted_run(line: &str, start: usize, escaped: bool) -> Option<(&str, usize)> {
    let rest = &line[start..];
    let end = rest
        .find(|c| c == '"' || escaped && c == '\\')
        .unwrap_or(rest.len());
    if end == 0 {
        return None;
    }
    let closing = if escaped { "\\\"" } else { "\"" };
    rest[end..]
        .starts_with(closing)
        .then(|| (&rest[..end], start + end + closing.len()))
}

/// Each `overall_correctness` value `re.finditer` finds in one rollout line, escaped inside a JSON string
/// or bare.
fn overall_values(line: &str) -> Vec<&str> {
    let mut found = Vec::new();
    let mut index = 0;
    while index <= line.len() {
        let attempt = |escaped: bool| {
            let key = if escaped {
                "\\\"overall_correctness\\\":"
            } else {
                "\"overall_correctness\":"
            };
            if !line[index..].starts_with(key) {
                return None;
            }
            let mut value = index + key.len();
            while let Some(c) = line[value..]
                .chars()
                .next()
                .filter(|c| pytext::is_space(*c))
            {
                value += c.len_utf8();
            }
            let opening = if escaped { "\\\"" } else { "\"" };
            line[value..]
                .starts_with(opening)
                .then(|| quoted_run(line, value + opening.len(), escaped))
                .flatten()
        };
        match attempt(true).or_else(|| attempt(false)) {
            Some((value, end)) => {
                found.push(value);
                index = end;
            }
            None => {
                index += line[index..].chars().next().map_or(1, char::len_utf8);
            }
        }
    }
    found
}

/// The `rollout-*.jsonl` files under `root`, sorted as `sorted(Path(root).rglob(...))` sorts them: by
/// path components. Symbolic links to directories are not followed.
fn rollouts(root: &PyPath) -> Vec<PyPath> {
    let mut found = Vec::new();
    let mut directories = vec![root.clone()];
    while let Some(directory) = directories.pop() {
        let Ok(entries) = std::fs::read_dir(directory.to_path_buf()) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = pytext::os_bytes(&name);
            let path = directory.join(name);
            if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                directories.push(path.clone());
            }
            if name.starts_with(b"rollout-") && name.ends_with(b".jsonl") && name.len() >= 14 {
                found.push(path);
            }
        }
    }
    let key = |path: &PyPath| -> Vec<Vec<u32>> {
        path.as_bytes()
            .split(|byte| *byte == b'/')
            .map(pytext::surrogateescape)
            .collect()
    };
    found.sort_by_key(key);
    found
}

/// The child rollout's final `overall_correctness`, the last one found in path order.
pub fn codex_overall(sessions_dir: Option<&[u8]>) -> Result<Option<String>, String> {
    let Some(sessions_dir) = sessions_dir.filter(|directory| !directory.is_empty()) else {
        return Ok(None);
    };
    let mut best = None;
    for path in rollouts(&PyPath::new(sessions_dir)) {
        let content = pytext::read_text(&path).map_err(|error| match error {
            pytext::ReadError::Os(message) => message,
            pytext::ReadError::Decode(message) => format!("{}: {message}", path.display()),
        })?;
        for line in content.split_inclusive('\n') {
            if line.contains("overall_correctness")
                && let Some(value) = overall_values(line).last()
            {
                best = Some((*value).to_string());
            }
        }
    }
    Ok(best)
}

pub fn codex_marker(stripped: &str) -> bool {
    matches!(
        stripped,
        "Review comment:" | "Review comments:" | "Full review comments:"
    )
}

pub struct Bullet<'a> {
    pub priority: &'a str,
    pub title: &'a str,
    pub path: &'a str,
    pub start: &'a str,
    pub end: Option<&'a str>,
}

/// `^- \[(P\d)\] (.+?) — (\S+?):(\d+)(?:-(\d+))?\s*$`.
pub fn codex_bullet(line: &str) -> Option<Bullet<'_>> {
    let digit = line.strip_prefix("- [P")?.chars().next()?;
    pytext::decimal_value(digit)?;
    let priority_end = 4 + digit.len_utf8();
    let body = line[priority_end..].strip_prefix("] ")?;
    let separator = " \u{2014} ";
    let mut from = body.chars().next()?.len_utf8();
    while let Some(found) = body.get(from..).and_then(|rest| rest.find(separator)) {
        let at = from + found;
        if let Some((path, start, end)) = location(&body[at + separator.len()..]) {
            return Some(Bullet {
                priority: &line[3..priority_end],
                title: &body[..at],
                path,
                start,
                end,
            });
        }
        from = at + 1;
    }
    None
}

/// `(\S+?):(\d+)(?:-(\d+))?\s*$`.
fn location(text: &str) -> Option<(&str, &str, Option<&str>)> {
    let core = text.trim_end_matches(pytext::is_space);
    if core.chars().any(pytext::is_space) {
        return None;
    }
    let digits =
        |text: &str| !text.is_empty() && text.chars().all(|c| pytext::decimal_value(c).is_some());
    for (colon, _) in core.match_indices(':').filter(|(colon, _)| *colon > 0) {
        let suffix = &core[colon + 1..];
        let (start, end) = match suffix.split_once('-') {
            Some((start, end)) => (start, Some(end)),
            None => (suffix, None),
        };
        if digits(start) && end.is_none_or(digits) {
            return Some((&core[..colon], start, end));
        }
    }
    None
}

/// `int(digits)` for the digits `\d` matched.
fn integer(digits: &str) -> Value {
    let ascii: String = digits
        .chars()
        .filter_map(pytext::decimal_value)
        .filter_map(|value| char::from_digit(value, 10))
        .collect();
    let trimmed = ascii.trim_start_matches('0');
    Value::Int(if trimmed.is_empty() { "0" } else { trimmed }.to_string())
}

fn first_characters(text: &str, count: usize) -> &str {
    text.char_indices()
        .nth(count)
        .map_or(text, |(index, _)| &text[..index])
}

pub fn from_codex(
    stdout: &str,
    clone: Option<&[u8]>,
    sessions_dir: Option<&[u8]>,
) -> Result<Normalized, String> {
    let mut items: Vec<Item> = Vec::new();
    let mut notes = Vec::new();
    let lines = pytext::splitlines(stdout);
    let mut summary = Vec::new();
    let mut index = 0;
    while index < lines.len() && !codex_marker(pytext::strip(lines[index])) {
        let stripped = pytext::strip(lines[index]);
        if !stripped.is_empty() {
            summary.push(stripped);
        }
        index += 1;
    }
    let has_marker = index < lines.len();
    let mut current: Option<usize> = None;
    let mut raw = false;
    for line in lines.iter().skip(index + 1) {
        let stripped = pytext::strip(line);
        if let Some(bullet) = codex_bullet(line) {
            let start = integer(bullet.start);
            let end = bullet.end.map_or_else(|| start.clone(), integer);
            items.push(Item {
                file: relpath(text(bullet.path), clone, "a bullet's path")?,
                line_start: start,
                line_end: end,
                claim: text(pytext::strip(bullet.title)),
                consequence: text(""),
                native_priority: text(bullet.priority),
                ..Item::default()
            });
            current = Some(items.len() - 1);
        } else if stripped.is_empty() {
        } else if let Some(position) =
            current.filter(|_| line.chars().next().is_some_and(pytext::is_space))
        {
            let consequence = items[position].consequence.py_str();
            items[position].consequence = text(pytext::strip(&format!("{consequence} {stripped}")));
        } else {
            notes.push(format!(
                "unresolved: line did not match the review-comment form: {}",
                first_characters(stripped, 120)
            ));
            items.push(Item {
                claim: text(stripped),
                consequence: text(""),
                ..Item::default()
            });
            current = Some(items.len() - 1);
            raw = true;
        }
    }
    let rollout = codex_overall(sessions_dir)?;
    let from_rollout = rollout.is_some();
    let overall = rollout.or_else(|| Some(summary.join(" ")).filter(|joined| !joined.is_empty()));
    let source = if from_rollout {
        Some("rollout overall_correctness")
    } else if overall.is_some() {
        Some("stdout summary")
    } else {
        None
    };
    let bullets = lines.iter().any(|line| codex_bullet(line).is_some());
    let parse_status = if !has_marker
        && !bullets
        && from_rollout
        && overall.as_deref() == Some("patch is correct")
    {
        "empty"
    } else if !has_marker && items.is_empty() {
        notes.push(
            "unresolved: no review-comments marker and no bullets; stdout kept raw".to_string(),
        );
        items.push(Item {
            claim: text(pytext::strip(stdout)),
            ..Item::default()
        });
        "unresolved"
    } else if raw {
        "unresolved"
    } else if items.is_empty() {
        "empty"
    } else {
        "parsed"
    };
    Ok(Normalized {
        parse_status,
        native_verdict: overall.map_or(Value::Null, Value::Str),
        verdict_source: source.map_or(Value::Null, text),
        items,
        parse_notes: notes,
        native_payload: None,
    })
}

/// Every item rendered uniformly for blind scoring, as `print(render(doc))` prints it, without the final
/// newline `print` adds.
pub fn render(doc: &Value) -> Result<String, String> {
    let items = doc
        .as_object()
        .and_then(|doc| doc.get("items"))
        .ok_or("the document has no items")?;
    let items = objects(items, "items")?;
    let mut out = Vec::new();
    for (number, item) in items.iter().enumerate() {
        let field = |key: &str| {
            item.get(key)
                .ok_or_else(|| format!("item {} has no {}", number + 1, pytext::repr_str(key)))
        };
        let (file, start) = (field("file")?, field("line_start")?);
        let mut location = if file.truthy() {
            file.clone()
        } else {
            text("(no file)")
        };
        if start.truthy() {
            let end = field("line_end")?;
            let mut suffix = format!(":{}", start.py_str());
            if end.truthy() && !end.py_eq(start) {
                suffix.push_str(&format!("-{}", end.py_str()));
            }
            location = match location {
                Value::Str(path) => Value::Str(path + &suffix),
                // `list += str` extends the list with the text's characters.
                Value::Array(mut values) => {
                    values.extend(suffix.chars().map(|c| Value::Str(c.to_string())));
                    Value::Array(values)
                }
                _ => return Err(format!("item {} file is not text", number + 1)),
            };
        }
        let or_dash = |value: &Value| {
            if value.truthy() {
                value.py_str()
            } else {
                "\u{2014}".to_string()
            }
        };
        out.push(format!(
            "### Item {}\nLocation: {}\nClaim: {}\nConsequence: {}\nFix: {}\n",
            number + 1,
            location.py_str(),
            field("claim")?.py_str(),
            or_dash(field("consequence")?),
            or_dash(field("proposed_fix")?)
        ));
    }
    if items.is_empty() {
        out.push("(no items)\n".to_string());
    }
    Ok(out.join("\n"))
}

#[derive(Clone, Debug, Default)]
pub struct Arguments {
    pub arm: Option<Arm>,
    pub composition: Option<Vec<u8>>,
    pub payload: Option<Vec<u8>>,
    pub stdout: Option<Vec<u8>>,
    pub native_review: Option<Vec<u8>>,
    pub sessions_dir: Option<Vec<u8>>,
    pub clone: Option<Vec<u8>>,
    pub out: Option<Vec<u8>>,
    pub render: Option<Vec<u8>>,
    pub timing: Option<Vec<u8>>,
}

/// What the command prints and its exit code. Python writes stderr as it goes and stdout when it exits, so
/// a caller that sends both to one file writes stderr first.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Outcome {
    pub code: u8,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

impl Outcome {
    fn failed(mut self, message: &str) -> Self {
        self.stderr
            .extend_from_slice(format!("{PROGRAM}: {message}\n").as_bytes());
        self.code = 2;
        self
    }
}

fn read_input(path: &[u8]) -> Result<String, String> {
    pytext::read_file_text(path).map_err(|error| match error {
        pytext::ReadError::Os(message) => message,
        pytext::ReadError::Decode(message) => {
            format!("{}: {message}", pytext::stderr_text(path))
        }
    })
}

fn read_json(path: &[u8]) -> Result<Value, String> {
    pyjson::loads(&read_input(path)?, Duplicates::KeepLast).map_err(|error| error.to_string())
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

/// The normalizer for an arm whose input the arguments name; the caller has checked that it is given.
fn normalize(arguments: &Arguments, arm: Arm) -> Result<Normalized, String> {
    let clone = arguments.clone.as_deref();
    let input = |path: &Option<Vec<u8>>| path.as_deref().unwrap_or_default().to_vec();
    match arm {
        Arm::ReviewCode => from_review_code(&read_json(&input(&arguments.composition))?, clone),
        Arm::ClaudeBuiltin => from_claude_builtin(&read_json(&input(&arguments.payload))?),
        Arm::Codex => from_codex(
            &read_input(&input(&arguments.stdout))?,
            clone,
            arguments.sessions_dir.as_deref(),
        ),
        Arm::CodexSkill | Arm::ClaudeSkill => {
            from_codex_skill(&read_json(&input(&arguments.native_review))?, clone)
        }
    }
}

/// Run the command; `now` gives the UTC instant `--timing` stamps.
pub fn command(arguments: &Arguments, now: &dyn Fn() -> String) -> Outcome {
    let outcome = Outcome::default();
    let given = |value: &Option<Vec<u8>>| value.clone().filter(|value| !value.is_empty());
    if let Some(path) = given(&arguments.render) {
        return match read_json(&path).and_then(|doc| render(&doc)) {
            Ok(rendered) => Outcome {
                stdout: format!("{rendered}\n").into_bytes(),
                ..outcome
            },
            Err(message) => outcome.failed(&message),
        };
    }
    let Some(arm) = arguments.arm else {
        return outcome.failed("--arm is required");
    };
    let normalized = match normalize(arguments, arm) {
        Ok(normalized) => normalized,
        Err(message) => return outcome.failed(&message),
    };
    let unresolved = normalized.parse_status == "unresolved";
    let document = indented(&normalized.into_value(arm.name()));
    let mut outcome = outcome;
    match given(&arguments.out) {
        Some(path) => {
            if let Err(message) = pytext::write_file_text(&path, &format!("{document}\n")) {
                return outcome.failed(&message);
            }
        }
        None => outcome
            .stdout
            .extend_from_slice(format!("{document}\n").as_bytes()),
    }
    if unresolved {
        outcome.stdout.extend_from_slice(UNRESOLVED.as_bytes());
        outcome.code = 1;
        return outcome;
    }
    if let Some(path) = given(&arguments.timing) {
        let stamped = read_json(&path).and_then(|sidecar| {
            let Value::Object(mut sidecar) = sidecar else {
                return Err(format!(
                    "{}: the timing sidecar is not an object",
                    pytext::stderr_text(&path)
                ));
            };
            sidecar.insert("payload_validated_at".to_string(), Value::Str(now()));
            pytext::write_file_text(&path, &format!("{}\n", indented(&Value::Object(sidecar))))
        });
        if let Err(message) = stamped {
            return outcome.failed(&message);
        }
    }
    outcome
}

/// `datetime.now(timezone.utc).isoformat(timespec="seconds")` with `Z` for `+00:00`.
pub fn utc_now() -> String {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs());
    utc_stamp(seconds)
}

/// The UTC civil time of a count of seconds since 1970, as `YYYY-MM-DDTHH:MM:SSZ`.
pub fn utc_stamp(seconds: u64) -> String {
    let days = seconds / 86_400;
    let rest = seconds % 86_400;
    // Days to civil date, after Howard Hinnant's `civil_from_days`, for days on or after 1970-01-01.
    let shifted = days + 719_468;
    let era = shifted / 146_097;
    let day_of_era = shifted % 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year = year_of_era + era * 400 + u64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rest / 3_600,
        rest % 3_600 / 60,
        rest % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Scratch(std::path::PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let path =
                std::env::temp_dir().join(format!("bench-tools-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn file(&self, name: &str, content: &str) -> Vec<u8> {
            let path = self.0.join(name);
            std::fs::write(&path, content).unwrap();
            path.into_os_string().into_encoded_bytes()
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn timing_rewrite_stamps_the_clock_in_place() {
        let scratch = Scratch::new("timing");
        let payload = scratch.file(
            "payload.json",
            "{\"final_text\": \"```json\\n[]\\n```\", \"report_findings\": []}",
        );
        let timing = scratch.file(
            "timing.json",
            "{\"completion_mode\": \"r\u{e9}nder\", \"root_dispatched_at\": \"2026-01-01T00:00:00Z\", \"payload_validated_at\": null, \"completed_at\": null}",
        );
        let out = scratch
            .0
            .join("normalized.json")
            .into_os_string()
            .into_encoded_bytes();
        let arguments = Arguments {
            arm: Some(Arm::ClaudeBuiltin),
            payload: Some(payload),
            out: Some(out),
            timing: Some(timing.clone()),
            ..Arguments::default()
        };
        let outcome = command(&arguments, &|| "2026-10-18T12:34:56Z".to_string());
        assert_eq!(outcome, Outcome::default());
        assert_eq!(
            std::fs::read_to_string(String::from_utf8(timing.clone()).unwrap()).unwrap(),
            "{\n  \"completion_mode\": \"r\\u00e9nder\",\n  \"root_dispatched_at\": \"2026-01-01T00:00:00Z\",\n  \"payload_validated_at\": \"2026-10-18T12:34:56Z\",\n  \"completed_at\": null\n}\n"
        );
        let appended = scratch.file("appended.json", "{\"a\": 1}");
        let arguments = Arguments {
            timing: Some(appended.clone()),
            ..arguments
        };
        command(&arguments, &|| "2026-10-18T00:00:00Z".to_string());
        assert_eq!(
            std::fs::read_to_string(String::from_utf8(appended).unwrap()).unwrap(),
            "{\n  \"a\": 1,\n  \"payload_validated_at\": \"2026-10-18T00:00:00Z\"\n}\n"
        );
    }

    #[test]
    fn unresolved_results_leave_the_sidecar_alone() {
        let scratch = Scratch::new("unresolved");
        let payload = scratch.file("payload.json", "{\"final_text\": \"prose\"}");
        let timing = scratch.file("timing.json", "{\"payload_validated_at\": null}");
        let outcome = command(
            &Arguments {
                arm: Some(Arm::ClaudeBuiltin),
                payload: Some(payload),
                timing: Some(timing.clone()),
                ..Arguments::default()
            },
            &|| unreachable!("an unresolved result reads no clock"),
        );
        assert_eq!(outcome.code, 1);
        assert!(outcome.stdout.ends_with(UNRESOLVED.as_bytes()));
        assert_eq!(
            std::fs::read_to_string(String::from_utf8(timing).unwrap()).unwrap(),
            "{\"payload_validated_at\": null}"
        );
    }

    #[test]
    fn stamps_are_utc_civil_time() {
        assert_eq!(utc_stamp(0), "1970-01-01T00:00:00Z");
        assert_eq!(utc_stamp(951_782_400), "2000-02-29T00:00:00Z");
        assert_eq!(utc_stamp(1_792_326_896), "2026-10-18T12:34:56Z");
        assert_eq!(utc_stamp(4_107_542_399), "2100-02-28T23:59:59Z");
        assert_eq!(utc_stamp(4_107_542_400), "2100-03-01T00:00:00Z");
    }

    #[test]
    fn matchers_follow_the_python_patterns() {
        assert_eq!(fenced_array("x ```json\n [1] y ]  \n```"), Some("[1] y ]"));
        assert_eq!(fenced_array("````[]```"), Some("[]"));
        assert_eq!(fenced_array("```jsonx [] ```"), None);
        let bullet = codex_bullet("- [P2] A \u{2014} b \u{2014} c:d:12-034  ").unwrap();
        assert_eq!(
            (
                bullet.priority,
                bullet.title,
                bullet.path,
                bullet.start,
                bullet.end
            ),
            ("P2", "A \u{2014} b", "c:d", "12", Some("034"))
        );
        assert!(codex_bullet("- [P2] A \u{2014} a b:1").is_none());
        assert!(codex_bullet("- [P2]  \u{2014} a:1").is_none());
        assert_eq!(
            codex_bullet("- [P2]   \u{2014} a:1").map(|bullet| bullet.title),
            Some(" ")
        );
        let bullet = codex_bullet("- [P\u{661}] A \u{2014} a:\u{661}\u{662}-\u{1d7ce}").unwrap();
        assert_eq!(
            (bullet.priority, bullet.start, bullet.end),
            ("P\u{661}", "\u{661}\u{662}", Some("\u{1d7ce}"))
        );
        assert!(matches!(integer(bullet.start), Value::Int(digits) if digits == "12"));
        assert!(codex_bullet("- [P\u{b2}] A \u{2014} a:1").is_none());
        assert!(codex_bullet("- [P2] A \u{2014} a:\u{b2}").is_none());
        assert_eq!(
            overall_values("\"overall_correctness\": \"x\" \\\"overall_correctness\\\":\\\"y\\\""),
            ["x", "y"]
        );
    }
}
