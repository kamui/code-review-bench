//! The runner's own checks: each comparison and loader rule fails a deliberately wrong program or corpus, and the
//! corpora keep the properties no single case shows.

use crate::compare::compare_case;
use crate::corpus::{self, Corpus, field, has, list, string};
use crate::execute::{self, Failure, Scratch};
use crate::normalize::{Kind, Observed};
use crate::{b64, history, json};
use bench_tools::pyjson::{Object, Value};
use bench_tools::sha256::hex_digest;
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

pub type Check = fn(&Scratch) -> Result<(), Failure>;

type Files = BTreeMap<Vec<u8>, Vec<u8>>;

macro_rules! ensure {
    ($condition:expr, $($message:tt)+) => {
        if !$condition {
            return Err(Failure::Error(format!($($message)+)));
        }
    };
}

pub fn checks() -> Vec<(&'static str, Check)> {
    vec![
        ("wrong-outputs", wrong_outputs),
        ("comparison", comparison),
        ("removed-case", removed_case),
        ("changed-digest", changed_digest),
        ("open-defects", open_defects),
        ("published-decisions", published_decisions),
        ("active-history", active_history),
        ("decision-families", decision_families),
        ("shallow-history", shallow_history),
        ("prefix", prefix),
        ("gh-upstream", gh_upstream),
        ("upstream-offline", upstream_offline),
        ("storage-offline", storage_offline),
        ("derive-packet-pins", derive_packet_pins),
        ("v1-mapping-fallback", v1_mapping_fallback),
        ("duplicate-keys", duplicate_keys),
        ("archives", archives),
        ("archive-names", archive_names),
        ("storage-references", storage_references),
        ("special-files", special_files),
        ("steps", steps),
        ("file-size-limit", file_size_limit),
        ("fail-write", fail_write),
        ("step-baseline-defect", step_baseline_defect),
        ("gh-release", gh_release),
        ("structured-grading", structured_grading),
        ("structured-labels", structured_labels),
        ("structured-collision", structured_collision),
        ("structured-sites", structured_sites),
        ("structured-pins", structured_pins),
        ("structured-keys", structured_keys),
        ("structured-ordinals", structured_ordinals),
        ("structured-declarations", structured_declarations),
        ("hand-stated-outcomes", hand_stated_outcomes),
        ("fixture-git", fixture_git),
    ]
}

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn contracts() -> PathBuf {
    repo().join("tools/behavior_contracts")
}

fn s(text: &str) -> Value {
    json::text(text)
}

fn n(number: i64) -> Value {
    Value::Int(number.to_string())
}

fn enc(data: &[u8]) -> Value {
    s(&b64::encode(data))
}

fn obj(pairs: Vec<(&str, Value)>) -> Value {
    Value::Object(
        pairs
            .into_iter()
            .map(|(key, value)| (key.to_string(), value))
            .collect(),
    )
}

fn literal(text: &str) -> Result<Value, Failure> {
    Ok(json::value(text.as_bytes())?)
}

fn file(path: &[u8], data: &[u8]) -> Value {
    obj(vec![
        ("kind", s("file")),
        ("path_b64", enc(path)),
        ("bytes_b64", enc(data)),
    ])
}

/// `{**base, key: value, ...}`.
fn with(base: &Value, pairs: Vec<(&str, Value)>) -> Value {
    let mut value = base.clone();
    for (key, item) in pairs {
        set(&mut value, key, item);
    }
    value
}

fn set(value: &mut Value, key: &str, item: Value) {
    if let Value::Object(object) = value {
        object.insert(key.to_string(), item);
    }
}

fn without(value: &Value, key: &str) -> Value {
    match value {
        Value::Object(object) => Value::Object(
            object
                .iter()
                .filter(|(name, _)| name.as_str() != key)
                .map(|(name, item)| (name.clone(), item.clone()))
                .collect(),
        ),
        other => other.clone(),
    }
}

fn put(path: &Path, data: impl AsRef<[u8]>) -> Result<(), Failure> {
    Ok(std::fs::write(path, data).map_err(|error| format!("{}: {error}", path.display()))?)
}

fn get(path: &Path) -> Result<Vec<u8>, Failure> {
    Ok(std::fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?)
}

fn read_json(path: &Path) -> Result<Value, Failure> {
    Ok(json::value(&get(path)?).map_err(|error| format!("{}: {error}", path.display()))?)
}

fn write_json(path: &Path, value: &Value) -> Result<(), Failure> {
    put(path, json::dumps(value))
}

/// A directory removed with everything in it when the check ends.
struct Temp(PathBuf);

impl Temp {
    fn new() -> Result<Self, Failure> {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "behavior-contract-check-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).map_err(|error| format!("{}: {error}", path.display()))?;
        Ok(Self(path))
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn writer(args: &[&str]) -> Result<Vec<OsString>, Failure> {
    let mut line = vec![
        std::env::current_exe()
            .map_err(|error| format!("current executable: {error}"))?
            .into_os_string(),
        OsString::from("--contract-writer"),
    ];
    line.extend(args.iter().map(OsString::from));
    Ok(line)
}

/// Runs a case whose program is `writer` as the named synthetic writer.
fn observe(
    case: &Value,
    options: Value,
    scratch: &Scratch,
    name: &[&str],
) -> Result<Vec<Observed>, Failure> {
    let programs = [("writer", writer(name)?)];
    execute::execute(
        case,
        &Corpus {
            options,
            cases: Vec::new(),
        },
        scratch,
        &programs,
    )
}

fn single(
    case: &Value,
    options: Value,
    scratch: &Scratch,
    name: &[&str],
) -> Result<Observed, Failure> {
    let mut observed = observe(case, options, scratch, name)?;
    ensure!(
        observed.len() == 1,
        "{} results for one command",
        observed.len()
    );
    Ok(observed.remove(0))
}

fn no_options() -> Value {
    Value::Object(Object::default())
}

fn files(observed: &Observed) -> Files {
    observed
        .filesystem
        .iter()
        .filter_map(|entry| match &entry.kind {
            Kind::File(data) => Some((entry.path.clone(), data.clone())),
            _ => None,
        })
        .collect()
}

fn entries(observed: &Observed) -> BTreeMap<Vec<u8>, Value> {
    observed
        .filesystem
        .iter()
        .map(|entry| (entry.path.clone(), entry.to_value()))
        .collect()
}

fn named<'a>(found: &'a Files, path: &str) -> Result<&'a [u8], Failure> {
    Ok(found
        .get(path.as_bytes())
        .ok_or_else(|| format!("no file {path}"))?)
}

fn observed(exit: i64, stdout: &[u8], stderr: &[u8]) -> Observed {
    Observed {
        exit,
        stdout: stdout.to_vec(),
        stderr: stderr.to_vec(),
        filesystem: Vec::new(),
        inconsistencies: Vec::new(),
    }
}

fn bytes_stream(data: &[u8]) -> Value {
    obj(vec![("mode", s("bytes")), ("bytes_b64", enc(data))])
}

/// An expectation that pins every byte the observation holds, as a corpus derived from it would.
fn expectation(observed: &Observed, outcomes: &[Value]) -> Value {
    let mut value = obj(vec![
        ("exit", n(observed.exit)),
        ("stdout", bytes_stream(&observed.stdout)),
        ("stderr", bytes_stream(&observed.stderr)),
        (
            "filesystem",
            Value::Array(
                observed
                    .filesystem
                    .iter()
                    .map(|entry| entry.to_value())
                    .collect(),
            ),
        ),
    ]);
    if !outcomes.is_empty() {
        set(&mut value, "outcomes", Value::Array(outcomes.to_vec()));
    }
    value
}

fn compare(expected: Value, actual: &Observed) -> Result<Vec<String>, Failure> {
    Ok(compare_case(
        &obj(vec![("expected", expected)]),
        std::slice::from_ref(actual),
    )?)
}

/// Every way `right` differs from `left`, with `left`'s own inconsistencies, so two runs of the same writer show
/// no difference and a wrong one shows what it got wrong.
fn differences(
    left: &Observed,
    right: &Observed,
    outcomes: &[Value],
) -> Result<Vec<String>, Failure> {
    let mut found: Vec<String> = left
        .inconsistencies
        .iter()
        .map(|issue| format!("primary consistency: {issue}"))
        .collect();
    found.extend(compare(expectation(left, outcomes), right)?);
    Ok(found)
}

fn mentions(found: &[String], needle: &str) -> bool {
    found.iter().any(|item| item.contains(needle))
}

fn decision(case: &Value, reason: &str, supersedes: Option<Value>) -> Result<Value, Failure> {
    let mut payload = vec![
        ("reason", s(reason)),
        ("contract_sha256", s(&json::digest(case)?)),
    ];
    if let Some(digest) = supersedes {
        payload.push(("supersedes", digest));
    }
    let payload = obj(payload);
    let mut record = obj(vec![(
        "decision",
        s(&format!("decision-{}", json::digest(&payload)?)),
    )]);
    for (key, value) in payload.as_object().map(Object::iter).into_iter().flatten() {
        set(&mut record, key, value.clone());
    }
    Ok(record)
}

/// Writes `corpus.json` with the cases, each recorded in `d.json`, and returns its path.
fn save(directory: &Path, cases: &[Value], extra: Vec<(&str, Value)>) -> Result<PathBuf, Failure> {
    let mut records = Object::default();
    for case in cases {
        records.insert(
            string(case, "id")?.to_string(),
            decision(case, "test", None)?,
        );
    }
    write_json(
        &directory.join("d.json"),
        &obj(vec![("records", Value::Object(records))]),
    )?;
    let mut corpus = obj(vec![
        ("format", s("behavior-contract/1")),
        ("decisions", s("d.json")),
        ("cases", Value::Array(cases.to_vec())),
    ]);
    for (key, value) in extra {
        set(&mut corpus, key, value);
    }
    let path = directory.join("corpus.json");
    write_json(&path, &corpus)?;
    Ok(path)
}

fn refusal<T>(result: Result<T, String>, needles: &[&str], what: &str) -> Result<(), Failure> {
    match result {
        Ok(_) => Err(Failure::Error(format!(
            "{what} was accepted; expected a refusal naming {needles:?}"
        ))),
        Err(error) if needles.iter().any(|needle| error.contains(needle)) => Ok(()),
        Err(error) => Err(Failure::Error(format!(
            "{what} was refused with {error:?}, not {needles:?}"
        ))),
    }
}

fn refused(path: &Path, needles: &[&str], what: &str) -> Result<(), Failure> {
    refusal(corpus::load(path), needles, what)
}

