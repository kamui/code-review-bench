#![forbid(unsafe_code)]

use std::{fs::File, io::BufReader, time::Instant};
use usage_pilot::{Rates, read_rollout, summarize};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args_os().nth(1).ok_or("input path required")?;
    let file = File::open(path)?;
    let start = Instant::now();
    let rollout = read_rollout(BufReader::new(file), "profile")?;
    let read_parse = start.elapsed();
    let start = Instant::now();
    let summary = summarize(&rollout, "profile", Rates::new(10.0, 50.0, 0.1, 1.25)?)?;
    let aggregate = start.elapsed();
    println!("{}", serde_json::to_string(&summary)?);
    eprintln!(
        "read_parse_seconds={} summarize_seconds={}",
        read_parse.as_secs_f64(),
        aggregate.as_secs_f64()
    );
    Ok(())
}
