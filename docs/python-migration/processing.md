# Deterministic processing in Rust

This records the processing stage, [#154](https://github.com/kamui/code-review-bench/issues/154), part of
[#149](https://github.com/kamui/code-review-bench/issues/149). It lands in slices that follow the
[delivery plan](https://github.com/kamui/code-review-bench/issues/154#issuecomment-6253814508). Slice 0,
[#192](https://github.com/kamui/code-review-bench/issues/192), retires modules with no current caller, as
[processing retirement](processing-retirement.md) records. Slice 1,
[#193](https://github.com/kamui/code-review-bench/issues/193), adds the crate and ports `roster` and `diff_identity`.
Slice 2, [#194](https://github.com/kamui/code-review-bench/issues/194), ports the review normalizers and moves their
runner and dispatch callers, as [the normalizers](#the-review-normalizers) describes.
Slice 3, [#195](https://github.com/kamui/code-review-bench/issues/195), ports `derive_packet`. Slice 4,
[#196](https://github.com/kamui/code-review-bench/issues/196), ports `build_packet`. Slice 5,
[#197](https://github.com/kamui/code-review-bench/issues/197), adds the schema validator and ports the read-only
readers of `rates` and `upstream`. Slice 6, [#198](https://github.com/kamui/code-review-bench/issues/198), ports the
explorer export, as [the explorer export](#the-explorer-export) describes. Slice 7,
[#199](https://github.com/kamui/code-review-bench/issues/199), records the resulting ownership in the
[v3 inventory](README.md#reproducible-records) and closes the stage, as [the close-out](#close-out) lists. The
storage stage, [#155](https://github.com/kamui/code-review-bench/issues/155), adds its commands to the same crate;
[storage](storage.md) records them.

## The `bench-tools` crate

[`rust/bench-tools`](../../rust/bench-tools) builds one binary, `bench-tools`, with one subcommand per tool and one
library module per tool in `src/`. Later slices add their tools to it. Its lints, toolchain and release profile are
those of the `rust/usage-pilot` pilot, since folded into `bench-tools usage` and removed. It may use pinned registry dependencies, listed under
[registry dependencies](#registry-dependencies), through its committed `Cargo.lock`; a first install needs the
network once to fetch them (`cargo fetch --locked` in `rust/bench-tools`, as the README's Run locally steps do).
Shared modules carry the Python behavior that byte-pinned outputs depend on:

- `pyjson` reads JSON as `json.loads` does and writes it as `json.dumps` does, including `float.__repr__` spelling,
  integers of any length up to Python's 4300-digit limit, key order, and `JSONDecodeError` messages. Both halves
  stay hand-written ([#215](https://github.com/kamui/code-review-bench/issues/215) slice C). `serde_json` 1.0.154
  refuses `NaN` and `Infinity` and has no option to accept them. Its error messages and positions differ from
  `JSONDecodeError`'s, which normalize-review notes save. By default it stops at 128 levels of nesting, and its only
  depth option removes the limit; `pyjson` stops at 1000. It refuses a lone surrogate as `pyjson` does, with another
  message. Its writer writes `null` for `NaN`, `1.5e-7` for `1.5e-07` and non-ASCII text unescaped. A wrapper for
  these cases would keep the whole scanner beside the crate. `loads_os_text` reads a string holding the `\udcXX`
  escapes that `os.fsdecode` writes for undecodable bytes, value or object key, as an `OsText` holding those bytes,
  which the writer spells the same way; it refuses escapes for bytes that decode, which `os.fsdecode` never writes.
- `pytext` holds `surrogateescape` decoding, `str.strip` whitespace, `repr`, `pathlib` path joining, universal
  newlines, and the `OSError` and `UnicodeDecodeError` messages that diagnostics quote.
- `sha256` spells the `sha2` crate's SHA-256 digest as `hashlib`'s `hexdigest()` does, in one call or incrementally.
- `pyregex` is `re.search` for the pattern syntax the schemas use, matched by the `regex` crate, `pyunicode` holds
  Python's decimal digits and `int(str)`, and `pyfs` holds `Path.resolve`, `relative_to` and directory listing.
- `pyobject` holds Python's comparison, membership, indexing, `len`, iteration, `set` and `sorted` on JSON values,
  failing where Python raises, so rules ported from Python reach the same result on malformed input.
- `archive` reads and writes gzip, tar and zip through the [registry dependencies](#registry-dependencies), as [archives](storage.md#archives)
  describes.

Run a subcommand through the pinned executable, as [Rust runtime pinning](runtime.md#running-a-maintained-command)
describes:

```sh
(cd rust/bench-tools && cargo fetch --locked)
python3 bench/tools/rust_runtime.py install --crate rust/bench-tools --select
bun run roster
python3 bench/tools/rust_runtime.py exec bench-tools -- diff-identity <repo> <base> <head>
python3 bench/tools/rust_runtime.py exec bench-tools -- derive-packet <packet.legacy.md> --out <packet.md>
python3 bench/tools/rust_runtime.py exec bench-tools -- build-packet --repo <owner/name> --pr <n> ... --out <packet.md>
bun run data
```

## Registry dependencies

The owner allowed pinned crates.io dependencies on 2026-10-19 ([#215](https://github.com/kamui/code-review-bench/issues/215)).
These are every registry crate the four supported targets build, as `cargo tree --target <target> -e normal,build`
lists them. When the Cargo cache lacks one, `rust_runtime.py install` fails and names the `cargo fetch --locked` step.

| Crate | Version | License | Purpose |
| --- | --- | --- | --- |
| `clap` | 4.6.7 | MIT OR Apache-2.0 | Parses every subcommand's arguments, as [command-line parsing](#command-line-parsing) describes; a direct dependency built with features `derive`, `std`, `help`, `usage` and `error-context` only |
| `clap_builder` | 4.6.7 | MIT OR Apache-2.0 | The parser behind `clap` |
| `clap_derive` | 4.6.7 | MIT OR Apache-2.0 | Build-time derive macros for `clap` |
| `clap_lex` | 1.1.1 | MIT OR Apache-2.0 | Argument lexing for `clap_builder` |
| `anstyle` | 1.0.14 | MIT OR Apache-2.0 | Text styles for `clap_builder` |
| `heck` | 0.5.0 | MIT OR Apache-2.0 | Build-time case conversion for `clap_derive` |
| `proc-macro2` | 1.0.107 | MIT OR Apache-2.0 | Build-time token handling for `clap_derive` |
| `quote` | 1.0.47 | MIT OR Apache-2.0 | Build-time code generation for `clap_derive` |
| `syn` | 3.0.9 | MIT OR Apache-2.0 | Build-time Rust parsing for `clap_derive` |
| `unicode-ident` | 1.0.27 | (MIT OR Apache-2.0) AND Unicode-3.0 | Build-time identifier tables for `proc-macro2` |
| `sha2` | 0.11.1 | MIT OR Apache-2.0 | SHA-256 for `bench_tools::sha256`; a direct dependency built without default features |
| `sha1_smol` | 1.0.1 | BSD-3-Clause | Git blob SHA-1 in `evidence inventory` ([#222](https://github.com/kamui/code-review-bench/issues/222)); a direct dependency built without default features, with no dependencies |
| `digest` | 0.11.3 | MIT OR Apache-2.0 | Hash traits for `sha2` |
| `crypto-common` | 0.2.2 | MIT OR Apache-2.0 | Shared cryptographic types for `digest` |
| `block-buffer` | 0.12.1 | MIT OR Apache-2.0 | Block buffering for `digest` |
| `hybrid-array` | 0.4.15 | MIT OR Apache-2.0 | Fixed-size arrays for `digest` and `block-buffer` |
| `typenum` | 1.20.1 | MIT OR Apache-2.0 | Type-level sizes for `hybrid-array` |
| `cpufeatures` | 0.3.1 | MIT OR Apache-2.0 | Runtime CPU feature detection for `sha2` |
| `regex`, with only its `std` feature | 1.13.1 | MIT OR Apache-2.0 | Matches `pyregex`'s translation of a schema pattern ([#215](https://github.com/kamui/code-review-bench/issues/215)) |
| `regex-automata` | 0.4.18 | MIT OR Apache-2.0 | `regex`'s engines |
| `regex-syntax` | 0.8.11 | MIT OR Apache-2.0 | `regex`'s parser, without its Unicode tables |
| `ureq` | 3.4.2 | MIT OR Apache-2.0 | `rates check`'s HTTP client, a direct dependency; built without default features except `rustls` |
| `ureq-proto` | 0.6.5 | MIT OR Apache-2.0 | HTTP/1.1 protocol state for `ureq` |
| `http` | 1.5.0 | MIT OR Apache-2.0 | Request, response and status types for `ureq` |
| `httparse` | 1.10.1 | MIT OR Apache-2.0 | HTTP response parsing for `ureq-proto` |
| `bytes` | 1.12.1 | MIT | Byte buffers for `http` |
| `itoa` | 1.0.18 | MIT OR Apache-2.0 | Integer formatting for `http` |
| `base64` | 0.23.1 | MIT OR Apache-2.0 | Basic credentials for proxies and URL user info in `ureq` |
| `percent-encoding` | 2.3.2 | MIT OR Apache-2.0 | URL encoding for `ureq` |
| `utf8-zero` | 0.8.1 | MIT OR Apache-2.0 | Lossy UTF-8 body decoding in `ureq` |
| `log` | 0.4.34 | MIT OR Apache-2.0 | Logging facade for `ureq` and `rustls`; nothing installs a logger |
| `rustls` | 0.23.46 | Apache-2.0 OR ISC OR MIT | TLS for `https` pages |
| `rustls-webpki` | 0.103.15 | ISC | Certificate verification for `rustls` |
| `rustls-pki-types` | 1.15.1 | MIT OR Apache-2.0 | Certificate and key types for `rustls` |
| `webpki-roots` | 1.0.9 | CDLA-Permissive-2.0 | The Mozilla root certificates `https` pages are checked against |
| `ring` | 0.17.14 | Apache-2.0 AND ISC | Cryptography for `rustls`; its build script compiles C and assembly with the host C compiler |
| `cc` | 1.9.0 | MIT OR Apache-2.0 | Build-time C compiler driver for `ring` |
| `find-msvc-tools` | 0.1.15 | MIT OR Apache-2.0 | Build-time dependency of `cc`, used only on Windows |
| `shlex` | 2.0.1 | MIT OR Apache-2.0 | Build-time flag splitting for `cc` |
| `getrandom` | 0.2.17 | MIT OR Apache-2.0 | Operating-system randomness for `ring` |
| `libc` | 0.2.191 | MIT OR Apache-2.0 | System calls for `getrandom`, `tar`, `filetime` and `nix` |
| `cfg-if` | 1.0.5 | MIT OR Apache-2.0 | Platform selection for `ring`, `getrandom`, `crc32fast`, `filetime` and `nix` |
| `untrusted` | 0.9.0 | ISC | Safe input parsing for `ring` and `rustls-webpki` |
| `subtle` | 2.6.1 | BSD-3-Clause | Constant-time comparison for `rustls` |
| `zeroize` | 1.9.1 | Apache-2.0 OR MIT | Clearing secrets for `rustls` and `rustls-pki-types` |
| `once_cell` | 1.21.4 | MIT OR Apache-2.0 | Lazy statics for `rustls` |
| `flate2` | 1.1.10 | MIT OR Apache-2.0 | gzip and deflate for `archive` ([#218](https://github.com/kamui/code-review-bench/issues/218)), a direct dependency; built with `rust_backend` only |
| `tar` | 0.4.46 | MIT OR Apache-2.0 | tar with PAX for `archive`, a direct dependency; built without default features |
| `zip` | 9.0.5 | MIT | Native scratch archives for `archive`, a direct dependency; built with `deflate-flate2` only |
| `miniz_oxide` | 0.9.1 | MIT OR Zlib OR Apache-2.0 | Pure-Rust deflate for `flate2` |
| `adler2` | 2.0.1 | 0BSD OR MIT OR Apache-2.0 | Adler-32 for `miniz_oxide` |
| `simd-adler32` | 0.3.10 | MIT | Vectorized Adler-32 for `miniz_oxide` |
| `crc32fast` | 1.5.2 | MIT OR Apache-2.0 | CRC-32 for `flate2` and `zip` |
| `filetime` | 0.2.29 | MIT OR Apache-2.0 | File times for `tar` |
| `indexmap` | 2.14.2 | MIT OR Apache-2.0 | Ordered entry map for `zip` |
| `hashbrown` | 0.17.1 | MIT OR Apache-2.0 | Hash table for `indexmap` |
| `equivalent` | 1.0.3 | MIT OR Apache-2.0 | Key comparison for `indexmap` |
| `memchr` | 2.8.3 | Unlicense OR MIT | Byte search for `zip` |
| `typed-path` | 0.12.3 | MIT OR Apache-2.0 | Entry path handling for `zip` |
| `nix` | 0.31.3 | MIT | `statvfs` for `regrade status`'s free space ([#239](https://github.com/kamui/code-review-bench/issues/239)), and blocking `SIGINT` while the explorer export publishes (#234), a direct dependency built with `fs` and `signal` only, since the crate forbids `unsafe`; on macOS also Unix calls for `zip` |
| `bitflags` | 2.13.2 | MIT OR Apache-2.0 | Flag types for `nix` |
| `cfg_aliases` | 0.2.2 | MIT | Build-time configuration for `nix` |

`Cargo.lock` also lists `aho-corasick` 1.1.5 (Unlicense OR MIT), an optional dependency of `regex` that its disabled
`perf` feature would build.

## Migrated responsibilities

| Python | Rust | Callers moved | Python implementation |
| --- | --- | --- | --- |
| `bench/tools/roster.py` | `bench-tools roster` | `bun run roster` in `AGENTS.md`, `README.md` and `bench/README.md` | Deleted with `test_roster.py`, whose cases are in [`roster.v1.json`](../../tools/behavior_contracts/roster.v1.json). |
| `bench/tools/diff_identity.py` | `bench-tools diff-identity`, `bench_tools::diff_identity` | The CLI in `bench/README.md` | Retained library, below. |
| `bench/tools/normalize_review.py` | `bench-tools normalize-review`, `bench_tools::normalize_review` | `dispatch.sh`, `claude_skill_runner.py`, `codex_skill_runner.py`, `file_attempt.py --replay` and the `test:bench` script | Retained library, below. `test_normalize_review.py`'s cases are in [`normalize-review.v1.json`](../../tools/behavior_contracts/normalize-review.v1.json), which `test:bench` runs against the retained Python. |
| `bench/tools/normalize_thermo.py` | `bench-tools normalize-thermo`, `bench_tools::normalize_thermo` | The same skill runners and `file_attempt.py --replay` | Deleted with `test_normalize_thermo.py`, whose cases are in [`normalize-thermo.v1.json`](../../tools/behavior_contracts/normalize-thermo.v1.json). |
| `bench/tools/derive_packet.py` | `bench-tools derive-packet`, `bench_tools::derive_packet` | `bench-tools build-packet --factual`, in-process; `bench/README.md` | Deleted. Its self-test cases and every target's legacy packet are in [`derive-packet.v1.json`](../../tools/behavior_contracts/derive-packet.v1.json). The imported original is preserved at `artifacts/import-source/bench/tools/derive_packet.py`. |
| `bench/tools/build_packet.py` | `bench-tools build-packet`, `bench_tools::build_packet` | `bench/README.md`, `docs/pr-selection.md` | Deleted with `test_build_packet.py` and `test_build_packet_fixture.json`. Every run the tests made, with its replay files and staging mirror, is a case in [`build-packet.v1.json`](../../tools/behavior_contracts/build-packet.v1.json), beside 36 more. The imported originals are preserved under `artifacts/import-source/bench/tools/`. |
| `bench/tools/check_manifest.py` | `bench_tools::schema`, `bench-tools check-manifest` | `bench/README.md`, `docs/current-grading.md`, `.github/workflows/rates.yml`; `test:bench` runs [`check-manifest.v1.json`](../../tools/behavior_contracts/check-manifest.v1.json) against Rust, which [#234](https://github.com/kamui/code-review-bench/issues/234) moved from the Python CLI | Retained library, below. |
| `bench/tools/rates.py`, its `check` command | `bench-tools rates check`, `bench_tools::rates` | `bun run rates:check`; `docs/rates.md` | Retained library, below. `refresh` stays in Python. |
| `bench/tools/upstream.py`, its `report` command | `bench-tools upstream report`, `bench_tools::upstream`, with the registry checks of `claims.load_registry` in `bench_tools::claims` | `bun run audit:maintainers`; `docs/maintainer-adjudication.md` | Retained library, below. `collect` moved to `bench-tools upstream collect` with #235. |
| `tools/export_explorer.py`, its export | `bench-tools export-explorer`, and its `evidence`, `stage` and `publish`, `bench_tools::export_explorer` | `bun run data`; `.github/workflows/pages.yml` installs `bench-tools`; `src/lib/scoring.test.ts` passes `--root` and `--public` | Deleted with `test_export_explorer.py` by #234, whose cases are in `tests/export_explorer.rs`, the unit tests and [`export-explorer.v1.json`](../../tools/behavior_contracts/export-explorer.v1.json). |

[Storage](storage.md#migrated-responsibilities) lists the responsibilities #155 moved.
`tools/test_moved_cli_callers.py` fails when a current file other than the retained module still runs a moved
command through Python. Frozen runs, imported originals and research records keep their commands.

## Retained libraries

A retained library stays byte-unchanged while a later stage still imports or hashes it. Where #154 ported it, its CLI
callers moved to Rust and CI runs the corpus against both implementations with a differential. `compare.py` and
`stage_edition.py` have no Rust port; [processing retirement](processing-retirement.md) kept them for their callers.
A retained library is removed by the stage in the last column once its removal condition holds, and the v4 inventory
assigns it to that stage. [Storage](storage.md#retained-libraries) lists the libraries #155 retains.

| Module | Imported or hashed by | Removal condition | Removed by |
| --- | --- | --- | --- |
| `bench/tools/diff_identity.py` | `provision.py` imports `identity`; `test_cache_replacements.py` imports it; `grade.runner_files()` hashes it, so a byte change refuses every prepared grading batch | `provision.py` computes diff identity through the Rust runtime ([#157](https://github.com/kamui/code-review-bench/issues/157)) and `grade.runner_files()` no longer hashes `diff_identity.py` ([#156](https://github.com/kamui/code-review-bench/issues/156)) | #157 |
| `bench/tools/normalize_review.py` | `grade.py` imports `render` for blind grading; `contributor_adapters.collect` calls `from_codex`, `from_claude_builtin` and `from_codex_skill` in process; `grade.runner_files()` hashes it | `grade.render` renders through the Rust runtime and `grade.runner_files()` no longer hashes `normalize_review.py` ([#156](https://github.com/kamui/code-review-bench/issues/156)), and the contributor backend normalizes through it ([#158](https://github.com/kamui/code-review-bench/issues/158)) | #158 |
| `bench/tools/check_manifest.py` | `claims`, `claim_grading`, `current_grading`, `evidence_store`, `file_attempt`, `provision`, `rates`, `run_cell` and `upstream` import `validate`; `grade.runner_files()` hashes it. #234 moved its CLI callers to `bench-tools check-manifest` | No maintained module imports it and `grade.runner_files()` no longer hashes it. The delivery plan leaves `check_manifest` with [#156](https://github.com/kamui/code-review-bench/issues/156), which moves its grading importers; `provision`, `run_cell` and `file_attempt` leave with [#157](https://github.com/kamui/code-review-bench/issues/157), `evidence_store` with [#159](https://github.com/kamui/code-review-bench/issues/159), and `rates`, the last importer, with [#160](https://github.com/kamui/code-review-bench/issues/160) | #160 |
| `bench/tools/rates.py` | `run_cell` and `contributor_adapters` call `read_catalog` and `inspect`, and `contributor_adapters` calls `latest` and `fetch`; `rates refresh` runs in `rates:refresh` and `.github/workflows/rates.yml` | `rates refresh` and the dispatch check move to Rust ([#160](https://github.com/kamui/code-review-bench/issues/160) owns `rates.yml`) and no module imports `rates` | #160 |
| `bench/tools/upstream.py` | `claims` imports `validate_assessment` and `route`; `grade.runner_files()` hashes it; `upstream collect` is the documented snapshot capture | `claims` moves to Rust, `collect` has a replacement and `grade.runner_files()` no longer hashes `upstream.py` ([#156](https://github.com/kamui/code-review-bench/issues/156)) | #156 |
| `bench/tools/compare.py` | The research run helpers `replacement_runs.py` and `skill_replacement_runs.py`, which `stage_edition.staged_registry()` executes, and `review_code_run.py` call `provisioning_hash` for a run manifest's `provisioning_sha256`; the #156 filing tests reach it through `move.py` and `selection.py`. Its comparison CLI is documented in `bench/README.md` | No current test or command runs those helpers from the current checkout ([#156](https://github.com/kamui/code-review-bench/issues/156)), and another command computes `provisioning_sha256` for new run freezes ([#157](https://github.com/kamui/code-review-bench/issues/157)) | #157 |
| `docs/research/last-push-recut-2026-10-07/stage_edition.py` | The #156 filing scripts `last-push-rulings-2026-10-11/filing/move.py` and `last-push-review-code-2026-10-12/selection/selection.py` import it; `test_packet_replacements.StagedSelectionTests` loads it | A research record, so it is never deleted. It becomes historical replay when no current test or command loads it or those filing scripts ([#156](https://github.com/kamui/code-review-bench/issues/156)) | #156 |

`grade.runner_files()` also hashes 19 files that no #154 responsibility ported, such as `provision.py`, `claims.py`
and `grading-hosts.v1`. All 23 are byte-identical at `bd1fde436`, where the delivery plan was checked, and at the v3
inventory's source commit. The v3 inventory assigns each Python file to its own stage.

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

## Temporary bridges

A bridge lets a Python caller run a Rust command until the caller itself moves to Rust. [Storage](storage.md#temporary-bridges)
lists the contributor CLI and `file_attempt.py` bridges.

#195's bridge, `build_packet.py --factual` running `bench-tools derive-packet` through `rust_runtime.resolve`, was
removed by #196: `bench-tools build-packet --factual` calls `bench_tools::derive_packet::derive` in-process. It
cannot exit 2 for a missing `bench-tools`, as the bridge did, because it is `bench-tools`. `derive-packet` keeps
`--no-newline-translation`, which the bridge passed, because `derive-packet-no-newline-translation` pins it and
decision history cannot remove a case.

#156's bridge, `tools/export_explorer.py` loading the current facts with `current_grading.load_current` and passing
them to `stage` in a temporary file, was removed by #234: `bench-tools export-explorer` with no command loads them in
process, as [the explorer export](#the-explorer-export) describes.

## The explorer export

`bench-tools export-explorer` writes the same `public/data` and `public/evidence` bytes as the Python export it
replaces. `src/lib/scoring.ts` stays the only owner of scorecard arithmetic; the export computes no score, rate or
ranking.

- With no command, `export-explorer [--root R] [--public P]` runs the whole export, as `tools/export_explorer.py`
  did. It loads the current facts in process with `current_grading::load_current` and `coverage_status`, refusing
  what the front's `json.dumps(allow_nan=False)` and facts file refused, restores the evidence they link, stages,
  reads the record again and refuses when the dataset hash changed, then publishes with `SIGINT` blocked. It reads
  `--base-path` from `BASE_PATH`, as the front did; `bun run data` runs it through `rust_runtime.py exec`. With no
  `bench-tools` selected for `HEAD`, `exec` prints the install command and exits 1, as `test_rust_runtime.py` pins
  for every `exec` caller. The deleted front exited 2 there, but `bun run data` never reached it: its
  `evidence:fetch` step already refused with exit 1, so the script's exit status is unchanged.

- `evidence` lists every file the export links, with its frozen SHA-256 or null, in the order the export first
  names it, as JSON on stdout. The front restores the missing ones.
- `stage` refuses while `STAGE/previous` holds backups, replaces `STAGE`, writes the dataset, the attempt details
  and the linked evidence, and checks them as `validate_export` did. It prints the whole export's summary line.
- `publish` moves both directories into `PUBLIC`, restoring the previous pair when a move fails, and keeps backups
  it cannot restore in `STAGE/previous`.

Every failure exits 1 with the last line of the traceback `export_explorer.py` printed, after
`export_explorer.py: `. Usage errors exit 2. `export-explorer-decisions.v3.json` records each change from the Python
export with its case:

- `export-explorer-evidence-reads-every-input-first`: every input is read before any evidence is restored, so an
  input error is reported before a missing file.
- `export-explorer-publish-replaces-both`: `SIGINT` is ignored while `publish` runs, where Python restored the
  previous pair on `KeyboardInterrupt`.
- `export-explorer-iso-24-midnight`: review times are parsed as Python 3.14's `datetime.fromisoformat` parses them,
  which differs from Python 3.12 and 3.13 on an hour of 24 and on some malformed times.
- `export-explorer-mixed-token-sum`: output tokens are summed as Python 3.14's `sum` adds them, which differs from
  Python 3.12 and 3.13 when an integer follows a float.
- `export-explorer-unsortable-manifestations`: sorting values that are not all text is refused.
- `export-explorer-lone-surrogate-facts`: facts with a lone surrogate escape are refused, and the front refuses a
  non-finite number in them.
- `export-explorer-stage-unassessed` also records that `bench/runs/*/transcripts.json` is listed as Python 3.14's
  glob lists it, keeping a dangling link that 3.12 dropped.
- `export-explorer-unicode-15-1-adjudication-id`: a diagnostic quotes input as the `repr` paragraph below
  describes, which differs from Python 3.12 on characters added in Unicode 15.1 and 16.0.

`export-explorer-decisions.v7.json` adds the whole export's `export-explorer-current-*` cases, which agree with the
deleted front except: a record the export refuses prints the traceback's last line and keeps exit 1
(`*-billing-receipt`, `*-missing-inventory`, `*-unicode-15-1-record`); `--r` is ambiguous with the hidden
`--restore` (`*-abbreviated-root`); an option of a command given without one is refused
(`*-facts-without-command`); and a value led by `-` and a Unicode 16.0 digit is taken, as Python 3.14's argparse
took it and 3.12's refused it (`*-unicode-16-digit-public`). Dash-led `--root` and `--public` values such as `-.5`
and `'-published dir'` are taken as the front took them (`*-dash-led-*`, `*-negative-number-public`): `main.rs`
attaches them before clap parses, as for `build-packet`, so `export-explorer`'s commands take them too. It supersedes
`export-explorer-dot-digit-facts`, which v6 decided as refused, and `export-explorer-help-with-attached-argument` for
the new first help line. `tests/export_explorer.rs` covers a record changed after staging, a failed publication, `SIGINT` during
publication and `BASE_PATH`.

## Behavior the corpora do not pin

These differences from the Python implementations are outside every contract. A caller that relied on one needs a
recorded decision. [Storage](storage.md#behavior-the-corpora-do-not-pin) lists the storage commands' differences.

- `bench-tools roster` reads the repository at the current directory unless `--root` names another. `roster.py`
  read the repository that held the script. `bun run roster` runs from the repository root.
- An input whose shape made Python raise a traceback, such as a registry without `suites`, exits 2 with a message
  naming the file. Python exited 1.
- A lone surrogate escape such as `"\ud800"` and JSON nested more than 1,000 levels deep are refused, exiting 2.
  Python kept the surrogate, and its own nesting limit depends on its version and stack.
- `diff-identity` has no `--self-test`; `cargo test` and the corpus cover it. It accepts options between
  positional arguments, which argparse refuses for some orders.
- `normalize-review` has no `--self-test`.
- `derive-packet` has no `--self-test`; `cargo test` and the corpus cover it.
- Where the normalizers' Python raised a traceback and exited 1, they exit 2 with a message: an input that is not
  UTF-8, a shape such as a `findings` value that is not a list, an `--out` that cannot be written, a timing sidecar that
  is not an object, a fenced JSON array whose integer exceeds Python's digit limit, and a Codex bullet whose start or
  end line exceeds it. The v2 decisions of both normalizer corpora record each case, and the v4 review decisions add
  the Codex line numbers. In `normalize-review.v1.json` they carry the retained Python's traceback as a
  `baseline_defect`, so its corpus runs with `--allow-known-defects`.
- `\d` in the Codex bullet pattern and `\w` in the thermo no-findings statement's `\b` follow Python 3.14, whose
  Unicode database is 16.0. `pytext` holds both tables, generated from Python 3.14; `derive-packet` reads the
  same digit table and `build-packet` both. Python 3.12 does not match the
  characters Unicode 16.0 added. The v3 decisions of both normalizer corpora record that difference.
- `normalize-thermo` resolves a relative missing `--artifact-root` as Python 3.13 and later do, naming the absolute
  component in the error; Python 3.12 named the relative one. A symbolic link loop in it is an `ELOOP` error, as in
  Python 3.13 and later; Python 3.12 raised a traceback. Callers pass an absolute root.
- `--sessions-dir` does not follow symbolic links to directories; Python's `rglob` follows them in some versions.
- `build-packet` calls `gh` only without `--replay`, which no case uses. Its argv was compared once against
  `build_packet.py` through a stub `gh` (see #196's pull request); `cargo test` pins the GraphQL and edit-history
  commands.
- Usage errors and help are clap's: `error: ...` and the usage on stderr, exit 2, or help on stdout, exit 0.
  Argparse wording is not a contract.
- `check-manifest` and `bench_tools::schema` refuse with exit 2, as for an unsupported keyword: a schema that is not
  an object, a keyword value of a JSON type the check cannot use, such as `"required": 1`, and `$ref` chains or
  nesting deeper than 900 checks. Python raised a traceback, exit 1. A text or object `required` or `enum` is
  iterated as Python iterates it.
- A `pattern` outside `pyregex`'s syntax, such as a lookaround, `(?:...)` or a scoped flag, exits 2 naming the
  pattern, where Python applied it. `check-manifest-unsupported-pattern` pins this change, decided in
  `check-manifest-decisions.v2.json`. `\w`, `\b`, their negations, and the global flags `(?i)`, `(?m)`, `(?s)`,
  `(?x)` and `(?u)` match as Python 3.14 matches them, from tables Python 3.14.7 generated; `\B` matches an
  empty text, which Python 3.12 does not. Every pattern in a tracked `*.schema.json` is inside the syntax.
  `check-manifest-decisions.v3.json` records where the pinned Unicode 16.0 tables differ from an older
  interpreter: `\w` on 5,004 characters against Python 3.12, including the 622 Unicode 15.1 added, `\d` on 80
  and `(?i)` on 54, each with a case. The `regex` crate matches a translation that keeps these tables:
  `check-manifest-crate-unicode-classes` pins characters where the crate's own `\w`, `\s`, `\b` and `(?i)` would
  differ. A pattern whose translation exceeds the crate's compiled size limit, such as `^\w{1000}$`, or nests more
  than 2,000 levels of the crate's syntax, exits 2; Python applied the first and raised `RecursionError` on the
  second. No pattern Python compiles nests that deeply. `check-manifest-decisions.v6.json` records both. A pattern
  nested near 100,000 groups still overflows the parser's stack and aborts, as before.
- A schema or manifest that is not UTF-8 exits 2 with the `UnicodeDecodeError` text. Python raised a traceback.
- `\d` and `int()` accept the decimal digits of Unicode 16.0, as Python 3.14 does; Python 3.12 lacks the digits
  16.0 added. `rates-decisions.v4.json` records this for the catalog date, price footnotes and long-prompt rows,
  and `upstream-decisions.v2.json` for snapshot versions. Lower-casing an Anthropic model or a repository name
  uses Rust's Unicode tables.
- `rates check` fetches `http` and `https` pages with `ureq` over `rustls`, within 20 seconds for the whole request,
  and checks certificates against the Mozilla roots `webpki-roots` bundles, not the system trust store. An
  operating-system error and an HTTP error status print as urllib printed them, except that the reason phrase is the
  standard one for the code; a TLS or name-resolution failure prints ureq's text inside `<urlopen error ...>`. Each
  exits 2. `rates-transport-failure`, decided in `rates-decisions.v7.json`, pins a refused connection by connecting to
  a closed loopback port, and `tests/rates_http.rs` pins an HTTP error status, a redirect and a TLS failure. `file:`
  URLs, in any letter case, are read as urllib reads them. Any other scheme, such as `data:` or `ftp:`, is refused
  with exit 2, as `rates-unsupported-url-scheme` pins. ureq takes one proxy for every URL, the first set of
  `ALL_PROXY`, `HTTPS_PROXY` and `HTTP_PROXY` (each before its lower-case name), and tunnels through it with
  `CONNECT`; urllib chose `http_proxy` or `https_proxy` by scheme and ignored `ALL_PROXY`. A host that sets only
  `HTTP_PROXY` therefore sends `https` sources through it. ureq is built without its `socks-proxy` feature, but a
  `socks`, `socks4`, `socks4a`, `socks5` or `socks5h` URL in `ALL_PROXY` or `all_proxy` is still that one proxy, so
  `HTTPS_PROXY` and `HTTP_PROXY` go unread and no proxy is used: `socks`, `socks4` and `socks5` connect straight to the
  page's host, and `socks4a` and `socks5h` print `<urlopen error Connection refused>` without connecting. urllib and
  curl used `https_proxy` there. The v7 decision records this, and `tests/rates_http.rs` pins both cases.
  `--source PROVIDER=URL` is new. A successful `--json` receipt is outside the corpus because `checked_at` is the
  current time. A date out of range uses Python 3.14's `datetime` wording, and an integer price too large for a
  float exits 2 where Python raised `OverflowError`. `rates refresh` is refused with a pointer to `rates.py`.
- `upstream report` exits 1 for every failure, as `upstream.py` did, including inputs that made Python raise a
  traceback; the message approximates the traceback's last line. It reads the repository at the current directory
  unless `--root` names another. Listing `bench/targets` follows Python 3.14's glob, which keeps a dangling
  `target.json` link that 3.12 drops. Targets run in the order `pathlib` compares their paths, component by
  component, so `t-example-1` runs before `t-example-1-b`; `upstream-sibling-target-order` pins it. Between snapshot files of equal version, such as `snapshot.v1.json` and
  `snapshot.v01.json`, both take the first the directory lists, which no fixture can fix.

`derive-packet-invalid-utf8` pins one intended change: an input that is not UTF-8 exits 2 with a message naming the
file. `derive_packet.py` raised an uncaught `UnicodeDecodeError`, exiting 1 with a traceback.

`derive-packet-unicode-16-digit-title` and `derive-packet-unicode-16-digit-duplicate-title` pin another, recorded in
`derive-packet-decisions.v5.json`: the issue number in the H1 matches the Unicode 16.0 decimal digits, as Python
3.14's `\d` does. `derive_packet.py` followed the running interpreter's Unicode database, so it missed every digit
that database lacked: the 80 added in Unicode 16.0 on CI's Python 3.12, and 110 on Python 3.10, 30 of which predate
Unicode 16.0.

`build-packet` matches `build_packet.py` as Python 3.14 ran it. Where older interpreters differed, a recorded
decision pins Python 3.14: `build-packet-hour-24-cutoff`, `build-packet-month-out-of-range` and
`build-packet-dedent-whitespace-only-line`. Its Unicode classes are fixed at Unicode 16.0, Python 3.14's database:
`\d` and `\w` in the closing-reference pattern, `\d` in the instant pattern, `int()` of `--pr` and `--ref-pr`, and
`str.lower()` of repository names. `build-packet-unicode-16-closing-digit`, `build-packet-unicode-16-word-character`
and `build-packet-unicode-16-lowercase-letter` pin one character added in Unicode 16.0 each, and their v3 decisions
state the impact against Python 3.12 (80 decimal digits, 5,004 word characters, 96 lowercase code points) and Python
3.10 (110, 9,917 and 510).

The decisions also record eight intended changes from `build_packet.py`, each with its case. An input that made Python
raise an uncaught exception exits 2 and names the exception: `build-packet-missing-merged-at`,
`build-packet-null-author` and `build-packet-overflowing-cutoff`. A replay file or extra section that is not UTF-8
exits 2 naming the file: `build-packet-invalid-utf8-replay` and `build-packet-invalid-utf8-extra-section`. A lone
surrogate escape and JSON nested deeper than 1,000 levels are refused even in a field the packet never renders:
`build-packet-lone-surrogate` and `build-packet-deeply-nested-replay`. A text option that is not UTF-8 is a usage
error: `build-packet-non-utf8-text-option`. A ninth, `build-packet-repr-unassigned-code-point`, printed a record
name's unassigned code point as is until #207; v5 supersedes it with Python's escaped bytes.

`build-packet-decisions.v4.json` adds the four `build-packet-unicode-digit-basic-offset-*` cases: a basic offset whose
hour units are a non-ASCII digit, in `--cutoff`, `--pushed-at`, the forge's `mergedAt` and its force-push event. They
match `build_packet.py`, which inserts the offset colon by character position and lets `datetime.fromisoformat`
refuse the result.

`repr` in a bench-tools diagnostic escapes exactly the code points Python 3.14's `str.isprintable()` rejects,
unassigned ones included. Python escaped the characters its own Unicode database lacked, so the Rust output differs on
5,812 code points added in Unicode 15.1 and 16.0 from CI's Python 3.12, and on 11,130 from Python 3.10. Each family
whose diagnostics quote input records this with a U+2EBF0 case: `derive-packet-unicode-15-1-path`,
`build-packet-unicode-15-1-record-name`, `roster-unicode-15-1-arm-kind`, `thermo-unicode-15-1-report-path`,
`claude-skill-unicode-15-1-report-status`, `check-manifest-unicode-15-1-path`, `rates-unicode-15-1-source-path`,
`upstream-unicode-15-1-registry-path`, `diff-identity-unicode-15-1-git-record`,
`export-explorer-unicode-15-1-adjudication-id`, `fetch-unicode-15-1-path`, `inventory-unicode-15-1-output-parent`,
`collect-run-unicode-15-1-record` and `reconcile-unicode-15-1-inventory-path`. `export-explorer-current-unicode-15-1-record` and `methodology-unicode-15-1-repr` pin U+2FFC, added in Unicode 15.1. The seven whose Python is retained
carry Python 3.12's escaped output as their `baseline_defect`. `derive-packet-unassigned-path` pins U+0378, which
every Python version escapes.

Diagnostics keep the Python program names, `roster.py:`, `diff_identity.py:`, `normalize_review.py:`,
`normalize_thermo.py:`, `derive_packet.py:`, `build_packet:`, `check_manifest.py:`, `rates.py:`, `upstream.py:`,
`export_explorer.py:`, `methodology.py:`, `calibration.py:`, `evaluator_audit.py:`, `collect_run.py:`, `reconcile_evidence.py:` and `inventory:`, so saved stderr stays identical and the differential compares them byte for byte. Renaming them
needs a new decision version.

## Command-line parsing

`main.rs` parses arguments with `clap`'s derive API ([#215](https://github.com/kamui/code-review-bench/issues/215)).
Every subcommand expands a unique prefix of a long option, as argparse does, through `infer_long_args`; the top level
does not. A usage error exits 2 and help exits 0. No corpus reaches the top level, so `tests/cli.rs` pins its exit
codes as the hand-written parser had them; `main.rs` refuses `bench-tools -hx` before clap, which would print help.
Each family's latest decisions record where clap reads a subcommand's command line differently from the parser it
replaced, each with its case:

- `*-abbreviated-*`: a unique prefix such as `--ro` runs; before, only `derive-packet` and `build-packet` took one.
- `*-dot-digit-*`: a dash-led value such as `-.5` is refused, exit 2, except where `main.rs` attaches it as described
  below; `--root=-.5` still works, and `-5` and `-1.5`
  are still values. `*-exponent-*`: `-1e5` is taken as a value, as Python 3.14 takes it, where it was refused, as
  Python 3.12 refuses it.
- `*-double-dash`: a `--` with nothing after it is ignored where it was refused.
- `normalize-review-arm-checked-when-read` and `thermo-arm-checked-when-read`: `--arm` is checked as it is read, so a
  later valid `--arm` no longer replaces an invalid one.
- `build-packet-help-after-unknown-option` and `rates-help-after-extra-argument`: an unknown argument before `-h` is a
  usage error where help was printed. This holds for every subcommand.
- `*-help-with-attached-argument`: `-hx` prints help, as argparse does under Python 3.12 and 3.14, where every
  subcommand but `build-packet` refused it. `*-help-with-equals-or-dash`: `-h` with `=` or `-` attached, such as
  `-h=1` or `-h-x`, is still refused, exit 2, as argparse refuses it; `main.rs` refuses it when clap would print help,
  with argparse's message. Like argparse, it first skips further `h`s, so `-hh=1` is refused and `-hhx` prints help;
  `build-packet-help-run-with-equals-or-dash` records that `build-packet` printed help for `-hh=1`.
  `*-help-before-invalid-short-help` and `*-help-abbreviation-before-invalid-short-help`: only the first help before
  `--` decides, as argparse acts on the first help it reads, so `--help -h=1` and `--hel -h=1` print help in every
  subcommand ([#268](https://github.com/kamui/code-review-bench/issues/268)); before, only `grading`, `calibration`
  and `evaluator-audit` did.
- `rates-invalid-command-before-help*`: `rates` refuses a command other than `check` or `refresh` as it reads it, as
  argparse's `choices` does, so an invalid command before `--help` exits 2
  ([#268](https://github.com/kamui/code-review-bench/issues/268)).
- `build-packet-unicode-16-digit-option-value`: `build-packet` keeps Python 3.14's argparse reading of dash-led option
  values, which `build-packet-option-values-with-leading-dash` pins: a value that names no option and holds a space or
  starts with a dash, an optional dot and a decimal digit. `main.rs` attaches such a value to its option with `=`
  before clap parses. Its digits are `pytext`'s Unicode 16.0 decimal digits, as before; the decision states the
  impact against Python 3.12 and older.
- `inventory-dash-led-space-root`, `collect-run-dash-led-space-run` and `reconcile-dash-led-space-paths`: `evidence
  inventory`, `collect` and `reconcile` take a dash-led value that names no option and holds a space, as argparse
  does in every version; `main.rs` attaches it as it does for `build-packet`. A dash-led number stays clap's reading.
  `export-explorer`, `current-grading`, `methodology`, `calibration` and `evaluator-audit` take both readings, as
  `build-packet` does
  (`export-explorer-decisions.v7.json` records the change for `export-explorer-dot-digit-facts`).
- `build-packet-help`, four `build-packet-usage-*` cases, `build-packet-non-utf8-text-option` and
  `export-explorer-usage-missing-stage` pin clap's wording by prefix; their exit codes are unchanged.
- `archive`, which [#218](https://github.com/kamui/code-review-bench/issues/218) added with a hand-written parser, takes
  the same rules through `archive-decisions.v3.json`: `archive-abbreviated-*`, `archive-dot-digit-*`,
  `archive-exponent-archive`, `archive-help-with-*`, `archive-help-after-unknown-command`, and `archive-help` and
  `archive-usage-missing-cache` for clap's wording. It has no Python counterpart, so only CI and maintainers see the
  change; the Pages workflow's `archive check-packages` call parses as before.

## Close-out

Each responsibility the v2 inventory assigned to #154 now has one of three outcomes. It is migrated, with its Python
deleted or kept as a retained library. It is retired. Or it moved to the later stage named in the last column, which
the v3 inventory records.

| v2 #154 responsibility | Outcome | v3 owner |
| --- | --- | --- |
| `roster`, `normalize_thermo`, `derive_packet`, `build_packet` | Migrated to `bench-tools`; the Python is deleted | None |
| `diff_identity`, `normalize_review` | Migrated to `bench-tools`; retained library | #157, #158 |
| `check_manifest`'s schema validation, `rates check`, `upstream report` | Migrated to `bench-tools`; `check_manifest`, `rates` and `upstream` are retained libraries | #160, #160, #156 |
| `export_explorer` | Migrated to `bench-tools export-explorer`; the Python front is a temporary bridge | #156 |
| `rates refresh`, evidence restoration during export | Kept in Python as effects, as the delivery plan assigns | #160, #155 |
| `upstream collect` | Kept in Python as an effect, then migrated to `bench-tools upstream collect` by #235 | None |
| `score`, `scoreboard_svg` | Retired by #192 | None |
| `compare` | Retained library with no port | #157 |
| `methodology`, `collect_run` | Moved by the delivery plan | #156, #155 |
| `stage_edition.py` | Retained research record | #156 |
| `plan.py`, `recut.py` and `verify.py` in `last-push-recut-2026-10-07` | Historical replay, as #192 classified them | None |
| `builtin-review-benchmark-2026-09-24/answers.py` | Historical replay by role, as #192 classified it. The census marked it active because `tools/import_benchmark.py` listed its path to copy and a fixture in `tools/test_python_inventory.py` named the same path; [#223](https://github.com/kamui/code-review-bench/issues/223) removed both, so a census at its head reads it as replay | #155 |

## Research records

[Storage](storage.md#research-records) lists the research records that import a deleted storage module.

`plan.py` in `docs/research/last-push-recut-2026-10-07` imports `roster`. It is a saved research record whose derived
`review-plan.v1.json` is already committed, and no test loads it. [Processing retirement](processing-retirement.md#research-helpers)
classifies it as historical replay. It stays byte-unchanged. Replay it at a commit that has `bench/tools/roster.py`,
such as `5922561cf`, the v2 inventory's source commit.

`recut.py` in the same directory imports `build_packet` and runs `build_packet.py`. Processing
retirement classifies it as historical replay too, and it stays byte-unchanged. It now replays only at a commit that
has `bench/tools/build_packet.py`, such as `1bbc3eed6`, the last main commit that holds it.

## Verification

```sh
cd rust/bench-tools && cargo fetch --locked && cargo fmt --check && cargo clippy --locked --all-targets -- -D warnings && cargo test --locked
python3 bench/tools/rust_runtime.py install --crate rust/bench-tools --select
python3 bench/tools/rust_runtime.py resolve bench-tools
python3 tools/behavior_contract.py --corpus tools/behavior_contracts/roster.v1.json
python3 tools/behavior_contract.py --corpus tools/behavior_contracts/diff-identity.v1.json --differential MAP
python3 tools/behavior_contract.py --corpus tools/behavior_contracts/normalize-review.v1.json --differential MAP
python3 tools/behavior_contract.py --corpus tools/behavior_contracts/check-manifest.v1.json
python3 tools/behavior_contract.py --corpus tools/behavior_contracts/check-manifest.v1.json --implementations PYTHON_MAP --differential MAP
python3 tools/behavior_contract.py --corpus tools/behavior_contracts/rates.v1.json --differential MAP
python3 tools/behavior_contract.py --corpus tools/behavior_contracts/upstream.v1.json --differential MAP
python3 tools/behavior_contract.py --corpus tools/behavior_contracts/normalize-thermo.v1.json
python3 tools/behavior_contract.py --corpus tools/behavior_contracts/derive-packet.v1.json
python3 tools/behavior_contract.py --corpus tools/behavior_contracts/build-packet.v1.json
python3 tools/behavior_contract.py --corpus tools/behavior_contracts/export-explorer.v1.json
python3 tools/behavior_contract.py --verify-decision-history
```

`MAP` is a [command map](contracts/README.md#run-the-usage-corpus) naming the resolved executable, as the `Rust
runtime` workflow writes it. The `roster`, `normalize-thermo`, `derive-packet`, `build-packet` and `export-explorer`
corpora run the Rust executable by default; the other corpora run the retained Python libraries by default.
`derive-packet`, `build-packet` and `export-explorer` have no Python implementation left to compare with, so they
have no differential. [Storage](storage.md#verification) lists the storage corpora. The base commit's Python export derived every `export-explorer` expectation except those of decided
changes. `rates.py` and `upstream.py` run through drivers in `tools/behavior_contract.py` that point them at the case
directory, as the Rust commands default to the current directory, and give `rates.py` the `--source` pages.
