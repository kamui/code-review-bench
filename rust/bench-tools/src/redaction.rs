//! Replace account identifiers and machine names in evidence before anything pins its hash, and find what is left.
//!
//! Two groups of rules apply. The field rules need no private value: the `organizationUuid` value Claude Code
//! records, the address in the sentence `The user's email address is …`, and the Codex `creator_user_id`,
//! `creator_account_id` and `plan_type` values, in plain JSON or inside JSON strings whose quotes are escaped any
//! number of times. The literal rules come from a private values file outside the repository
//! ([`values_path`]): each listed email address, organization id and account id becomes its placeholder, and each
//! machine name becomes its stand-in of the same length, in each form [`written_forms`] matches. A missing file leaves the field rules alone. The
//! structure rules replace the owner's personal agent instructions, the list of installed skills a Codex session
//! gives, the Windows account in a `/mnt/<drive>/Users/` path and the skill folder names under it.
//!
//! [`redact`] follows the bytes into what it can rewrite: gzip streams and tar archives (rebuilt only when a member
//! changes, keeping names, modes and order). A SQLite database, a SQLite write-ahead log or a zip member that holds
//! an identifier is reported as beyond rewriting and left as it is, because a replacement cannot keep a database
//! valid without SQLite and a zip member would need the archive rebuilt. An xz stream is not opened. Bytes no rule
//! changes come back as `None`, so redaction never rewrites clean evidence.
//!
//! [`replaced_values`] decides, without private values, whether redacted bytes are earlier bytes with only values
//! replaced. With `--staged`, [`check`] also requires each entry a commit adds to [`RECORD`] to be exactly what the
//! rules make of the file at `HEAD`, and the entries already at `HEAD` to stay as they were but for their `history`
//! pins; the commit hook runs it with the values file.

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
pub const INSTRUCTIONS: &[u8] = b"[redacted-instructions]";
pub const USER: &[u8] = b"[redacted-user]";
pub const SKILL: &[u8] = b"[redacted-skill]";
pub const SKILL_LIST: &[u8] = b"[redacted-skill-list]";
/// The environment variable that names the private values file.
pub const VALUES_VARIABLE: &str = "BENCH_REDACTION_VALUES";
/// The values file under the home directory when [`VALUES_VARIABLE`] is unset.
pub const DEFAULT_VALUES: &str = ".config/code-review-bench/redaction.json";
/// The shortest literal a values file may list; a shorter one would replace ordinary text.
pub const SHORTEST_LITERAL: usize = 4;
const PLACEHOLDER_PREFIX: &[u8] = b"[redacted";
/// How far past a field's name its value may end.
const FIELD_WINDOW: usize = 1024;
/// The record of evidence files redacted after a claim or decision pinned them, which `redaction repin` writes.
pub const RECORD: &str = "bench/grading/current/redacted-evidence.json";

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

/// Where the owner's personal agent instructions start inside a JSON string, newlines escaped once.
const INSTRUCTIONS_ANCHOR: &[u8] =
    b"# AGENTS.md\\n\\nMake the smallest complete change that satisfies the request";
/// What ends those instructions inside the string, besides an unescaped quote.
const INSTRUCTIONS_ENDS: [&[u8]; 4] = [
    b"\\n</system-reminder>",
    b"\\n</INSTRUCTIONS>",
    b"\\n\\n--- project-doc ---",
    b"\\n\\nContents of ",
];

/// The owner's personal instructions, from [`INSTRUCTIONS_ANCHOR`] to the first end, become [`INSTRUCTIONS`].
fn instruction_block(rest: &[u8]) -> Option<(usize, Vec<u8>)> {
    let mut at = INSTRUCTIONS_ANCHOR.len();
    while at < rest.len() {
        if rest[at] == b'"'
            || INSTRUCTIONS_ENDS
                .iter()
                .any(|end| rest[at..].starts_with(end))
        {
            return Some((at, INSTRUCTIONS.to_vec()));
        }
        at += if rest[at] == b'\\' { 2 } else { 1 };
    }
    None
}

