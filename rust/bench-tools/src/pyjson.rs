//! JSON read and written as Python's `json` module does, for hashes, retry payloads and diagnostics that
//! depend on its spelling.
//!
//! Reading follows `json.loads` with the C scanner of Python 3.13 and later: `NaN`, `Infinity` and
//! `-Infinity` are accepted, integers keep every digit up to Python's 4300-digit limit, and a repeated key
//! keeps its first position and last value, as `dict` does. Error messages and positions are those of
//! `json.JSONDecodeError`. A lone surrogate escape, which Python keeps in a `str`, is refused.
//!
//! Writing follows `json.dumps`: `ensure_ascii`, `indent`, `separators` and `sort_keys`, with `float.__repr__`
//! spelling and `NaN`, `Infinity` and `-Infinity` for non-finite numbers.

use crate::pytext;
use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;

#[derive(Clone, Debug)]
pub enum Value {
    Null,
    Bool(bool),
    /// Decimal digits with an optional leading `-`, never `-0`.
    Int(String),
    Float(f64),
    Str(String),
    Array(Vec<Value>),
    Object(Object),
}

/// A `dict`: keys are unique and keep the position of their first occurrence.
#[derive(Clone, Debug, Default)]
pub struct Object {
    entries: Vec<(String, Value)>,
    index: HashMap<String, usize>,
}

impl Object {
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.index
            .get(key)
            .map(|position| &self.entries[*position].1)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &Value)> {
        self.entries.iter().map(|(key, value)| (key, value))
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// `dict[key] = value`: a new key goes last, an existing one keeps its position.
    pub fn insert(&mut self, key: String, value: Value) {
        match self.index.get(&key) {
            Some(position) => self.entries[*position].1 = value,
            None => {
                self.index.insert(key.clone(), self.entries.len());
                self.entries.push((key, value));
            }
        }
    }
}

impl FromIterator<(String, Value)> for Object {
    fn from_iter<I: IntoIterator<Item = (String, Value)>>(pairs: I) -> Self {
        let mut object = Self::default();
        for (key, value) in pairs {
            object.insert(key, value);
        }
        object
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Duplicates {
    /// As `dict(pairs)` does.
    KeepLast,
    /// Refuse an object that writes a key more than once, as an `object_pairs_hook` that raises would.
    Refuse,
}

#[derive(Debug, PartialEq)]
pub enum ParseError {
    /// `json.JSONDecodeError`.
    Syntax {
        message: String,
        position: usize,
        line: usize,
        column: usize,
    },
    /// The keys one object writes more than once, sorted, under `Duplicates::Refuse`; displayed as
    /// `a, b is written more than once`.
    Duplicates(Vec<String>),
    /// Another `ValueError`, such as Python's integer digit limit.
    Value(String),
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Syntax {
                message,
                position,
                line,
                column,
            } => {
                write!(
                    f,
                    "{message}: line {line} column {column} (char {position})"
                )
            }
            Self::Duplicates(keys) => write!(f, "{} is written more than once", keys.join(", ")),
            Self::Value(message) => f.write_str(message),
        }
    }
}

const INT_MAX_STR_DIGITS: usize = 4300;
/// Python's own limit depends on its version and stack size; deeper input is refused here.
const MAX_DEPTH: usize = 1_000;

/// Python's `ValueError` message when `int()` reads more decimal digits than its default limit.
pub fn int_digit_limit(digits: usize) -> Option<String> {
    (digits > INT_MAX_STR_DIGITS).then(|| format!(
        "Exceeds the limit ({INT_MAX_STR_DIGITS} digits) for integer string conversion: value has {digits} digits; use sys.set_int_max_str_digits() to increase the limit"
    ))
}

enum Failure {
    /// The scanner found no value here: Python's `StopIteration`, reported as `Expecting value`.
    NoValue(usize),
    Error(ParseError),
}

struct Parser {
    chars: Vec<char>,
    duplicates: Duplicates,
    depth: usize,
}

