//! Comparing outputs that carry blind tokens, shuffles, clocks or runner identity.
//!
//! Every pin the case declares is checked against the bytes it names before anything is normalized. Then only
//! the declared normalizations apply: with mask_root, the case root in file contents becomes <root>; clock fields
//! and runner identity fields take fixed labels, each run of dossier sections is ordered by content, and each token
//! the run wrote at a declared site becomes its first-seen ordinal at that site and at the rule's declared
//! references, scanning stdout, stderr, then entries in path order, names before contents. A rule with
//! ordinals: sites numbers its tokens first in the order its sites hold them, sites in declared order, files in
//! path order, pointers in declared order and keys in written order, for tokens that every other place holds in
//! an order set by their random values. Each run of adjacent lines a lines rule names is then ordered by its
//! relabeled bytes. A reference is a JSON string value or object's keys at a pointer, entry names,
//! a stream, or a whole file; a whole-file reference must never name a file that carries quotes, source text or
//! evidence, so those files declare pointer references. Every other byte stays exact, occurrences of site tokens
//! included. A generated token that the case's inputs already hold, raw or as a decoded JSON string, and text
//! already in a label's form are inconsistencies, since either could make a changed literal look like a reference.
//! Last, every pin is recomputed over the normalized bytes it names, so the trees compare byte for byte.

use crate::corpus::{field, has, keys, list, string};
use crate::normalize::{self, Kind, Observed, matches, sorted};
use crate::{b64, json, pattern};
use bench_tools::pyjson::{self, Duplicates, Object, Value};
use bench_tools::{pyfs, sha256};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::Path;

type Files = HashMap<Vec<u8>, Vec<u8>>;

fn shown(name: &[u8]) -> String {
    String::from_utf8_lossy(name).into_owned()
}

fn strings<'a>(value: &'a Value, key: &str) -> Result<Vec<&'a str>, String> {
    list(value, key)?
        .iter()
        .map(|item| {
            item.as_str()
                .ok_or_else(|| format!("{key} holds a non-string"))
        })
        .collect()
}

struct Pin {
    record: Vec<u8>,
    at: String,
    sha: Value,
    target: Vec<u8>,
}

fn pin_targets(
    rule: &Value,
    name: &[u8],
    raw: &[u8],
) -> Result<Vec<(String, Value, Vec<u8>)>, String> {
    let value = json::value(raw)?;
    let mut found: Vec<(String, Value, Value)> = Vec::new();
    if has(rule, "pointer") {
        let at = string(rule, "pointer")?;
        found.push((
            at.to_string(),
            json::pointer(&value, at)?.clone(),
            field(rule, "path")?.clone(),
        ));
    } else if has(rule, "map") {
        let at = string(rule, "map")?;
        let members = json::pointer(&value, at)?
            .as_object()
            .ok_or_else(|| format!("{at} is not an object of pins"))?;
        for (key, sha) in members.iter() {
            found.push((
                format!("{at}/{}", json::escape(key)),
                sha.clone(),
                json::text(key),
            ));
        }
    } else {
        fn walk(item: &Value, path: &str, found: &mut Vec<(String, Value, Value)>) {
            match item {
                Value::Object(object) => {
                    if let (Some(Value::Str(target)), Some(Value::Str(sha))) =
                        (object.get("path"), object.get("sha256"))
                    {
                        found.push((
                            format!("{path}/sha256"),
                            json::text(sha),
                            json::text(target),
                        ));
                    }
                    for (key, child) in object.iter() {
                        walk(child, &format!("{path}/{}", json::escape(key)), found);
                    }
                }
                Value::Array(items) => {
                    for (index, child) in items.iter().enumerate() {
                        walk(child, &format!("{path}/{index}"), found);
                    }
                }
                _ => {}
            }
        }
        walk(&value, "", &mut found);
    }
    let base: Vec<u8> = match rule.as_object().and_then(|object| object.get("base")) {
        Some(Value::Str(base)) if base == "record" => name
            .iter()
            .rposition(|byte| *byte == b'/')
            .map_or(Vec::new(), |at| name[..at].to_vec()),
        Some(Value::Str(base)) => base.as_bytes().to_vec(),
        None => Vec::new(),
        Some(other) => return Err(format!("a pin base is a string, not {}", other.py_repr())),
    };
    let mut result = Vec::new();
    for (at, sha, path) in found {
        let outside = || {
            format!(
                "{at} names {}, which is outside the case root",
                path.py_repr()
            )
        };
        let Some(text) = path.as_str() else {
            return Err(outside());
        };
        let joined = if text.starts_with('/') || base.is_empty() {
            text.as_bytes().to_vec()
        } else if base.ends_with(b"/") {
            [&base, text.as_bytes()].concat()
        } else {
            [&base, b"/" as &[u8], text.as_bytes()].concat()
        };
        let joined = pyfs::normpath(&joined);
        if text.starts_with('/') || joined == b".." || joined.starts_with(b"../") {
            return Err(outside());
        }
        result.push((at, sha, joined));
    }
    Ok(result)
}