/// Where the list of installed skills starts inside a JSON string, newlines escaped once.
const SKILL_LIST_ANCHOR: &[u8] = b"\\n### Available skills\\n";
/// What ends that list, besides an unescaped quote.
const SKILL_LIST_ENDS: [&[u8]; 3] = [b"\\n</skills_instructions>", b"\\n### ", b"\\n## "];

/// The skills a Codex session lists as available, from [`SKILL_LIST_ANCHOR`] to the first end, become [`SKILL_LIST`].
fn skill_list(rest: &[u8]) -> Option<(usize, Vec<u8>)> {
    let start = SKILL_LIST_ANCHOR.len();
    let mut at = start;
    while at < rest.len() {
        if rest[at] == b'"'
            || SKILL_LIST_ENDS
                .iter()
                .any(|end| rest[at..].starts_with(end))
        {
            return (at > start)
                .then(|| {
                    let mut new = SKILL_LIST_ANCHOR.to_vec();
                    new.extend_from_slice(SKILL_LIST);
                    (at, new)
                })
                .filter(|(_, new)| rest[..at] != new[..]);
        }
        at += if rest[at] == b'\\' { 2 } else { 1 };
    }
    None
}

static MOUNT_PATH: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?-u)\A(/mnt/[a-z]/Users/)(\[redacted-user\]|[^/"\s\\\[][^/"\s\\]*)((?:/iCloudDrive/[^/"\s\\]+/\.agents/skills/)(?:\[redacted-skill\]|[A-Za-z0-9_][A-Za-z0-9._-]*))?"#,
    )
    .expect("the mount path pattern compiles")
});

static SKILL_PATH: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?-u)\A(/\.t3/agents/(?:\.agents_skills|\.codex/skills)/)(\[redacted-skill\]|[A-Za-z0-9_][A-Za-z0-9._-]*)"#,
    )
    .expect("the skill path pattern compiles")
});

/// The Windows account in `/mnt/<drive>/Users/<account>` becomes [`USER`], and the skill folder name under the
/// account's `iCloudDrive/<machine>/.agents/skills/` becomes [`SKILL`].
fn mount_path(rest: &[u8]) -> Option<(usize, Vec<u8>)> {
    let found = MOUNT_PATH.captures(rest)?;
    let whole = found.get(0)?;
    let mut new = found.get(1)?.as_bytes().to_vec();
    new.extend_from_slice(USER);
    if let Some(tail) = found.get(3) {
        let tail = tail.as_bytes();
        let cut = tail.len() - tail.iter().rev().position(|byte| *byte == b'/')?;
        new.extend_from_slice(&tail[..cut]);
        new.extend_from_slice(SKILL);
    }
    Some((whole.end(), new))
}

/// A skill folder name under the service user's skill directories becomes [`SKILL`].
fn skill_path(rest: &[u8]) -> Option<(usize, Vec<u8>)> {
    let found = SKILL_PATH.captures(rest)?;
    let mut new = found.get(1)?.as_bytes().to_vec();
    new.extend_from_slice(SKILL);
    Some((found.get(0)?.end(), new))
}

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

/// A private value in any form [`written_forms`] matches, and what replaces it.
#[derive(Clone, Debug)]
struct Literal {
    forms: Regex,
    replacement: Vec<u8>,
}

/// A pattern for `value` as written plainly or in a form a program or a person may give it: ASCII letters in either
/// case, and each other byte optionally after any number of backslashes, as in a regular expression or a string
/// escaped one or more times, or percent-encoded, as in a URL.
fn written_forms(value: &[u8]) -> String {
    let forms: String = value
        .iter()
        .map(|byte| {
            if byte.is_ascii_alphanumeric() {
                char::from(*byte).to_string()
            } else {
                format!(r"\\*(?:\x{byte:02x}|%{byte:02x})")
            }
        })
        .collect();
    format!("(?i-u){forms}")
}

fn compiled(pattern: &str) -> Regex {
    Regex::new(pattern).expect("a written-forms pattern is built from escaped bytes")
}

