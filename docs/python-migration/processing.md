# Deterministic processing in Rust

This records the processing stage, [#154](https://github.com/kamui/code-review-bench/issues/154), part of
[#149](https://github.com/kamui/code-review-bench/issues/149). It lands in slices that follow the
[delivery plan](https://github.com/kamui/code-review-bench/issues/154#issuecomment-6253814508). Slice 0,
[#192](https://github.com/kamui/code-review-bench/issues/192), retires modules with no current caller, as
[processing retirement](processing-retirement.md) records. Slice 1,
[#193](https://github.com/kamui/code-review-bench/issues/193), adds the crate and ports `roster` and `diff_identity`.
Slice 3, [#195](https://github.com/kamui/code-review-bench/issues/195), ports `derive_packet`.

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
python3 bench/tools/rust_runtime.py exec bench-tools -- derive-packet <packet.legacy.md> --out <packet.md>
```

## Migrated responsibilities

| Python | Rust | Callers moved | Python implementation |
| --- | --- | --- | --- |
| `bench/tools/roster.py` | `bench-tools roster` | `bun run roster` in `AGENTS.md`, `README.md` and `bench/README.md` | Deleted with `test_roster.py`, whose cases are in [`roster.v1.json`](../../tools/behavior_contracts/roster.v1.json). |
| `bench/tools/diff_identity.py` | `bench-tools diff-identity`, `bench_tools::diff_identity` | The CLI in `bench/README.md` | Retained library, below. |
| `bench/tools/derive_packet.py` | `bench-tools derive-packet`, `bench_tools::derive_packet` | `build_packet.py --factual`, through the bridge below; `bench/README.md` | Deleted. Its self-test cases and every target's legacy packet are in [`derive-packet.v1.json`](../../tools/behavior_contracts/derive-packet.v1.json). The imported original is preserved at `artifacts/import-source/bench/tools/derive_packet.py`. |

## Retained libraries

A retained library stays byte-unchanged while a later stage still imports or hashes it. Its CLI callers move to Rust,
and CI runs the corpus against both implementations with a differential. It is removed once its removal condition holds.

| Module | Imported or hashed by | Removal condition | Owner |
| --- | --- | --- | --- |
| `bench/tools/diff_identity.py` | `provision.py` imports `identity`; `test_cache_replacements.py` imports it; `grade.runner_files()` hashes it, so a byte change refuses every prepared grading batch | `provision.py` computes diff identity through the Rust runtime ([#157](https://github.com/kamui/code-review-bench/issues/157)) and `grade.runner_files()` no longer hashes `diff_identity.py` ([#156](https://github.com/kamui/code-review-bench/issues/156)) | #157 |

## Temporary bridges

A bridge lets a Python caller run a Rust command until the caller itself moves to Rust.

| Bridge | Removal condition | Owner |
| --- | --- | --- |
| `build_packet.py --factual` writes its rendering to a temporary file and runs `bench-tools derive-packet --no-newline-translation` on it, through `rust_runtime.resolve`, so a carriage return in the rendering stays as the in-process call it replaces kept it. It exits 2 when `bench-tools` is not installed for `HEAD`. | `build_packet` moves to Rust and calls `bench_tools::derive_packet::derive` in-process ([#196](https://github.com/kamui/code-review-bench/issues/196)) | #196 |

The `build_packet --factual` tests skip when `bench-tools` is not installed for `HEAD`. The `Rust runtime` workflow
installs it and sets `RUST_RUNTIME_REQUIRE_BENCH_TOOLS=1`, so they fail there instead of skipping.

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
  the noncharacters, because the crate carries no table of assigned code points.
- `diff-identity` has no `--self-test`; `cargo test` and the corpus cover it. It accepts options between
  positional arguments, which argparse refuses for some orders.
- `derive-packet` has no `--self-test`; `cargo test` and the corpus cover it.
- Usage errors print `bench-tools` usage text. Argparse wording is not a contract.

`derive-packet-invalid-utf8` pins one intended change: an input that is not UTF-8 exits 2 with a message naming the
file. `derive_packet.py` raised an uncaught `UnicodeDecodeError`, exiting 1 with a traceback.

`derive-packet-unicode-16-digit-title` and `derive-packet-unicode-16-digit-duplicate-title` pin another, recorded in
`derive-packet-decisions.v5.json`: the issue number in the H1 matches the Unicode 16.0 decimal digits, as Python
3.14's `\d` does. `derive_packet.py` followed the running interpreter's Unicode database, so it missed every digit
that database lacked: the 80 added in Unicode 16.0 on CI's Python 3.12, and 110 on Python 3.10, 30 of which predate
Unicode 16.0.

Diagnostics keep the Python program names, `roster.py:`, `diff_identity.py:` and `derive_packet.py:`, so saved
stderr stays identical and the differential compares them byte for byte. Renaming them needs a new decision version.

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
python3 tools/behavior_contract.py --corpus tools/behavior_contracts/derive-packet.v1.json
(cd bench/tools && RUST_RUNTIME_REQUIRE_BENCH_TOOLS=1 python3 -m unittest -k factual test_build_packet)
python3 tools/behavior_contract.py --verify-decision-history
```

`MAP` is a [command map](contracts/README.md#run-the-usage-corpus) naming the resolved executable, as the
`Rust runtime` workflow writes it. The `roster` and `derive-packet` corpora run the Rust executable by default; the
`diff-identity` corpus runs the retained Python library by default. `derive-packet` has no Python implementation
left to compare with, so it has no differential.