fn pins_of(spec: &Value, files: &Files) -> Result<(Vec<Pin>, Vec<String>), String> {
    let (mut pins, mut issues) = (Vec::new(), Vec::new());
    for rule in list(spec, "pins")? {
        for name in sorted(files.keys()) {
            if matches(&name, string(rule, "glob")?)? {
                match pin_targets(rule, &name, &files[&name]) {
                    Ok(found) => pins.extend(found.into_iter().map(|(at, sha, target)| Pin {
                        record: name.clone(),
                        at,
                        sha,
                        target,
                    })),
                    Err(error) => {
                        issues.push(format!("{}: unreadable pins: {error}", shown(&name)))
                    }
                }
            }
        }
    }
    Ok((pins, issues))
}

fn replace_pointers(
    name: &[u8],
    data: &[u8],
    values: &HashMap<String, Value>,
) -> Result<Vec<u8>, String> {
    std::str::from_utf8(data).map_err(|error| error.to_string())?;
    let wanted: HashSet<String> = values.keys().cloned().collect();
    let spans = json::spans(data, &wanted, &HashSet::new())?.values;
    let mut missing: Vec<&String> = wanted
        .iter()
        .filter(|at| !spans.contains_key(*at))
        .collect();
    if !missing.is_empty() {
        missing.sort();
        return Err(format!("{}: missing fields {missing:?}", shown(name)));
    }
    let mut found: Vec<(&String, &(usize, usize))> = spans.iter().collect();
    found.sort_by_key(|(_, (start, _))| std::cmp::Reverse(*start));
    let mut text = data.to_vec();
    for (at, (start, end)) in found {
        text.splice(*start..*end, json::dumps(&values[at]).into_bytes());
    }
    Ok(text)
}

/// `bytes.splitlines(keepends=True)`.
fn split_lines(data: &[u8]) -> Vec<&[u8]> {
    let mut lines = Vec::new();
    let mut start = 0;
    let mut at = 0;
    while at < data.len() {
        match data[at] {
            b'\n' => {
                lines.push(&data[start..=at]);
                start = at + 1;
            }
            b'\r' => {
                let end = if data.get(at + 1) == Some(&b'\n') {
                    at + 1
                } else {
                    at
                };
                lines.push(&data[start..=end]);
                at = end;
                start = end + 1;
            }
            _ => {}
        }
        at += 1;
    }
    if start < data.len() {
        lines.push(&data[start..]);
    }
    lines
}

fn trim_ending(line: &[u8]) -> &[u8] {
    let kept = line.len()
        - line
            .iter()
            .rev()
            .take_while(|byte| matches!(byte, b'\r' | b'\n'))
            .count();
    &line[..kept]
}

fn flush_sections<'a>(run: &mut Vec<Vec<&'a [u8]>>, result: &mut Vec<&'a [u8]>) {
    run.sort_by_cached_key(|section| {
        let joined = section.concat();
        (
            pattern::replace_tokens(&joined, None, |_| b"<token>".to_vec()),
            joined,
        )
    });
    result.extend(run.drain(..).flatten());
}

