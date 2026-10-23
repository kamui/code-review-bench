//! Python's operations on the values `json.loads` returns, failing with the exception Python raises.
//!
//! The ruling readers apply subscripts, `in`, iteration, sets and sorting to JSON of unchecked shape, and their
//! commands catch only some exceptions. A port returns [`Raised`] wherever Python raises, so a command catches
//! exactly what the Python catches and treats the rest as the crash it was.

use crate::pyjson::{self, Duplicates, Value};
use crate::pytext::{self, PyPath};
use std::cmp::Ordering;

#[derive(Clone, Debug, PartialEq)]
pub enum Raised {
    /// `KeyError`, holding `str(error)`: the key's repr.
    Key(String),
    /// `ValueError`, including `json.JSONDecodeError` and `UnicodeDecodeError`.
    Value(String),
    Os(String),
    Type(String),
    Attribute(String),
    Index(String),
    StopIteration,
    /// Input Python reads and Rust refuses: a lone surrogate or JSON nested deeper than 1,000 levels.
    Unsupported(String),
}

impl Raised {
    /// `str(error)` for the exceptions the ruling commands catch: `ValueError`, `OSError` and `KeyError`.
    pub fn caught(&self) -> Option<&str> {
        match self {
            Self::Key(message) | Self::Value(message) | Self::Os(message) => Some(message),
            _ => None,
        }
    }

    /// The failure as printed: [`Self::traceback`] with the exception's plain label.
    pub fn describe(&self) -> String {
        crate::pysemantics::relabel(&self.traceback())
    }

    /// The last line of the traceback Python prints, or the refusal of unsupported input.
    pub fn traceback(&self) -> String {
        match self {
            Self::Key(message) => format!("KeyError: {message}"),
            Self::Value(message) => format!("ValueError: {message}"),
            Self::Os(message) => format!("OSError: {message}"),
            Self::Type(message) => format!("TypeError: {message}"),
            Self::Attribute(message) => format!("AttributeError: {message}"),
            Self::Index(message) => format!("IndexError: {message}"),
            Self::StopIteration => "StopIteration".to_string(),
            Self::Unsupported(message) => message.clone(),
        }
    }
}

pub type Py<T> = Result<T, Raised>;

pub static NONE: Value = Value::Null;

fn type_error<T>(message: String) -> Py<T> {
    Err(Raised::Type(message))
}

/// `KeyError(key)` for a key `json.loads` produced.
pub fn key_error(key: &Value) -> Raised {
    Raised::Key(key.py_repr())
}

pub fn text(value: &str) -> Value {
    Value::Str(value.to_string())
}

/// `value[key]` for a text key.
pub fn item<'a>(value: &'a Value, key: &str) -> Py<&'a Value> {
    match value {
        Value::Object(object) => object
            .get(key)
            .ok_or_else(|| Raised::Key(pytext::repr_str(key))),
        Value::Array(_) => type_error("list indices must be integers or slices, not str".into()),
        Value::Str(_) => type_error("string indices must be integers, not 'str'".into()),
        other => type_error(format!(
            "'{}' object is not subscriptable",
            other.type_name()
        )),
    }
}

/// `value[key]` for any key: a dict by equal key, a list by integer index.
pub fn subscript<'a>(value: &'a Value, key: &Value) -> Py<&'a Value> {
    match (value, key) {
        (Value::Object(object), _) => {
            hashable(key)?;
            object
                .iter()
                .find(|(name, _)| Value::Str((*name).clone()).py_eq(key))
                .map(|(_, member)| member)
                .ok_or_else(|| key_error(key))
        }
        (Value::Array(values), Value::Int(_) | Value::Bool(_)) => {
            let index = integer(key).and_then(|digits| digits.parse::<i128>().ok());
            let length = values.len() as i128;
            index
                .map(|index| if index < 0 { index + length } else { index })
                .filter(|index| (0..length).contains(index))
                .map(|index| &values[index as usize])
                .ok_or_else(|| Raised::Index("list index out of range".into()))
        }
        (Value::Array(_), other) => type_error(format!(
            "list indices must be integers or slices, not {}",
            other.type_name()
        )),
        (Value::Str(_), other) => type_error(format!(
            "string indices must be integers, not '{}'",
            other.type_name()
        )),
        (other, _) => type_error(format!(
            "'{}' object is not subscriptable",
            other.type_name()
        )),
    }
}