pub fn loads(text: &str, duplicates: Duplicates) -> Result<Value, ParseError> {
    let mut parser = Parser {
        chars: text.chars().collect(),
        duplicates,
        depth: 0,
    };
    if parser.chars.first() == Some(&'\u{feff}') {
        return Err(parser.syntax("Unexpected UTF-8 BOM (decode using utf-8-sig)", 0));
    }
    let start = parser.whitespace(0);
    let (value, end) = match parser.value(start) {
        Ok(found) => found,
        Err(Failure::NoValue(position)) => return Err(parser.syntax("Expecting value", position)),
        Err(Failure::Error(error)) => return Err(error),
    };
    let end = parser.whitespace(end);
    if end != parser.chars.len() {
        return Err(parser.syntax("Extra data", end));
    }
    Ok(value)
}

impl Parser {
    fn syntax(&self, message: &str, position: usize) -> ParseError {
        let before = &self.chars[..position.min(self.chars.len())];
        let line = before.iter().filter(|c| **c == '\n').count() + 1;
        let column = match before.iter().rposition(|c| *c == '\n') {
            Some(newline) => position - newline,
            None => position + 1,
        };
        ParseError::Syntax {
            message: message.to_string(),
            position,
            line,
            column,
        }
    }

    fn fail(&self, message: &str, position: usize) -> Failure {
        Failure::Error(self.syntax(message, position))
    }

    fn at(&self, index: usize) -> Option<char> {
        self.chars.get(index).copied()
    }

    fn whitespace(&self, mut index: usize) -> usize {
        while matches!(self.at(index), Some(' ' | '\t' | '\n' | '\r')) {
            index += 1;
        }
        index
    }

    fn literal(&self, index: usize, word: &str) -> bool {
        (index..)
            .zip(word.chars())
            .all(|(position, expected)| self.at(position) == Some(expected))
    }

    fn value(&mut self, index: usize) -> Result<(Value, usize), Failure> {
        match self.at(index) {
            None => Err(Failure::NoValue(index)),
            Some('"') => {
                let (text, end) = self.string(index + 1)?;
                Ok((Value::Str(text), end))
            }
            Some('{') => self.nested(|parser| parser.object(index + 1)),
            Some('[') => self.nested(|parser| parser.array(index + 1)),
            Some('n') if self.literal(index, "null") => Ok((Value::Null, index + 4)),
            Some('t') if self.literal(index, "true") => Ok((Value::Bool(true), index + 4)),
            Some('f') if self.literal(index, "false") => Ok((Value::Bool(false), index + 5)),
            Some('N') if self.literal(index, "NaN") => Ok((Value::Float(f64::NAN), index + 3)),
            Some('I') if self.literal(index, "Infinity") => {
                Ok((Value::Float(f64::INFINITY), index + 8))
            }
            Some('-') if self.literal(index, "-Infinity") => {
                Ok((Value::Float(f64::NEG_INFINITY), index + 9))
            }
            Some(_) => self.number(index),
        }
    }