impl Literal {
    fn new(value: &[u8], replacement: &[u8]) -> Self {
        Self {
            forms: compiled(&written_forms(value)),
            replacement: replacement.to_vec(),
        }
    }

    fn rewrite(&self, data: &[u8]) -> Option<Vec<u8>> {
        let mut out: Option<Vec<u8>> = None;
        let mut copied = 0;
        for found in self.forms.find_iter(data) {
            let out = out.get_or_insert_with(|| Vec::with_capacity(data.len()));
            out.extend_from_slice(&data[copied..found.start()]);
            out.extend_from_slice(&self.replacement);
            copied = found.end();
        }
        let mut out = out?;
        out.extend_from_slice(&data[copied..]);
        Some(out)
    }
}

/// The rules [`redact`] applies.
#[derive(Clone, Debug, Default)]
pub struct Rules {
    literals: Vec<Literal>,
    hosts: Vec<Literal>,
    /// Every literal and host pattern in one, so bytes with none of them are read once.
    any_value: Option<Regex>,
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
        let mut patterns = Vec::new();
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
                patterns.push(written_forms(value.as_bytes()));
                rules
                    .literals
                    .push(Literal::new(value.as_bytes(), placeholder));
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
                patterns.push(written_forms(name.as_bytes()));
                rules
                    .hosts
                    .push(Literal::new(name.as_bytes(), stand_in.as_bytes()));
            }
        }
        let written: Vec<&[u8]> = [EMAIL, ORGANIZATION, ACCOUNT, PLAN]
            .into_iter()
            .chain(rules.hosts.iter().map(|host| host.replacement.as_slice()))
            .collect();
        for value in rules.literals.iter().chain(&rules.hosts) {
            if written.iter().any(|text| value.forms.is_match(text)) {
                return Err(fail(
                    "a value occurs inside a placeholder or stand-in, so redaction would not settle",
                ));
            }
        }
        if !patterns.is_empty() {
            let any: Vec<String> = patterns.iter().map(|one| format!("(?:{one})")).collect();
            rules.any_value = Some(compiled(&any.join("|")));
        }
        Ok(rules)
    }

    /// The values file these rules read, or `None` when only the field rules apply.
    pub fn values_file(&self) -> Option<&Path> {
        self.values.as_deref()
    }

    /// The stand-ins that replace machine names. They are written into redacted files, so they are not private.
    pub fn stand_ins(&self) -> Vec<&[u8]> {
        self.hosts
            .iter()
            .map(|host| host.replacement.as_slice())
            .collect()
    }

    fn apply(&self, data: &[u8]) -> Option<Vec<u8>> {
        let mut current = None;
        let values = self
            .any_value
            .as_ref()
            .is_some_and(|any| any.is_match(data));
        if values {
            for literal in &self.literals {
                chain(&mut current, data, |bytes| literal.rewrite(bytes));
            }
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
        chain(&mut current, data, |bytes| {
            rewrite(bytes, INSTRUCTIONS_ANCHOR, instruction_block)
        });
        chain(&mut current, data, |bytes| {
            rewrite(bytes, SKILL_LIST_ANCHOR, skill_list)
        });
        chain(&mut current, data, |bytes| {
            rewrite(bytes, b"/mnt/", mount_path)
        });
        for anchor in [
            &b"/.t3/agents/.agents_skills/"[..],
            b"/.t3/agents/.codex/skills/",
        ] {
            chain(&mut current, data, |bytes| {
                rewrite(bytes, anchor, skill_path)
            });
        }
        if values {
            for host in &self.hosts {
                chain(&mut current, data, |bytes| host.rewrite(bytes));
            }
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

/// What a rule can have replaced where a placeholder now stands.
#[derive(Clone, Copy)]
enum Gap {
    /// One token of at most `longest` bytes, with no whitespace, quote or backslash, that `fits` accepts as the
    /// earlier bytes from `from` to `to`.
    Token {
        longest: usize,
        fits: fn(before: &[u8], from: usize, to: usize) -> bool,
    },
    /// The personal instructions, from their anchor to where [`instruction_block`] ends them.
    Instructions,
    /// The installed skills after their heading, to where [`skill_list`] ends them.
    SkillList,
    /// A machine name, as long as its stand-in.
    Host,
}

fn token_byte(byte: u8) -> bool {
    !(byte.is_ascii_whitespace() || matches!(byte, 0x0b | b'"' | b'\\'))
}

/// A byte of an id or a machine name.
fn id_byte(byte: &u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-')
}

fn email_value(before: &[u8], from: usize, to: usize) -> bool {
    let gap = &before[from..to];
    let Some(at) = gap.iter().position(|byte| *byte == b'@') else {
        return false;
    };
    let (local, domain) = (&gap[..at], &gap[at + 1..]);
    (1..=128).contains(&local.len())
        && local
            .iter()
            .all(|byte| id_byte(byte) || matches!(byte, b'%' | b'+'))
        && (1..=254).contains(&domain.len())
        && domain
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-'))
        && domain.last().is_some_and(u8::is_ascii_alphanumeric)
}

fn organization_value(before: &[u8], from: usize, to: usize) -> bool {
    to - from >= SHORTEST_LITERAL
        && before[from..to]
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || *byte == b'-')
}

fn account_value(before: &[u8], from: usize, to: usize) -> bool {
    before[from..to].iter().all(id_byte)
}

fn plan_value(before: &[u8], from: usize, to: usize) -> bool {
    to - from >= 2
        && before[from..to]
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

static MOUNT_USERS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?-u)/mnt/[a-z]/Users/\z").expect("the mount prefix pattern compiles")
});

