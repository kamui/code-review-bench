//! Python object semantics over the values `pyjson` reads, where an output or a diagnostic depends on them:
//! `dict` and `set` membership as `==` and hashing decide it, iteration over the container `json.loads`
//! returned, truthiness, `int` arithmetic of any size, `sum` as Python 3.14 adds, and `datetime.fromisoformat`
//! as Python 3.14 parses. A failure is the last line of the traceback Python would print.

use crate::pyjson::{self, Duplicates, Object, Value};
use crate::pytext::{self, PyPath};
use std::borrow::Cow;
use std::collections::HashMap;
use std::io;

#[derive(Debug, PartialEq, Eq)]
pub struct Failure {
    pub(crate) kind: &'static str,
    pub(crate) message: String,
}

impl Failure {
    pub(crate) fn new(kind: &'static str, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    /// The last line of the traceback Python printed for the same failure.
    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn line(&self) -> String {
        if self.message.is_empty() {
            self.kind.to_string()
        } else {
            format!("{}: {}", self.kind, self.message)
        }
    }
}

pub type Result<T> = std::result::Result<T, Failure>;

pub(crate) fn type_error(message: impl Into<String>) -> Failure {
    Failure::new("TypeError", message)
}

pub(crate) fn os_failure(error: &io::Error, message: String) -> Failure {
    let kind = match error.kind() {
        io::ErrorKind::NotFound => "FileNotFoundError",
        io::ErrorKind::PermissionDenied => "PermissionError",
        io::ErrorKind::AlreadyExists => "FileExistsError",
        io::ErrorKind::IsADirectory => "IsADirectoryError",
        io::ErrorKind::NotADirectory => "NotADirectoryError",
        _ => "OSError",
    };
    Failure::new(kind, message)
}

pub(crate) fn file_failure(error: &io::Error, path: &[u8]) -> Failure {
    os_failure(error, pytext::os_error(error, path))
}

/// An `os.replace` error, which names both paths.
pub(crate) fn move_failure(error: &io::Error, from: &[u8], to: &[u8]) -> Failure {
    let quoted = pytext::repr(&pytext::surrogateescape(to));
    os_failure(
        error,
        format!("{} -> {quoted}", pytext::os_error(error, from)),
    )
}

pub(crate) fn read_bytes(path: &PyPath) -> Result<Vec<u8>> {
    std::fs::read(path.to_path_buf()).map_err(|error| file_failure(&error, path.as_bytes()))
}

/// `Path.read_text(encoding="utf-8")`.
pub(crate) fn read_text(path: &PyPath) -> Result<String> {
    let bytes = read_bytes(path)?;
    let text = pytext::decode_utf8(&bytes)
        .map_err(|message| Failure::new("UnicodeDecodeError", message))?;
    Ok(pytext::universal_newlines(&text))
}

pub(crate) fn loads(text: &str) -> Result<Value> {
    pyjson::loads(text, Duplicates::KeepLast).map_err(|error| match error {
        pyjson::ParseError::Syntax { .. } => {
            Failure::new("json.decoder.JSONDecodeError", error.to_string())
        }
        other => Failure::new("ValueError", other.to_string()),
    })
}

pub(crate) fn read(path: &PyPath) -> Result<Value> {
    loads(&read_text(path)?)
}

pub(crate) fn text(value: &str) -> Value {
    Value::Str(value.to_string())
}

pub(crate) fn object(pairs: Vec<(&str, Value)>) -> Value {
    Value::Object(
        pairs
            .into_iter()
            .map(|(key, value)| (key.to_string(), value))
            .collect::<Object>(),
    )
}

pub(crate) fn unhashable(value: &Value) -> Failure {
    type_error(format!("unhashable type: '{}'", value.type_name()))
}

/// `container[key]`.
pub(crate) fn index<'v>(container: &'v Value, key: &Value) -> Result<&'v Value> {
    match container {
        Value::Object(entries) => {
            if !key.hashable() {
                return Err(unhashable(key));
            }
            key.as_str()
                .and_then(|name| entries.get(name))
                .ok_or_else(|| Failure::new("KeyError", key.py_repr()))
        }
        Value::Array(_) => Err(type_error(format!(
            "list indices must be integers or slices, not {}",
            key.type_name()
        ))),
        Value::Str(_) => Err(type_error(format!(
            "string indices must be integers, not '{}'",
            key.type_name()
        ))),
        other => Err(type_error(format!(
            "'{}' object is not subscriptable",
            other.type_name()
        ))),
    }
}

pub(crate) fn at<'v>(container: &'v Value, key: &str) -> Result<&'v Value> {
    index(container, &text(key))
}

/// `container.get(key)`.
pub(crate) fn lookup<'v>(container: &'v Value, key: &Value) -> Result<Option<&'v Value>> {
    match container {
        Value::Object(entries) => {
            if !key.hashable() {
                return Err(unhashable(key));
            }
            Ok(key.as_str().and_then(|name| entries.get(name)))
        }
        other => Err(Failure::new(
            "AttributeError",
            format!("'{}' object has no attribute 'get'", other.type_name()),
        )),
    }
}

pub(crate) fn get<'v>(container: &'v Value, key: &str) -> Result<Option<&'v Value>> {
    lookup(container, &text(key))
}

/// `container.get(key, default)`.
pub(crate) fn get_or(container: &Value, key: &str, default: Value) -> Result<Value> {
    Ok(get(container, key)?.cloned().unwrap_or(default))
}

/// `key in container`.
pub(crate) fn contains(container: &Value, key: &str) -> Result<bool> {
    match container {
        Value::Object(entries) => Ok(entries.get(key).is_some()),
        Value::Array(items) => Ok(items.iter().any(|item| item.py_eq(&text(key)))),
        Value::Str(haystack) => Ok(haystack.contains(key)),
        other => Err(type_error(format!(
            "argument of type '{}' is not iterable",
            other.type_name()
        ))),
    }
}

/// The items `for item in value` visits: a list's elements, a dict's keys or a string's characters.
pub(crate) fn iterate(value: &Value) -> Result<Cow<'_, [Value]>> {
    match value {
        Value::Array(items) => Ok(Cow::Borrowed(items)),
        Value::Object(entries) => Ok(Cow::Owned(
            entries.iter().map(|(key, _)| text(key)).collect(),
        )),
        Value::Str(characters) => Ok(Cow::Owned(
            characters
                .chars()
                .map(|c| Value::Str(c.to_string()))
                .collect(),
        )),
        other => Err(type_error(format!(
            "'{}' object is not iterable",
            other.type_name()
        ))),
    }
}

