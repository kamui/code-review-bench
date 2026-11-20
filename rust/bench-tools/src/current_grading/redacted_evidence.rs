//! The record of evidence files redacted after a claim or an eligibility decision pinned them: its checks, what it
//! changes in an input fingerprint, and `bench-tools redaction repin`, which writes it.
//!
//! A batch's input fingerprint describes what the batch was graded from. Of an evidence file, only its pinned hash
//! travels into the fingerprint; no grader receives the replaced values. So where a claim's evidence source or an
//! eligibility decision's evidence pin names a recorded file at its redacted hash, the fingerprint holds the hash the
//! file had before its first recorded redaction, and the batch stays current.

use super::{
    CURRENT, Failure, R, Root, canonical, dumps, each, file_hash, inconsistent, item, read_json,
    require, verify_pins,
};
use crate::pyjson::{Object, Value};
use crate::pytext::{self, PyPath};
use crate::{pyfs, pyvalue, redaction, sha256};
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Stdio};

/// Where an entry's receipt, the owner's approval for this path on this file, is saved.
const RULINGS: &str = "bench/grading/rulings/";
const CLAIMS: &str = "claims.json";
const DECISIONS: &str = "adjudications.json";

/// One redaction of one evidence file.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub path: String,
    pub before: String,
    pub blob: String,
    pub after: String,
    pub placeholders: Vec<redaction::Replaced>,
    /// The saved record of a history rewrite that removed the `before` blob.
    pub history: Option<String>,
}

fn text(value: &Value, key: &str) -> R<String> {
    Ok(item(value, key)?.py_str())
}

fn shown(entry: &Entry) -> String {
    format!("redacted evidence {}", entry.path)
}

/// The entries of the record in `directory`, none when it has no record, after checking the record's shape, its
/// receipt and history pins, that each file holds the bytes its last entry redacted to, and that each file's entries
/// form one chain.
pub fn load(root: &Root, directory: &PyPath) -> R<Vec<Entry>> {
    let name = redaction::RECORD
        .rsplit('/')
        .next()
        .unwrap_or(redaction::RECORD);
    let record_path = directory.join(name.as_bytes());
    if !record_path.to_path_buf().exists() {
        return Ok(Vec::new());
    }
    let record = read_json(&record_path)?;
    root.validate_schema("redacted-evidence", &record)?;
    verify_pins(root, &record)?;
    let mut entries: Vec<Entry> = Vec::new();
    for value in each(item(&record, "entries")?)?.iter() {
        let mut placeholders = Vec::new();
        for replaced in each(item(value, "placeholders")?)?.iter() {
            let count = text(replaced, "count")?;
            placeholders.push((
                text(replaced, "text")?.into_bytes(),
                count.parse().unwrap_or(usize::MAX),
            ));
        }
        let entry = Entry {
            path: text(value, "path")?,
            before: text(item(value, "before")?, "sha256")?,
            blob: text(item(value, "before")?, "blob")?,
            after: text(item(value, "after")?, "sha256")?,
            placeholders,
            history: match pyvalue::get(value, "history")? {
                Value::Null => None,
                pin => Some(text(pin, "path")?),
            },
        };
        require(
            text(item(value, "receipt")?, "path")?.starts_with(RULINGS),
            format!(
                "{}: the receipt of an entry is the owner's approval, saved under {RULINGS}",
                shown(&entry)
            ),
        )?;
        require(
            entry.before != entry.after,
            format!("{}: an entry records no change", shown(&entry)),
        )?;
        let earlier: Vec<&Entry> = entries
            .iter()
            .filter(|known| known.path == entry.path)
            .collect();
        require(
            !earlier.iter().any(|known| known.after == entry.after),
            format!(
                "{}: two entries record the same redacted bytes; remove the repeated entry",
                shown(&entry)
            ),
        )?;
        if let Some(last) = earlier.last() {
            require(
                last.after == entry.before,
                format!(
                    "{}: an entry does not start from the bytes the entry before it redacted to; a later redaction \
                     starts from the file's last recorded bytes",
                    shown(&entry)
                ),
            )?;
        }
        entries.push(entry);
    }
    for (at, entry) in entries.iter().enumerate() {
        if entries[at + 1..]
            .iter()
            .any(|later| later.path == entry.path)
        {
            continue;
        }
        require(
            file_hash(&root.local(entry.path.as_bytes())?)? == entry.after,
            format!(
                "{}: the file is not the redacted bytes its last entry records; restore it from git, or record its \
                 redaction with bench-tools redaction repin",
                shown(entry)
            ),
        )?;
    }
    Ok(entries)
}