/// The account in a mount path, all of it, as [`MOUNT_PATH`] takes it.
fn user_value(before: &[u8], from: usize, to: usize) -> bool {
    MOUNT_USERS.is_match(&before[..from])
        && before[from] != b'['
        && !before[from..to].contains(&b'/')
        && before
            .get(to)
            .is_none_or(|byte| matches!(byte, b'/' | b'"' | b'\\') || !token_byte(*byte))
}

/// A skill folder name in one of the skill directories, all of it, as [`MOUNT_PATH`] and [`SKILL_PATH`] take it.
fn skill_value(before: &[u8], from: usize, to: usize) -> bool {
    ["/.agents/skills/", "/.agents_skills/", "/.codex/skills/"]
        .iter()
        .any(|directory| before[..from].ends_with(directory.as_bytes()))
        && (before[from].is_ascii_alphanumeric() || before[from] == b'_')
        && before[from..to].iter().all(id_byte)
        && !before.get(to).is_some_and(id_byte)
}

/// Each placeholder and what it can stand for. The longest values are the field rules' own bounds: an address of
/// 128, 1 and 254 bytes, a Codex account id of 256 and a plan of 20; a path segment is at most 255.
const GAPS: [(&[u8], Gap); 8] = [
    (
        EMAIL,
        Gap::Token {
            longest: 383,
            fits: email_value,
        },
    ),
    (
        ORGANIZATION,
        Gap::Token {
            longest: 64,
            fits: organization_value,
        },
    ),
    (
        ACCOUNT,
        Gap::Token {
            longest: 256,
            fits: account_value,
        },
    ),
    (
        PLAN,
        Gap::Token {
            longest: 20,
            fits: plan_value,
        },
    ),
    (
        USER,
        Gap::Token {
            longest: 255,
            fits: user_value,
        },
    ),
    (
        SKILL,
        Gap::Token {
            longest: 255,
            fits: skill_value,
        },
    ),
    (INSTRUCTIONS, Gap::Instructions),
    (SKILL_LIST, Gap::SkillList),
];