fn integer(value: &Value) -> Option<String> {
    match value {
        Value::Bool(flag) => Some(u8::from(*flag).to_string()),
        Value::Int(digits) => Some(digits.clone()),
        _ => None,
    }
}

/// `value.get(key)`, `None` when absent.
pub fn get<'a>(value: &'a Value, key: &str) -> Py<&'a Value> {
    match value {
        Value::Object(object) => Ok(object.get(key).unwrap_or(&NONE)),
        other => Err(Raised::Attribute(format!(
            "'{}' object has no attribute 'get'",
            other.type_name()
        ))),
    }
}

/// `value.get(key, default)`.
pub fn get_or<'a>(value: &'a Value, key: &str, default: &'a Value) -> Py<&'a Value> {
    match value {
        Value::Object(object) => Ok(object.get(key).unwrap_or(default)),
        other => Err(Raised::Attribute(format!(
            "'{}' object has no attribute 'get'",
            other.type_name()
        ))),
    }
}

/// A value Python can hash, as a set member or dict key must be.
pub fn hashable(value: &Value) -> Py<()> {
    if value.hashable() {
        Ok(())
    } else {
        type_error(format!("unhashable type: '{}'", value.type_name()))
    }
}

/// `item in container`.
pub fn contains(container: &Value, item: &Value) -> Py<bool> {
    match container {
        Value::Object(object) => {
            hashable(item)?;
            Ok(object
                .iter()
                .any(|(name, _)| Value::Str(name.clone()).py_eq(item)))
        }
        Value::Array(values) => Ok(values.iter().any(|value| value.py_eq(item))),
        Value::Str(text) => match item {
            Value::Str(part) => Ok(text.contains(part.as_str())),
            other => type_error(format!(
                "'in <string>' requires string as left operand, not {}",
                other.type_name()
            )),
        },
        other => type_error(format!(
            "argument of type '{}' is not a container or iterable",
            other.type_name()
        )),
    }
}

/// `key in container` for a text key.
pub fn has(container: &Value, key: &str) -> Py<bool> {
    contains(container, &text(key))
}

/// What a `for` loop visits: list elements, characters or dict keys.
pub fn iterate(value: &Value) -> Py<Vec<Value>> {
    match value {
        Value::Array(values) => Ok(values.clone()),
        Value::Str(text) => Ok(text.chars().map(|c| Value::Str(c.to_string())).collect()),
        Value::Object(object) => Ok(object
            .iter()
            .map(|(key, _)| Value::Str(key.clone()))
            .collect()),
        other => type_error(format!("'{}' object is not iterable", other.type_name())),
    }
}

/// `len(value)`.
pub fn length(value: &Value) -> Py<usize> {
    match value {
        Value::Array(values) => Ok(values.len()),
        Value::Str(text) => Ok(text.chars().count()),
        Value::Object(object) => Ok(object.len()),
        other => type_error(format!(
            "object of type '{}' has no len()",
            other.type_name()
        )),
    }
}

/// `value in ("a", "b", ...)`.
pub fn one_of(value: &Value, names: &[&str]) -> bool {
    value.as_str().is_some_and(|text| names.contains(&text))
}

/// `value == "text"`.
pub fn is(value: &Value, expected: &str) -> bool {
    value.as_str() == Some(expected)
}

/// The members of `set(values)` in first-seen order, which is all a length or subset test reads.
pub fn distinct(values: &[Value]) -> Py<Vec<Value>> {
    let mut members: Vec<Value> = Vec::new();
    for value in values {
        hashable(value)?;
        if !members.iter().any(|member| member.py_eq(value)) {
            members.push(value.clone());
        }
    }
    Ok(members)
}

fn numeric(value: &Value) -> bool {
    matches!(value, Value::Bool(_) | Value::Int(_) | Value::Float(_))
}

/// The digits of an integer with its sign, for an exact comparison of any length.
fn signed(digits: &str) -> (bool, &str) {
    match digits.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, digits),
    }
}

/// Orders two integers written as `int` digits with an optional `-`, as Python orders them, at any length.
pub fn compare_integers(left: &str, right: &str) -> Ordering {
    let ((left_negative, left), (right_negative, right)) = (signed(left), signed(right));
    let magnitude = left.len().cmp(&right.len()).then_with(|| left.cmp(right));
    match (left_negative, right_negative) {
        (false, false) => magnitude,
        (true, true) => magnitude.reverse(),
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
    }
}

