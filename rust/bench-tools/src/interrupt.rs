//! Stop signals held while a command has something to clean up or record, such as a copied sign-in or a charge.

use nix::sys::signal::{SigSet, Signal, kill, raise};
use nix::unistd::Pid;
use std::ffi::OsString;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::thread::JoinHandle;

/// The first argument that makes this executable release held signals and become the rest of its command line.
pub const RELEASE: &str = "__release-held-signals";
/// The first argument that makes this executable raise one signal, which it survives only if the signal is ignored.
pub const PROBE: &str = "__probe-signal";

const STOPPING: [Signal; 3] = [Signal::SIGHUP, Signal::SIGINT, Signal::SIGTERM];
/// The signals that can wake the watcher once the hold ends. macOS discards an ignored signal even while it is
/// blocked, so an ignored one cannot, nor can one whose default is to be ignored. Linux and macOS both take the
/// lowest-numbered pending signal first, and these are numbered above every stop signal on both, so a stop signal
/// still pending at the wake is noted before the watcher closes.
const WAKING: [Signal; 2] = [Signal::SIGVTALRM, Signal::SIGPROF];

fn set(signals: &[Signal]) -> SigSet {
    let mut set = SigSet::empty();
    for signal in signals {
        set.add(*signal);
    }
    set
}

/// Whether this process ignores `signal`, as a child started from `executable` inherits it. Reading a disposition
/// directly needs `sigaction`, which no safe API offers.
fn ignored(executable: &Path, signal: Signal) -> Result<bool, String> {
    let status = Command::new(executable)
        .arg(PROBE)
        .arg((signal as i32).to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|error| format!("cannot learn whether {signal} is ignored: {error}"))?;
    if status.success() {
        Ok(true)
    } else if status.signal() == Some(signal as i32) {
        Ok(false)
    } else {
        Err(format!(
            "cannot learn whether {signal} is ignored: the probe ended with {status}"
        ))
    }
}

#[derive(Default)]
struct State {
    received: AtomicI32,
    closing: AtomicBool,
}

/// Held stop signals; dropping it ends the process by the first that arrived, or releases them.
pub struct Held {
    executable: PathBuf,
    /// The signals this hold blocked, and so the ones it and a released child unblock.
    blocked: Vec<Signal>,
    state: Arc<State>,
    /// The thread noting held signals, and the signal that wakes it to finish.
    watcher: Option<(Signal, JoinHandle<()>)>,
}

/// Hold each of SIGHUP, SIGINT and SIGTERM that is neither ignored nor already blocked until the guard drops, noting
/// the first to arrive. Threads inherit the mask, so call this while no other thread runs, and start children with
/// [`Held::released`]. `executable` is this program, which answers [`PROBE`].
pub fn hold(executable: &Path) -> Result<Held, String> {
    let inherited = SigSet::thread_get_mask().map_err(|error| error.to_string())?;
    let mut held = Vec::new();
    for signal in STOPPING {
        if !inherited.contains(signal) && !ignored(executable, signal)? {
            held.push(signal);
        }
    }
    let state = Arc::new(State::default());
    if held.is_empty() {
        return Ok(Held {
            executable: executable.to_path_buf(),
            blocked: held,
            state,
            watcher: None,
        });
    }
    let mut close = None;
    for signal in WAKING {
        if !ignored(executable, signal)? {
            close = Some(signal);
            break;
        }
    }
    let Some(close) = close else {
        return Err("SIGVTALRM and SIGPROF are both ignored".to_string());
    };
    let waited = set(&[&held[..], &[close]].concat());
    let mut blocked = held;
    if !inherited.contains(close) {
        blocked.push(close);
    }
    set(&blocked)
        .thread_block()
        .map_err(|error| error.to_string())?;
    let watching = Arc::clone(&state);
    let watcher = std::thread::spawn(move || watch(waited, close, &watching));
    Ok(Held {
        executable: executable.to_path_buf(),
        blocked,
        state,
        watcher: Some((close, watcher)),
    })
}

