//! Runs every behavior-contract corpus in `tools/behavior_contracts` against the built bench-tools, one disposable
//! root per case, plus the published decision history, the canonical digest vectors and the runner's own checks.
//!
//! Arguments follow the libtest conventions cargo passes through: substring filters, `--exact`, `--skip`,
//! `--list`, `--test-threads`. A test is named `CORPUS::CASE`, where CORPUS is the corpus file stem without its
//! version.

mod b64;
mod compare;
mod corpus;
mod execute;
#[path = "../support/fixture_git.rs"]
mod fixture_git;
mod fixture_json;
mod gh_stub;
mod history;
mod json;
mod normalize;
mod pattern;
mod selfcheck;
mod structured;
mod writers;

use bench_tools::pyjson::Value;
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fmt::Write as _;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

pub enum Outcome {
    Pass,
    Fail(String),
    Skip(String),
}

type Run<'a> = Box<dyn Fn() -> Outcome + Send + Sync + 'a>;

struct Test<'a> {
    name: String,
    run: Run<'a>,
}

/// Corpora whose programs refuse a root below a symlink, which every macOS temporary directory is.
const LINUX_ONLY: [&str; 3] = ["evidence-store", "evidence-inventory", "native-artifacts"];

#[derive(Default)]
struct Arguments {
    filters: Vec<String>,
    skip: Vec<String>,
    exact: bool,
    list: bool,
    ignored: bool,
    threads: Option<usize>,
}

fn arguments(args: &[OsString]) -> Result<Arguments, String> {
    let mut parsed = Arguments::default();
    let mut words = args.iter().map(|arg| arg.to_string_lossy().into_owned());
    while let Some(word) = words.next() {
        let mut value = |flag: &str| words.next().ok_or_else(|| format!("{flag} needs a value"));
        match word.as_str() {
            "--exact" => parsed.exact = true,
            "--list" => parsed.list = true,
            "--ignored" => parsed.ignored = true,
            "--skip" => parsed.skip.push(value("--skip")?),
            "--test-threads" => {
                parsed.threads = Some(
                    value("--test-threads")?
                        .parse()
                        .map_err(|error| format!("--test-threads: {error}"))?,
                );
            }
            "--color" | "--format" | "--logfile" | "-Z" => {
                value(&word)?;
            }
            flag if flag.starts_with('-') => {}
            filter => parsed.filters.push(filter.to_string()),
        }
    }
    Ok(parsed)
}

impl Arguments {
    fn hits(&self, filter: &str, name: &str) -> bool {
        if self.exact {
            name == filter
        } else {
            name.contains(filter)
        }
    }

    fn selects(&self, name: &str) -> bool {
        (self.filters.is_empty() || self.filters.iter().any(|filter| self.hits(filter, name)))
            && !self.skip.iter().any(|filter| self.hits(filter, name))
    }

    /// Filters that select none of `names`. Cargo passes one filter to every test binary, so a filter that could
    /// name a Rust test, such as `provision` or `grade::tests`, may rightly select nothing here; only a filter that
    /// no Rust test path can match, such as one with a `-`, is a mistake.
    fn unmatched<'a>(&'a self, names: &[&str]) -> Vec<&'a str> {
        self.filters
            .iter()
            .filter(|filter| {
                !filter
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b':')
                    && !names.iter().any(|name| self.hits(filter, name))
            })
            .map(String::as_str)
            .collect()
    }
}

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Every corpus file, sorted, and the name its tests carry.
fn corpora(directory: &Path) -> Result<Vec<(String, PathBuf)>, String> {
    let mut found = Vec::new();
    for entry in
        std::fs::read_dir(directory).map_err(|error| format!("{}: {error}", directory.display()))?
    {
        let path = entry.map_err(|error| error.to_string())?.path();
        let Some(file) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let Some(stem) = file.strip_suffix(".json") else {
            continue;
        };
        if stem == "digest-vectors" || stem.contains("-decisions.") {
            continue;
        }
        let name = match stem.rsplit_once(".v") {
            Some((name, version))
                if !version.is_empty() && version.bytes().all(|byte| byte.is_ascii_digit()) =>
            {
                name
            }
            _ => stem,
        };
        found.push((name.to_string(), path));
    }
    found.sort();
    Ok(found)
}