fn as_float(value: &Value) -> f64 {
    match value {
        Value::Bool(flag) => f64::from(u8::from(*flag)),
        Value::Int(digits) => digits.parse().unwrap_or(f64::NAN),
        Value::Float(number) => *number,
        _ => f64::NAN,
    }
}

/// `left < right` and friends: text with text by code point, numbers with numbers.
pub fn compare(left: &Value, right: &Value) -> Py<Ordering> {
    match (left, right) {
        (Value::Str(left), Value::Str(right)) => Ok(left.cmp(right)),
        (left, right) if numeric(left) && numeric(right) => match (integer(left), integer(right)) {
            (Some(left), Some(right)) => Ok(compare_integers(&left, &right)),
            _ => Ok(as_float(left)
                .partial_cmp(&as_float(right))
                .unwrap_or(Ordering::Equal)),
        },
        (left, right) => type_error(format!(
            "'<' not supported between instances of '{}' and '{}'",
            left.type_name(),
            right.type_name()
        )),
    }
}

/// `sorted(values)`: Python compares neighbours, so values of two incomparable kinds raise.
pub fn sorted(mut values: Vec<Value>) -> Py<Vec<Value>> {
    if let Some(first) = values.first() {
        for value in &values[1..] {
            compare(first, value)?;
        }
    }
    values.sort_by(|left, right| compare(left, right).unwrap_or(Ordering::Equal));
    Ok(values)
}

/// `separator.join(values)`, which takes text only.
pub fn join(separator: &str, values: &[Value]) -> Py<String> {
    let mut parts = Vec::with_capacity(values.len());
    for (position, value) in values.iter().enumerate() {
        match value {
            Value::Str(text) => parts.push(text.as_str()),
            other => {
                return type_error(format!(
                    "sequence item {position}: expected str instance, {} found",
                    other.type_name()
                ));
            }
        }
    }
    Ok(parts.join(separator))
}

/// `int(value)` compared with a small integer, as `asked < 1` reads it.
pub fn compare_with(value: &Value, number: i64) -> Py<Ordering> {
    compare(value, &Value::Int(number.to_string()))
}

/// `str.split()` with no argument.
pub fn split_whitespace(text: &str) -> Vec<&str> {
    text.split(pytext::is_space)
        .filter(|part| !part.is_empty())
        .collect()
}

/// `root / segment`, where `segment` came from JSON and must be text.
pub fn join_path(root: &PyPath, segment: &Value) -> Py<PyPath> {
    match segment {
        Value::Str(text) => Ok(root.join(text.as_bytes())),
        other => type_error(format!(
            "unsupported operand type(s) for /: 'PosixPath' and '{}'",
            other.type_name()
        )),
    }
}

/// `Path.read_bytes()`.
pub fn read_bytes(path: &PyPath) -> Py<Vec<u8>> {
    if path.as_bytes().contains(&0) {
        return Err(Raised::Value("embedded null byte".into()));
    }
    std::fs::read(path.to_path_buf())
        .map_err(|error| Raised::Os(pytext::os_error(&error, path.as_bytes())))
}

/// `Path.read_text(encoding="utf-8")`, with universal newlines.
pub fn read_text(path: &PyPath) -> Py<String> {
    let bytes = read_bytes(path)?;
    let text = pytext::decode_utf8(&bytes).map_err(Raised::Value)?;
    Ok(pytext::universal_newlines(&text))
}

/// `json.loads(Path(path).read_text(encoding="utf-8"))`.
pub fn load(path: &PyPath) -> Py<Value> {
    loads(&read_text(path)?)
}

pub fn loads(text: &str) -> Py<Value> {
    pyjson::loads(text, Duplicates::KeepLast).map_err(|error| match error {
        pyjson::ParseError::Value(message) if !message.starts_with("Exceeds the limit") => {
            Raised::Unsupported(format!("unsupported JSON: {message}"))
        }
        other => Raised::Value(other.to_string()),
    })
}

/// `Path.is_file()`, false when the path cannot be read.
pub fn is_file(path: &PyPath) -> bool {
    !path.as_bytes().contains(&0) && path.is_file()
}

/// `Path.exists()`.
pub fn exists(path: &PyPath) -> bool {
    !path.as_bytes().contains(&0) && path.to_path_buf().exists()
}

