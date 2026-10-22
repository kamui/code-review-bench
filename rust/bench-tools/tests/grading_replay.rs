//! Writes `docs/python-migration/grading-replay.v1.json`, which accounts for every assessment receipt in the current
//! record: the regrading archive that holds its key, validator inputs and verdicts, whether those bytes restore by
//! hash through the evidence manifests, its verdict contract and the commit that filed it. The Pages build job runs
//! it after restoring the repository evidence and fails when the committed file differs:
//!
//! ```sh
//! cargo test --locked --test grading_replay -- --ignored --exact the_replay_inventory_accounts_for_every_receipt
//! git diff --exit-code -- docs/python-migration/grading-replay.v1.json
//! ```
//!
//! The same job then replays the blinded validator: `bench-tools grading validate` on each restorable archive's
//! validator inputs and verdicts must give the stdout and exit status `EXPECTATIONS` records. Last it replays the
//! mapping: `bench-tools grade map` maps every restorable archive's workspace and key with a fixture `--assessor`,
//! through `--root`, into a disposable worktree of the commit that filed its receipt, and must end with the recorded
//! exit status and lines and write nothing. `EXPECTATIONS` holds the retained Python's outcomes, recorded before
//! `grade.py` lost `validate` and `map`. A verdict contract no replay accepts must name its fixture case in
//! `FIXTURE_CASES`.

use bench_tools::archive;
use bench_tools::pyjson::{self, Dumps, Duplicates, Object, Value};
use bench_tools::sha256::hex_digest;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

const RECEIPTS: &str = "bench/grading/current/assessments";
const ARCHIVES: &str = "bench/regrading";
const MANIFESTS: &str = "bench/evidence/manifests";
const OUTPUT: &str = "docs/python-migration/grading-replay.v1.json";
const EXPECTATIONS: &str = "docs/python-migration/grading-replay-expectations.v1.json";
const KEY: &str = "key.json";
const INPUTS: &str = "work/validator/inputs.json";
const VERDICTS: &str = "work/verdicts.json";
/// `grading_validation.CONTRACT`, which a validator input or policy without `contract` or `verdicts` uses.
/// `evidence.json` lists each member's hash but not its size, so the decoded stream is bounded by this instead. The
/// archive itself is checked against its manifest hash before it is read.
const MEMBER_BYTES_LIMIT: u64 = 4 * 1024 * 1024 * 1024;
const DEFAULT_CONTRACT: &str = "current-verdicts/v1";
const RECORD_LOCK: &str = ".cache/current-record.lock";
const MAP_CORPUS: &str = "tools/behavior_contracts/grade-map.v1.json";
/// The `MAP_CORPUS` case that maps each verdict contract's verdicts in both implementations, for a contract no
/// archived mapping replay accepts.
const FIXTURE_CASES: [(&str, &str); 3] = [
    ("current-verdicts/v1", "grade-map-verdicts-v1"),
    ("current-verdicts/v2", "grade-map-verdicts-v2"),
    ("current-verdicts/v3", "grade-map-pending-candidate"),
];
/// Fields `bench-tools grade map` copies from the key into the receipt, and the key field the receipt's
/// `runner.prepared` copies.
const KEY_FIELDS: [&str; 6] = [
    "run",
    "target",
    "input_fingerprint",
    "prepared_files",
    "validator",
    "claim_snapshot",
];

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn read_json(path: &Path) -> Value {
    let text =
        std::fs::read_to_string(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    pyjson::loads(&text, Duplicates::Refuse)
        .unwrap_or_else(|error| panic!("{}: {error:?}", path.display()))
}

fn field<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    path.iter()
        .try_fold(value, |value, key| value.as_object()?.get(key))
}

fn text<'a>(value: &'a Value, path: &[&str]) -> Option<&'a str> {
    field(value, path).and_then(Value::as_str)
}

fn files_named(directory: &Path, name: &str, found: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            files_named(&path, name, found);
        } else if path.file_name().is_some_and(|file| file == name) {
            found.push(path);
        }
    }
}

fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap()
        .to_string_lossy()
        .into_owned()
}

fn git(root: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .unwrap();
    output
        .status
        .success()
        .then(|| String::from_utf8(output.stdout).unwrap())
}

/// The newest commit that added each receipt, from one walk of the history.
fn filing_commits(root: &Path) -> BTreeMap<String, String> {
    assert_eq!(
        git(root, &["rev-parse", "--is-shallow-repository"]).as_deref(),
        Some("false\n"),
        "the filing commits need the full history"
    );
    let log = git(
        root,
        &[
            "log",
            "--diff-filter=A",
            "--format=commit %H",
            "--name-only",
            "--",
            RECEIPTS,
        ],
    )
    .unwrap();
    let mut commits = BTreeMap::new();
    let mut current = String::new();
    for line in log.lines().filter(|line| !line.is_empty()) {
        match line.strip_prefix("commit ") {
            Some(commit) => current = commit.to_string(),
            None => {
                commits
                    .entry(line.to_string())
                    .or_insert_with(|| current.clone());
            }
        }
    }
    commits
}

struct Archive {
    evidence: String,
    archive: String,
    archive_sha256: String,
    files: BTreeMap<String, String>,
}

fn archives(root: &Path) -> Vec<Archive> {
    let mut found = Vec::new();
    files_named(&root.join(ARCHIVES), "evidence.json", &mut found);
    found.sort();
    found
        .iter()
        .map(|path| {
            let evidence = read_json(path);
            let files = field(&evidence, &["files"])
                .and_then(Value::as_array)
                .unwrap_or_default()
                .iter()
                .filter_map(|file| {
                    Some((
                        text(file, &["path"])?.to_string(),
                        text(file, &["sha256"])?.to_string(),
                    ))
                })
                .collect();
            Archive {
                evidence: relative(root, path),
                archive: text(&evidence, &["archive", "path"]).unwrap().to_string(),
                archive_sha256: text(&evidence, &["archive", "sha256"]).unwrap().to_string(),
                files,
            }
        })
        .collect()
}

/// Every manifest member's SHA-256 by logical path, across all evidence manifests.
fn manifest_members(root: &Path) -> BTreeMap<String, String> {
    let mut members = BTreeMap::new();
    let mut names: Vec<PathBuf> = std::fs::read_dir(root.join(MANIFESTS))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    names.sort();
    for path in names {
        let manifest = read_json(&path);
        for package in field(&manifest, &["packages"])
            .and_then(Value::as_array)
            .unwrap_or_default()
        {
            for member in field(package, &["members"])
                .and_then(Value::as_array)
                .unwrap_or_default()
            {
                if let (Some(logical), Some(sha)) =
                    (text(member, &["path"]), text(member, &["sha256"]))
                {
                    members.insert(logical.to_string(), sha.to_string());
                }
            }
        }
    }
    members
}

/// The bytes of each archive member `wanted` names, read by hash: the archive must be the manifest's member, and each
/// file the bytes `evidence.json` lists. `Err` names the first step that failed.
fn restore_members(
    root: &Path,
    archive: &Archive,
    members: &BTreeMap<String, String>,
    wanted: impl Fn(&str) -> bool,
) -> Result<BTreeMap<String, Vec<u8>>, String> {
    if members.get(&archive.archive) != Some(&archive.archive_sha256) {
        return Err(
            "no evidence manifest lists the archive with the hash evidence.json pins".to_string(),
        );
    }
    let path = root.join(&archive.archive);
    let bytes = std::fs::read(&path).unwrap_or_else(|error| {
        panic!(
            "{}: {error}; restore the repository evidence first",
            archive.archive
        )
    });
    if hex_digest(&bytes) != archive.archive_sha256 {
        return Err("the restored archive differs from its manifest hash".to_string());
    }
    let mut found = BTreeMap::new();
    let limits = archive::Limits {
        members: archive.files.len() as u64,
        payload: MEMBER_BYTES_LIMIT,
    };
    archive::read_tar_gz(bytes.as_slice(), &limits, |member, data| {
        if let Some(name) = std::str::from_utf8(&member.name)
            .ok()
            .filter(|name| wanted(name))
        {
            let mut content = Vec::new();
            data.read_to_end(&mut content)?;
            found.insert(name.to_string(), content);
        }
        Ok(())
    })
    .map_err(|error| format!("the archive cannot be read: {error}"))?;
    for (name, bytes) in &found {
        if Some(&hex_digest(bytes)) != archive.files.get(name) {
            return Err(format!("{name} differs from the hash evidence.json lists"));
        }
    }
    Ok(found)
}