/// The hash a pin of `path` at `sha256` had before the first recorded redaction of the file, if one is recorded.
fn earliest<'a>(entries: &'a [Entry], path: &str, sha256: &str) -> Option<&'a str> {
    entries
        .iter()
        .any(|entry| entry.path == path && entry.after == sha256)
        .then(|| {
            entries
                .iter()
                .find(|entry| entry.path == path)
                .map(|first| first.before.as_str())
        })
        .flatten()
}

/// `inputs` as their batch was graded: each claim evidence source and each eligibility decision evidence pin that
/// names a recorded file at a redacted hash names the file's earliest hash instead. Nothing else changes.
pub fn as_graded(inputs: &mut Value, entries: &[Entry]) {
    if entries.is_empty() {
        return;
    }
    let restore = |pin: &mut Value| {
        let Value::Object(pin) = pin else { return };
        let (Some(Value::Str(path)), Some(Value::Str(sha256))) =
            (pin.get("path"), pin.get("sha256"))
        else {
            return;
        };
        if let Some(first) = earliest(entries, path, sha256) {
            pin.insert("sha256".into(), Value::Str(first.to_string()));
        }
    };
    let Value::Object(inputs) = inputs else {
        return;
    };
    for (list, field) in [("claims", Some("source")), ("adjudications", None)] {
        let Some(Value::Array(records)) = inputs.get_mut(list) else {
            continue;
        };
        for record in records {
            let Value::Object(record) = record else {
                continue;
            };
            let Some(Value::Array(evidence)) = record.get_mut("evidence") else {
                continue;
            };
            for pinned in evidence {
                match field {
                    Some(field) => {
                        if let Value::Object(pinned) = pinned
                            && let Some(source) = pinned.get_mut(field)
                        {
                            restore(source);
                        }
                    }
                    None => restore(pinned),
                }
            }
        }
    }
}

/// The bytes of `blob` in the object store of `root`, or `None` when it holds no such blob.
fn read_blob(root: &Root, blob: &str) -> Option<Vec<u8>> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root.path.to_path_buf())
        .args(["cat-file", "blob", blob])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    output.status.success().then_some(output.stdout)
}

