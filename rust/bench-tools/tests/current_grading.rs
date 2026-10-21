//! No `current-grading` operation takes the record lock; the grading and audit writers take it. These cases check
//! that `grade map` and `grade invalidate` wait for `current_grading::record_lock` while another process holds it.

use bench_tools::current_grading;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

fn root(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).unwrap();
    path
}

/// Longer than `grade map` or `grade invalidate` takes to fail on an empty root without the lock, so a command still
/// running after it is waiting for the lock.
const BLOCKED: Duration = Duration::from_secs(2);
const GRADE_COMMANDS: [&[&str]; 2] = [
    &["map", "--work", "work", "--key", "key.json"],
    &["invalidate"],
];

fn grade(root: &Path, arguments: &[&str]) -> Child {
    Command::new(env!("CARGO_BIN_EXE_bench-tools"))
        .arg("grade")
        .args(arguments)
        .arg("--root")
        .arg(root)
        .current_dir(root)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap()
}

#[test]
fn grade_commands_wait_while_the_record_lock_is_held() {
    let root = root("grade-lock-held");
    let lock = current_grading::record_lock(root.as_os_str().as_encoded_bytes()).unwrap();
    let mut commands: Vec<Child> = GRADE_COMMANDS
        .iter()
        .map(|arguments| grade(&root, arguments))
        .collect();
    std::thread::sleep(BLOCKED);
    for command in &mut commands {
        assert!(
            command.try_wait().unwrap().is_none(),
            "a grade command ran while the lock was held"
        );
    }
    drop(lock);
    for command in commands {
        let output = command.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(2), "{output:?}");
    }
    let _ = std::fs::remove_dir_all(&root);
}