/// Where each placeholder or stand-in stands in `after`, the longest at each place.
fn marks(after: &[u8], stand_ins: &[&[u8]]) -> Vec<(usize, usize, Gap)> {
    let mut found = Vec::new();
    let mut at = 0;
    while at < after.len() {
        let longest = GAPS
            .iter()
            .copied()
            .chain(stand_ins.iter().map(|text| (*text, Gap::Host)))
            .filter(|(text, _)| after[at..].starts_with(text))
            .max_by_key(|(text, _)| text.len());
        match longest {
            Some((text, gap)) => {
                found.push((at, at + text.len(), gap));
                at += text.len();
            }
            None => at += 1,
        }
    }
    found
}

/// Where a value that `placeholder` replaced can end in `before`, when it starts at `from`. The placeholder itself,
/// already in the earlier bytes, is one of them.
fn gap_ends(before: &[u8], from: usize, placeholder: &[u8], gap: Gap, after: &[u8]) -> Vec<usize> {
    let mut ends = Vec::new();
    if before[from..].starts_with(placeholder) {
        ends.push(from + placeholder.len());
    }
    match gap {
        Gap::Token { longest, fits } => {
            let mut to = from;
            while to < before.len() && to - from < longest && token_byte(before[to]) {
                to += 1;
                if fits(before, from, to) && before[from..to] != *placeholder {
                    ends.push(to);
                }
            }
        }
        Gap::Instructions => {
            if before[from..].starts_with(INSTRUCTIONS_ANCHOR)
                && let Some((length, _)) = instruction_block(&before[from..])
            {
                ends.push(from + length);
            }
        }
        Gap::SkillList => {
            let heading = from.checked_sub(SKILL_LIST_ANCHOR.len());
            if let Some(heading) = heading
                && before[heading..].starts_with(SKILL_LIST_ANCHOR)
                && let Some((length, _)) = skill_list(&before[heading..])
                && heading + length > from
            {
                ends.push(heading + length);
            }
        }
        Gap::Host => {
            let to = from + placeholder.len();
            if to <= before.len() {
                let name = &before[from..to];
                if name != placeholder && name.iter().all(id_byte) && find(after, name, 0).is_none()
                {
                    ends.push(to);
                }
            }
        }
    }
    ends
}

/// The most places the literal piece after one placeholder may align at. An honest redaction aligns at one; the bound
/// keeps a crafted record from making the alignment's memory grow with the square of its length.
const WIDEST: usize = 4096;

/// A placeholder or stand-in and how many values it replaced.
pub type Replaced = (Vec<u8>, usize);

/// Whether `text` is one of the placeholders the rules write, rather than a stand-in.
pub fn is_placeholder(text: &[u8]) -> bool {
    GAPS.iter().any(|(placeholder, _)| *placeholder == text)
}