fn listed(placeholders: &[redaction::Replaced]) -> String {
    placeholders
        .iter()
        .map(|(text, count)| format!("{count} {}", String::from_utf8_lossy(text)))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The redaction-only re-check, which needs no private value: each entry's redacted bytes are its earlier bytes,
/// read from the object store, with values replaced by the placeholders it records, as
/// [`redaction::replaced_values`] decides. An entry whose blob is gone passes only with a history pin, and the
/// returned notes say which were not re-checked.
pub fn recheck(root: &Root, entries: &[Entry]) -> R<Vec<String>> {
    let mut notes = Vec::new();
    for (at, entry) in entries.iter().enumerate() {
        let later = entries[at + 1..]
            .iter()
            .find(|later| later.path == entry.path);
        let before = read_blob(root, &entry.blob);
        let after = match later {
            Some(later) => read_blob(root, &later.blob),
            None => Some(pyvalue::read_bytes(&root.local(entry.path.as_bytes())?)?),
        };
        let (before, after) = match (before, after) {
            (Some(before), Some(after)) => (before, after),
            (before, _) => {
                let gone = match (before, later) {
                    (Some(_), Some(later)) => later,
                    _ => entry,
                };
                match &gone.history {
                    Some(history) => {
                        notes.push(format!(
                        "{}: not re-checked, because blob {} is gone and {history} records the history rewrite",
                        shown(entry),
                        gone.blob
                    ));
                        continue;
                    }
                    None => {
                        return inconsistent(format!(
                            "{}: blob {} is not in the repository's object store, so the redaction cannot be re-checked; \
                         fetch the full history, or pin the saved record of the history rewrite that removed it as \
                         the entry's history",
                            shown(entry),
                            gone.blob
                        ));
                    }
                }
            }
        };
        require(
            sha256::hex_digest(&before) == entry.before,
            format!(
                "{}: blob {} does not hash to the entry's before sha256, so the entry names other earlier bytes",
                shown(entry),
                entry.blob
            ),
        )?;
        require(
            sha256::hex_digest(&after) == entry.after,
            format!(
                "{}: the next entry's blob does not hash to this entry's after sha256",
                shown(entry)
            ),
        )?;
        let stand_ins: Vec<&[u8]> = entry
            .placeholders
            .iter()
            .map(|(text, _)| text.as_slice())
            .filter(|text| !redaction::is_placeholder(text))
            .collect();
        let found = redaction::replaced_values(&before, &after, &stand_ins).map_err(
            |redaction::Error(reason)| {
                Failure::Inconsistent(format!("{}: {reason}", shown(entry)).into_bytes())
            },
        )?;
        let mut recorded = entry.placeholders.clone();
        recorded.sort();
        require(
            recorded == found,
            format!(
                "{}: the entry records {}, but the redacted bytes replace {}",
                shown(entry),
                listed(&recorded),
                listed(&found)
            ),
        )?;
    }
    Ok(notes)
}

/// A step from a JSON value to one of its members.
#[derive(Clone, Debug, PartialEq)]
enum Step {
    Key(Vec<u8>),
    Index(usize),
}

fn pointer(location: &[Step]) -> String {
    location
        .iter()
        .map(|step| match step {
            Step::Key(key) => format!("/{}", String::from_utf8_lossy(key)),
            Step::Index(at) => format!("/{at}"),
        })
        .collect()
}

/// Where each object with `"path": path` and a `sha256` sits in `value`, and that object.
fn pins_of(
    value: &Value,
    path: &str,
    location: &mut Vec<Step>,
    found: &mut Vec<(Vec<Step>, Object)>,
) {
    match value {
        Value::Object(object) => {
            if object.get("path").and_then(Value::as_str) == Some(path)
                && object.get("sha256").is_some()
            {
                found.push((location.clone(), object.clone()));
            }
            for (key, member) in object.iter() {
                location.push(Step::Key(key.as_bytes().to_vec()));
                pins_of(member, path, location, found);
                location.pop();
            }
        }
        Value::Array(members) => {
            for (at, member) in members.iter().enumerate() {
                location.push(Step::Index(at));
                pins_of(member, path, location, found);
                location.pop();
            }
        }
        _ => {}
    }
}

/// Reads JSON text far enough to give the byte range of each string value at one of the `wanted` locations.
struct Scanner<'a> {
    text: &'a [u8],
    at: usize,
    location: Vec<Step>,
    wanted: &'a [Vec<Step>],
    found: Vec<(usize, usize, usize)>,
}

impl Scanner<'_> {
    fn peek(&self) -> Option<u8> {
        self.text.get(self.at).copied()
    }

    fn space(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.at += 1;
        }
    }

    fn string(&mut self) -> Option<(usize, usize)> {
        self.at += 1;
        let start = self.at;
        loop {
            match self.peek()? {
                b'"' => {
                    self.at += 1;
                    return Some((start, self.at - 1));
                }
                b'\\' => self.at += 2,
                _ => self.at += 1,
            }
        }
    }

    fn members(
        &mut self,
        close: u8,
        mut member: impl FnMut(&mut Self, usize) -> Option<()>,
    ) -> Option<()> {
        self.at += 1;
        self.space();
        if self.peek()? == close {
            self.at += 1;
            return Some(());
        }
        let mut count = 0;
        loop {
            member(self, count)?;
            count += 1;
            self.space();
            match self.peek()? {
                b',' => self.at += 1,
                found if found == close => {
                    self.at += 1;
                    return Some(());
                }
                _ => return None,
            }
        }
    }

    fn value(&mut self) -> Option<()> {
        self.space();
        match self.peek()? {
            b'{' => self.members(b'}', |scanner, _| {
                scanner.space();
                if scanner.peek()? != b'"' {
                    return None;
                }
                let (start, end) = scanner.string()?;
                scanner.space();
                if scanner.peek()? != b':' {
                    return None;
                }
                scanner.at += 1;
                scanner
                    .location
                    .push(Step::Key(scanner.text[start..end].to_vec()));
                scanner.value()?;
                scanner.location.pop();
                Some(())
            }),
            b'[' => self.members(b']', |scanner, at| {
                scanner.location.push(Step::Index(at));
                scanner.value()?;
                scanner.location.pop();
                Some(())
            }),
            b'"' => {
                let (start, end) = self.string()?;
                if let Some(wanted) = self
                    .wanted
                    .iter()
                    .position(|wanted| *wanted == self.location)
                {
                    self.found.push((wanted, start, end));
                }
                Some(())
            }
            _ => {
                while !matches!(
                    self.peek(),
                    None | Some(b',' | b']' | b'}' | b' ' | b'\t' | b'\n' | b'\r')
                ) {
                    self.at += 1;
                }
                Some(())
            }
        }
    }
}