/// `len(value)`.
pub(crate) fn length(value: &Value) -> Result<Value> {
    let count = match value {
        Value::Array(items) => items.len(),
        Value::Object(entries) => entries.len(),
        Value::Str(characters) => characters.chars().count(),
        other => {
            return Err(type_error(format!(
                "object of type '{}' has no len()",
                other.type_name()
            )));
        }
    };
    Ok(Value::Int(count.to_string()))
}

pub(crate) fn count(items: usize) -> Value {
    Value::Int(items.to_string())
}

/// `path / segment`.
pub(crate) fn join(path: &PyPath, segment: &Value) -> Result<PyPath> {
    match segment {
        Value::Str(part) => Ok(path.join(part.as_bytes())),
        other => Err(type_error(format!(
            "unsupported operand type(s) for /: 'PosixPath' and '{}'",
            other.type_name()
        ))),
    }
}

/// `Path(value)`.
pub(crate) fn path_of(value: &Value) -> Result<PyPath> {
    match value {
        Value::Str(part) => Ok(PyPath::new(part.as_bytes())),
        other => Err(type_error(format!(
            "argument should be a str or an os.PathLike object where __fspath__ returns a str, not '{}'",
            other.type_name()
        ))),
    }
}

/// `left + right` for two strings.
pub(crate) fn concat(left: &Value, right: &Value) -> Result<Value> {
    match (left, right) {
        (Value::Str(a), Value::Str(b)) => Ok(Value::Str(format!("{a}{b}"))),
        (Value::Str(_), other) => Err(type_error(format!(
            "can only concatenate str (not \"{}\") to str",
            other.type_name()
        ))),
        (other, _) => Err(type_error(format!(
            "unsupported operand type(s) for +: '{}' and 'str'",
            other.type_name()
        ))),
    }
}

pub(crate) fn str_text(value: &Value) -> Result<&str> {
    value
        .as_str()
        .ok_or_else(|| type_error(format!("expected str, not {}", value.type_name())))
}

/// A `dict` or `set` keyed by JSON values, as `==` and hashing decide membership.
pub(crate) struct Dict<V> {
    entries: Vec<(Value, V)>,
    text: HashMap<String, usize>,
    other: Vec<usize>,
}

impl<V> Dict<V> {
    pub(crate) fn new() -> Self {
        Self {
            entries: Vec::new(),
            text: HashMap::new(),
            other: Vec::new(),
        }
    }

    pub(crate) fn position(&self, key: &Value) -> Result<Option<usize>> {
        if !key.hashable() {
            return Err(unhashable(key));
        }
        Ok(match key {
            Value::Str(name) => self.text.get(name).copied(),
            _ => self
                .other
                .iter()
                .copied()
                .find(|at| self.entries[*at].0.py_eq(key)),
        })
    }

    /// `dict[key] = value`: an existing key keeps its first position.
    pub(crate) fn insert(&mut self, key: Value, value: V) -> Result<()> {
        match self.position(&key)? {
            Some(at) => self.entries[at].1 = value,
            None => {
                let at = self.entries.len();
                match &key {
                    Value::Str(name) => {
                        self.text.insert(name.clone(), at);
                    }
                    _ => self.other.push(at),
                }
                self.entries.push((key, value));
            }
        }
        Ok(())
    }

    pub(crate) fn get(&self, key: &Value) -> Result<Option<&V>> {
        Ok(self.position(key)?.map(|at| &self.entries[at].1))
    }

    pub(crate) fn get_mut(&mut self, key: &Value) -> Result<Option<&mut V>> {
        Ok(self.position(key)?.map(|at| &mut self.entries[at].1))
    }

    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    pub(crate) fn keys(&self) -> impl Iterator<Item = &Value> {
        self.entries.iter().map(|(key, _)| key)
    }

    pub(crate) fn values(&self) -> impl Iterator<Item = &V> {
        self.entries.iter().map(|(_, value)| value)
    }
}

/// A `set` of JSON values.
pub(crate) fn set_of<'v>(values: impl IntoIterator<Item = &'v Value>) -> Result<Dict<()>> {
    let mut set = Dict::new();
    for value in values {
        set.insert(value.clone(), ())?;
    }
    Ok(set)
}

/// `sorted(values)` for text; other values are refused rather than ordered as Python orders numbers.
pub(crate) fn sorted(mut values: Vec<Value>) -> Result<Vec<Value>> {
    if values.len() < 2 {
        return Ok(values);
    }
    if let Some(other) = values.iter().find(|value| value.as_str().is_none()) {
        let first = &values[0];
        let (left, right) = if first.as_str().is_none() {
            (first, &values[1])
        } else {
            (other, first)
        };
        return Err(type_error(format!(
            "'<' not supported between instances of '{}' and '{}'",
            left.type_name(),
            right.type_name()
        )));
    }
    values.sort_by(|a, b| a.as_str().cmp(&b.as_str()));
    Ok(values)
}

/// `", ".join(values)`.
pub(crate) fn join_text(values: &[Value]) -> Result<String> {
    let mut parts = Vec::with_capacity(values.len());
    for (position, value) in values.iter().enumerate() {
        match value {
            Value::Str(part) => parts.push(part.as_str()),
            other => {
                return Err(type_error(format!(
                    "sequence item {position}: expected str instance, {} found",
                    other.type_name()
                )));
            }
        }
    }
    Ok(parts.join(", "))
}

/// `left or right`.
pub(crate) fn or(left: Option<&Value>, right: Value) -> Value {
    match left {
        Some(value) if value.truthy() => value.clone(),
        _ => right,
    }
}

pub(crate) fn optional(value: Option<&Value>) -> Value {
    value.cloned().unwrap_or(Value::Null)
}

/// `int` arithmetic beyond 64 bits: a sign and base-10^9 limbs, least significant first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Big {
    negative: bool,
    limbs: Vec<u32>,
}

const LIMB: u64 = 1_000_000_000;