    fn nested(
        &mut self,
        parse: impl FnOnce(&mut Self) -> Result<(Value, usize), Failure>,
    ) -> Result<(Value, usize), Failure> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err(Failure::Error(ParseError::Value(format!(
                "JSON nests deeper than {MAX_DEPTH} levels"
            ))));
        }
        let result = parse(self);
        self.depth -= 1;
        result
    }

    fn digit(&self, index: usize) -> bool {
        self.at(index).is_some_and(|c| c.is_ascii_digit())
    }

    fn number(&self, start: usize) -> Result<(Value, usize), Failure> {
        let mut index = start;
        if self.at(index) == Some('-') {
            index += 1;
        }
        match self.at(index) {
            Some('1'..='9') => {
                while self.digit(index) {
                    index += 1;
                }
            }
            Some('0') => index += 1,
            _ => return Err(Failure::NoValue(start)),
        }
        let mut float = false;
        if self.at(index) == Some('.') && self.digit(index + 1) {
            float = true;
            index += 2;
            while self.digit(index) {
                index += 1;
            }
        }
        if matches!(self.at(index), Some('e' | 'E')) {
            let exponent = index;
            index += 1;
            if matches!(self.at(index), Some('-' | '+')) {
                index += 1;
            }
            let digits = index;
            while self.digit(index) {
                index += 1;
            }
            if index > digits {
                float = true;
            } else {
                index = exponent;
            }
        }
        let text: String = self.chars[start..index].iter().collect();
        if float {
            let value = text.parse::<f64>().map_err(|error| {
                Failure::Error(ParseError::Value(format!(
                    "cannot read {text} as a float: {error}"
                )))
            })?;
            return Ok((Value::Float(value), index));
        }
        let digits = text.trim_start_matches('-');
        if let Some(message) = int_digit_limit(digits.len()) {
            return Err(Failure::Error(ParseError::Value(message)));
        }
        let normalized = if digits == "0" {
            digits.to_string()
        } else {
            text
        };
        Ok((Value::Int(normalized), index))
    }

    fn hex4(&self, start: usize, error_at: usize) -> Result<u32, Failure> {
        let mut code = 0u32;
        for index in start..start + 4 {
            let digit = self
                .at(index)
                .and_then(|c| c.to_digit(16))
                .ok_or_else(|| self.fail("Invalid \\uXXXX escape", error_at))?;
            code = code * 16 + digit;
        }
        Ok(code)
    }

    /// Python's `scanstring`, from the character after the opening quote.
    fn string(&self, mut end: usize) -> Result<(String, usize), Failure> {
        let begin = end - 1;
        let length = self.chars.len();
        let mut points: Vec<u32> = Vec::new();
        loop {
            let mut next = end;
            let mut terminator = None;
            while next < length {
                let c = self.chars[next];
                if c == '"' || c == '\\' {
                    terminator = Some(c);
                    break;
                }
                if u32::from(c) <= 0x1F {
                    return Err(self.fail("Invalid control character at", next));
                }
                next += 1;
            }
            let Some(terminator) = terminator else {
                return Err(self.fail("Unterminated string starting at", begin));
            };
            points.extend(self.chars[end..next].iter().map(|c| u32::from(*c)));
            next += 1;
            if terminator == '"' {
                end = next;
                break;
            }
            let Some(escape) = self.at(next) else {
                return Err(self.fail("Unterminated string starting at", begin));
            };
            if escape != 'u' {
                end = next + 1;
                let decoded = match escape {
                    '"' => '"',
                    '\\' => '\\',
                    '/' => '/',
                    'b' => '\u{8}',
                    'f' => '\u{c}',
                    'n' => '\n',
                    'r' => '\r',
                    't' => '\t',
                    _ => return Err(self.fail("Invalid \\escape", end - 2)),
                };
                points.push(u32::from(decoded));
                continue;
            }
            next += 1;
            end = next + 4;
            if end > length {
                return Err(self.fail("Invalid \\uXXXX escape", next - 1));
            }
            let mut code = self.hex4(next, end - 5)?;
            if (0xD800..=0xDBFF).contains(&code)
                && end + 6 < length
                && self.at(end) == Some('\\')
                && self.at(end + 1) == Some('u')
            {
                let low = self.hex4(end + 2, end + 1)?;
                if (0xDC00..=0xDFFF).contains(&low) {
                    code = 0x10000 + ((code - 0xD800) << 10) + (low - 0xDC00);
                    end += 6;
                }
            }
            points.push(code);
        }
        let mut text = String::with_capacity(points.len());
        for point in points {
            let Some(c) = char::from_u32(point) else {
                return Err(Failure::Error(ParseError::Value(format!(
                    "the string starting at char {begin} holds a lone surrogate \\u{point:04x}"
                ))));
            };
            text.push(c);
        }
        Ok((text, end))
    }

    fn object(&mut self, start: usize) -> Result<(Value, usize), Failure> {
        let mut pairs: Vec<(String, Value)> = Vec::new();
        let mut index = self.whitespace(start);
        if self.at(index) == Some('}') {
            return Ok((Value::Object(Object::default()), index + 1));
        }
        loop {
            if self.at(index) != Some('"') {
                return Err(self.fail("Expecting property name enclosed in double quotes", index));
            }
            let (key, next) = self.string(index + 1)?;
            index = self.whitespace(next);
            if self.at(index) != Some(':') {
                return Err(self.fail("Expecting ':' delimiter", index));
            }
            index = self.whitespace(index + 1);
            let (value, next) = self.value(index)?;
            pairs.push((key, value));
            index = self.whitespace(next);
            match self.at(index) {
                Some('}') => break,
                Some(',') => {}
                _ => return Err(self.fail("Expecting ',' delimiter", index)),
            }
            let comma = index;
            index = self.whitespace(index + 1);
            if self.at(index) == Some('}') {
                return Err(self.fail("Illegal trailing comma before end of object", comma));
            }
        }
        if self.duplicates == Duplicates::Refuse {
            let mut seen = HashSet::new();
            let mut repeated: Vec<String> = pairs
                .iter()
                .filter(|(key, _)| !seen.insert(key.as_str()))
                .map(|(key, _)| key.clone())
                .collect();
            repeated.sort();
            repeated.dedup();
            if !repeated.is_empty() {
                return Err(Failure::Error(ParseError::Duplicates(repeated)));
            }
        }
        Ok((Value::Object(pairs.into_iter().collect()), index + 1))
    }

    fn array(&mut self, start: usize) -> Result<(Value, usize), Failure> {
        let mut values = Vec::new();
        let mut index = self.whitespace(start);
        if self.at(index) == Some(']') {
            return Ok((Value::Array(values), index + 1));
        }
        loop {
            let (value, next) = self.value(index)?;
            values.push(value);
            index = self.whitespace(next);
            match self.at(index) {
                Some(']') => break,
                Some(',') => {}
                _ => return Err(self.fail("Expecting ',' delimiter", index)),
            }
            let comma = index;
            index = self.whitespace(index + 1);
            if self.at(index) == Some(']') {
                return Err(self.fail("Illegal trailing comma before end of array", comma));
            }
        }
        Ok((Value::Array(values), index + 1))
    }
}

