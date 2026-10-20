//! The whole export's steps that a corpus cannot reach: the record read again after staging, a failed
//! publication, `SIGINT` during publication and the `BASE_PATH` the command reads. Each root is the initial tree of
//! one of `export-explorer.v1.json`'s whole-export cases.

use bench_tools::evidence;
use bench_tools::export_explorer::{self, Effects, ExportFailure, Moves};
use bench_tools::pyjson::{self, Duplicates, Value};
use nix::sys::signal::{SigSet, Signal};
use std::collections::BTreeMap;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::process::Command;

fn decode(text: &str) -> Vec<u8> {
    let value = |byte: u8| match byte {
        b'A'..=b'Z' => byte - b'A',
        b'a'..=b'z' => byte - b'a' + 26,
        b'0'..=b'9' => byte - b'0' + 52,
        b'+' => 62,
        b'/' => 63,
        _ => panic!("not base64: {byte}"),
    };
    let digits: Vec<u8> = text
        .bytes()
        .filter(|byte| *byte != b'=')
        .map(value)
        .collect();
    digits
        .chunks(4)
        .flat_map(|chunk| {
            let bits = chunk.iter().enumerate().fold(0u32, |bits, (index, digit)| {
                bits | (u32::from(*digit) << (18 - 6 * index))
            });
            bits.to_be_bytes()[1..chunk.len()].to_vec()
        })
        .collect()
}

