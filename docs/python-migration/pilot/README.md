# Usage pilot result

**Recommendation for independent review: stop expansion of #149.** Rust improves
large-file usage processing, but this bounded hybrid integration has not established
the small-file CLI target fixed in [the precommitted protocol](protocol.md).
The measured Python adapter spawning Rust adds 6.37 ms median against the 5 ms limit;
its 1.45 ms standard deviation exceeds the 1.37 ms margin, and 2 of 7 paired trials
add less than 5 ms. The native executable alone takes 2.31 ms. Retain the isolated pilot,
strict Python adapter, corpus comparisons and arithmetic tests. No production caller
has moved, and this result does not authorize #153.

## Measured result

The [final run](evidence/measurements-v4/summary.json) has seven interleaved
trials per cell, 126 successful trials, and no wrong summaries or work counts.
Inputs contain 100 requests in 33,477 bytes or 100,000 requests in 33,672,912 bytes.
They scale the approved corpus, rather than replaying a paid review.

| Path, median | Existing Python | Typed Python | Release Rust |
| --- | ---: | ---: | ---: |
| Large cold core elapsed | 443.32 ms | 609.93 ms | 187.12 ms |
| Large cold core peak RSS | 89,704 KiB | 49,596 KiB | 19,152 KiB |
| Large warm core elapsed per read | 422.95 ms | 574.67 ms | 186.17 ms |
| Large CLI elapsed | 450.07 ms | 615.36 ms | 228.73 ms |
| Large CLI peak RSS | 90,632 KiB | 50,348 KiB | 19,392 KiB |
| Small CLI elapsed | 35.73 ms | 43.20 ms | 42.11 ms |

Large CLI elapsed is **450.07 → 228.73 ms**, with ranges 421.85–467.09 ms and
223.93–232.30 ms. Large core latency and RSS clear the 30% target against both
Python variants. Small CLI latency adds 6.37 ms against the permitted 5 ms.
Its ranges, 35.02–37.29 ms versus 39.80–43.72 ms, do not overlap.
This misses the median gate on this shared host; it does not establish an immutable
Rust startup cost or rule out a later, differently scoped integration. Small-CLI
standard deviations are 0.72 ms (existing), 1.12 ms (typed) and 1.45 ms (Rust adapter),
so this result does not establish a robust pass near the 5 ms boundary.
The native executable alone takes 2.31 ms; the adapter remains the measured CLI.

[Raw trials](evidence/measurements-v4/trials.jsonl) retain elapsed, user/system and
loop CPU, RSS, commands, byte counts, output digests, summaries and load. Warm trials
perform one excluded warmup and five complete measured rereads. Cold means a new
process, with an uncontrolled OS page cache. RSS is the maximum process RSS including
reaped children, not simultaneous aggregate tree memory; warm RSS includes warmup.
Cold elapsed and CPU include the small GNU time supervisor. The [first run's RSS
was invalid](evidence/measurements-v1/disposition.json) because the Python launcher's
fork memory floor contaminated it. All earlier raw data remains available. [Run lineage](evidence/measurement-lineage.json)
marks v2 as the earlier adapter result and v3 as diagnostic because it briefly
coincided with contract checks. V4 started after those checks finished, with the same
inputs, targets and trial count.

## What limits the result

The host is an AMD Ryzen 9 7900X, 24 logical CPUs, Linux under WSL2, Python 3.14.7
and Rust 1.99.1. Starting load was 1.49. [Environment and source identities](evidence/measurements-v4/environment.json)
include toolchains and active process names. This shared-host result is not a
cross-platform estimate or a prediction of paid review completion time.

[Profiles](evidence/profiles/) locate the cost in reading, JSON decoding and request
construction. Rust's separate phase run took 168.59 ms to read/parse and 5.46 ms
to summarize. Large cold core CPU is 183.12 ms against 187.12 ms elapsed, consistent
with one busy core. Its roughly 180 MB/s is below the local read/allocation ceiling:
[seven raw reads](evidence/profiles/raw-read.json) have a 26.01 ms median for the
same 33.67 MB. The syscall trace confirms full reads. Traced and profiled times are
excluded from the result table. The typed Python profile led to replacing per-count
objects with `NewType` integers before the first measurement run; its earlier profile
is retained. No failing parse, download, compilation or model wait counts as a gain.