/// Sorts each run of adjacent sections under one heading level by their bytes with tokens masked.
fn order_sections(data: &[u8], heading: &str) -> Vec<u8> {
    let level = heading.len() - 1;
    let mut result: Vec<&[u8]> = Vec::new();
    let mut run: Vec<Vec<&[u8]>> = Vec::new();
    let mut open = false;
    for line in split_lines(data) {
        let depth = line.iter().take_while(|byte| **byte == b'#').count();
        if line.starts_with(heading.as_bytes()) {
            run.push(vec![line]);
            open = true;
        } else if open && depth > 0 && depth <= level && line.get(depth) == Some(&b' ') {
            open = false;
            flush_sections(&mut run, &mut result);
            result.push(line);
        } else if open {
            if let Some(section) = run.last_mut() {
                section.push(line);
            }
        } else {
            result.push(line);
        }
    }
    flush_sections(&mut run, &mut result);
    result.concat()
}

/// Sorts each run of adjacent lines that begin with `start` by their bytes, keeping each line ending in place.
fn order_lines(data: &[u8], start: &str) -> Vec<u8> {
    let mut result: Vec<u8> = Vec::new();
    let mut run: Vec<&[u8]> = Vec::new();
    let flush = |run: &mut Vec<&[u8]>, result: &mut Vec<u8>| {
        let mut bodies: Vec<&[u8]> = run.iter().map(|line| trim_ending(line)).collect();
        bodies.sort();
        for (body, line) in bodies.into_iter().zip(run.drain(..)) {
            result.extend_from_slice(body);
            result.extend_from_slice(&line[trim_ending(line).len()..]);
        }
    };
    for line in split_lines(data) {
        if line.starts_with(start.as_bytes()) {
            run.push(line);
        } else {
            flush(&mut run, &mut result);
            result.extend_from_slice(line);
        }
    }
    flush(&mut run, &mut result);
    result
}

fn sorted_object(object: &Object) -> Value {
    let mut members: Vec<(&String, &Value)> = object.iter().collect();
    members.sort_by(|left, right| left.0.cmp(right.0));
    let mut ordered = Object::default();
    for (key, value) in members {
        ordered.insert(key.clone(), value.clone());
    }
    Value::Object(ordered)
}

/// Orders the members of the objects at the pointers by key, in a file written as `json.dumps(value, indent=2)`
/// and a newline, so an object built in a shuffled order compares by its members. Any other form is refused.
fn order_members(data: &[u8], pointers: &[&str]) -> Result<Vec<u8>, String> {
    let mut value = json::value(data)?;
    if data != format!("{}\n", json::dumps_indented(&value)).as_bytes() {
        return Err("not written as json.dumps(value, indent=2) and a newline".to_string());
    }
    for at in pointers {
        let members = json::pointer_mut(&mut value, at)?;
        let Value::Object(object) = members else {
            return Err(format!("{at} is not an object"));
        };
        *members = sorted_object(object);
    }
    Ok(format!("{}\n", json::dumps_indented(&value)).into_bytes())
}

/// Every input the case gives its commands, raw and as each string a JSON input decodes to: names and bytes it
/// writes, arguments and stdin.
fn case_inputs(case: &Value) -> Result<Vec<Vec<u8>>, String> {
    let mut blobs = Vec::new();
    let mut steps = vec![case];
    steps.extend(list(case, "steps")?);
    for step in steps {
        for entry in list(step, "initial")?.iter().chain(list(step, "write")?) {
            for key in ["path_b64", "bytes_b64", "target_b64"] {
                if has(entry, key) {
                    blobs.push(b64::decode(string(entry, key)?)?);
                }
            }
        }
        for arg in list(step, "argv")? {
            blobs.push(match arg {
                Value::Object(_) => b64::decode(string(arg, "bytes_b64")?)?,
                other => other.os_bytes().unwrap_or_default().to_vec(),
            });
        }
        blobs.push(
            match step.as_object().and_then(|object| object.get("stdin_b64")) {
                Some(encoded) => b64::decode(encoded.as_str().ok_or("stdin_b64 is a string")?)?,
                None => Vec::new(),
            },
        );
    }
    fn walk(value: &Value, out: &mut Vec<Vec<u8>>) {
        match value {
            Value::Str(_) | Value::OsText(_) => {
                out.push(value.os_bytes().unwrap_or_default().to_vec())
            }
            Value::Object(object) => {
                for (key, child) in object.members() {
                    out.push(key.os_bytes().to_vec());
                    walk(child, out);
                }
            }
            Value::Array(items) => items.iter().for_each(|child| walk(child, out)),
            _ => {}
        }
    }
    let mut strings = Vec::new();
    for blob in &blobs {
        if let Ok(text) = std::str::from_utf8(blob)
            && let Ok(value) = pyjson::loads_os_text(text, Duplicates::KeepLast)
        {
            walk(&value, &mut strings);
        }
    }
    blobs.extend(strings);
    Ok(blobs)
}

