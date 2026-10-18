#![forbid(unsafe_code)]

use cpu_time::ProcessTime;
use serde::Serialize;
use std::fs::File;
use std::io::{BufReader, BufWriter, Write};
use std::path::PathBuf;
use std::time::Instant;
use usage_pilot::{ErrorKind, Rates, UsageError, read_rollout, summarize};

#[derive(Serialize)]
struct Measurement {
    elapsed_seconds: f64,
    cpu_seconds: f64,
    completed_iterations: u64,
    bytes_processed: u64,
}

fn run() -> Result<(), UsageError> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if !(5..=7).contains(&args.len()) {
        return Err(UsageError::new(
            ErrorKind::InvalidInput,
            "usage-pilot PATH IN_PRICE OUT_PRICE CACHED_MULT WRITE_MULT [REPEAT] [WARMUP]",
        ));
    }
    let path = PathBuf::from(&args[0]);
    let label = path.to_string_lossy();
    let number = |i: usize| -> Result<f64, UsageError> {
        args[i]
            .to_str()
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| UsageError::new(ErrorKind::InvalidInput, "invalid numeric argument"))
    };
    let rates = Rates::new(number(1)?, number(2)?, number(3)?, number(4)?)?;
    let iterations = |i: usize, default: u64| -> Result<u64, UsageError> {
        args.get(i).map_or(Ok(default), |arg| {
            arg.to_str()
                .and_then(|s| s.parse().ok())
                .ok_or_else(|| UsageError::new(ErrorKind::InvalidInput, "invalid iteration count"))
        })
    };
    let repeat = iterations(5, 1)?;
    let warmup = iterations(6, 0)?;
    if repeat == 0 {
        return Err(UsageError::new(
            ErrorKind::InvalidInput,
            "repeat must be positive",
        ));
    }
    let process = || {
        let file = File::open(&path)
            .map_err(|e| UsageError::new(ErrorKind::Io, format!("{label}: {e}")))?;
        let bytes = file
            .metadata()
            .map_err(|e| UsageError::new(ErrorKind::Io, e.to_string()))?
            .len();
        let rollout = read_rollout(BufReader::new(file), &label)?;
        Ok::<_, UsageError>((summarize(&rollout, &label, rates)?, bytes))
    };
    for _ in 0..warmup {
        let (summary, _) = process()?;
        std::hint::black_box(
            serde_json::to_vec(&summary)
                .map_err(|e| UsageError::new(ErrorKind::Io, e.to_string()))?,
        );
    }
    let stdout = std::io::stdout();
    let mut output = BufWriter::new(stdout.lock());
    let start = Instant::now();
    let cpu = ProcessTime::now();
    let mut bytes_processed = 0_u64;
    for _ in 0..repeat {
        let (summary, bytes) = process()?;
        serde_json::to_writer(&mut output, &summary)
            .map_err(|e| UsageError::new(ErrorKind::Io, e.to_string()))?;
        output
            .write_all(b"\n")
            .map_err(|e| UsageError::new(ErrorKind::Io, e.to_string()))?;
        output
            .flush()
            .map_err(|e| UsageError::new(ErrorKind::Io, e.to_string()))?;
        bytes_processed = bytes_processed
            .checked_add(bytes)
            .ok_or_else(|| UsageError::new(ErrorKind::Overflow, "processed bytes exceed u64"))?;
    }
    output
        .flush()
        .map_err(|e| UsageError::new(ErrorKind::Io, e.to_string()))?;
    let measurement = Measurement {
        elapsed_seconds: start.elapsed().as_secs_f64(),
        cpu_seconds: cpu.elapsed().as_secs_f64(),
        completed_iterations: repeat,
        bytes_processed,
    };
    eprintln!(
        "{}",
        serde_json::to_string(&measurement)
            .map_err(|e| UsageError::new(ErrorKind::Io, e.to_string()))?
    );
    Ok(())
}

fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            match serde_json::to_string(&error) {
                Ok(json) => eprintln!("{json}"),
                Err(_) => eprintln!("{error}"),
            }
            std::process::ExitCode::from(2)
        }
    }
}
