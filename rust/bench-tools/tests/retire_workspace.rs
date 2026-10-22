//! `bench-tools retire-workspace` exit codes and streams, for plans refused before any forge call. Every fixture
//! lives under the system temporary directory.

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};

struct Temp(PathBuf);

impl Temp {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "bench-tools-retire-workspace-{}-{name}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(path.join("work")).unwrap();
        fs::write(path.join("work/keep"), "kept").unwrap();
        Self(fs::canonicalize(&path).unwrap())
    }

    fn retire(&self, plan: &str, apply: bool) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_bench-tools"));
        command
            .arg("retire-workspace")
            .arg("--root")
            .arg(&self.0)
            .args(["--plan", plan, "--shared-commit", &"a".repeat(40)])
            .args(["--shared-branch", "main", "--receipt"])
            .arg(self.0.join("receipt.json"));
        if apply {
            command.arg("--apply");
        }
        command.output().unwrap()
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn an_open_plan_is_refused_on_stdout_with_exit_1() {
    let temp = Temp::new("open");
    fs::create_dir_all(temp.0.join("bench")).unwrap();
    fs::write(
        temp.0.join("bench/plan.json"),
        format!(
            r#"{{"schema_version": 1, "workspace": "{}", "kind": "review", "investigation": "open"}}"#,
            temp.0.join("work").display()
        ),
    )
    .unwrap();
    let output = temp.retire("bench/plan.json", true);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.starts_with("retirement refused: invalid closure: "),
        "{output:?}"
    );
    assert!(output.stderr.is_empty(), "{output:?}");
    assert!(temp.0.join("work/keep").exists());
    assert!(!temp.0.join("receipt.json").exists());
}

#[test]
fn a_missing_plan_fails_on_stderr_with_exit_2_and_a_path_outside_the_root_is_refused() {
    let temp = Temp::new("missing");
    let output = temp.retire("bench/plan.json", true);
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    assert!(
        String::from_utf8_lossy(&output.stderr).starts_with("retirement: "),
        "{output:?}"
    );
    let outside = temp.retire("bench/../plan.json", false);
    assert_eq!(outside.status.code(), Some(1), "{outside:?}");
    assert!(temp.0.join("work/keep").exists());
}