#[derive(Clone, PartialEq)]
enum At {
    Value(String),
    Keys(String),
}

/// Numbers each generated token by its first sighting, per prefix.
struct Ordinals {
    generated: BTreeSet<Vec<u8>>,
    labels: HashMap<Vec<u8>, Vec<u8>>,
    counts: HashMap<String, usize>,
}

impl Ordinals {
    fn number(&mut self, prefix: &str, token: &[u8]) -> Vec<u8> {
        if let Some(label) = self.labels.get(token) {
            return label.clone();
        }
        let count = self.counts.entry(prefix.to_string()).or_default();
        *count += 1;
        let label = format!("<{prefix} {count}>").into_bytes();
        self.labels.insert(token.to_vec(), label.clone());
        label
    }

    fn label(&mut self, prefix: &str, token: &[u8]) -> Vec<u8> {
        if self.generated.contains(token) {
            self.number(prefix, token)
        } else {
            token.to_vec()
        }
    }

    fn relabel(&mut self, prefix: &str, data: &[u8]) -> Vec<u8> {
        pattern::replace_tokens(data, Some(&[prefix]), |token| self.label(prefix, token))
    }

    /// Relabels only a token written right after one of the line starts, such as the `### ` of a dossier heading,
    /// leaving the rest of each line, which may quote source text, exact.
    fn relabel_line_starts(&mut self, prefix: &str, data: &[u8], starts: &[&str]) -> Vec<u8> {
        let mut out = Vec::with_capacity(data.len());
        for line in split_lines(data) {
            let mut written = line.to_vec();
            if let Some(start) = starts
                .iter()
                .find(|start| line.starts_with(start.as_bytes()))
            {
                let at = start.len();
                if let Some(end) = pattern::token_at(line, at, Some(&[prefix])) {
                    written = [
                        &line[..at],
                        &self.label(prefix, &line[at..end]),
                        &line[end..],
                    ]
                    .concat();
                }
            }
            out.extend(written);
        }
        out
    }

    /// Relabels within the raw JSON strings at the value pointers and within the keys of the objects at the key
    /// pointers, leaving every other byte as written.
    fn relabel_json(
        &mut self,
        prefix: &str,
        data: &[u8],
        values: &[String],
        keys: &[String],
    ) -> Result<Vec<u8>, String> {
        json::value(data)?;
        let pointers: HashSet<String> = values.iter().chain(keys).cloned().collect();
        let objects: HashSet<String> = keys.iter().cloned().collect();
        let spans = json::spans(data, &pointers, &objects)?;
        let mut missing: Vec<&String> = pointers
            .iter()
            .filter(|at| !spans.values.contains_key(*at))
            .collect();
        if !missing.is_empty() {
            missing.sort();
            return Err(format!("missing fields {missing:?}"));
        }
        for at in values {
            if data[spans.values[at].0] != b'"' {
                return Err(format!("{at} is not a string"));
            }
        }
        for at in keys {
            if data[spans.values[at].0] != b'{' {
                return Err(format!("{at} is not an object"));
            }
        }
        let regions: BTreeSet<(usize, usize)> = values
            .iter()
            .map(|at| spans.values[at])
            .chain(spans.keys.values().copied())
            .collect();
        let relabeled: Vec<(usize, usize, Vec<u8>)> = regions
            .into_iter()
            .map(|(start, end)| (start, end, self.relabel(prefix, &data[start..end])))
            .collect();
        let mut text = data.to_vec();
        for (start, end, replacement) in relabeled.into_iter().rev() {
            text.splice(start..end, replacement);
        }
        Ok(text)
    }
}

