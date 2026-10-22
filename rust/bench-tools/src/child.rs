//! A child process run to completion or to a deadline, with its output captured.

use std::io::{Read, Write};
use std::os::unix::process::ExitStatusExt;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

pub struct Output {
    /// The exit code, the negated signal number when a signal ended it, or `None` when the deadline passed.
    pub code: Option<i32>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

pub fn exit_code(status: ExitStatus) -> i32 {
    status
        .code()
        .unwrap_or_else(|| -status.signal().unwrap_or(0))
}

fn drain(stream: Option<impl Read + Send + 'static>) -> std::thread::JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        let mut out = Vec::new();
        if let Some(mut stream) = stream {
            let _ = stream.read_to_end(&mut out);
        }
        out
    })
}

/// Wait for `child`, killing it once `timeout` passes; `None` means it was killed.
pub fn wait(child: &mut Child, timeout: Duration) -> std::io::Result<Option<i32>> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(Some(exit_code(status)));
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Ok(None);
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Run `command` with stdin closed, capturing both streams.
pub fn run(command: &mut Command, timeout: Duration) -> std::io::Result<Output> {
    run_input(command, None, timeout)
}

/// Run `command` with `input` as its stdin, or stdin closed for `None`, capturing both streams.
pub fn run_input(
    command: &mut Command,
    input: Option<&[u8]>,
    timeout: Duration,
) -> std::io::Result<Output> {
    let mut child = command
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let feeder = child.stdin.take().map(|mut stdin| {
        let input = input.unwrap_or_default().to_vec();
        std::thread::spawn(move || {
            let _ = stdin.write_all(&input);
        })
    });
    let stdout = drain(child.stdout.take());
    let stderr = drain(child.stderr.take());
    let code = wait(&mut child, timeout)?;
    if let Some(feeder) = feeder {
        let _ = feeder.join();
    }
    Ok(Output {
        code,
        stdout: stdout.join().unwrap_or_default(),
        stderr: stderr.join().unwrap_or_default(),
    })
}
