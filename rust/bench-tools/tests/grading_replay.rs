//! Writes `docs/python-migration/grading-replay.v1.json`, which accounts for every assessment receipt in the current
//! record: the regrading archive that holds its key, validator inputs and verdicts, whether those bytes restore by
//! hash through the evidence manifests, its verdict contract and the commit that filed it. The Pages build job runs
//! it after restoring the repository evidence and fails when the committed file differs:
//!
//! ```sh
//! cargo test --locked --test grading_replay -- --ignored
//! git diff --exit-code -- docs/python-migration/grading-replay.v1.json
//! ```

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
const KEY: &str = "key.json";
const INPUTS: &str = "work/validator/inputs.json";
const VERDICTS: &str = "work/verdicts.json";
/// `grading_validation.CONTRACT`, which a validator input or policy without `contract` or `verdicts` uses.
/// `evidence.json` lists each member's hash but not its size, so the decoded stream is bounded by this instead. The
/// archive itself is checked against its manifest hash before it is read.
const MEMBER_BYTES_LIMIT: u64 = 4 * 1024 * 1024 * 1024;
const DEFAULT_CONTRACT: &str = "current-verdicts/v1";
/// Fields `grade.py map` copies from the key into the receipt, and the key field the receipt's
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

/// The key, validator inputs and verdicts of an archive, read by hash: the archive must be the manifest's member,
/// and each file the bytes `evidence.json` lists. `Err` names the first step that failed.
fn restore(
    root: &Path,
    archive: &Archive,
    members: &BTreeMap<String, String>,
) -> Result<[Value; 3], String> {
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
    let mut found: BTreeMap<&str, Vec<u8>> = BTreeMap::new();
    let limits = archive::Limits {
        members: archive.files.len() as u64,
        payload: MEMBER_BYTES_LIMIT,
    };
    archive::read_tar_gz(bytes.as_slice(), &limits, |member, data| {
        if let Some(name) = [KEY, INPUTS, VERDICTS]
            .into_iter()
            .find(|name| member.name == name.as_bytes())
        {
            let mut content = Vec::new();
            data.read_to_end(&mut content)?;
            found.insert(name, content);
        }
        Ok(())
    })
    .map_err(|error| format!("the archive cannot be read: {error}"))?;
    let mut values = Vec::new();
    for name in [KEY, INPUTS, VERDICTS] {
        let bytes = found
            .remove(name)
            .ok_or(format!("{name} is not in the archive"))?;
        if Some(&hex_digest(&bytes)) != archive.files.get(name) {
            return Err(format!("{name} differs from the hash evidence.json lists"));
        }
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
            o(vec![
                ("contract", s(contract)),
                ("receipts", n(*receipts)),
                (
                    "gap",
                    s("No receipt of this contract has a restorable archive, so no validator or mapping replay \
                       exists. #156 slice 8 covers it with a fixture case."),
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