/// The bytes of an archive's key, validator inputs and verdicts, as `restore_members` reads them.
fn restore_bytes(
    root: &Path,
    archive: &Archive,
    members: &BTreeMap<String, String>,
) -> Result<[Vec<u8>; 3], String> {
    let mut found = restore_members(root, archive, members, |name| {
        [KEY, INPUTS, VERDICTS].contains(&name)
    })?;
    let mut take = |name: &str| {
        found
            .remove(name)
            .ok_or(format!("{name} is not in the archive"))
    };
    Ok([take(KEY)?, take(INPUTS)?, take(VERDICTS)?])
}

/// The parsed key, validator inputs and verdicts of an archive that `restore_bytes` reads.
fn restore(
    root: &Path,
    archive: &Archive,
    members: &BTreeMap<String, String>,
) -> Result<[Value; 3], String> {
    let mut values = Vec::new();
    for (name, bytes) in [KEY, INPUTS, VERDICTS]
        .into_iter()
        .zip(restore_bytes(root, archive, members)?)
    {
        let text = String::from_utf8(bytes).map_err(|_| format!("{name} is not UTF-8"))?;
        values.push(
            pyjson::loads(&text, Duplicates::Refuse)
                .map_err(|error| format!("{name}: {error:?}"))?,
        );
    }
    Ok([values.remove(0), values.remove(0), values.remove(0)])
}

fn same(left: Option<&Value>, right: Option<&Value>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => left.py_eq(right),
        (None, None) => true,
        _ => false,
    }
}

fn s(value: &str) -> Value {
    Value::Str(value.to_string())
}

fn n(value: usize) -> Value {
    Value::Int(value.to_string())
}

fn o(members: Vec<(&str, Value)>) -> Value {
    Value::Object(
        members
            .into_iter()
            .map(|(key, value)| (key.to_string(), value))
            .collect::<Object>(),
    )
}

fn optional(value: Option<&str>) -> Value {
    value.map_or(Value::Null, s)
}

fn fixture_case(contract: &str) -> Option<&'static str> {
    FIXTURE_CASES
        .iter()
        .find(|(name, _)| *name == contract)
        .map(|(_, case)| *case)
}

