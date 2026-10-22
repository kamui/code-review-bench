//! `--contract-json`, run as the `contract-json` a case's shell steps find on `PATH`: reads, builds and edits fixture
//! JSON as Python's `json` module would, so a step writes the bytes its case expects without running Python.
//!
//! ```text
//! contract-json get FILE POINTER       the value at POINTER, as JSON
//! contract-json text FILE POINTER      the string at POINTER, as its text
//! contract-json sha256 FILE            the file's SHA-256, in hex
//! contract-json write FILE [--newline] the JSON on stdin, indented by two
//! contract-json edit FILE [--to OUT] [--newline] OPERATION...
//!     set POINTER JSON | text POINTER TEXT | insert POINTER JSON | delete POINTER
//! ```
//!
//! `edit` applies each operation in order and writes the result to OUT, or back to FILE. `set` assigns as Python's
//! `object[key] = value` does, `insert` as `list.insert`, `delete` as `del`. `--newline` ends the file with one.

use crate::json;
use bench_tools::pyjson::{self, Duplicates, Object, Value};
use std::ffi::OsString;

pub fn role(args: &[OsString]) -> i32 {
    let args: Option<Vec<&str>> = args.iter().map(|arg| arg.to_str()).collect();
    let result = args
        .ok_or_else(|| "arguments must be UTF-8".to_string())
        .and_then(|args| run(&args));
    match result {
        Ok(Some(out)) => {
            println!("{out}");
            0
        }
        Ok(None) => 0,
        Err(error) => {
            eprintln!("contract-json: {error}");
            1
        }
    }
}

fn run(args: &[&str]) -> Result<Option<String>, String> {
    match args {
        ["get", file, pointer] => Ok(Some(json::dumps(json::pointer(&read(file)?, pointer)?))),
        ["text", file, pointer] => json::pointer(&read(file)?, pointer)?
            .as_str()
            .map(|text| Some(text.to_string()))
            .ok_or_else(|| format!("{pointer} is not a string")),
        ["sha256", file] => Ok(Some(bench_tools::sha256::hex_digest(
            &std::fs::read(file).map_err(|error| format!("{file}: {error}"))?,
        ))),
        ["write", file, rest @ ..] => {
            let newline = match rest {
                [] => false,
                ["--newline"] => true,
                _ => return Err(format!("write takes only --newline, not {rest:?}")),
            };
            let mut input = String::new();
            std::io::Read::read_to_string(&mut std::io::stdin(), &mut input)
                .map_err(|error| format!("stdin: {error}"))?;
            save(file, &parse(&input, "stdin")?, newline).map(|()| None)
        }
        ["edit", file, rest @ ..] => edit(file, rest).map(|()| None),
        _ => Err(format!("unknown command {args:?}")),
    }
}

fn parse(text: &str, origin: &str) -> Result<Value, String> {
    pyjson::loads(text, Duplicates::Refuse).map_err(|error| format!("{origin}: {error}"))
}

fn read(file: &str) -> Result<Value, String> {
    let text = std::fs::read_to_string(file).map_err(|error| format!("{file}: {error}"))?;
    parse(&text, file)
}

fn save(file: &str, value: &Value, newline: bool) -> Result<(), String> {
    let mut text = json::dumps_indented(value);
    if newline {
        text.push('\n');
    }
    std::fs::write(file, text).map_err(|error| format!("{file}: {error}"))
}

fn edit(file: &str, mut args: &[&str]) -> Result<(), String> {
    let mut out = file;
    let mut newline = false;
    loop {
        match args {
            ["--to", to, rest @ ..] => {
                out = to;
                args = rest;
            }
            ["--newline", rest @ ..] => {
                newline = true;
                args = rest;
            }
            _ => break,
        }
    }
    let mut document = read(file)?;
    while !args.is_empty() {
        args = match args {
            ["set", pointer, value, rest @ ..] => {
                assign(&mut document, pointer, parse(value, pointer)?)?;
                rest
            }
            ["text", pointer, value, rest @ ..] => {
                assign(&mut document, pointer, json::text(value))?;
                rest
            }
            ["insert", pointer, value, rest @ ..] => {
                let value = parse(value, pointer)?;
                let (parent, last) = parent(&mut document, pointer)?;
                let Value::Array(items) = parent else {
                    return Err(format!("insert {pointer}: the parent is not a list"));
                };
                let at = last
                    .parse::<usize>()
                    .ok()
                    .filter(|at| *at <= items.len())
                    .ok_or_else(|| format!("insert {pointer}: no list position {last:?}"))?;
                items.insert(at, value);
                rest
            }
            ["delete", pointer, rest @ ..] => {
                match parent(&mut document, pointer)? {
                    (Value::Object(object), key) if object.get(&key).is_some() => {
                        *object = object
                            .iter()
                            .filter(|(name, _)| **name != key)
                            .map(|(name, value)| (name.clone(), value.clone()))
                            .collect::<Object>();
                    }
                    (Value::Array(items), at) => {
                        let at = at
                            .parse::<usize>()
                            .ok()
                            .filter(|at| *at < items.len())
                            .ok_or_else(|| format!("delete {pointer}: no list item {at:?}"))?;
                        items.remove(at);
                    }
                    _ => return Err(format!("delete {pointer}: nothing to delete")),
                }
                rest
            }
            _ => return Err(format!("unknown edit {args:?}")),
        };
    }
    save(out, &document, newline)
}

/// The value holding a pointer's last reference token, and that token unescaped.
fn parent<'a>(document: &'a mut Value, pointer: &str) -> Result<(&'a mut Value, String), String> {
    let (path, last) = pointer
        .rsplit_once('/')
        .ok_or_else(|| format!("{pointer:?} is not a JSON pointer to a member"))?;
    let last = last.replace("~1", "/").replace("~0", "~");
    Ok((json::pointer_mut(document, path)?, last))
}

/// `parent[last] = value`: a new key goes last, as in a `dict`; a list position must already exist.
fn assign(document: &mut Value, pointer: &str, value: Value) -> Result<(), String> {
    match parent(document, pointer)? {
        (Value::Object(object), key) => object.insert(key, value),
        (Value::Array(items), at) => {
            let slot = at
                .parse::<usize>()
                .ok()
                .and_then(|at| items.get_mut(at))
                .ok_or_else(|| format!("set {pointer}: no list item {at:?}"))?;
            *slot = value;
        }
        (other, _) => {
            return Err(format!(
                "set {pointer}: the parent is a {}",
                other.type_name()
            ));
        }
    }
    Ok(())
}
