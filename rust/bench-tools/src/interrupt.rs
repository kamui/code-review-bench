//! Stop signals held while a command has something to clean up, such as a copied sign-in.

use nix::sys::signal::{SigSet, Signal, kill, raise};
use nix::unistd::Pid;
use std::ffi::OsString;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Command, ExitCode};
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::thread::JoinHandle;

/// The first argument that makes this executable release held signals and become the rest of its command line.
pub const RELEASE: &str = "__release-held-signals";

static RECEIVED: AtomicI32 = AtomicI32::new(0);
static CLOSING: AtomicBool = AtomicBool::new(false);

const STOPPING: [Signal; 3] = [Signal::SIGHUP, Signal::SIGINT, Signal::SIGTERM];
/// Wakes the watcher once the hold ends.
const CLOSE: Signal = Signal::SIGUSR2;

fn set(signals: &[Signal]) -> SigSet {
    let mut set = SigSet::empty();
    for signal in signals {
        set.add(*signal);
    }
    set
}

fn waited() -> SigSet {
    set(&[&STOPPING[..], &[CLOSE]].concat())
}

/// Held stop signals; dropping it ends the process by the first that arrived, or releases them.
pub struct Held {
    watcher: Option<JoinHandle<()>>,
}

/// Hold SIGHUP, SIGINT and SIGTERM until the guard drops, noting the first to arrive. Threads inherit the mask,
/// so call this while no other thread runs, and start children with [`released`].
pub fn hold() -> nix::Result<Held> {
    let waited = waited();
    waited.thread_block()?;
    let watcher = std::thread::spawn(move || {
        while let Ok(signal) = waited.wait() {
            if signal == CLOSE {
                if CLOSING.load(Ordering::SeqCst) {
                    return;
                }
            } else {
                let _ =
                    RECEIVED.compare_exchange(0, signal as i32, Ordering::SeqCst, Ordering::SeqCst);
            }
        }
    });
    Ok(Held {
        watcher: Some(watcher),
    })
}

pub fn received() -> Option<Signal> {
    Signal::try_from(RECEIVED.load(Ordering::SeqCst)).ok()
}

impl Drop for Held {
    fn drop(&mut self) {
        CLOSING.store(true, Ordering::SeqCst);
        if let Some(watcher) = self.watcher.take() {
            if !watcher.is_finished() {
                let _ = kill(Pid::this(), CLOSE);
            }
            let _ = watcher.join();
        }
        if let Some(signal) = received() {
            let _ = set(&STOPPING).thread_unblock();
            let _ = raise(signal);
            std::process::exit(128 + signal as i32);
        }
        let _ = waited().thread_unblock();
    }
}

/// `program` started as `name` through `executable`, which releases the held signals first: a child keeps its
/// parent's signal mask across `exec`, so it would otherwise start with the stop signals blocked.
pub fn released(executable: &Path, program: &Path, name: &str) -> Command {
    let mut command = Command::new(executable);
    command.arg(RELEASE).arg(program).arg(name);
    command
}

/// Run as `bench-tools RELEASE program name arguments…`: release the held signals and become `program`.
pub fn exec_released(arguments: &[OsString]) -> ExitCode {
    let [program, name, rest @ ..] = arguments else {
        eprintln!("bench-tools: {RELEASE} needs a program and its name");
        return ExitCode::from(2);
    };
    let _ = waited().thread_unblock();
    let error = Command::new(program)
        .arg0(name)
        .args(rest.iter().map(OsString::as_os_str))
        .exec();
    eprintln!(
        "bench-tools: cannot run {}: {error}",
        Path::new(program).display()
    );
    ExitCode::from(127)
}