/// Ranges of the characters `str.isdigit()` accepts that are not decimal digits, in Unicode 16.0, generated from
/// Python 3.14. Python 3.9 to 3.14 hold the same 128.
const DIGITS_NOT_DECIMAL: [(u32, u32); 20] = [
    (0xB2, 0xB3),
    (0xB9, 0xB9),
    (0x1369, 0x1371),
    (0x19DA, 0x19DA),
    (0x2070, 0x2070),
    (0x2074, 0x2079),
    (0x2080, 0x2089),
    (0x2460, 0x2468),
    (0x2474, 0x247C),
    (0x2488, 0x2490),
    (0x24EA, 0x24EA),
    (0x24F5, 0x24FD),
    (0x24FF, 0x24FF),
    (0x2776, 0x277E),
    (0x2780, 0x2788),
    (0x278A, 0x2792),
    (0x10A40, 0x10A43),
    (0x10E60, 0x10E68),
    (0x11052, 0x1105A),
    (0x1F100, 0x1F10A),
];

/// `str.isdigit()` for one character.
pub fn is_digit(c: char) -> bool {
    let point = u32::from(c);
    pytext::decimal_value(c).is_some()
        || DIGITS_NOT_DECIMAL
            .iter()
            .any(|(low, high)| (*low..=*high).contains(&point))
}

/// One element of a natural sort key: text, or the value of a run of decimal digits.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Natural {
    Text(String),
    /// Without leading zeros; `0` is empty.
    Number(String),
}

impl Natural {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::Text(_), Self::Number(_)) => Ordering::Less,
            (Self::Number(_), Self::Text(_)) => Ordering::Greater,
            (Self::Text(left), Self::Text(right)) => left.cmp(right),
            (Self::Number(left), Self::Number(right)) => {
                left.len().cmp(&right.len()).then_with(|| left.cmp(right))
            }
        }
    }
}

pub fn natural_order(left: &[Natural], right: &[Natural]) -> Ordering {
    for (left, right) in left.iter().zip(right) {
        let order = left.cmp(right);
        if order != Ordering::Equal {
            return order;
        }
    }
    left.len().cmp(&right.len())
}

/// `ruling_summary.natural`: `re.split(r"(\d+)", value)`, each digit run read by `int()`. A text part made only
/// of characters `str.isdigit()` accepts, such as `²`, makes `int()` raise.
pub fn natural(value: &str) -> Py<Vec<Natural>> {
    let mut parts: Vec<(bool, String)> = vec![(false, String::new())];
    for c in value.chars() {
        let digit = pytext::decimal_value(c).is_some();
        if parts.last().is_some_and(|(kind, _)| *kind != digit) {
            parts.push((digit, String::new()));
        }
        if let Some(part) = parts.last_mut() {
            part.1.push(c);
        }
    }
    if parts.last().is_some_and(|(digit, _)| *digit) {
        parts.push((false, String::new()));
    }
    let mut key = Vec::with_capacity(parts.len());
    for (digit, part) in parts {
        if !digit && !part.is_empty() && part.chars().all(is_digit) {
            return Err(Raised::Value(format!(
                "invalid literal for int() with base 10: {}",
                pytext::repr_str(&part)
            )));
        }
        if !digit {
            key.push(Natural::Text(part));
            continue;
        }
        let count = part.chars().count();
        if let Some(message) = pyjson::int_digit_limit(count) {
            return Err(Raised::Value(message));
        }
        let digits: String = part
            .chars()
            .filter_map(pytext::decimal_value)
            .filter_map(|digit| char::from_digit(digit, 10))
            .collect();
        key.push(Natural::Number(digits.trim_start_matches('0').to_string()));
    }
    Ok(key)
}

/// `re.split(r"[-,\s]+", text)`.
pub fn split_separators(text: &str) -> Vec<String> {
    let separator = |c: char| c == '-' || c == ',' || pytext::is_space(c);
    let mut parts = vec![String::new()];
    let mut in_separator = false;
    for c in text.chars() {
        if separator(c) {
            if !in_separator {
                parts.push(String::new());
            }
            in_separator = true;
        } else {
            in_separator = false;
            if let Some(part) = parts.last_mut() {
                part.push(c);
            }
        }
    }
    parts
}

