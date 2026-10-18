#![forbid(unsafe_code)]

use bench_tools::pytext;
use std::ffi::OsString;
use std::process::ExitCode;

const USAGE: &str = "usage: bench-tools COMMAND ...";

fn main() -> ExitCode {
    let arguments: Vec<OsString> = std::env::args_os().skip(1).collect();
    match arguments.first().map(|command| pytext::os_bytes(command)) {
        Some(b"-h" | b"--help") => {
            println!("{USAGE}\n\nDeterministic benchmark tools.");
            ExitCode::SUCCESS
        }
        Some(other) => {
            eprintln!(
                "{USAGE}\nbench-tools: error: unknown command {}",
                pytext::stderr_text(other)
            );
            ExitCode::from(2)
        }
        None => {
            eprintln!("{USAGE}\nbench-tools: error: give a command");
            ExitCode::from(2)
        }
    }
}
