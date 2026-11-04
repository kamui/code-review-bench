//! `bench-tools evidence pinned` on a small repository layout: the pins it reads, the published hashes it compares
//! them with, and the exceptions file. No network is used.

use bench_tools::evidence::pinned::{self, EXCEPTIONS};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const ARCHIVE: &str = "1111111111111111111111111111111111111111111111111111111111111111";
const REPLACEMENT: &str = "2222222222222222222222222222222222222222222222222222222222222222";
const SUPPLEMENT: &str = "3333333333333333333333333333333333333333333333333333333333333333";
const PACKAGE: &str = "4444444444444444444444444444444444444444444444444444444444444444";

fn write(root: &Path, name: &str, text: &str) {
    let path = root.join(name);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

/// A repository pinning `ARCHIVE` (target), `REPLACEMENT` (replacement manifest) and `SUPPLEMENT` (policy), whose
/// only published member is `published`.
fn repository(label: &str, published: &str, exceptions: &str) -> PathBuf {
    let root =
        std::env::temp_dir().join(format!("bench-tools-pinned-{label}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    write(
        &root,
        "bench/targets/t-one/target.json",
        &format!(
            r#"{{"id": "t-one", "provisioning": {{"dependency_identity": [
              {{"name": "setup.py", "sha256": "{}", "location": "head:setup.py"}},
              {{"name": "cache archive (npm)", "sha256": "{ARCHIVE}", "location": "archives/t-one.tar.gz"}}]}}}}"#,
            "9".repeat(64)
        ),
    );
    write(
        &root,
        "docs/research/rebuild/cache-replacements.v1.json",
        &format!(
            r#"{{"schema_version": 1, "version": 1, "reason": "r", "targets": [{{"target": "t-one", "sha256": "{REPLACEMENT}"}}]}}"#
        ),
    );
    write(
        &root,
        "bench/policies/grading-commands.v1.json",
        &format!(
            r#"{{"version": 1, "targets": {{"t-one": {{"dependency_supplement": {{"path": "x", "sha256": "{SUPPLEMENT}"}}}}, "t-two": {{}}}}}}"#
        ),
    );
    write(
        &root,
        "bench/evidence/manifests/m-v1.json",
        &format!(
            r#"{{"schema_version": 1, "repository": "o/r", "tag": "evidence-test-v1", "subjects": ["s"], "packages": [{{
              "asset": "{PACKAGE}.tar.gz", "sha256": "{PACKAGE}", "bytes": 10,
              "members": [{{"path": "artifacts/reproducibility/archives/t-one.tar.gz", "sha256": "{published}", "bytes": 1, "mode": 420, "kind": "reproducibility"}}],
              "publication": {{"release_id": 1, "asset_id": 2, "sha256": "{PACKAGE}", "verified_at": "2026-11-01T00:00:00+00:00"}}}}]}}"#
        ),
    );
    write(&root, EXCEPTIONS, exceptions);
    root
}

