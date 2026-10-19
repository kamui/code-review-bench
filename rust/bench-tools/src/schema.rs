//! Validate a document against a schema as `bench/tools/check_manifest.py` does.
//!
//! The keywords are those `check_manifest.py` supports: `type`, `const`, `enum`, `required`, `properties`,
//! `additionalProperties`, `items`, `pattern`, `minimum`, local `$ref` into `$defs`, and the annotations
//! `$id`, `$schema`, `title` and `description`. Any other keyword is refused when its schema is visited.
//! As in Python, a keyword is read only when the document reaches it, violations are reported in the
//! same order and wording, and a `$ref` replaces the other keywords of its schema.
//!
//! Refused where Python would raise a traceback or read the value another way: a schema that is not an
//! object, a keyword whose value has a JSON type the check cannot use, a `pattern` outside
//! [`crate::pyregex`]'s syntax, and `$ref` chains deeper than [`MAX_DEPTH`].

use crate::pyjson::Value;
use crate::pyregex::{self, Regex};
use crate::pytext;
use std::collections::HashMap;

const SUPPORTED: [&str; 15] = [
    "$id",
    "$schema",
    "title",
    "description",
    "type",
    "const",
    "enum",
    "required",
    "properties",
    "additionalProperties",
    "items",
    "pattern",
    "minimum",
    "$ref",
    "$defs",
];

/// Python's recursion limit stops a `$ref` cycle with a traceback; this stops it with a refusal.
pub const MAX_DEPTH: usize = 900;

/// Enough stack for [`MAX_DEPTH`] nested checks in an unoptimized build.
const STACK_BYTES: usize = 64 << 20;

/// `check_manifest.SchemaError`: the schema uses something the check does not enforce.
#[derive(Debug, PartialEq, Eq)]
pub struct SchemaError(pub String);

#[derive(Default)]
pub struct Validator {
    patterns: HashMap<String, Regex>,
}

fn type_ok(value: &Value, name: &str) -> Option<bool> {
    Some(match name {
        "integer" => matches!(value, Value::Int(_)),
        "number" => matches!(value, Value::Int(_) | Value::Float(_)),
        "boolean" => matches!(value, Value::Bool(_)),
        "object" => matches!(value, Value::Object(_)),
        "array" => matches!(value, Value::Array(_)),
        "string" => matches!(value, Value::Str(_)),
        "null" => matches!(value, Value::Null),
        _ => return None,
    })
}

/// `check_manifest.same`: JSON Schema equality, where a boolean never equals a number and `1` equals `1.0`.
pub fn same(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Bool(left), Value::Bool(right)) => left == right,
        (Value::Bool(_), _) | (_, Value::Bool(_)) => false,
        (Value::Int(_) | Value::Float(_), Value::Int(_) | Value::Float(_)) => left.py_eq(right),
        (Value::Array(left), Value::Array(right)) => {
            left.len() == right.len() && left.iter().zip(right).all(|(a, b)| same(a, b))
        }
        (Value::Object(left), Value::Object(right)) => {
            left.len() == right.len()
                && left
                    .iter()
                    .all(|(key, value)| right.get(key).is_some_and(|other| same(value, other)))
        }
        _ => left.py_eq(right),
    }
}

/// What a Python `for` loop over the value visits: a list's elements, a string's characters or an object's
/// keys. Other values cannot be iterated.
fn py_iter(value: &Value) -> Option<Vec<Value>> {
    match value {
        Value::Array(values) => Some(values.clone()),
        Value::Str(text) => Some(text.chars().map(|c| Value::Str(c.to_string())).collect()),
        Value::Object(object) => Some(
            object
                .iter()
                .map(|(key, _)| Value::Str(key.clone()))
                .collect(),
        ),
        _ => None,
    }
}

/// `left < right` for two numbers, booleans counting as integers, compared exactly as Python does.
fn less(left: &Value, right: &Value) -> Option<bool> {
    let integer = |value: &Value| match value {
        Value::Bool(flag) => Some(if *flag { 1_i8 } else { 0 }.to_string()),
        Value::Int(digits) => Some(digits.clone()),
        _ => None,
    };
    match (left, right) {
        (Value::Float(left), Value::Float(right)) => Some(left < right),
        (Value::Float(number), other) => {
            let digits = integer(other)?;
            Some(compare_float_integer(*number, &digits) == Some(std::cmp::Ordering::Less))
        }
        (other, Value::Float(number)) => {
            let digits = integer(other)?;
            Some(compare_float_integer(*number, &digits) == Some(std::cmp::Ordering::Greater))
        }
        _ => Some(compare_integers(&integer(left)?, &integer(right)?) == std::cmp::Ordering::Less),
    }
}