fn case_outcome(case: &Value, corpus: &corpus::Corpus, scratch: &execute::Scratch) -> Outcome {
    match execute::execute(case, corpus, scratch, &[]) {
        Err(execute::Failure::Unavailable(reason)) => Outcome::Skip(reason),
        Err(execute::Failure::Error(error)) => Outcome::Fail(error),
        Ok(observed) => match compare::compare_case(case, &observed) {
            Ok(differences) if differences.is_empty() => Outcome::Pass,
            Ok(differences) => Outcome::Fail(differences.join("\n")),
            Err(error) => Outcome::Fail(error),
        },
    }
}

fn digest_vectors(directory: &Path) -> Result<(), String> {
    let int = |digits: &str| Value::Int(digits.to_string());
    let values = [
        Value::Null,
        Value::Bool(true),
        int("1"),
        Value::Float(1.0),
        Value::Float(4e-5),
        int("9007199254740993"),
        Value::Float(-0.0),
        Value::Str("雪😀".to_string()),
        Value::Object(
            [
                (
                    "雪".to_string(),
                    Value::Array(vec![Value::Bool(true), Value::Null, Value::Float(10.0)]),
                ),
                ("a".to_string(), int("1")),
            ]
            .into_iter()
            .collect(),
        ),
    ];
    let path = directory.join("digest-vectors.json");
    let raw = std::fs::read(&path).map_err(|error| format!("{}: {error}", path.display()))?;
    let vectors = json::value(&raw)?;
    let vectors = vectors.as_array().ok_or("digest vectors are not a list")?;
    if vectors.len() != values.len() {
        return Err(format!(
            "{} digest vectors for {} values",
            vectors.len(),
            values.len()
        ));
    }
    for (vector, value) in vectors.iter().zip(&values) {
        let name = corpus::string(vector, "name")?;
        let mut bytes = Vec::new();
        json::canonical(value, &mut bytes)?;
        let hex: String = bytes.iter().fold(String::new(), |mut out, byte| {
            let _ = write!(out, "{byte:02x}");
            out
        });
        if hex != corpus::string(vector, "canonical_hex")? {
            return Err(format!("{name}: canonical bytes {hex}"));
        }
        if json::digest(value)? != corpus::string(vector, "sha256")? {
            return Err(format!("{name}: digest differs"));
        }
    }
    for refused in [Value::Float(f64::INFINITY), Value::Float(f64::NAN)] {
        if json::canonical(&refused, &mut Vec::new()).is_ok() {
            return Err(format!("{} has a canonical form", refused.py_repr()));
        }
    }
    Ok(())
}

fn self_role(args: &[OsString]) -> Option<i32> {
    let (role, rest) = args.split_first()?;
    Some(match role.to_str()? {
        "--contract-gh-stub" => gh_stub::role(rest),
        "--contract-file-size-limit" => execute::file_size_limit_role(rest),
        "--contract-fail-write-probe" => execute::fail_write_probe_role(rest),
        "--contract-writer" => writers::role(rest),
        "--contract-json" => fixture_json::role(rest),
        _ => return None,
    })
}

