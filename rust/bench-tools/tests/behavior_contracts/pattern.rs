//! A case's `mask_patterns` are Python bytes regexes. Each byte maps to the code point of the same value, so a
//! Unicode regex sees one character per byte, and the classes whose Unicode meaning reaches past ASCII are
//! rewritten as their ASCII bytes-mode sets. A construct whose translation is not exact is refused.

use fancy_regex::{NoExpand, Regex};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

fn translate(pattern: &str) -> Result<String, String> {
    let refuse = |what: &str| {
        Err(format!(
            "mask pattern {pattern:?}: {what} has no exact bytes translation here"
        ))
    };
    let mut out = String::new();
    let mut chars = pattern.chars().peekable();
    let mut in_class = false;
    let mut class_start = false;
    while let Some(c) = chars.next() {
        if c == '\\' {
            let Some(escaped) = chars.next() else {
                return refuse("a trailing backslash");
            };
            let ascii = match escaped {
                's' => Some(r"[\t\n\x0B\x0C\r ]"),
                'S' => Some(r"[^\t\n\x0B\x0C\r ]"),
                'd' => Some("[0-9]"),
                'D' => Some("[^0-9]"),
                'w' => Some("[0-9A-Za-z_]"),
                'W' => Some("[^0-9A-Za-z_]"),
                'b' | 'B' | 'Z' => return refuse(&format!("\\{escaped}")),
                _ => None,
            };
            match ascii {
                Some(_) if in_class => return refuse(&format!("\\{escaped} in a class")),
                Some(set) => out.push_str(set),
                None => {
                    out.push('\\');
                    out.push(escaped);
                }
            }
            class_start = false;
            continue;
        }
        if in_class {
            if c == ']' && !class_start {
                in_class = false;
            } else if c == '[' {
                return refuse("a nested class");
            } else if matches!(c, '&' | '-' | '~') && chars.peek() == Some(&c) {
                return refuse("a set operation");
            }
            class_start = false;
            out.push(c);
            continue;
        }
        out.push(c);
        if c == '[' {
            in_class = true;
            class_start = true;
            if chars.peek() == Some(&'^') {
                out.push('^');
                chars.next();
            }
        }
    }
    Ok(out)
}

fn compiled(pattern: &str) -> Result<Regex, String> {
    static CACHE: OnceLock<Mutex<HashMap<String, Regex>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Mutex::default);
    if let Some(found) = cache
        .lock()
        .map_err(|error| error.to_string())?
        .get(pattern)
    {
        return Ok(found.clone());
    }
    let regex = Regex::new(&translate(pattern)?)
        .map_err(|error| format!("mask pattern {pattern:?}: {error}"))?;
    cache
        .lock()
        .map_err(|error| error.to_string())?
        .insert(pattern.to_string(), regex.clone());
    Ok(regex)
}

/// `re.sub(pattern.encode(), label.encode(), data)` for a label without escapes.
pub fn substitute(pattern: &str, label: &str, data: &[u8]) -> Result<Vec<u8>, String> {
    if !label.is_ascii() || label.contains('\\') {
        return Err(format!(
            "mask label {label:?} must be ASCII without escapes"
        ));
    }
    let text: String = data.iter().map(|byte| char::from(*byte)).collect();
    let replaced = compiled(pattern)?
        .try_replacen(&text, 0, NoExpand(label))
        .map_err(|error| format!("mask pattern {pattern:?}: {error}"))?;
    Ok(replaced.chars().map(|c| c as u32 as u8).collect())
}

fn word(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

/// The end of a token written at `at`: one of `prefixes`, or with none any lowercase run, then `-` and six to
/// eight lowercase hex digits, with no word character or `-` before it and no word character after.
pub fn token_at(data: &[u8], at: usize, prefixes: Option<&[&str]>) -> Option<usize> {
    if at > 0 && (word(data[at - 1]) || data[at - 1] == b'-') {
        return None;
    }
    let after_prefix = match prefixes {
        Some(prefixes) => prefixes.iter().find_map(|prefix| {
            let end = at + prefix.len();
            (data[at..].starts_with(prefix.as_bytes()) && data.get(end) == Some(&b'-'))
                .then_some(end)
        })?,
        None => {
            let run = data[at..]
                .iter()
                .take_while(|byte| byte.is_ascii_lowercase())
                .count();
            (run > 0 && data.get(at + run) == Some(&b'-')).then_some(at + run)?
        }
    };
    let start = after_prefix + 1;
    let hex = data[start..]
        .iter()
        .take_while(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
        .count();
    let end = start + hex;
    ((6..=8).contains(&hex) && !data.get(end).copied().is_some_and(word)).then_some(end)
}

/// Each token in `data`, as `token_at` finds them scanning left to right without overlap.
pub fn tokens(data: &[u8], prefixes: Option<&[&str]>) -> Vec<(usize, usize)> {
    let mut found = Vec::new();
    let mut at = 0;
    while at < data.len() {
        match token_at(data, at, prefixes) {
            Some(end) => {
                found.push((at, end));
                at = end;
            }
            None => at += 1,
        }
    }
    found
}

/// `data` with each token replaced by `label(token)`.
pub fn replace_tokens(
    data: &[u8],
    prefixes: Option<&[&str]>,
    mut label: impl FnMut(&[u8]) -> Vec<u8>,
) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len());
    let mut last = 0;
    for (start, end) in tokens(data, prefixes) {
        out.extend_from_slice(&data[last..start]);
        out.extend(label(&data[start..end]));
        last = end;
    }
    out.extend_from_slice(&data[last..]);
    out
}

/// Whether `value` is exactly `prefix-` and six to eight lowercase hex digits.
pub fn is_token(value: &str, prefix: &str) -> bool {
    value
        .strip_prefix(prefix)
        .and_then(|rest| rest.strip_prefix('-'))
        .is_some_and(|hex| {
            (6..=8).contains(&hex.len())
                && hex.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
        })
}

/// Every `<PREFIX N>` label already in `data` for one of `prefixes`.
pub fn labels_held(data: &[u8], prefixes: &[&str]) -> Vec<String> {
    let mut held = Vec::new();
    for at in 0..data.len() {
        if data[at] != b'<' {
            continue;
        }
        for prefix in prefixes {
            let rest = &data[at + 1..];
            if rest.starts_with(prefix.as_bytes()) && rest.get(prefix.len()) == Some(&b' ') {
                let digits = rest[prefix.len() + 1..]
                    .iter()
                    .take_while(|byte| byte.is_ascii_digit())
                    .count();
                let end = prefix.len() + 1 + digits;
                if digits > 0 && rest.get(end) == Some(&b'>') {
                    held.push(String::from_utf8_lossy(&data[at..at + 2 + end]).into_owned());
                }
            }
        }
    }
    held
}