/// `raw` with the `sha256` string of each pin at `locations` changed from `old` to `new`, and nothing else changed:
/// the parsed result must equal `parsed` with exactly those hashes replaced.
fn rehashed(
    raw: &[u8],
    parsed: &Value,
    locations: &[Vec<Step>],
    old: &str,
    new: &str,
) -> R<Vec<u8>> {
    let wanted: Vec<Vec<Step>> = locations
        .iter()
        .map(|location| {
            let mut location = location.clone();
            location.push(Step::Key(b"sha256".to_vec()));
            location
        })
        .collect();
    let mut scanner = Scanner {
        text: raw,
        at: 0,
        location: Vec::new(),
        wanted: &wanted,
        found: Vec::new(),
    };
    let unchanged = || Failure::Crash("the document cannot be rewritten in place".into());
    scanner.value().ok_or_else(unchanged)?;
    let mut found = scanner.found;
    found.sort_by_key(|(wanted, _, _)| *wanted);
    let each_once = found
        .iter()
        .map(|(wanted, _, _)| *wanted)
        .eq(0..wanted.len());
    if !each_once
        || found
            .iter()
            .any(|(_, start, end)| raw[*start..*end] != *old.as_bytes())
    {
        return Err(unchanged());
    }
    let mut written = raw.to_vec();
    for (_, start, end) in &found {
        written[*start..*end].copy_from_slice(new.as_bytes());
    }
    let mut expected = parsed.clone();
    for location in locations {
        let mut value = &mut expected;
        for step in location {
            value = match (value, step) {
                (Value::Object(object), Step::Key(key)) => object
                    .get_mut(&String::from_utf8_lossy(key))
                    .ok_or_else(unchanged)?,
                (Value::Array(members), Step::Index(at)) => {
                    members.get_mut(*at).ok_or_else(unchanged)?
                }
                _ => return Err(unchanged()),
            };
        }
        if let Value::Object(pin) = value {
            pin.insert("sha256".into(), Value::Str(new.to_string()));
        }
    }
    let reread = pyvalue::loads(std::str::from_utf8(&written).map_err(|_| unchanged())?)?;
    if canonical(&reread) != canonical(&expected) {
        return Err(unchanged());
    }
    Ok(written)
}

/// Every JSON file at or below `directory`, by its path relative to `root`.
fn documents(root: &[u8], directory: &[u8], found: &mut Vec<Vec<u8>>) -> R<()> {
    let path = PyPath::new(root).join(directory);
    let entries = std::fs::read_dir(path.to_path_buf())
        .map_err(|error| Failure::Input(pytext::os_error(&error, path.as_bytes())))?;
    let mut names: Vec<Vec<u8>> = Vec::new();
    for entry in entries {
        let entry =
            entry.map_err(|error| Failure::Input(pytext::os_error(&error, path.as_bytes())))?;
        names.push(pytext::os_bytes(&entry.file_name()).to_vec());
    }
    names.sort();
    for name in names {
        let relative = [directory, b"/", &name].concat();
        let metadata = std::fs::symlink_metadata(PyPath::new(root).join(&relative).to_path_buf())
            .map_err(|error| Failure::Input(pytext::os_error(&error, &relative)))?;
        if metadata.is_dir() {
            documents(root, &relative, found)?;
        } else if metadata.is_file() && relative.ends_with(b".json") {
            found.push(relative);
        }
    }
    Ok(())
}

fn git(root: &Root, arguments: &[&str]) -> Option<Vec<u8>> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root.path.to_path_buf())
        .args(arguments)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    output.status.success().then_some(output.stdout)
}

fn blob_id(bytes: &[u8]) -> String {
    let mut blob = sha1_smol::Sha1::new();
    blob.update(format!("blob {}\0", bytes.len()).as_bytes());
    blob.update(bytes);
    blob.digest().to_string()
}

/// `name`, a path the command was given, relative to `root`.
fn relative(root: &Root, name: &[u8]) -> R<String> {
    let directory =
        std::env::current_dir().map_err(|error| Failure::Input(pytext::os_error(&error, b".")))?;
    let resolved = pyfs::resolve(
        PyPath::new(pytext::os_bytes(directory.as_os_str()))
            .join(name)
            .as_bytes(),
    );
    match pyfs::relative_parts(&resolved, root.path.as_bytes()) {
        Some(parts) if !parts.is_empty() => {
            Ok(String::from_utf8_lossy(&parts.join(&b'/')).into_owned())
        }
        _ => inconsistent(
            [
                pytext::stderr_text(name).as_bytes(),
                b" is outside the repository",
            ]
            .concat(),
        ),
    }
}