/// `float.__repr__`: the shortest digits that round-trip, positional for exponents -4 through 15.
pub fn float_repr(value: f64) -> String {
    if value.is_nan() {
        return "nan".to_string();
    }
    if value.is_infinite() {
        return if value > 0.0 { "inf" } else { "-inf" }.to_string();
    }
    let sign = if value.is_sign_negative() { "-" } else { "" };
    // `{:e}` finds how many digits round-trip but rounds an exact tie up; Python's `repr` takes the nearest
    // string of that length and rounds a tie to even, as fixed-precision formatting does.
    let shortest = format!("{:e}", value.abs());
    let length = shortest
        .split('e')
        .next()
        .unwrap_or("")
        .chars()
        .filter(char::is_ascii_digit)
        .count();
    let scientific = format!("{:.*e}", length.saturating_sub(1), value.abs());
    let (mantissa, exponent) = scientific.split_once('e').unwrap_or((&scientific, "0"));
    let digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    let exponent: i32 = exponent.parse().unwrap_or(0);
    let point = exponent + 1;
    let count = digits.len() as i32;
    let body = if point <= -4 || point > 16 {
        let (first, rest) = digits.split_at(1);
        let fraction = if rest.is_empty() {
            String::new()
        } else {
            format!(".{rest}")
        };
        let exponent_sign = if exponent < 0 { '-' } else { '+' };
        format!("{first}{fraction}e{exponent_sign}{:02}", exponent.abs())
    } else if point <= 0 {
        format!("0.{}{digits}", "0".repeat((-point) as usize))
    } else if point >= count {
        format!("{digits}{}.0", "0".repeat((point - count) as usize))
    } else {
        let (whole, fraction) = digits.split_at(point as usize);
        format!("{whole}.{fraction}")
    };
    format!("{sign}{body}")
}

#[derive(Clone, Debug)]
pub struct Dumps {
    pub ensure_ascii: bool,
    /// `indent=n`; `Some(0)` still breaks lines, as in Python.
    pub indent: Option<usize>,
    /// `separators=(item, key)`; the default is `(", ", ": ")`, or `(",", ": ")` with an indent.
    pub separators: Option<(String, String)>,
    pub sort_keys: bool,
}

