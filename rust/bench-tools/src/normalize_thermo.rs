//! Validate the thermo skill's verbatim finding index and map it to the shared schema, as
//! `bench/tools/normalize_thermo.py` did, byte for byte.
//!
//! The artifact root holds `summary.md` and `finding-index.json`, whose `findings` and `questions` entries
//! each quote a report under the root verbatim. The output is written as
//! `json.dumps(result, ensure_ascii=False, indent=2)` with a final newline, then one line is printed. A
//! quote that is not verbatim, or an empty index that the summary does not call empty, is unresolved and
//! exits 1; an index that cannot be used exits 2 with its message on stderr.

use crate::normalize_review::{Arm, Item, Normalized, Outcome};
use crate::pyjson::{self, Dumps, Duplicates, Value};
use crate::pytext::{self, PyPath};

pub const PROGRAM: &str = "normalize_thermo.py";

#[derive(Clone, Debug)]
pub struct Arguments {
    pub artifact_root: Vec<u8>,
    pub clone: Option<Vec<u8>>,
    pub out: Vec<u8>,
    pub arm: Arm,
}

/// One character of a lowercase pattern under `re.IGNORECASE`, which also folds `İ` and `ı` to `i` and `ſ`
/// to `s`.
fn folds(c: char, pattern: char) -> bool {
    match pattern {
        'i' => matches!(c, 'i' | 'I' | '\u{130}' | '\u{131}'),
        's' => matches!(c, 's' | 'S' | '\u{17f}'),
        _ => c.is_ascii() && c.to_ascii_lowercase() == pattern,
    }
}

fn literal(chars: &[char], at: usize, pattern: &str) -> Option<usize> {
    let mut index = at;
    for expected in pattern.chars() {
        if !chars.get(index).is_some_and(|c| folds(*c, expected)) {
            return None;
        }
        index += 1;
    }
    Some(index)
}

/// `\s+`, taking every whitespace character.
fn spaces(chars: &[char], at: usize) -> Option<usize> {
    let end = at
        + chars[at..]
            .iter()
            .take_while(|c| pytext::is_space(**c))
            .count();
    (end > at).then_some(end)
}

fn statement_at(chars: &[char], start: usize) -> bool {
    if start > 0 && pytext::is_word(chars[start - 1]) {
        return false;
    }
    let Some(after) = literal(chars, start, "no").and_then(|at| spaces(chars, at)) else {
        return false;
    };
    let actionable = |at| literal(chars, at, "actionable").and_then(|at| spaces(chars, at));
    let quality = |at| {
        literal(chars, at, "code")
            .filter(|at| matches!(chars.get(*at), Some('-' | ' ')))
            .and_then(|at| literal(chars, at + 1, "quality"))
            .and_then(|at| spaces(chars, at))
    };
    let options = [
        actionable(after).and_then(quality),
        actionable(after),
        quality(after),
        Some(after),
    ];
    options.into_iter().flatten().any(|at| {
        literal(chars, at, "findings")
            .is_some_and(|end| chars.get(end).is_none_or(|c| !pytext::is_word(*c)))
    })
}

/// `re.search(r"\bno\s+(?:actionable\s+)?(?:code[- ]quality\s+)?findings\b", summary, re.I)`.
pub fn explicit_empty(summary: &str) -> bool {
    let chars: Vec<char> = summary.chars().collect();
    (0..chars.len()).any(|start| statement_at(&chars, start))
}

fn read(path: &PyPath) -> Result<String, String> {
    pytext::read_text(path).map_err(|error| match error {
        pytext::ReadError::Os(message) => message,
        pytext::ReadError::Decode(message) => format!("{}: {message}", path.display()),
    })
}

fn within(path: &[u8], root: &[u8]) -> bool {
    path == root || root == b"/" || path.starts_with(root) && path.get(root.len()) == Some(&b'/')
}

/// The report an entry quotes: its path as written in the index, normalized, and its text.
fn report(root: &[u8], value: Option<&Value>) -> Result<(String, String), String> {
    let Some(Value::Str(value)) = value.filter(|value| value.truthy()) else {
        return Err("each indexed entry needs a non-empty report_path".to_string());
    };
    let relative = PyPath::new(value.as_bytes());
    let escapes = relative
        .as_bytes()
        .split(|byte| *byte == b'/')
        .any(|part| part == b"..");
    if value.starts_with('/') || escapes {
        return Err(format!(
            "report_path escapes artifact root: {}",
            pytext::repr_str(value)
        ));
    }
    let joined = PyPath::new(root).join(relative.as_bytes());
    let resolved = PyPath::new(&pytext::realpath(joined.as_bytes(), false)?);
    if !within(resolved.as_bytes(), root) || !resolved.is_file() {
        return Err(format!(
            "indexed report is missing or outside artifact root: {}",
            pytext::repr_str(value)
        ));
    }
    Ok((relative.display(), read(&resolved)?))
}

