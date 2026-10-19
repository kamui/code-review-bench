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
[v3 inventory](README.md#reproducible-records) and closes the stage, as [the close-out](#close-out) lists.

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
python3 bench/tools/rust_runtime.py exec bench-tools -- derive-packet <packet.legacy.md> --out <packet.md>
python3 bench/tools/rust_runtime.py exec bench-tools -- build-packet --repo <owner/name> --pr <n> ... --out <packet.md>
bun run data
```

## Migrated responsibilities

| Python | Rust | Callers moved | Python implementation |
| --- | --- | --- | --- |
| `bench/tools/roster.py` | `bench-tools roster` | `bun run roster` in `AGENTS.md`, `README.md` and `bench/README.md` | Deleted with `test_roster.py`, whose cases are in [`roster.v1.json`](../../tools/behavior_contracts/roster.v1.json). |
| `bench/tools/diff_identity.py` | `bench-tools diff-identity`, `bench_tools::diff_identity` | The CLI in `bench/README.md` | Retained library, below. |
| `bench/tools/normalize_review.py` | `bench-tools normalize-review`, `bench_tools::normalize_review` | `dispatch.sh`, `claude_skill_runner.py`, `codex_skill_runner.py`, `file_attempt.py --replay` and the `test:bench` script | Retained library, below. `test_normalize_review.py`'s cases are in [`normalize-review.v1.json`](../../tools/behavior_contracts/normalize-review.v1.json), which `test:bench` runs against the retained Python. |
| `bench/tools/normalize_thermo.py` | `bench-tools normalize-thermo`, `bench_tools::normalize_thermo` | The same skill runners and `file_attempt.py --replay` | Deleted with `test_normalize_thermo.py`, whose cases are in [`normalize-thermo.v1.json`](../../tools/behavior_contracts/normalize-thermo.v1.json). |
| `bench/tools/derive_packet.py` | `bench-tools derive-packet`, `bench_tools::derive_packet` | `bench-tools build-packet --factual`, in-process; `bench/README.md` | Deleted. Its self-test cases and every target's legacy packet are in [`derive-packet.v1.json`](../../tools/behavior_contracts/derive-packet.v1.json). The imported original is preserved at `artifacts/import-source/bench/tools/derive_packet.py`. |
| `bench/tools/build_packet.py` | `bench-tools build-packet`, `bench_tools::build_packet` | `bench/README.md`, `docs/pr-selection.md` | Deleted with `test_build_packet.py` and `test_build_packet_fixture.json`. Every run the tests made, with its replay files and staging mirror, is a case in [`build-packet.v1.json`](../../tools/behavior_contracts/build-packet.v1.json), beside 36 more. The imported originals are preserved under `artifacts/import-source/bench/tools/`. |
| `bench/tools/check_manifest.py` | `bench_tools::schema`, `bench-tools check-manifest` | None; its CLI callers stay with #156, below | Retained library, below. |
| `bench/tools/rates.py`, its `check` command | `bench-tools rates check`, `bench_tools::rates` | `bun run rates:check`; `docs/rates.md` | Retained library, below. `refresh` stays in Python. |
| `bench/tools/upstream.py`, its `report` command | `bench-tools upstream report`, `bench_tools::upstream`, with the registry checks of `claims.load_registry` in `bench_tools::claims` | `bun run audit:maintainers`; `docs/maintainer-adjudication.md` | Retained library, below. `collect` stays in Python. |
| `tools/export_explorer.py`, its export | `bench-tools export-explorer evidence`, `stage` and `publish`, `bench_tools::export_explorer` | `bun run data` runs the front below; `.github/workflows/pages.yml` installs `bench-tools`; `src/lib/scoring.test.ts` passes `--root` and `--public` | The Python front, a temporary bridge below. The cases of `test_export_explorer.py` that read the export's internals moved to cargo tests and [`export-explorer.v1.json`](../../tools/behavior_contracts/export-explorer.v1.json); the rest run the front. |

`tools/test_moved_cli_callers.py` fails when a current file other than the retained module still runs a moved
command through Python. Frozen runs, imported originals and research records keep their commands.

## Retained libraries

A retained library stays byte-unchanged while a later stage still imports or hashes it. Where #154 ported it, its CLI
callers moved to Rust and CI runs the corpus against both implementations with a differential. `compare.py` and
`stage_edition.py` have no Rust port; [processing retirement](processing-retirement.md) kept them for their callers.
A retained library is removed by the stage in the last column once its removal condition holds, and the v3 inventory
assigns it to that stage.

| Module | Imported or hashed by | Removal condition | Removed by |
| --- | --- | --- | --- |
| `bench/tools/diff_identity.py` | `provision.py` imports `identity`; `test_cache_replacements.py` imports it; `grade.runner_files()` hashes it, so a byte change refuses every prepared grading batch | `provision.py` computes diff identity through the Rust runtime ([#157](https://github.com/kamui/code-review-bench/issues/157)) and `grade.runner_files()` no longer hashes `diff_identity.py` ([#156](https://github.com/kamui/code-review-bench/issues/156)) | #157 |
| `bench/tools/normalize_review.py` | `grade.py` imports `render` for blind grading; `contributor_adapters.collect` calls `from_codex`, `from_claude_builtin` and `from_codex_skill` in process; `grade.runner_files()` hashes it | `grade.render` renders through the Rust runtime and `grade.runner_files()` no longer hashes `normalize_review.py` ([#156](https://github.com/kamui/code-review-bench/issues/156)), and the contributor backend normalizes through it ([#158](https://github.com/kamui/code-review-bench/issues/158)) | #158 |
| `bench/tools/check_manifest.py` | `claims`, `claim_grading`, `current_grading`, `evidence_store`, `file_attempt`, `provision`, `rates`, `reconcile_evidence`, `run_cell` and `upstream` import `validate`; `grade.runner_files()` hashes it. Its CLI runs in `bench/README.md`, `.github/workflows/rates.yml` and `test:bench`'s `--self-test` | No maintained module imports it and `grade.runner_files()` no longer hashes it. The delivery plan leaves `check_manifest` with [#156](https://github.com/kamui/code-review-bench/issues/156), which moves its CLI callers and its grading importers; `provision`, `run_cell` and `file_attempt` leave with [#157](https://github.com/kamui/code-review-bench/issues/157), and `rates`, the last importer, with [#160](https://github.com/kamui/code-review-bench/issues/160) | #160 |
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

A bridge lets a Python caller run a Rust command until the caller itself moves to Rust.

| Bridge | Removal condition | Removed by |
| --- | --- | --- |
| `tools/export_explorer.py` loads the current facts with `current_grading.load_current`, writes them to a temporary file with `json.dumps(allow_nan=False)`, restores the evidence `bench-tools export-explorer evidence` lists through `evidence_store.resolve`, runs `stage`, checks the dataset hash again, and runs `publish` with `SIGINT` ignored. It exits 2 when `bench-tools` is not installed for `HEAD`. | Evidence resolution moves to Rust ([#155](https://github.com/kamui/code-review-bench/issues/155)) and `load_current` moves to Rust ([#156](https://github.com/kamui/code-review-bench/issues/156)) | #156 |

#195's bridge, `build_packet.py --factual` running `bench-tools derive-packet` through `rust_runtime.resolve`, was
removed by #196: `bench-tools build-packet --factual` calls `bench_tools::derive_packet::derive` in-process. It
cannot exit 2 for a missing `bench-tools`, as the bridge did, because it is `bench-tools`. `derive-packet` keeps
`--no-newline-translation`, which the bridge passed, because `derive-packet-no-newline-translation` pins it and
decision history cannot remove a case.

## The explorer export

`bench-tools export-explorer` writes the same `public/data` and `public/evidence` bytes as the Python export it
replaces. `src/lib/scoring.ts` stays the only owner of scorecard arithmetic; the export computes no score, rate or
ranking. The front passes `--base-path` from `BASE_PATH`, as the Python export read it.

- `evidence` lists every file the export links, with its frozen SHA-256 or null, in the order the export first
  names it, as JSON on stdout. The front restores the missing ones.
- `stage` refuses while `STAGE/previous` holds backups, replaces `STAGE`, writes the dataset, the attempt details
  and the linked evidence, and checks them as `validate_export` did. It prints the summary line the front prints.
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

## Behavior the corpora do not pin

These differences from the Python implementations are outside every contract. A caller that relied on one needs a
recorded decision.

- `bench-tools roster` reads the repository at the current directory unless `--root` names another. `roster.py`
  read the repository that held the script. `bun run roster` runs from the repository root.
- An input whose shape made Python raise a traceback, such as a registry without `suites`, exits 2 with a message
  naming the file. Python exited 1.
- A lone surrogate escape such as `"\ud800"` and JSON nested more than 1,000 levels deep are refused, exiting 2.
  Python kept the surrogate, and its own nesting limit depends on its version and stack.
- `diff-identity` has no `--self-test`; `cargo test` and the corpus cover it. It accepts options between
  positional arguments, which argparse refuses for some orders.
- `normalize-review` has no `--self-test`, and options must be spelled in full; argparse also accepted unique prefixes.
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
  `check-manifest-decisions.v3.json` records where the pinned Unicode 16.0 tables differ from an older
  interpreter: `\w` on 5,004 characters against Python 3.12, including the 622 Unicode 15.1 added, `\d` on 80
  and `(?i)` on 54, each with a case.
- A schema or manifest that is not UTF-8 exits 2 with the `UnicodeDecodeError` text. Python raised a traceback.
- `\d` and `int()` accept the decimal digits of Unicode 16.0, as Python 3.14 does; Python 3.12 lacks the digits
  16.0 added. `rates-decisions.v4.json` records this for the catalog date, price footnotes and long-prompt rows,
  and `upstream-decisions.v2.json` for snapshot versions. Lower-casing an Anthropic model or a repository name
  uses Rust's Unicode tables.
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
`upstream-unicode-15-1-registry-path`, `diff-identity-unicode-15-1-git-record` and
`export-explorer-unicode-15-1-adjudication-id`. The five whose Python is retained
carry Python 3.12's escaped output as their `baseline_defect`. `derive-packet-unassigned-path` pins U+0378, which
every Python version escapes.

Diagnostics keep the Python program names, `roster.py:`, `diff_identity.py:`, `normalize_review.py:`,
`normalize_thermo.py:`, `derive_packet.py:`, `build_packet:`, `check_manifest.py:`, `rates.py:`, `upstream.py:` and
`export_explorer.py:`, so saved stderr stays identical and the differential compares them byte for byte. Renaming them
needs a new decision version.

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
| `rates refresh`, `upstream collect`, evidence restoration during export | Kept in Python as effects, as the delivery plan assigns | #160, #156, #155 |
| `score`, `scoreboard_svg` | Retired by #192 | None |
| `compare` | Retained library with no port | #157 |
| `methodology`, `collect_run` | Moved by the delivery plan | #156, #155 |
| `stage_edition.py` | Retained research record | #156 |
| `plan.py`, `recut.py` and `verify.py` in `last-push-recut-2026-10-07` | Historical replay, as #192 classified them | None |
| `builtin-review-benchmark-2026-09-24/answers.py` | Historical replay by role, as #192 classified it. The census still marks it active because `tools/import_benchmark.py` lists its path to copy and a fixture in `tools/test_python_inventory.py` names the same path; both must go before it reads as replay | #155 |

## Research records

`plan.py` in `docs/research/last-push-recut-2026-10-07` imports `roster`. It is a saved research record whose derived
`review-plan.v1.json` is already committed, and no test loads it. [Processing retirement](processing-retirement.md#research-helpers)
classifies it as historical replay. It stays byte-unchanged. Replay it at a commit that has `bench/tools/roster.py`,
such as `5922561cf`, the v2 inventory's source commit.

`recut.py` in the same directory imports `build_packet` and runs `build_packet.py`. Processing
retirement classifies it as historical replay too, and it stays byte-unchanged. It now replays only at a commit that
has `bench/tools/build_packet.py`, such as `1bbc3eed6`, the last main commit that holds it.

## Verification

```sh
cd rust/bench-tools && cargo fmt --check && cargo clippy --locked --all-targets -- -D warnings && cargo test --locked
python3 bench/tools/rust_runtime.py install --crate rust/bench-tools --select
python3 bench/tools/rust_runtime.py resolve bench-tools
python3 tools/behavior_contract.py --corpus tools/behavior_contracts/roster.v1.json
python3 tools/behavior_contract.py --corpus tools/behavior_contracts/diff-identity.v1.json --differential MAP
python3 tools/behavior_contract.py --corpus tools/behavior_contracts/normalize-review.v1.json --differential MAP
python3 tools/behavior_contract.py --corpus tools/behavior_contracts/check-manifest.v1.json --differential MAP
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
`derive-packet`, `build-packet` and `export-explorer` have no Python implementation left to compare with, so they have
no differential. The base commit's Python export derived every `export-explorer` expectation except those of decided
changes. `rates.py` and `upstream.py` run through drivers in `tools/behavior_contract.py` that point them at the case
directory, as the Rust commands default to the current directory, and give `rates.py` the `--source` pages.