fn inventory(root: &Path) -> Value {
    let commits = filing_commits(root);
    let archives = archives(root);
    let members = manifest_members(root);
    let mut receipts = Vec::new();
    files_named(&root.join(RECEIPTS), "receipt.json", &mut receipts);
    receipts.sort();
    let mut rows = Vec::new();
    let mut validator: BTreeMap<String, usize> = BTreeMap::new();
    let mut mapping: BTreeMap<String, usize> = BTreeMap::new();
    let mut held: BTreeMap<String, usize> = BTreeMap::new();
    let mut restored_count = 0;
    for path in &receipts {
        let name = relative(root, path);
        let receipt = read_json(path);
        let commit = commits.get(&name).cloned();
        let verdicts = text(&receipt, &["verdicts_sha256"]);
        let inputs = text(&receipt, &["validator", "inputs.json"]);
        let candidates: Vec<&Archive> = archives
            .iter()
            .filter(|archive| {
                archive.files.contains_key(KEY)
                    && archive.files.get(VERDICTS).map(String::as_str) == verdicts
                    && archive.files.get(INPUTS).map(String::as_str) == inputs
            })
            .collect();
        let (archive, restored, contract, source, reason) = match candidates.as_slice() {
            [] => {
                let policy = commit.as_ref().and_then(|commit| {
                    git(
                        root,
                        &[
                            "show",
                            &format!("{commit}:bench/grading/current/validation-policy.json"),
                        ],
                    )
                });
                let contract = policy
                    .and_then(|policy| pyjson::loads(&policy, Duplicates::Refuse).ok())
                    .map(|policy| {
                        text(&policy, &["verdicts"])
                            .unwrap_or(DEFAULT_CONTRACT)
                            .to_string()
                    });
                (
                    None,
                    Value::Null,
                    contract,
                    "the validation policy at the filing commit",
                    Some("no archive under bench/regrading lists this receipt's key, validator inputs and verdicts".to_string()),
                )
            }
            [archive] => match restore(root, archive, &members) {
                Ok([key, inputs, _verdicts]) => {
                    let differing: Vec<&str> = KEY_FIELDS
                        .into_iter()
                        .filter(|name| !same(field(&key, &[name]), field(&receipt, &[name])))
                        .chain(
                            (!same(
                                field(&key, &["runner_deviation"]),
                                field(&receipt, &["runner", "prepared"]),
                            ))
                            .then_some("runner_deviation"),
                        )
                        .collect();
                    let contract = text(&inputs, &["contract"])
                        .unwrap_or(DEFAULT_CONTRACT)
                        .to_string();
                    if differing.is_empty() {
                        (
                            Some(*archive),
                            Value::Bool(true),
                            Some(contract),
                            "the archived validator inputs",
                            None,
                        )
                    } else {
                        (
                            Some(*archive),
                            Value::Bool(false),
                            Some(contract),
                            "the archived validator inputs",
                            Some(format!(
                                "the archived key differs from the receipt in {}",
                                differing.join(", ")
                            )),
                        )
                    }
                }
                Err(reason) => (
                    Some(*archive),
                    Value::Bool(false),
                    None,
                    "unknown",
                    Some(reason),
                ),
            },
            _ => (
                None,
                Value::Null,
                None,
                "unknown",
                Some(format!(
                    "{} archives list this receipt's verdicts",
                    candidates.len()
                )),
            ),
        };
        let contract_name = contract.clone().unwrap_or_else(|| "unknown".to_string());
        *held.entry(contract_name.clone()).or_default() += 1;
        if restored.py_eq(&Value::Bool(true)) {
            restored_count += 1;
            *validator.entry(contract_name.clone()).or_default() += 1;
            if commit.is_some() {
                *mapping.entry(contract_name).or_default() += 1;
            }
        }
        rows.push(o(vec![
            (
                "receipt",
                o(vec![
                    ("path", s(&name)),
                    ("sha256", s(&hex_digest(&std::fs::read(path).unwrap()))),
                ]),
            ),
            ("filed_by", optional(commit.as_deref())),
            ("contract", optional(contract.as_deref())),
            ("contract_from", s(source)),
            (
                "archive",
                optional(archive.map(|archive| archive.evidence.as_str())),
            ),
            ("restored", restored),
            ("reason", optional(reason.as_deref())),
        ]));
    }
    let counts = |source: &BTreeMap<String, usize>| {
        Value::Object(
            held.keys()
                .map(|contract| {
                    (
                        contract.clone(),
                        n(source.get(contract).copied().unwrap_or(0)),
                    )
                })
                .collect(),
        )
    };
    let gaps = held
        .iter()
        .filter(|(contract, _)| !validator.contains_key(*contract) || !mapping.contains_key(*contract))
        .map(|(contract, receipts)| {
            let cover = fixture_case(contract).map_or(String::new(), |case| {
                format!(" The {case} case in {MAP_CORPUS} maps verdicts of this contract instead.")
            });
            o(vec![
                ("contract", s(contract)),
                ("receipts", n(*receipts)),
                (
                    "gap",
                    s(&format!(
                        "No receipt of this contract has a restorable archive, so no validator or mapping replay \
                         exists.{cover}"
                    )),
                ),
            ])
        })
        .collect();
    o(vec![
        ("format", s("grading-replay/1")),
        (
            "description",
            s(
                "Every assessment receipt in the current record and the archived key, validator inputs and verdicts \
               a replay of it reads. A replay is possible when that archive restores by hash through the evidence \
               manifests and its key matches the receipt; a mapping replay also needs the filing commit. Written by \
               rust/bench-tools/tests/grading_replay.rs after the evidence fetch.",
            ),
        ),
        (
            "totals",
            o(vec![
                ("receipts", n(receipts.len())),
                ("archives", n(archives.len())),
                ("restorable", n(restored_count)),
                ("without_replay", n(receipts.len() - restored_count)),
            ]),
        ),
        ("receipts_by_contract", counts(&held)),
        (
            "replays",
            o(vec![
                ("validator", counts(&validator)),
                ("mapping", counts(&mapping)),
            ]),
        ),
        ("gaps", Value::Array(gaps)),
        ("receipts", Value::Array(rows)),
    ])
}