/// Loads a saved corpus and the differences of its one case run with `programs`.
fn run_saved(
    path: &Path,
    scratch: &Scratch,
    programs: &[(&str, Vec<OsString>)],
) -> Result<Vec<String>, Failure> {
    let loaded = corpus::load(path)?;
    let [case] = loaded.cases.as_slice() else {
        return Err(Failure::Error(format!(
            "{} cases saved",
            loaded.cases.len()
        )));
    };
    let observed = execute::execute(case, &loaded, scratch, programs)?;
    Ok(compare_case(case, &observed)?)
}

fn wrong_outputs(scratch: &Scratch) -> Result<(), Failure> {
    let temp = Temp::new()?;
    let mut base = obj(vec![
        ("id", s("probe")),
        ("program", s("writer")),
        ("argv", Value::Array(Vec::new())),
        ("stdin_b64", enc(b"input\x00\xff")),
        ("initial", Value::Array(vec![file(b"hashed", b"a\n")])),
        ("derivation", s("Synthetic local negative demonstration")),
    ]);
    let actual = single(&base, no_options(), scratch, &["probe", "correct"])?;
    set(
        &mut base,
        "expected",
        obj(vec![
            ("exit", n(0)),
            (
                "stdout",
                literal(r#"{"mode": "json", "value": {"count": 1, "unknown": null}}"#)?,
            ),
            ("stderr", bytes_stream(b"")),
            (
                "filesystem",
                Value::Array(
                    actual
                        .filesystem
                        .iter()
                        .map(|entry| entry.to_value())
                        .collect(),
                ),
            ),
        ]),
    );
    let path = save(&temp.0, std::slice::from_ref(&base), Vec::new())?;
    for (variant, wanted) in [
        ("correct", None),
        ("count", Some("semantic")),
        ("hashed", Some("filesystem effects")),
        ("append", Some("filesystem effects")),
    ] {
        let found = run_saved(&path, scratch, &[("writer", writer(&["probe", variant])?)])?;
        match wanted {
            None => ensure!(found.is_empty(), "the correct probe differs: {found:?}"),
            Some(needle) => ensure!(
                mentions(&found, needle),
                "{variant}: no {needle} difference in {found:?}"
            ),
        }
    }
    let mut changed = base.clone();
    if let Value::Object(object) = &mut changed
        && let Some(expected) = object.get_mut("expected")
    {
        set(expected, "exit", n(9));
    }
    let corpus = obj(vec![
        ("format", s("behavior-contract/1")),
        ("decisions", s("d.json")),
        ("cases", Value::Array(vec![changed])),
    ]);
    write_json(&path, &corpus)?;
    refused(&path, &["explicit decision"], "a changed expectation")
}

fn comparison(_: &Scratch) -> Result<(), Failure> {
    let actual = observed(0, b"{\"snow\":\"\\u96ea\",\"missing\":null}\n", b"");
    let expected = literal(
        r#"{"exit": 0, "stdout": {"mode": "json", "value": {"missing": null, "snow": "雪"}},
            "stderr": {"mode": "bytes", "bytes_b64": ""}, "filesystem": []}"#,
    )?;
    let found = compare(expected.clone(), &actual)?;
    ensure!(found.is_empty(), "equal JSON differs: {found:?}");
    for value in [r#"{"snow": "雪"}"#, r#"{"snow": "雪", "missing": 0}"#] {
        let changed = with(
            &expected,
            vec![(
                "stdout",
                obj(vec![("mode", s("json")), ("value", literal(value)?)]),
            )],
        );
        ensure!(
            !compare(changed, &actual)?.is_empty(),
            "{value} matched other JSON"
        );
    }
    for raw in [
        &b"{\"missing\":null,\"missing\":null}"[..],
        b"{\"value\":NaN}",
    ] {
        let other = observed(0, raw, b"");
        ensure!(
            !compare(expected.clone(), &other)?.is_empty(),
            "{} compared as JSON",
            String::from_utf8_lossy(raw)
        );
    }
    let exact = with(&expected, vec![("stdout", bytes_stream(&actual.stdout))]);
    let stripped = observed(0, actual.stdout.trim_ascii_end(), b"");
    ensure!(
        !compare(exact, &stripped)?.is_empty(),
        "bytes mode ignored a trailing newline"
    );
    for raw in [&b"../outside"[..], b"/absolute", b"a/../b", b"a\0b"] {
        ensure!(
            corpus::relative(&enc(raw)).is_err(),
            "{} was accepted as a relative path",
            String::from_utf8_lossy(raw)
        );
    }
    Ok(())
}

fn removed_case(_: &Scratch) -> Result<(), Failure> {
    let temp = Temp::new()?;
    for entry in std::fs::read_dir(contracts()).map_err(|error| error.to_string())? {
        let path = entry.map_err(|error| error.to_string())?.path();
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        if name.starts_with("usage-decisions.") && name.ends_with(".json") {
            put(&temp.0.join(name), get(&path)?)?;
        }
    }
    let mut corpus = read_json(&contracts().join("usage.v1.json"))?;
    let cases: Vec<Value> = list(&corpus, "cases")?
        .iter()
        .filter(|case| string(case, "id").ok() != Some("unknown-is-not-zero"))
        .cloned()
        .collect();
    set(&mut corpus, "cases", Value::Array(cases));
    let path = temp.0.join("corpus.json");
    write_json(&path, &corpus)?;
    refused(
        &path,
        &["removal needs an explicit decision"],
        "a removed case",
    )
}