impl Default for Dumps {
    fn default() -> Self {
        Self {
            ensure_ascii: true,
            indent: None,
            separators: None,
            sort_keys: false,
        }
    }
}

/// `json.dumps(value, ...)`.
pub fn dumps(value: &Value, options: &Dumps) -> String {
    let (item, key) = match &options.separators {
        Some((item, key)) => (item.clone(), key.clone()),
        None if options.indent.is_some() => (",".to_string(), ": ".to_string()),
        None => (", ".to_string(), ": ".to_string()),
    };
    let mut out = String::new();
    write_value(&mut out, value, options, &item, &key, 0);
    out
}

fn write_value(
    out: &mut String,
    value: &Value,
    options: &Dumps,
    item: &str,
    key: &str,
    level: usize,
) {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(true) => out.push_str("true"),
        Value::Bool(false) => out.push_str("false"),
        Value::Int(digits) => out.push_str(digits),
        Value::Float(number) if number.is_nan() => out.push_str("NaN"),
        Value::Float(number) if number.is_infinite() => out.push_str(if *number > 0.0 {
            "Infinity"
        } else {
            "-Infinity"
        }),
        Value::Float(number) => out.push_str(&float_repr(*number)),
        Value::Str(text) => write_string(out, text, options.ensure_ascii),
        Value::Array(values) if values.is_empty() => out.push_str("[]"),
        Value::Object(object) if object.is_empty() => out.push_str("{}"),
        Value::Array(values) => {
            out.push('[');
            for (position, element) in values.iter().enumerate() {
                separate(out, options, item, level + 1, position == 0);
                write_value(out, element, options, item, key, level + 1);
            }
            close(out, options, level, ']');
        }
        Value::Object(object) => {
            let mut members: Vec<(&String, &Value)> = object.iter().collect();
            if options.sort_keys {
                members.sort_by(|left, right| left.0.cmp(right.0));
            }
            out.push('{');
            for (position, (name, member)) in members.into_iter().enumerate() {
                separate(out, options, item, level + 1, position == 0);
                write_string(out, name, options.ensure_ascii);
                out.push_str(key);
                write_value(out, member, options, item, key, level + 1);
            }
            close(out, options, level, '}');
        }
    }
}

fn separate(out: &mut String, options: &Dumps, item: &str, level: usize, first: bool) {
    if !first {
        out.push_str(item);
    }
    if let Some(width) = options.indent {
        out.push('\n');
        out.push_str(&" ".repeat(width * level));
    }
}

fn close(out: &mut String, options: &Dumps, level: usize, bracket: char) {
    if let Some(width) = options.indent {
        out.push('\n');
        out.push_str(&" ".repeat(width * level));
    }
    out.push(bracket);
}

