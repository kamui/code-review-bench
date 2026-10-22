//! `bench-tools regrade run` before its queue: the authorization's pinned runtime, the hand-over to it from
//! another installed runtime, and the queue's controller lock.

#[path = "support/runtime_store.rs"]
mod runtime_store;

use bench_tools::sha256::hex_digest;
use runtime_store::{built, receipt};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// A store holding this build and a recording stand-in, with an empty queue directory.
struct Bench {
    root: PathBuf,
    tools: PathBuf,
    stand_in: PathBuf,
}

impl Drop for Bench {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// The store entry for `bytes`, with its receipt; the pin `rust_runtime.py pin` prints for it.
fn install(store: &Path, source: Option<&Path>, bytes: &[u8]) -> (PathBuf, String) {
    let sha = hex_digest(bytes);
    let entry = store.join("bench-tools").join(&sha);
    fs::create_dir_all(&entry).unwrap();
    let executable = entry.join("bench-tools");
    if source.is_none_or(|source| fs::hard_link(source, &executable).is_err()) {
        fs::write(&executable, bytes).unwrap();
    }
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
    let receipt = receipt(&sha, bytes.len() as u64);
    fs::write(entry.join("receipt.json"), &receipt).unwrap();
    let pin = format!(
        r#"{{"sha256": "{sha}", "receipt_sha256": "{}"}}"#,
        hex_digest(receipt.as_bytes())
    );
    (executable, pin)
}

const STAND_IN: &str = r#"#!/bin/sh
printf '%s\n' "$BENCH_REGRADE_PINNED_RUNTIME" "$@" > "$(dirname "$0")/started"
exit 7
"#;

fn bench(label: &str) -> (Bench, String, String) {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("regrade-run-{label}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    let root = fs::canonicalize(&root).unwrap();
    let (source, _) = built();
    let store = root.join("store");
    let (tools, own) = install(&store, Some(source), &fs::read(source).unwrap());
    let (stand_in, other) = install(&store, None, STAND_IN.as_bytes());
    (
        Bench {
            root,
            tools,
            stand_in,
        },
        own,
        other,
    )
}

fn output_text(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

impl Bench {
    fn authorize(&self, runtimes: Option<&str>) -> PathBuf {
        let path = self.root.join("authorization.json");
        let pinned = runtimes.map_or(String::new(), |pin| {
            format!(r#""runtimes": {{"bench-tools": {pin}}}, "#)
        });
        fs::write(&path, format!(r#"{{{pinned}"budgetCapUsd": 5}}"#)).unwrap();
        path
    }

    fn regrade(&self, executable: &Path, authorization: &Path, extra: &[(&str, &str)]) -> Output {
        let mut command = Command::new(executable);
        command
            .args(["regrade", "run", "--authorization"])
            .arg(authorization)
            .arg("--directory")
            .arg(self.root.join("queue"))
            .arg("--root")
            .arg(&self.root)
            .env_remove("BENCH_REGRADE_PINNED_RUNTIME");
        for (name, value) in extra {
            command.env(name, value);
        }
        command.output().unwrap()
    }
}

fn stopped(output: &Output) -> String {
    assert_eq!(output.status.code(), Some(2), "{}", output_text(output));
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    stderr
        .strip_prefix("regrading stopped: ")
        .unwrap_or_else(|| panic!("{stderr}"))
        .trim_end()
        .to_string()
}

#[test]
fn an_authorization_without_a_runtime_pin_is_refused() {
    let (bench, _, _) = bench("unpinned");
    let refused = stopped(&bench.regrade(&bench.tools, &bench.authorize(None), &[]));
    assert!(
        refused.starts_with("authorization pins no bench-tools runtime"),
        "{refused}"
    );
    assert!(!bench.root.join("queue").exists());
}

#[test]
fn a_runtime_outside_its_store_is_refused() {
    let (bench, own, _) = bench("unstored");
    let refused = stopped(&bench.regrade(&built().0, &bench.authorize(Some(&own)), &[]));
    assert!(
        refused.starts_with("the authorization's pinned bench-tools is unavailable: "),
        "{refused}"
    );
}

#[test]
fn another_installed_runtime_hands_the_run_to_the_pinned_one() {
    let (bench, _, other) = bench("handover");
    let authorization = bench.authorize(Some(&other));
    let done = bench.regrade(&bench.tools, &authorization, &[]);
    assert_eq!(done.status.code(), Some(7), "{}", output_text(&done));
    let started = fs::read_to_string(bench.stand_in.with_file_name("started")).unwrap();
    let mut expected = vec!["1", "regrade", "run", "--authorization"];
    let (authorization, queue, root) = (
        authorization.display().to_string(),
        bench.root.join("queue").display().to_string(),
        bench.root.display().to_string(),
    );
    expected.extend([
        authorization.as_str(),
        "--directory",
        &queue,
        "--root",
        &root,
    ]);
    assert_eq!(started.lines().collect::<Vec<_>>(), expected);
    assert!(!bench.root.join("queue").exists());

    let looped = bench.regrade(
        &bench.tools,
        &bench.root.join("authorization.json"),
        &[("BENCH_REGRADE_PINNED_RUNTIME", "1")],
    );
    assert!(stopped(&looped).contains("did not start as itself"));
}

#[test]
fn a_second_controller_is_locked_out_of_the_queue() {
    let (bench, own, _) = bench("locked");
    let authorization = bench.authorize(Some(&own));
    let first = stopped(&bench.regrade(&bench.tools, &authorization, &[]));
    assert_eq!(first, "missing field 'sourcePlan'");
    let lock = fs::OpenOptions::new()
        .append(true)
        .open(bench.root.join("queue/controller.lock"))
        .unwrap();
    let _held = nix::fcntl::Flock::lock(lock, nix::fcntl::FlockArg::LockExclusiveNonblock).unwrap();
    let second = stopped(&bench.regrade(&bench.tools, &authorization, &[]));
    assert!(second.starts_with("another controller holds "), "{second}");
}
