//! Python's object operations on the values `json.loads` returns: comparison, membership, indexing, `len`,
//! iteration, `set` and `sorted`, each failing where Python raises.
//!
//! Code that applies Python rules to untrusted JSON depends on exactly when they raise, because a raise
//! anywhere can replace the whole result. Every JSON `NaN` is one Python object (`json.decoder.NaN`), so the
//! identity check of `in`, list and dict comparison and set lookup finds it equal to itself, while `==`
//! between two `NaN` values is false.

use crate::pyjson::Value;
use std::borrow::Cow;
use std::cmp::Ordering;

/// The Python exception an operation raises, or `Refused` where the port stops on purpose.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Raised {
    KeyError,
    TypeError,
    IndexError,
    AttributeError,
    ValueError,
    /// `sorted` of numbers that include `NaN`, whose order Python leaves to its hash-ordered set.
    Refused,
}

pub type PyResult<T> = Result<T, Raised>;

fn integer(value: &Value) -> Option<Cow<'_, str>> {
    match value {
        Value::Bool(flag) => Some(Cow::Borrowed(if *flag { "1" } else { "0" })),
        Value::Int(digits) => Some(Cow::Borrowed(digits)),
        _ => None,
    }
}

fn compare_integers(left: &str, right: &str) -> Ordering {
    let (left_negative, left) = left
        .strip_prefix('-')
        .map_or((false, left), |rest| (true, rest));
    let (right_negative, right) = right
        .strip_prefix('-')
        .map_or((false, right), |rest| (true, rest));
    let magnitude = left.len().cmp(&right.len()).then_with(|| left.cmp(right));
    match (left_negative, right_negative) {
        (false, false) => magnitude,
        (true, true) => magnitude.reverse(),
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
    }
}

/// The exact decimal digits of an integral, finite float.
fn integral_digits(value: f64) -> String {
    if value == 0.0 {
        "0".to_string()
    } else {
        format!("{value:.0}")
    }
}

fn compare_integer_float(integer: &str, float: f64) -> Option<Ordering> {
    if float.is_nan() {
        return None;
    }
    if float.is_infinite() {
        return Some(if float > 0.0 {
            Ordering::Less
        } else {
            Ordering::Greater
        });
    }
    let floor = float.floor();
    Some(match compare_integers(integer, &integral_digits(floor)) {
        Ordering::Equal if float != floor => Ordering::Less,
        order => order,
    })
}

/// How two numbers (`bool`, `int` or `float`) compare in Python, exactly; `None` when either is not a number or
/// a comparison involves `NaN`.
pub fn compare_numbers(left: &Value, right: &Value) -> Option<Ordering> {
    match (left, right) {
        (Value::Float(left), Value::Float(right)) => left.partial_cmp(right),
        (Value::Float(float), other) => {
            compare_integer_float(&integer(other)?, *float).map(Ordering::reverse)
        }
        (other, Value::Float(float)) => compare_integer_float(&integer(other)?, *float),
        _ => Some(compare_integers(&integer(left)?, &integer(right)?)),
    }
}

/// `left == right`.
pub fn eq(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Null, Value::Null) => true,
        (Value::Str(left), Value::Str(right)) => left == right,
        (Value::Array(left), Value::Array(right)) => {
            left.len() == right.len() && left.iter().zip(right).all(|(a, b)| same(a, b))
        }
        (Value::Object(left), Value::Object(right)) => {
            left.len() == right.len()
                && left
                    .iter()
                    .all(|(key, value)| right.get(key).is_some_and(|other| same(value, other)))
        }
        _ => compare_numbers(left, right) == Some(Ordering::Equal),
    }
}

/// The comparison `in`, list equality and set lookup make: identity first, then `==`.
pub fn same(left: &Value, right: &Value) -> bool {
    matches!((left, right), (Value::Float(a), Value::Float(b)) if a.is_nan() && b.is_nan())
        || eq(left, right)
}