impl Big {
    pub(crate) fn parse(digits: &str) -> Self {
        let (negative, body) = match digits.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, digits),
        };
        let bytes = body.as_bytes();
        let mut limbs = Vec::new();
        let mut end = bytes.len();
        while end > 0 {
            let start = end.saturating_sub(9);
            let chunk = std::str::from_utf8(&bytes[start..end]).unwrap_or("0");
            limbs.push(chunk.parse().unwrap_or(0));
            end = start;
        }
        let mut big = Self { negative, limbs };
        big.trim();
        big
    }

    pub(crate) fn from_i64(value: i64) -> Self {
        Self::parse(&value.to_string())
    }

    fn trim(&mut self) {
        while self.limbs.last() == Some(&0) {
            self.limbs.pop();
        }
        if self.limbs.is_empty() {
            self.negative = false;
        }
    }

    fn compare_magnitude(a: &[u32], b: &[u32]) -> std::cmp::Ordering {
        a.len()
            .cmp(&b.len())
            .then_with(|| a.iter().rev().cmp(b.iter().rev()))
    }

    pub(crate) fn add(&self, other: &Self) -> Self {
        let mut result = if self.negative == other.negative {
            let mut limbs = Vec::new();
            let mut carry = 0;
            for at in 0..self.limbs.len().max(other.limbs.len()) {
                let sum = u64::from(*self.limbs.get(at).unwrap_or(&0))
                    + u64::from(*other.limbs.get(at).unwrap_or(&0))
                    + carry;
                limbs.push((sum % LIMB) as u32);
                carry = sum / LIMB;
            }
            if carry > 0 {
                limbs.push(carry as u32);
            }
            Self {
                negative: self.negative,
                limbs,
            }
        } else {
            let (larger, smaller) = if Self::compare_magnitude(&self.limbs, &other.limbs).is_ge() {
                (self, other)
            } else {
                (other, self)
            };
            let mut limbs = Vec::new();
            let mut borrow = 0i64;
            for at in 0..larger.limbs.len() {
                let mut difference = i64::from(larger.limbs[at])
                    - i64::from(*smaller.limbs.get(at).unwrap_or(&0))
                    - borrow;
                borrow = 0;
                if difference < 0 {
                    difference += LIMB as i64;
                    borrow = 1;
                }
                limbs.push(difference as u32);
            }
            Self {
                negative: larger.negative,
                limbs,
            }
        };
        result.trim();
        result
    }

    pub(crate) fn digits(&self) -> String {
        let Some((last, rest)) = self.limbs.split_last() else {
            return "0".to_string();
        };
        let mut out = String::new();
        if self.negative {
            out.push('-');
        }
        out.push_str(&last.to_string());
        for limb in rest.iter().rev() {
            out.push_str(&format!("{limb:09}"));
        }
        out
    }

    /// `float(int)`, correctly rounded, with the `OverflowError` Python raises past the largest float.
    pub(crate) fn to_float(&self) -> Result<f64> {
        let value: f64 = self.digits().parse().unwrap_or(f64::INFINITY);
        if value.is_finite() {
            Ok(value)
        } else {
            Err(Failure::new(
                "OverflowError",
                "int too large to convert to float",
            ))
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) enum Number {
    Int(Big),
    Float(f64),
}

impl Number {
    /// `left + right` as `PyNumber_Add` adds an `int` or `float`.
    pub(crate) fn add(&self, other: &Self) -> Result<Self> {
        Ok(match (self, other) {
            (Self::Int(a), Self::Int(b)) => Self::Int(a.add(b)),
            (Self::Int(a), Self::Float(b)) => Self::Float(a.to_float()? + b),
            (Self::Float(a), Self::Int(b)) => Self::Float(a + b.to_float()?),
            (Self::Float(a), Self::Float(b)) => Self::Float(a + b),
        })
    }
}

/// An item `sum` reads: `bool` is an `int`.
pub(crate) fn number(value: &Value) -> Option<Number> {
    match value {
        Value::Bool(flag) => Some(Number::Int(Big::from_i64(i64::from(*flag)))),
        Value::Int(digits) => Some(Number::Int(Big::parse(digits))),
        Value::Float(float) => Some(Number::Float(*float)),
        _ => None,
    }
}

pub(crate) fn small(value: &Number) -> Option<i64> {
    match value {
        Number::Int(big) => big.digits().parse().ok(),
        Number::Float(_) => None,
    }
}

/// Neumaier's compensated sum, as Python 3.14's `sum` keeps it for floats.
#[derive(Clone, Copy)]
pub(crate) struct Compensated {
    hi: f64,
    lo: f64,
}

impl Compensated {
    pub(crate) fn add(mut self, x: f64) -> Self {
        let t = self.hi + x;
        if self.hi.abs() >= x.abs() {
            self.lo += (self.hi - t) + x;
        } else {
            self.lo += (x - t) + self.hi;
        }
        self.hi = t;
        self
    }

    pub(crate) fn total(self) -> f64 {
        if self.lo != 0.0 && self.lo.is_finite() {
            self.hi + self.lo
        } else {
            self.hi
        }
    }
}

/// `sum(values)` with Python 3.14's fast paths: exact 64-bit integers, then a compensated float sum that
/// adds integers as correctly rounded floats, then ordinary `+`.
pub(crate) fn python_sum(values: &[Number]) -> Result<Number> {
    let mut items = values.iter();
    let mut integer: i64 = 0;
    let mut result: Number = loop {
        let Some(item) = items.next() else {
            return Ok(Number::Int(Big::from_i64(integer)));
        };
        if let Some(value) = small(item)
            && let Some(sum) = integer.checked_add(value)
        {
            integer = sum;
            continue;
        }
        break Number::Int(Big::from_i64(integer)).add(item)?;
    };
    if let Number::Float(start) = result {
        let mut total = Compensated { hi: start, lo: 0.0 };
        for item in items.by_ref() {
            total = match item {
                Number::Float(x) => total.add(*x),
                Number::Int(big) => total.add(big.to_float()?),
            };
        }
        return Ok(Number::Float(total.total()));
    }
    for item in items {
        result = result.add(item)?;
    }
    Ok(result)
}

pub(crate) fn number_value(value: Number) -> Result<Value> {
    Ok(match value {
        Number::Float(float) => Value::Float(float),
        Number::Int(big) => {
            let digits = big.digits();
            if let Some(message) = pyjson::int_digit_limit(digits.trim_start_matches('-').len()) {
                return Err(Failure::new("ValueError", message));
            }
            Value::Int(digits)
        }
    })
}

/// Days before a month, as `datetime` counts them.
const DAYS_BEFORE_MONTH: [i64; 13] = [0, 0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334];
const DAYS_IN_MONTH: [i64; 13] = [0, 31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];

fn is_leap(year: i64) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

fn days_in_month(year: i64, month: i64) -> i64 {
    if month == 2 && is_leap(year) {
        29
    } else {
        DAYS_IN_MONTH[month as usize]
    }
}

fn ymd_to_ord(year: i64, month: i64, day: i64) -> i64 {
    let y = year - 1;
    let leap = i64::from(month > 2 && is_leap(year));
    y * 365 + y / 4 - y / 100 + y / 400 + DAYS_BEFORE_MONTH[month as usize] + leap + day
}

fn ord_to_ymd(ordinal: i64) -> (i64, i64, i64) {
    let mut n = ordinal - 1;
    let n400 = n / 146_097;
    n %= 146_097;
    let mut year = n400 * 400 + 1;
    let n100 = n / 36_524;
    n %= 36_524;
    let n4 = n / 1_461;
    n %= 1_461;
    let n1 = n / 365;
    n %= 365;
    year += n100 * 100 + n4 * 4 + n1;
    if n1 == 4 || n100 == 4 {
        return (year - 1, 12, 31);
    }
    let leap = n1 == 3 && (n4 != 24 || n100 == 3);
    let mut month = (n + 50) >> 5;
    let mut preceding = DAYS_BEFORE_MONTH[month as usize] + i64::from(month > 2 && leap);
    if preceding > n {
        month -= 1;
        preceding -= days_in_month(year, month);
    }
    (year, month, n - preceding + 1)
}

/// `iso_to_ymd`: an ISO calendar date, or `None` where Python raises.
fn iso_to_ymd(iso_year: i64, week: i64, weekday: i64) -> Option<(i64, i64, i64)> {
    if !(1..=9999).contains(&iso_year) {
        return None;
    }
    if week <= 0 || week >= 53 {
        let first_weekday = (ymd_to_ord(iso_year, 1, 1) + 6) % 7;
        let long_year = first_weekday == 3 || (first_weekday == 2 && is_leap(iso_year));
        if week != 53 || !long_year {
            return None;
        }
    }
    if !(1..=7).contains(&weekday) {
        return None;
    }
    let first_day = ymd_to_ord(iso_year, 1, 1);
    let first_weekday = (first_day + 6) % 7;
    let mut monday = first_day - first_weekday;
    if first_weekday > 3 {
        monday += 7;
    }
    Some(ord_to_ymd(monday + (week - 1) * 7 + weekday - 1))
}

/// The bytes of a C string: a read past the end finds the terminating NUL.
struct Bytes<'a>(&'a [u8]);