fn exception(file: &str, sha256: &str) -> String {
    format!(r#"{{"file": "{file}", "sha256": "{sha256}", "reason": "Lost with its host."}}"#)
}

fn exceptions(entries: &[String]) -> String {
    format!(
        r#"{{"schema_version": 1, "unpublished": [{}]}}"#,
        entries.join(", ")
    )
}

const TARGET: &str = "bench/targets/t-one/target.json";
const MANIFEST: &str = "docs/research/rebuild/cache-replacements.v1.json";
const POLICY: &str = "bench/policies/grading-commands.v1.json";

#[test]
fn it_reads_every_kind_of_pin_and_ignores_other_dependency_identities() {
    let root = repository("pins", ARCHIVE, &exceptions(&[]));
    let found: Vec<(String, String)> = pinned::pins(&root)
        .unwrap()
        .into_iter()
        .map(|pin| (pin.file, pin.sha256))
        .collect();
    assert_eq!(
        found,
        [
            (POLICY, SUPPLEMENT),
            (TARGET, ARCHIVE),
            (MANIFEST, REPLACEMENT)
        ]
        .map(|(file, sha)| (file.to_string(), sha.to_string()))
    );
}

#[test]
fn it_reports_each_unpublished_pin_with_its_file() {
    let root = repository("unpublished", ARCHIVE, &exceptions(&[]));
    let report = pinned::check(&root).unwrap();
    assert_eq!(
        report.problems,
        [
            format!(
                "unpublished: {SUPPLEMENT} pinned by {POLICY}; publish it, or list it in {EXCEPTIONS} with a reason"
            ),
            format!(
                "unpublished: {REPLACEMENT} pinned by {MANIFEST}; publish it, or list it in {EXCEPTIONS} with a reason"
            ),
        ]
    );
}

#[test]
fn a_listed_exception_passes() {
    let listed = exceptions(&[
        exception(POLICY, SUPPLEMENT),
        exception(MANIFEST, REPLACEMENT),
    ]);
    let root = repository("listed", ARCHIVE, &listed);
    let report = pinned::check(&root).unwrap();
    assert_eq!(report.problems, Vec::<String>::new());
    assert_eq!(
        report.summary,
        format!("pinned: 3 hashes in 3 files; 2 pins unpublished, 2 listed in {EXCEPTIONS}")
    );
}

#[test]
fn an_exception_belongs_to_the_file_that_pins_the_hash() {
    let listed = exceptions(&[
        exception(TARGET, SUPPLEMENT),
        exception(MANIFEST, REPLACEMENT),
    ]);
    let root = repository("other-file", ARCHIVE, &listed);
    let problems = pinned::check(&root).unwrap().problems;
    assert_eq!(problems.len(), 2, "{problems:?}");
    assert!(problems[0].starts_with(&format!("unpublished: {SUPPLEMENT} pinned by {POLICY}")));
    assert_eq!(
        problems[1],
        format!(
            "stale exception: {SUPPLEMENT} for {TARGET} is no longer pinned there; remove it from {EXCEPTIONS}"
        )
    );
}

#[test]
fn an_exception_for_a_published_hash_is_stale() {
    let listed = exceptions(&[
        exception(POLICY, SUPPLEMENT),
        exception(MANIFEST, REPLACEMENT),
        exception(TARGET, ARCHIVE),
    ]);
    let root = repository("stale", ARCHIVE, &listed);
    assert_eq!(
        pinned::check(&root).unwrap().problems,
        [format!(
            "stale exception: {ARCHIVE} for {TARGET} is now published; remove it from {EXCEPTIONS}"
        )]
    );
}

#[test]
fn a_published_hash_needs_no_exception() {
    let root = repository(
        "published",
        REPLACEMENT,
        &exceptions(&[exception(POLICY, SUPPLEMENT)]),
    );
    let problems = pinned::check(&root).unwrap().problems;
    assert_eq!(
        problems,
        [format!(
            "unpublished: {ARCHIVE} pinned by {TARGET}; publish it, or list it in {EXCEPTIONS} with a reason"
        )]
    );
}

#[test]
fn a_malformed_exceptions_file_is_refused() {
    let entry = format!(r#"{{"file": "{TARGET}", "sha256": "{ARCHIVE}"}}"#);
    let root = repository("no-reason", ARCHIVE, &exceptions(&[entry]));
    assert!(pinned::check(&root).is_err());
    let twice = exceptions(&[exception(TARGET, ARCHIVE), exception(TARGET, ARCHIVE)]);
    let root = repository("twice", ARCHIVE, &twice);
    assert!(
        pinned::check(&root)
            .unwrap_err()
            .message()
            .contains("duplicate entry")
    );
}

#[test]
fn the_command_exits_one_for_a_problem_and_zero_for_none() {
    let run = |root: &Path| {
        Command::new(env!("CARGO_BIN_EXE_bench-tools"))
            .args(["evidence", "pinned", "--root"])
            .arg(root)
            .output()
            .unwrap()
    };
    let failing = run(&repository("cli-fail", ARCHIVE, &exceptions(&[])));
    assert_eq!(failing.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&failing.stdout).contains(&format!("unpublished: {REPLACEMENT}"))
    );
    let listed = exceptions(&[
        exception(POLICY, SUPPLEMENT),
        exception(MANIFEST, REPLACEMENT),
    ]);
    let passing = run(&repository("cli-pass", ARCHIVE, &listed));
    assert_eq!(
        passing.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&passing.stdout)
    );
}
