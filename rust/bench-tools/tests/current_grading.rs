//! No `current-grading` operation takes the record lock, as no `current_grading.py` operation does; the grading and
//! audit writers take it. These cases check that `current_grading::record_lock` excludes the retained Python's
//! `record_lock` on the same root in both directions, so a Rust writer and a Python writer cannot interleave.

use bench_tools::current_grading;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

const SETTLE: Duration = Duration::from_millis(400);

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

struct Root(PathBuf);

impl Root {
    fn new(name: &str) -> Root {
        let path = std::env::temp_dir().join(format!("{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        Root(path)
    }
}

impl Drop for Root {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The retained Python, which prints `held` once it holds the lock and releases it when its stdin closes.
fn python_holder(root: &Path) -> Child {
    let source = "import sys\nsys.path.insert(0, sys.argv[1])\nimport current_grading\n\
        with current_grading.record_lock(sys.argv[2]):\n    print('held', flush=True)\n    sys.stdin.read()\n";
    Command::new("python3")
        .args(["-c", source])
        .arg(repository().join("bench/tools"))
        .arg(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap()
}

#[test]
fn rust_waits_while_python_holds_the_lock() {
    let root = Root::new("current-record-lock-python-first");
    let mut python = python_holder(&root.0);
    let mut line = String::new();
    BufReader::new(python.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    assert_eq!(line, "held\n");
    let (sender, receiver) = mpsc::channel();
    let path = root.0.clone();
    let waiter = std::thread::spawn(move || {
        let lock = current_grading::record_lock(path.as_os_str().as_encoded_bytes()).unwrap();
        sender.send(()).unwrap();
        drop(lock);
    });
    assert!(
        receiver.recv_timeout(SETTLE).is_err(),
        "Rust took the lock while Python held it"
    );
    drop(python.stdin.take());
    assert!(python.wait().unwrap().success());
    receiver
        .recv_timeout(Duration::from_secs(10))
        .expect("Rust takes the lock once Python releases it");
    waiter.join().unwrap();
}

#[test]
fn python_waits_while_rust_holds_the_lock() {
    let root = Root::new("current-record-lock-rust-first");
    let lock = current_grading::record_lock(root.0.as_os_str().as_encoded_bytes()).unwrap();
    let source = "import sys\nsys.path.insert(0, sys.argv[1])\nimport current_grading\n\
        with current_grading.record_lock(sys.argv[2]):\n    print('acquired', end='')\n";
    let mut python = Command::new("python3")
        .args(["-c", source])
        .arg(repository().join("bench/tools"))
        .arg(&root.0)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    std::thread::sleep(SETTLE);
    assert!(
        python.try_wait().unwrap().is_none(),
        "Python took the lock while Rust held it"
    );
    drop(lock);
    let output = python.wait_with_output().unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"acquired");
}
