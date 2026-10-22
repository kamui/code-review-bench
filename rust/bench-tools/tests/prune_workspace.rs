//! `bench-tools prune-workspace` against temporary grading workspaces and attempt directories. Every fixture lives
//! under the system temporary directory.

use bench_tools::sha256;
use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

struct Temp(PathBuf);

impl Temp {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "bench-tools-prune-workspace-{}-{name}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        Self(fs::canonicalize(&path).unwrap())
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn bench_tools(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_bench-tools"))
        .arg("prune-workspace")
        .args(arguments)
        .output()
        .unwrap()
}

fn git(clone: &Path, arguments: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(clone)
        .args([
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
        ])
        .args(arguments)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    String::from_utf8(output.stdout).unwrap().trim().to_string()
}

/// A mapped grading workspace whose dispatched session exited cleanly and priced.
struct Grading {
    temp: Temp,
}

impl Grading {
    fn new(name: &str) -> Self {
        let temp = Temp::new(name);
        let work = temp.0.join("work");
        let clone = work.join("clone");
        fs::create_dir_all(&clone).unwrap();
        git(&clone, &["init", "-q"]);
        git(&clone, &["commit", "--allow-empty", "-qm", "fixture"]);
        let head = git(&clone, &["rev-parse", "HEAD"]);
        for name in ["clone-cache", "clone-work", "home"] {
            fs::create_dir_all(work.join(name)).unwrap();
            fs::write(work.join(name).join("keep"), name).unwrap();
        }
        let verdicts = b"{\"reviews\": {}}";
        fs::write(work.join("verdicts.json"), verdicts).unwrap();
        fs::write(
            work.join("dispatch.json"),
            r#"{"session_id": "session-a", "exit_code": 0, "verdicts_present": true, "audit_violations": [], "usage": {"priced_total_usd": 1.5}}"#,
        )
        .unwrap();
        fs::create_dir_all(temp.0.join("target")).unwrap();
        fs::write(
            temp.0.join("target/target.json"),
            format!("{{\"head\": \"{head}\"}}"),
        )
        .unwrap();
        fs::write(
            temp.0.join("receipt.json"),
            format!(
                r#"{{"provenance": {{"kind": "dispatch", "session_id": "session-a"}}, "verdicts_sha256": "{}"}}"#,
                sha256::hex_digest(verdicts)
            ),
        )
        .unwrap();
        Self { temp }
    }

    fn path(&self, name: &str) -> String {
        self.temp.0.join(name).to_str().unwrap().to_string()
    }

    fn prune(&self, apply: bool) -> Output {
        let mut arguments = vec![
            "--grading-work".to_string(),
            self.path("work"),
            "--target".to_string(),
            self.path("target"),
            "--receipt".to_string(),
            self.path("receipt.json"),
        ];
        if apply {
            arguments.push("--apply".to_string());
        }
        bench_tools(&arguments.iter().map(String::as_str).collect::<Vec<_>>())
    }
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn a_grading_workspace_is_previewed_then_pruned_and_a_retry_reprints_its_receipt() {
    let grading = Grading::new("grading");
    let work = grading.temp.0.join("work");
    let preview = grading.prune(false);
    assert_eq!(preview.status.code(), Some(0), "{preview:?}");
    assert!(stdout(&preview).contains("\"applied\": false"));
    assert!(work.join("clone").exists() && work.join("clone-cache").exists());
    assert!(!work.join("workspace-pruned.json").exists());

    let applied = grading.prune(true);
    assert_eq!(applied.status.code(), Some(0), "{applied:?}");
    assert!(stdout(&applied).contains("\"applied\": true"));
    assert!(!work.join("clone").exists() && !work.join("clone-cache").exists());
    for name in ["clone-work", "home"] {
        assert_eq!(
            fs::read_to_string(work.join(name).join("keep")).unwrap(),
            name
        );
    }
    let receipt = fs::read(work.join("workspace-pruned.json")).unwrap();

    let again = grading.prune(true);
    assert_eq!(again.status.code(), Some(0), "{again:?}");
    assert_eq!(
        stdout(&again).trim_end(),
        String::from_utf8(receipt.clone()).unwrap().trim_end()
    );
    assert_eq!(
        fs::read(work.join("workspace-pruned.json")).unwrap(),
        receipt
    );
}

#[test]
fn a_refusal_exits_1_and_an_input_error_2_on_stdout_and_nothing_goes() {
    let grading = Grading::new("refusal");
    let work = grading.temp.0.join("work");
    fs::remove_dir_all(work.join("clone-cache")).unwrap();
    symlink(work.join("clone-work"), work.join("clone-cache")).unwrap();
    let refused = grading.prune(true);
    assert_eq!(refused.status.code(), Some(1), "{refused:?}");
    assert!(
        stdout(&refused).starts_with("cleanup refused: "),
        "{refused:?}"
    );
    assert!(work.join("clone").exists() && work.join("clone-work/keep").exists());

    fs::remove_file(grading.temp.0.join("target/target.json")).unwrap();
    let failed = grading.prune(true);
    assert_eq!(failed.status.code(), Some(2), "{failed:?}");
    assert!(
        stdout(&failed).starts_with("cleanup failed: "),
        "{failed:?}"
    );
    assert!(work.join("clone").exists());

    let attempt = grading.temp.0.join("attempt");
    fs::create_dir_all(&attempt).unwrap();
    let unfiled = bench_tools(&[
        "--attempt",
        attempt.to_str().unwrap(),
        "--workspace",
        work.to_str().unwrap(),
        "--apply",
    ]);
    assert_ne!(unfiled.status.code(), Some(0), "{unfiled:?}");
    assert!(stdout(&unfiled).starts_with("cleanup "), "{unfiled:?}");
    assert!(work.join("clone").exists());
}

#[test]
fn mixed_or_missing_arguments_are_misuse() {
    for arguments in [
        &[][..],
        &["--attempt", "a"][..],
        &["--attempt", "a", "--workspace", "w", "--receipt", "r"][..],
        &["--attempt", "a", "--workspace", "w", "--grading-work", "g"][..],
        &["--grading-work", "g", "--target", "t"][..],
        &[
            "--grading-work",
            "g",
            "--target",
            "t",
            "--receipt",
            "r",
            "--workspace",
            "w",
        ][..],
    ] {
        let output = bench_tools(arguments);
        assert_eq!(output.status.code(), Some(2), "{arguments:?}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("give --attempt with --workspace"),
            "{arguments:?}: {output:?}"
        );
        assert_eq!(output.stdout, [] as [u8; 0]);
    }
}