Independent review identified unused imports on the small Rust CLI path. The
[before/after import profiles](evidence/profiles/small-cli-imports.json) show that
`typed_usage` and `transcript_usage` are no longer loaded there. Their cumulative
import times overlap dependencies and are not additive speedup measurements.
The earlier adapter median was 46.31 ms; the final 42.11 ms comes from a separate
interleaved run, not a controlled subtraction of import costs. The remaining timing
includes the Python adapter, its required imports, argument parsing, the child
process and formatting; this report does not attribute that whole cost to Rust.

## Contracts and costs

Both candidate command maps pass all 23 unchanged corpus cases. Of the 13 Codex
cases, 12 reach the Rust executable; `missing-root` fails in shared Python discovery
before Rust runs. The 10 transcript cases use the same strict Python adapter in both
maps. Corpus differential results identify only approved U1–U3 corrections. The
[defect register](../contracts/defects.md#pilot-resolution-in-152) records rejection
of unknown counts and damaged transcripts in the pilot, with production unchanged.
[Boundary details](contracts.md) name additional validation and compatibility limits.

Rust's private count/rate constructors, distinct identity types, known/unknown enum
and `Result` errors constrain construction and prevent accidental identity mixing
in safe callers. Public summary fields still require checked arithmetic. `forbid(unsafe_code)`, warnings-as-errors, Clippy, and release overflow
checks are gates. Parsing, count/cache bounds, finite prices and checked sums remain
runtime work. Python's strict mypy gate checks the typed domain, but `NewType` and
dataclass annotations do not enforce construction at runtime. Both implementations
still need independent behavior tests. These structural differences alone do not
justify expansion while the measured CLI target remains unestablished.

The initial release build took 4.91 s elapsed, 11.40 s CPU and 284,672 KiB peak RSS,
with dependencies already downloaded. The implementation span between the protocol
commit and the code commit was 12 min 21 s; the RSS correction followed 1 min 56 s
later. These are wall-clock spans, not active edit time or billed model time.
[Cost and check records](evidence/costs.json) retain failed checks, replacements and
available timings. Active human time, model tokens/dollars, and remote CI cost are
unknown. Round 1 requested two fixes and took 17 min 42 s to report finalization, including
3 min 19 s of verifier wall time. Both fixes and a full replacement measurement run
are retained. The next review outcome and elapsed time are supplied with the PR handoff.

## Reproduce

`tools/usage_pilot/` was removed with the Python side of the pilot. The steps below run at commit
`220bcf794`, the last one with it; `cargo test` in `rust/usage-pilot` still runs at any later commit.

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
new. [Check logs](evidence/checks-r2.json.gz) cover dev/release arithmetic, seeded
successes, malformed/truncated inputs, integer bounds, identity collisions and
timestamp forms, ignored-record payloads, existing usage tests, the corpus and its immutable decision history.
Strict mypy and import-preservation logs are alongside them. CI repeated the pilot
checks on Python 3.12; local measurements use Python 3.14.7. Supported-host binaries,
runtime provenance and production recovery remain outside this pilot.

## Exact decision text for #149

After independent approval, the parent can record this decision on the epic:

> Stop the Rust migration expansion from #152. The candidate command map passes the
> approved corpus: 12 Codex cases reach Rust, while missing-root discovery and all
> transcript cases run in shared Python. Large-file CLI median improves from
> 450.07 ms to 228.73 ms. Small-file CLI median increases from 35.73 ms to 42.11 ms
> (+6.37 ms against the precommitted 5 ms limit), with near-boundary variation:
> the Rust adapter's 1.45 ms standard deviation exceeds the 1.37 ms margin, and 2 of
> 7 paired trials add less than 5 ms. This measures the Python adapter spawning the
> Rust executable; native Rust alone takes 2.31 ms. The small-CLI target is not
> established for this bounded hybrid integration on this shared host. Under the
> precommitted protocol, stop; the structural type benefits do not independently
> justify expansion. Retain the pilot, strict Python adapters
> and behavioral/arithmetic tests. Keep production entrypoints unchanged and mark
> conditional issues #153–#160 not planned. This is a stopped migration, not a
> completed Python retirement. Evidence: the #152 PR's pilot report, protocol,
> final v4 raw measurements and independent review.

The parent owns that epic comment, downstream issue disposition and final merge.