impl Bytes<'_> {
    fn at(&self, position: usize) -> u8 {
        self.0.get(position).copied().unwrap_or(0)
    }

    /// `parse_digits`: the position after `count` ASCII digits, or `None`.
    fn digits(&self, mut position: usize, count: usize, value: &mut i64) -> Option<usize> {
        for _ in 0..count {
            let digit = self.at(position);
            if !digit.is_ascii_digit() {
                return None;
            }
            *value = *value * 10 + i64::from(digit - b'0');
            position += 1;
        }
        Some(position)
    }
}

/// `_find_isoformat_datetime_separator`, or `None` where it returns -1.
fn separator(text: &Bytes<'_>) -> Option<usize> {
    let length = text.0.len();
    if length == 7 {
        return Some(7);
    }
    if text.at(4) == b'-' {
        if text.at(5) != b'W' {
            return Some(10);
        }
        if length < 8 {
            return None;
        }
        if length > 8 && text.at(8) == b'-' {
            if length == 9 {
                return None;
            }
            if length > 10 && text.at(10).is_ascii_digit() {
                return Some(8);
            }
            return Some(10);
        }
        return Some(8);
    }
    if text.at(4) == b'W' {
        let mut position = 7;
        while position < length && text.at(position).is_ascii_digit() {
            position += 1;
        }
        if position < 9 {
            return Some(position);
        }
        return Some(if position % 2 == 0 { 7 } else { 8 });
    }
    Some(8)
}

/// `parse_isoformat_date` over the first `length` bytes.
fn parse_date(text: &Bytes<'_>, length: usize) -> Option<(i64, i64, i64)> {
    let mut year = 0;
    let mut position = text.digits(0, 4, &mut year)?;
    let separated = text.at(position) == b'-';
    if separated {
        position += 1;
    }
    if text.at(position) == b'W' {
        position += 1;
        let mut week = 0;
        position = text.digits(position, 2, &mut week)?;
        let mut weekday = 0;
        if position < length {
            if separated {
                if text.at(position) != b'-' {
                    return None;
                }
                position += 1;
            }
            text.digits(position, 1, &mut weekday)?;
        } else {
            weekday = 1;
        }
        return iso_to_ymd(year, week, weekday);
    }
    let mut month = 0;
    position = text.digits(position, 2, &mut month)?;
    if separated {
        if text.at(position) != b'-' {
            return None;
        }
        position += 1;
    }
    let mut day = 0;
    text.digits(position, 2, &mut day)?;
    Some((year, month, day))
}

/// `parse_hh_mm_ss_ff` from `start` to `end`: the fields and whether text follows them, or `None`.
fn parse_clock(text: &Bytes<'_>, start: usize, end: usize) -> Option<([i64; 4], bool)> {
    let mut fields = [0i64; 4];
    let mut position = start;
    let mut separated = true;
    for field in 0..3 {
        position = text.digits(position, 2, &mut fields[field])?;
        let c = text.at(position);
        position += 1;
        if field == 0 {
            separated = c == b':';
        }
        if position >= end {
            if c == b'.' || c == b',' {
                return None;
            }
            return Some((fields, c != 0));
        } else if separated && c == b':' {
            if field == 2 {
                return None;
            }
        } else if c == b'.' || c == b',' {
            if field < 2 {
                return None;
            }
            break;
        } else if !separated {
            position -= 1;
        } else {
            return None;
        }
    }
    let remaining = end - position;
    let parsed = remaining.min(6);
    position = text.digits(position, parsed, &mut fields[3])?;
    if parsed < 6 {
        fields[3] *= 10i64.pow((6 - parsed) as u32);
    }
    while text.at(position).is_ascii_digit() {
        position += 1;
    }
    Some((fields, text.at(position) != 0))
}

/// A parsed time: the clock fields and the UTC offset in microseconds, if any.
type Clock = ([i64; 4], Option<i64>);

/// `parse_isoformat_time` from `start` to the end of the text.
fn parse_time(text: &Bytes<'_>, start: usize) -> Option<Clock> {
    let end = text.0.len();
    let mut zone = start;
    loop {
        if matches!(text.at(zone), b'Z' | b'+' | b'-') {
            break;
        }
        zone += 1;
        if zone >= end {
            break;
        }
    }
    let (fields, trailing) = parse_clock(text, start, zone)?;
    if zone == end {
        return if trailing { None } else { Some((fields, None)) };
    }
    if text.at(zone) == b'Z' {
        return if text.at(zone + 1) != 0 {
            None
        } else {
            Some((fields, Some(0)))
        };
    }
    let sign = if text.at(zone) == b'-' { -1 } else { 1 };
    let (offset, trailing) = parse_clock(text, zone + 1, end)?;
    if trailing {
        return None;
    }
    let seconds = offset[0] * 3600 + offset[1] * 60 + offset[2];
    Some((fields, Some(sign * (seconds * 1_000_000 + offset[3]))))
}