/// A positive integer's digits, or None for null.
fn line_number<'a>(entry: &'a pyjson::Object, name: &str) -> Result<Option<&'a str>, String> {
    match entry.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Int(digits)) if !digits.starts_with('-') && digits != "0" => Ok(Some(digits)),
        Some(_) => Err(format!("{name} must be a positive integer or null")),
    }
}

fn anchor(entry: &pyjson::Object, clone: Option<&[u8]>) -> Result<(Value, Value, Value), String> {
    let file = match entry.get("file") {
        None | Some(Value::Null) => Value::Null,
        Some(Value::Str(file)) => match clone {
            Some(clone) if file.starts_with('/') => Value::Str(
                pytext::relpath(file, clone)
                    .map_err(|error| format!("cannot read the current directory: {error}"))?,
            ),
            _ => Value::Str(file.clone()),
        },
        Some(_) => return Err("file anchor must be a string or null".to_string()),
    };
    let first = line_number(entry, "line_start")?;
    let last = line_number(entry, "line_end")?;
    if let (Some(first), Some(last)) = (first, last)
        && (last.len(), last) < (first.len(), first)
    {
        return Err("line_end precedes line_start".to_string());
    }
    let number =
        |digits: Option<&str>| digits.map_or(Value::Null, |digits| Value::Int(digits.to_string()));
    Ok((file, number(first), number(last)))
}

pub fn normalize(root: &[u8], clone: Option<&[u8]>, arm: Arm) -> Result<Value, String> {
    let root = pytext::realpath(PyPath::new(root).as_bytes(), true)?;
    let base = PyPath::new(&root);
    let summary = base.join(b"summary.md");
    let index_path = base.join(b"finding-index.json");
    if !summary.is_file() {
        return Err("summary.md is missing".to_string());
    }
    let summary = read(&summary)?;
    let index = read(&index_path)
        .and_then(|text| {
            pyjson::loads(&text, Duplicates::KeepLast).map_err(|error| error.to_string())
        })
        .map_err(|error| format!("cannot read {}: {error}", index_path.display()))?;
    let Some(Value::Array(findings)) = index.as_object().and_then(|index| index.get("findings"))
    else {
        return Err("finding-index.json must contain a findings array".to_string());
    };
    let questions = match index.as_object().and_then(|index| index.get("questions")) {
        None => &[][..],
        Some(Value::Array(questions)) => questions.as_slice(),
        Some(_) => return Err("finding-index.json questions must be an array".to_string()),
    };
    let empty = explicit_empty(&summary);
    let mut notes = Vec::new();
    let mut items = Vec::new();
    for (kind, rows) in [("finding", findings.as_slice()), ("question", questions)] {
        for (index, entry) in rows.iter().enumerate() {
            let number = index + 1;
            let entry = entry
                .as_object()
                .ok_or_else(|| format!("{kind} {number} must be an object"))?;
            let quote = match entry.get("quote") {
                Some(Value::Str(quote)) if !pytext::strip(quote).is_empty() => quote,
                _ => return Err(format!("{kind} {number} needs a non-empty exact quote")),
            };
            let (report_path, report_text) = report(&root, entry.get("report_path"))?;
            let title_verbatim = match entry.get("title") {
                None | Some(Value::Null) => true,
                Some(Value::Str(title)) => report_text.contains(title.as_str()),
                Some(_) => false,
            };
            if !report_text.contains(quote.as_str()) || !title_verbatim {
                notes.push(format!(
                    "unresolved: {kind} {number} quote/title is not verbatim in {report_path}"
                ));
            }
            let (file, line_start, line_end) = anchor(entry, clone)?;
            let mut fields = entry.clone();
            fields.insert("report_path".to_string(), Value::Str(report_path));
            items.push(Item {
                file,
                line_start,
                line_end,
                claim: Value::Str(quote.clone()),
                kind,
                native_fields: Some(Value::Object(fields)),
                ..Item::default()
            });
        }
    }
    if findings.is_empty() && !empty {
        notes.push(
            "unresolved: empty findings array without an explicit no-findings statement in summary.md"
                .to_string(),
        );
    }
    if empty && !findings.is_empty() {
        notes.push(
            "unresolved: summary says there are no findings but finding index is non-empty"
                .to_string(),
        );
    }
    let parse_status = if !notes.is_empty() {
        "unresolved"
    } else if items.is_empty() {
        "empty"
    } else {
        "parsed"
    };
    Ok(Normalized {
        parse_status,
        native_verdict: Value::Null,
        verdict_source: Value::Null,
        items,
        parse_notes: notes,
        native_payload: Some(index.clone()),
    }
    .into_value(arm.name()))
}