/// `re.search(r"\.v\d+\.", name)`.
pub fn has_version(name: &[u32]) -> bool {
    let digit = |point: &u32| {
        char::from_u32(*point)
            .and_then(pytext::decimal_value)
            .is_some()
    };
    (0..name.len()).any(|start| {
        name[start..].starts_with(&[u32::from('.'), u32::from('v')]) && {
            let rest = &name[start + 2..];
            let run = rest.iter().take_while(|point| digit(point)).count();
            run > 0 && rest.get(run) == Some(&u32::from('.'))
        }
    })
}

/// `re.fullmatch(r"\d{4}-\d{2}-\d{2}", text)`.
pub fn is_date(text: &str) -> bool {
    let characters: Vec<char> = text.chars().collect();
    characters.len() == 10
        && characters.iter().enumerate().all(|(position, c)| {
            if position == 4 || position == 7 {
                *c == '-'
            } else {
                pytext::decimal_value(*c).is_some()
            }
        })
}

enum Token {
    Star,
    Any,
    Literal(u32),
    /// A bracket expression: negated, single characters and inclusive ranges. An empty, non-negated set never
    /// matches.
    Set(bool, Vec<(u32, u32)>),
}

/// `fnmatch._translate`'s bracket expression for the text between `[` and `]`, read as `re` reads the class it
/// writes: each chunk's characters are literal, and the hyphen joining two chunks spans a range.
fn bracket(pattern: &[u32], start: usize, end: usize) -> Py<Token> {
    let dash = u32::from('-');
    let bang = u32::from('!');
    let stuff = &pattern[start..end];
    if !stuff.contains(&dash) {
        let negated = stuff.first() == Some(&bang);
        let singles = stuff[usize::from(negated)..]
            .iter()
            .map(|point| (*point, *point))
            .collect();
        return Ok(Token::Set(negated, singles));
    }
    let mut chunks: Vec<Vec<u32>> = Vec::new();
    let mut i = start;
    let mut k = if pattern[start] == bang {
        start + 2
    } else {
        start + 1
    };
    while let Some(position) = (k..end).find(|position| pattern[*position] == dash) {
        chunks.push(pattern[i..position].to_vec());
        i = position + 1;
        k = position + 3;
    }
    let chunk = pattern[i..end].to_vec();
    if chunk.is_empty() {
        match chunks.last_mut() {
            Some(last) => last.push(dash),
            None => return Err(Raised::Index("list index out of range".into())),
        }
    } else {
        chunks.push(chunk);
    }
    for k in (1..chunks.len()).rev() {
        let (Some(previous), Some(next)) = (chunks[k - 1].last(), chunks[k].first()) else {
            return Err(Raised::Index("string index out of range".into()));
        };
        if previous > next {
            let mut merged = chunks[k - 1][..chunks[k - 1].len() - 1].to_vec();
            merged.extend_from_slice(&chunks[k][1..]);
            chunks[k - 1] = merged;
            chunks.remove(k);
        }
    }
    if chunks.len() == 1 && chunks[0] == [bang] {
        return Ok(Token::Set(true, Vec::new()));
    }
    let negated = chunks[0].first() == Some(&bang);
    // The class `re` compiles: `None` is a hyphen joining two chunks.
    let mut class: Vec<Option<u32>> = Vec::new();
    for (position, chunk) in chunks.iter().enumerate() {
        if position > 0 {
            class.push(None);
        }
        let skip = usize::from(position == 0 && negated);
        class.extend(chunk[skip..].iter().map(|point| Some(*point)));
    }
    let mut members = Vec::new();
    let mut at = 0;
    while at < class.len() {
        let low = class[at].unwrap_or(dash);
        match (class.get(at + 1), class.get(at + 2)) {
            (Some(None), Some(high)) => {
                members.push((low, high.unwrap_or(dash)));
                at += 3;
            }
            _ => {
                members.push((low, low));
                at += 1;
            }
        }
    }
    Ok(Token::Set(negated, members))
}

fn tokens(pattern: &[u32]) -> Py<Vec<Token>> {
    let mut tokens = Vec::new();
    let mut i = 0;
    let n = pattern.len();
    while i < n {
        let c = pattern[i];
        i += 1;
        match char::from_u32(c) {
            Some('*') => {
                if !matches!(tokens.last(), Some(Token::Star)) {
                    tokens.push(Token::Star);
                }
            }
            Some('?') => tokens.push(Token::Any),
            Some('[') => {
                let mut j = i;
                if j < n && pattern[j] == u32::from('!') {
                    j += 1;
                }
                if j < n && pattern[j] == u32::from(']') {
                    j += 1;
                }
                while j < n && pattern[j] != u32::from(']') {
                    j += 1;
                }
                if j >= n {
                    tokens.push(Token::Literal(c));
                } else {
                    tokens.push(bracket(pattern, i, j)?);
                    i = j + 1;
                }
            }
            _ => tokens.push(Token::Literal(c)),
        }
    }
    Ok(tokens)
}