/// Writes `data` beside `path` and renames it over `path`, keeping the mode of an existing file.
fn write(path: &PyPath, data: &[u8]) -> R<()> {
    let target = path.to_path_buf();
    if target.exists() {
        return redaction::replace(&target, data)
            .map_err(|redaction::Error(message)| Failure::Input(message));
    }
    let failed = |error: std::io::Error| Failure::Input(pytext::os_error(&error, path.as_bytes()));
    let temporary = target.with_extension("json.repinning");
    let written = (|| {
        let mut file = std::fs::File::options()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(data)?;
        file.set_permissions(std::fs::Permissions::from_mode(0o644))?;
        file.sync_all()?;
        std::fs::rename(&temporary, &target)
    })();
    let _ = std::fs::remove_file(&temporary);
    written.map_err(failed)
}

/// `bench-tools redaction repin PATH --receipt RULING`: redact an evidence file that claims or eligibility decisions
/// pin, update exactly those pins and record the redaction, so their batches keep their fingerprints. Everything is
/// checked and built in memory first. The file is written first, then the record, then the documents, so an
/// interruption never leaves a pin naming bytes that do not exist, and the next run refuses the changed file.
pub fn repin(root: &Root, path: &[u8], receipt: &[u8], rules: &redaction::Rules) -> R<Vec<String>> {
    let path = relative(root, path)?;
    let file = root.local(path.as_bytes())?;
    let bytes = pyvalue::read_bytes(&file)?;
    let Some(committed) = git(
        root,
        &["rev-parse", "--verify", "--quiet", &format!("HEAD:{path}")],
    ) else {
        return inconsistent(format!(
            "{path} is not committed at HEAD; commit it first, so its earlier bytes stay in the repository's history"
        ));
    };
    require(
        String::from_utf8_lossy(&committed).trim() == blob_id(&bytes),
        format!(
            "{path} differs from the version committed at HEAD; commit it first, or, if an earlier repin was \
             interrupted, restore it and the current grading documents from HEAD and run repin again"
        ),
    )?;
    let receipt = relative(root, receipt)?;
    let receipt_file = root.local(receipt.as_bytes())?;
    require(
        receipt.starts_with(RULINGS) && receipt_file.to_path_buf().is_file(),
        format!("the receipt {receipt} is not a saved ruling under {RULINGS}"),
    )?;
    let redacted = redaction::redact(&bytes, rules);
    if let Some(finding) = redacted
        .findings
        .iter()
        .find(|finding| finding.unredactable())
    {
        return inconsistent(format!(
            "{}; repin cannot record it",
            finding.describe(&path)
        ));
    }
    let Some(clean) = redacted.data else {
        return inconsistent(format!(
            "the redaction rules do not change {path}; there is nothing to repin"
        ));
    };
    let placeholders = redaction::replaced_values(&bytes, &clean, &rules.stand_ins()).map_err(
        |redaction::Error(reason)| {
            Failure::Inconsistent(
                format!("the redaction-only re-check would refuse this redaction of {path} ({reason}); repin cannot record it")
                    .into_bytes(),
            )
        },
    )?;
    let old = sha256::hex_digest(&bytes);
    let new = sha256::hex_digest(&clean);
    let mut found = Vec::new();
    documents(root.path.as_bytes(), CURRENT.as_bytes(), &mut found)?;
    let mut rewritten: Vec<(String, usize, Vec<u8>)> = Vec::new();
    for document in found {
        let document = String::from_utf8_lossy(&document).into_owned();
        let raw = pyvalue::read_bytes(&root.local(document.as_bytes())?)?;
        if !raw
            .windows(path.len())
            .any(|window| window == path.as_bytes())
        {
            continue;
        }
        let parsed = read_json(&root.local(document.as_bytes())?)?;
        let mut pins = Vec::new();
        pins_of(&parsed, &path, &mut Vec::new(), &mut pins);
        if pins.is_empty() {
            continue;
        }
        let name = document
            .strip_prefix(&format!("{CURRENT}/"))
            .unwrap_or(&document);
        for (location, pin) in &pins {
            let evidence = pin.len() == 2
                && match (name, &location[..]) {
                    (
                        CLAIMS,
                        [
                            Step::Key(list),
                            Step::Index(_),
                            Step::Key(field),
                            Step::Index(_),
                            Step::Key(source),
                        ],
                    ) => list == b"claims" && field == b"evidence" && source == b"source",
                    (
                        DECISIONS,
                        [
                            Step::Key(list),
                            Step::Index(at),
                            Step::Key(field),
                            Step::Index(_),
                        ],
                    ) => {
                        list == b"decisions"
                            && field == b"evidence"
                            && each(item(&parsed, "decisions")?)?
                                .get(*at)
                                .map(|decision| text(decision, "dimension"))
                                .transpose()?
                                .as_deref()
                                == Some("eligibility")
                    }
                    _ => false,
                };
            require(
                evidence,
                format!(
                    "{document} pins {path} at {}, which is not claim or eligibility decision evidence; repin covers \
                     evidence files of claims and decisions only",
                    pointer(location)
                ),
            )?;
            require(
                pin.get("sha256").and_then(Value::as_str) == Some(old.as_str()),
                format!(
                    "{document} pins {path} at {} with a hash other than its current bytes; correct that pin first",
                    pointer(location)
                ),
            )?;
        }
        let locations: Vec<Vec<Step>> = pins.into_iter().map(|(location, _)| location).collect();
        let written = rehashed(&raw, &parsed, &locations, &old, &new)?;
        rewritten.push((document, locations.len(), written));
    }
    require(
        !rewritten.is_empty(),
        format!(
            "no claim or decision evidence pins {path}; redact it with bench-tools redaction apply"
        ),
    )?;
    let record_path = root.path.join(redaction::RECORD.as_bytes());
    let mut record = if record_path.to_path_buf().exists() {
        read_json(&record_path)?
    } else {
        Value::Object(
            [
                ("schema_version".to_string(), Value::Int("1".into())),
                ("entries".to_string(), Value::Array(Vec::new())),
            ]
            .into_iter()
            .collect(),
        )
    };
    let mut last = None;
    for entry in each(item(&record, "entries")?)?.iter() {
        if text(entry, "path")? == path {
            last = Some(text(item(entry, "after")?, "sha256")?);
        }
    }
    require(
        last.as_ref().is_none_or(|last| *last == old),
        format!(
            "{} records other bytes of {path} than the file holds; run bench-tools current-grading check",
            redaction::RECORD
        ),
    )?;
    let pin = |path: &str, sha256: String| {
        Value::Object(
            [
                ("path".to_string(), Value::Str(path.to_string())),
                ("sha256".to_string(), Value::Str(sha256)),
            ]
            .into_iter()
            .collect(),
        )
    };
    let entry: Object = [
        ("path".to_string(), Value::Str(path.clone())),
        (
            "before".to_string(),
            Value::Object(
                [
                    ("sha256".to_string(), Value::Str(old.clone())),
                    ("blob".to_string(), Value::Str(blob_id(&bytes))),
                ]
                .into_iter()
                .collect(),
            ),
        ),
        (
            "after".to_string(),
            Value::Object(
                [("sha256".to_string(), Value::Str(new.clone()))]
                    .into_iter()
                    .collect(),
            ),
        ),
        (
            "placeholders".to_string(),
            Value::Array(
                placeholders
                    .iter()
                    .map(|(text, count)| {
                        Value::Object(
                            [
                                (
                                    "text".to_string(),
                                    Value::Str(String::from_utf8_lossy(text).into_owned()),
                                ),
                                ("count".to_string(), Value::Int(count.to_string())),
                            ]
                            .into_iter()
                            .collect(),
                        )
                    })
                    .collect(),
            ),
        ),
        (
            "receipt".to_string(),
            pin(&receipt, file_hash(&receipt_file)?),
        ),
    ]
    .into_iter()
    .collect();
    if let Value::Object(record) = &mut record
        && let Some(Value::Array(entries)) = record.get_mut("entries")
    {
        entries.push(Value::Object(entry));
    }
    let record_text = format!("{}\n", dumps(&record));
    write(&file, &clean)?;
    write(&record_path, record_text.as_bytes())?;
    for (document, _, written) in &rewritten {
        write(&root.local(document.as_bytes())?, written)?;
    }
    let mut lines = vec![format!("redacted: {path} ({})", listed(&placeholders))];
    for (document, count, _) in &rewritten {
        lines.push(format!(
            "repinned: {document} ({count} pin{})",
            if *count == 1 { "" } else { "s" }
        ));
    }
    lines.push(format!("recorded: {}", redaction::RECORD));
    Ok(lines)
}