pub fn command(arguments: &Arguments) -> Outcome {
    let failed = |message: String| Outcome {
        code: 2,
        stderr: format!("{PROGRAM}: {message}\n").into_bytes(),
        ..Outcome::default()
    };
    let clone = match &arguments.clone {
        Some(clone) => match pytext::realpath(PyPath::new(clone).as_bytes(), false) {
            Ok(resolved) => Some(resolved),
            Err(message) => return failed(message),
        },
        None => None,
    };
    let result = match normalize(&arguments.artifact_root, clone.as_deref(), arguments.arm) {
        Ok(result) => result,
        Err(message) => return failed(message),
    };
    let document = pyjson::dumps(
        &result,
        &Dumps {
            ensure_ascii: false,
            indent: Some(2),
            ..Dumps::default()
        },
    );
    let out = PyPath::new(&arguments.out);
    if let Err(message) = pytext::write_file_text(out.as_bytes(), &format!("{document}\n")) {
        return failed(message);
    }
    let object = result.as_object();
    let status = object
        .and_then(|result| result.get("parse_status"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    let count = object
        .and_then(|result| result.get("items"))
        .and_then(Value::as_array)
        .map_or(0, <[Value]>::len);
    Outcome {
        code: u8::from(status == "unresolved"),
        stdout: format!("{status}: {count} indexed entries\n").into_bytes(),
        stderr: Vec::new(),
    }
}

pub fn arm(name: &[u8]) -> Option<Arm> {
    Arm::parse(name).filter(|arm| matches!(arm, Arm::CodexSkill | Arm::ClaudeSkill))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_findings_statement_follows_python_regex() {
        for summary in [
            "No actionable findings.",
            "No code quality findings.",
            "no actionable code-quality findings",
            "NO\tFINDINGS",
            "No fındings",
            "No findingſ",
            "(no findings)",
            "No findings\u{345}",
        ] {
            assert!(explicit_empty(summary), "{summary:?}");
        }
        for summary in [
            "Review complete.",
            "Ano findings",
            "No findings_x",
            "no-findings",
            "No actionablefindings",
            "Nofindings",
            "No code_quality findings",
            "No findingsé",
            "No findings\u{2160}",
        ] {
            assert!(!explicit_empty(summary), "{summary:?}");
        }
    }

    #[test]
    fn diagnostics_name_the_resolved_index() {
        let root = std::env::temp_dir().join(format!("bench-tools-thermo-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("reports")).unwrap();
        std::fs::write(root.join("reports/summary.md"), "No findings.").unwrap();
        std::fs::write(root.join("reports/finding-index.json"), "{bad").unwrap();
        let canonical = std::fs::canonicalize(root.join("reports")).unwrap();
        let arguments = Arguments {
            artifact_root: root.join("reports").into_os_string().into_encoded_bytes(),
            clone: None,
            out: root.join("out.json").into_os_string().into_encoded_bytes(),
            arm: Arm::CodexSkill,
        };
        let outcome = command(&arguments);
        assert_eq!(outcome.code, 2);
        assert_eq!(
            String::from_utf8(outcome.stderr).unwrap(),
            format!(
                "normalize_thermo.py: cannot read {}/finding-index.json: Expecting property name enclosed in double quotes: line 1 column 2 (char 1)\n",
                canonical.display()
            )
        );
        std::fs::remove_file(root.join("reports/finding-index.json")).unwrap();
        let outcome = command(&arguments);
        let index = format!("{}/finding-index.json", canonical.display());
        assert_eq!(
            String::from_utf8(outcome.stderr).unwrap(),
            format!(
                "normalize_thermo.py: cannot read {index}: [Errno 2] No such file or directory: '{index}'\n"
            )
        );
        assert!(!root.join("out.json").exists());
        std::fs::remove_dir_all(&root).unwrap();
    }
}