fn changed_digest(_: &Scratch) -> Result<(), Failure> {
    let temp = Temp::new()?;
    let previous = temp.0.join("previous.json");
    let rejected = literal(r#"{"id": "probe", "expected": {"exit": 2}}"#)?;
    write_json(
        &previous,
        &obj(vec![(
            "records",
            obj(vec![(
                "probe",
                decision(&rejected, "Reject unknown", None)?,
            )]),
        )]),
    )?;
    let case = literal(r#"{"id": "probe", "expected": {"exit": 0}}"#)?;
    let path = temp.0.join("corpus.json");
    write_json(
        &path,
        &obj(vec![
            ("format", s("behavior-contract/1")),
            ("decisions", s("decisions.json")),
            ("cases", Value::Array(vec![case.clone()])),
        ]),
    )?;
    let old = field(field(&read_json(&previous)?, "records")?, "probe")?.clone();
    let superseded = field(&old, "contract_sha256")?.clone();
    let changed = with(&old, vec![("contract_sha256", s(&json::digest(&case)?))]);
    let target = temp.0.join("decisions.json");
    let decisions = |record: Value| -> Result<Value, Failure> {
        Ok(obj(vec![
            (
                "previous",
                obj(vec![
                    ("path", s("previous.json")),
                    ("format", s("decisions/2")),
                    ("sha256", s(&hex_digest(&get(&previous)?))),
                ]),
            ),
            ("records", obj(vec![("probe", record)])),
        ]))
    };
    for (record, what) in [
        (changed, "an edited digest"),
        (
            decision(&case, "Reject unknown", None)?,
            "a record that supersedes nothing",
        ),
        (
            decision(&case, "Approve known zero", None)?,
            "a new reason that supersedes nothing",
        ),
        (
            decision(&case, "Reject unknown", Some(superseded.clone()))?,
            "a superseding record with the old reason",
        ),
    ] {
        write_json(&target, &decisions(record)?)?;
        refused(&path, &["explicit decision", "supersede"], what)?;
    }
    write_json(
        &target,
        &decisions(decision(&case, "Approve known zero", Some(superseded))?)?,
    )?;
    let loaded = corpus::load(&path)?;
    ensure!(
        loaded.cases.len() == 1 && loaded.cases[0].py_eq(&case),
        "the superseding decision did not load the case"
    );
    put(&previous, "{}")?;
    refused(
        &path,
        &["previous decisions changed"],
        "a changed predecessor",
    )
}

/// Every open defect in the register names exactly one usage case whose saved observation still fails it.
fn open_defects(_: &Scratch) -> Result<(), Failure> {
    let register = String::from_utf8(get(
        &repo().join("docs/python-migration/contracts/defects.md")
    )?)
    .map_err(|error| error.to_string())?;
    let usage = corpus::load(&contracts().join("usage.v1.json"))?;
    let resolution = |case: &Value| -> String {
        field(case, "baseline_defect")
            .and_then(|defect| string(defect, "resolution"))
            .unwrap_or_default()
            .to_string()
    };
    let mut registered = std::collections::BTreeSet::new();
    for section in register.split("\n## ").skip(1) {
        if !section.contains("- Status: open") {
            continue;
        }
        let identifier = section.split(':').next().unwrap_or_default();
        let needle = format!("#{}-", identifier.to_lowercase());
        let matches: Vec<&Value> = usage
            .cases
            .iter()
            .filter(|case| resolution(case).contains(&needle))
            .collect();
        let [case] = matches.as_slice() else {
            return Err(Failure::Error(format!(
                "{identifier}: {} cases resolve it",
                matches.len()
            )));
        };
        let id = string(case, "id")?;
        registered.insert(id.to_string());
        ensure!(
            section.contains(&format!("`{id}`")),
            "{identifier}: the register does not name `{id}`"
        );
        let saved = field(field(case, "baseline_defect")?, "observed")?;
        let exit = field(saved, "exit")?
            .py_str()
            .parse::<i64>()
            .map_err(|error| error.to_string())?;
        let stdout = json::dumps(field(field(saved, "stdout")?, "value")?);
        let expected = with(
            field(case, "expected")?,
            vec![("filesystem", Value::Array(Vec::new()))],
        );
        ensure!(
            !compare(expected, &observed(exit, stdout.as_bytes(), b""))?.is_empty(),
            "{identifier}: the saved defect meets the expectation"
        );
    }
    let defects: std::collections::BTreeSet<String> = usage
        .cases
        .iter()
        .filter(|case| has(case, "baseline_defect"))
        .filter_map(|case| string(case, "id").ok().map(str::to_string))
        .collect();
    ensure!(
        registered == defects,
        "open defects {registered:?}, baseline defects {defects:?}"
    );
    Ok(())
}

fn git(root: &Path, args: &[&str]) -> Result<(), Failure> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args([
            "-c",
            "user.name=kamui",
            "-c",
            "user.email=2255+kamui@users.noreply.github.com",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "core.hooksPath=/dev/null",
        ])
        .args(args)
        .output()
        .map_err(|error| format!("git: {error}"))?;
    ensure!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}

fn commit(root: &Path, message: &str) -> Result<(), Failure> {
    git(root, &["add", "."])?;
    git(root, &["commit", "-q", "-m", message])
}

fn probe_case() -> Value {
    obj(vec![("id", s("probe"))])
}

fn predecessor(path: &Path, format: &str) -> Result<Value, Failure> {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    Ok(obj(vec![
        ("path", s(name)),
        ("format", s(format)),
        ("sha256", s(&hex_digest(&get(path)?))),
    ]))
}

/// Publishes usage decision version `number`, superseding the one before, and points the usage corpus at it.
fn decision_version(directory: &Path, number: u32) -> Result<PathBuf, Failure> {
    let previous_path = directory.join(format!("usage-decisions.v{}.json", number - 1));
    let previous = read_json(&previous_path)?;
    let old = if number == 2 {
        previous
    } else {
        field(&previous, "records")?.clone()
    };
    let superseded = field(field(&old, "probe")?, "contract_sha256")?.clone();
    let format = if number == 2 {
        "decisions/1"
    } else {
        "decisions/2"
    };
    let value = obj(vec![
        ("previous", predecessor(&previous_path, format)?),
        (
            "records",
            obj(vec![(
                "probe",
                decision(
                    &probe_case(),
                    &format!("version {number}"),
                    Some(superseded),
                )?,
            )]),
        ),
    ]);
    let name = format!("usage-decisions.v{number}.json");
    let path = directory.join(&name);
    write_json(&path, &value)?;
    write_json(
        &directory.join("usage.v1.json"),
        &obj(vec![("decisions", s(&name))]),
    )?;
    Ok(path)
}

fn family_version(directory: &Path, family: &str, number: u32) -> Result<PathBuf, Failure> {
    let value = if number == 1 {
        obj(vec![(
            "records",
            obj(vec![("probe", decision(&probe_case(), "version 1", None)?)]),
        )])
    } else {
        let previous_path = directory.join(format!("{family}-decisions.v{}.json", number - 1));
        let old = field(field(&read_json(&previous_path)?, "records")?, "probe")?.clone();
        obj(vec![
            ("previous", predecessor(&previous_path, "decisions/2")?),
            (
                "records",
                obj(vec![(
                    "probe",
                    decision(
                        &probe_case(),
                        &format!("version {number}"),
                        Some(field(&old, "contract_sha256")?.clone()),
                    )?,
                )]),
            ),
        ])
    };
    let name = format!("{family}-decisions.v{number}.json");
    let path = directory.join(&name);
    write_json(&path, &value)?;
    write_json(
        &directory.join(format!("{family}.v1.json")),
        &obj(vec![("decisions", s(&name))]),
    )?;
    Ok(path)
}

/// A repository whose usage history holds a legacy first version and a second that supersedes it.
fn history_repo(root: &Path) -> Result<PathBuf, Failure> {
    git(root, &["init", "-q", "--initial-branch=main"])?;
    let directory = root.join("tools/behavior_contracts");
    std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    write_json(
        &directory.join("usage-decisions.v1.json"),
        &obj(vec![("probe", decision(&probe_case(), "legacy", None)?)]),
    )?;
    decision_version(&directory, 2)?;
    commit(root, "test(bench): adopt decision history")?;
    Ok(directory)
}

fn history_refused(root: &Path, needles: &[&str], what: &str) -> Result<(), Failure> {
    refusal(history::verify(root), needles, what)
}

fn published_decisions(_: &Scratch) -> Result<(), Failure> {
    let temp = Temp::new()?;
    let root = temp.0.as_path();
    let directory = history_repo(root)?;
    let v1 = directory.join("usage-decisions.v1.json");
    let v2 = directory.join("usage-decisions.v2.json");
    let published = get(&v2)?;
    history::verify(root)?;
    put(&v2, "same reason with recomputed digest and ID")?;
    history_refused(
        root,
        &["published decision changed"],
        "an edited published version",
    )?;
    commit(root, "test(bench): simulate an in-place decision edit")?;
    history_refused(root, &["published decision changed"], "a committed edit")?;
    put(&v2, &published)?;
    let v3 = decision_version(&directory, 3)?;
    let published_v3 = get(&v3)?;
    commit(root, "test(bench): retain history and add a decision")?;
    history::verify(root)?;
    put(&v3, "edited new case with no earlier record")?;
    history_refused(
        root,
        &["published decision changed"],
        "an edited latest version",
    )?;
    put(&v3, &published_v3)?;
    std::fs::remove_file(&v1).map_err(|error| error.to_string())?;
    history_refused(root, &["published decision changed"], "a removed version")
}

fn active_history(_: &Scratch) -> Result<(), Failure> {
    for scenario in ["rollback", "new-root", "skip-predecessor"] {
        let temp = Temp::new()?;
        let root = temp.0.as_path();
        let directory = history_repo(root)?;
        decision_version(&directory, 3)?;
        commit(root, "test(bench): publish another decision")?;
        history::verify(root)?;
        if scenario == "rollback" {
            write_json(
                &directory.join("usage.v1.json"),
                &obj(vec![("decisions", s("usage-decisions.v2.json"))]),
            )?;
        } else {
            let v4 = decision_version(&directory, 4)?;
            let value = read_json(&v4)?;
            let value = if scenario == "new-root" {
                without(&value, "previous")
            } else {
                let skipped = read_json(&directory.join("usage-decisions.v2.json"))?;
                with(
                    &value,
                    vec![("previous", field(&skipped, "previous")?.clone())],
                )
            };
            write_json(&v4, &value)?;
        }
        commit(root, "test(bench): simulate bypassing active history")?;
        history_refused(
            root,
            &["latest published", "immediate predecessor"],
            scenario,
        )?;
    }
    Ok(())
}

fn decision_families(_: &Scratch) -> Result<(), Failure> {
    {
        let temp = Temp::new()?;
        let root = temp.0.as_path();
        let directory = history_repo(root)?;
        let v1 = family_version(&directory, "roster", 1)?;
        commit(root, "test(bench): publish a second decision family")?;
        history::verify(root)?;
        let published = get(&v1)?;
        put(&v1, json::dumps(&literal(r#"{"records": {}}"#)?))?;
        history_refused(
            root,
            &["roster-decisions.v1.json: published decision changed"],
            "an edited second family",
        )?;
        put(&v1, &published)?;
        family_version(&directory, "roster", 2)?;
        commit(root, "test(bench): supersede the second family")?;
        history::verify(root)?;
        write_json(
            &directory.join("roster.v1.json"),
            &obj(vec![("decisions", s("roster-decisions.v1.json"))]),
        )?;
        history_refused(
            root,
            &["roster.v1.json: corpus must use the latest published"],
            "a stale corpus",
        )?;
        std::fs::remove_file(directory.join("roster.v1.json"))
            .map_err(|error| error.to_string())?;
        history_refused(
            root,
            &["roster: no corpus uses this decision history"],
            "an unused family",
        )?;
    }
    let temp = Temp::new()?;
    let root = temp.0.as_path();
    let directory = history_repo(root)?;
    let v1 = family_version(&directory, "roster", 1)?;
    let borrowed = read_json(&directory.join("usage-decisions.v2.json"))?;
    let value = with(
        &read_json(&v1)?,
        vec![("previous", field(&borrowed, "previous")?.clone())],
    );
    write_json(&v1, &value)?;
    commit(root, "test(bench): give a first version a predecessor")?;
    history_refused(
        root,
        &["a first decision version has no predecessor"],
        "a first version with a predecessor",
    )
}

fn shallow_history(_: &Scratch) -> Result<(), Failure> {
    let temp = Temp::new()?;
    let root = temp.0.join("full");
    std::fs::create_dir(&root).map_err(|error| error.to_string())?;
    let directory = history_repo(&root)?;
    decision_version(&directory, 3)?;
    commit(&root, "test(bench): publish another decision")?;
    history::verify(&root)?;
    let shallow = temp.0.join("shallow");
    let source = format!("file://{}", root.display());
    let target = shallow.to_string_lossy().into_owned();
    git(&temp.0, &["clone", "-q", "--depth=1", &source, &target])?;
    history_refused(&shallow, &["full git checkout"], "a shallow clone")
}

fn prefix(_: &Scratch) -> Result<(), Failure> {
    let expected = obj(vec![
        ("exit", n(2)),
        ("stdout", bytes_stream(b"")),
        (
            "stderr",
            obj(vec![
                ("mode", s("prefix")),
                ("bytes_b64", enc(b"tool: command failed: git\n")),
            ]),
        ),
        ("filesystem", Value::Array(Vec::new())),
    ]);
    let actual = observed(2, b"", b"tool: command failed: git\nfatal: any wording\n");
    let found = compare(expected.clone(), &actual)?;
    ensure!(found.is_empty(), "a matching prefix differs: {found:?}");
    ensure!(
        !compare(expected, &observed(2, b"", b"tool: command failed"))?.is_empty(),
        "a shorter stream matched the prefix"
    );
    let other = observed(2, b"", b"tool: command failed: git\nother\n");
    let found = differences(&actual, &other, &[])?;
    ensure!(
        mentions(&found, "stderr bytes"),
        "two runs compare by prefix: {found:?}"
    );
    Ok(())
}

fn gh_options() -> Value {
    obj(vec![("gh_stub_b64", enc(b"gh"))])
}

/// Each `CODE BASE64...` line a gh writer printed, decoded.
fn gh_lines(stdout: &[u8]) -> Result<Vec<Vec<Vec<u8>>>, Failure> {
    let text = String::from_utf8(stdout.to_vec()).map_err(|error| error.to_string())?;
    let mut lines = Vec::new();
    for line in text.lines() {
        let mut words = line.split(' ');
        let mut decoded = vec![words.next().unwrap_or_default().as_bytes().to_vec()];
        for word in words {
            decoded.push(b64::decode(word)?);
        }
        lines.push(decoded);
    }
    Ok(lines)
}

fn calls(found: &Files) -> Result<Vec<Value>, Failure> {
    let log = named(found, "gh/calls.jsonl")?;
    log.split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .map(|line| Ok(json::value(line)?))
        .collect()
}

fn gh_upstream(scratch: &Scratch) -> Result<(), Failure> {
    let responses = obj(vec![
        (
            "repos/o/r/pulls/1",
            literal(r#"{"pages": [{"number": 1}]}"#)?,
        ),
        (
            "repos/o/r/pulls/1/reviews",
            literal(r#"{"exit": 1, "stdout": "[[]]", "stderr": "HTTP 502\r\n"}"#)?,
        ),
        (
            "repos/o/r/issues/1/comments",
            obj(vec![("stdout_b64", enc(b"[[]]\xff"))]),
        ),
    ]);
    let case = obj(vec![
        ("id", s("probe")),
        ("program", s("writer")),
        ("argv", Value::Array(Vec::new())),
        ("stdin_b64", s("")),
        (
            "initial",
            Value::Array(vec![file(
                b"gh/upstream.json",
                json::dumps(&responses).as_bytes(),
            )]),
        ),
    ]);
    let actual = single(&case, gh_options(), scratch, &["gh-upstream"])?;
    let lines = gh_lines(&actual.stdout)?;
    let wanted: Vec<Vec<Vec<u8>>> = [
        (&b"0"[..], &b"[{\"number\": 1}]"[..], &b""[..]),
        (b"1", b"[[]]", b"HTTP 502\r\n"),
        (b"0", b"[[]]\xff", b""),
        (b"1", b"", b"gh: Not Found (HTTP 404)\n"),
    ]
    .iter()
    .map(|(code, out, err)| vec![code.to_vec(), out.to_vec(), err.to_vec()])
    .collect();
    ensure!(lines == wanted, "the stub answered {lines:?}");
    let recorded = calls(&files(&actual))?;
    let endpoints: Vec<String> = recorded
        .iter()
        .map(|call| {
            call.as_array()
                .and_then(|args| args.get(5))
                .map(Value::py_str)
                .unwrap_or_default()
        })
        .collect();
    let mut sorted = endpoints.clone();
    sorted.sort();
    ensure!(
        endpoints.len() == 4 && endpoints == sorted,
        "the stub recorded {endpoints:?}"
    );
    Ok(())
}

/// Counts the recorded gh calls of a corpus's expectations, refusing any call the stub would not answer and any
/// argument naming a URL.
fn offline_calls(name: &str, answered: &regex::Regex) -> Result<usize, Failure> {
    let loaded = corpus::load(&contracts().join(format!("{name}.v1.json")))?;
    ensure!(
        field(&loaded.options, "gh_stub_b64").is_ok_and(|stub| stub.py_eq(&enc(b"gh"))),
        "{name}: the corpus does not stub gh"
    );
    let mut count = 0;
    for case in &loaded.cases {
        let id = string(case, "id")?;
        for step in corpus::steps(case) {
            if !has(step, "argv") {
                continue;
            }
            ensure!(
                !list(step, "argv")?
                    .iter()
                    .any(|arg| arg.py_str().contains("://")),
                "{name}::{id}: an argument names a URL"
            );
            for entry in list(field(step, "expected")?, "filesystem")? {
                if b64::decode(string(entry, "path_b64")?)? != b"gh/calls.jsonl" {
                    continue;
                }
                let log = String::from_utf8(b64::decode(string(entry, "bytes_b64")?)?)
                    .map_err(|error| error.to_string())?;
                for line in log.lines() {
                    ensure!(
                        answered.is_match(line),
                        "{name}::{id}: the stub cannot answer {line}"
                    );
                    count += 1;
                }
            }
        }
    }
    Ok(count)
}

fn upstream_offline(_: &Scratch) -> Result<(), Failure> {
    let read = regex::Regex::new(
        r#"^\["api", "--method", "GET", "--paginate", "--slurp", "repos/example/(?:project|other)/(?:pulls/\d+(?:/comments|/reviews)?|issues/\d+/comments)"\]$"#,
    )
    .map_err(|error| error.to_string())?;
    let count = offline_calls("upstream", &read)?;
    ensure!(count > 50, "only {count} upstream calls are pinned");
    Ok(())
}

fn storage_offline(_: &Scratch) -> Result<(), Failure> {
    let answered = regex::Regex::new(concat!(
        r#"^(?:\["api", "repos/[^/"]+/[^/"]+/releases/tags/[^"/]+"\]"#,
        r#"|\["api", "--paginate", "--slurp", "repos/[^/"]+/[^/"]+/releases/\d+/assets\?per_page=100"\]"#,
        r#"|\["release", "upload", "[^"]+", "[^"]+", "--repo", "[^/"]+/[^/"]+"\]"#,
        r#"|\["api", "repos/[^/"]+/[^/"]+/releases/assets/\d+", "-H", "Accept: application/octet-stream"\])$"#,
    ))
    .map_err(|error| error.to_string())?;
    let mut count = 0;
    for name in [
        "evidence-store",
        "native-artifacts",
        "evidence-inventory",
        "import-benchmark",
    ] {
        count += offline_calls(name, &answered)?;
    }
    ensure!(count > 100, "only {count} storage calls are pinned");
    Ok(())
}

fn derive_packet_pins(_: &Scratch) -> Result<(), Failure> {
    let loaded = corpus::load(&contracts().join("derive-packet.v1.json"))?;
    let cases: BTreeMap<&str, &Value> = loaded
        .cases
        .iter()
        .filter_map(|case| string(case, "id").ok().map(|id| (id, case)))
        .collect();
    let targets = repo().join("bench/targets");
    let mut legacy = Vec::new();
    for entry in std::fs::read_dir(&targets).map_err(|error| error.to_string())? {
        let path = entry.map_err(|error| error.to_string())?.path();
        if path.join("packet.legacy.md").is_file() {
            legacy.push(path);
        }
    }
    legacy.sort();
    ensure!(!legacy.is_empty(), "no target has a legacy packet");
    let pinned: std::collections::BTreeSet<String> = cases
        .keys()
        .filter(|id| id.starts_with("derive-packet-target-"))
        .map(|id| id.to_string())
        .collect();
    let committed: std::collections::BTreeSet<String> = legacy
        .iter()
        .map(|path| {
            format!(
                "derive-packet-target-{}",
                path.file_name().unwrap_or_default().to_string_lossy()
            )
        })
        .collect();
    ensure!(
        pinned == committed,
        "pinned {pinned:?}, committed {committed:?}"
    );
    for path in &legacy {
        let id = format!(
            "derive-packet-target-{}",
            path.file_name().unwrap_or_default().to_string_lossy()
        );
        let case = cases[id.as_str()];
        let argv: Vec<String> = list(case, "argv")?.iter().map(Value::py_str).collect();
        ensure!(
            argv == ["packet.legacy.md", "--out", "packet.md"],
            "{id}: argv {argv:?}"
        );
        let mut written = BTreeMap::new();
        for entry in list(field(case, "expected")?, "filesystem")? {
            if string(entry, "kind")? == "file" {
                written.insert(
                    b64::decode(string(entry, "path_b64")?)?,
                    b64::decode(string(entry, "bytes_b64")?)?,
                );
            }
        }
        let committed = BTreeMap::from([
            (
                b"packet.legacy.md".to_vec(),
                get(&path.join("packet.legacy.md"))?,
            ),
            (b"packet.md".to_vec(), get(&path.join("packet.md"))?),
        ]);
        ensure!(
            written == committed,
            "{id}: the pinned packets differ from the committed ones"
        );
    }
    Ok(())
}

fn v1_mapping_fallback(_: &Scratch) -> Result<(), Failure> {
    let corpus = read_json(&contracts().join("grade-map.v1.json"))?;
    let case = list(&corpus, "cases")?
        .iter()
        .find(|case| string(case, "id").ok() == Some("grade-map-verdicts-v1"))
        .ok_or("no grade-map-verdicts-v1 case")?;
    let archive = repo().join(
        "bench/regrading/cohort-rebuild-2026-10-04-rebuilt-v4/2026-09-24-builtin-baseline/i-requests-6667/attempt-1/evidence.json",
    );
    let mut pins = BTreeMap::new();
    for entry in list(&read_json(&archive)?, "files")? {
        pins.insert(
            string(entry, "path")?.to_string(),
            string(entry, "sha256")?.to_string(),
        );
    }
    let mut initial = BTreeMap::new();
    for entry in list(case, "initial")? {
        if string(entry, "kind")? == "file" {
            let name = String::from_utf8(b64::decode(string(entry, "path_b64")?)?)
                .map_err(|error| error.to_string())?;
            initial.insert(name, b64::decode(string(entry, "bytes_b64")?)?);
        }
    }
    let key = json::value(initial.get("key.json").ok_or("no key.json")?)?;
    let key = with(
        &key,
        vec![(
            "input_fingerprint",
            s("5419f9781ce77f714c814f838c09ebc85766e972de37338df9c194c1272c5f88"),
        )],
    );
    let mut archived: BTreeMap<String, Vec<u8>> = initial
        .into_iter()
        .filter(|(name, _)| name.starts_with("work/"))
        .collect();
    archived.insert(
        "key.json".to_string(),
        (json::dumps_indented(&key) + "\n").into_bytes(),
    );
    ensure!(archived.len() == 22, "{} archived files", archived.len());
    for (name, data) in &archived {
        ensure!(
            pins.get(name).is_some_and(|pin| *pin == hex_digest(data)),
            "{name}: differs from the archived evidence"
        );
    }
    Ok(())
}

fn duplicate_keys(_: &Scratch) -> Result<(), Failure> {
    let temp = Temp::new()?;
    let path = temp.0.join("corpus.json");
    put(
        &path,
        r#"{"format":"behavior-contract/1","format":"behavior-contract/1"}"#,
    )?;
    refused(
        &path,
        &["format is written more than once"],
        "a repeated key",
    )
}

fn archive_case(level: &str, variant: &str) -> Result<Value, Failure> {
    let case = literal(
        r#"{"id": "probe", "program": "writer", "stdin_b64": "", "initial": [],
            "archives": [{"glob": "out/*.tar.gz", "format": "tar.gz"}], "manifests": ["out/manifest.json"]}"#,
    )?;
    Ok(with(
        &case,
        vec![("argv", Value::Array(vec![s(level), s(variant)]))],
    ))
}

fn archives(scratch: &Scratch) -> Result<(), Failure> {
    let modes = literal(r#"{"snapshot_modes": true}"#)?;
    let run = |level: &str, variant: &str| {
        single(
            &archive_case(level, variant)?,
            modes.clone(),
            scratch,
            &["archive"],
        )
    };
    let best = run("9", "same")?;
    let fastest = run("1", "same")?;
    ensure!(
        best.stdout == b"<archive 1>\n",
        "stdout {:?}",
        String::from_utf8_lossy(&best.stdout)
    );
    ensure!(
        best.inconsistencies.is_empty(),
        "inconsistent: {:?}",
        best.inconsistencies
    );
    let found = differences(&best, &fastest, &[])?;
    ensure!(found.is_empty(), "compression levels differ: {found:?}");
    let written = files(&best);
    let manifest = named(&written, "out/manifest.json")?;
    ensure!(
        String::from_utf8_lossy(manifest).contains(r#""bytes": "<archive size>""#),
        "the manifest keeps its archive size"
    );
    let archive = best
        .filesystem
        .iter()
        .find(|entry| matches!(entry.kind, Kind::Archive { .. }))
        .ok_or("no archive entry")?;
    ensure!(
        archive.path == b"out/<archive 1>.tar.gz",
        "archive at {:?}",
        String::from_utf8_lossy(&archive.path)
    );
    let Kind::Archive { members, .. } = &archive.kind else {
        return Err(Failure::Error("no archive members".to_string()));
    };
    let listed: Vec<(String, String)> = members
        .as_array()
        .unwrap_or_default()
        .iter()
        .map(|member| {
            (
                string(member, "name").unwrap_or_default().to_string(),
                field(member, "mode").map(Value::py_str).unwrap_or_default(),
            )
        })
        .collect();
    ensure!(
        listed == [("bench/a.txt".to_string(), 0o644.to_string())],
        "members {listed:?}"
    );
    for variant in ["byte", "mode"] {
        let found = differences(&best, &run("9", variant)?, &[])?;
        ensure!(
            mentions(&found, "filesystem effects"),
            "{variant}: {found:?}"
        );
    }
    let wrong = run("9", "manifest")?;
    ensure!(
        mentions(
            &wrong.inconsistencies,
            "differ from the archive written beside it"
        ),
        "a wrong manifest digest: {:?}",
        wrong.inconsistencies
    );
    let found = compare(expectation(&best, &[]), &wrong)?;
    ensure!(
        found.iter().any(|item| item.starts_with("consistency:")),
        "{found:?}"
    );
    Ok(())
}

fn archive_names(scratch: &Scratch) -> Result<(), Failure> {
    let case = literal(
        r#"{"id": "probe", "program": "writer", "argv": [], "stdin_b64": "", "initial": [],
            "archives": [{"glob": "out/names.tar.gz", "format": "tar.gz"}]}"#,
    )?;
    let actual = single(&case, no_options(), scratch, &["archive-names"])?;
    let Some(Kind::Archive { members, .. }) = actual
        .filesystem
        .iter()
        .find(|entry| entry.path == b"out/names.tar.gz")
        .map(|entry| &entry.kind)
    else {
        return Err(Failure::Error(
            "out/names.tar.gz was not read as an archive".to_string(),
        ));
    };
    let listed = members.as_array().unwrap_or_default();
    ensure!(listed.len() == 2, "{} members", listed.len());
    ensure!(
        string(&listed[0], "name")? == "plain.txt",
        "first member {}",
        listed[0].py_repr()
    );
    ensure!(
        !has(&listed[1], "name"),
        "a non-UTF-8 name is listed as text"
    );
    ensure!(
        b64::decode(string(&listed[1], "name_b64")?)? == b"review-\xff.md",
        "second member {}",
        listed[1].py_repr()
    );
    ensure!(
        json::digest(members)?.len() == 64,
        "the members have no digest"
    );
    Ok(())
}

fn storage_references(scratch: &Scratch) -> Result<(), Failure> {
    let case = literal(
        r#"{"id": "probe", "program": "writer", "stdin_b64": "", "initial": [],
            "archives": [{"glob": "out/native-scratch.v1.zip", "format": "zip"}],
            "references": [{"path": "out/native-artifact-storage.v1.json", "fields": ["archive"]}]}"#,
    )?;
    let run = |level: &str, variant: &str| {
        let case = with(
            &case,
            vec![("argv", Value::Array(vec![s(level), s(variant)]))],
        );
        single(&case, no_options(), scratch, &["native"])
    };
    let best = run("9", "same")?;
    ensure!(
        best.inconsistencies.is_empty(),
        "inconsistent: {:?}",
        best.inconsistencies
    );
    let found = differences(&best, &run("1", "same")?, &[])?;
    ensure!(found.is_empty(), "compression levels differ: {found:?}");
    for variant in ["label", "path"] {
        let wrong = run("9", variant)?;
        ensure!(
            mentions(
                &wrong.inconsistencies,
                "differs from the file written beside it"
            ),
            "{variant}: {:?}",
            wrong.inconsistencies
        );
        let found = differences(&best, &wrong, &[])?;
        ensure!(
            found.iter().any(|item| item.starts_with("consistency:")),
            "{variant}: {found:?}"
        );
    }
    let storage = obj(vec![("path", s("out/native-artifact-storage.v1.json"))]);
    let rule = |key: &str| {
        with(
            &case,
            vec![
                ("argv", Value::Array(vec![s("9"), s("same")])),
                (key, Value::Array(vec![storage.clone()])),
            ],
        )
    };
    let unfielded = single(&rule("references"), no_options(), scratch, &["native"])?;
    ensure!(
        mentions(&unfielded.inconsistencies, "unreadable storage reference"),
        "a references rule without fields: {:?}",
        unfielded.inconsistencies
    );
    ensure!(
        single(&rule("volatile"), no_options(), scratch, &["native"]).is_err(),
        "a volatile rule without pointers ran"
    );
    let bare = with(
        &without(&case, "initial"),
        vec![("argv", Value::Array(vec![s("9"), s("same")]))],
    );
    ensure!(
        single(&bare, no_options(), scratch, &["native"]).is_err(),
        "a case without initial ran"
    );
    Ok(())
}

fn special_files(scratch: &Scratch) -> Result<(), Failure> {
    let case = obj(vec![
        ("id", s("probe")),
        ("program", s("writer")),
        ("argv", Value::Array(Vec::new())),
        ("stdin_b64", s("")),
        (
            "initial",
            Value::Array(vec![obj(vec![
                ("kind", s("special")),
                ("path_b64", enc(b"pipe")),
                ("mode", n(0o640)),
            ])]),
        ),
    ]);
    std::thread::scope(|scope| {
        let (send, receive) = std::sync::mpsc::channel();
        scope.spawn(move || {
            let _ = send.send(single(&case, no_options(), scratch, &["noop"]));
        });
        let Ok(result) = receive.recv_timeout(Duration::from_secs(5)) else {
            eprintln!("self-check::special-files: the snapshot blocked on a FIFO");
            std::process::exit(1);
        };
        let actual = result?;
        let listed = Value::Array(
            actual
                .filesystem
                .iter()
                .map(|entry| entry.to_value())
                .collect(),
        );
        let wanted = Value::Array(vec![
            obj(vec![("path_b64", enc(b".home")), ("kind", s("directory"))]),
            obj(vec![
                ("path_b64", enc(b"pipe")),
                ("kind", s("special")),
                ("mode", n(0o640)),
            ]),
        ]);
        ensure!(listed.py_eq(&wanted), "snapshot {}", listed.py_repr());
        Ok(())
    })
}

fn step_case() -> Value {
    let mode =
        |path: &[u8], data: &[u8], bits: i64| with(&file(path, data), vec![("mode", n(bits))]);
    obj(vec![
        ("id", s("probe")),
        ("program", s("writer")),
        (
            "initial",
            Value::Array(vec![
                mode(b"kept", b"x", 0o600),
                obj(vec![("kind", s("directory")), ("path_b64", enc(b"gone"))]),
                obj(vec![
                    ("kind", s("symlink")),
                    ("path_b64", enc(b"link")),
                    ("target_b64", enc(b"kept")),
                ]),
            ]),
        ),
        (
            "steps",
            Value::Array(vec![
                obj(vec![
                    ("argv", Value::Array(vec![s("first")])),
                    ("stdin_b64", s("")),
                ]),
                obj(vec![(
                    "remove",
                    Value::Array(vec![enc(b"gone"), enc(b"first")]),
                )]),
                obj(vec![(
                    "write",
                    Value::Array(vec![mode(b"kept", b"y", 0o640)]),
                )]),
                obj(vec![
                    ("argv", Value::Array(vec![s("second")])),
                    ("stdin_b64", s("")),
                ]),
            ]),
        ),
    ])
}

fn steps(scratch: &Scratch) -> Result<(), Failure> {
    let case = step_case();
    let results = observe(
        &case,
        literal(r#"{"snapshot_modes": true}"#)?,
        scratch,
        &["list"],
    )?;
    let [first, second] = results.as_slice() else {
        return Err(Failure::Error(format!(
            "{} results for two commands",
            results.len()
        )));
    };
    let listed = named(&files(first), "first")?.to_vec();
    ensure!(
        listed == b".home gone kept link",
        "the first step saw {:?}",
        String::from_utf8_lossy(&listed)
    );
    let after = entries(second);
    let names: Vec<&[u8]> = after.keys().map(Vec::as_slice).collect();
    ensure!(
        names == [&b".home"[..], b"kept", b"link", b"second"],
        "the second step left {names:?}"
    );
    let wanted = [
        (
            &b"kept"[..],
            with(&file(b"kept", b"y"), vec![("mode", n(0o640))]),
        ),
        (
            b"second",
            with(
                &file(b"second", b".home kept link"),
                vec![("mode", n(0o644))],
            ),
        ),
        (
            b".home",
            obj(vec![
                ("path_b64", enc(b".home")),
                ("kind", s("directory")),
                ("mode", n(0o755)),
            ]),
        ),
        (
            b"link",
            obj(vec![
                ("path_b64", enc(b"link")),
                ("kind", s("symlink")),
                ("target_b64", enc(b"kept")),
            ]),
        ),
    ];
    for (name, entry) in wanted {
        let found = &after[name];
        let mut entry = entry;
        if let Some(digest) = found.as_object().and_then(|object| object.get("sha256")) {
            set(&mut entry, "sha256", digest.clone());
        }
        ensure!(
            found.py_eq(&entry),
            "{}: {}",
            String::from_utf8_lossy(name),
            found.py_repr()
        );
    }
    let mut first_only = case.clone();
    set(
        &mut first_only,
        "steps",
        Value::Array(list(&case, "steps")?[..1].to_vec()),
    );
    let plain = observe(&first_only, no_options(), scratch, &["list"])?;
    ensure!(
        plain
            .iter()
            .flat_map(|step| &step.filesystem)
            .all(|entry| entry.mode.is_none()),
        "modes recorded without snapshot_modes"
    );
    let temp = Temp::new()?;
    for broken in [
        with(&case, vec![("expected", Value::Object(Object::default()))]),
        with(&case, vec![("steps", literal(r#"[{"remove": []}]"#)?)]),
    ] {
        let path = save(&temp.0, &[broken], Vec::new())?;
        refused(&path, &["step case"], "a step case with its own command")?;
    }
    let path = save(&temp.0, &[], vec![("snapshot_mode", Value::Bool(true))])?;
    refused(&path, &["unknown corpus option"], "a misspelled option")
}

fn command_step() -> Result<Value, Failure> {
    literal(
        r#"{"argv": [], "stdin_b64": "", "expected": {"exit": 0, "stdout": {"mode": "bytes", "bytes_b64": ""},
            "stderr": {"mode": "bytes", "bytes_b64": ""}, "filesystem": []}}"#,
    )
}

fn file_size_limit(scratch: &Scratch) -> Result<(), Failure> {
    let case = literal(
        r#"{"id": "probe", "program": "writer", "initial": [], "steps": [
            {"argv": ["small", "4"], "stdin_b64": "", "file_size_limit": 4},
            {"argv": ["large", "5"], "stdin_b64": "", "file_size_limit": 4},
            {"argv": ["free", "5"], "stdin_b64": ""}]}"#,
    )?;
    let results = observe(&case, no_options(), scratch, &["size"])?;
    let [small, large, free] = results.as_slice() else {
        return Err(Failure::Error(format!(
            "{} results for three commands",
            results.len()
        )));
    };
    let exits = (small.exit, large.exit, free.exit);
    ensure!(exits == (0, 1, 0), "exits {exits:?}");
    ensure!(
        large.stderr == b"File too large\n",
        "stderr {:?}",
        String::from_utf8_lossy(&large.stderr)
    );
    let sizes = files(free);
    let size = |name: &str| sizes.get(name.as_bytes()).map(Vec::len);
    let found = (size("small"), size("large"), size("free"));
    ensure!(found == (Some(4), Some(4), Some(5)), "sizes {found:?}");
    let command = command_step()?;
    let temp = Temp::new()?;
    for broken in [
        vec![
            command.clone(),
            literal(r#"{"remove": [], "file_size_limit": 4}"#)?,
        ],
        vec![with(&command, vec![("file_size_limit", n(-1))])],
        vec![with(&command, vec![("file_size_limit", Value::Bool(true))])],
    ] {
        let path = save(
            &temp.0,
            &[with(&case, vec![("steps", Value::Array(broken))])],
            Vec::new(),
        )?;
        refused(
            &path,
            &["file_size_limit"],
            "a limit that is not a command's byte count",
        )?;
    }
    Ok(())
}

fn fail_write(scratch: &Scratch) -> Result<(), Failure> {
    let case = obj(vec![
        ("id", s("probe")),
        ("program", s("writer")),
        ("initial", Value::Array(Vec::new())),
        (
            "steps",
            Value::Array(vec![
                obj(vec![
                    ("argv", Value::Array(vec![s("kept"), s("refused")])),
                    ("stdin_b64", s("")),
                    ("fail_write_b64", enc(b"out/refused")),
                ]),
                obj(vec![
                    ("argv", Value::Array(vec![s("refused")])),
                    ("stdin_b64", s("")),
                ]),
            ]),
        ),
    ]);
    let results = observe(&case, no_options(), scratch, &["fail-write"])?;
    let [faulted, free] = results.as_slice() else {
        return Err(Failure::Error(format!(
            "{} results for two commands",
            results.len()
        )));
    };
    ensure!(
        faulted.stderr == b"refused No space left on device\n",
        "stderr {:?}",
        String::from_utf8_lossy(&faulted.stderr)
    );
    let written: Vec<Vec<u8>> = files(faulted).into_keys().collect();
    ensure!(written == [b"out/kept".to_vec()], "wrote {written:?}");
    ensure!(
        free.stderr.is_empty(),
        "the unfaulted step failed: {:?}",
        String::from_utf8_lossy(&free.stderr)
    );
    let command = command_step()?;
    let temp = Temp::new()?;
    for (broken, needle) in [
        (
            vec![
                command.clone(),
                obj(vec![
                    ("remove", Value::Array(Vec::new())),
                    ("fail_write_b64", enc(b"a")),
                ]),
            ],
            "fail_write_b64",
        ),
        (
            vec![with(&command, vec![("fail_write_b64", n(1))])],
            "fail_write_b64",
        ),
        (
            vec![with(&command, vec![("fail_write_b64", enc(b"../a"))])],
            "relative",
        ),
    ] {
        let path = save(
            &temp.0,
            &[with(&case, vec![("steps", Value::Array(broken))])],
            Vec::new(),
        )?;
        refused(
            &path,
            &[needle],
            "a fault that is not a command's relative path",
        )?;
    }
    Ok(())
}

fn step_baseline_defect(scratch: &Scratch) -> Result<(), Failure> {
    let expected = |text: &[u8]| {
        obj(vec![
            ("exit", n(0)),
            ("stdout", bytes_stream(text)),
            ("stderr", bytes_stream(b"")),
            (
                "filesystem",
                Value::Array(vec![obj(vec![
                    ("path_b64", enc(b".home")),
                    ("kind", s("directory")),
                ])]),
            ),
        ])
    };
    let known = obj(vec![
        ("observed", expected(b"old\n")),
        ("resolution", s("d.json#probe")),
    ]);
    let command = |arg: &str, text: &[u8]| {
        obj(vec![
            ("argv", Value::Array(vec![s(arg)])),
            ("stdin_b64", s("")),
            ("expected", expected(text)),
        ])
    };
    let case = obj(vec![
        ("id", s("probe")),
        ("program", s("writer")),
        ("initial", Value::Array(Vec::new())),
        (
            "steps",
            Value::Array(vec![
                command("same", b"same\n"),
                with(
                    &command("old", b"new\n"),
                    vec![("baseline_defect", known.clone())],
                ),
            ]),
        ),
    ]);
    let temp = Temp::new()?;
    let path = save(&temp.0, std::slice::from_ref(&case), Vec::new())?;
    let found = run_saved(&path, scratch, &[("writer", writer(&["printer"])?)])?;
    ensure!(
        found
            .first()
            .is_some_and(|first| first.starts_with("step 2: stdout bytes")),
        "the defect step did not fail: {found:?}"
    );
    let mut placed = list(&case, "steps")?.to_vec();
    placed.push(obj(vec![
        ("remove", Value::Array(Vec::new())),
        ("baseline_defect", known),
    ]));
    let path = save(
        &temp.0,
        &[with(&case, vec![("steps", Value::Array(placed))])],
        Vec::new(),
    )?;
    refused(
        &path,
        &["step case"],
        "a baseline defect on a step without a command",
    )
}

/// Runs the release writer against a stub release state and returns each `CODE STDOUT` line and the files left.
fn release(scratch: &Scratch, state: Vec<(&str, Value)>) -> Result<(Vec<Vec<u8>>, Files), Failure> {
    let mut release = obj(vec![
        ("repository", s("o/r")),
        ("tag", s("evidence-t")),
        ("id", n(7)),
    ]);
    for (key, value) in state {
        set(&mut release, key, value);
    }
    let case = obj(vec![
        ("id", s("probe")),
        ("program", s("writer")),
        ("argv", Value::Array(Vec::new())),
        ("stdin_b64", s("")),
        (
            "initial",
            Value::Array(vec![
                file(b"a.bin", b"0123456789"),
                file(b"gh/release.json", json::dumps(&release).as_bytes()),
            ]),
        ),
    ]);
    let actual = single(&case, gh_options(), scratch, &["gh-release"])?;
    let lines = gh_lines(&actual.stdout)?
        .into_iter()
        .map(|words| {
            let mut line = words.first().cloned().unwrap_or_default();
            line.push(b' ');
            line.extend(words.get(1).cloned().unwrap_or_default());
            line
        })
        .collect();
    Ok((lines, files(&actual)))
}

fn shown(lines: &[Vec<u8>]) -> Vec<String> {
    lines
        .iter()
        .map(|line| String::from_utf8_lossy(line).into_owned())
        .collect()
}

fn gh_release(scratch: &Scratch) -> Result<(), Failure> {
    let faults = |names: &[&str]| Value::Array(names.iter().map(|name| s(name)).collect());
    let (lines, found) = release(scratch, Vec::new())?;
    let wanted = [
        &b"0 {\"id\": 7, \"tag_name\": \"evidence-t\", \"draft\": false}"[..],
        b"0 [[]]",
        b"0 ",
        b"0 0123456789",
        b"2 ",
    ];
    ensure!(lines == wanted, "lines {:?}", shown(&lines));
    ensure!(
        named(&found, "gh/assets/101/a.bin")? == b"0123456789",
        "the uploaded asset differs"
    );
    let recorded = calls(&found)?;
    ensure!(recorded.len() == 5, "{} calls", recorded.len());
    let upload = literal(r#"["release", "upload", "evidence-t", "a.bin", "--repo", "o/r"]"#)?;
    ensure!(
        recorded[2].py_eq(&upload),
        "upload {}",
        recorded[2].py_repr()
    );
    let (lines, found) = release(
        scratch,
        vec![
            ("faults", faults(&["partial-upload"])),
            ("filler_assets", n(150)),
        ],
    )?;
    ensure!(
        lines[1].starts_with(b"0 [[{") && lines[2] == b"1 " && lines[3] == b"1 ",
        "partial upload: {:?}",
        shown(&lines)
    );
    ensure!(
        named(&found, "gh/assets/151/a.bin")? == b"01234",
        "the partial asset differs"
    );
    let (lines, _) = release(scratch, vec![("faults", faults(&["truncated-download"]))])?;
    ensure!(
        lines[2] == b"0 " && lines[3] == b"0 01234",
        "truncated download: {:?}",
        shown(&lines)
    );
    let (lines, _) = release(
        scratch,
        vec![(
            "faults",
            faults(&["lost-upload-response", "oversized-download"]),
        )],
    )?;
    ensure!(
        lines[2] == b"1 " && lines[3].starts_with(b"0 0123456789\x00"),
        "lost response and oversized download: {:?}",
        shown(&lines)
    );
    let (lines, found) = release(scratch, vec![("faults", faults(&["rejected-upload"]))])?;
    ensure!(
        lines[2] == b"1 " && lines[3] == b"1 ",
        "rejected upload: {:?}",
        shown(&lines)
    );
    ensure!(
        !found.keys().any(|path| path.starts_with(b"gh/assets")),
        "a rejected upload stored an asset"
    );
    Ok(())
}

const STRUCTURED: &str = r####"{
    "pins": [{"glob": "out/receipt.json", "pointer": "/verdicts_sha256", "path": "verdicts.json", "base": "record"},
             {"glob": "out/receipt.json", "map": "/prepared_files", "base": "record"},
             {"glob": "out/grades.json"}],
    "tokens": [{"prefix": "blind",
                "sites": [{"glob": "out/grades.json", "pointers": ["/reviews/0/token", "/reviews/1/token"]}],
                "references": [{"stream": "stdout"}, {"glob": "out/reviews/*.md", "names": true},
                               {"glob": "out/verdicts.json", "pointers": ["/reviews"], "keys": true},
                               {"glob": "out/receipt.json", "pointers": ["/prepared_files"], "keys": true}]},
               {"prefix": "review",
                "sites": [{"glob": "out/grades.json", "pointers": ["/links/0", "/links/1", "/links/2"]}],
                "references": [{"glob": "out/dossier.md"}]}],
    "clocks": [{"glob": "out/receipt.json", "pointers": ["/mapped_at"]}],
    "identity": [{"glob": "out/receipt.json", "pointers": ["/runner/mapping"], "label": "grade-map-runner-identity-v2"}],
    "sections": [{"glob": "out/dossier.md", "heading": "### "}]}"####;

fn grading_inputs() -> Value {
    Value::Array(vec![
        file(b"in/quote.txt", b"The output must contain blind-abcdef."),
        file(b"in/quote.json", br#"{"quote": "The output must contain blind-\u0063afe01."}"#),
        file(
            b"in/source.go",
            b"package main\n\nvar names = []string{\"review-012345\", \"review-6789ab\", \"review-abcdef\"}\n",
        ),
    ])
}

fn grading_case(seed: u32, mutation: &str, structured: Option<Value>) -> Value {
    let mut case = obj(vec![
        ("id", s("probe")),
        ("program", s("writer")),
        (
            "argv",
            Value::Array(vec![s(&seed.to_string()), s(mutation)]),
        ),
        ("stdin_b64", s("")),
        ("initial", grading_inputs()),
    ]);
    if let Some(structured) = structured {
        set(&mut case, "structured", structured);
    }
    case
}

fn grading(scratch: &Scratch, seed: u32, mutation: &str) -> Result<Observed, Failure> {
    single(
        &grading_case(seed, mutation, Some(literal(STRUCTURED)?)),
        no_options(),
        scratch,
        &["grading"],
    )
}

fn structured_grading(scratch: &Scratch) -> Result<(), Failure> {
    let first = grading(scratch, 1, "none")?;
    let second = grading(scratch, 2, "none")?;
    ensure!(
        first.inconsistencies.is_empty(),
        "inconsistent: {:?}",
        first.inconsistencies
    );
    let raw = single(
        &grading_case(1, "none", None),
        no_options(),
        scratch,
        &["grading"],
    )?;
    ensure!(
        raw.stdout != first.stdout,
        "an undeclared run masked its tokens"
    );
    ensure!(
        first.stdout == b"mapped <blind 1>\n",
        "stdout {:?}",
        String::from_utf8_lossy(&first.stdout)
    );
    let found = differences(&first, &second, &[])?;
    ensure!(found.is_empty(), "two seeds differ: {found:?}");
    let written = files(&first);
    ensure!(
        written.contains_key(&b"out/reviews/<blind 2>.md"[..]),
        "no labelled review name"
    );
    let receipt = json::value(named(&written, "out/receipt.json")?)?;
    ensure!(
        string(&receipt, "mapped_at")? == "<volatile>"
            && field(field(&receipt, "runner")?, "mapping")?
                .py_eq(&s("grade-map-runner-identity-v2")),
        "receipt {}",
        receipt.py_repr()
    );
    ensure!(
        string(&receipt, "verdicts_sha256")? == hex_digest(named(&written, "out/verdicts.json")?),
        "the receipt pin was not rewritten"
    );
    let grades = json::value(named(&written, "out/grades.json")?)?;
    ensure!(
        string(field(field(&grades, "assessor")?, "receipt")?, "sha256")?
            == hex_digest(named(&written, "out/receipt.json")?),
        "the grades pin was not rewritten"
    );
    let dossier = String::from_utf8_lossy(named(&written, "out/dossier.md")?).into_owned();
    let related = dossier.find("(related)").ok_or("no related link")?;
    let same = dossier.find("(same)").ok_or("no same link")?;
    ensure!(
        related < same,
        "dossier sections are not ordered: {dossier}"
    );
    ensure!(
        String::from_utf8_lossy(named(&written, "out/source.go")?).contains("\"review-abcdef\""),
        "given source text was masked"
    );
    ensure!(
        String::from_utf8_lossy(named(&written, "out/verdicts.json")?)
            .contains("The output must contain blind-abcdef."),
        "given quote text was masked"
    );
    ensure!(
        named(&written, "out/quote.txt")? == b"The output must contain blind-cafe01.",
        "a decoded input literal was masked"
    );
    for mutation in [
        "credit",
        "unresolved",
        "quote",
        "source",
        "section",
        "pin",
        "section-drop",
        "section-dup",
    ] {
        let changed = grading(scratch, 2, mutation)?;
        ensure!(
            changed.inconsistencies.is_empty(),
            "{mutation}: {:?}",
            changed.inconsistencies
        );
        let found = differences(&first, &changed, &[])?;
        ensure!(
            mentions(&found, "filesystem effects"),
            "{mutation}: {found:?}"
        );
    }
    Ok(())
}

fn structured_labels(scratch: &Scratch) -> Result<(), Failure> {
    let first = grading(scratch, 1, "none")?;
    for mutation in ["source-label", "quote-label", "quote-escaped"] {
        let changed = grading(scratch, 2, mutation)?;
        for (left, right) in [(&first, &changed), (&changed, &first)] {
            let found = differences(left, right, &[])?;
            ensure!(
                mentions(&found, "filesystem effects"),
                "{mutation}: {found:?}"
            );
        }
    }
    Ok(())
}

fn collision(scratch: &Scratch, run: &str, mutation: &str) -> Result<Observed, Failure> {
    let case = obj(vec![
        ("id", s("probe")),
        ("program", s("writer")),
        ("argv", Value::Array(vec![s(run), s(mutation)])),
        ("stdin_b64", s("")),
        (
            "initial",
            Value::Array(vec![file(
                b"input.json",
                br#"{"quote":"The output must contain blind-\u0061bcdef."}"#,
            )]),
        ),
        (
            "structured",
            literal(
                r#"{"tokens": [{"prefix": "blind", "sites": [{"glob": "ids.json", "pointers": ["/token"]}], "references": []}]}"#,
            )?,
        ),
    ]);
    single(&case, no_options(), scratch, &["collision"])
}

fn structured_collision(scratch: &Scratch) -> Result<(), Failure> {
    let primary = collision(scratch, "primary", "none")?;
    let candidate = collision(scratch, "candidate", "quote")?;
    ensure!(
        primary.inconsistencies.iter().any(|issue| issue
            .starts_with("blind-abcdef: the run generated a token the case's inputs already hold")),
        "a token equal to an input literal passed: {:?}",
        primary.inconsistencies
    );
    for (left, right) in [(&primary, &candidate), (&candidate, &primary)] {
        let found = differences(left, right, &[])?;
        ensure!(
            mentions(&found, "consistency: blind-abcdef") && mentions(&found, "filesystem effects"),
            "{found:?}"
        );
    }
    let plain = collision(scratch, "plain", "none")?;
    ensure!(
        plain.inconsistencies.is_empty(),
        "inconsistent: {:?}",
        plain.inconsistencies
    );
    let written = files(&plain);
    ensure!(
        named(&written, "ids.json")? == br#"{"token": "<blind 1>"}"#,
        "the site was not labelled"
    );
    ensure!(
        named(&written, "notes.txt")? == b"minted blind-111111",
        "undeclared bytes were masked"
    );
    Ok(())
}

fn structured_sites(scratch: &Scratch) -> Result<(), Failure> {
    let structured = literal(
        r#"{"tokens": [{"prefix": "blind", "references": [], "sites": [
            {"glob": "out/grades.json", "pointers": ["/reviews/0/token", "/reviews/2/token", "/reviews/0/families/0/id"]},
            {"glob": "out/missing.json", "pointers": ["/token"]}]}]}"#,
    )?;
    let issues = single(
        &grading_case(1, "none", Some(structured)),
        no_options(),
        scratch,
        &["grading"],
    )?
    .inconsistencies;
    ensure!(issues.len() == 3, "issues {issues:?}");
    ensure!(
        mentions(&issues, "/reviews/2/token should hold a blind token"),
        "{issues:?}"
    );
    ensure!(
        issues.contains(
            &r#"out/grades.json: /reviews/0/families/0/id holds "GT-t1", not a blind token"#
                .to_string()
        ),
        "{issues:?}"
    );
    ensure!(
        issues.contains(&"out/missing.json: no file holds the declared blind tokens".to_string()),
        "{issues:?}"
    );
    Ok(())
}

fn structured_pins(scratch: &Scratch) -> Result<(), Failure> {
    let first = grading(scratch, 1, "none")?;
    let wrong = grading(scratch, 2, "wrong-pin")?;
    ensure!(
        mentions(&wrong.inconsistencies, "/verdicts_sha256 pins 0000"),
        "{:?}",
        wrong.inconsistencies
    );
    let starts = |found: &[String], prefix: &str| found.iter().any(|item| item.starts_with(prefix));
    ensure!(
        starts(&differences(&first, &wrong, &[])?, "consistency:"),
        "a wrong candidate pin passed"
    );
    ensure!(
        starts(&differences(&wrong, &first, &[])?, "primary consistency:"),
        "a wrong primary pin passed"
    );
    let found = compare(expectation(&first, &[]), &grading(scratch, 3, "none")?)?;
    ensure!(found.is_empty(), "a third seed differs: {found:?}");
    ensure!(
        starts(&compare(expectation(&first, &[]), &wrong)?, "consistency:"),
        "a wrong pin passed"
    );
    Ok(())
}

fn keyed(scratch: &Scratch, seed: u32, mutation: &str) -> Result<Observed, Failure> {
    let case = obj(vec![
        ("id", s("probe")),
        ("program", s("writer")),
        (
            "argv",
            Value::Array(vec![s(&seed.to_string()), s(mutation)]),
        ),
        ("stdin_b64", s("")),
        ("initial", Value::Array(Vec::new())),
        (
            "structured",
            literal(
                r####"{"tokens": [{"prefix": "blind", "sites": [{"glob": "key.json", "pointers": [""], "keys": true}],
                                "references": [{"glob": "dossier.md", "line_starts": ["### "]}, {"stream": "stdout"}]}],
                    "members": [{"glob": "key.json", "pointers": [""]}]}"####,
            )?,
        ),
    ]);
    single(&case, no_options(), scratch, &["keyed"])
}

fn structured_keys(scratch: &Scratch) -> Result<(), Failure> {
    let first = keyed(scratch, 1, "none")?;
    ensure!(
        first.inconsistencies.is_empty(),
        "inconsistent: {:?}",
        first.inconsistencies
    );
    let found = differences(&first, &keyed(scratch, 2, "none")?, &[])?;
    ensure!(found.is_empty(), "two seeds differ: {found:?}");
    let written = files(&first);
    let key = json::value(named(&written, "key.json")?)?;
    ensure!(
        key.py_eq(&literal(
            r#"{"<blind 1>": "att-001", "<blind 2>": "att-002"}"#
        )?),
        "key {}",
        key.py_repr()
    );
    ensure!(
        named(&written, "dossier.md")? == b"### <blind 1>\n\nQuotes blind-abcdef.\n\n### <blind 2>\n\nQuotes blind-abcdef.\n\n",
        "dossier {:?}",
        String::from_utf8_lossy(named(&written, "dossier.md")?)
    );
    ensure!(
        first.stdout
            == br#"{"claims": 2, "first": "<blind 1>"}
"#,
        "stdout {:?}",
        String::from_utf8_lossy(&first.stdout)
    );
    let found = differences(&first, &keyed(scratch, 2, "swap")?, &[])?;
    ensure!(
        mentions(&found, "filesystem effects"),
        "swapped keys passed: {found:?}"
    );
    let extra = keyed(scratch, 2, "extra-key")?.inconsistencies;
    ensure!(
        extra.contains(
            &r#"key.json:  has the key "plain", which no token rule declares"#.to_string()
        ),
        "{extra:?}"
    );
    let compact = keyed(scratch, 2, "compact")?.inconsistencies;
    ensure!(
        mentions(
            &compact,
            "declared members unreadable: not written as json.dumps"
        ),
        "{compact:?}"
    );
    let outcome = literal(
        r#"{"kind": "family-outcome", "stream": "stdout", "pointer": "/claims", "value": 2}"#,
    )?;
    let found = compare(expectation(&first, std::slice::from_ref(&outcome)), &first)?;
    ensure!(found.is_empty(), "a held outcome differs: {found:?}");
    let wrong = with(&outcome, vec![("value", n(3))]);
    let found = compare(expectation(&first, &[wrong]), &first)?;
    ensure!(
        found == ["outcome family-outcome stdout/claims: expected 3, actual 2"],
        "{found:?}"
    );
    Ok(())
}

const SITES: &str = r#"{"tokens": [{"prefix": "blind", "ordinals": "sites",
    "sites": [{"glob": "work/key.json", "pointers": ["/reviews/0/token", "/reviews/1/token", "/reviews/2/token"]}],
    "references": [{"glob": "prompt.md", "line_starts": ["- `reviews/"]}]}],
    "lines": [{"glob": "prompt.md", "start": "- `reviews/"}]}"#;