fn token_value(files: &Files, name: &[u8], at: &str) -> Result<Value, String> {
    let value = json::value(&files[name])?;
    json::pointer(&value, at).cloned()
}

/// References of one rule whose fields, apart from `keys`, are exactly `glob` and `fields`, and whose glob
/// matches `name`.
fn declared<'a>(
    references: &'a [Value],
    name: &[u8],
    fields: &[&str],
) -> Result<Vec<&'a Value>, String> {
    let mut wanted: HashSet<&str> = fields.iter().copied().collect();
    wanted.insert("glob");
    let mut found = Vec::new();
    for reference in references {
        let mut have = keys(reference);
        have.remove("keys");
        if have == wanted && matches(name, string(reference, "glob")?)? {
            found.push(reference);
        }
    }
    Ok(found)
}

fn wants_keys(value: &Value) -> bool {
    value
        .as_object()
        .and_then(|object| object.get("keys"))
        .is_some_and(Value::truthy)
}

#[allow(clippy::too_many_lines)]
fn tokens(
    case: &Value,
    rules: &[Value],
    names: &mut [Vec<u8>],
    files: &mut Files,
    streams: &mut [Vec<u8>; 2],
    issues: &mut Vec<String>,
) -> Result<(), String> {
    let prefixes: Vec<&str> = rules
        .iter()
        .map(|rule| string(rule, "prefix"))
        .collect::<Result<_, _>>()?;
    let mut generated = BTreeSet::new();
    let mut found: Vec<(String, Vec<u8>, At)> = Vec::new();
    let mut claimed: BTreeMap<(Vec<u8>, String), BTreeSet<String>> = BTreeMap::new();
    let mut unclaimed: BTreeMap<(Vec<u8>, String), BTreeSet<String>> = BTreeMap::new();
    for rule in rules {
        let prefix = string(rule, "prefix")?;
        for site in list(rule, "sites")? {
            let glob = string(site, "glob")?;
            let mut held = Vec::new();
            for name in sorted(files.keys()) {
                if matches(&name, glob)? {
                    held.push(name);
                }
            }
            if held.is_empty() {
                issues.push(format!(
                    "{glob}: no file holds the declared {prefix} tokens"
                ));
            }
            for name in held {
                for at in strings(site, "pointers")? {
                    let value = match token_value(files, &name, at) {
                        Ok(value) => value,
                        Err(error) => {
                            issues.push(format!(
                                "{}: {at} should hold a {prefix} token: {error}",
                                shown(&name)
                            ));
                            continue;
                        }
                    };
                    if wants_keys(site) {
                        let Some(object) = value.as_object() else {
                            issues.push(format!(
                                "{}: {at} holds {}, not an object of {prefix} tokens",
                                shown(&name),
                                json::dumps(&value)
                            ));
                            continue;
                        };
                        let place = (name.clone(), at.to_string());
                        let claimed = claimed.entry(place.clone()).or_default();
                        let unclaimed = unclaimed.entry(place).or_default();
                        for (key, _) in object.iter() {
                            unclaimed.insert(key.clone());
                            if !key.starts_with(&format!("{prefix}-")) {
                                continue;
                            }
                            claimed.insert(key.clone());
                            if pattern::is_token(key, prefix) {
                                generated.insert(key.as_bytes().to_vec());
                                found.push((
                                    prefix.to_string(),
                                    name.clone(),
                                    At::Keys(at.to_string()),
                                ));
                            } else {
                                issues.push(format!(
                                    "{}: {at} has the key {}, not a {prefix} token",
                                    shown(&name),
                                    json::dumps(&json::text(key))
                                ));
                            }
                        }
                    } else if let Some(token) = value
                        .as_str()
                        .filter(|token| pattern::is_token(token, prefix))
                    {
                        generated.insert(token.as_bytes().to_vec());
                        found.push((prefix.to_string(), name.clone(), At::Value(at.to_string())));
                    } else {
                        issues.push(format!(
                            "{}: {at} holds {}, not a {prefix} token",
                            shown(&name),
                            json::dumps(&value)
                        ));
                    }
                }
            }
        }
    }
    for (place, keys) in &unclaimed {
        let none = BTreeSet::new();
        for key in keys.difference(claimed.get(place).unwrap_or(&none)) {
            issues.push(format!(
                "{}: {} has the key {}, which no token rule declares",
                shown(&place.0),
                place.1,
                json::dumps(&json::text(key))
            ));
        }
    }
    let inputs = case_inputs(case)?;
    for token in &generated {
        if inputs.iter().any(|blob| {
            blob.windows(token.len())
                .any(|window| window == token.as_slice())
        }) {
            issues.push(format!(
                "{}: the run generated a token the case's inputs already hold, so its references cannot be told from literal text",
                shown(token)
            ));
        }
    }
    let mut places: Vec<(String, &[u8])> = vec![
        ("stdout".into(), &streams[0]),
        ("stderr".into(), &streams[1]),
    ];
    places.extend(
        names
            .iter()
            .map(|name| (format!("path {}", shown(name)), name.as_slice())),
    );
    places.extend(
        names
            .iter()
            .filter_map(|name| files.get(name).map(|data| (shown(name), data.as_slice()))),
    );
    for (place, data) in places {
        let held: BTreeSet<String> = pattern::labels_held(data, &prefixes).into_iter().collect();
        if !held.is_empty() {
            issues.push(format!(
                "{place}: already holds {}, the form of a token label",
                held.into_iter().collect::<Vec<_>>().join(", ")
            ));
        }
    }
    let mut ordinals = Ordinals {
        generated,
        labels: HashMap::new(),
        counts: HashMap::new(),
    };
    for rule in rules {
        if !rule
            .as_object()
            .and_then(|object| object.get("ordinals"))
            .is_some_and(|ordinals| ordinals.as_str() == Some("sites"))
        {
            continue;
        }
        let prefix = string(rule, "prefix")?;
        for site in list(rule, "sites")? {
            for name in sorted(files.keys()) {
                if !matches(&name, string(site, "glob")?)? {
                    continue;
                }
                for at in strings(site, "pointers")? {
                    let Ok(value) = token_value(files, &name, at) else {
                        continue;
                    };
                    let held: Vec<String> = match (&value, wants_keys(site)) {
                        (Value::Object(object), true) => {
                            object.iter().map(|(key, _)| key.clone()).collect()
                        }
                        (Value::Str(token), _) => vec![token.clone()],
                        _ => Vec::new(),
                    };
                    for token in held {
                        if ordinals.generated.contains(token.as_bytes())
                            && token.starts_with(&format!("{prefix}-"))
                        {
                            ordinals.number(prefix, token.as_bytes());
                        }
                    }
                }
            }
        }
    }
    let mut renamed: Vec<Vec<u8>> = names.to_vec();
    let mut order: Vec<usize> = (0..names.len()).collect();
    order.sort_by_cached_key(|&index| {
        let name = &names[index];
        (
            pattern::replace_tokens(name, Some(&prefixes), |_| b"<token>".to_vec()),
            name.clone(),
        )
    });
    for rule in rules {
        let prefix = string(rule, "prefix")?;
        let references = list(rule, "references")?;
        for (index, stream) in ["stdout", "stderr"].into_iter().enumerate() {
            if references.iter().any(|reference| {
                keys(reference) == HashSet::from(["stream"])
                    && reference
                        .as_object()
                        .and_then(|o| o.get("stream"))
                        .and_then(Value::as_str)
                        == Some(stream)
            }) {
                streams[index] = ordinals.relabel(prefix, &streams[index]);
            }
        }
        for &index in &order {
            let name = names[index].clone();
            if !declared(references, &name, &["names"])?.is_empty() {
                renamed[index] = ordinals.relabel(prefix, &renamed[index]);
            }
            let Some(data) = files.get(&name).cloned() else {
                continue;
            };
            if !declared(references, &name, &[])?.is_empty() {
                files.insert(name, ordinals.relabel(prefix, &data));
                continue;
            }
            let mut data = data;
            for reference in declared(references, &name, &["line_starts"])? {
                data = ordinals.relabel_line_starts(
                    prefix,
                    &data,
                    &strings(reference, "line_starts")?,
                );
            }
            let at_pointers = declared(references, &name, &["pointers"])?;
            let sites: Vec<&At> = found
                .iter()
                .filter(|(owner, site, _)| owner == prefix && *site == name)
                .map(|(_, _, at)| at)
                .collect();
            let mut values: Vec<String> = sites
                .iter()
                .filter_map(|at| match at {
                    At::Value(at) => Some(at.clone()),
                    At::Keys(_) => None,
                })
                .collect();
            let mut object_keys: Vec<String> = sites
                .iter()
                .filter_map(|at| match at {
                    At::Keys(at) => Some(at.clone()),
                    At::Value(_) => None,
                })
                .collect();
            for reference in at_pointers {
                let target = if wants_keys(reference) {
                    &mut object_keys
                } else {
                    &mut values
                };
                target.extend(
                    strings(reference, "pointers")?
                        .into_iter()
                        .map(str::to_string),
                );
            }
            if !values.is_empty() || !object_keys.is_empty() {
                match ordinals.relabel_json(prefix, &data, &values, &object_keys) {
                    Ok(relabeled) => data = relabeled,
                    Err(error) => issues.push(format!(
                        "{}: declared {prefix} references unreadable: {error}",
                        shown(&name)
                    )),
                }
            }
            files.insert(name, data);
        }
    }
    let mut moved = Files::new();
    for (index, name) in names.iter_mut().enumerate() {
        if let Some(data) = files.remove(name) {
            moved.insert(renamed[index].clone(), data);
        }
        *name = renamed[index].clone();
    }
    *files = moved;
    Ok(())
}

