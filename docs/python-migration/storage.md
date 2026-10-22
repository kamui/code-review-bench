# Evidence storage in Rust

This records the storage stage, [#155](https://github.com/kamui/code-review-bench/issues/155), part of
[#149](https://github.com/kamui/code-review-bench/issues/149). It lands in slices that follow the [delivery
plan](https://github.com/kamui/code-review-bench/issues/155#issuecomment-6263888656). Slice 0,
[#217](https://github.com/kamui/code-review-bench/issues/217), pins the Python behavior in the storage corpora before
any Rust. Slice 1, [#218](https://github.com/kamui/code-review-bench/issues/218), adds the archive crates and
`bench_tools::archive`. Slice 2, [#219](https://github.com/kamui/code-review-bench/issues/219), moves restoration,
verification and recovery; slice 3, [#220](https://github.com/kamui/code-review-bench/issues/220), packing and
publication, with the contributor CLI; slice 4, [#221](https://github.com/kamui/code-review-bench/issues/221), native
artifacts and transcript archives; slice 5, [#222](https://github.com/kamui/code-review-bench/issues/222), collection,
the inventory command and reconciliation; and slice 6, [#223](https://github.com/kamui/code-review-bench/issues/223),
import verification. Slice 7, [#224](https://github.com/kamui/code-review-bench/issues/224), records the resulting
ownership in the [v4 inventory](README.md#reproducible-records) and closes the stage, as [the close-out](#close-out)
lists.

The commands are subcommands of the `bench-tools` crate, which [processing](processing.md#the-bench-tools-crate)
describes, and run through the pinned executable as [Rust runtime pinning](runtime.md#running-a-maintained-command)
describes. The [storage contract](../evidence-storage.md) is the user guide.

## Commands

### Archives

`bench_tools::archive` wraps the crates in the limits and refusals of #155's
[delivery plan](https://github.com/kamui/code-review-bench/issues/155#issuecomment-6263888656). Readers stream each
regular member to a visitor; writers stream each member in.

- Packages are gzip-compressed tar. The writer uses level 9, no file name and modification time 0, as
  `evidence_store.pack` does, with a PAX `path` record for a name over 100 bytes. Its bytes may differ from Python's,
  as the storage contract allows for new outer packages.
- The reader accepts what `tarfile` accepts for a package: PAX and GNU long names, multi-member gzip with zero padding,
  and regular and contiguous members. It parses nothing after the end marker, and bytes after a gzip member that do
  not start another member end the stream, as `tarfile` never reads them. It refuses non-regular members by type
  and mode bits, duplicate names and the changes from Python that
  [`archive-decisions.v1.json`](../../tools/behavior_contracts/archive-decisions.v1.json) records.
- Metadata limits come from the package's index, before a crate allocates. The decoded stream may hold the indexed
  payload plus, per member, its header blocks and up to three extension headers of at most 64 KiB each. A PAX header,
  GNU long name or GNU long link over 64 KiB is refused from its header. Headers and members are capped by the index's
  member count. `flate2` refuses a gzip FNAME or FCOMMENT field over 65,535 bytes. For zip, every end record in the
  comment window is read first, and an entry count, a central directory size or zip64 extensible data over its limit
  is refused before `ZipArchive` opens. `ZipArchive` opens at the archive start that `zipfile` infers from the last
  end record, so it never indexes signatures in member data.
- Native scratch archives are zip. The writer names deflate entries by content hash, with mode 0600 and the
  1980-01-01 timestamp, as `native_artifacts.file_artifacts` does. The reader accepts stored, deflate and zip64
  entries, and refuses encryption, other methods, duplicate names and entries that are not regular files.

`bench-tools archive check-packages --cache DIR MANIFEST...` checks each cached package as `evidence_store.unpack` does,
without restoring it: the archive's size and SHA-256, then each member's name, size, mode and SHA-256 against the
manifest. `bench-tools archive check-scratch ZIP...` checks that each scratch entry hashes to its name. Both print a JSON
report and exit 1 when an archive fails. The Pages build runs `check-packages` on the 12 packages it restores. A
one-time check read all 33 published packages, from the 19 manifests in `bench/evidence/manifests/`, after Python's
`fetch` restored them into a disposable root:
[`published-packages.2026-10-19.json`](evidence/published-packages.2026-10-19.json).

### Evidence restoration

[#219](https://github.com/kamui/code-review-bench/issues/219) moves restoration and verification to
`bench-tools evidence`, in `bench_tools::evidence`:

| Command | Python it replaces |
| --- | --- |
| `evidence fetch --manifest M [--root R] [--path P]... [--offline]` | `evidence_store.py`'s `fetch` |
| `evidence verify --manifest M [--receipt R]` | `evidence_store.py`'s `verify` |
| `evidence resolve [--sha256 H] [--root R] [--offline] PATH` | `evidence_store.resolve`, which `prune_workspace.py` calls and the deleted `export_explorer.py` called |
| `evidence recover --source S [--root R] [--path P]... [--offline]` | `bench/tools/recover_evidence.py` |
| `evidence restore-local ROOT ORIGINAL OUTPUT` | `local_evidence_resolution`, with the research record `restore-local.py`'s arguments |

Packages are read through `bench_tools::archive`, so a fetch refuses
what [archives](#archives) refuses; the evidence-store v3 decisions record each form Python restored. Restoration is
durable: each downloaded archive and restored member is written to a temporary file in `.cache/evidence`, synced, then
hard-linked into place, and the directory is synced. Before it reports success, `fetch` also syncs every directory
from the root to each requested file and the cache, so a retry settles links an interrupted fetch made but never
synced. A receipt is written to a synced temporary file and linked, so an
interrupted `verify --receipt` leaves no receipt. A fetch checks every requested destination before it restores any:
a changed regular file is refused with its expected and found size, SHA-256 and mode, and the shell commands that
move it aside under `.cache/evidence/moved-aside/<found sha256>/`; any other file there is refused as Python refuses
it. `tests/evidence.rs` injects a failure at each disk effect and checks that a retry restores the package and
syncs every directory it relies on. `--path`, `--source` and `restore-local`'s names keep their argument bytes: one
that is not UTF-8 names nothing, as argparse's `surrogateescape` text matches no JSON string
(`fetch-non-utf8-path-names-no-member`). `resolve`, which has no Python command, takes `PATH` and `--sha256` as UTF-8
only.

`bench-tools export-explorer`, and its `evidence --restore`, resolve each linked file the same way. `bun run
evidence:fetch` and the Pages and rates workflows run `evidence fetch`; each job that fetches installs `bench-tools`
first. `prune_workspace.py` still calls the retained `evidence_store.resolve`; `resolve-across-implementations`
restores a path through Rust and resolves it through Python.

### Evidence packing and publication

[#220](https://github.com/kamui/code-review-bench/issues/220) moves packing and publication to `bench-tools evidence`, in
`bench_tools::evidence::publication`, so Rust is the only packer and publisher:

| Command | Python it replaces |
| --- | --- |
| `evidence pack --selection S --out O [--part-bytes N]` | `evidence_store.py`'s `pack` |
| `evidence check-package DIR` | `contributor.archive_attempt`'s check of a package it packed: `DIR` holds exactly the manifest and its assets, and each part restores into an empty temporary directory |
| `evidence publish --manifest M --out O` | `evidence_store.py`'s `publish` |

`pack` keeps Python's selection shape, refusals, part order and messages, and restores each archive before linking it
into place. `publish` keeps the draft and 1,000-asset checks, never replaces an asset, reads the release after a failed
upload instead of uploading again, checks the asset's size and identity, and downloads and restores every package
before it writes the published manifest; an existing published manifest is verified, and its directories synced, not
uploaded again. `check-package` refuses a directory it cannot list. Each package
and manifest is written as receipts are: to a synced temporary file, linked to its name without replacing one, and
the directory synced. `tests/publication.rs` interrupts every disk effect of both commands, and injects a changed
input, a failed link, a lost upload response, a partial asset, a rejected upload and a corrupted download.
`evidence-store-decisions.v6.json` and `v7` record the changes from Python, each with its case: durable writes
(`publish-leftover-temporary`), `--part-bytes` read with Unicode 16.0 digits (`pack-unicode-16-digit-part-bytes`), a
selection holding a lone surrogate escape refused (`pack-lone-surrogate-selection`), dash-led option values read by
Python 3.14's argparse rule (`pack-dash-led-out-python-3-14-rule`), and clap's wording for an invalid `--part-bytes`
(`pack-int-control-whitespace-part-bytes`). `pack` and `publish` attach a dash-led value such as `-١` to its option,
as `build-packet` does (`pack-unicode-negative-out`, `publish-unicode-negative-out`). A receipt or published manifest
that cannot be created now names itself, not its temporary file, as `open(path, 'x')` does
(`verify-receipt-unwritable-directory`, `publish-unwritable-directory`). `pack-restored-by-python` packs and publishes
through Rust and restores through the retained `evidence_store.resolve`.

The contributor CLI runs these commands through each run's pinned `bench-tools`, a [temporary bridge](#temporary-bridges).
`prepare_run` writes the selected pin to the run's `runtimes.frozen.json` and records that file's SHA-256 in the run
manifest. `run`, `run --resume` and `submit` resolve it before any other effect or paid-work prompt and print the
install commands when it is missing; filing (`archive_attempt`) and `submit` run only the pinned executable, through
`rust_runtime.for_run`. A submission takes the one pin its runs share, refuses runs with different pins, and saves the
pin in its journal when it first packs. Its public records carry each run's `runtimes.frozen.json` beside the run
manifest that hashes it. A run prepared before this change pins none and is refused with its
`source_commit`. `prune_workspace.prune` still runs in process on the retained Python until
[#159](https://github.com/kamui/code-review-bench/issues/159). `test_contributor.py` runs the CLI on disposable
repositories with the installed `bench-tools` and the contract harness's stub `gh`.

### Native artifacts and transcript archives

[#221](https://github.com/kamui/code-review-bench/issues/221) moves native artifact storage and transcript archiving
to `bench-tools`, in `bench_tools::native_artifacts` and `bench_tools::evidence::transcripts`:

| Command | Python it replaces |
| --- | --- |
| `native-artifacts verify ATTEMPT` | `native_artifacts.py`'s `verify` |
| `native-artifacts restore ATTEMPT --out OUT` | `native_artifacts.py`'s `restore` |
| `native-artifacts file ROOT INDEX OUT [--native-relative R]` | `native_artifacts.file_artifacts` and `unindexed`, which `file_attempt.py` called; it prints `{"pin": ..., "unindexed": [...]}` |
| `evidence archive-transcripts --attempt-dir A --dest D --out O [PATH]...` | `file_attempt.archive_transcripts` and `recorded_archive_path` |

Filing keeps Python's rules: one zip entry per content hash, modes other than 0644 recorded, a new output directory
for restoration, and the same refusals of links, special files, unindexed files, changed content, escaping paths and
changes to an existing filing. A new archived filing is staged and synced, then placed with its storage record linked
last, so an interruption never leaves a storage record. A retry completes the filing when the names an interruption
left hold what it would write and refuses them otherwise, as Python refuses any existing name
(`file-interrupted-before-storage-record`). Transcripts are written to a synced temporary archive and linked; an
existing archive is kept when its members are the transcripts and refused otherwise, where Python replaced it
(`archive-transcripts-existing-same`, `-existing-different`). The native-artifacts v4 decisions record each
deliberate difference. `tests/native_artifacts.rs` interrupts both at each disk effect.

`bench-tools file-attempt` calls both in process since [#157](https://github.com/kamui/code-review-bench/issues/157);
`bench-tools run-cell --file` refuses a run whose pin lacks `file-attempt`, naming the run's freeze commit. The corpus runs
against Rust, with a differential against the retained `native_artifacts.py` and the former
`file_attempt.archive_transcripts`, kept in the deleted contract harness's `python-archive-transcripts` driver until
[#160](https://github.com/kamui/code-review-bench/issues/160) moved the corpora to the Rust runner.

`tests/python_archives.rs` checks that the retained `evidence_store.py` and `native_artifacts.py` can restore what
`bench_tools::archive` writes, and that it reads what they write. It is deleted with them
([#159](https://github.com/kamui/code-review-bench/issues/159)).

### Evidence collection, inventory and reconciliation

[#222](https://github.com/kamui/code-review-bench/issues/222) moves the remaining evidence record writers to
`bench-tools evidence`:

| Command | Python it replaces |
| --- | --- |
| `evidence collect --run RUN [--missing RECORD]...` | `tools/collect_run.py`, in `bench_tools::evidence::collect` |
| `evidence inventory --root R... --out OUT [--hash-all]` | `bench/tools/evidence_inventory.py`'s command, in `bench_tools::evidence_inventory` |
| `evidence reconcile --inventory I [--root R] --out OUT` | `bench/tools/reconcile_evidence.py`, in `bench_tools::evidence_inventory::reconcile` |

`collect` reads the repository at the current directory, as the corpus driver for `collect_run.py` did. It copies an
archive through a synced temporary file linked into place, then replaces `transcripts.json` through a synced
temporary file and a rename that keeps the file's mode; `tests/evidence.rs` interrupts it at each disk effect. Python
copied in place and rewrote the mapping through any link at its path. `collect` also hashes the bytes it copies and
refuses an archive that changed after it verified, where Python copied whatever its second read returned.
`inventory` writes `\udcXX` escapes for non-UTF-8 names through `pyjson`'s `OsText`, so its output matches Python's
byte for byte, and `retire_workspace.py` accepts it as a snapshot. It refuses an output that reaches an inspected root
through a parent traversal or a link, which Python wrote inside the root; `inventory-out-parent-traversal-into-root`
and `inventory-out-symlinked-parent` pin that. `reconcile` writes gzip at level 9 with no name and a zero time, as
Python did, and its JSON matches Python's after decompression; it refuses an inventory holding a lone surrogate,
which `pyjson` cannot read.
`evidence_inventory.py` stays for `retire_workspace.py` until
[#159](https://github.com/kamui/code-review-bench/issues/159).

### Import verification

[#223](https://github.com/kamui/code-review-bench/issues/223) moves `tools/import_benchmark.py` to
`bench-tools import-benchmark`, in `bench_tools::import_benchmark`. It reads the repository at its working directory.

- `--check` verifies each entry of `bench/import-manifest.json` at its `preserved_path`, or its path, through
  `evidence::resolve`, skips entries marked missing, and prints Python's summary line. A refusal lists the failing
  paths and, for an edited imported file that has no preserved copy and is not under `bench/runs/`, the
  `bun run import:preserve PATH...` command, the package script that runs `--preserve`.
- `--preserve PATH...` takes the imported bytes from the current file, or from the oldest commit that holds them,
  and writes them under `artifacts/import-source/`. The copy is written to a synced temporary file and linked to its
  name, never replacing a file. The new manifest is checked before it is written, then renamed over the old one in
  Python's `json.dumps(indent=2)` spelling, keeping its permission bits. `tests/import_benchmark.rs` interrupts each
  of these steps and retries.
- `--source`, which extracted the skills repository, is retired; `README.md` names a commit that still has it.

`bun run verify:import` and `.github/workflows/rates.yml` run `--check`. Every refusal prints
`import_benchmark.py: ` and the message on stderr with exit 1, where Python raised a traceback. The other differences
are in `import-benchmark-decisions.v3.json`, each with its case. Before the switch, both implementations printed the
same summary line on this checkout: `Verified copied bytes for 2775 source files and 288 archives; 9 superseded
source-hash mismatches, 0 unavailable`.

## Registry dependencies

#155 added these crates to `rust/bench-tools`, under the owner's decision of 2026-10-19 in the delivery plan. The
[registry dependencies](processing.md#registry-dependencies) table lists every crate the crate builds, with its
purpose; `Cargo.lock` pins them.

| Crate | Version | License | Added by |
| --- | --- | --- | --- |
| `flate2`, with `rust_backend` only | 1.1.10 | MIT OR Apache-2.0 | [#218](https://github.com/kamui/code-review-bench/issues/218) |
| `tar`, without default features | 0.4.46 | MIT OR Apache-2.0 | [#218](https://github.com/kamui/code-review-bench/issues/218) |
| `zip`, with `deflate-flate2` only | 9.0.5 | MIT | [#218](https://github.com/kamui/code-review-bench/issues/218) |
| `sha1_smol`, without default features | 1.0.1 | BSD-3-Clause | [#222](https://github.com/kamui/code-review-bench/issues/222) |
| `miniz_oxide` | 0.9.1 | MIT OR Zlib OR Apache-2.0 | [#218](https://github.com/kamui/code-review-bench/issues/218), for `flate2` |
| `adler2` | 2.0.1 | 0BSD OR MIT OR Apache-2.0 | [#218](https://github.com/kamui/code-review-bench/issues/218), for `miniz_oxide` |
| `simd-adler32` | 0.3.10 | MIT | [#218](https://github.com/kamui/code-review-bench/issues/218), for `miniz_oxide` |
| `crc32fast` | 1.5.2 | MIT OR Apache-2.0 | [#218](https://github.com/kamui/code-review-bench/issues/218), for `flate2` and `zip` |
| `filetime` | 0.2.29 | MIT OR Apache-2.0 | [#218](https://github.com/kamui/code-review-bench/issues/218), for `tar` |
| `indexmap` | 2.14.2 | MIT OR Apache-2.0 | [#218](https://github.com/kamui/code-review-bench/issues/218), for `zip` |
| `hashbrown` | 0.17.1 | MIT OR Apache-2.0 | [#218](https://github.com/kamui/code-review-bench/issues/218), for `indexmap` |
| `equivalent` | 1.0.3 | MIT OR Apache-2.0 | [#218](https://github.com/kamui/code-review-bench/issues/218), for `indexmap` |
| `memchr` | 2.8.3 | Unlicense OR MIT | [#218](https://github.com/kamui/code-review-bench/issues/218), for `zip` |
| `typed-path` | 0.12.3 | MIT OR Apache-2.0 | [#218](https://github.com/kamui/code-review-bench/issues/218), for `zip` |

`tar`, `filetime` and `zip` also use `libc`, `cfg-if` and, on macOS, `nix` with `bitflags` and `cfg_aliases`, which
the registry dependencies table lists with their other users.

## Migrated responsibilities

| Python | Rust | Callers moved | Python implementation |
| --- | --- | --- | --- |
| `bench/tools/evidence_store.py`, its `pack`, `publish`, `fetch` and `verify` commands and `resolve` | `bench-tools evidence pack`, `check-package`, `publish`, `fetch`, `verify` and `resolve`, `bench_tools::evidence` | `bun run evidence:fetch`; `.github/workflows/pages.yml` and `rates.yml`; `bench-tools export-explorer`; `contributor.py` and `contributor_submit.py` through each run's pinned `bench-tools`; `docs/evidence-storage.md`, `docs/codex-container-fixture.md`, `docs/contributing-reviews.md` | Retained byte-unchanged: `prune_workspace.py`, `retire_workspace.py` and `native_artifacts.py` import it. [`evidence-store.v1.json`](../../tools/behavior_contracts/evidence-store.v1.json) runs every command against Rust, with a differential against Python in CI. |
| `bench/tools/native_artifacts.py`, its `verify` and `restore` commands and `file_artifacts`; `file_attempt.archive_transcripts` | `bench-tools native-artifacts verify`, `restore` and `file`, `evidence archive-transcripts` | `file_attempt.py`, below; `bench/README.md`, `docs/evidence-storage.md` | `native_artifacts.py` is retained byte-unchanged for `prune_workspace.py`. `archive_transcripts` is deleted; the corpus keeps its source as a driver. [`native-artifacts.v1.json`](../../tools/behavior_contracts/native-artifacts.v1.json) runs against Rust, with a differential against Python in CI. |
| `tools/import_benchmark.py`, its `--check` and `--preserve` | `bench-tools import-benchmark`, `bench_tools::import_benchmark` | `bun run verify:import` and `bun run import:preserve`; `.github/workflows/rates.yml`; `tools/test_evidence_consumers.py`; `README.md`, `docs/evidence-storage.md` | Deleted with `test_import_benchmark.py`, whose cases are in [`import-benchmark.v1.json`](../../tools/behavior_contracts/import-benchmark.v1.json); the corpus ran against both before deletion. `--source` is retired. |
| `bench/tools/recover_evidence.py`, `bench/tools/local_evidence_resolution.py` | `bench-tools evidence recover` and `restore-local`, `bench_tools::evidence::recovery` | `docs/evidence-storage.md` | Deleted with `test_recover_evidence.py` and `test_local_evidence_resolution.py`, whose cases are in the cargo tests and `evidence-store.v1.json`; the corpus ran against both before deletion. |
| `tools/collect_run.py`, `bench/tools/reconcile_evidence.py` | `bench-tools evidence collect` and `reconcile`, `bench_tools::evidence::collect` and `bench_tools::evidence_inventory::reconcile` | `docs/evidence-storage.md`; the collection steps of `tools/test_evidence_consumers.py` | Deleted with `test_collect_run.py` and `test_reconcile_evidence.py`, whose cases are in the cargo tests and [`evidence-inventory.v1.json`](../../tools/behavior_contracts/evidence-inventory.v1.json); the corpus ran against both with a differential before deletion. |
| `bench/tools/evidence_inventory.py`, its command | `bench-tools evidence inventory`, `bench_tools::evidence_inventory` | `docs/evidence-storage.md` | Retained byte-unchanged: `retire_workspace.py` imports it until #159. `evidence-inventory.v1.json` runs against Rust, with a differential against Python in CI. |
| `tools/export_explorer.py`'s evidence restoration | `bench-tools export-explorer evidence --restore` ([#219](https://github.com/kamui/code-review-bench/issues/219)) | `bench-tools export-explorer`, which [#234](https://github.com/kamui/code-review-bench/issues/234) gave the whole export | Deleted with the front by #234; see [the explorer export](processing.md#the-explorer-export) |

`tools/test_moved_cli_callers.py` fails when a current file other than a retained module still runs a moved command
through Python.

## Retained libraries

A retained library stays byte-unchanged while a module that #155 must leave unchanged still imports it. Its commands
moved to Rust, and CI runs its corpus against both implementations with a differential. Until
[#159](https://github.com/kamui/code-review-bench/issues/159), the retained readers and `bench-tools` share
`.cache/evidence` and the manifests; each links without overwriting and verifies before it trusts a file, and
cross-implementation cases check that each restores what the other wrote.

| Module | Imported or hashed by | Removal condition | Removed by |
| --- | --- | --- | --- |
| `bench/tools/evidence_store.py` | `prune_workspace.py` resolves and verifies through it, and `grade.runner_files()` hashes `prune_workspace.py`, so a byte change refuses every prepared grading batch; `contributor.py` calls `prune_workspace.prune` in process; `retire_workspace.py` and `evidence_inventory.py` import it | `prune_workspace.py` and `retire_workspace.py` resolve, verify and inventory through `bench-tools` ([#159](https://github.com/kamui/code-review-bench/issues/159)) | #159 |
| `bench/tools/native_artifacts.py` | `prune_workspace.py` verifies filings with `verify` before it removes a clone; `grade.runner_files()` hashes `prune_workspace.py` | `prune_workspace.py` verifies through `bench-tools native-artifacts verify` ([#159](https://github.com/kamui/code-review-bench/issues/159)) | #159 |
| `bench/tools/evidence_inventory.py` | `retire_workspace.py` inventories the workspace, takes Git blob identities and runs Git through it | `retire_workspace.py` moves to Rust and inventories through `bench_tools::evidence_inventory` ([#159](https://github.com/kamui/code-review-bench/issues/159)) | #159 |

The tests of the retained libraries, the `python-resolve` corpus program and `tests/python_archives.rs` call the
retained Python to show that both implementations read each other's output. They go with the libraries. Since #160 the
corpus runner runs `python-native-artifacts` as `bench-tools native-artifacts`.

## Temporary bridges

A bridge lets a Python caller run a Rust command until the caller itself moves to Rust.

| Bridge | Removal condition | Removed by |
| --- | --- | --- |
| `contributor.py` and `contributor_submit.py` run `evidence pack`, `check-package` and `publish`, and `evidence verify --receipt`, through the run's pinned `bench-tools` ([#220](https://github.com/kamui/code-review-bench/issues/220)) | The contributor backend moves to Rust | [#158](https://github.com/kamui/code-review-bench/issues/158) |

`tools/export_explorer.py`, the processing stage's bridge, was deleted by
[#234](https://github.com/kamui/code-review-bench/issues/234), part of [#156](https://github.com/kamui/code-review-bench/issues/156).

## Behavior the corpora do not pin

These differences from the Python implementations are outside every contract. A caller that relied on one needs a
recorded decision.

- `evidence collect` reads the repository at the current directory; `collect_run.py` read the repository that held
  the script. Run it from the repository root.
- `evidence collect` and `evidence reconcile` write each new file through a synced temporary file linked into place,
  so an interrupted run leaves no partial archive copy or reconciliation. Python wrote in place.
- Every failure that made `collect_run.py` or `reconcile_evidence.py` raise a traceback exits 1 with the program's
  prefix and a message that approximates the traceback's last line, such as `reconcile_evidence.py: list indices must
  be integers or slices, not str`. The decided `collect-run-*` and `reconcile-*` cases pin the forms the corpus covers.
- Durable writes sync each file and directory before reporting success. Cargo tests inject a failure at each disk
  effect; neither they nor the corpora cut power, so the promise holds only on a filesystem that honors `fsync`.
- Every corpus and cargo test reaches `gh` through a stub or the `Remote` fake. Only CI's read-only fetch of the Pages
  manifests calls the real service.

## Decisions

Each corpus has its decisions family under `tools/behavior_contracts/`. A record names its case; an intended change
also states the old and new behavior, the rationale, the compatibility impact and the resolving issue. The latest
version of each family supersedes the earlier ones, which `--verify-decision-history` keeps unchanged.

| Latest decisions | Commands | Records | Intended changes from Python, each named by its case |
| --- | --- | --- | --- |
| [`archive-decisions.v3.json`](../../tools/behavior_contracts/archive-decisions.v3.json) | `archive` | 23 | `archive-pax-global-header-refused`, `archive-gnu-sparse-refused`, `archive-pax-size-refused`, `archive-gzip-crc-after-end-marker-refused`, `archive-invalid-header-after-members-refused`, `archive-extension-header-over-limit-refused`, `archive-gzip-header-field-over-limit-refused`, `archive-zip64-extensible-data-over-limit-refused`, `archive-zip-bzip2-refused`, `archive-zip-symlink-entry-refused` |
| [`evidence-store-decisions.v7.json`](../../tools/behavior_contracts/evidence-store-decisions.v7.json) | `evidence fetch`, `verify`, `resolve`, `recover`, `restore-local`, `pack`, `check-package` and `publish` | 120 | `fetch-existing-changed`, `fetch-existing-changed-mode`, `fetch-archive-corrupt-deflate`, `fetch-archive-truncated-gzip`, `recover-unknown-source`, `recover-changed-manifest`, `restore-local-not-published`, `restore-local-mismatched-resolution`, `fetch-changed-in-later-package`, `verify-receipt-leftover-temporary`, `recover-missing-release`, `restore-local-wrong-argument-count`, `fetch-archive-pax-global-header`, `fetch-archive-gnu-sparse`, `fetch-archive-pax-size`, `fetch-archive-gzip-crc-after-end-marker`, `fetch-archive-invalid-header-after-members`, `fetch-archive-extension-header-over-limit`, `fetch-archive-gzip-header-field-over-limit`, `fetch-unicode-15-1-path`, `pack-unicode-16-digit-part-bytes`, `pack-lone-surrogate-selection`, `publish-leftover-temporary`, `pack-dash-led-out-python-3-14-rule`, `pack-int-control-whitespace-part-bytes` |
| [`native-artifacts-decisions.v5.json`](../../tools/behavior_contracts/native-artifacts-decisions.v5.json) | `native-artifacts`, `evidence archive-transcripts` | 40 | `file-archived-tree`, `file-loose-tree`, `file-existing-filing`, `file-interrupted-before-storage-record`, `native-artifacts-unicode-15-1-path`, `verify-storage-record-not-object`, `verify-bzip2-archive`, `verify-encrypted-member`, `file-lone-surrogate-index`, `file-non-escape-surrogate-index`, `file-non-canonical-escape-index`, `file-reserved-code-point-index`, `archive-transcripts-existing-different`, `archive-transcripts-symlink-transcript`, `archive-transcripts-same-member-name`, `archive-transcripts-missing-transcript` |
| [`evidence-inventory-decisions.v5.json`](../../tools/behavior_contracts/evidence-inventory-decisions.v5.json) | `evidence inventory`, `collect` and `reconcile` | 54 | `collect-run-checksum-mismatch`, `inventory-unicode-15-1-output-parent`, `collect-run-unavailable`, `collect-run-unsafe-recorded-path`, `collect-run-other-records-archive-refused`, `collect-run-partly-named-loss-refused`, `collect-run-no-such-record`, `collect-run-ambiguous-stored-identity-refused`, `collect-run-changed-release-copy-refused`, `collect-run-lone-surrogate-record`, `collect-run-dangling-attempt-record`, `collect-run-named-user-home`, `collect-run-linked-mapping-replaced`, `collect-run-dangling-destination-link`, `collect-run-unicode-15-1-record`, `reconcile-existing-output`, `reconcile-invalid-record`, `reconcile-duplicate-path`, `reconcile-missing-entries`, `reconcile-unicode-15-1-inventory-path`, `reconcile-lone-surrogate-inventory`, `inventory-out-parent-traversal-into-root`, `inventory-out-symlinked-parent` |
| [`import-benchmark-decisions.v3.json`](../../tools/behavior_contracts/import-benchmark-decisions.v3.json) | `import-benchmark` | 34 | `import-check-changed-file`, `import-check-changed-preserved-and-run-file`, `import-check-missing-manifest`, `import-check-invalid-json`, `import-check-entry-without-hash`, `import-check-lone-surrogate`, `import-check-release-unavailable`, `import-check-unreadable-evidence-manifest`, `import-no-arguments`, `import-source-retired`, `import-help`, `import-preserve-unknown-path`, `import-preserve-refuses-different-copy`, `import-preserve-unrecoverable`, `import-preserve-outside-git`, `import-preserve-other-failure-keeps-manifest`, `import-preserve-symlinked-destination`, `import-check-trailing-double-dash`, `import-preserve-trailing-double-dash`, `import-preserve-dot-digit-name`, `import-preserve-exponent-name`, `import-preserve-unicode-15-1-path`, `import-preserve-dash-led-name-with-space`, `import-preserve-unicode-digit-name`, `import-preserve-negative-exponent-name` |

Eight of these follow Python 3.14's Unicode 16.0 tables, where CI's Python 3.12 and older interpreters differ, and
each states that impact: `fetch-unicode-15-1-path`, `pack-unicode-16-digit-part-bytes`,
`pack-dash-led-out-python-3-14-rule`, `native-artifacts-unicode-15-1-path`, `inventory-unicode-15-1-output-parent`,
`collect-run-unicode-15-1-record`, `reconcile-unicode-15-1-inventory-path` and `import-preserve-unicode-15-1-path`.
[Processing](processing.md#behavior-the-corpora-do-not-pin) describes the shared `repr` rule behind the
`unicode-15-1` cases.

## Close-out

Each responsibility the v3 inventory assigned to #155 now has one of three outcomes. It is migrated, with its Python
deleted or kept as a retained library. It is retired. Or it moved to the later stage named in the last column, which
the v4 inventory records. The v4 inventory assigns no module to #155.

| v3 #155 responsibility | Outcome | v4 owner |
| --- | --- | --- |
| `recover_evidence`, `local_evidence_resolution` | Migrated to `bench-tools evidence recover` and `restore-local`; the Python is deleted | None |
| `collect_run`, `reconcile_evidence` | Migrated to `bench-tools evidence collect` and `reconcile`; the Python is deleted | None |
| `import_benchmark` | `--check` and `--preserve` migrated to `bench-tools import-benchmark`; `--source` retired; the Python is deleted | None |
| `evidence_store` | `pack`, `publish`, `fetch`, `verify` and `resolve` migrated to `bench-tools evidence`; retained library | #159 |
| `native_artifacts` | `verify`, `restore` and `file_artifacts` migrated to `bench-tools native-artifacts`; retained library | #159 |
| `evidence_inventory` | Its command migrated to `bench-tools evidence inventory`; retained library | #159 |
| `builtin-review-benchmark-2026-09-24/answers.py` | Historical replay. The census kept it active only because `tools/import_benchmark.py` listed its path, which [#223](https://github.com/kamui/code-review-bench/issues/223) removed | None |
| The evidence parts of `file_attempt.py` | Migrated to `bench-tools evidence archive-transcripts` and `native-artifacts file`; #157 moved filing to `bench-tools file-attempt` and deleted `file_attempt.py` | None |
| The contributor CLI's packing and publication | Migrated to `bench-tools evidence pack`, `check-package` and `publish`; a temporary bridge | #158 |
| Evidence restoration in the explorer export, which v3 left in Python for #155 | Migrated to `bench-tools export-explorer evidence --restore`; the front stays with #156 | #156 |
| `bun run verify:import` and `bun run evidence:fetch` | Run `bench-tools` | None |

`evidence-migration-2026-10-08/restore-local.py` was historical replay in v3. In v4 it is active again: the
evidence-store decision records from [#217](https://github.com/kamui/code-review-bench/issues/217) on describe the
Python baseline as that script, by its full path, and the census reads a full path in a maintained file as a caller.
No command or test runs it, and it cannot run on this checkout because `local_evidence_resolution` is deleted.
Decision history is never rewritten, so it stays active until the census itself changes. v4 assigns it to
[#160](https://github.com/kamui/code-review-bench/issues/160), which owns the census.

## Research records

`restore-local.py` in `docs/research/evidence-migration-2026-10-08` imports `local_evidence_resolution`, and
`verify-recovery.py` in `docs/research/evidence-completion-2026-10-08` imports `recover_evidence`. Both stay
byte-unchanged and replay at a commit that holds those modules, such as `198760baf`. `bench-tools evidence
restore-local` takes `restore-local.py`'s arguments, and `evidence recover` replaces the recovery it checks.

`verify-cold.py` and `references.py` in `docs/research/portable-evidence-2026-10-10`, `verify-pilot.py` in
`docs/research/evidence-migration-2026-10-08` and `publish.py` in `docs/research/skill-matrix-2026-10-02` import or run
`collect_run`, and `verify-completion.py` in `docs/research/evidence-completion-2026-10-08` imports
`reconcile_evidence`. No test loads them. They stay byte-unchanged and replay at a commit that holds those modules,
such as `9b2b0ad9f`.

## Verification

```sh
cd rust/bench-tools && cargo fetch --locked && cargo fmt --check && cargo clippy --locked --all-targets -- -D warnings && cargo test --locked
python3 bench/tools/rust_runtime.py install --crate rust/bench-tools --select
bun run verify:import
```

`cargo test --locked` runs the storage corpora through the [corpus runner](contracts/README.md#runner-moved) since
#160, which dropped their differentials against the retained Python. Each `archive` decision states what Python 3.12
and 3.14 do with its fixture.