fn sites(
    scratch: &Scratch,
    seed: u32,
    mutation: &str,
    structured: &Value,
) -> Result<Observed, Failure> {
    let case = obj(vec![
        ("id", s("probe")),
        ("program", s("writer")),
        (
            "argv",
            Value::Array(vec![s(&seed.to_string()), s(mutation)]),
        ),
        ("stdin_b64", s("")),
        ("initial", Value::Array(Vec::new())),
        ("mask_root", Value::Bool(true)),
        ("structured", structured.clone()),
    ]);
    single(&case, no_options(), scratch, &["sites"])
}

fn structured_ordinals(scratch: &Scratch) -> Result<(), Failure> {
    let spec = literal(SITES)?;
    let first = sites(scratch, 1, "none", &spec)?;
    ensure!(
        first.inconsistencies.is_empty(),
        "inconsistent: {:?}",
        first.inconsistencies
    );
    let found = differences(&first, &sites(scratch, 2, "none", &spec)?, &[])?;
    ensure!(found.is_empty(), "two seeds differ: {found:?}");
    let written = files(&first);
    let key = json::value(named(&written, "work/key.json")?)?;
    let wanted = literal(
        r#"{"reviews": [{"token": "<blind 1>"}, {"token": "<blind 2>"}, {"token": "<blind 3>"}], "root": "<root>"}"#,
    )?;
    ensure!(key.py_eq(&wanted), "key {}", key.py_repr());
    ensure!(
        named(&written, "prompt.md")?
            == b"Reviews:\n- `reviews/<blind 1>.md`\n- `reviews/<blind 2>.md`\n- `reviews/<blind 3>.md`\nEnd.\n",
        "prompt {:?}",
        String::from_utf8_lossy(named(&written, "prompt.md")?)
    );
    let found = differences(&first, &sites(scratch, 2, "drop", &spec)?, &[])?;
    ensure!(
        mentions(&found, "filesystem effects"),
        "a dropped line passed: {found:?}"
    );
    let mut by_scan = spec.clone();
    if let Value::Object(object) = &mut by_scan
        && let Some(Value::Array(tokens)) = object.get_mut("tokens")
        && let Some(rule) = tokens.first_mut()
    {
        *rule = without(rule, "ordinals");
    }
    let found = differences(
        &sites(scratch, 1, "none", &by_scan)?,
        &sites(scratch, 2, "none", &by_scan)?,
        &[],
    )?;
    ensure!(
        mentions(&found, "filesystem effects"),
        "scan order hid a reordering: {found:?}"
    );
    Ok(())
}

