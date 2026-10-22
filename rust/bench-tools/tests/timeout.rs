//! `bench-tools timeout`, which `bench/tools/dispatch.sh` runs each reviewer under.

use nix::sys::signal::{Signal, kill};
use nix::unistd::Pid;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::process::ExitStatusExt;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn timeout(seconds: &str, script: &str) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_bench-tools"));
    command.args(["timeout", seconds, "sh", "-c", script]);
    command
}

#[test]
fn the_command_keeps_its_stdin_and_exit_code_and_a_signal_ends_it_as_128_plus_the_number() {
    let mut child = timeout("60", r#"read line; [ "$line" = prompt ] && exit 7"#)
        .stdin(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"prompt\n").unwrap();
    assert_eq!(child.wait().unwrap().code(), Some(7));
    let killed = timeout("60", "kill -TERM $$").status().unwrap();
    assert_eq!(killed.code(), Some(128 + Signal::SIGTERM as i32));
}

#[test]
fn the_deadline_ends_the_whole_process_group_with_124() {
    let started = Instant::now();
    let output = timeout("1", "sleep 30 & wait").output().unwrap();
    assert_eq!(output.status.code(), Some(124), "{output:?}");
    // The backgrounded sleep holds stdout open, so the output ends this soon only if the group was killed.
    assert!(started.elapsed() < Duration::from_secs(20));
}

#[test]
fn a_stop_signal_reaches_the_group_and_then_ends_bench_tools_by_that_signal() {
    let started = Instant::now();
    let mut child = timeout("60", "echo ready; sleep 30 & wait")
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let mut line = String::new();
    stdout.read_line(&mut line).unwrap();
    assert_eq!(line, "ready\n");
    kill(
        Pid::from_raw(i32::try_from(child.id()).unwrap()),
        Signal::SIGTERM,
    )
    .unwrap();
    assert_eq!(child.wait().unwrap().signal(), Some(Signal::SIGTERM as i32));
    stdout.read_to_end(&mut Vec::new()).unwrap();
    assert!(started.elapsed() < Duration::from_secs(20));
}