#[test]
#[ignore = "needs the restored repository evidence and full history; the Pages build job runs it"]
fn the_replay_inventory_accounts_for_every_receipt() {
    let root = repository();
    let value = inventory(&root);
    let rows = field(&value, &["receipts"])
        .and_then(Value::as_array)
        .unwrap();
    assert!(!rows.is_empty(), "the record holds no receipts");
    for row in rows {
        let accounted = field(row, &["restored"])
            .is_some_and(|restored| restored.py_eq(&Value::Bool(true)))
            || text(row, &["reason"]).is_some();
        assert!(accounted, "{row:?}");
    }
    let replays = field(&value, &["replays", "validator"])
        .and_then(Value::as_object)
        .unwrap();
    assert!(
        replays.iter().any(|(_, count)| !count.py_eq(&n(0))),
        "an empty replay fails"
    );
    let mut text = pyjson::dumps(
        &value,
        &Dumps {
            indent: Some(2),
            ..Dumps::default()
        },
    );
    text.push('\n');
    std::fs::write(root.join(OUTPUT), text).unwrap();
}

/// Each restorable archive's recorded outcome in one replay of `EXPECTATIONS`, without the group's `archives`.
fn expectations(root: &Path, replay: &str) -> BTreeMap<String, Value> {
    let value = read_json(&root.join(EXPECTATIONS));
    let mut found = BTreeMap::new();
    for group in field(&value, &[replay]).and_then(Value::as_array).unwrap() {
        let outcome = Value::Object(
            group
                .as_object()
                .unwrap()
                .iter()
                .filter(|(key, _)| key.as_str() != "archives")
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect(),
        );
        for archive in field(group, &["archives"])
            .and_then(Value::as_array)
            .unwrap()
        {
            let archive = archive.as_str().unwrap().to_string();
            let previous = found.insert(archive.clone(), outcome.clone());
            assert!(
                previous.is_none(),
                "{archive} has two recorded {replay} outcomes"
            );
        }
    }
    found
}

fn exit_code(status: Option<i32>) -> Value {
    status.map_or(Value::Null, |code| Value::Int(code.to_string()))
}

/// `bench-tools grading validate` on one archive's validator inputs and verdicts: its exit status and stdout.
fn validate(work: &Path, inputs: &[u8], verdicts: &[u8]) -> Value {
    std::fs::create_dir_all(work.join("validator")).unwrap();
    std::fs::write(work.join("validator/inputs.json"), inputs).unwrap();
    std::fs::write(work.join("verdicts.json"), verdicts).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_bench-tools"))
        .args([
            "grading",
            "validate",
            "verdicts.json",
            "--inputs",
            "validator/inputs.json",
        ])
        .current_dir(work)
        .env("LANG", "C.UTF-8")
        .output()
        .unwrap();
    o(vec![
        ("exit_code", exit_code(output.status.code())),
        ("stdout", s(&String::from_utf8_lossy(&output.stdout))),
    ])
}

