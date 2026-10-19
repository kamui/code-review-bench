# Behavioral contracts

These local contracts implement [#151](https://github.com/kamui/code-review-bench/issues/151),
part of [#149](https://github.com/kamui/code-review-bench/issues/149).
The [inventory](../README.md#responsibilities-and-contracts) assigns the usage pilot
to `bench/tools/codex_usage.py` and `bench/tools/transcript_usage.py`.
Production commands, frozen evidence, and imported originals remain unchanged.
#152 owns the typed Python and Rust pilot and the recorded baseline defects' resolutions.

## Run the usage corpus

From the repository root, run:

```sh
python3 tools/behavior_contract.py --allow-known-defects
python3 -m unittest discover -s tools -p test_behavior_contract.py -v
python3 -m unittest discover -s bench/tools -p 'test_*usage.py' -v
python3 bench/tools/codex_usage.py --self-test
python3 bench/tools/transcript_usage.py --self-test
```

The corpus command exits 0 only if every case meets its independent expectation
or exactly matches its separately saved baseline defect observation, except cases
reported as `SKIP UNAVAILABLE`. Skipped cases establish no coverage and are never passes.
Without `--allow-known-defects`, the independent missing-count and truncation expectations fail
at exit 1. `KNOWN DEFECT` is never an ordinary pass. Any different mismatch fails,
even with that option. See [the defect register](defects.md).

To run another implementation, supply a JSON command map using absolute executable
and script paths. The same command map can select a typed Python variant or a Rust
binary. Each program receives the fixture's argv after its command prefix.

```json
{
  "codex": ["/absolute/path/to/usage-pilot", "codex"],
  "transcript": ["/absolute/path/to/usage-pilot", "transcript"]
}
```

```sh
python3 tools/behavior_contract.py --implementations /tmp/candidate.json
python3 tools/behavior_contract.py --allow-known-defects --differential /tmp/candidate.json
```

The first command tests independent expectations. The second additionally compares
current Python with the candidate on separate roots. Differential agreement cannot
establish correctness or suppress an independent failure. With `--allow-known-defects`,
a candidate that meets the independent expectation while Python exactly matches the
recorded baseline observation produces `APPROVED FIX`, its resolution link, and the
actual diff. Other differences still fail, including a candidate that regresses from
a fixed primary implementation to the old defect. Full successful JSON
outputs participate in the differential, including fields outside a case's selected
independent assertions. The harness never obtains expected output from a candidate.

## Run the bench-tools corpora

[`roster.v1.json`](../../../tools/behavior_contracts/roster.v1.json),
[`diff-identity.v1.json`](../../../tools/behavior_contracts/diff-identity.v1.json),
[`check-manifest.v1.json`](../../../tools/behavior_contracts/check-manifest.v1.json),
[`rates.v1.json`](../../../tools/behavior_contracts/rates.v1.json) and
[`upstream.v1.json`](../../../tools/behavior_contracts/upstream.v1.json) pin the commands of
[`bench-tools`](../processing.md). Select a corpus with `--corpus`:

```sh
python3 tools/behavior_contract.py --corpus tools/behavior_contracts/roster.v1.json
python3 tools/behavior_contract.py --corpus tools/behavior_contracts/diff-identity.v1.json --differential /tmp/rust.json
```

Without `--implementations`, each program runs its maintained implementation. `codex`, `transcript`,
`diff-identity` and `check-manifest` run their Python scripts; `diff_identity.py` and `check_manifest.py` are retained
libraries. `rates` and `upstream` run the retained `rates.py` and `upstream.py` through drivers that read the case
directory as the repository and, for `rates`, take `--source PROVIDER=URL` pages, so no case leaves the host;
`rates-transport-failure` connects to a closed loopback port. Cases whose Rust behavior was changed on purpose carry
a `baseline_defect` with Python's output, so pass `--allow-known-defects` when Python is the primary.
`roster` has no Python
implementation, so it runs the executable `rust_runtime.resolve("bench-tools")` selects, and the command refuses
when none is installed for `HEAD`. A command map for the Rust programs names that executable:

```json
{
  "roster": ["/absolute/path/to/bench-tools", "roster"],
  "diff-identity": ["/absolute/path/to/bench-tools", "diff-identity"],
  "check-manifest": ["/absolute/path/to/bench-tools", "check-manifest"],
  "rates": ["/absolute/path/to/bench-tools", "rates"],
  "upstream": ["/absolute/path/to/bench-tools", "upstream"]
}
```

The `diff-identity` fixtures are bare repositories whose non-UTF-8 path names exist only inside Git tree objects,
so they need no `requires` capability. The cases run `git` from the minimal `PATH`.

## Fixture format `behavior-contract/1`

The corpus is [`usage.v1.json`](../../../tools/behavior_contracts/usage.v1.json).
It contains a format version, a relative `decisions` file, and ordered `cases`.
Each case defines:

- `id`, `program`, and `argv`, without a shell or environment interpolation.
  Each argument is a Unicode string or an object `{ "bytes_b64": "..." }` for
  exact POSIX bytes. No lone surrogate appears in the corpus JSON.
- Optional `requires`, currently `non-utf8-paths`, probes the disposable filesystem
  before setup. Invalid filename errors produce `SKIP UNAVAILABLE` and the run
  continues. Other filesystem failures remain execution errors.
- `stdin_b64`, the exact standard input bytes, including empty input.
- `initial`, a list of `file` or `directory` entries with base64 relative
  `path_b64`. Files include `bytes_b64`. Parent directories are created.
- `expected.exit`, the process exit status, and `stdout` and `stderr` comparators.
- `expected.filesystem`, the complete resulting tree, including directories,
  files with base64 bytes and SHA-256, and symlinks with base64 targets.
- `derivation`, the independent accounting or protocol argument for the expectation.
- Optional `baseline_defect`, with an exact `observed` comparator and a resolution link.

The runner creates a fresh temporary directory and an empty `.home` for every
implementation and case. It sets that directory as cwd, gives the process a minimal
environment without inherited credentials, captures output bytes, and removes the
root afterward. Commands time out after ten seconds. These are trusted local test
programs; the disposable cwd is not an OS sandbox. Commands must not access external
state or spawn surviving descendants. No model, network, or paid operations belong
in this corpus. Filesystem comparison covers the disposable tree, not arbitrary
external effects, permissions, hard-link identity, or file metadata.

Paths are relative POSIX bytes, encoded in base64. Absolute paths, NULs, empty
components, and dot segments are refused. macOS and Linux, including WSL, are the
scope; native Windows is outside the existing script policy. Filesystems that refuse
non-UTF-8 names, including Apple filesystems, cannot cover `non-utf8-path`. The
capability probe reports that limit explicitly. On capable hosts, its stdout is
byte-exact ASCII JSON, including the escaped path, field order and final newline.
This keeps invalid Unicode out of the fixture document while preserving byte ff.

Codex session discovery follows the selected root and its descendants. Transcript
usage takes explicit parent and child file paths, as its production callers do.
`transcript-parent-child-explicit` pins their sum and keeps identical request IDs
in different sessions distinct. `duplicate-response-last` uses different repeated
Codex records and requires the last entire record. `transcript-duplicate-maxima`
requires each token field's maximum, with maxima split across records so neither
first-record nor last-record deduplication can pass. `transcript-missing-root-path` pins an unreadable
root input, and `transcript-known-zero` distinguishes explicit zero from U3.

## Comparison decisions

`bytes` compares decoded `bytes_b64` without normalization. `prefix` requires the output to start with the
decoded bytes, for diagnostics that end with another program's wording, such as git's. The differential still
compares the whole stream of both implementations byte for byte. Newlines, whitespace,
JSON escaping and order, and trailing bytes matter in hashed files and retry payloads.
The corpus preserves every input byte and its SHA-256. It also captures one rendered
Markdown row byte-for-byte, including two-decimal cost rounding and its final newline.

`json` compares complete decoded values. Object key order, Unicode escaping, and
formatting whitespace may differ. Arrays retain order; absent keys differ from null;
booleans differ from numbers. Duplicate keys and nonfinite JSON numbers are rejected.
Integer values retain arbitrary precision, including the `2^53 + 1` fixture.
The current comparator also distinguishes integer and floating-point spellings
such as `1` and `1.0`; a future relaxation requires a recorded decision. Dollar
values follow current binary64 calculation and the transcript CLI's six-decimal
rounding. No tolerance hides a cost discrepancy.

`json-pointers` selects the explicitly listed RFC 6901 paths before semantic
comparison. Transcript cache fixtures independently assert authoritative totals,
5m and 1h partitions, unknown residuals, low and high costs, known-tier status,
and unavailable timing. Other presentation fields are outside those independent
assertions. Their full values remain covered by differential comparison and existing
tests. A missing pointer fails with its actual bytes. `escaping-order-time` covers
escaped models, timezone-equivalent timestamps, and fractional second truncation;
`non-utf8-path` also covers a valid final JSONL record without a newline.

Every case has a saved SHA-256 in `usage-decisions.v3.json`. Its `previous` entry
pins v2, which pins the unchanged legacy v1 bytes and format. Each current record
contains `reason`, `contract_sha256`, optional `supersedes`, and a `decision` ID.
The ID is `decision-` followed by the canonical digest of the record without its
`decision` field. Editing only a contract digest leaves an invalid decision ID.

A new decision file retains its predecessor through `previous.path`, `sha256`, and
`format` (`decisions/1` for the legacy map, `decisions/2` for records with history).
Changed records must name the prior contract digest in `supersedes`, change their
reason, and receive a new decision ID. Unchanged records can be copied verbatim.
History cannot remove a case. Corpus and current decision ID sets must match.
The original v1 digests remain legacy Python JSON hashes and are never recomputed.
Version 3 records the non-UTF-8 comparator's old semantic rule and new byte rule,
including its formatting compatibility cost.
The v2 records explicitly adopt canonical bytes v1; accounting expectations are
retained, and the non-UTF-8 case explicitly pins rendered output bytes.

Published decision files are immutable. `bun run test:web` runs the Git-history
check and the JavaScript digest vectors before the rest of the suite. The history
check covers every decision family, each named `<area>-decisions.v<N>.json`, on its own.
In the `usage` family, whose v1 is the legacy map, it anchors all existing versions at
the first commit that adds v2. A family whose v1 already holds `records`, such as
`roster` and `diff-identity`, anchors v1 at its own first addition, and its v1 names no
predecessor. Each later version is anchored at its own first addition. Every corpus
must select its family's latest version, a family that no corpus uses fails, and each
version must link to its immediate predecessor, including the legacy v1 root. A fresh
root, skipped predecessor or rollback fails. This also works after squash merges.
An edited or removed published version fails, even after its self-digest is
recomputed or the edit is committed. Add a new version instead. The check requires
full Git history; CI uses `fetch-depth: 0`. A source archive alone cannot establish
publication history, so run this gate in the full checkout before publication:

```sh
python3 tools/behavior_contract.py --verify-decision-history
```

There is no golden update command. An approved behavior change requires a new
version with its authority, old and new behavior, rationale, compatibility impact,
and resolving issue in the reason. Matching a port is not authority. Earlier
expectations remain available at their original git revision. The open entries in
`defects.md` must retain their separate `baseline_defect` cases and independent
expectations; the harness tests enforce that relationship.

### Canonical bytes v1

SHA-256 hashes the following binary encoding, without a trailing newline. JSON
numbers without a fraction or exponent are arbitrary-precision integers; other
numbers are finite IEEE 754 binary64 values. A consumer must preserve this distinction
and exact integers when reading JSON. Strings contain Unicode scalar values only.
Lengths and counts are unsigned ASCII decimal with no leading zeros except zero.

| Value | Bytes |
| --- | --- |
| Null | ASCII `n` |
| Boolean | ASCII `t` or `f` |
| Integer | ASCII `i`, signed decimal without leading zeros or plus sign, then `;`. Zero is `0`. |
| Binary64 | ASCII `d`, then eight bytes in big-endian IEEE 754 format, including the sign of zero. |
| String | ASCII `s`, UTF-8 byte length, `:`, then the UTF-8 bytes. No normalization. |
| Array | ASCII `[`, element count, `:`, then each encoded element in order. No closing byte. |
| Object | ASCII `{`, member count, `:`, then encoded key and value pairs. Sort keys by unsigned UTF-8 byte order. No closing byte. |

This encoding distinguishes `1` from `1.0`, preserves integers above 2^53, and avoids
serializer-dependent exponent spellings. `digest-vectors.json` pins nine byte/hash
vectors, including `4e-5`, negative zero, Unicode and nested objects. Python harness
tests and the independent JavaScript encoder check those same vectors:

```sh
bun tools/behavior_contracts/check-digest.mjs
```

The negative CLI exercises in `test_behavior_contract.py` run deliberately wrong
local programs. They detect a changed JSON count, a removed newline in a hashed
file, and a duplicated publication journal entry. Diagnostics print actual semantic
diffs or expected and actual bytes and filesystem entries, including hashes.
Invalid primary JSON produces a per-case `DIFFERENTIAL` diagnostic and exit 1;
it does not abort the remaining cases as an input error.
The synthetic journal proves effect multiplicity detection without remote publication.
It does not claim to test a real remote service.

## Focused fault seams for later slices

Later migration tickets add concrete fixtures at these boundaries. They must preserve
single ownership and pinned recovery, rather than automatically retrying an uncertain
side effect in another implementation.

| Boundary | Focused seam | Required local cases and observation |
| --- | --- | --- |
| Clock | Pass explicit timestamp inputs or a narrow `now` callable at the subsystem boundary. Usage already reads JSONL timestamps and timing sidecars. | Equal instants, offsets, missing events, reversed events, and fractional precision. Never infer missing completion from summed agent spans. |
| Process | Inject a command prefix or subprocess launcher at the runner boundary. The contract command map is the pilot seam. | Exit failure, signal, timeout, truncated stdout, and a child that fails before writing a receipt. Preserve actual exit and partial output. |
| Files | Pass a disposable root and inject the specific read, replace, or fsync operation in the storage slice. | Missing or unreadable input, short write, rename failure, and crash before or after replacement. Assert before and after bytes and receipts. |
| Remote publication | Inject the publisher transport in the publication owner, backed by a local append-only request journal. | Refusal before acceptance, acceptance with lost response, duplicate submission, and retry with the same idempotency key. Assert one effect and preserved uncertain disposition. |

This issue defines those seams; it does not add a workflow engine or change
production dependency injection. The usage pilot is read-only. Existing timing,
usage, inventory, and recovery tests remain in place and move with their owning slice.