/// The initial tree of a whole-export case, written under a new directory.
fn root(case: &str, label: &str) -> PathBuf {
    let corpus = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/behavior_contracts/export-explorer.v1.json");
    let corpus = pyjson::loads(
        &std::fs::read_to_string(corpus).unwrap(),
        Duplicates::Refuse,
    )
    .unwrap();
    let cases = corpus
        .as_object()
        .unwrap()
        .get("cases")
        .unwrap()
        .as_array()
        .unwrap();
    let case = cases
        .iter()
        .find(|each| each.as_object().unwrap().get("id").unwrap().as_str() == Some(case))
        .unwrap();
    let directory =
        std::env::temp_dir().join(format!("bench-tools-export-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir_all(&directory).unwrap();
    let directory = std::fs::canonicalize(&directory).unwrap();
    let field = |entry: &Value, name: &str| {
        entry
            .as_object()
            .unwrap()
            .get(name)
            .unwrap()
            .as_str()
            .unwrap()
            .to_string()
    };
    for entry in case
        .as_object()
        .unwrap()
        .get("initial")
        .unwrap()
        .as_array()
        .unwrap()
    {
        let path = directory.join(std::ffi::OsStr::from_bytes(&decode(&field(
            entry, "path_b64",
        ))));
        match field(entry, "kind").as_str() {
            "directory" => std::fs::create_dir_all(&path).unwrap(),
            "file" => {
                std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                std::fs::write(&path, decode(&field(entry, "bytes_b64"))).unwrap();
            }
            other => panic!("unexpected entry kind {other}"),
        }
    }
    directory
}

/// Every file under `directory`, by its path relative to it.
fn files(directory: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut found = BTreeMap::new();
    let mut pending = vec![directory.to_path_buf()];
    while let Some(next) = pending.pop() {
        for entry in std::fs::read_dir(&next).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else {
                found.insert(
                    path.strip_prefix(directory).unwrap().to_path_buf(),
                    std::fs::read(&path).unwrap(),
                );
            }
        }
    }
    found
}

/// Replace the root's record with the one the assessed case holds, a valid record with another dataset hash.
fn assess(root: &Path) {
    let label = format!("assessed-{}", root.file_name().unwrap().to_str().unwrap());
    let assessed = self::root("export-explorer-current-assessed", &label);
    for (path, bytes) in files(&assessed) {
        let target = root.join(path);
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(target, bytes).unwrap();
    }
    std::fs::remove_dir_all(assessed).unwrap();
}

fn export(
    root: &Path,
    moves: &mut dyn Moves,
    staged: &mut dyn FnMut(),
) -> Result<String, ExportFailure> {
    let mut store = evidence::Store {
        remote: &mut evidence::GitHub,
        disk: &mut evidence::System,
        now: &evidence::now_isoformat,
        temporary: evidence::system_temp(),
    };
    let result = export_explorer::export(
        root.as_os_str().as_bytes(),
        None,
        "/",
        Effects {
            store: &mut store,
            moves,
            staged,
        },
    );
    let mut interrupt = SigSet::empty();
    interrupt.add(Signal::SIGINT);
    interrupt.thread_unblock().unwrap();
    result
}

fn line(result: Result<String, ExportFailure>) -> String {
    match result {
        Ok(summary) => summary,
        Err(failure) => failure.line(),
    }
}

fn interrupts_blocked() -> bool {
    SigSet::thread_get_mask().unwrap().contains(Signal::SIGINT)
}

/// Fails the `fail`th replace and counts every replace.
struct Failing {
    replaces: usize,
    fail: Option<usize>,
    blocked: Vec<bool>,
}

impl Moves for Failing {
    fn replace(&mut self, from: &[u8], to: &[u8]) -> io::Result<()> {
        self.replaces += 1;
        self.blocked.push(interrupts_blocked());
        if self.fail == Some(self.replaces) {
            return Err(io::Error::from_raw_os_error(28));
        }
        std::fs::rename(
            std::ffi::OsStr::from_bytes(from),
            std::ffi::OsStr::from_bytes(to),
        )
    }
}

fn moves(fail: Option<usize>) -> Failing {
    Failing {
        replaces: 0,
        fail,
        blocked: Vec::new(),
    }
}

#[test]
fn a_record_changed_after_staging_is_refused_and_the_previous_export_stays() {
    let root = root("export-explorer-current-unassessed", "changed");
    assert!(
        line(export(&root, &mut moves(None), &mut || {}))
            .ends_with("0/1 admitted reviews assessed")
    );
    let before = files(&root.join("public"));
    let result = export(&root, &mut moves(None), &mut || assess(&root));
    assert_eq!(
        line(result),
        "current_grading.Inconsistent: current evidence changed during export"
    );
    assert_eq!(files(&root.join("public")), before);
    assert!(!root.join(".cache/explorer-export").exists());
    assert!(
        line(export(&root, &mut moves(None), &mut || {}))
            .ends_with("1/1 admitted reviews assessed")
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_failed_publication_restores_the_previous_export_and_removes_the_stage() {
    let root = root("export-explorer-current-unassessed", "publish");
    export(&root, &mut moves(None), &mut || {}).ok().unwrap();
    let before = files(&root.join("public"));
    assess(&root);
    let mut failing = moves(Some(3));
    let result = export(&root, &mut failing, &mut || {});
    assert!(line(result).starts_with("OSError: [Errno 28] No space left on device"));
    assert_eq!(files(&root.join("public")), before);
    assert!(!root.join(".cache/explorer-export").exists());
    assert!(
        line(export(&root, &mut moves(None), &mut || {}))
            .ends_with("1/1 admitted reviews assessed")
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn interrupts_are_blocked_from_publication_on_and_not_before() {
    let root = root("export-explorer-current-unassessed", "interrupts");
    let mut publishing = moves(None);
    let mut staged = None;
    export(&root, &mut publishing, &mut || {
        staged = Some(interrupts_blocked())
    })
    .ok()
    .unwrap();
    assert_eq!(staged, Some(false));
    assert_eq!(publishing.blocked, [true, true]);
    assert!(!interrupts_blocked());
    std::fs::remove_dir_all(root).unwrap();
}

fn command(root: &Path, base_path: Option<&std::ffi::OsStr>) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_bench-tools"));
    command
        .arg("export-explorer")
        .current_dir(root)
        .env_remove("BASE_PATH");
    if let Some(base_path) = base_path {
        command.env("BASE_PATH", base_path);
    }
    command.output().unwrap()
}

fn packet_url(root: &Path) -> String {
    let data = std::fs::read_to_string(root.join("public/data/benchmark.json")).unwrap();
    let data = pyjson::loads(&data, Duplicates::Refuse).unwrap();
    let tasks = data
        .as_object()
        .unwrap()
        .get("tasks")
        .unwrap()
        .as_array()
        .unwrap();
    tasks[0]
        .as_object()
        .unwrap()
        .get("packetUrl")
        .unwrap()
        .as_str()
        .unwrap()
        .to_string()
}

#[test]
fn the_command_reads_the_base_path_from_the_environment() {
    let root = root("export-explorer-current-unassessed", "base-path");
    for (base_path, url) in [
        (None, "/evidence/bench/targets/t-example/packet.md"),
        (Some("/"), "/evidence/bench/targets/t-example/packet.md"),
        (Some(""), "/evidence/bench/targets/t-example/packet.md"),
        (
            Some("/code-review-bench/"),
            "/code-review-bench/evidence/bench/targets/t-example/packet.md",
        ),
    ] {
        let output = command(&root, base_path.map(std::ffi::OsStr::new));
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(packet_url(&root), url, "{base_path:?}");
    }
    let before = files(&root.join("public"));
    let output = command(&root, Some(std::ffi::OsStr::from_bytes(b"/b\xff/")));
    assert_eq!(output.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .starts_with("error: invalid UTF-8 was detected in one or more arguments\n")
    );
    assert_eq!(files(&root.join("public")), before);
    std::fs::remove_dir_all(root).unwrap();
}