#[test]
#[ignore = "needs the restored repository evidence and full history; the Pages build job runs it"]
fn the_validator_replays_every_restorable_archive_identically() {
    let root = repository();
    let value = inventory(&root);
    let archives = archives(&root);
    let members = manifest_members(&root);
    let recorded = expectations(&root, "validator");
    let scratch = Path::new(env!("CARGO_TARGET_TMPDIR")).join("validator-replay");
    let _ = std::fs::remove_dir_all(&scratch);
    let mut replays: BTreeMap<String, usize> = BTreeMap::new();
    let mut unrecorded = 0;
    let mut differences = Vec::new();
    for row in field(&value, &["receipts"])
        .and_then(Value::as_array)
        .unwrap()
    {
        if !field(row, &["restored"]).is_some_and(|restored| restored.py_eq(&Value::Bool(true))) {
            continue;
        }
        let evidence = text(row, &["archive"]).unwrap();
        let Some(expected) = recorded.get(evidence) else {
            unrecorded += 1;
            continue;
        };
        let archive = archives
            .iter()
            .find(|archive| archive.evidence == evidence)
            .unwrap();
        let [_, inputs, verdicts] = restore_bytes(&root, archive, &members).unwrap();
        let work = scratch.join(replays.values().sum::<usize>().to_string());
        let outcome = validate(&work, &inputs, &verdicts);
        if !outcome.py_eq(expected) {
            differences.push(format!(
                "{evidence}: recorded {}, validated {}",
                expected.py_repr(),
                outcome.py_repr()
            ));
        }
        std::fs::remove_dir_all(&work).unwrap();
        *replays
            .entry(text(row, &["contract"]).unwrap().to_string())
            .or_default() += 1;
    }
    for (contract, count) in &replays {
        println!("{contract}: {count} validator replays match the recorded outcome");
    }
    println!("{unrecorded} restorable archives have no recorded outcome in {EXPECTATIONS}");
    assert!(differences.is_empty(), "{differences:#?}");
    assert!(replays.values().sum::<usize>() > 0, "an empty replay fails");
}

/// The provenance `--assessor` gives each mapping replay, in place of the archived dispatch receipt.
const REPLAY_ASSESSOR: &str = r#"{"assessor": "replay fixture", "method": "the archived verdicts, replayed", "completed_at": "2026-10-20T00:00:00Z"}"#;
/// Workspace members a manual assessor's map refuses, which only the archived dispatch left behind.
const DISPATCH_ONLY: [&str; 2] = ["work/dispatch.json", "work/home/"];

/// A disposable detached worktree of one commit, removed on drop.
struct Worktree {
    root: PathBuf,
    path: PathBuf,
}

impl Worktree {
    fn add(root: &Path, path: &Path, commit: &str) -> Worktree {
        let tree = Worktree {
            root: root.to_path_buf(),
            path: path.to_path_buf(),
        };
        tree.remove();
        let added = git(
            root,
            &[
                "worktree",
                "add",
                "--quiet",
                "--detach",
                &path.to_string_lossy(),
                commit,
            ],
        );
        assert!(added.is_some(), "cannot add a worktree of {commit}");
        assert!(tree.written().is_empty(), "the worktree is not clean");
        tree
    }