struct Settle<'a> {
    files: &'a Files,
    repins: &'a [Pin],
    done: HashMap<Vec<u8>, Vec<u8>>,
    issues: &'a mut Vec<String>,
}

impl Settle<'_> {
    fn settle(&mut self, name: &[u8], open: &HashSet<Vec<u8>>) -> Vec<u8> {
        if let Some(done) = self.done.get(name) {
            return done.clone();
        }
        if open.contains(name) {
            self.issues
                .push(format!("{}: pins form a cycle", shown(name)));
            return self.files[name].clone();
        }
        let mut inner = open.clone();
        inner.insert(name.to_vec());
        let mut values = HashMap::new();
        for pin in self.repins {
            if pin.record == name && self.files.contains_key(&pin.target) {
                let settled = self.settle(&pin.target, &inner);
                values.insert(pin.at.clone(), json::text(&sha256::hex_digest(&settled)));
            }
        }
        let data = &self.files[name];
        let result = if values.is_empty() {
            data.clone()
        } else {
            match replace_pointers(name, data, &values) {
                Ok(result) => result,
                Err(error) => {
                    self.issues.push(format!(
                        "{}: pins unreadable after normalization: {error}",
                        shown(name)
                    ));
                    data.clone()
                }
            }
        };
        self.done.insert(name.to_vec(), result.clone());
        result
    }
}

