# Usage pilot protocol

This plan precedes implementation and measurements for #152, part of #149.
Base: `abb0176582020f1a519f2205a4c2091ef67857fb`. Sources: #150's inventory,
#151's 23-case corpus and decision history, and `contracts/defects.md` U1–U3.

## Boundary and compatibility decisions

Compare the existing `codex_usage.read_rollout` and `summarize`, a narrowly typed
Python implementation, and an isolated Rust library plus internal executable.
Production entrypoints and callers remain unchanged. A local Python adapter reuses
existing CLI discovery, formatting and pricing arguments around each candidate.
Measure this adapter end to end as well as the core executable; disclose its cost.

The Rust boundary is Codex JSONL, deduplication and accounting. Transcript processing
is not ported: both candidate command maps use the same strict Python transcript
adapter. Run all 23 corpus cases, but report Rust coverage separately from shared
transcript coverage. No transcript speedup may be claimed.

U1: all five Codex token categories are required. Missing or null counts are unknown,
not zero. Reject an unknown retained request before producing a total. Keep the last
whole record per response ID and its first tool attribution. Known zero is valid.
U2/U3: the isolated transcript adapter rejects malformed nonblank JSONL and missing
or null input, output, cache-creation or cache-read counts before the existing
transcript calculation. Keep unknown cache-write tiers and their cost bounds.
This deliberately changes the pilot diagnostics and rejects partial totals. It does
not change the documented production parser until a separately reviewed cutover.

Additional pilot boundaries: counts are integers in `0..=u64::MAX`; booleans,
fractions, negative values and overflowing aggregates are errors. Prices and costs
must be finite and nonnegative. Missing response IDs retain the baseline ordinal
fallback, represented separately from real IDs to prevent accidental collisions.
Native timestamps are ISO 8601; absent or unparseable timestamps stay unknown.
Retain existing corpus expectations and baseline defect observations unchanged.
Record any further discovered difference before accepting it.

## Targets fixed before tuning

The performance hypothesis is: release Rust reduces median elapsed time and peak
RSS for large, successful usage parsing by at least 30% against **both** Python
variants, with no more than 5 ms added median cold small-input CLI latency.
These thresholds justify operational work only for this bounded parser. A result
within observed variation is inconclusive. Failure to meet the targets does not
authorize changing the workload or dropping a side to manufacture a win.

Expansion also requires all independent expectations, checked release arithmetic,
no unexplained differences, and acceptable measured build/edit/review costs. A
structural-contract benefit can independently justify a bounded expansion, but the
report must name the rule and why typed Python cannot enforce it equally strongly.
Otherwise stop. No broad language migration follows automatically from this pilot.

## Measurements

Use deterministic small (100 requests) and large (100,000 requests) files derived
from the approved Codex records. Include repeated IDs with changed records, tool
events, models, timestamps, parent/child identity and irrelevant payloads. Preserve
generator parameters, hashes, byte counts and independent expected summaries.
Measure only completed, successful reads with checked outputs and exact work counts.

Run seven trials per implementation, size and mode, rotating implementation order:

- Cold process: one complete core invocation, including startup, read, summary and
  serialization. The OS page cache is uncontrolled and explicitly not called cold.
- Warm process: one unreported warmup followed by five complete rereads in the same
  process, each parsed, summarized and serialized. Record total and per-read time.
- CLI end to end: existing CLI or candidate adapter, including process startup and
  all formatting; seven cold-process trials on each size.

Record elapsed time, user/system CPU, per-process peak RSS, bytes processed, exit
status, verified result digest and completed iterations. Use child resource usage,
not a cumulative maximum across unrelated trials. Record hardware, OS, toolchains,
load, source identity and command lines. Retain every raw trial, including failures.
Do not include compilation, downloads, fixture generation, profiling or model waits
in parser timings. Report those costs separately. Interleave sides on this shared
host and retain median, min, max and dispersion. No privileged cache eviction.

Run an untimed profile for each side. Identify CPU/JSON/allocation/I/O limits using
profiles and CPU/elapsed ratios. Check bytes/time plausibility against a local read
measurement. The end-to-end scope is the usage CLI; paid review runtime is not
measured and no whole-benchmark acceleration may be claimed.

## Verification and delivery

Run the unchanged corpus against baseline (three known defects), both candidates,
and differentially. Add deterministic generated properties and malformed-input
cases, boundary integers, overflow, duplicate ordering and unknown accounting.
Run Cargo tests in dev and release, formatting, Clippy with warnings denied, the
existing usage tests, contract tests/history and import preservation. CI repeats
the isolated pilot checks without paid calls.

Retain build/check logs and timestamps, implementation and review commits, reviewer
rounds and actual available costs. Unknown model usage, dollars, active human time
and not-yet-run CI remain unknown. Independent review uses Claude Opus 5.5 High;
implementation, feedback and CI fixes use Astra 6 High. Approved optional findings
receive fixes or evidence-backed declines without another review unless very high
risk, under the user's explicit session rule. Changes Requested requires re-review.

Publish exactly one PR. Its report contains the exact expand/stop decision for #149,
with measured evidence and constraints. The parent owns the published review,
epic decision comment, final merge and any downstream issue changes.