/// `datetime.fromisoformat(text)` as Python 3.14 parses it: the instant in microseconds, counted from the
/// proleptic Gregorian ordinal 0 in UTC when an offset is given, and whether one is; `None` where it raises
/// `ValueError`.
pub(crate) fn fromisoformat(value: &str) -> Option<(i64, bool)> {
    if value.chars().count() < 7 {
        return None;
    }
    let text = Bytes(value.as_bytes());
    let length = text.0.len();
    let split = separator(&text);
    let date_length = split.unwrap_or(usize::MAX);
    let (mut year, mut month, mut day) = parse_date(&text, date_length)?;
    let ([mut hour, minute, second, microsecond], offset) = match split {
        Some(at) if length > at => {
            let lead = text.at(at);
            let width = if lead & 0x80 == 0 {
                1
            } else {
                match lead & 0xf0 {
                    0xe0 => 3,
                    0xf0 => 4,
                    _ => 2,
                }
            };
            parse_time(&text, at + width)?
        }
        None => return None,
        _ => ([0; 4], None),
    };
    if hour == 24 && (1..=12).contains(&month) && day <= days_in_month(year, month) {
        if minute != 0 || second != 0 || microsecond != 0 {
            return None;
        }
        hour = 0;
        day += 1;
        if day > days_in_month(year, month) {
            day = 1;
            month += 1;
            if month > 12 {
                month = 1;
                year += 1;
            }
        }
    }
    let valid = (1..=9999).contains(&year)
        && (1..=12).contains(&month)
        && day >= 1
        && day <= days_in_month(year, month)
        && (0..24).contains(&hour)
        && (0..60).contains(&minute)
        && (0..60).contains(&second)
        && (0..1_000_000).contains(&microsecond);
    if !valid {
        return None;
    }
    if let Some(offset) = offset
        && offset.abs() >= 86_400_000_000
    {
        return None;
    }
    let seconds = ymd_to_ord(year, month, day) * 86_400 + hour * 3_600 + minute * 60 + second;
    let instant = seconds * 1_000_000 + microsecond - offset.unwrap_or(0);
    Some((instant, offset.is_some()))
}

/// An `int` magnitude in base 2^32, least significant limb first, without leading zero limbs.
type Binary = Vec<u32>;

fn trimmed(mut magnitude: Binary) -> Binary {
    while magnitude.last() == Some(&0) {
        magnitude.pop();
    }
    magnitude
}

fn bit_length(magnitude: &[u32]) -> i64 {
    magnitude.last().map_or(0, |top| {
        (magnitude.len() as i64 - 1) * 32 + i64::from(32 - top.leading_zeros())
    })
}

fn multiply_add(magnitude: &mut Binary, factor: u32, addend: u32) {
    let mut carry = u64::from(addend);
    for limb in magnitude.iter_mut() {
        let value = u64::from(*limb) * u64::from(factor) + carry;
        *limb = value as u32;
        carry = value >> 32;
    }
    if carry > 0 {
        magnitude.push(carry as u32);
    }
}

/// Divides in place and returns the remainder.
fn divide_small(magnitude: &mut Binary, divisor: u64) -> u64 {
    let mut remainder: u128 = 0;
    for limb in magnitude.iter_mut().rev() {
        let current = (remainder << 32) | u128::from(*limb);
        *limb = (current / u128::from(divisor)) as u32;
        remainder = current % u128::from(divisor);
    }
    *magnitude = trimmed(std::mem::take(magnitude));
    remainder as u64
}

fn shifted_left(magnitude: &[u32], bits: i64) -> Binary {
    let words = (bits / 32) as usize;
    let shift = (bits % 32) as u32;
    let mut out = vec![0; words];
    let mut carry = 0u32;
    for limb in magnitude {
        if shift == 0 {
            out.push(*limb);
        } else {
            out.push((limb << shift) | carry);
            carry = limb >> (32 - shift);
        }
    }
    out.push(carry);
    trimmed(out)
}

/// The magnitude shifted right, and whether a nonzero bit was dropped.
fn shifted_right(magnitude: &[u32], bits: i64) -> (Binary, bool) {
    let words = (bits / 32) as usize;
    let shift = (bits % 32) as u32;
    if words >= magnitude.len() {
        return (Vec::new(), magnitude.iter().any(|limb| *limb != 0));
    }
    let mut lost = magnitude[..words].iter().any(|limb| *limb != 0);
    if shift > 0 {
        lost |= magnitude[words] & ((1u32 << shift) - 1) != 0;
    }
    let out = (words..magnitude.len())
        .map(|at| {
            let low = magnitude[at] >> shift;
            let high = match magnitude.get(at + 1) {
                Some(next) if shift > 0 => next << (32 - shift),
                _ => 0,
            };
            low | high
        })
        .collect();
    (trimmed(out), lost)
}

fn add_binary(left: &[u32], right: &[u32]) -> Binary {
    let mut out = Vec::with_capacity(left.len().max(right.len()) + 1);
    let mut carry = 0u64;
    for at in 0..left.len().max(right.len()) {
        let sum = u64::from(*left.get(at).unwrap_or(&0))
            + u64::from(*right.get(at).unwrap_or(&0))
            + carry;
        out.push(sum as u32);
        carry = sum >> 32;
    }
    out.push(carry as u32);
    trimmed(out)
}

fn compare_binary(left: &[u32], right: &[u32]) -> std::cmp::Ordering {
    left.len()
        .cmp(&right.len())
        .then_with(|| left.iter().rev().cmp(right.iter().rev()))
}

/// `larger - smaller` for magnitudes with `larger >= smaller`.
fn subtract_binary(larger: &[u32], smaller: &[u32]) -> Binary {
    let mut out = Vec::with_capacity(larger.len());
    let mut borrow = 0i64;
    for (at, limb) in larger.iter().enumerate() {
        let mut difference = i64::from(*limb) - i64::from(*smaller.get(at).unwrap_or(&0)) - borrow;
        borrow = 0;
        if difference < 0 {
            difference += 1 << 32;
            borrow = 1;
        }
        out.push(difference as u32);
    }
    trimmed(out)
}

/// A signed exact value: `(-1)^negative * magnitude`.
#[derive(Clone, Debug, Default)]
struct Signed {
    negative: bool,
    magnitude: Binary,
}

impl Signed {
    fn add(&self, other: &Self) -> Self {
        if self.negative == other.negative {
            return Self {
                negative: self.negative,
                magnitude: add_binary(&self.magnitude, &other.magnitude),
            };
        }
        let (larger, smaller) = if compare_binary(&self.magnitude, &other.magnitude).is_ge() {
            (self, other)
        } else {
            (other, self)
        };
        let magnitude = subtract_binary(&larger.magnitude, &smaller.magnitude);
        Self {
            negative: larger.negative && !magnitude.is_empty(),
            magnitude,
        }
    }

