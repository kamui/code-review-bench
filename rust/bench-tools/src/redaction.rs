//! Replace account identifiers and machine names in evidence before anything pins its hash, and find what is left.
//!
//! Two groups of rules apply. The field rules need no private value: the `organizationUuid` value Claude Code
//! records, the address in the sentence `The user's email address is …`, and the Codex `creator_user_id`,
//! `creator_account_id` and `plan_type` values, in plain JSON or inside JSON strings whose quotes are escaped any
//! number of times. The literal rules come from a private values file outside the repository
//! ([`values_path`]): each listed email address, organization id and account id becomes its placeholder, and each
//! machine name becomes its stand-in of the same length. A missing file leaves the field rules alone.
//!
//! [`redact`] follows the bytes into what it can rewrite: gzip streams and tar archives (rebuilt only when a member
//! changes, keeping names, modes and order). A SQLite database, a SQLite write-ahead log or a zip member that holds
//! an identifier is reported as beyond rewriting and left as it is, because a replacement cannot keep a database
//! valid without SQLite and a zip member would need the archive rebuilt. An xz stream is not opened. Bytes no rule
//! changes come back as `None`, so redaction never rewrites clean evidence.

use crate::archive;
use crate::pyjson::{self, Duplicates, Value};
use crate::pytext;
use regex::bytes::Regex;
use std::fmt;
use std::io::{self, Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::LazyLock;

pub const EMAIL: &[u8] = b"[redacted-email]";
pub const ORGANIZATION: &[u8] = b"[redacted-organization]";
pub const ACCOUNT: &[u8] = b"[redacted-account]";
pub const PLAN: &[u8] = b"[redacted-plan]";
/// The environment variable that names the private values file.
pub const VALUES_VARIABLE: &str = "BENCH_REDACTION_VALUES";
/// The values file under the home directory when [`VALUES_VARIABLE`] is unset.
pub const DEFAULT_VALUES: &str = ".config/code-review-bench/redaction.json";
/// The shortest literal a values file may list; a shorter one would replace ordinary text.
pub const SHORTEST_LITERAL: usize = 4;
const PLACEHOLDER_PREFIX: &[u8] = b"[redacted";
/// How far past a field's name its value may end.
const FIELD_WINDOW: usize = 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error(pub String);

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

type Result<T> = std::result::Result<T, Error>;

fn fail(message: impl Into<String>) -> Error {
    Error(message.into())
}

/// A field rule: where its name starts, the anchored pattern whose second group is the value, and the placeholder.
struct Field {
    anchor: &'static [u8],
    pattern: Regex,
    placeholder: &'static [u8],
}

fn field(anchor: &'static [u8], pattern: &str, placeholder: &'static [u8]) -> Field {
    Field {
        anchor,
        pattern: Regex::new(&format!(r#"(?-u)\A{pattern}"#)).expect("field patterns compile"),
        placeholder,
    }
}

const QUOTE: &str = r#"\\*""#;

static FIELDS: LazyLock<[Field; 4]> = LazyLock::new(|| {
    let named =
        |names: &str, value: &str| format!(r#"((?:{names}){QUOTE}\s*:\s*{QUOTE})({value}){QUOTE}"#);
    [
        field(
            b"organizationUuid",
            &named("organizationUuid", "[0-9A-Fa-f-]{36}"),
            ORGANIZATION,
        ),
        field(
            b"The user",
            r#"(The user(?:'|\\*\\u0027|’)s email address is )([A-Za-z0-9._%+-]{1,128}@[A-Za-z0-9.-]{0,253}[A-Za-z0-9])"#,
            EMAIL,
        ),
        field(
            b"creator_",
            &named("creator_user_id|creator_account_id", r#"[^"\\]{1,256}"#),
            ACCOUNT,
        ),
        field(
            b"plan_type",
            &named("plan_type", "[A-Za-z0-9_-]{2,20}"),
            PLAN,
        ),
    ]
});

fn find(haystack: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    let first = *needle.first()?;
    let mut at = from;
    while at + needle.len() <= haystack.len() {
        let offset = haystack[at..=haystack.len() - needle.len()]
            .iter()
            .position(|byte| *byte == first)?;
        at += offset;
        if haystack[at..at + needle.len()] == *needle {
            return Some(at);
        }
        at += 1;
    }
    None
}

/// Rewrite each place `anchor` occurs where `at` returns how many bytes it replaces and with what. `None` when
/// nothing differs.
fn rewrite(
    data: &[u8],
    anchor: &[u8],
    mut at: impl FnMut(&[u8]) -> Option<(usize, Vec<u8>)>,
) -> Option<Vec<u8>> {
    let mut out: Option<Vec<u8>> = None;
    let (mut copied, mut from) = (0, 0);
    while let Some(start) = find(data, anchor, from) {
        match at(&data[start..]) {
            Some((length, new)) if data[start..start + length] != *new => {
                let out = out.get_or_insert_with(|| Vec::with_capacity(data.len()));
                out.extend_from_slice(&data[copied..start]);
                out.extend_from_slice(&new);
                copied = start + length;
                from = copied;
            }
            Some((length, _)) => from = start + length.max(1),
            None => from = start + 1,
        }
    }
    let mut out = out?;
    out.extend_from_slice(&data[copied..]);
    Some(out)
}

/// Apply `step` to the latest bytes, keeping `None` while no step has changed anything.
fn chain(
    current: &mut Option<Vec<u8>>,
    original: &[u8],
    step: impl FnOnce(&[u8]) -> Option<Vec<u8>>,
) {
    if let Some(next) = step(current.as_deref().unwrap_or(original)) {
        *current = Some(next);
    }
}

/// A literal value and the placeholder that replaces it.
#[derive(Clone, Debug)]
struct Literal {
    value: Vec<u8>,
    placeholder: &'static [u8],
}

/// The rules [`redact`] applies.
#[derive(Clone, Debug, Default)]
pub struct Rules {
    literals: Vec<Literal>,
    hosts: Vec<(Vec<u8>, Vec<u8>)>,
    values: Option<PathBuf>,
}

/// `BENCH_REDACTION_VALUES` when set and not empty, else [`DEFAULT_VALUES`] under `HOME`.
pub fn values_path() -> Option<PathBuf> {
    match std::env::var_os(VALUES_VARIABLE) {
        Some(path) if !path.is_empty() => Some(PathBuf::from(path)),
        _ => std::env::var_os("HOME")
            .filter(|home| !home.is_empty())
            .map(|home| PathBuf::from(home).join(DEFAULT_VALUES)),
    }
}

impl Rules {
    /// The field rules alone.
    pub fn fields() -> Self {
        Self::default()
    }

    /// The field rules and the values file at [`values_path`], if there is one.
    pub fn load() -> Result<Self> {
        match values_path() {
            Some(path) => Self::from_path(&path),
            None => Ok(Self::fields()),
        }
    }

    /// The field rules and the values file at `path`; a missing file adds nothing.
    pub fn from_path(path: &Path) -> Result<Self> {
        let shown = path.display();
        match std::fs::read(path) {
            Ok(text) => {
                let mut rules = Self::from_values(&text).map_err(|Error(message)| {
                    fail(format!("redaction values {shown}: {message}"))
                })?;
                rules.values = Some(path.to_path_buf());
                Ok(rules)
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Self::fields()),
            Err(error) => Err(fail(format!(
                "cannot read redaction values {shown}: {error}"
            ))),
        }
    }

    /// The field rules and the values a values file holds. Messages never quote a value.
    pub fn from_values(text: &[u8]) -> Result<Self> {
        let text = std::str::from_utf8(text).map_err(|_| fail("the file is not UTF-8"))?;
        let document = pyjson::loads(text, Duplicates::Refuse)
            .map_err(|error| fail(format!("not JSON: {error}")))?;
        let object = document
            .as_object()
            .ok_or_else(|| fail("the file is not a JSON object"))?;
        for (key, _) in object.iter() {
            if ![
                "schema_version",
                "email",
                "organization",
                "account",
                "hosts",
            ]
            .contains(&key.as_str())
            {
                return Err(fail(format!("unknown key {}", pytext::repr_str(key))));
            }
        }
        if !matches!(object.get("schema_version"), Some(Value::Int(version)) if version == "1") {
            return Err(fail("schema_version must be 1"));
        }
        let mut rules = Self::fields();
        for (key, placeholder) in [
            ("email", EMAIL),
            ("organization", ORGANIZATION),
            ("account", ACCOUNT),
        ] {
            let Some(listed) = object.get(key) else {
                continue;
            };
            let listed = listed
                .as_array()
                .ok_or_else(|| fail(format!("{key} must be a list of strings")))?;
            for value in listed {
                let value = value
                    .as_str()
                    .ok_or_else(|| fail(format!("{key} must be a list of strings")))?;
                if value.len() < SHORTEST_LITERAL {
                    return Err(fail(format!(
                        "an {key} value is shorter than {SHORTEST_LITERAL} bytes"
                    )));
                }
                rules.literals.push(Literal {
                    value: value.as_bytes().to_vec(),
                    placeholder,
                });
            }
        }
        if let Some(hosts) = object.get("hosts") {
            let hosts = hosts
                .as_object()
                .ok_or_else(|| fail("hosts must map each machine name to its stand-in"))?;
            for (name, stand_in) in hosts.iter() {
                let stand_in = stand_in
                    .as_str()
                    .ok_or_else(|| fail("hosts must map each machine name to its stand-in"))?;
                if name.len() < SHORTEST_LITERAL {
                    return Err(fail(format!(
                        "a machine name is shorter than {SHORTEST_LITERAL} bytes"
                    )));
                }
                if stand_in.len() != name.len() {
                    return Err(fail(format!(
                        "the stand-in {} differs in length from the machine name it replaces",
                        pytext::repr_str(stand_in)
                    )));
                }
                rules
                    .hosts
                    .push((name.as_bytes().to_vec(), stand_in.as_bytes().to_vec()));
            }
        }
        let written: Vec<&[u8]> = [EMAIL, ORGANIZATION, ACCOUNT, PLAN]
            .into_iter()
            .chain(rules.hosts.iter().map(|(_, stand_in)| stand_in.as_slice()))
            .collect();
        let values = rules
            .literals
            .iter()
            .map(|literal| literal.value.as_slice())
            .chain(rules.hosts.iter().map(|(name, _)| name.as_slice()));
        for value in values {
            if written.iter().any(|text| find(text, value, 0).is_some()) {
                return Err(fail(
                    "a value occurs inside a placeholder or stand-in, so redaction would not settle",
                ));
            }
        }
        Ok(rules)
    }

    /// The values file these rules read, or `None` when only the field rules apply.
    pub fn values_file(&self) -> Option<&Path> {
        self.values.as_deref()
    }

    fn apply(&self, data: &[u8]) -> Option<Vec<u8>> {
        let mut current = None;
        for literal in &self.literals {
            chain(&mut current, data, |bytes| {
                rewrite(bytes, &literal.value, |_| {
                    Some((literal.value.len(), literal.placeholder.to_vec()))
                })
            });
        }
        for field in FIELDS.iter() {
            chain(&mut current, data, |bytes| {
                rewrite(bytes, field.anchor, |rest| {
                    let window = &rest[..rest.len().min(FIELD_WINDOW)];
                    let found = field.pattern.captures(window)?;
                    let (name, value) = (found.get(1)?, found.get(2)?);
                    let value_bytes = value.as_bytes();
                    if value_bytes.starts_with(PLACEHOLDER_PREFIX) {
                        return None;
                    }
                    let mut new = name.as_bytes().to_vec();
                    new.extend_from_slice(field.placeholder);
                    Some((value.end(), new))
                })
            });
        }
        for (name, stand_in) in &self.hosts {
            chain(&mut current, data, |bytes| {
                rewrite(bytes, name, |_| Some((name.len(), stand_in.clone())))
            });
        }
        current
    }
}

/// Something [`redact`] found: a member path inside the bytes (empty for the bytes themselves), and why it cannot
/// be rewritten when it cannot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Finding {
    pub member: Vec<Vec<u8>>,
    pub unredactable: Option<&'static str>,
}

impl Finding {
    /// `path`, then `!` and each member name, then the reason a member cannot be rewritten.
    pub fn describe(&self, path: &str) -> String {
        let mut shown = path.to_string();
        for name in &self.member {
            shown.push('!');
            shown.push_str(&String::from_utf8_lossy(name));
        }
        match self.unredactable {
            Some(reason) => format!("cannot redact: {shown} ({reason})"),
            None => format!("would change: {shown}"),
        }
    }
}

impl Finding {
    /// Whether the finding is something the tool cannot rewrite.
    pub fn unredactable(&self) -> bool {
        self.unredactable.is_some()
    }
}

/// What to do about a file `redaction check` lists as one the tool cannot rewrite.
pub const LEAVE_OUT: &str = "redact each file marked cannot redact by other means or leave it out, and do not commit or publish it";

/// What a refusal tells the reader to do about `findings`: `redaction apply` for a file that would change, other
/// means or leaving it out for a file the tool cannot rewrite.
pub fn remedy(findings: &[Finding]) -> String {
    let mut steps = Vec::new();
    if findings.iter().any(|finding| !finding.unredactable()) {
        steps.push("run bench-tools redaction apply on each file marked would change before anything pins its hash");
    }
    if findings.iter().any(Finding::unredactable) {
        steps.push(LEAVE_OUT);
    }
    steps.join("; ")
}

/// The redacted bytes, when any rule changed them, and what was found.
#[derive(Clone, Debug, Default)]
pub struct Redacted {
    pub data: Option<Vec<u8>>,
    pub findings: Vec<Finding>,
}

impl Redacted {
    /// Nothing would change and nothing is beyond rewriting.
    pub fn clean(&self) -> bool {
        self.data.is_none() && self.findings.is_empty()
    }

    /// The redacted bytes, or `original` when nothing changed.
    pub fn bytes<'a>(&'a self, original: &'a [u8]) -> &'a [u8] {
        self.data.as_deref().unwrap_or(original)
    }
}

/// Redact `data` at every level this module reads.
pub fn redact(data: &[u8], rules: &Rules) -> Redacted {
    let mut findings = Vec::new();
    let data = transform(data, rules, &mut Vec::new(), &mut findings);
    Redacted { data, findings }
}

/// Whether redaction would change `data` or finds something it cannot rewrite.
pub fn would_change(data: &[u8], rules: &Rules) -> bool {
    !redact(data, rules).clean()
}

const XZ: &[u8] = b"\xfd7zXZ\x00";
const SQLITE: &[u8] = b"SQLite format 3\0";
const WAL: [&[u8]; 2] = [b"\x37\x7f\x06\x82", b"\x37\x7f\x06\x83"];

fn transform(
    data: &[u8],
    rules: &Rules,
    member: &mut Vec<Vec<u8>>,
    findings: &mut Vec<Finding>,
) -> Option<Vec<u8>> {
    if data.starts_with(b"\x1f\x8b") {
        return gzip(data, rules, member, findings);
    }
    if data.len() > 262 && &data[257..262] == b"ustar" {
        return tar(data, rules, member, findings);
    }
    if data.starts_with(XZ) {
        return None;
    }
    if data.starts_with(b"PK\x03\x04") {
        zip(data, rules, member, findings);
        return None;
    }
    // A replacement cannot keep a database valid without SQLite: an index, a constraint or a full-text index may
    // hold the value in another form, and a write-ahead log beside the file may hold pages that index it.
    if data.starts_with(SQLITE) || WAL.iter().any(|magic| data.starts_with(magic)) {
        if rules.apply(data).is_some() {
            findings.push(Finding {
                member: member.clone(),
                unredactable: Some("SQLite files are not rewritten"),
            });
        }
        return None;
    }
    let changed = rules.apply(data);
    if changed.is_some() {
        findings.push(Finding {
            member: member.clone(),
            unredactable: None,
        });
    }
    changed
}

fn gzip(
    data: &[u8],
    rules: &Rules,
    member: &mut Vec<Vec<u8>>,
    findings: &mut Vec<Finding>,
) -> Option<Vec<u8>> {
    let mut inner = Vec::new();
    if archive::GzipReader::new(io::BufReader::new(data))
        .read_to_end(&mut inner)
        .is_err()
    {
        let changed = rules.apply(data);
        if changed.is_some() {
            findings.push(Finding {
                member: member.clone(),
                unredactable: None,
            });
        }
        return changed;
    }
    let clean = transform(&inner, rules, member, findings)?;
    let mut writer = archive::gzip_writer(Vec::new(), 9);
    writer.write_all(&clean).ok()?;
    writer.finish().ok()
}

/// The `path` and whether a `size` record appears in a PAX header's records.
fn pax(records: &[u8]) -> (Option<Vec<u8>>, bool) {
    let mut path = None;
    let mut size = false;
    for record in records.split(|byte| *byte == b'\n') {
        let Some(space) = record.iter().position(|byte| *byte == b' ') else {
            continue;
        };
        let field = &record[space + 1..];
        if let Some(value) = field.strip_prefix(b"path=") {
            path = Some(value.to_vec());
        } else if field.starts_with(b"size=") {
            size = true;
        }
    }
    (path, size)
}

fn tar(
    data: &[u8],
    rules: &Rules,
    member: &mut Vec<Vec<u8>>,
    findings: &mut Vec<Finding>,
) -> Option<Vec<u8>> {
    let read = (|| -> io::Result<Option<Vec<u8>>> {
        let mut bundle = tar::Archive::new(data);
        let mut rows: Vec<(tar::Header, Vec<u8>)> = Vec::new();
        let mut changed = false;
        let (mut long_name, mut pax_size) = (None, false);
        for entry in bundle.entries()?.raw(true) {
            let mut entry = entry?;
            let mut header = entry.header().clone();
            let mut body = Vec::new();
            entry.read_to_end(&mut body)?;
            match header.as_bytes()[156] {
                b'x' => {
                    let (path, size) = pax(&body);
                    long_name = long_name.or(path);
                    pax_size |= size;
                }
                b'L' => {
                    let end = body
                        .iter()
                        .position(|byte| *byte == 0)
                        .unwrap_or(body.len());
                    long_name = long_name.or_else(|| Some(body[..end].to_vec()));
                }
                b'K' => {}
                b'0' | 0 | b'7' => {
                    let name = long_name
                        .take()
                        .unwrap_or_else(|| header.path_bytes().into_owned());
                    member.push(name);
                    let clean = transform(&body, rules, member, findings);
                    if let Some(clean) = clean {
                        if pax_size {
                            findings.push(Finding {
                                member: member.clone(),
                                unredactable: Some("its size is in a PAX record"),
                            });
                        } else {
                            header.set_size(clean.len() as u64);
                            header.set_cksum();
                            body = clean;
                            changed = true;
                        }
                    }
                    member.pop();
                    pax_size = false;
                }
                _ => (long_name, pax_size) = (None, false),
            }
            rows.push((header, body));
        }
        if !changed {
            return Ok(None);
        }
        let mut builder = tar::Builder::new(Vec::new());
        for (header, body) in &rows {
            builder.append(header, body.as_slice())?;
        }
        builder.into_inner().map(Some)
    })();
    read.unwrap_or_else(|_| {
        findings.push(Finding {
            member: member.clone(),
            unredactable: Some("the tar archive cannot be read"),
        });
        None
    })
}

fn zip(data: &[u8], rules: &Rules, member: &mut Vec<Vec<u8>>, findings: &mut Vec<Finding>) {
    let Ok(mut bundle) = zip::ZipArchive::new(io::Cursor::new(data)) else {
        return;
    };
    for index in 0..bundle.len() {
        let Ok(mut file) = bundle.by_index(index) else {
            continue;
        };
        if !file.is_file() {
            continue;
        }
        let name = file.name_raw().to_vec();
        let mut body = Vec::new();
        if file.read_to_end(&mut body).is_err() {
            continue;
        }
        member.push(name);
        if would_change(&body, rules) {
            findings.push(Finding {
                member: member.clone(),
                unredactable: Some("zip members are not rewritten"),
            });
        }
        member.pop();
    }
}

/// What `redaction check` reads.
#[derive(Clone, Debug, Default)]
pub struct Sources {
    pub paths: Vec<PathBuf>,
    pub staged: bool,
    pub message: Option<PathBuf>,
    pub changed: Option<String>,
}

fn git(directory: Option<&Path>, arguments: &[&str]) -> Result<Vec<u8>> {
    let mut command = Command::new("git");
    if let Some(directory) = directory {
        command.arg("-C").arg(directory);
    }
    let output = command
        .args(arguments)
        .stdin(Stdio::null())
        .output()
        .map_err(|error| fail(format!("cannot run git: {error}")))?;
    if !output.status.success() {
        return Err(fail(format!(
            "git {} failed: {}",
            arguments.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(output.stdout)
}

fn top_level() -> Result<PathBuf> {
    let output = git(None, &["rev-parse", "--show-toplevel"])?;
    let text = output.strip_suffix(b"\n").unwrap_or(&output);
    Ok(PathBuf::from(std::ffi::OsStr::new(
        std::str::from_utf8(text).map_err(|_| fail("the repository path is not UTF-8"))?,
    )))
}

/// The index's blob of each staged file that is added, copied, modified or retyped, by path.
fn staged(top: &Path) -> Result<Vec<(String, Vec<u8>)>> {
    let raw = git(
        Some(top),
        &[
            "diff",
            "--cached",
            "--raw",
            "-z",
            "--no-renames",
            "--no-abbrev",
            "--diff-filter=d",
        ],
    )?;
    let mut fields = raw.split(|byte| *byte == 0).filter(|part| !part.is_empty());
    let mut listed = Vec::new();
    while let (Some(status), Some(path)) = (fields.next(), fields.next()) {
        let status = String::from_utf8_lossy(status);
        let columns: Vec<&str> = status.split(' ').collect();
        if columns.len() < 4 || columns[1] == "160000" {
            continue;
        }
        listed.push((
            String::from_utf8_lossy(path).into_owned(),
            columns[3].to_string(),
        ));
    }
    if listed.is_empty() {
        return Ok(Vec::new());
    }
    let mut child = Command::new("git")
        .arg("-C")
        .arg(top)
        .args(["cat-file", "--batch"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|error| fail(format!("cannot run git: {error}")))?;
    let mut input = child.stdin.take().expect("stdin is piped");
    let ids = listed
        .iter()
        .map(|(_, id)| id.as_str())
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    let writer = std::thread::spawn(move || input.write_all(ids.as_bytes()));
    let mut output = io::BufReader::new(child.stdout.take().expect("stdout is piped"));
    let mut blobs = Vec::with_capacity(listed.len());
    let unreadable = |error: io::Error| fail(format!("cannot read staged content: {error}"));
    for (path, id) in listed {
        let mut header = Vec::new();
        io::BufRead::read_until(&mut output, b'\n', &mut header).map_err(unreadable)?;
        let header = String::from_utf8_lossy(&header);
        let size: usize = match header.trim_end().split(' ').collect::<Vec<_>>()[..] {
            [found, "blob", size] if found == id => size
                .parse()
                .map_err(|_| fail(format!("git cat-file gave an unreadable header for {path}")))?,
            _ => return Err(fail(format!("git cat-file has no blob for {path}"))),
        };
        let mut blob = vec![0; size + 1];
        output.read_exact(&mut blob).map_err(unreadable)?;
        blob.pop();
        blobs.push((path, blob));
    }
    writer
        .join()
        .map_err(|_| fail("git cat-file input failed"))?
        .map_err(unreadable)?;
    let status = child.wait().map_err(unreadable)?;
    if !status.success() {
        return Err(fail("git cat-file failed"));
    }
    Ok(blobs)
}

/// The tracked files that differ between the merge base of `base` and `HEAD`, and the working tree.
fn changed_since(top: &Path, base: &str) -> Result<Vec<PathBuf>> {
    let merge_base = git(Some(top), &["merge-base", base, "HEAD"])?;
    let merge_base = String::from_utf8_lossy(&merge_base).trim().to_string();
    let names = git(
        Some(top),
        &[
            "diff",
            "--name-only",
            "-z",
            "--no-renames",
            "--diff-filter=d",
            &merge_base,
        ],
    )?;
    Ok(names
        .split(|byte| *byte == 0)
        .filter(|name| !name.is_empty())
        .map(|name| top.join(std::ffi::OsStr::new(&*String::from_utf8_lossy(name))))
        .collect())
}

/// Every regular file at or below `path`, skipping `.git` and symbolic links.
fn files(path: &Path, found: &mut Vec<PathBuf>) -> Result<()> {
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|error| fail(pytext::os_error(&error, pytext::os_bytes(path.as_os_str()))))?;
    if metadata.is_dir() {
        let mut entries: Vec<PathBuf> = std::fs::read_dir(path)
            .and_then(|entries| {
                entries
                    .map(|entry| entry.map(|entry| entry.path()))
                    .collect()
            })
            .map_err(|error| fail(pytext::os_error(&error, pytext::os_bytes(path.as_os_str()))))?;
        entries.sort();
        for entry in entries {
            if entry.file_name() != Some(std::ffi::OsStr::new(".git")) {
                files(&entry, found)?;
            }
        }
    } else if metadata.is_file() {
        found.push(path.to_path_buf());
    }
    Ok(())
}

fn read(path: &Path) -> Result<Vec<u8>> {
    std::fs::read(path)
        .map_err(|error| fail(pytext::os_error(&error, pytext::os_bytes(path.as_os_str()))))
}

fn shown(path: &Path) -> String {
    pytext::stderr_text(pytext::os_bytes(path.as_os_str()))
}

/// What `redaction check` found: one line for each file or member that redaction would change or cannot rewrite,
/// and the findings behind them.
#[derive(Clone, Debug, Default)]
pub struct Checked {
    pub lines: Vec<String>,
    pub findings: Vec<Finding>,
}

/// `redaction check`: what redaction would change or cannot rewrite in `sources`.
pub fn check(sources: &Sources, rules: &Rules) -> Result<Checked> {
    let mut checked = Checked::default();
    let mut scan = |name: &str, data: &[u8]| {
        for finding in redact(data, rules).findings {
            checked.lines.push(finding.describe(name));
            checked.findings.push(finding);
        }
    };
    let mut paths = Vec::new();
    for path in &sources.paths {
        files(path, &mut paths)?;
    }
    if let Some(base) = &sources.changed {
        paths.extend(changed_since(&top_level()?, base)?);
    }
    for path in &paths {
        scan(&shown(path), &read(path)?);
    }
    if let Some(message) = &sources.message {
        scan(&shown(message), &read(message)?);
    }
    if sources.staged {
        for (path, blob) in staged(&top_level()?)? {
            scan(&path, &blob);
        }
    }
    Ok(checked)
}

/// What `redaction apply` did: the files it rewrote, and a line for each member it cannot rewrite.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Applied {
    pub rewritten: Vec<String>,
    pub unredactable: Vec<String>,
}

/// `redaction apply`: rewrite each file redaction changes, in place and in one step, keeping its mode.
pub fn apply(paths: &[PathBuf], rules: &Rules) -> Result<Applied> {
    let mut found = Vec::new();
    for path in paths {
        files(path, &mut found)?;
    }
    let mut applied = Applied::default();
    for path in &found {
        let data = read(path)?;
        let redacted = redact(&data, rules);
        let name = shown(path);
        applied.unredactable.extend(
            redacted
                .findings
                .iter()
                .filter(|finding| finding.unredactable())
                .map(|finding| finding.describe(&name)),
        );
        if let Some(clean) = &redacted.data {
            replace(path, clean)?;
            applied.rewritten.push(name);
        }
    }
    Ok(applied)
}

/// Write `data` beside `path` and rename it over `path`, with `path`'s permissions.
pub fn replace(path: &Path, data: &[u8]) -> Result<()> {
    let failed =
        |error: io::Error| fail(pytext::os_error(&error, pytext::os_bytes(path.as_os_str())));
    let mode = std::fs::metadata(path)
        .map_err(failed)?
        .permissions()
        .mode();
    let directory = path.parent().unwrap_or(Path::new("."));
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    let temporary = directory.join(format!(
        ".{name}.{}.redacting",
        crate::evidence::random_name("")
    ));
    let written = (|| {
        let mut file = std::fs::File::options()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(data)?;
        file.set_permissions(std::fs::Permissions::from_mode(mode))?;
        file.sync_all()?;
        std::fs::rename(&temporary, path)
    })();
    let _ = std::fs::remove_file(&temporary);
    written.map_err(failed)
}

#[cfg(test)]
mod tests;
