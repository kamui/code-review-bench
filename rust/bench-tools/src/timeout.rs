//! A command run under a deadline as GNU `timeout` runs it, for `bench/tools/dispatch.sh` on hosts without
//! `timeout(1)`, such as macOS.

use crate::interrupt;
use nix::sys::signal::{Signal, killpg};
use nix::unistd::Pid;
use std::ffi::OsString;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::Path;
use std::time::{Duration, Instant};

/// The exit code once the deadline has passed, GNU `timeout`'s.
pub const TIMED_OUT: i32 = 124;

/// Run `command` leading its own process group, and send the group SIGTERM once `seconds` pass, or the first held
/// stop signal when one arrives. Returns the command's exit code, 128 plus the signal that ended it, or
/// [`TIMED_OUT`]; after a stop signal this process ends by that signal once the command has. `executable` is this
/// program, which answers [`interrupt::PROBE`] and [`interrupt::RELEASE`].
pub fn run(executable: &Path, seconds: u64, command: &[OsString]) -> Result<i32, String> {
    let [program, arguments @ ..] = command else {
        return Err("timeout needs a command".to_string());
    };
    let held = interrupt::hold(executable)?;
    let mut child = held
        .released(Path::new(program), &program.to_string_lossy())
        .args(arguments)
        .process_group(0)
        .spawn()
        .map_err(|error| format!("cannot run {}: {error}", Path::new(program).display()))?;
    let group = Pid::from_raw(i32::try_from(child.id()).map_err(|error| error.to_string())?);
    let deadline = Instant::now().checked_add(Duration::from_secs(seconds));
    let mut timed_out = false;
    let mut forwarded = false;
    let status = loop {
        if let Some(status) = child.try_wait().map_err(|error| error.to_string())? {
            break status;
        }
        if !forwarded && let Some(signal) = held.received() {
            let _ = killpg(group, signal);
            forwarded = true;
        }
        if !timed_out && deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            let _ = killpg(group, Signal::SIGTERM);
            timed_out = true;
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    if timed_out {
        return Ok(TIMED_OUT);
    }
    Ok(status
        .code()
        .unwrap_or_else(|| 128 + status.signal().unwrap_or(0)))
}