    fn compare(&self, other: &Self) -> std::cmp::Ordering {
        use std::cmp::Ordering;
        let zero = |value: &Self| value.magnitude.is_empty();
        match (self.negative && !zero(self), other.negative && !zero(other)) {
            (false, true) => Ordering::Greater,
            (true, false) => Ordering::Less,
            (false, false) => compare_binary(&self.magnitude, &other.magnitude),
            (true, true) => compare_binary(&other.magnitude, &self.magnitude),
        }
    }
}

impl Big {
    fn binary(&self) -> Signed {
        let mut magnitude = Vec::new();
        for limb in self.limbs.iter().rev() {
            multiply_add(&mut magnitude, LIMB as u32, *limb);
        }
        Signed {
            negative: self.negative,
            magnitude: trimmed(magnitude),
        }
    }

    fn from_binary(value: &Signed) -> Self {
        let mut rest = value.magnitude.clone();
        let mut limbs = Vec::new();
        while !rest.is_empty() {
            limbs.push(divide_small(&mut rest, LIMB) as u32);
        }
        let mut big = Self {
            negative: value.negative,
            limbs,
        };
        big.trim();
        big
    }
}

/// A finite float as an exact `mantissa * 2^exponent`.
fn float_parts(value: f64) -> (Signed, i64) {
    let bits = value.to_bits();
    let field = ((bits >> 52) & 0x7ff) as i64;
    let fraction = bits & ((1 << 52) - 1);
    let (mantissa, exponent) = if field == 0 {
        (fraction, -1074)
    } else {
        (fraction | (1 << 52), field - 1075)
    };
    let magnitude = trimmed(vec![mantissa as u32, (mantissa >> 32) as u32]);
    (
        Signed {
            negative: value.is_sign_negative(),
            magnitude,
        },
        exponent,
    )
}

/// `numerator / denominator * 2^exponent`, correctly rounded to a float with ties to even, as Python divides
/// two `int`s; `None` past the largest float. A zero numerator gives `0.0`.
fn ratio_to_float(numerator: &Signed, denominator: u64, exponent: i64) -> Option<f64> {
    if numerator.magnitude.is_empty() {
        return Some(0.0);
    }
    let denominator_bits = i64::from(64 - denominator.leading_zeros());
    let shift = 56 - (bit_length(&numerator.magnitude) - denominator_bits);
    let (mut scaled, mut inexact) = if shift >= 0 {
        (shifted_left(&numerator.magnitude, shift), false)
    } else {
        shifted_right(&numerator.magnitude, -shift)
    };
    inexact |= divide_small(&mut scaled, denominator) != 0;
    let quotient = scaled
        .iter()
        .rev()
        .fold(0u128, |acc, limb| (acc << 32) | u128::from(*limb));
    let length = i64::from(128 - quotient.leading_zeros());
    let top = exponent - shift + length - 1;
    let precision = if top >= -1022 { 53 } else { 1075 + top };
    let drop = length - precision;
    let sign = if numerator.negative { -1.0 } else { 1.0 };
    if drop > length + 1 {
        return Some(sign * 0.0);
    }
    let half = 1u128 << (drop - 1);
    let low = quotient & ((half << 1) - 1);
    let mut kept = quotient >> drop;
    if low > half || (low == half && (inexact || kept & 1 == 1)) {
        kept += 1;
    }
    let mut unit = exponent - shift + drop;
    let bits = if precision == 53 {
        if kept == 1 << 53 {
            kept >>= 1;
            unit += 1;
        }
        let biased = unit + 52 + 1023;
        if biased >= 0x7ff {
            return None;
        }
        ((biased as u64) << 52) | (kept as u64 & ((1 << 52) - 1))
    } else {
        kept as u64
    };
    Some(sign * f64::from_bits(bits))
}

fn exact(number: &Number) -> (Signed, i64) {
    match number {
        Number::Int(big) => (big.binary(), 0),
        Number::Float(float) => float_parts(*float),
    }
}

const TOO_LARGE_QUOTIENT: &str = "integer division result too large for a float";

impl Number {
    /// `left - right` for an `int` or `float`.
    pub(crate) fn subtract(&self, other: &Self) -> Result<Self> {
        let negated = match other {
            Self::Int(big) => Self::Int(big.negated()),
            Self::Float(float) => Self::Float(-float),
        };
        self.add(&negated)
    }

    /// `self / divisor` for a positive `int` divisor, as Python's true division computes it.
    pub(crate) fn divide(&self, divisor: u64) -> Result<f64> {
        match self {
            Self::Float(float) => Ok(float / divisor as f64),
            Self::Int(big) => ratio_to_float(&big.binary(), divisor, 0)
                .ok_or_else(|| Failure::new("OverflowError", TOO_LARGE_QUOTIENT)),
        }
    }

    /// `float(self)`.
    pub(crate) fn to_float(&self) -> Result<f64> {
        match self {
            Self::Float(float) => Ok(*float),
            Self::Int(big) => big.to_float(),
        }
    }

    /// `self < other`, comparing an `int` and a `float` exactly; false when either is NaN.
    pub(crate) fn less(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Float(a), Self::Float(b)) => a < b,
            (Self::Float(a), _) if !a.is_finite() => *a < 0.0 && !a.is_nan(),
            (_, Self::Float(b)) if !b.is_finite() => *b > 0.0,
            _ => {
                let ((left, left_exponent), (right, right_exponent)) = (exact(self), exact(other));
                let low = left_exponent.min(right_exponent);
                let scale = |value: Signed, exponent: i64| Signed {
                    magnitude: shifted_left(&value.magnitude, exponent - low),
                    negative: value.negative,
                };
                scale(left, left_exponent)
                    .compare(&scale(right, right_exponent))
                    .is_lt()
            }
        }
    }

    /// `round(self, 6)`, or `round(self, 1)` and so on: a float rounds through its exact decimal value.
    pub(crate) fn round_digits(&self, digits: usize) -> Self {
        match self {
            Self::Float(float) if float.is_finite() => {
                Self::Float(format!("{float:.digits$}").parse().unwrap_or(*float))
            }
            other => other.clone(),
        }
    }

    /// `round(self)`: the nearest `int`, ties to even.
    pub(crate) fn round_integer(&self) -> Result<Self> {
        match self {
            Self::Int(big) => Ok(Self::Int(big.clone())),
            Self::Float(float) if float.is_nan() => Err(Failure::new(
                "ValueError",
                "cannot convert float NaN to integer",
            )),
            Self::Float(float) if float.is_infinite() => Err(Failure::new(
                "OverflowError",
                "cannot convert float infinity to integer",
            )),
            Self::Float(float) => Ok(Self::Int(
                Big::parse(&format!("{:.0}", float.round_ties_even().abs()))
                    .with_sign(*float < 0.0),
            )),
        }
    }
}