fn matches_one(token: &Token, point: u32) -> bool {
    match token {
        Token::Any => true,
        Token::Literal(literal) => *literal == point,
        Token::Set(negated, members) => {
            members
                .iter()
                .any(|(low, high)| (*low..=*high).contains(&point))
                != *negated
        }
        Token::Star => false,
    }
}

/// `fnmatch.fnmatchcase(name, pattern)`, and a `pathlib` glob of one path component.
pub fn fnmatchcase(name: &[u32], pattern: &[u32]) -> Py<bool> {
    let tokens = tokens(pattern)?;
    let (mut t, mut p) = (0, 0);
    let mut backtrack: Option<(usize, usize)> = None;
    while p < name.len() {
        if t < tokens.len() && matches!(tokens[t], Token::Star) {
            backtrack = Some((t, p));
            t += 1;
        } else if t < tokens.len() && matches_one(&tokens[t], name[p]) {
            t += 1;
            p += 1;
        } else if let Some((star, from)) = backtrack {
            t = star + 1;
            p = from + 1;
            backtrack = Some((star, from + 1));
        } else {
            return Ok(false);
        }
    }
    Ok(tokens[t..].iter().all(|token| matches!(token, Token::Star)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn matches(name: &str, pattern: &str) -> bool {
        let points = |text: &str| text.chars().map(u32::from).collect::<Vec<_>>();
        fnmatchcase(&points(name), &points(pattern)).unwrap()
    }

    #[test]
    fn fnmatch_reads_brackets_as_python_does() {
        assert!(matches("a.md", "*.md"));
        assert!(matches(".md", "*.md"));
        assert!(!matches("a.mdx", "*.md"));
        assert!(matches("ab", "a[b]"));
        assert!(!matches("a[b]", "a[b]"));
        assert!(matches("a[b", "a[b"));
        assert!(matches("x", "[!a]"));
        assert!(!matches("a", "[!a]"));
        assert!(matches("-", "[a-]"));
        assert!(matches("c", "[a-c]"));
        assert!(!matches("d", "[a-c]"));
        assert!(matches("-", "[a-b-c]"));
        assert!(matches("c", "[a-b-c]"));
        assert!(!matches("a", "[z-a]"));
        assert!(!matches("\n", "[!]"));
        assert!(matches("[!]", "[!]"));
        assert!(matches("]", "[]]"));
        assert!(!matches("-", "[!-a]"));
        assert!(matches("b", "[!-a]"));
        assert!(matches("abc", "a*c"));
        assert!(matches("ac", "a*c"));
        assert!(!matches("ab", "a*c"));
        assert!(matches("12-x.md", "12-*"));
    }

    #[test]
    fn natural_keys_split_on_decimal_runs() {
        assert_eq!(
            natural("a10b").unwrap(),
            vec![
                Natural::Text("a".into()),
                Natural::Number("10".into()),
                Natural::Text("b".into())
            ]
        );
        assert_eq!(
            natural("7").unwrap(),
            vec![
                Natural::Text(String::new()),
                Natural::Number("7".into()),
                Natural::Text(String::new())
            ]
        );
        assert!(natural_order(&natural("a2").unwrap(), &natural("a10").unwrap()).is_lt());
        assert_eq!(
            natural("x1²"),
            Err(Raised::Value(
                "invalid literal for int() with base 10: '²'".into()
            ))
        );
        assert!(natural("x²y").is_ok());
    }

    #[test]
    fn separators_and_patterns_match_python() {
        assert_eq!(split_separators("N1-N2"), vec!["N1", "N2"]);
        assert_eq!(split_separators("-a, b-"), vec!["", "a", "b", ""]);
        assert_eq!(split_separators(""), vec![""]);
        let points = |text: &str| text.chars().map(u32::from).collect::<Vec<_>>();
        assert!(has_version(&points("rule.v12.md")));
        assert!(!has_version(&points("rule.v.md")));
        assert!(has_version(&points("x.v1.v2.md")));
        assert!(is_date("2026-10-19"));
        assert!(!is_date("2026-1-19"));
    }
}