/// Note the first of `waited` other than `close` until `close` arrives while closing.
fn watch(waited: SigSet, close: Signal, state: &State) {
    while let Ok(signal) = waited.wait() {
        if signal == close {
            if state.closing.load(Ordering::SeqCst) {
                return;
            }
        } else {
            let _ = state.received.compare_exchange(
                0,
                signal as i32,
                Ordering::SeqCst,
                Ordering::SeqCst,
            );
        }
    }
}

impl Held {
    /// The first held signal to arrive.
    pub fn received(&self) -> Option<Signal> {
        Signal::try_from(self.state.received.load(Ordering::SeqCst)).ok()
    }

    /// `program` started as `name` through this executable, which unblocks the held signals first: a child keeps its
    /// parent's signal mask across `exec`, so it would otherwise start with them blocked.
    pub fn released(&self, program: &Path, name: &str) -> Command {
        let numbers: Vec<String> = self
            .blocked
            .iter()
            .map(|signal| (*signal as i32).to_string())
            .collect();
        let mut command = Command::new(&self.executable);
        command
            .arg(RELEASE)
            .arg(numbers.join(","))
            .arg(program)
            .arg(name);
        command
    }

    /// Stop noting signals and return the first held signal that arrived. Dropping the guard then ends the process
    /// by that signal; without one, a held signal arriving later stays blocked until exit and never ends it.
    pub fn finish(&mut self) -> Option<Signal> {
        self.stop_watching();
        let received = self.received();
        if received.is_none() {
            self.blocked.clear();
        }
        received
    }

    fn stop_watching(&mut self) {
        self.state.closing.store(true, Ordering::SeqCst);
        if let Some((close, watcher)) = self.watcher.take() {
            if !watcher.is_finished() {
                let _ = kill(Pid::this(), close);
            }
            let _ = watcher.join();
        }
    }
}

impl Drop for Held {
    fn drop(&mut self) {
        self.stop_watching();
        if let Some(signal) = self.received() {
            let _ = set(&[signal]).thread_unblock();
            let _ = raise(signal);
            std::process::exit(128 + signal as i32);
        }
        let _ = set(&self.blocked).thread_unblock();
    }
}

fn signals(numbers: &OsString) -> Option<Vec<Signal>> {
    let numbers = numbers.to_str()?;
    if numbers.is_empty() {
        return Some(Vec::new());
    }
    numbers
        .split(',')
        .map(|number| Signal::try_from(number.parse::<i32>().ok()?).ok())
        .collect()
}

/// Run as `bench-tools RELEASE signals program name arguments…`: unblock the comma-separated `signals` and become
/// `program`.
pub fn exec_released(arguments: &[OsString]) -> ExitCode {
    let [numbers, program, name, rest @ ..] = arguments else {
        eprintln!("bench-tools: {RELEASE} needs the held signals, a program and its name");
        return ExitCode::from(2);
    };
    let Some(blocked) = signals(numbers) else {
        eprintln!("bench-tools: {RELEASE} needs comma-separated signal numbers");
        return ExitCode::from(2);
    };
    let _ = set(&blocked).thread_unblock();
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

/// Run as `bench-tools PROBE signal`: raise `signal`, which ends this process unless it is ignored.
pub fn exec_probe(arguments: &[OsString]) -> ExitCode {
    let signal = match arguments {
        [number] => signals(number).and_then(|found| match found[..] {
            [signal] => Some(signal),
            _ => None,
        }),
        _ => None,
    };
    let Some(signal) = signal else {
        eprintln!("bench-tools: {PROBE} needs one signal number");
        return ExitCode::from(2);
    };
    if set(&[signal]).thread_unblock().is_err() || raise(signal).is_err() {
        return ExitCode::from(2);
    }
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stop_signal_pending_with_the_closing_signal_is_noted_before_the_watcher_closes() {
        for close in WAKING {
            for stop in STOPPING {
                let waited = set(&[&STOPPING[..], &[close]].concat());
                waited.thread_block().unwrap();
                raise(stop).unwrap();
                raise(close).unwrap();
                let state = State::default();
                state.closing.store(true, Ordering::SeqCst);
                watch(waited, close, &state);
                assert_eq!(
                    state.received.load(Ordering::SeqCst),
                    stop as i32,
                    "{stop} pending with {close}"
                );
            }
        }
    }
}