fn write_string(out: &mut String, text: &str, ensure_ascii: bool) {
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if u32::from(c) < 0x20 || (ensure_ascii && !(' '..='~').contains(&c)) => {
                let mut units = [0u16; 2];
                for unit in c.encode_utf16(&mut units) {
                    let _ = write!(out, "\\u{unit:04x}");
                }
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

impl Value {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Str(text) => Some(text),
            _ => None,
        }
    }

    pub fn as_object(&self) -> Option<&Object> {
        match self {
            Self::Object(object) => Some(object),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[Value]> {
        match self {
            Self::Array(values) => Some(values),
            _ => None,
        }
    }

    /// `type(value).__name__` of the Python object `json.loads` returns.
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Null => "NoneType",
            Self::Bool(_) => "bool",
            Self::Int(_) => "int",
            Self::Float(_) => "float",
            Self::Str(_) => "str",
            Self::Array(_) => "list",
            Self::Object(_) => "dict",
        }
    }

    /// `bool(value)`.
    pub fn truthy(&self) -> bool {
        match self {
            Self::Null => false,
            Self::Bool(flag) => *flag,
            Self::Int(digits) => digits != "0",
            Self::Float(number) => *number != 0.0,
            Self::Str(text) => !text.is_empty(),
            Self::Array(values) => !values.is_empty(),
            Self::Object(object) => !object.is_empty(),
        }
    }

    /// Whether Python can put the value in a set or use it as a key.
    pub fn hashable(&self) -> bool {
        !matches!(self, Self::Array(_) | Self::Object(_))
    }

    /// The exact integer a number equals, if any, as `int` and `float` compare in Python.
    fn integer(&self) -> Option<String> {
        match self {
            Self::Bool(flag) => Some(if *flag { "1" } else { "0" }.to_string()),
            Self::Int(digits) => Some(digits.clone()),
            Self::Float(number) if number.is_finite() && number.fract() == 0.0 => {
                Some(if *number == 0.0 {
                    "0".to_string()
                } else {
                    format!("{number:.0}")
                })
            }
            _ => None,
        }
    }

    /// `left == right` in Python, where `True == 1 == 1.0` and dicts compare without order.
    pub fn py_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Null, Self::Null) => true,
            (Self::Str(left), Self::Str(right)) => left == right,
            (Self::Float(left), Self::Float(right)) => left == right,
            (Self::Array(left), Self::Array(right)) => {
                left.len() == right.len() && left.iter().zip(right).all(|(a, b)| a.py_eq(b))
            }
            (Self::Object(left), Self::Object(right)) => {
                left.len() == right.len()
                    && left
                        .iter()
                        .all(|(key, value)| right.get(key).is_some_and(|other| value.py_eq(other)))
            }
            (
                Self::Bool(_) | Self::Int(_) | Self::Float(_),
                Self::Bool(_) | Self::Int(_) | Self::Float(_),
            ) => match (self.integer(), other.integer()) {
                (Some(left), Some(right)) => left == right,
                _ => false,
            },
            _ => false,
        }
    }

    /// `repr(value)` of the Python object `json.loads` returns.
    pub fn py_repr(&self) -> String {
        match self {
            Self::Null => "None".to_string(),
            Self::Bool(true) => "True".to_string(),
            Self::Bool(false) => "False".to_string(),
            Self::Int(digits) => digits.clone(),
            Self::Float(number) => float_repr(*number),
            Self::Str(text) => pytext::repr_str(text),
            Self::Array(values) => {
                let inner: Vec<String> = values.iter().map(Self::py_repr).collect();
                format!("[{}]", inner.join(", "))
            }
            Self::Object(object) => {
                let inner: Vec<String> = object
                    .iter()
                    .map(|(key, value)| format!("{}: {}", pytext::repr_str(key), value.py_repr()))
                    .collect();
                format!("{{{}}}", inner.join(", "))
            }
        }
    }

    /// `str(value)`, as an f-string formats it.
    pub fn py_str(&self) -> String {
        match self {
            Self::Str(text) => text.clone(),
            _ => self.py_repr(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn error(text: &str) -> String {
        loads(text, Duplicates::KeepLast).unwrap_err().to_string()
    }

    #[test]
    fn decode_errors_match_python() {
        let cases = [
            (
                "{\"a\":1,}",
                "Illegal trailing comma before end of object: line 1 column 7 (char 6)",
            ),
            (
                "[1,]",
                "Illegal trailing comma before end of array: line 1 column 3 (char 2)",
            ),
            (
                "{\"a\" 1}",
                "Expecting ':' delimiter: line 1 column 6 (char 5)",
            ),
            (
                "{1:2}",
                "Expecting property name enclosed in double quotes: line 1 column 2 (char 1)",
            ),
            ("[1 2]", "Expecting ',' delimiter: line 1 column 4 (char 3)"),
            (
                "\"abc",
                "Unterminated string starting at: line 1 column 1 (char 0)",
            ),
            (
                "\"a\u{1}\"",
                "Invalid control character at: line 1 column 3 (char 2)",
            ),
            ("\"\\q\"", "Invalid \\escape: line 1 column 2 (char 1)"),
            (
                "\"\\u12\"",
                "Invalid \\uXXXX escape: line 1 column 3 (char 2)",
            ),
            (
                "\"\\u12x4\"",
                "Invalid \\uXXXX escape: line 1 column 3 (char 2)",
            ),
            ("\"\\u", "Invalid \\uXXXX escape: line 1 column 3 (char 2)"),
            (
                "\"\\u1234",
                "Unterminated string starting at: line 1 column 1 (char 0)",
            ),
            (
                "\"\\",
                "Unterminated string starting at: line 1 column 1 (char 0)",
            ),
            ("1e", "Extra data: line 1 column 2 (char 1)"),
            ("1.", "Extra data: line 1 column 2 (char 1)"),
            ("-01", "Extra data: line 1 column 3 (char 2)"),
            ("-", "Expecting value: line 1 column 1 (char 0)"),
            ("-Inf", "Expecting value: line 1 column 1 (char 0)"),
            ("nul", "Expecting value: line 1 column 1 (char 0)"),
            ("{\"a\":}", "Expecting value: line 1 column 6 (char 5)"),
            ("[", "Expecting value: line 1 column 2 (char 1)"),
            ("[1,", "Expecting value: line 1 column 4 (char 3)"),
            ("[1,2", "Expecting ',' delimiter: line 1 column 5 (char 4)"),
            (
                "{",
                "Expecting property name enclosed in double quotes: line 1 column 2 (char 1)",
            ),
            (
                "{\"a\":1,",
                "Expecting property name enclosed in double quotes: line 1 column 8 (char 7)",
            ),
            (
                "{\"a\"",
                "Expecting ':' delimiter: line 1 column 5 (char 4)",
            ),
            (
                "\u{feff}{}",
                "Unexpected UTF-8 BOM (decode using utf-8-sig): line 1 column 1 (char 0)",
            ),
            (" ", "Expecting value: line 1 column 2 (char 1)"),
            (
                "{\"a\":1 \"b\":2}",
                "Expecting ',' delimiter: line 1 column 8 (char 7)",
            ),
            (
                "{\"é\": [1]\n \"b\": 2}",
                "Expecting ',' delimiter: line 2 column 2 (char 11)",
            ),
        ];
        for (text, message) in cases {
            assert_eq!(error(text), message, "{text:?}");
        }
        assert!(
            error(&format!("1{}", "0".repeat(4300))).starts_with("Exceeds the limit (4300 digits)")
        );
        assert!(loads(&format!("-{}", "9".repeat(4300)), Duplicates::KeepLast).is_ok());
    }

    #[test]
    fn values_keep_python_semantics() {
        let value = loads(
            "{\"b\": 1, \"a\": [NaN, -Infinity, 1E+2, -0, 9007199254740993, \"\\ud83d\\ude00\"], \"b\": 2}",
            Duplicates::KeepLast,
        )
        .unwrap();
        let object = value.as_object().unwrap();
        assert_eq!(
            object
                .iter()
                .map(|(key, _)| key.as_str())
                .collect::<Vec<_>>(),
            ["b", "a"]
        );
        assert!(object.get("b").unwrap().py_eq(&Value::Int("2".into())));
        assert_eq!(
            object.get("a").unwrap().py_repr(),
            "[nan, -inf, 100.0, 0, 9007199254740993, '\u{1f600}']"
        );
        assert_eq!(
            loads(
                "{\"b\": {\"x\": 1, \"x\": 2, \"a\": 0, \"a\": 1}}",
                Duplicates::Refuse
            )
            .unwrap_err(),
            ParseError::Duplicates(vec!["a".into(), "x".into()])
        );
        assert!(matches!(
            loads("\"\\ud800\"", Duplicates::KeepLast),
            Err(ParseError::Value(_))
        ));
        // Unoptimized frames are large; a release build runs on the 8 MiB main-thread stack.
        let deep = std::thread::Builder::new()
            .stack_size(64 << 20)
            .spawn(|| {
                let nested = format!("{}{}", "[".repeat(MAX_DEPTH), "]".repeat(MAX_DEPTH));
                let refused = loads(&"[".repeat(MAX_DEPTH + 1), Duplicates::KeepLast);
                (
                    loads(&nested, Duplicates::KeepLast).is_ok(),
                    matches!(refused, Err(ParseError::Value(_))),
                )
            })
            .unwrap()
            .join()
            .unwrap();
        assert_eq!(deep, (true, true));
    }

    #[test]
    fn equality_follows_python() {
        let one = Value::Int("1".into());
        assert!(one.py_eq(&Value::Bool(true)));
        assert!(one.py_eq(&Value::Float(1.0)));
        assert!(Value::Int("0".into()).py_eq(&Value::Float(-0.0)));
        assert!(!Value::Int("9007199254740993".into()).py_eq(&Value::Float(9007199254740992.0)));
        assert!(!one.py_eq(&Value::Str("1".into())));
        let left = loads("{\"a\": 1, \"b\": [true]}", Duplicates::KeepLast).unwrap();
        let right = loads("{\"b\": [1.0], \"a\": 1}", Duplicates::KeepLast).unwrap();
        assert!(left.py_eq(&right));
    }

    #[test]
    fn float_repr_matches_python() {
        let cases = [
            (0.0, "0.0"),
            (-0.0, "-0.0"),
            (1.5, "1.5"),
            (100.0, "100.0"),
            (1e16, "1e+16"),
            (1.5e16, "1.5e+16"),
            (1e15, "1000000000000000.0"),
            (123456789012345680.0, "1.2345678901234568e+17"),
            (0.0001, "0.0001"),
            (0.00001, "1e-05"),
            (1.5e-7, "1.5e-07"),
            (4e-5, "4e-05"),
            (1e100, "1e+100"),
            (0.1 + 0.2, "0.30000000000000004"),
            (5e-324, "5e-324"),
            (1.7976931348623157e308, "1.7976931348623157e+308"),
            (0.001045, "0.001045"),
            (f64::from_bits(0xc3139c02f39d9a3d), "-1379890262140559.2"),
            (f64::from_bits(0x4318b888cc0e795d), "1739574279904855.2"),
        ];
        for (value, expected) in cases {
            assert_eq!(float_repr(value), expected);
        }
    }

    #[test]
    fn dumps_matches_python() {
        let value = loads(
            "{\"z\": [1, 2.5, null, true], \"a\": {\"q\\\"\\\\\": \"\\u00e9\\u96ea\\ud83d\\ude00\\u007f\\u0001\\n\"}, \"e\": [], \"o\": {}}",
            Duplicates::KeepLast,
        )
        .unwrap();
        assert_eq!(
            dumps(&value, &Dumps::default()),
            "{\"z\": [1, 2.5, null, true], \"a\": {\"q\\\"\\\\\": \"\\u00e9\\u96ea\\ud83d\\ude00\\u007f\\u0001\\n\"}, \"e\": [], \"o\": {}}"
        );
        assert_eq!(
            dumps(
                &value,
                &Dumps {
                    indent: Some(2),
                    sort_keys: true,
                    ensure_ascii: false,
                    ..Dumps::default()
                }
            ),
            "{\n  \"a\": {\n    \"q\\\"\\\\\": \"é雪\u{1f600}\u{7f}\\u0001\\n\"\n  },\n  \"e\": [],\n  \"o\": {},\n  \"z\": [\n    1,\n    2.5,\n    null,\n    true\n  ]\n}"
        );
        assert_eq!(
            dumps(
                &value,
                &Dumps {
                    indent: Some(0),
                    ..Dumps::default()
                }
            ),
            "{\n\"z\": [\n1,\n2.5,\nnull,\ntrue\n],\n\"a\": {\n\"q\\\"\\\\\": \"\\u00e9\\u96ea\\ud83d\\ude00\\u007f\\u0001\\n\"\n},\n\"e\": [],\n\"o\": {}\n}"
        );
        let compact = Dumps {
            separators: Some((",".into(), ":".into())),
            ..Dumps::default()
        };
        let numbers = loads(
            "[NaN, Infinity, -Infinity, 1e16, -0.0, 12345678901234567890]",
            Duplicates::KeepLast,
        )
        .unwrap();
        assert_eq!(
            dumps(&numbers, &compact),
            "[NaN,Infinity,-Infinity,1e+16,-0.0,12345678901234567890]"
        );
    }
}