impl Big {
    fn negated(&self) -> Self {
        let mut big = self.clone();
        big.negative = !big.negative;
        big.trim();
        big
    }

    fn with_sign(mut self, negative: bool) -> Self {
        self.negative = negative;
        self.trim();
        self
    }
}

/// `max(values)`: the first of the largest, as Python keeps it when values compare equal.
pub(crate) fn maximum(values: &[Number]) -> Option<Number> {
    let mut items = values.iter();
    let mut best = items.next()?.clone();
    for item in items {
        if best.less(item) {
            best = item.clone();
        }
    }
    Some(best)
}

/// `statistics.median(values)` of `int`s: the middle value, or the mean of the two middle values as a float.
pub(crate) fn median(values: &[Number]) -> Result<Number> {
    let mut sorted = values.to_vec();
    sorted.sort_by(|left, right| {
        if left.less(right) {
            std::cmp::Ordering::Less
        } else if right.less(left) {
            std::cmp::Ordering::Greater
        } else {
            std::cmp::Ordering::Equal
        }
    });
    let count = sorted.len();
    if count == 0 {
        return Err(Failure::new(
            "statistics.StatisticsError",
            "no median for empty data",
        ));
    }
    if count % 2 == 1 {
        return Ok(sorted[count / 2].clone());
    }
    Ok(Number::Float(
        sorted[count / 2 - 1].add(&sorted[count / 2])?.divide(2)?,
    ))
}

/// `statistics.mean(values)`: the exact mean, an `int` when every value is one and the mean is whole, else
/// correctly rounded to a float.
pub(crate) fn mean(values: &[Number]) -> Result<Number> {
    let count = values.len() as u64;
    let floats = values.iter().any(|value| matches!(value, Number::Float(_)));
    let special: Vec<f64> = values
        .iter()
        .filter_map(|value| match value {
            Number::Float(float) if !float.is_finite() => Some(*float),
            _ => None,
        })
        .collect();
    if !special.is_empty() {
        return Ok(Number::Float(special.iter().sum::<f64>() / count as f64));
    }
    let parts: Vec<(Signed, i64)> = values.iter().map(exact).collect();
    let low = parts
        .iter()
        .map(|(_, exponent)| *exponent)
        .min()
        .unwrap_or(0)
        .min(0);
    let total = parts
        .iter()
        .fold(Signed::default(), |total, (value, exponent)| {
            total.add(&Signed {
                negative: value.negative,
                magnitude: shifted_left(&value.magnitude, exponent - low),
            })
        });
    if !floats {
        let mut quotient = total.clone();
        if divide_small(&mut quotient.magnitude, count) == 0 {
            return Ok(Number::Int(Big::from_binary(&quotient)));
        }
    }
    ratio_to_float(&total, count, low)
        .map(Number::Float)
        .ok_or_else(|| Failure::new("OverflowError", TOO_LARGE_QUOTIENT))
}

/// `format(value, f".{digits}f")` for an `int` or `float`.
pub(crate) fn fixed(value: &Number, digits: usize) -> Result<String> {
    let float = value.to_float()?;
    Ok(if float.is_nan() {
        "nan".to_string()
    } else if float.is_infinite() {
        if float > 0.0 { "inf" } else { "-inf" }.to_string()
    } else {
        format!("{float:.digits$}")
    })
}

/// `float(text)`: Unicode decimal digits and whitespace, underscores between digits, `inf` and `nan`.
pub(crate) fn parse_float(text: &str) -> Result<f64> {
    let invalid = || {
        Failure::new(
            "ValueError",
            format!(
                "could not convert string to float: {}",
                pytext::repr_str(text)
            ),
        )
    };
    let ascii: String = text
        .chars()
        .map(|c| {
            if c.is_ascii() {
                c
            } else if pytext::is_space(c) {
                ' '
            } else if let Some(digit) = pytext::decimal_value(c) {
                char::from_digit(digit, 10).unwrap_or('?')
            } else {
                '?'
            }
        })
        .collect();
    let mut cleaned = String::with_capacity(ascii.len());
    let mut previous = '\0';
    for c in ascii.chars() {
        if c == '_' {
            if !previous.is_ascii_digit() {
                return Err(invalid());
            }
        } else {
            if previous == '_' && !c.is_ascii_digit() {
                return Err(invalid());
            }
            cleaned.push(c);
        }
        previous = c;
    }
    if previous == '_' {
        return Err(invalid());
    }
    let body = cleaned.trim_matches([' ', '\t', '\n', '\x0b', '\x0c', '\r']);
    let digits = body.trim_start_matches(['+', '-']);
    let word = digits.to_ascii_lowercase();
    if body.len() - digits.len() > 1
        || !(matches!(word.as_str(), "inf" | "infinity" | "nan")
            || digits.bytes().all(|byte| {
                byte.is_ascii_digit() || matches!(byte, b'.' | b'e' | b'E' | b'+' | b'-')
            }))
    {
        return Err(invalid());
    }
    body.parse().map_err(|_| invalid())
}