pub fn structured(case: &Value, root: &Path, observed: Observed) -> Result<Observed, String> {
    let spec = field(case, "structured")?;
    let Observed {
        exit,
        stdout,
        stderr,
        filesystem: mut entries,
        inconsistencies: mut issues,
    } = observed;
    let mut files: Files = entries
        .iter()
        .filter_map(|entry| match &entry.kind {
            Kind::File(data) => Some((entry.path.clone(), data.clone())),
            _ => None,
        })
        .collect();
    let (pins, problems) = pins_of(spec, &files)?;
    issues.extend(problems);
    for pin in &pins {
        match files.get(&pin.target) {
            None => issues.push(format!(
                "{}: {} pins {}, which was not written",
                shown(&pin.record),
                pin.at,
                shown(&pin.target)
            )),
            Some(data) => {
                let actual = sha256::hex_digest(data);
                if !pin.sha.py_eq(&json::text(&actual)) {
                    issues.push(format!(
                        "{}: {} pins {} for {}, whose bytes hash to {actual}",
                        shown(&pin.record),
                        pin.at,
                        pin.sha.py_str(),
                        shown(&pin.target)
                    ));
                }
            }
        }
    }
    let roots = normalize::roots(root);
    for data in files.values_mut() {
        *data = normalize::mask_root(case, &roots, std::mem::take(data));
    }
    let mut labels: Vec<(&Value, HashMap<String, Value>)> = Vec::new();
    for rule in list(spec, "clocks")? {
        let values = strings(rule, "pointers")?
            .into_iter()
            .map(|at| (at.to_string(), json::text("<volatile>")))
            .collect();
        labels.push((rule, values));
    }
    for rule in list(spec, "identity")? {
        let label = field(rule, "label")?;
        let values = strings(rule, "pointers")?
            .into_iter()
            .map(|at| (at.to_string(), label.clone()))
            .collect();
        labels.push((rule, values));
    }
    for (rule, values) in &labels {
        for name in sorted(files.keys()) {
            if matches(&name, string(rule, "glob")?)? {
                let data = &files[&name];
                match json::value(data).and_then(|_| replace_pointers(&name, data, values)) {
                    Ok(replaced) => {
                        files.insert(name, replaced);
                    }
                    Err(error) => issues.push(format!(
                        "{}: declared fields unreadable: {error}",
                        shown(&name)
                    )),
                }
            }
        }
    }
    for rule in list(spec, "sections")? {
        for name in sorted(files.keys()) {
            if matches(&name, string(rule, "glob")?)? {
                let ordered = order_sections(&files[&name], string(rule, "heading")?);
                files.insert(name, ordered);
            }
        }
    }
    let mut streams = [stdout, stderr];
    let mut names: Vec<Vec<u8>> = entries.iter().map(|entry| entry.path.clone()).collect();
    let rules = list(spec, "tokens")?;
    if !rules.is_empty() {
        tokens(
            case,
            rules,
            &mut names,
            &mut files,
            &mut streams,
            &mut issues,
        )?;
        for (entry, name) in entries.iter_mut().zip(&names) {
            entry.path = name.clone();
        }
    }
    for rule in list(spec, "lines")? {
        for name in sorted(files.keys()) {
            if matches(&name, string(rule, "glob")?)? {
                let ordered = order_lines(&files[&name], string(rule, "start")?);
                files.insert(name, ordered);
            }
        }
    }
    for rule in list(spec, "members")? {
        let pointers = strings(rule, "pointers")?;
        for name in sorted(files.keys()) {
            if matches(&name, string(rule, "glob")?)? {
                match order_members(&files[&name], &pointers) {
                    Ok(ordered) => {
                        files.insert(name, ordered);
                    }
                    Err(error) => issues.push(format!(
                        "{}: declared members unreadable: {error}",
                        shown(&name)
                    )),
                }
            }
        }
    }
    let (repins, problems) = pins_of(spec, &files)?;
    issues.extend(problems);
    let mut settle = Settle {
        files: &files,
        repins: &repins,
        done: HashMap::new(),
        issues: &mut issues,
    };
    for entry in &mut entries {
        if let Kind::File(data) = &mut entry.kind {
            *data = settle.settle(&entry.path, &HashSet::new());
        }
    }
    normalize::sort_entries(&mut entries);
    let [stdout, stderr] = streams;
    let diagnostic =
        |data: Vec<u8>| normalize::mask_patterns(case, normalize::mask_root(case, &roots, data));
    Ok(Observed {
        exit,
        stdout: diagnostic(stdout)?,
        stderr: diagnostic(stderr)?,
        filesystem: entries,
        inconsistencies: issues,
    })
}
