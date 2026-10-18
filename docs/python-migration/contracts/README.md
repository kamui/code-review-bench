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
or exactly matches its separately saved baseline defect observation.
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

## Fixture format `behavior-contract/1`

The corpus is [`usage.v1.json`](../../../tools/behavior_contracts/usage.v1.json).
It contains a format version, a relative `decisions` file, and ordered `cases`.
Each case defines:

- `id`, `program`, and `argv`, without a shell or environment interpolation.
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
components, and dot segments are refused. JSON argv uses escaped surrogate code
points for non-UTF-8 POSIX path bytes. macOS and Linux, including WSL, are the scope;
native Windows is outside the existing script policy.

Codex session discovery follows the selected root and its descendants. Transcript
usage takes explicit parent and child file paths, as its production callers do.
`transcript-parent-child-explicit` pins their sum and keeps identical request IDs
in different sessions distinct. `transcript-missing-root-path` pins an unreadable
root input, and `transcript-known-zero` distinguishes explicit zero from U3.

## Comparison decisions

`bytes` compares decoded `bytes_b64` without normalization. Newlines, whitespace,
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

Every case has a saved SHA-256 in `usage-decisions.v1.json`, computed over the
whole case as sorted, ASCII JSON with compact separators. A changed input,
comparator, expectation, or baseline exception requires updating its decision and
reason. The runner refuses an unmatched digest or unequal corpus and decision ID sets,
including an unrecorded case deletion. There is deliberately no golden
update command. Retain earlier decisions and expectations when approving a behavior
change; record the authority, old and new behavior, rationale, compatibility impact,
and resolving issue in a new decision version. Matching a port is not authority.

The negative CLI exercises in `test_behavior_contract.py` run deliberately wrong
local programs. They detect a changed JSON count, a removed newline in a hashed
file, and a duplicated publication journal entry. Diagnostics print actual semantic
diffs or expected and actual bytes and filesystem entries, including hashes.
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