fn main() -> ExitCode {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    if let Some(code) = self_role(&args) {
        return ExitCode::from(u8::try_from(code).unwrap_or(1));
    }
    nix::sys::stat::umask(nix::sys::stat::Mode::from_bits_truncate(0o022));
    let parsed = match arguments(&args) {
        Ok(parsed) => parsed,
        Err(error) => {
            eprintln!("behavior_contracts: {error}");
            return ExitCode::from(2);
        }
    };
    let scratch = match execute::Scratch::create() {
        Ok(scratch) => scratch,
        Err(error) => {
            eprintln!("behavior_contracts: {error}");
            return ExitCode::from(2);
        }
    };
    let repo = repository();
    let directory = repo.join("tools/behavior_contracts");
    let found = match corpora(&directory) {
        Ok(found) => found,
        Err(error) => {
            eprintln!("behavior_contracts: {error}");
            return ExitCode::from(2);
        }
    };
    let loaded: Vec<(String, Result<corpus::Corpus, String>)> = found
        .into_iter()
        .map(|(name, path)| {
            let corpus =
                corpus::load(&path).map_err(|error| format!("{}: {error}", path.display()));
            (name, corpus)
        })
        .collect();
    let mut tests: Vec<Test> = vec![
        Test {
            name: "decision-history".to_string(),
            run: Box::new(|| match history::verify(&repo) {
                Ok(()) => Outcome::Pass,
                Err(error) => Outcome::Fail(error),
            }),
        },
        Test {
            name: "digest-vectors".to_string(),
            run: Box::new(|| match digest_vectors(&directory) {
                Ok(()) => Outcome::Pass,
                Err(error) => Outcome::Fail(error),
            }),
        },
    ];
    for (name, check) in selfcheck::checks() {
        let scratch = &scratch;
        tests.push(Test {
            name: format!("self-check::{name}"),
            run: Box::new(move || match check(scratch) {
                Ok(()) => Outcome::Pass,
                Err(execute::Failure::Unavailable(reason)) => Outcome::Skip(reason),
                Err(execute::Failure::Error(error)) => Outcome::Fail(error),
            }),
        });
    }
    for (name, corpus) in &loaded {
        match corpus {
            Err(error) => tests.push(Test {
                name: format!("{name}::(load)"),
                run: Box::new(move || Outcome::Fail(error.clone())),
            }),
            Ok(corpus) => {
                for case in &corpus.cases {
                    let id = corpus::string(case, "id").unwrap_or("(no id)");
                    let unsupported =
                        LINUX_ONLY.contains(&name.as_str()) && !cfg!(target_os = "linux");
                    let scratch = &scratch;
                    tests.push(Test {
                        name: format!("{name}::{id}"),
                        run: Box::new(move || {
                            if unsupported {
                                Outcome::Skip(format!(
                                    "{name} needs a temporary directory with no symlink above it"
                                ))
                            } else {
                                case_outcome(case, corpus, scratch)
                            }
                        }),
                    });
                }
            }
        }
    }
    let names: Vec<&str> = tests.iter().map(|test| test.name.as_str()).collect();
    let unmatched = parsed.unmatched(&names);
    if !unmatched.is_empty() {
        eprintln!(
            "behavior_contracts: no test matches {}",
            unmatched.join(", ")
        );
        return ExitCode::from(2);
    }
    tests.retain(|test| parsed.selects(&test.name));
    if parsed.list {
        for test in &tests {
            println!("{}: test", test.name);
        }
        return ExitCode::SUCCESS;
    }
    if parsed.ignored {
        println!("0 ignored tests");
        return ExitCode::SUCCESS;
    }
    run(&tests, parsed.threads)
}

fn run(tests: &[Test], threads: Option<usize>) -> ExitCode {
    let threads = threads
        .or_else(|| {
            std::env::var("RUST_TEST_THREADS")
                .ok()
                .and_then(|value| value.parse().ok())
        })
        .or_else(|| std::thread::available_parallelism().ok().map(usize::from))
        .unwrap_or(1)
        .max(1);
    let next = AtomicUsize::new(0);
    let counts: Mutex<BTreeMap<String, [usize; 3]>> = Mutex::new(BTreeMap::new());
    let output = Mutex::new(std::io::stdout());
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| {
                loop {
                    let index = next.fetch_add(1, Ordering::Relaxed);
                    let Some(test) = tests.get(index) else {
                        break;
                    };
                    let outcome =
                        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| (test.run)()))
                            .unwrap_or_else(|_| Outcome::Fail("the runner panicked".to_string()));
                    let group = test.name.split("::").next().unwrap_or_default().to_string();
                    let (slot, line) = match outcome {
                        Outcome::Pass => (0, format!("PASS {}\n", test.name)),
                        Outcome::Fail(differences) => {
                            (1, format!("FAIL {}\n{differences}\n", test.name))
                        }
                        Outcome::Skip(reason) => {
                            (2, format!("SKIP UNAVAILABLE {}: {reason}\n", test.name))
                        }
                    };
                    if let Ok(mut counts) = counts.lock() {
                        counts.entry(group).or_default()[slot] += 1;
                    }
                    if let Ok(mut output) = output.lock() {
                        let _ = output.write_all(line.as_bytes());
                    }
                }
            });
        }
    });
    let counts = counts.into_inner().unwrap_or_default();
    let mut total = [0; 3];
    println!();
    for (group, [passed, failed, skipped]) in &counts {
        println!("{group}: {passed} passed, {failed} failed, {skipped} skipped");
        total[0] += passed;
        total[1] += failed;
        total[2] += skipped;
    }
    println!(
        "total: {} passed, {} failed, {} skipped",
        total[0], total[1], total[2]
    );
    if total[1] == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
