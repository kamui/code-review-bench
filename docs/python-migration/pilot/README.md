# Usage pilot result

**Recommendation for independent review: stop expansion of #149.** Rust improves
large-file usage processing, but the complete small-file CLI exceeds the latency
limit fixed in [the precommitted protocol](protocol.md). Retain the isolated pilot,
strict Python adapter, corpus comparisons and arithmetic tests. No production caller
has moved, and this result does not authorize #153.

## Measured result

The [replacement run](evidence/measurements-v2/summary.json) has seven interleaved
trials per cell, 126 successful trials, and no wrong summaries or work counts.
Inputs contain 100 requests in 33,477 bytes or 100,000 requests in 33,672,912 bytes.
They scale the approved corpus, rather than replaying a paid review.

| Path, median | Existing Python | Typed Python | Release Rust |
| --- | ---: | ---: | ---: |
| Large cold core elapsed | 468.42 ms | 672.28 ms | 195.56 ms |
| Large cold core peak RSS | 89,704 KiB | 49,392 KiB | 19,156 KiB |
| Large warm core elapsed per read | 444.87 ms | 588.56 ms | 196.75 ms |
| Large CLI elapsed | 490.31 ms | 651.48 ms | 234.37 ms |
| Large CLI peak RSS | 90,400 KiB | 52,280 KiB | 21,312 KiB |
| Small CLI elapsed | 35.47 ms | 45.74 ms | 46.31 ms |

Large CLI elapsed is **490.31 → 234.37 ms**, with ranges 438.27–525.83 ms and
226.75–272.71 ms. Large core latency and RSS clear the 30% target against both
Python variants. Small CLI latency adds 10.84 ms against the permitted 5 ms.
Its ranges, 34.81–41.02 ms versus 44.50–49.04 ms, do not overlap.
The adapter's Python startup and subprocess boundary outweigh the small parse saving.
The native executable alone is faster; that does not make the measured CLI faster.

[Raw trials](evidence/measurements-v2/trials.jsonl) retain elapsed, user/system and
loop CPU, RSS, commands, byte counts, output digests, summaries and load. Warm trials
perform one excluded warmup and five complete measured rereads. Cold means a new
process, with an uncontrolled OS page cache. RSS is the maximum process RSS including
reaped children, not simultaneous aggregate tree memory; warm RSS includes warmup.
Cold elapsed and CPU include the small GNU time supervisor. The [first run's RSS
was invalid](evidence/measurements-v1/disposition.json) because the Python launcher's
fork memory floor contaminated it. All first-run data remains available.

## What limits the result

The host is an AMD Ryzen 9 7900X, 24 logical CPUs, Linux under WSL2, Python 3.14.7
and Rust 1.99.1. Starting load was 3.56. [Environment and source identities](evidence/measurements-v2/environment.json)
include toolchains and active process names. This shared-host result is not a
cross-platform estimate or a prediction of paid review completion time.

[Profiles](evidence/profiles/) locate the cost in reading, JSON decoding and request
construction. Rust's separate phase run took 168.59 ms to read/parse and 5.46 ms
to summarize. Large cold core CPU is 191.59 ms against 195.56 ms elapsed, consistent
with one busy core. Its roughly 172 MB/s is below the local read/allocation ceiling:
[seven raw reads](evidence/profiles/raw-read.json) have a 26.01 ms median for the
same 33.67 MB. The syscall trace confirms full reads. Traced and profiled times are
excluded from the result table. The typed Python profile led to replacing per-count
objects with `NewType` integers before either measurement run; its earlier profile
is retained. No failing parse, download, compilation or model wait counts as a gain.

## Contracts and costs

Both candidate command maps pass all 23 unchanged corpus cases. Rust owns the 13
Codex cases; the 10 transcript cases use the same strict Python adapter in both
maps. Differential results identify only approved U1–U3 corrections. The
[defect register](../contracts/defects.md#pilot-resolution-in-152) records rejection
of unknown counts and damaged transcripts in the pilot, with production unchanged.
[Boundary details](contracts.md) name additional validation and compatibility limits.

Rust's private count constructor, distinct identity types, known/unknown enum and
`Result` errors prevent unchecked domain construction and accidental identity mixing
in safe callers. `forbid(unsafe_code)`, warnings-as-errors, Clippy, and release overflow
checks are gates. Parsing, count/cache bounds, finite prices and checked sums remain
runtime work. Python's strict mypy gate checks the typed domain, but `NewType` and
dataclass annotations do not enforce construction at runtime. Both implementations
still need independent behavior tests. These structural differences alone do not
justify expansion after the measured CLI gate failed.

The initial release build took 4.91 s elapsed, 11.40 s CPU and 284,672 KiB peak RSS,
with dependencies already downloaded. The implementation span between the protocol
commit and the code commit was 12 min 21 s; the RSS correction followed 1 min 56 s
later. These are wall-clock spans, not active edit time or billed model time.
[Cost and check records](evidence/costs.json) retain failed checks, replacements and
available timings. Active human time, model tokens/dollars, and remote CI cost are
unknown. Independent review outcomes and elapsed time are supplied with the PR handoff.

## Reproduce

From `rust/usage-pilot`, run `cargo fetch --locked`, then `cargo build --locked --release --examples --bins`.
From the repository root:

```sh
python3 tools/usage_pilot/verify.py --output /tmp/pilot-checks-new
python3 -m pip install -r tools/usage_pilot/requirements-checks.txt
python3 -m mypy --strict --follow-imports=silent tools/usage_pilot/typed_usage.py
python3 tools/usage_pilot/measure.py --prepare /tmp/pilot-inputs-new
python3 tools/usage_pilot/measure.py --inputs /tmp/pilot-inputs-new --output /tmp/pilot-results-new
bun run verify:import
```

The measurement runner requires Linux and GNU time. Each output directory must be
new. [Check logs](evidence/checks-final.json.gz) cover dev/release arithmetic, seeded
successes, malformed/truncated inputs, integer bounds, identity collisions and
timestamp forms, existing usage tests, the corpus and its immutable decision history.
Strict mypy and import-preservation logs are alongside them. CI repeats the pilot
checks on Python 3.12; local measurements use Python 3.14.7. Supported-host binaries,
runtime provenance and production recovery remain outside this pilot.

## Exact decision text for #149

After independent approval, the parent can record this decision on the epic:

> Stop the Rust migration expansion from #152. The isolated Rust usage parser passes
> the approved contracts and improves the large-file CLI median from 490.31 ms to
> 234.37 ms, but small-file CLI latency increases from 35.47 ms to 46.31 ms, exceeding
> the precommitted 5 ms limit. The structural type benefits do not independently
> justify expansion after that gate failed. Retain the pilot, strict Python adapters
> and behavioral/arithmetic tests. Keep production entrypoints unchanged and mark
> conditional issues #153–#160 not planned. This is a stopped migration, not a
> completed Python retirement. Evidence: the #152 PR's pilot report, protocol,
> corrected raw measurements and independent review.

The parent owns that epic comment, downstream issue disposition and final merge.