/// How many values each placeholder and stand-in replaced, when `after` is `before` with values replaced and nothing
/// else. Split at every placeholder and at each of `stand_ins`, `after` must give literal pieces that occur in
/// `before` in the same order from its start to its end, and each gap between them must be the same placeholder or
/// what [`GAPS`] lets its rule replace. A stand-in replaces one value as long as itself, which no longer occurs in
/// `after`. Messages never quote the bytes.
pub fn replaced_values(before: &[u8], after: &[u8], stand_ins: &[&[u8]]) -> Result<Vec<Replaced>> {
    for stand_in in stand_ins {
        if stand_in.len() < SHORTEST_LITERAL
            || !stand_in.iter().all(id_byte)
            || GAPS.iter().any(|(text, _)| text == stand_in)
        {
            return Err(fail(format!(
                "{} cannot be the stand-in of a machine name",
                pytext::repr_str(&String::from_utf8_lossy(stand_in))
            )));
        }
    }
    let marks = marks(after, stand_ins);
    let piece = |k: usize| {
        let start = if k == 0 { 0 } else { marks[k - 1].1 };
        let end = marks.get(k).map_or(after.len(), |mark| mark.0);
        &after[start..end]
    };
    let parted = |at: usize| {
        fail(format!(
            "the redacted bytes are not the earlier bytes with values replaced by placeholders; they part at byte \
             {at} of the redacted bytes"
        ))
    };
    if !before.starts_with(piece(0)) || (marks.is_empty() && before.len() != after.len()) {
        return Err(parted(0));
    }
    let mut layers = vec![std::collections::BTreeMap::from([(piece(0).len(), 0)])];
    for (k, &(start, end, gap)) in marks.iter().enumerate() {
        let next = piece(k + 1);
        let last = k + 1 == marks.len();
        let mut reached = std::collections::BTreeMap::new();
        for &from in layers[k].keys() {
            for to in gap_ends(before, from, &after[start..end], gap, after) {
                let fits = if last {
                    before[to..] == *next
                } else {
                    before[to..].starts_with(next)
                };
                if fits {
                    reached.entry(to + next.len()).or_insert(from);
                }
            }
        }
        if reached.is_empty() {
            return Err(parted(start));
        }
        if reached.len() > WIDEST {
            return Err(fail(format!(
                "the redacted bytes align with the earlier bytes in too many ways to re-check, from byte {start} of \
                 the redacted bytes"
            )));
        }
        layers.push(reached);
    }
    let mut counts: Vec<Replaced> = Vec::new();
    let mut names: Vec<(&[u8], &[u8])> = Vec::new();
    let mut at = before.len();
    for k in (1..layers.len()).rev() {
        let from = layers[k][&at];
        let (start, end, gap) = marks[k - 1];
        let placeholder = &after[start..end];
        let value = &before[from..at - piece(k).len()];
        at = from;
        if value == placeholder {
            continue;
        }
        if matches!(gap, Gap::Host) {
            match names.iter().find(|(known, _)| *known == placeholder) {
                Some((_, name)) if *name != value => {
                    return Err(fail(format!(
                        "the stand-in {} replaced two different values",
                        pytext::repr_str(&String::from_utf8_lossy(placeholder))
                    )));
                }
                Some(_) => {}
                None => names.push((placeholder, value)),
            }
        }
        match counts.iter_mut().find(|(known, _)| known == placeholder) {
            Some((_, count)) => *count += 1,
            None => counts.push((placeholder.to_vec(), 1)),
        }
    }
    counts.sort();
    Ok(counts)
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

/// The top level of the repository the current directory is in.
pub fn top_level() -> Result<PathBuf> {
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
/// and the findings behind them, then one line for each staged [`RECORD`] entry the rules do not confirm.
#[derive(Clone, Debug, Default)]
pub struct Checked {
    pub lines: Vec<String>,
    pub findings: Vec<Finding>,
    pub entries: Vec<String>,
}

/// One entry of a [`RECORD`]: its `path`, `before.sha256`, `before.blob` and `after.sha256`, and the entry without its
/// `history` pin, in canonical form.
struct Recorded {
    path: String,
    before: String,
    blob: String,
    after: String,
    fixed: String,
}

/// The entries of a [`RECORD`], or `None` for another document.
fn record_entries(text: &[u8]) -> Option<Vec<Recorded>> {
    let document = pyjson::loads(std::str::from_utf8(text).ok()?, Duplicates::Refuse).ok()?;
    let field = |value: &Value, names: &[&str]| -> Option<String> {
        let mut value = value;
        for name in names {
            value = value.as_object()?.get(name)?;
        }
        value.as_str().map(str::to_string)
    };
    document
        .as_object()?
        .get("entries")?
        .as_array()?
        .iter()
        .map(|entry| {
            let fixed: pyjson::Object = entry
                .as_object()?
                .iter()
                .filter(|(key, _)| key.as_str() != "history")
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect();
            Some(Recorded {
                path: field(entry, &["path"])?,
                before: field(entry, &["before", "sha256"])?,
                blob: field(entry, &["before", "blob"])?,
                after: field(entry, &["after", "sha256"])?,
                fixed: pyjson::dumps(
                    &Value::Object(fixed),
                    &pyjson::Dumps {
                        sort_keys: true,
                        ..pyjson::Dumps::default()
                    },
                ),
            })
        })
        .collect()
}

/// A line for each file whose entries at `HEAD` the staged `record` does not keep as they were, in order and first,
/// apart from `history` pins and from removing a file's last entries; then for each entry it adds whose earlier blob
/// is not the file at `HEAD` with the recorded hash, or whose staged file is not what `rules` make of that blob, byte
/// for byte. An entry at `HEAD` decides which hash a batch's fingerprint takes, so only its `history` may change, and
/// removing an earlier entry while a later one stays would move the file's earliest hash.
fn unconfirmed(top: &Path, record: &[u8], rules: &Rules) -> Vec<String> {
    let Some(staged) = record_entries(record) else {
        return vec![format!(
            "cannot confirm: {RECORD} is not a redacted evidence record"
        )];
    };
    let committed = git(Some(top), &["cat-file", "blob", &format!("HEAD:{RECORD}")])
        .ok()
        .and_then(|text| record_entries(&text))
        .unwrap_or_default();
    let mut lines = Vec::new();
    let mut paths: Vec<&str> = committed.iter().map(|entry| entry.path.as_str()).collect();
    paths.sort_unstable();
    paths.dedup();
    for path in paths {
        let head: Vec<&Recorded> = committed
            .iter()
            .filter(|entry| entry.path == path)
            .collect();
        let now: Vec<&Recorded> = staged.iter().filter(|entry| entry.path == path).collect();
        let kept: Vec<&&Recorded> = head
            .iter()
            .filter(|old| now.iter().any(|new| new.after == old.after))
            .collect();
        let in_place = kept
            .iter()
            .zip(&head)
            .chain(kept.iter().zip(&now))
            .all(|(old, other)| old.after == other.after);
        if !in_place {
            lines.push(format!(
                "an entry already at HEAD was moved, or removed while a later entry of its file stays: {path} \
                 ({RECORD})"
            ));
        }
    }
    for entry in &staged {
        let Recorded {
            path,
            before,
            blob,
            after,
            fixed,
        } = entry;
        if let Some(old) = committed
            .iter()
            .find(|old| old.path == *path && old.after == *after)
        {
            if old.fixed != *fixed {
                lines.push(format!(
                    "an entry already at HEAD changed in more than its history pin: {path} ({RECORD})"
                ));
            }
            continue;
        }
        if rules.values_file().is_none() {
            lines.push(format!(
                "cannot confirm without the values file: {path} ({RECORD})"
            ));
            continue;
        }
        let committed = git(
            Some(top),
            &["rev-parse", "--verify", "--quiet", &format!("HEAD:{path}")],
        )
        .ok()
        .filter(|id| String::from_utf8_lossy(id).trim() == blob);
        let earlier = committed
            .and_then(|_| git(Some(top), &["cat-file", "blob", blob]).ok())
            .filter(|earlier| crate::sha256::hex_digest(earlier) == *before);
        let Some(earlier) = earlier else {
            lines.push(format!(
                "the entry's earlier bytes are not the file at HEAD: {path} ({RECORD})"
            ));
            continue;
        };
        let now = git(Some(top), &["cat-file", "blob", &format!(":{path}")]);
        let confirmed = match now {
            Ok(now) => {
                let redacted = redact(&earlier, rules);
                !redacted.findings.iter().any(Finding::unredactable)
                    && redacted.data.as_deref() == Some(now.as_slice())
                    && crate::sha256::hex_digest(&now) == *after
            }
            _ => false,
        };
        if !confirmed {
            lines.push(format!(
                "not what the rules make of its earlier bytes: {path} ({RECORD})"
            ));
        }
    }
    lines
}

/// `redaction check`: what redaction would change or cannot rewrite in `sources`, and with `staged`, each entry the
/// staged [`RECORD`] adds that the rules do not confirm.
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
    let mut record = None;
    if sources.staged {
        let top = top_level()?;
        for (path, blob) in staged(&top)? {
            scan(&path, &blob);
            if path == RECORD {
                record = Some((top.clone(), blob));
            }
        }
    }
    if let Some((top, blob)) = record {
        checked.entries = unconfirmed(&top, &blob, rules);
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