pub fn hashable(value: &Value) -> bool {
    !matches!(value, Value::Array(_) | Value::Object(_))
}

/// `needle in container`.
pub fn contains(container: &Value, needle: &Value) -> PyResult<bool> {
    match container {
        Value::Array(values) => Ok(values.iter().any(|value| same(value, needle))),
        Value::Object(object) => match needle {
            _ if !hashable(needle) => Err(Raised::TypeError),
            Value::Str(key) => Ok(object.get(key).is_some()),
            _ => Ok(false),
        },
        Value::Str(text) => match needle {
            Value::Str(part) => Ok(text.contains(part.as_str())),
            _ => Err(Raised::TypeError),
        },
        _ => Err(Raised::TypeError),
    }
}

/// `value[key]` with a `str` key.
pub fn getitem<'a>(value: &'a Value, key: &str) -> PyResult<&'a Value> {
    match value {
        Value::Object(object) => object.get(key).ok_or(Raised::KeyError),
        _ => Err(Raised::TypeError),
    }
}

/// `value.get(key)`, which only a `dict` has.
pub fn get<'a>(value: &'a Value, key: &str) -> PyResult<Option<&'a Value>> {
    match value {
        Value::Object(object) => Ok(object.get(key)),
        _ => Err(Raised::AttributeError),
    }
}

/// `value[index]` with a non-negative `int` index.
pub fn index(value: &Value, index: usize) -> PyResult<Cow<'_, Value>> {
    match value {
        Value::Array(values) => values
            .get(index)
            .map(Cow::Borrowed)
            .ok_or(Raised::IndexError),
        Value::Str(text) => text
            .chars()
            .nth(index)
            .map(|c| Cow::Owned(Value::Str(c.to_string())))
            .ok_or(Raised::IndexError),
        Value::Object(_) => Err(Raised::KeyError),
        _ => Err(Raised::TypeError),
    }
}

/// `len(value)`.
pub fn len(value: &Value) -> PyResult<usize> {
    match value {
        Value::Array(values) => Ok(values.len()),
        Value::Object(object) => Ok(object.len()),
        Value::Str(text) => Ok(text.chars().count()),
        _ => Err(Raised::TypeError),
    }
}

/// The values `for x in value` visits: list items, dict keys, or one-character strings.
pub fn iterate(value: &Value) -> PyResult<Vec<Cow<'_, Value>>> {
    match value {
        Value::Array(values) => Ok(values.iter().map(Cow::Borrowed).collect()),
        Value::Object(object) => Ok(object
            .iter()
            .map(|(key, _)| Cow::Owned(Value::Str(key.clone())))
            .collect()),
        Value::Str(text) => Ok(text
            .chars()
            .map(|c| Cow::Owned(Value::Str(c.to_string())))
            .collect()),
        _ => Err(Raised::TypeError),
    }
}

/// `set(value)`, each member kept as first seen.
pub fn set(value: &Value) -> PyResult<Vec<Value>> {
    let mut members: Vec<Value> = Vec::new();
    for member in iterate(value)? {
        if !hashable(&member) {
            return Err(Raised::TypeError);
        }
        if !members.iter().any(|seen| same(seen, &member)) {
            members.push(member.into_owned());
        }
    }
    Ok(members)
}

/// `left - right` of two sets.
pub fn difference(left: &[Value], right: &[Value]) -> Vec<Value> {
    left.iter()
        .filter(|member| !right.iter().any(|other| same(other, member)))
        .cloned()
        .collect()
}