/// `float(value)` of a value `json.loads` returned.
pub(crate) fn to_float(value: &Value) -> Result<f64> {
    match value {
        Value::Str(text) => parse_float(text),
        other => match number(other) {
            Some(number) => number.to_float(),
            None => Err(type_error(format!(
                "float() argument must be a string or a real number, not '{}'",
                other.type_name()
            ))),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn micros(text: &str) -> Option<(i64, bool)> {
        fromisoformat(text)
    }

    #[test]
    fn fromisoformat_follows_python_3_14() {
        let midnight = micros("2026-09-30T00:00:00+00:00").unwrap();
        assert_eq!(micros("2026-09-29T24:00:00+00:00"), Some(midnight));
        assert_eq!(micros("2026-09-29T24:00:01+00:00"), None);
        assert_eq!(
            micros("2026-09-29T10:00"),
            Some((micros("2026-09-29T10:00:00").unwrap().0, false))
        );
        assert_eq!(
            micros("2026-W01-2T10:00:00+00:00"),
            micros("2025-12-30T10:00:00+00:00")
        );
        assert_eq!(
            micros("2026-09-29T10:00:00,5+01:00").map(|(at, _)| at),
            micros("2026-09-29T09:00:00.500000+00:00").map(|(at, _)| at)
        );
        assert_eq!(micros("2026-09-29\u{e9}10:00"), micros("2026-09-29T10:00"));
        assert_eq!(
            micros("20260929T100000.1234567+0530").map(|(at, _)| at),
            micros("2026-09-29T04:30:00.123456+00:00").map(|(at, _)| at)
        );
        assert_eq!(micros("2026-09-29T10:00:00+24:00"), None);
        for refused in [
            "2026-09-29T10:00:00.Z",
            "2026-09-29T10,Z",
            "2026-09-29T10.5Z",
            "2026-09-29T10:00.5Z",
            "2026-09-29T10:00:00:00Z",
            "2026-09-29T10:00:00+01:00.5",
        ] {
            assert_eq!(micros(refused), None, "{refused}");
        }
        assert_eq!(micros("2026-02-30"), None);
        assert_eq!(micros("2026-9-29"), None);
        assert_eq!(micros("short"), None);
    }

    fn numbers(values: &[&str]) -> Vec<Number> {
        values
            .iter()
            .map(|value| number(&loads(value).unwrap()).unwrap())
            .collect()
    }

    fn total(values: &[&str]) -> Value {
        number_value(python_sum(&numbers(values)).unwrap()).unwrap()
    }

    #[test]
    fn sums_add_as_python_3_14_adds() {
        assert!(matches!(total(&["7", "3"]), Value::Int(digits) if digits == "10"));
        assert!(matches!(total(&["true", "2"]), Value::Int(digits) if digits == "3"));
        assert!(matches!(
            total(&["9223372036854775807", "1", "-2"]),
            Value::Int(digits) if digits == "9223372036854775806"
        ));
        let tenths = vec!["0.1"; 10];
        assert!(matches!(total(&tenths), Value::Float(sum) if sum == 1.0));
        assert!(
            matches!(total(&["1e16", "1", "1"]), Value::Float(sum) if sum == 10_000_000_000_000_002.0)
        );
        assert!(matches!(total(&["1", "0.5"]), Value::Float(sum) if sum == 1.5));
        assert!(
            matches!(total(&["1", "1e16", "1"]), Value::Float(sum) if sum == 10_000_000_000_000_000.0)
        );
        let huge = format!("1{}", "0".repeat(400));
        assert!(python_sum(&numbers(&["0.5", &huge])).is_err());
    }

    fn int(digits: &str) -> Number {
        Number::Int(Big::parse(digits))
    }

    fn float_of(number: Number) -> f64 {
        match number {
            Number::Float(float) => float,
            Number::Int(big) => panic!("expected a float, got the int {}", big.digits()),
        }
    }

    #[test]
    fn statistics_follow_python() {
        assert!(
            matches!(mean(&[int("1"), int("3")]).unwrap(), Number::Int(big) if big.digits() == "2")
        );
        assert_eq!(float_of(mean(&[int("1"), int("2")]).unwrap()), 1.5);
        let two_53 = int("9007199254740992");
        assert_eq!(
            float_of(mean(&[Number::Float(0.1), two_53.clone(), int("1")]).unwrap()),
            3002399751580331.0
        );
        assert_eq!(
            float_of(mean(&[int("1"), Number::Float(f64::from_bits(1))]).unwrap()),
            0.5
        );
        assert_eq!(
            float_of(mean(&[Number::Float(f64::from_bits(1)), int("0")]).unwrap()),
            0.0
        );
        assert!(float_of(mean(&[Number::Float(f64::INFINITY), int("1")]).unwrap()).is_infinite());
        assert!(mean(&[int(&format!("1{}", "0".repeat(400))), int("1")]).is_err());
        assert!(
            matches!(median(&[int("3"), int("1"), int("2")]).unwrap(), Number::Int(big) if big.digits() == "2")
        );
        assert_eq!(float_of(median(&[int("1"), int("2")]).unwrap()), 1.5);
        assert_eq!(
            float_of(median(&[int("1152921504606846977"), int("1152921504606846976")]).unwrap()),
            1.152921504606847e18
        );
        assert_eq!(
            median(&[]).unwrap_err().line(),
            "statistics.StatisticsError: no median for empty data"
        );
        assert!(matches!(
            maximum(&[int("1"), Number::Float(1.0)]).unwrap(),
            Number::Int(_)
        ));
        assert!(matches!(
            maximum(&[Number::Float(1.0), int("1")]).unwrap(),
            Number::Float(_)
        ));
        assert!(Number::Float(9007199254740992.0).less(&int("9007199254740993")));
    }

    #[test]
    fn integer_division_rounds_as_python() {
        assert_eq!(
            int("1152921504606846977").divide(3).unwrap(),
            3.843071682022823e17
        );
        assert_eq!(
            int("1000000000000000000000000000001").divide(7).unwrap(),
            1.4285714285714285e29
        );
        assert_eq!(int("1").divide(3).unwrap(), 0.3333333333333333);
        assert!(int(&format!("1{}", "0".repeat(400))).divide(1).is_err());
    }

    #[test]
    fn rounding_and_formatting_follow_python() {
        assert_eq!(float_of(Number::Float(2.675).round_digits(2)), 2.67);
        assert_eq!(float_of(Number::Float(0.125).round_digits(2)), 0.12);
        assert!(
            matches!(Number::Float(2.5).round_integer().unwrap(), Number::Int(big) if big.digits() == "2")
        );
        assert!(
            matches!(Number::Float(-2.5).round_integer().unwrap(), Number::Int(big) if big.digits() == "-2")
        );
        assert!(
            matches!(Number::Float(3.5).round_integer().unwrap(), Number::Int(big) if big.digits() == "4")
        );
        assert!(Number::Float(f64::NAN).round_integer().is_err());
        assert_eq!(fixed(&Number::Float(-0.001), 2).unwrap(), "-0.00");
        assert_eq!(fixed(&Number::Float(0.125), 2).unwrap(), "0.12");
        assert_eq!(
            fixed(&Number::Float(1e16), 2).unwrap(),
            "10000000000000000.00"
        );
        assert_eq!(fixed(&Number::Float(f64::NEG_INFINITY), 2).unwrap(), "-inf");
    }

    #[test]
    fn float_text_parses_as_python_3_14() {
        for (text, value) in [
            ("1_0.5", 10.5),
            (" 1.5 ", 1.5),
            ("\u{661}.5", 1.5),
            ("\u{10D41}.5", 1.5),
            ("1e5", 100000.0),
            (".5", 0.5),
            ("5.", 5.0),
            ("\u{a0}1\u{2003}", 1.0),
            ("\u{b}1.5\u{85}", 1.5),
            ("-Infinity", f64::NEG_INFINITY),
        ] {
            assert_eq!(parse_float(text).unwrap(), value, "{text:?}");
        }
        assert!(parse_float("nan").unwrap().is_nan());
        for text in [
            "1__0",
            "_1",
            "1_",
            "--1",
            "1_.5",
            "1._5",
            "0x10",
            "1e_5",
            "1_e5",
            "\u{1c}1.5",
            "1.5\u{1f}",
            "\u{7f}1",
        ] {
            assert_eq!(
                parse_float(text).unwrap_err().message(),
                format!(
                    "could not convert string to float: {}",
                    pytext::repr_str(text)
                ),
            );
        }
    }
}
