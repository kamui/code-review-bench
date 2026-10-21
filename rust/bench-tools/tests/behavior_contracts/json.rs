//! JSON as the corpora and their comparisons read it: values without repeated keys or non-finite numbers,
//! RFC 6901 pointers, the byte spans of values and keys in written text, and the canonical contract digest.

use bench_tools::pyjson::{self, Dumps, Duplicates, Key, Value};
use std::collections::{HashMap, HashSet};

/// UTF-8 JSON with each key written once and only finite numbers. A `\udcXX` escape, which `json.dumps` writes
/// for a filename byte `os.fsdecode` could not decode, reads as those bytes.
pub fn value(raw: &[u8]) -> Result<Value, String> {
    let text = std::str::from_utf8(raw).map_err(|error| format!("not UTF-8: {error}"))?;
    let value =
        pyjson::loads_os_text(text, Duplicates::Refuse).map_err(|error| error.to_string())?;
    finite(&value)?;
    Ok(value)
}

fn finite(value: &Value) -> Result<(), String> {
    match value {
        Value::Float(number) if !number.is_finite() => {
            Err(format!("nonfinite JSON number {number}"))
        }
        Value::Array(items) => items.iter().try_for_each(finite),
        Value::Object(object) => object.members().try_for_each(|(_, child)| finite(child)),
        _ => Ok(()),
    }
}

pub fn dumps(value: &Value) -> String {
    pyjson::dumps(value, &Dumps::default())
}

pub fn dumps_sorted(value: &Value) -> String {
    pyjson::dumps(
        value,
        &Dumps {
            sort_keys: true,
            ..Dumps::default()
        },
    )
}

pub fn dumps_sorted_indented(value: &Value) -> String {
    pyjson::dumps(
        value,
        &Dumps {
            indent: Some(2),
            sort_keys: true,
            ..Dumps::default()
        },
    )
}

pub fn dumps_indented(value: &Value) -> String {
    pyjson::dumps(
        value,
        &Dumps {
            indent: Some(2),
            ..Dumps::default()
        },
    )
}

pub fn text(value: &str) -> Value {
    Value::Str(value.to_string())
}

pub fn escape(key: &str) -> String {
    key.replace('~', "~0").replace('/', "~1")
}

fn unescape(part: &str) -> String {
    part.replace("~1", "/").replace("~0", "~")
}

fn index(items: usize, part: &str) -> Result<usize, String> {
    let number: i64 = part
        .trim()
        .parse()
        .map_err(|_| format!("invalid list index {part:?}"))?;
    let at = if number < 0 {
        number + items as i64
    } else {
        number
    };
    if at < 0 || at >= items as i64 {
        return Err(format!("list index {number} out of range"));
    }
    Ok(at as usize)
}

/// The value at an RFC 6901 pointer, read as Python subscripts: a list takes an integer, a dict a key.
pub fn pointer<'a>(mut value: &'a Value, path: &str) -> Result<&'a Value, String> {
    for part in path.split('/').skip(1) {
        let key = unescape(part);
        value = match value {
            Value::Array(items) => &items[index(items.len(), &key)?],
            Value::Object(object) => object
                .get(&key)
                .ok_or_else(|| format!("missing key {key:?}"))?,
            other => {
                return Err(format!(
                    "{} is not subscriptable by {key:?}",
                    other.type_name()
                ));
            }
        };
    }
    Ok(value)
}

pub fn pointer_mut<'a>(mut value: &'a mut Value, path: &str) -> Result<&'a mut Value, String> {
    for part in path.split('/').skip(1) {
        let key = unescape(part);
        value = match value {
            Value::Array(items) => {
                let at = index(items.len(), &key)?;
                &mut items[at]
            }
            Value::Object(object) => object
                .get_mut(&key)
                .ok_or_else(|| format!("missing key {key:?}"))?,
            other => {
                return Err(format!(
                    "{} is not subscriptable by {key:?}",
                    other.type_name()
                ));
            }
        };
    }
    Ok(value)
}

/// Byte spans in valid JSON text: of the values at `pointers`, and of each key, by (object pointer, key), of the
/// objects at `keys`.
#[derive(Default)]
pub struct Spans {
    pub values: HashMap<String, (usize, usize)>,
    pub keys: HashMap<(String, String), (usize, usize)>,
}

pub fn spans(
    text: &[u8],
    pointers: &HashSet<String>,
    keys: &HashSet<String>,
) -> Result<Spans, String> {
    let mut scanner = Scanner {
        text,
        pointers,
        keys,
        spans: Spans::default(),
    };
    scanner.value(0, String::new())?;
    Ok(scanner.spans)
}

struct Scanner<'a> {
    text: &'a [u8],
    pointers: &'a HashSet<String>,
    keys: &'a HashSet<String>,
    spans: Spans,
}