/// `sorted(members)` of a set's members. Strings sort by code point and numbers by value; members Python cannot
/// order against each other raise `TypeError`.
pub fn sorted(mut members: Vec<Value>) -> PyResult<Vec<Value>> {
    if members.len() < 2 {
        return Ok(members);
    }
    let class = |value: &Value| match value {
        Value::Str(_) | Value::OsText(_) => Ok(0),
        Value::Null => Ok(1),
        Value::Bool(_) | Value::Int(_) | Value::Float(_) => Ok(2),
        Value::Array(_) | Value::Object(_) => Err(Raised::TypeError),
    };
    let first = class(&members[0])?;
    for member in &members {
        if class(member)? != first {
            return Err(Raised::TypeError);
        }
    }
    match first {
        0 => members.sort_by_cached_key(|member| match member {
            Value::OsText(text) => crate::pytext::surrogateescape(text.as_bytes()),
            _ => crate::pytext::surrogateescape(member.as_str().unwrap_or_default().as_bytes()),
        }),
        2 => {
            if members
                .iter()
                .any(|member| matches!(member, Value::Float(f) if f.is_nan()))
            {
                return Err(Raised::Refused);
            }
            members.sort_by(|left, right| compare_numbers(left, right).unwrap_or(Ordering::Equal));
        }
        _ => return Err(Raised::TypeError),
    }
    Ok(members)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pyjson::{Duplicates, loads};

    fn value(text: &str) -> Value {
        loads(text, Duplicates::KeepLast).unwrap()
    }

    #[test]
    fn numbers_compare_exactly_across_int_and_float() {
        assert!(eq(&value("1"), &value("1.0")));
        assert!(eq(&value("true"), &value("1")));
        assert!(!eq(
            &value("9007199254740993"),
            &value("9007199254740992.0")
        ));
        assert!(eq(&value("100000000000000000000"), &value("1e20")));
        assert!(eq(&value("0"), &value("-0.0")));
        assert_eq!(
            compare_numbers(&value("2"), &value("1.5")),
            Some(Ordering::Greater)
        );
        assert_eq!(
            compare_numbers(&value("-2"), &value("-1.5")),
            Some(Ordering::Less)
        );
        assert_eq!(
            compare_numbers(&value("1"), &value("Infinity")),
            Some(Ordering::Less)
        );
        assert!(!eq(&value("\"1\""), &value("1")));
    }

    #[test]
    fn nan_is_itself_inside_containers_only() {
        let nan = value("NaN");
        assert!(!eq(&nan, &nan));
        assert!(same(&nan, &nan));
        assert!(eq(&value("[NaN]"), &value("[NaN]")));
        assert!(contains(&value("[NaN]"), &nan).unwrap());
    }

    #[test]
    fn membership_raises_where_python_does() {
        assert_eq!(
            contains(&value("{\"a\": 1}"), &value("[]")),
            Err(Raised::TypeError)
        );
        assert_eq!(
            contains(&value("\"abc\""), &value("1")),
            Err(Raised::TypeError)
        );
        assert_eq!(
            contains(&value("3"), &value("\"a\"")),
            Err(Raised::TypeError)
        );
        assert!(contains(&value("\"abc\""), &value("\"bc\"")).unwrap());
        assert!(!contains(&value("{\"1\": 1}"), &value("1")).unwrap());
        assert!(contains(&value("[1]"), &value("true")).unwrap());
    }

    #[test]
    fn sets_and_sorting_follow_python() {
        assert_eq!(
            set(&value("[1, true, 1.0, 2]")).unwrap().len(),
            2,
            "1, True and 1.0 are one member"
        );
        assert_eq!(set(&value("[[1]]")).err(), Some(Raised::TypeError));
        let ordered = sorted(set(&value("[3, 1.5, -2, 10]")).unwrap()).unwrap();
        assert_eq!(
            ordered.iter().map(Value::py_str).collect::<Vec<_>>(),
            ["-2", "1.5", "3", "10"]
        );
        assert_eq!(
            sorted(set(&value("[1, \"a\"]")).unwrap()).err(),
            Some(Raised::TypeError)
        );
        assert_eq!(
            sorted(set(&value("[1, NaN]")).unwrap()).err(),
            Some(Raised::Refused)
        );
        assert_eq!(
            sorted(set(&value("\"b\\u00e9a\"")).unwrap())
                .unwrap()
                .iter()
                .map(Value::py_str)
                .collect::<String>(),
            "ab\u{e9}"
        );
    }
}
