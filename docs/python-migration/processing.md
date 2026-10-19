# Deterministic processing in Rust

This records the processing stage, [#154](https://github.com/kamui/code-review-bench/issues/154), part of
[#149](https://github.com/kamui/code-review-bench/issues/149). It lands in slices that follow the
[delivery plan](https://github.com/kamui/code-review-bench/issues/154#issuecomment-6253814508). Slice 0,
[#192](https://github.com/kamui/code-review-bench/issues/192), retires modules with no current caller, as
[processing retirement](processing-retirement.md) records. Slice 1,
[#193](https://github.com/kamui/code-review-bench/issues/193), adds the crate and ports `roster` and `diff_identity`.
Slice 2, [#194](https://github.com/kamui/code-review-bench/issues/194), ports the review normalizers and moves their
runner and dispatch callers, as [the normalizers](#the-review-normalizers) describes.

## The `bench-tools` crate

[`rust/bench-tools`](../../rust/bench-tools) builds one binary, `bench-tools`, with one subcommand per tool and one
library module per tool in `src/`. Later slices add their tools to it. Its lints, toolchain and release profile are
those of `rust/usage-pilot`, which stays as pilot evidence. It has no registry dependencies, so an install needs no
network. Shared modules carry the Python behavior that byte-pinned outputs depend on:

- `pyjson` reads JSON as `json.loads` does and writes it as `json.dumps` does, including `float.__repr__` spelling,
  integers of any length up to Python's 4300-digit limit, key order, and `JSONDecodeError` messages.
- `pytext` holds `surrogateescape` decoding, `str.strip` whitespace, `repr`, `pathlib` path joining, universal
  newlines, and the `OSError` and `UnicodeDecodeError` messages that diagnostics quote.
- `sha256` implements SHA-256.

Run a subcommand through the pinned executable, as [Rust runtime pinning](runtime.md#running-a-maintained-command)
describes:

```sh
python3 bench/tools/rust_runtime.py install --crate rust/bench-tools --select
bun run roster
python3 bench/tools/rust_runtime.py exec bench-tools -- diff-identity <repo> <base> <head>
```

## Migrated responsibilities

| Python | Rust | Callers moved | Python implementation |
| --- | --- | --- | --- |
| `bench/tools/roster.py` | `bench-tools roster` | `bun run roster` in `AGENTS.md`, `README.md` and `bench/README.md` | Deleted with `test_roster.py`, whose cases are in [`roster.v1.json`](../../tools/behavior_contracts/roster.v1.json). |
| `bench/tools/diff_identity.py` | `bench-tools diff-identity`, `bench_tools::diff_identity` | The CLI in `bench/README.md` | Retained library, below. |
| `bench/tools/normalize_review.py` | `bench-tools normalize-review`, `bench_tools::normalize_review` | `dispatch.sh`, `claude_skill_runner.py`, `codex_skill_runner.py`, `file_attempt.py --replay` and the `test:bench` script | Retained library, below. `test_normalize_review.py`'s cases are in [`normalize-review.v1.json`](../../tools/behavior_contracts/normalize-review.v1.json), which `test:bench` runs against the retained Python. |
| `bench/tools/normalize_thermo.py` | `bench-tools normalize-thermo`, `bench_tools::normalize_thermo` | The same skill runners and `file_attempt.py --replay` | Deleted with `test_normalize_thermo.py`, whose cases are in [`normalize-thermo.v1.json`](../../tools/behavior_contracts/normalize-thermo.v1.json). |

## Retained libraries

A retained library stays byte-unchanged while a later stage still imports or hashes it. Its CLI callers move to Rust,
and CI runs the corpus against both implementations with a differential. It is removed once its removal condition holds.

| Module | Imported or hashed by | Removal condition | Owner |
| --- | --- | --- | --- |
| `bench/tools/diff_identity.py` | `provision.py` imports `identity`; `test_cache_replacements.py` imports it; `grade.runner_files()` hashes it, so a byte change refuses every prepared grading batch | `provision.py` computes diff identity through the Rust runtime ([#157](https://github.com/kamui/code-review-bench/issues/157)) and `grade.runner_files()` no longer hashes `diff_identity.py` ([#156](https://github.com/kamui/code-review-bench/issues/156)) | #157 |
| `bench/tools/normalize_review.py` | `grade.py` imports `render` for blind grading; `contributor_adapters.collect` calls `from_codex`, `from_claude_builtin` and `from_codex_skill` in process; `grade.runner_files()` hashes it | `grade.render` renders through the Rust runtime and `grade.runner_files()` no longer hashes `normalize_review.py` ([#156](https://github.com/kamui/code-review-bench/issues/156)), and the contributor backend normalizes through it ([#158](https://github.com/kamui/code-review-bench/issues/158)) | #156, #158 |

## The review normalizers

`bench-tools normalize-review` and `bench-tools normalize-thermo` take the arguments of `normalize_review.py` and
`normalize_thermo.py` and write the same `normalized.json` bytes, stdout and stderr, so a saved `normalize.txt`
(`dispatch.sh`) or `normalization.txt` (the skill runners) reads as it did. Diagnostics keep the names
`normalize_review.py:` and `normalize_thermo.py:`. `--timing` stamps `payload_validated_at` from the system clock;
cargo tests pin that rewrite with a fixed clock, because a corpus cannot.

A review's normalizer runs after the reviewer has returned, so every caller resolves it before a review can start:

- `run_cell.py` resolves the run's pinned `bench-tools` with `rust_runtime.for_run` before it chooses a cell or claims
  one, dry runs included, and passes it to the dispatch as `BENCH_RUST_BENCH_TOOLS` with its SHA-256.
- `dispatch.sh` checks that executable and its hash before it makes the fresh home; the skill runners resolve it first in
  `prepare_attempt`. Either refusal leaves no `timing.json`, so `run_cell.py` releases the claim.
- `review_queue.py` refuses, before any launch, a run whose `freeze_commit` holds `rust/bench-tools` but whose queue pins
  no `bench-tools`.
- `file_attempt.py --replay` runs the `bench-tools` that the run pins, the run being `--run` or `bench/runs/<run-id>`.

A run frozen before this change pins no `bench-tools`. Its own freeze checkout still has the Python normalizers, and
it runs and replays from there exactly as before. From a checkout after this change, `run_cell.py`, the skill runners
and `file_attempt.py --replay` refuse such a run, naming its freeze commit, and `review_queue.py` already refuses it
because its runner files differ from that commit.

The contributor path never reaches this branch: `contributor_adapters.collect` normalizes in process with the retained
library, and it uses the skill runners only for helpers, never `prepare_attempt` or `launch`.
`test_contributor.AdapterTests.test_openrouter_adapters_execute_fresh_subprocesses_and_collect_reports` fails if
collection resolves a Rust runtime or calls either.

## Behavior the corpora do not pin

These differences from the Python implementations are outside every contract. A caller that relied on one needs a
recorded decision.

- `bench-tools roster` reads the repository at the current directory unless `--root` names another. `roster.py`
  read the repository that held the script. `bun run roster` runs from the repository root.
- An input whose shape made Python raise a traceback, such as a registry without `suites`, exits 2 with a message
  naming the file. Python exited 1.
- A lone surrogate escape such as `"\ud800"` and JSON nested more than 1,000 levels deep are refused, exiting 2.
  Python kept the surrogate, and its own nesting limit depends on its version and stack.
- `repr` in a diagnostic escapes the non-printable characters Python escapes, except unassigned code points outside
  the noncharacters, because the crate carries no Unicode tables.
- `diff-identity` has no `--self-test`; `cargo test` and the corpus cover it. It accepts options between
  positional arguments, which argparse refuses for some orders.
- `normalize-review` has no `--self-test`, and options must be spelled in full; argparse also accepted unique prefixes.
- Where the normalizers' Python raised a traceback and exited 1, they exit 2 with a message: an input that is not
  UTF-8, a shape such as a `findings` value that is not a list, an `--out` that cannot be written, a timing sidecar that
  is not an object, and a fenced JSON array whose integer exceeds Python's digit limit. The v2 decisions of both
  normalizer corpora record each case. In `normalize-review.v1.json` they carry the retained Python's traceback as a
  `baseline_defect`, so its corpus runs with `--allow-known-defects`.
- `\d` in the Codex bullet pattern and `\w` in the thermo no-findings statement's `\b` follow Python 3.14, whose
  Unicode database is 16.0. `pytext` holds both tables, generated from Python 3.14. Python 3.12 does not match the
  characters Unicode 16.0 added.
- `normalize-thermo` resolves a relative missing `--artifact-root` as Python 3.13 and later do, naming the absolute
  component in the error; Python 3.12 named the relative one. A symbolic link loop in it is an `ELOOP` error, as in
  Python 3.13 and later; Python 3.12 raised a traceback. Callers pass an absolute root.
- `--sessions-dir` does not follow symbolic links to directories; Python's `rglob` follows them in some versions.
- Usage errors print `bench-tools` usage text. Argparse wording is not a contract.

Diagnostics keep the Python program names, `roster.py:`, `diff_identity.py:`, `normalize_review.py:` and
`normalize_thermo.py:`, so the differential compares them byte for byte. Renaming them needs a new decision version.

## Research records

`docs/research/last-push-recut-2026-10-07/plan.py` imports `roster`. It is a saved research record whose derived
`review-plan.v1.json` is already committed, and no test loads it. [Processing retirement](processing-retirement.md#research-helpers)
classifies it as historical replay. It stays byte-unchanged. Replay it at a commit that has `bench/tools/roster.py`,
such as the source commit its inventory entry resolves to.

## Verification

```sh
cd rust/bench-tools && cargo fmt --check && cargo clippy --locked --all-targets -- -D warnings && cargo test --locked
python3 bench/tools/rust_runtime.py install --crate rust/bench-tools --select
python3 bench/tools/rust_runtime.py resolve bench-tools
python3 tools/behavior_contract.py --corpus tools/behavior_contracts/roster.v1.json
python3 tools/behavior_contract.py --corpus tools/behavior_contracts/diff-identity.v1.json --differential MAP
python3 tools/behavior_contract.py --corpus tools/behavior_contracts/normalize-review.v1.json --differential MAP
python3 tools/behavior_contract.py --corpus tools/behavior_contracts/normalize-thermo.v1.json
python3 tools/behavior_contract.py --verify-decision-history
```

`MAP` is a [command map](contracts/README.md#run-the-usage-corpus) naming the resolved executable, as the
`Rust runtime` workflow writes it. The `roster` and `normalize-thermo` corpora run the Rust executable by default; the
`diff-identity` and `normalize-review` corpora run the retained Python library by default.
