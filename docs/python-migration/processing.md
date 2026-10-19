# Deterministic processing in Rust

This records the processing stage, [#154](https://github.com/kamui/code-review-bench/issues/154), part of
[#149](https://github.com/kamui/code-review-bench/issues/149). It lands in slices that follow the
[delivery plan](https://github.com/kamui/code-review-bench/issues/154#issuecomment-6253814508). Slice 0,
[#192](https://github.com/kamui/code-review-bench/issues/192), retires modules with no current caller, as
[processing retirement](processing-retirement.md) records. Slice 1,
[#193](https://github.com/kamui/code-review-bench/issues/193), adds the crate and ports `roster` and `diff_identity`. Slice 5,
[#197](https://github.com/kamui/code-review-bench/issues/197), adds the schema validator and ports the read-only
readers of `rates` and `upstream`.

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
- `pyregex` is `re.search` for the pattern syntax the schemas use, `pyunicode` holds Python's decimal digits and
  `int(str)`, and `pyfs` holds `Path.resolve`, `relative_to` and directory listing.

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
| `bench/tools/check_manifest.py` | `bench_tools::schema`, `bench-tools check-manifest` | None; its CLI callers stay with #156, below | Retained library, below. |
| `bench/tools/rates.py`, its `check` command | `bench-tools rates check`, `bench_tools::rates` | `bun run rates:check`; `docs/rates.md` | Retained library, below. `refresh` stays in Python. |
| `bench/tools/upstream.py`, its `report` command | `bench-tools upstream report`, `bench_tools::upstream`, with the registry checks of `claims.load_registry` in `bench_tools::claims` | `bun run audit:maintainers`; `docs/maintainer-adjudication.md` | Retained library, below. `collect` stays in Python. |

`tools/test_moved_cli_callers.py` fails when a current file other than the retained module still runs a moved
command through Python. Frozen runs, imported originals and research records keep their commands.

## Retained libraries

A retained library stays byte-unchanged while a later stage still imports or hashes it. Its CLI callers move to Rust,
and CI runs the corpus against both implementations with a differential. It is removed once its removal condition holds.

| Module | Imported or hashed by | Removal condition | Owner |
| --- | --- | --- | --- |
| `bench/tools/diff_identity.py` | `provision.py` imports `identity`; `test_cache_replacements.py` imports it; `grade.runner_files()` hashes it, so a byte change refuses every prepared grading batch | `provision.py` computes diff identity through the Rust runtime ([#157](https://github.com/kamui/code-review-bench/issues/157)) and `grade.runner_files()` no longer hashes `diff_identity.py` ([#156](https://github.com/kamui/code-review-bench/issues/156)) | #157 |
| `bench/tools/check_manifest.py` | `claims`, `claim_grading`, `current_grading`, `evidence_store`, `file_attempt`, `provision`, `rates`, `reconcile_evidence`, `run_cell` and `upstream` import `validate`; `grade.runner_files()` hashes it. Its CLI runs in `bench/README.md`, `.github/workflows/rates.yml` and `test:bench`'s `--self-test` | No maintained module imports it and `grade.runner_files()` no longer hashes it. The delivery plan leaves `check_manifest` with [#156](https://github.com/kamui/code-review-bench/issues/156), which moves its CLI callers | #156 |
| `bench/tools/rates.py` | `run_cell` and `contributor_adapters` call `read_catalog` and `inspect`, and `contributor_adapters` calls `latest` and `fetch`; `rates refresh` runs in `rates:refresh` and `.github/workflows/rates.yml` | `rates refresh` and the dispatch check move to Rust ([#160](https://github.com/kamui/code-review-bench/issues/160) owns `rates.yml`) and no module imports `rates` | #160 |
| `bench/tools/upstream.py` | `claims` imports `validate_assessment` and `route`; `grade.runner_files()` hashes it; `upstream collect` is the documented snapshot capture | `claims` moves to Rust, `collect` has a replacement and `grade.runner_files()` no longer hashes `upstream.py` ([#156](https://github.com/kamui/code-review-bench/issues/156)) | #156 |

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
- Usage errors print `bench-tools` usage text. Argparse wording is not a contract.
- `check-manifest` and `bench_tools::schema` refuse with exit 2, as for an unsupported keyword: a schema that is not
  an object, a keyword value of a JSON type the check cannot use, such as `"required": 1`, and `$ref` chains or
  nesting deeper than 900 checks. Python raised a traceback, exit 1. A text or object `required` or `enum` is
  iterated as Python iterates it.
- A `pattern` outside `pyregex`'s syntax, such as a lookaround, `(?:...)` or a scoped flag, exits 2 naming the
  pattern, where Python applied it. `check-manifest-unsupported-pattern` pins this change, decided in
  `check-manifest-decisions.v2.json`. `\w`, `\b`, their negations, and the global flags `(?i)`, `(?m)`, `(?s)`,
  `(?x)` and `(?u)` match as Python 3.14 matches them, from tables Python 3.14.7 generated; `\B` matches an
  empty text, which Python 3.12 does not. Every pattern in a tracked `*.schema.json` is inside the syntax.
- A schema or manifest that is not UTF-8 exits 2 with the `UnicodeDecodeError` text. Python raised a traceback.
- `\d` and `int()` accept the decimal digits of Unicode 16.0, as Python 3.14 does; Python 3.12 lacks the digits
  16.0 added. Lower-casing an Anthropic model or a repository name uses Rust's Unicode tables.
- `rates check` fetches `https` pages with `curl --disable --fail --location --max-time 20`, so no curl
  configuration file applies, and a network or HTTP failure prints curl's message where urllib printed its own;
  both exit 2. `rates-transport-failure` pins this change, decided in `rates-decisions.v2.json`, by connecting to a
  closed loopback port. `file:` URLs, in any letter case, are read as urllib reads them.
  `--source PROVIDER=URL` is new. A successful `--json` receipt is outside the corpus because `checked_at` is the
  current time. A date out of range uses Python 3.14's `datetime` wording, and an integer price too large for a
  float exits 2 where Python raised `OverflowError`. `rates refresh` is refused with a pointer to `rates.py`.
- `upstream report` exits 1 for every failure, as `upstream.py` did, including inputs that made Python raise a
  traceback; the message approximates the traceback's last line. It reads the repository at the current directory
  unless `--root` names another. Listing `bench/targets` follows Python 3.14's glob, which keeps a dangling
  `target.json` link that 3.12 drops. Between snapshot files of equal version, such as `snapshot.v1.json` and
  `snapshot.v01.json`, both take the first the directory lists, which no fixture can fix.

Diagnostics keep the Python program names, `roster.py:`, `diff_identity.py:`, `check_manifest.py:`, `rates.py:`
and `upstream.py:`, so the differential compares them byte for byte. Renaming them needs a new decision version.

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
python3 tools/behavior_contract.py --corpus tools/behavior_contracts/check-manifest.v1.json --differential MAP
python3 tools/behavior_contract.py --corpus tools/behavior_contracts/rates.v1.json --differential MAP
python3 tools/behavior_contract.py --corpus tools/behavior_contracts/upstream.v1.json --differential MAP
python3 tools/behavior_contract.py --verify-decision-history
```

`MAP` is a [command map](contracts/README.md#run-the-usage-corpus) naming the resolved executable, as the
`Rust runtime` workflow writes it. The `roster` corpus runs the Rust executable by default; the other corpora run
the retained Python libraries by default. `rates.py` and `upstream.py` run through drivers in
`tools/behavior_contract.py` that point them at the case directory, as the Rust commands default to the current
directory, and give `rates.py` the `--source` pages.