    fn remove(&self) {
        if self.path.exists() {
            git(
                &self.root,
                &[
                    "worktree",
                    "remove",
                    "--force",
                    &self.path.to_string_lossy(),
                ],
            );
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    /// Each file a command added or changed, ignored files included, except the record lock that both
    /// implementations create before they read anything.
    fn written(&self) -> BTreeMap<String, Vec<u8>> {
        git(
            &self.path,
            &[
                "status",
                "--porcelain=v1",
                "--ignored",
                "--untracked-files=all",
            ],
        )
        .unwrap()
        .lines()
        .map(|line| line[3..].to_string())
        .filter(|name| name != RECORD_LOCK)
        .map(|name| {
            let bytes = std::fs::read(self.path.join(&name)).unwrap_or_default();
            (name, bytes)
        })
        .collect()
    }

    fn reset(&self) {
        assert!(git(&self.path, &["checkout", "--quiet", "--", "."]).is_some());
        assert!(git(&self.path, &["clean", "-fdxq"]).is_some());
    }
}

impl Drop for Worktree {
    fn drop(&mut self) {
        self.remove();
    }
}

struct Mapped {
    status: Option<i32>,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    written: BTreeMap<String, Vec<u8>>,
}

/// One implementation's `map` of an archive's workspace and key into a clean worktree, which it leaves clean again.
fn map_once(
    command: &mut Command,
    tree: &Worktree,
    scratch: &Path,
    restored: &BTreeMap<String, Vec<u8>>,
) -> Mapped {
    let work = scratch.join("work");
    let _ = std::fs::remove_dir_all(&work);
    for (name, bytes) in restored {
        if let Some(member) = name.strip_prefix("work/") {
            let path = work.join(member);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, bytes).unwrap();
        }
    }
    std::fs::write(scratch.join(KEY), &restored[KEY]).unwrap();
    std::fs::write(scratch.join("assessor.json"), REPLAY_ASSESSOR).unwrap();
    let output = command
        .arg("map")
        .arg("--root")
        .arg(&tree.path)
        .arg("--work")
        .arg(&work)
        .arg("--key")
        .arg(scratch.join(KEY))
        .arg("--assessor")
        .arg(scratch.join("assessor.json"))
        .env("LANG", "C.UTF-8")
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .output()
        .unwrap();
    let written = tree.written();
    if !written.is_empty() {
        tree.reset();
        assert!(tree.written().is_empty(), "the worktree is not clean");
    }
    Mapped {
        status: output.status.code(),
        stdout: output.stdout,
        stderr: output.stderr,
        written,
    }
}

/// `None` when the map ends as recorded, with nothing written. Its lines name the replay's scratch directory as
/// `<place>`.
fn mapping_difference(recorded: &Value, mapped: &Mapped, place: &Path) -> Option<String> {
    let place = place.to_string_lossy();
    let lines = |bytes: &[u8]| s(&String::from_utf8_lossy(bytes).replace(&*place, "<place>"));
    let outcome = o(vec![
        ("exit_code", exit_code(mapped.status)),
        ("stdout", lines(&mapped.stdout)),
        ("stderr", lines(&mapped.stderr)),
    ]);
    if !outcome.py_eq(recorded) {
        return Some(format!(
            "recorded {}, mapped {}",
            recorded.py_repr(),
            outcome.py_repr()
        ));
    }
    let written: Vec<&String> = mapped.written.keys().collect();
    (!written.is_empty()).then(|| format!("a refused map wrote {written:?}"))
}

#[derive(Default)]
struct MappingCounts {
    attempted: usize,
    refused: usize,
}

#[test]
#[ignore = "needs the restored repository evidence and full history; the Pages mapping replay job runs it"]
fn the_mapping_replays_every_restorable_archive_identically() {
    let root = repository();
    let value = inventory(&root);
    let archives = archives(&root);
    let members = manifest_members(&root);
    let recorded = expectations(&root, "mapping");
    assert!(
        recorded
            .values()
            .all(|outcome| !field(outcome, &["exit_code"]).unwrap().py_eq(&n(0))),
        "an accepted mapping outcome needs its records in {EXPECTATIONS}"
    );
    let scratch = Path::new(env!("CARGO_TARGET_TMPDIR")).join("mapping-replay");
    let mut by_commit: BTreeMap<&str, Vec<(&str, &str)>> = BTreeMap::new();
    let mut unrecorded = 0;
    for row in field(&value, &["receipts"])
        .and_then(Value::as_array)
        .unwrap()
    {
        if let (true, Some(commit)) = (
            field(row, &["restored"]).is_some_and(|restored| restored.py_eq(&Value::Bool(true))),
            text(row, &["filed_by"]),
        ) {
            let evidence = text(row, &["archive"]).unwrap();
            if !recorded.contains_key(evidence) {
                unrecorded += 1;
                continue;
            }
            by_commit
                .entry(commit)
                .or_default()
                .push((evidence, text(row, &["contract"]).unwrap()));
        }
    }
    let workers = std::thread::available_parallelism().map_or(1, |count| count.get().min(4));
    let mut counts: BTreeMap<String, MappingCounts> = BTreeMap::new();
    let mut differences = Vec::new();
    for (commit, rows) in &by_commit {
        let results: Vec<Vec<(&str, &str, Option<String>)>> = std::thread::scope(|scope| {
            let handles: Vec<_> = (0..workers)
                .map(|worker| {
                    let (root, archives, members, scratch, recorded) =
                        (&root, &archives, &members, &scratch, &recorded);
                    scope.spawn(move || {
                        let place = scratch.join(format!("{}-{worker}", &commit[..12]));
                        let tree = Worktree::add(root, &place.join("root"), commit);
                        rows.iter()
                            .skip(worker)
                            .step_by(workers)
                            .map(|&(evidence, contract)| {
                                let archive = archives
                                    .iter()
                                    .find(|archive| archive.evidence == evidence)
                                    .unwrap();
                                let restored = restore_members(root, archive, members, |name| {
                                    name == KEY
                                        || (name.starts_with("work/")
                                            && !DISPATCH_ONLY.iter().any(|skip| {
                                                name == *skip
                                                    || (skip.ends_with('/')
                                                        && name.starts_with(skip))
                                            }))
                                })
                                .unwrap();
                                let mapped = map_once(
                                    Command::new(env!("CARGO_BIN_EXE_bench-tools")).arg("grade"),
                                    &tree,
                                    &place,
                                    &restored,
                                );
                                let difference =
                                    mapping_difference(&recorded[evidence], &mapped, &place);
                                (evidence, contract, difference)
                            })
                            .collect()
                    })
                })
                .collect();
            handles
                .into_iter()
                .map(|handle| handle.join().unwrap())
                .collect()
        });
        for (evidence, contract, difference) in results.into_iter().flatten() {
            let count = counts.entry(contract.to_string()).or_default();
            count.attempted += 1;
            match difference {
                None => count.refused += 1,
                Some(difference) => {
                    differences.push(format!("{evidence} at {commit}: {difference}"))
                }
            }
        }
    }
    let _ = std::fs::remove_dir_all(&scratch);
    let held = field(&value, &["receipts_by_contract"])
        .and_then(Value::as_object)
        .unwrap();
    let corpus = std::fs::read_to_string(root.join(MAP_CORPUS)).unwrap();
    let mut gaps = Vec::new();
    for (contract, _) in held.iter() {
        let count = counts.remove(contract).unwrap_or_default();
        println!(
            "{contract}: {} mapping replays attempted, {} refused as recorded",
            count.attempted, count.refused
        );
        let case = fixture_case(contract);
        println!(
            "{contract}: no replay accepts; covered by the fixture case {} in {MAP_CORPUS}",
            case.unwrap_or("(none)")
        );
        if !case.is_some_and(|case| corpus.contains(&format!("\"id\": \"{case}\""))) {
            gaps.push(contract.clone());
        }
    }
    println!("{unrecorded} restorable archives have no recorded outcome in {EXPECTATIONS}");
    assert!(differences.is_empty(), "{differences:#?}");
    assert!(
        gaps.is_empty(),
        "contracts without a fixture case: {gaps:?}"
    );
    assert!(
        by_commit.values().map(Vec::len).sum::<usize>() > 0,
        "an empty replay fails"
    );
}