impl Scanner<'_> {
    fn skip(&self, mut position: usize) -> usize {
        while matches!(self.text.get(position), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            position += 1;
        }
        position
    }

    fn at(&self, position: usize) -> Result<u8, String> {
        self.text
            .get(position)
            .copied()
            .ok_or_else(|| "unexpected end of JSON text".to_string())
    }

    /// The end of the string that opens at `start`, and its decoded text.
    fn string(&self, start: usize) -> Result<(String, usize), String> {
        if self.at(start)? != b'"' {
            return Err(format!("expected a string at byte {start}"));
        }
        let mut position = start + 1;
        loop {
            match self.at(position)? {
                b'\\' => position += 2,
                b'"' => break,
                _ => position += 1,
            }
        }
        let quoted =
            std::str::from_utf8(&self.text[start..=position]).map_err(|error| error.to_string())?;
        match pyjson::loads_os_text(quoted, Duplicates::KeepLast)
            .map_err(|error| error.to_string())?
        {
            Value::Str(text) => Ok((text, position + 1)),
            other @ Value::OsText(_) => Ok((
                String::from_utf8_lossy(other.os_bytes().unwrap_or_default()).into_owned(),
                position + 1,
            )),
            _ => Err(format!("unreadable string at byte {start}")),
        }
    }

    fn value(&mut self, position: usize, path: String) -> Result<usize, String> {
        let mut position = self.skip(position);
        let start = position;
        let opening = self.at(position)?;
        if opening == b'{' || opening == b'[' {
            let close = if opening == b'{' { b'}' } else { b']' };
            position = self.skip(position + 1);
            if self.at(position)? == close {
                position += 1;
            } else {
                let mut index = 0;
                loop {
                    let child = if opening == b'{' {
                        let start_key = self.skip(position);
                        let (key, end) = self.string(start_key)?;
                        position = end;
                        if self.keys.contains(&path) {
                            self.spans
                                .keys
                                .insert((path.clone(), key.clone()), (start_key, position));
                        }
                        position = self.skip(position) + 1;
                        format!("{path}/{}", escape(&key))
                    } else {
                        index += 1;
                        format!("{path}/{}", index - 1)
                    };
                    let end = self.value(position, child)?;
                    position = self.skip(end) + 1;
                    if self.at(position - 1)? == close {
                        break;
                    }
                }
            }
        } else if opening == b'"' {
            position = self.string(position)?.1;
        } else if matches!(opening, b'-' | b'0'..=b'9') {
            while matches!(
                self.text.get(position),
                Some(b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9')
            ) {
                position += 1;
            }
        } else {
            let word = [&b"true"[..], b"false", b"null"]
                .into_iter()
                .find(|word| self.text[position..].starts_with(word))
                .ok_or_else(|| format!("unexpected JSON at byte {position}"))?;
            position += word.len();
        }
        if self.pointers.contains(&path) {
            self.spans.values.insert(path, (start, position));
        }
        Ok(position)
    }
}

/// The canonical contract encoding a decision's `contract_sha256` digests.
pub fn canonical(value: &Value, out: &mut Vec<u8>) -> Result<(), String> {
    match value {
        Value::Null => out.push(b'n'),
        Value::Bool(flag) => out.push(if *flag { b't' } else { b'f' }),
        Value::Int(digits) => {
            out.push(b'i');
            out.extend_from_slice(digits.as_bytes());
            out.push(b';');
        }
        Value::Float(number) => {
            if !number.is_finite() {
                return Err("nonfinite contract number".to_string());
            }
            out.push(b'd');
            out.extend_from_slice(&number.to_be_bytes());
        }
        Value::Str(text) => {
            out.push(b's');
            out.extend_from_slice(text.len().to_string().as_bytes());
            out.push(b':');
            out.extend_from_slice(text.as_bytes());
        }
        Value::OsText(_) => return Err("a contract string must be UTF-8".to_string()),
        Value::Array(items) => {
            out.push(b'[');
            out.extend_from_slice(items.len().to_string().as_bytes());
            out.push(b':');
            for item in items {
                canonical(item, out)?;
            }
        }
        Value::Object(object) => {
            let mut members: Vec<(&str, &Value)> = Vec::with_capacity(object.len());
            for (key, child) in object.members() {
                match key {
                    Key::Str(text) => members.push((text, child)),
                    Key::OsText(_) => return Err("a contract key must be UTF-8".to_string()),
                }
            }
            members.sort_by(|left, right| left.0.as_bytes().cmp(right.0.as_bytes()));
            out.push(b'{');
            out.extend_from_slice(members.len().to_string().as_bytes());
            out.push(b':');
            for (key, child) in members {
                canonical(&text(key), out)?;
                canonical(child, out)?;
            }
        }
    }
    Ok(())
}

pub fn digest(value: &Value) -> Result<String, String> {
    let mut out = Vec::new();
    canonical(value, &mut out)?;
    Ok(bench_tools::sha256::hex_digest(&out))
}