fn compare_integers(left: &str, right: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;
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

/// How a float compares with an integer of any size; `None` for NaN.
fn compare_float_integer(number: f64, digits: &str) -> Option<std::cmp::Ordering> {
    use std::cmp::Ordering;
    if number.is_nan() {
        return None;
    }
    if number.is_infinite() {
        return Some(if number > 0.0 {
            Ordering::Greater
        } else {
            Ordering::Less
        });
    }
    let whole = number.trunc();
    let whole_digits = if whole == 0.0 {
        "0".to_string()
    } else {
        format!("{whole:.0}")
    };
    let ordering = compare_integers(&whole_digits, digits);
    if ordering != Ordering::Equal {
        return Some(ordering);
    }
    let fraction = number - whole;
    Some(if fraction > 0.0 {
        Ordering::Greater
    } else if fraction < 0.0 {
        Ordering::Less
    } else {
        Ordering::Equal
    })
}

fn unusable(path: &str, keyword: &str, value: &Value) -> SchemaError {
    SchemaError(format!(
        "{path}: unsupported {keyword} value {}",
        value.py_repr()
    ))
}

impl Validator {
    /// `check_manifest.validate(schema, value)`: one `path: reason` string per violation.
    pub fn validate(&mut self, schema: &Value, value: &Value) -> Result<Vec<String>, SchemaError> {
        std::thread::scope(|scope| {
            std::thread::Builder::new()
                .stack_size(STACK_BYTES)
                .spawn_scoped(scope, || {
                    let mut out = Vec::new();
                    self.check(schema, value, "$", schema, &mut out, 0)?;
                    Ok(out)
                })
                .map_err(|error| SchemaError(format!("$: cannot start the validator: {error}")))?
                .join()
                .unwrap_or_else(|_| Err(SchemaError("$: the validator stopped".to_string())))
        })
    }

    fn check(
        &mut self,
        schema: &Value,
        value: &Value,
        path: &str,
        root: &Value,
        out: &mut Vec<String>,
        depth: usize,
    ) -> Result<(), SchemaError> {
        let mut current = schema;
        let mut depth = depth;
        let schema = loop {
            if depth > MAX_DEPTH {
                return Err(SchemaError(format!("{path}: schema nesting is too deep")));
            }
            let Value::Object(schema) = current else {
                return Err(SchemaError(format!(
                    "{path}: schema {} is not an object",
                    current.py_repr()
                )));
            };
            let mut unknown: Vec<&String> = schema
                .iter()
                .map(|(key, _)| key)
                .filter(|key| !SUPPORTED.contains(&key.as_str()))
                .collect();
            if !unknown.is_empty() {
                unknown.sort();
                let names: Vec<String> = unknown.iter().map(|key| pytext::repr_str(key)).collect();
                return Err(SchemaError(format!(
                    "{path}: unsupported schema keyword(s) [{}]",
                    names.join(", ")
                )));
            }
            let Some(reference) = schema.get("$ref") else {
                break schema;
            };
            let Some(name) = reference
                .as_str()
                .and_then(|text| text.strip_prefix("#/$defs/"))
            else {
                return Err(SchemaError(format!(
                    "{path}: only local $defs refs are supported, got {}",
                    reference.py_str()
                )));
            };
            let definitions = match root.as_object().and_then(|root| root.get("$defs")) {
                None => None,
                Some(Value::Object(definitions)) => definitions.get(name),
                Some(other) => return Err(unusable(path, "$defs", other)),
            };
            let target = match definitions {
                None | Some(Value::Null) => {
                    return Err(SchemaError(format!(
                        "{path}: unresolved {}",
                        reference.py_str()
                    )));
                }
                Some(target) => target,
            };
            current = target;
            depth += 1;
        };
        if let Some(constant) = schema.get("const")
            && !same(value, constant)
        {
            out.push(format!(
                "{path}: expected constant {}, got {}",
                constant.py_repr(),
                value.py_repr()
            ));
            return Ok(());
        }
        if let Some(members) = schema.get("enum") {
            let listed = py_iter(members).ok_or_else(|| unusable(path, "enum", members))?;
            if !listed.iter().any(|member| same(value, member)) {
                out.push(format!(
                    "{path}: {} not in {}",
                    value.py_repr(),
                    members.py_repr()
                ));
                return Ok(());
            }
        }
        if let Some(declared) = schema.get("type") {
            let names: Vec<&Value> = match declared {
                Value::Array(names) => names.iter().collect(),
                single => vec![single],
            };
            let mut matched = false;
            for name in &names {
                let known = name
                    .as_str()
                    .and_then(|name| type_ok(value, name))
                    .ok_or_else(|| unusable(path, "type", declared))?;
                if known {
                    matched = true;
                    break;
                }
            }
            if !matched {
                let joined: Vec<&str> = names.iter().filter_map(|name| name.as_str()).collect();
                out.push(format!(
                    "{path}: expected type {}, got {}",
                    joined.join("/"),
                    value.type_name()
                ));
                return Ok(());
            }
        }
        if let (Value::Str(text), Some(pattern)) = (value, schema.get("pattern")) {
            let Value::Str(source) = pattern else {
                return Err(unusable(path, "pattern", pattern));
            };
            if !self.search(source, text, path)? {
                out.push(format!(
                    "{path}: {} does not match /{source}/",
                    value.py_repr()
                ));
            }
        }
        if let (Value::Int(_) | Value::Float(_), Some(minimum)) = (value, schema.get("minimum")) {
            let below = match minimum {
                Value::Int(_) | Value::Float(_) | Value::Bool(_) => less(value, minimum),
                _ => None,
            }
            .ok_or_else(|| unusable(path, "minimum", minimum))?;
            if below {
                out.push(format!(
                    "{path}: {} below minimum {}",
                    value.py_str(),
                    minimum.py_str()
                ));
            }
        }
        if let Value::Object(object) = value {
            if let Some(required) = schema.get("required") {
                let keys = py_iter(required).ok_or_else(|| unusable(path, "required", required))?;
                for key in keys {
                    let present = match &key {
                        Value::Str(name) => object.get(name).is_some(),
                        other if other.hashable() => false,
                        _ => return Err(unusable(path, "required", required)),
                    };
                    if !present {
                        out.push(format!("{path}: missing required key {}", key.py_repr()));
                    }
                }
            }
            let properties = schema.get("properties");
            let extra = schema.get("additionalProperties");
            for (key, item) in object.iter() {
                let declared = match properties {
                    None => None,
                    Some(Value::Object(properties)) => properties.get(key),
                    Some(Value::Array(names))
                        if !names.iter().any(|name| name.as_str() == Some(key.as_str())) =>
                    {
                        None
                    }
                    Some(Value::Str(text)) if !text.contains(key.as_str()) => None,
                    Some(other) => return Err(unusable(path, "properties", other)),
                };
                let child = format!("{path}.{key}");
                match (declared, extra) {
                    (Some(property), _) => {
                        self.check(property, item, &child, root, out, depth + 1)?
                    }
                    (None, Some(Value::Bool(false))) => {
                        out.push(format!("{path}: unexpected key {}", pytext::repr_str(key)));
                    }
                    (None, Some(extra @ Value::Object(_))) => {
                        self.check(extra, item, &child, root, out, depth + 1)?;
                    }
                    (None, _) => {}
                }
            }
        }
        if let (Value::Array(values), Some(items)) = (value, schema.get("items")) {
            for (index, item) in values.iter().enumerate() {
                self.check(
                    items,
                    item,
                    &format!("{path}[{index}]"),
                    root,
                    out,
                    depth + 1,
                )?;
            }
        }
        Ok(())
    }

    fn search(&mut self, pattern: &str, text: &str, path: &str) -> Result<bool, SchemaError> {
        if !self.patterns.contains_key(pattern) {
            let compiled = pyregex::compile(pattern).map_err(|pyregex::Unsupported(reason)| {
                SchemaError(format!(
                    "{path}: unsupported pattern {}: {reason}",
                    pytext::repr_str(pattern)
                ))
            })?;
            self.patterns.insert(pattern.to_string(), compiled);
        }
        Ok(self.patterns[pattern].search(text))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pyjson::{Duplicates, loads};

    fn json(text: &str) -> Value {
        loads(text, Duplicates::KeepLast).unwrap()
    }

    fn validate(schema: &str, value: &str) -> Result<Vec<String>, SchemaError> {
        Validator::default().validate(&json(schema), &json(value))
    }

    #[test]
    fn python_self_test_cases() {
        let schema = r##"{"type": "object", "required": ["id", "n"], "additionalProperties": false,
            "properties": {"id": {"type": "string", "pattern": "^[a-z]+$"}, "n": {"type": "integer", "minimum": 1},
            "kind": {"enum": ["a", "b"]}, "v": {"const": 1}, "opt": {"type": ["string", "null"]},
            "rows": {"type": "array", "items": {"$ref": "#/$defs/row"}}},
            "$defs": {"row": {"type": "object", "required": ["k"], "properties": {"k": {"type": "number"}}}}}"##;
        assert_eq!(
            validate(
                schema,
                r#"{"id": "ab", "n": 1, "kind": "a", "v": 1, "opt": null, "rows": [{"k": 1.5}]}"#
            ),
            Ok(vec![])
        );
        assert_eq!(
            validate(
                schema,
                r#"{"id": "A1", "n": 0, "kind": "c", "v": 2, "opt": 3, "rows": [{}], "zzz": 1}"#
            ),
            Ok(vec![
                "$.id: 'A1' does not match /^[a-z]+$/".to_string(),
                "$.n: 0 below minimum 1".to_string(),
                "$.kind: 'c' not in ['a', 'b']".to_string(),
                "$.v: expected constant 1, got 2".to_string(),
                "$.opt: expected type string/null, got int".to_string(),
                "$.rows[0]: missing required key 'k'".to_string(),
                "$: unexpected key 'zzz'".to_string(),
            ])
        );
        assert_eq!(validate(r#"{"type": "integer"}"#, "true").unwrap().len(), 1);
        assert_eq!(validate(r#"{"const": 1}"#, "true").unwrap().len(), 1);
        assert_eq!(validate(r#"{"const": true}"#, "1").unwrap().len(), 1);
        assert_eq!(
            validate(r#"{"enum": [true, false, "n/a"]}"#, "0")
                .unwrap()
                .len(),
            1
        );
        assert_eq!(validate(r#"{"enum": [0, 1]}"#, "false").unwrap().len(), 1);
        assert_eq!(validate(r#"{"const": 1}"#, "1.0"), Ok(vec![]));
        assert_eq!(validate(r#"{"enum": [true, "n/a"]}"#, "true"), Ok(vec![]));
        assert_eq!(
            validate(r#"{"type": "object", "oneOf": []}"#, "{}"),
            Err(SchemaError(
                "$: unsupported schema keyword(s) ['oneOf']".to_string()
            ))
        );
    }

    #[test]
    fn keywords_are_read_only_where_the_document_reaches() {
        let schema = r#"{"properties": {"a": {"oneOf": []}}}"#;
        assert_eq!(validate(schema, "{}"), Ok(vec![]));
        assert!(validate(schema, r#"{"a": 1}"#).is_err());
        assert_eq!(
            validate(r#"{"type": ["string", "bogus"]}"#, r#""x""#),
            Ok(vec![])
        );
        assert!(validate(r#"{"type": ["string", "bogus"]}"#, "1").is_err());
        assert_eq!(validate(r#"{"pattern": "(?=x)"}"#, "1"), Ok(vec![]));
        assert!(validate(r#"{"pattern": "(?=x)"}"#, r#""x""#).is_err());
    }

    #[test]
    fn numbers_compare_exactly() {
        assert_eq!(
            validate(r#"{"minimum": 1}"#, "0.9999999999999999")
                .unwrap()
                .len(),
            1
        );
        assert_eq!(validate(r#"{"minimum": 1}"#, "1.0"), Ok(vec![]));
        assert_eq!(
            validate(r#"{"minimum": 9007199254740993}"#, "9007199254740992.0").unwrap(),
            vec!["$: 9007199254740992.0 below minimum 9007199254740993".to_string()]
        );
        assert_eq!(validate(r#"{"minimum": 0}"#, "NaN"), Ok(vec![]));
        assert_eq!(
            validate(r#"{"minimum": 0}"#, "-Infinity").unwrap(),
            vec!["$: -inf below minimum 0".to_string()]
        );
        assert_eq!(validate(r#"{"minimum": true}"#, "0").unwrap().len(), 1);
        assert_eq!(validate(r#"{"minimum": 0}"#, "true"), Ok(vec![]));
        assert!(validate(r#"{"minimum": "0"}"#, "1").is_err());
    }

    #[test]
    fn refs_resolve_only_into_local_defs() {
        assert_eq!(
            validate(r##"{"$ref": "#/$defs/x"}"##, "1"),
            Err(SchemaError("$: unresolved #/$defs/x".to_string()))
        );
        assert_eq!(
            validate(r##"{"$ref": "other.json"}"##, "1"),
            Err(SchemaError(
                "$: only local $defs refs are supported, got other.json".to_string()
            ))
        );
        assert_eq!(
            validate(r##"{"$ref": "#/$defs/x", "$defs": {"x": null}}"##, "1"),
            Err(SchemaError("$: unresolved #/$defs/x".to_string()))
        );
        assert!(
            validate(
                r##"{"$ref": "#/$defs/x", "$defs": {"x": {"$ref": "#/$defs/x"}}}"##,
                "1"
            )
            .is_err()
        );
        assert_eq!(
            validate(
                r##"{"$ref": "#/$defs/x", "type": "string", "$defs": {"x": {}}}"##,
                "1"
            ),
            Ok(vec![])
        );
    }

    #[test]
    fn deep_documents_are_refused_without_exhausting_the_stack() {
        // Parsing and dropping 999 levels needs more than a test thread's stack in an unoptimized build.
        std::thread::Builder::new()
            .stack_size(STACK_BYTES)
            .spawn(|| {
                let schema =
                    r##"{"$ref": "#/$defs/a", "$defs": {"a": {"items": {"$ref": "#/$defs/a"}}}}"##;
                let deep = format!("{}{}", "[".repeat(999), "]".repeat(999));
                assert!(validate(schema, &deep).is_err());
                let shallow = format!("{}{}", "[".repeat(400), "]".repeat(400));
                assert_eq!(validate(schema, &shallow), Ok(vec![]));
            })
            .unwrap()
            .join()
            .unwrap();
    }

    /// Every `pattern` in the repository's schemas compiles, so none is refused where Python would apply it.
    #[test]
    fn repository_schema_patterns_are_supported() {
        fn patterns(value: &Value, found: &mut Vec<String>) {
            match value {
                Value::Object(object) => {
                    for (key, item) in object.iter() {
                        match (key.as_str(), item) {
                            ("pattern", Value::Str(pattern)) => found.push(pattern.clone()),
                            _ => patterns(item, found),
                        }
                    }
                }
                Value::Array(values) => values.iter().for_each(|item| patterns(item, found)),
                _ => {}
            }
        }
        let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bench/schema");
        let mut found = Vec::new();
        let mut files = 0;
        for entry in std::fs::read_dir(&directory).unwrap() {
            let path = entry.unwrap().path();
            if path.to_string_lossy().ends_with(".schema.json") {
                files += 1;
                patterns(&json(&std::fs::read_to_string(&path).unwrap()), &mut found);
            }
        }
        assert!(
            files > 20 && found.len() > 300,
            "{files} schemas, {} patterns",
            found.len()
        );
        for pattern in found {
            assert!(pyregex::compile(&pattern).is_ok(), "{pattern:?}");
        }
    }

    #[test]
    fn iterable_keyword_values_are_read_as_python_iterates_them() {
        assert_eq!(
            validate(r#"{"required": "ab"}"#, r#"{"a": 1}"#).unwrap(),
            vec!["$: missing required key 'b'".to_string()]
        );
        assert_eq!(validate(r#"{"enum": "abc"}"#, r#""b""#), Ok(vec![]));
        assert_eq!(
            validate(r#"{"required": [1]}"#, "{}").unwrap(),
            vec!["$: missing required key 1".to_string()]
        );
        assert!(validate(r#"{"required": [[1]]}"#, "{}").is_err());
        assert!(validate(r#"{"required": 1}"#, "{}").is_err());
        assert_eq!(validate(r#"{"required": 1}"#, "[]"), Ok(vec![]));
        assert_eq!(
            validate(
                r#"{"properties": ["b"], "additionalProperties": false}"#,
                r#"{"a": 1}"#
            )
            .unwrap()
            .len(),
            1
        );
        assert!(validate(r#"{"properties": ["a"]}"#, r#"{"a": 1}"#).is_err());
        assert_eq!(
            validate(r#"{"additionalProperties": 0}"#, r#"{"a": 1}"#),
            Ok(vec![])
        );
    }
}