fn structured_declarations(_: &Scratch) -> Result<(), Failure> {
    let base = literal(
        r#"{"id": "probe", "program": "writer", "argv": [], "stdin_b64": "", "initial": []}"#,
    )?;
    let sites = r#"[{"glob": "x", "pointers": ["/a"]}]"#;
    let structured = |text: &str| -> Result<Value, Failure> {
        Ok(with(&base, vec![("structured", literal(text)?)]))
    };
    let temp = Temp::new()?;
    for (broken, needle) in [
        (
            with(
                &structured(r#"{"tokens": ["blind"]}"#)?,
                vec![("volatile", Value::Array(Vec::new()))],
            ),
            "structured comparison",
        ),
        (
            structured(r#"{"pins": [{"glob": "x", "pointer": "/a"}]}"#)?,
            "pin rule",
        ),
        (structured(r#"{"tokens": ["blind"]}"#)?, "token rule"),
        (
            structured(r#"{"tokens": [{"prefix": "blind", "sites": [{"glob": "x"}]}]}"#)?,
            "token rule",
        ),
        (
            structured(&format!(
                r#"{{"tokens": [{{"prefix": "blind", "sites": {sites}}}]}}"#
            ))?,
            "token rule",
        ),
        (
            structured(&format!(
                r#"{{"tokens": [{{"prefix": "blind", "sites": {sites}, "references": [{{"glob": "x", "keys": true}}]}}]}}"#
            ))?,
            "token rule",
        ),
        (
            structured(r#"{"identity": [{"glob": "x", "pointers": ["/a"]}]}"#)?,
            "decided label",
        ),
        (
            structured(r#"{"sections": [{"glob": "x", "heading": "Links"}]}"#)?,
            "heading prefix",
        ),
        (structured(r#"{"lines": [{"glob": "x"}]}"#)?, "lines rule"),
        (
            structured(&format!(
                r#"{{"tokens": [{{"prefix": "blind", "sites": {sites}, "references": [], "ordinals": "order"}}]}}"#
            ))?,
            "token rule",
        ),
        (
            with(
                &base,
                vec![(
                    "expected",
                    literal(
                        r#"{"outcomes": [{"kind": "score", "path": "g", "pointer": "", "value": 1}]}"#,
                    )?,
                )],
            ),
            "outcome states its kind",
        ),
    ] {
        let path = save(&temp.0, &[broken], Vec::new())?;
        refused(&path, &[needle], needle)?;
    }
    Ok(())
}

fn hand_stated_outcomes(scratch: &Scratch) -> Result<(), Failure> {
    let first = grading(scratch, 1, "none")?;
    let credit = grading(scratch, 2, "credit")?;
    let outcomes = [
        literal(
            r#"{"kind": "family-outcome", "path": "out/grades.json", "pointer": "/reviews/1/families/0/outcome", "value": "caught"}"#,
        )?,
        literal(
            r#"{"kind": "credit-fact", "path": "out/grades.json", "pointer": "/reviews/1/credit/identifies_cause", "value": "no"}"#,
        )?,
    ];
    let found = compare(expectation(&first, &outcomes), &first)?;
    ensure!(found.is_empty(), "held outcomes differ: {found:?}");
    let found = compare(expectation(&first, &outcomes), &credit)?;
    ensure!(
        found.contains(
            &r#"outcome credit-fact out/grades.json/reviews/1/credit/identifies_cause: expected "no", actual "yes""#
                .to_string()
        ),
        "{found:?}"
    );
    let found = differences(&first, &credit, &outcomes)?;
    ensure!(
        found
            .iter()
            .any(|item| item.starts_with("outcome credit-fact")),
        "a changed credit fact passed: {found:?}"
    );
    let missing = with(&outcomes[0], vec![("path", s("out/missing.json"))]);
    let found = compare(expectation(&first, &[missing]), &first)?;
    ensure!(
        found
            .first()
            .is_some_and(|item| item.starts_with("outcome family-outcome out/missing.json")),
        "{found:?}"
    );
    Ok(())
}

fn same(left: &[Observed], right: &[Observed]) -> bool {
    left.len() == right.len()
        && left.iter().zip(right).all(|(left, right)| {
            left.exit == right.exit
                && left.stdout == right.stdout
                && left.stderr == right.stderr
                && left.inconsistencies == right.inconsistencies
                && expectation(left, &[]).py_eq(&expectation(right, &[]))
        })
}

fn fixture_git(scratch: &Scratch) -> Result<(), Failure> {
    let manifest = b"{\"freeze_commit\": \"<freeze commit>\"}\n";
    let case = obj(vec![
        ("id", s("probe")),
        ("program", s("show")),
        (
            "initial",
            Value::Array(vec![file(b"bench/runs/r/manifest.json", manifest)]),
        ),
        (
            "steps",
            literal(
                r#"[{"program": "fixture-git", "argv": ["bench/runs/r/manifest.json"], "stdin_b64": ""},
                    {"argv": ["show", "HEAD:bench/runs/r/manifest.json"], "stdin_b64": ""}]"#,
            )?,
        ),
    ]);
    let options = Corpus {
        options: no_options(),
        cases: Vec::new(),
    };
    let actual = execute::execute(
        &case,
        &options,
        scratch,
        &[
            ("show", vec!["git".into()]),
            ("fixture-git", vec!["/bin/false".into()]),
        ],
    )?;
    let again = execute::execute(&case, &options, scratch, &[("show", vec!["git".into()])])?;
    ensure!(same(&actual, &again), "two freezes differ");
    let [freeze, show] = actual.as_slice() else {
        return Err(Failure::Error(format!(
            "{} results for two commands",
            actual.len()
        )));
    };
    ensure!(
        (freeze.exit, show.exit) == (0, 0),
        "exits {} and {}: {}",
        freeze.exit,
        show.exit,
        String::from_utf8_lossy(&freeze.stderr)
    );
    ensure!(
        show.stdout == manifest,
        "the commit holds {:?}",
        String::from_utf8_lossy(&show.stdout)
    );
    let written = files(freeze);
    let frozen = json::value(named(&written, "bench/runs/r/manifest.json")?)?;
    let commit = string(&frozen, "freeze_commit")?;
    ensure!(
        commit.len() == 40
            && commit
                .bytes()
                .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f')),
        "freeze commit {commit:?}"
    );
    ensure!(
        named(&written, ".git/refs/heads/main")? == format!("{commit}\n").as_bytes(),
        "main does not name the freeze"
    );
    ensure!(
        !written.contains_key(&b".git/config"[..])
            && !written.contains_key(&b".git/COMMIT_EDITMSG"[..]),
        "host-dependent git files remain"
    );
    let index = named(&written, ".git/index")?;
    ensure!(
        index
            .get(12..36)
            .is_some_and(|stamps| stamps.iter().all(|byte| *byte == 0)),
        "the index keeps file timestamps"
    );
    Ok(())
}
