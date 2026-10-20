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
[`normalize-review.v1.json`](../../../tools/behavior_contracts/normalize-review.v1.json),
[`normalize-thermo.v1.json`](../../../tools/behavior_contracts/normalize-thermo.v1.json),
[`derive-packet.v1.json`](../../../tools/behavior_contracts/derive-packet.v1.json),
[`build-packet.v1.json`](../../../tools/behavior_contracts/build-packet.v1.json),
[`export-explorer.v1.json`](../../../tools/behavior_contracts/export-explorer.v1.json),
[`check-manifest.v1.json`](../../../tools/behavior_contracts/check-manifest.v1.json),
[`rates.v1.json`](../../../tools/behavior_contracts/rates.v1.json),
[`upstream.v1.json`](../../../tools/behavior_contracts/upstream.v1.json),
[`ruling-record.v1.json`](../../../tools/behavior_contracts/ruling-record.v1.json),
[`ruling-dossier.v1.json`](../../../tools/behavior_contracts/ruling-dossier.v1.json),
[`current-grading.v1.json`](../../../tools/behavior_contracts/current-grading.v1.json),
[`claims.v1.json`](../../../tools/behavior_contracts/claims.v1.json),
[`calibration.v1.json`](../../../tools/behavior_contracts/calibration.v1.json),
[`evaluator-audit.v1.json`](../../../tools/behavior_contracts/evaluator-audit.v1.json),
[`methodology.v1.json`](../../../tools/behavior_contracts/methodology.v1.json),
[`archive.v1.json`](../../../tools/behavior_contracts/archive.v1.json),
[`regrade-reports.v1.json`](../../../tools/behavior_contracts/regrade-reports.v1.json) and
[`audit-log.v1.json`](../../../tools/behavior_contracts/audit-log.v1.json) pin the commands of
[`bench-tools`](../processing.md). Select a corpus with `--corpus`:

```sh
python3 tools/behavior_contract.py --corpus tools/behavior_contracts/roster.v1.json
python3 tools/behavior_contract.py --corpus tools/behavior_contracts/diff-identity.v1.json --differential /tmp/rust.json
```

Without `--implementations`, each program runs its maintained implementation. `codex`, `transcript`,
`diff-identity` and `normalize-review` run their Python scripts; `diff_identity.py` and `normalize_review.py` are
retained libraries. Since [#234](https://github.com/kamui/code-review-bench/issues/234), `check-manifest` runs
`bench-tools`, and `behavior_contract.python_commands` maps it back to the retained `check_manifest.py` for the
differential. `rates` and `upstream` run the retained
`rates.py` and `upstream.py` through drivers that read the case directory as the repository and, for `rates`, take
`--source PROVIDER=URL` pages, so no case leaves the host; `rates-transport-failure` connects to a closed loopback
port. `regrade-archive-attempt` runs the retained `regrade.archive_attempt` through a driver that reads the case
directory as the repository. `ruling` runs the retained `ruling_record.py` and `ruling_dossier.py` through a driver that
reads the case directory, or the `--root` argument, as the repository. `current-grading` runs the retained
`current_grading.py` the same way; its `fingerprints` operation, which the Python command line lacks, prints each
selected batch with `current_grading.batch_state`'s fingerprint and state. `methodology` runs the retained
`methodology.py` through a driver that reads the case directory as the repository. `claims` runs the retained
`claims.py` under the entropy driver, through a driver that rebinds `claims.py`, `current_grading.py` and `upstream.py`
to the case directory, or the `--root` argument. `upstream collect` cases run the stub `gh`, which answers each
paginated read from the case's `gh/upstream.json`. `calibration` and `evaluator-audit` run the retained
`calibration.py` and `evaluator_audit.py` through drivers that read the case directory, or the `--root` argument,
as the repository. Cases whose Rust behavior was changed on purpose
carry a `baseline_defect` with Python's output, so pass
`--allow-known-defects` when Python is the primary. `roster`, `normalize-thermo`, `derive-packet`, `build-packet`,
`export-explorer`, `archive`, `regrade-status` and `audit-log` have no Python implementation, so they, and
`check-manifest`, run the executable `rust_runtime.resolve("bench-tools")` selects, and the command refuses when none is installed for `HEAD`. A command map for the Rust programs names that
executable:

```json
{
  "roster": ["/absolute/path/to/bench-tools", "roster"],
  "diff-identity": ["/absolute/path/to/bench-tools", "diff-identity"],
  "normalize-review": ["/absolute/path/to/bench-tools", "normalize-review"],
  "normalize-thermo": ["/absolute/path/to/bench-tools", "normalize-thermo"],
  "derive-packet": ["/absolute/path/to/bench-tools", "derive-packet"],
  "build-packet": ["/absolute/path/to/bench-tools", "build-packet"],
  "export-explorer": ["/absolute/path/to/bench-tools", "export-explorer"],
  "check-manifest": ["/absolute/path/to/bench-tools", "check-manifest"],
  "rates": ["/absolute/path/to/bench-tools", "rates"],
  "upstream": ["/absolute/path/to/bench-tools", "upstream"],
  "archive": ["/absolute/path/to/bench-tools", "archive"],
  "regrade-status": ["/absolute/path/to/bench-tools", "regrade", "status", "--"],
  "regrade-archive-attempt": ["/absolute/path/to/bench-tools", "regrade", "archive-attempt"],
  "audit-log": ["/absolute/path/to/bench-tools", "audit-log"],
  "ruling": ["/absolute/path/to/bench-tools", "ruling"],
  "current-grading": ["/absolute/path/to/bench-tools", "current-grading"],
  "methodology": ["/absolute/path/to/bench-tools", "methodology"],
  "claims": ["/absolute/path/to/bench-tools", "claims"],
  "calibration": ["/absolute/path/to/bench-tools", "calibration"],
  "evaluator-audit": ["/absolute/path/to/bench-tools", "evaluator-audit"],
  "grading": ["/absolute/path/to/bench-tools", "grading"],
  "grade": ["/absolute/path/to/bench-tools", "grade"],
  "rust-grade": ["/absolute/path/to/bench-tools", "grade"]
}
```

The normalizer corpora pin `normalized.json` bytes, the stdout and stderr that `dispatch.sh` saves as `normalize.txt`
and the skill runners as `normalization.txt`, and every exit code. A successful `--timing` stamp depends on the clock,
so cargo tests pin that rewrite; the corpora pin the sidecar left unchanged by an unresolved result and the failures.
A `normalize-thermo` diagnostic that names the resolved artifact root holds the run's temporary path, so cargo tests
pin those too.

The `diff-identity` fixtures are bare repositories whose non-UTF-8 path names exist only inside Git tree objects,
so they need no `requires` capability. The cases run `git` from the minimal `PATH`.

Each `derive-packet-target-<id>` case holds a target's committed `packet.legacy.md` and expects its committed
`packet.md`, byte for byte. `test_behavior_contract.py` checks that every target with a legacy packet has such a case
and that its bytes are the committed files.

Each `build-packet` case holds its staging mirror as a bare repository with one pack and its saved forge responses
under `replay/`, so no case calls `gh` or the network. `build-packet-non-utf8-output-path` requires `non-utf8-paths`.

## Run the grading validation corpus

[`grading-validation.v1.json`](../../../tools/behavior_contracts/grading-validation.v1.json) pins
`bench-tools grading validate [VERDICTS] --inputs FILE` ([#231](https://github.com/kamui/code-review-bench/issues/231)).
Its `grading` program runs the retained `grading_validation.py` through a driver that takes `--inputs`, which must
name a file called `inputs.json`, and points the script at it as `grade.py prepare`'s `validator/tools/` layout
does. Twelve cases carry a `baseline_defect`, so pass `--allow-known-defects` when Python is the primary:
`non-object-inputs` and `links-not-an-object`, where Python raises an uncaught `AttributeError`; `deep-nesting` and
`lone-surrogate`, which `pyjson` refuses; `nan-review-tokens`, whose Python line order depends on a `NaN`'s address;
`unicode-15-1-outcome`, quoted through Python 3.14's `repr`; `dash-led-exponent-path` and
`dash-led-unicode-16-digit-path`, read as values as Python 3.14's argparse reads them (these three record Python
3.12's result); and `help`, `help-before-invalid-short-help`, `missing-inputs-option` and `extra-argument`,
which print clap's wording. `dash-led-paths`
pins the dash-led values both Pythons accept. The `fixture-*` cases build on the grading
fixture's `pending`, `proposed` and `disputed-without-ruling` variants, and two `saved-v1-*` cases hold archived
validator inputs and verdicts.

```sh
python3 tools/behavior_contract.py --allow-known-defects --corpus tools/behavior_contracts/grading-validation.v1.json --differential /tmp/rust.json
```

## Run the grade preparation corpus

[`grade-prepare.v1.json`](../../../tools/behavior_contracts/grade-prepare.v1.json) pins `bench-tools grade prepare`,
`preflight-inputs` and `check-prepared` ([#237](https://github.com/kamui/code-review-bench/issues/237)). Its `grade`
program runs `bench-tools`, and `behavior_contract.python_commands` maps it to the `python-grade` driver: the retained
`grade.py` under the entropy driver, with the case directory as the checkout. There `preflight-inputs` is
`grade.py preflight --offline`, and `check-prepared` prints what `grade.check_prepared` finds. `rust-grade` runs
`bench-tools` in every map, so the v2 key cases prepare a key with it and then run the retained `grade.py dispatch` and
`map` on that key. The v1 and v2 key cases also check each key against
[`current-grading-key.v1.schema.json`](../../../bench/schema/current-grading-key.v1.schema.json).

Five cases carry a `baseline_defect`, so pass `--allow-known-defects` when Python is the primary. The `pending`,
`proposed` and `disputed-without-ruling` fixture cases differ only in what decision
`grade-prepare-runner-identity-v2` lists: `runner_deviation`, the missing `validator/tools/` and the hashes that pin
them. `grade-prepare-arguments` prints clap's usage. `grade-prepare-unicode-16-digit-source-link` records Python
3.12's result, as CI runs it, so the differential reports it under Python 3.14.

```sh
python3 tools/behavior_contract.py --corpus tools/behavior_contracts/grade-prepare.v1.json
python3 - > /tmp/grade-python.json <<'EOF'
import json, sys
from pathlib import Path
sys.path.insert(0, "tools")
import behavior_contract
corpus = behavior_contract.load_corpus(Path("tools/behavior_contracts/grade-prepare.v1.json"))
print(json.dumps(behavior_contract.python_commands(corpus)))
EOF
python3 tools/behavior_contract.py --allow-known-defects --corpus tools/behavior_contracts/grade-prepare.v1.json --implementations /tmp/grade-python.json --differential /tmp/rust.json
```

## Run the storage corpora

[#217](https://github.com/kamui/code-review-bench/issues/217) pins the storage behavior that
[#155](https://github.com/kamui/code-review-bench/issues/155) ports, against the current Python, before any Rust:

| Corpus | Programs | Python run |
| --- | --- | --- |
| [`evidence-store.v1.json`](../../../tools/behavior_contracts/evidence-store.v1.json) | `evidence-store`, `evidence`, `python-resolve`, `recover-evidence`, `restore-local` | `evidence_store.py` directly; `python-resolve` calls the retained `evidence_store.resolve` as `prune_workspace.py` does. `evidence`, `recover-evidence` and `restore-local` run `bench-tools evidence`, `evidence recover` and `evidence restore-local` |
| [`native-artifacts.v1.json`](../../../tools/behavior_contracts/native-artifacts.v1.json) | `native-artifacts`, `archive-transcripts`, `python-native-artifacts` | `native_artifacts.py` through the `python-native-artifacts` driver, which adds `file ROOT INDEX OUT [--native-relative PATH]`: it runs `file_artifacts` and `unindexed` and prints both. `archive-transcripts` runs the former `file_attempt.archive_transcripts` through the `python-archive-transcripts` driver. Since [#221](https://github.com/kamui/code-review-bench/issues/221), `native-artifacts` and `archive-transcripts` default to `bench-tools`, and `behavior_contract.python_commands` maps them back to those drivers for the differential; step cases run `python-native-artifacts` beside Rust to check each reads the other's filing |
| [`evidence-inventory.v1.json`](../../../tools/behavior_contracts/evidence-inventory.v1.json) | `evidence-inventory`, `collect-run`, `reconcile-evidence` | `evidence_inventory.py` directly. `collect-run` and `reconcile-evidence` run `bench-tools evidence collect` and `evidence reconcile` in the case directory |
| [`import-benchmark.v1.json`](../../../tools/behavior_contracts/import-benchmark.v1.json) | `import-benchmark` | None since [#223](https://github.com/kamui/code-review-bench/issues/223) deleted `import_benchmark.py`; `bench-tools import-benchmark`. Before that, `import_benchmark.py` ran through the same kind of driver |

```sh
python3 tools/behavior_contract.py --allow-known-defects --corpus tools/behavior_contracts/evidence-store.v1.json
```

The cases cover `pack`, `publish`, `fetch` (online and `--offline`) and `verify` with and without `--receipt`, recovery,
local restoration, native `verify`, `restore` and filing, the inventory and transcript collection. They pin each
refusal #155 names: traversal, `.git`, backslash, absolute and out-of-root paths, symlinked inputs and ancestors,
special files, credential names, malicious archives, changed and missing files, mismatched hashes, draft and replaced
releases, oversized and truncated downloads, and existing different manifests, receipts and filings.
`cold-offline-restoration` publishes, removes the sources, fetches, empties the remote, restores again with
`--offline`, then refuses a corrupted cache. `publish-retry-makes-no-upload` and `publish-lost-upload-response` show
one upload in the recorded `gh` calls. Each corpus runs a stub `gh`, so `gh/calls.jsonl` records every call;
`test_behavior_contract.py` checks that each recorded call is one the stub answers and runs every storage corpus
with its default commands: the retained Python for `evidence-store` and `python-resolve`, and `bench-tools` for the
programs that moved.

[#219](https://github.com/kamui/code-review-bench/issues/219) moves fetch, verify, resolution, recovery and local
restoration to `bench-tools evidence`. A command map value can route a program by its first argument, so
`evidence-store` can run Rust `fetch` and `verify` against Python `pack` and `publish`:

```json
{
  "evidence-store": {
    "fetch": ["/absolute/path/to/bench-tools", "evidence"],
    "verify": ["/absolute/path/to/bench-tools", "evidence"],
    "pack": ["/absolute/path/to/python3", "/absolute/path/to/bench/tools/evidence_store.py"],
    "publish": ["/absolute/path/to/python3", "/absolute/path/to/bench/tools/evidence_store.py"]
  }
}
```

A `shell` step runs one command line with `/bin/sh -c` in the case root, for example the move-aside commands a
refused fetch prints. `fetch-interrupted-*` cases leave the state a crash leaves at each durable step and fetch again.
`resolve-across-implementations` restores through Rust and then resolves through Python.

[#222](https://github.com/kamui/code-review-bench/issues/222) moves collection, the inventory command and
reconciliation to `bench-tools evidence`. Map `evidence-inventory` to `bench-tools evidence inventory` to run the
inventory cases against Rust; `inventory-non-utf8-names` and `inventory-links-special-files-and-hard-links` pin its
non-UTF-8 names, symlinks, special files and hard links. `reconcile-*` cases compare the gzip reconciliation by its
decompressed bytes.

[#220](https://github.com/kamui/code-review-bench/issues/220) moves `pack` and `publish` too, so the `Rust runtime`
workflow routes the whole `evidence-store` program to `bench-tools evidence` and compares it with the retained Python.
`check-package-*` cases run `bench-tools evidence check-package`, which has no Python command, and
`pack-restored-by-python` packs and publishes through Rust, then restores through Python's `evidence_store.resolve`.

Where Python raises a traceback, the case carries it as a `baseline_defect` and expects the intended exit and
message, so these corpora run with `--allow-known-defects`: `recover-unknown-source`, `recover-changed-manifest`,
`restore-local-not-published`, `restore-local-mismatched-resolution`, `fetch-archive-truncated-gzip`,
`import-check-changed-file`, and the `collect-run-*` and `reconcile-*` cases that
`evidence-inventory-decisions.v3.json` decides. Each decision records the old and new behavior.
Each observation masks the traceback's frames with `mask_patterns` and pins the terminal exception type and message,
so an unrelated crash is a failure, not the known defect.

[#223](https://github.com/kamui/code-review-bench/issues/223) ports the `--check` and `--preserve` of `import_benchmark.py` to
`bench-tools import-benchmark`. `import-benchmark-decisions.v3.json` records each case where the two differ, every
other traceback included, with Python's output as its `baseline_defect`. The corpus ran against both, with a
differential, before the Python was deleted; its expectations are now the gate. The `import-preserve-*` cases that
recover bytes from Git history hold a small `.git` directory, without config, in their `initial` entries.

## Grading comparison features

[#230](https://github.com/kamui/code-review-bench/issues/230) adds the comparisons the
[#156](https://github.com/kamui/code-review-bench/issues/156) grading corpora need, before any grading code is
ported. [`grading.md`](../grading.md) records the baseline they serve.

- **Structured comparison.** A case's `structured` object compares outputs that carry blind tokens, shuffles,
  clocks or runner identity. In order, the harness:
  1. checks every pin the `pins` rules find against the bytes it names, before anything is normalized. A rule
     names a `glob` and optional `base` (a directory, or `record` for the record's own directory). With no other
     field it finds every object holding a string `path` and `sha256`; `map` names an object of path-to-hash pins;
     `pointer` with `path` names one hash field and its file. A wrong or dangling pin is a `consistency:` difference
     in either implementation's output;
  2. applies only the declared normalizations. `clocks` and `identity` rules replace JSON pointers with
     `"<volatile>"` and a decided label. Each `sections` rule orders every run of adjacent Markdown sections under
     its `heading` prefix, such as `### `, by their bytes with tokens masked. Each `tokens` rule names a `prefix`,
     such as `blind` or `review`, the `sites` where the run writes its generated tokens, and the `references`
     where it repeats them. Each site is a `glob` and the JSON `pointers` in each matching file; the
     `<prefix>-<6 to 8 hex>` values there, or with `"keys": true` the keys of the objects there, are the run's
     generated tokens. Every key of a keyed site must be such a token. A reference is one of:
     - `{"glob", "pointers"}`: the JSON string values at those pointers;
     - `{"glob", "pointers", "keys": true}`: the keys of the JSON objects at those pointers;
     - `{"glob", "line_starts"}`: a token written right after one of those line prefixes, such as `### `, in
       matching files, leaving the rest of the line exact;
     - `{"glob", "names": true}`: the names of matching files and directories;
     - `{"stream": "stdout"}` or `{"stream": "stderr"}`;
     - `{"glob"}`: the whole bytes of matching files.

     At the sites and references only, each generated token becomes its first-seen ordinal, such as `<blind 1>`,
     scanning stdout, stderr, then entries in path order, names before contents. Every other byte stays exact,
     including other occurrences of a generated token, so a changed quote or source byte differs whatever it
     holds. A whole-file reference must never name a file that carries quotes, source text or evidence; that is
     the case author's declaration, and the #156 corpora use pointer or key references for those files. A missing
     site, a site holding anything but a token of its prefix, an unreadable pointer reference, a generated token
     that the case's inputs already hold (raw, or in any string a JSON input decodes to), and output text already
     in a label's form, such as `<blind 2>`, are `consistency:` differences. A tokens rule with
     `"ordinals": "sites"` numbers its tokens first in the order its sites hold them, for tokens that every other
     place holds in an order their random values set;
  3. orders each run of adjacent lines that start with a `lines` rule's `start`, in files matching its `glob`, by
     their relabeled bytes. It then orders, for each `members` rule, the members of the objects at its `pointers` by key in files matching its
     `glob`. Such a file must be written as `json.dumps(value, indent=2)` and a newline, or the rule refuses it.
     With `mask_root`, the case root in file contents becomes `<root>` before these steps;
  4. recomputes every pin over the normalized bytes it names and compares the trees byte for byte. A dropped,
     duplicated or changed section, quote, source byte, credit fact, unresolved state or pin still differs.

  A structured case declares none of `archives`, `digests`, `manifests`, `references` or `volatile`. Clock and
  identity pointers name fields without tokens.
- **Independent outcomes.** A command's `expected.outcomes` lists hand-stated grading results, each with a `kind`
  (`family-outcome`, `credit-fact`, `unresolved-state`, `candidate-state`, `prepared-review` or
  `prepared-family`), a `path` or the `stream` that printed
  JSON, an RFC 6901 `pointer` and a `value`. Every implementation must produce them, including the candidate of a differential, which must also match
  the primary.
- **Python entropy driver.** A program in `PYTHON_ENTROPY` runs under a driver that replaces `secrets.token_hex`
  and `random.SystemRandom` with a fixed sequence in the child process, so two recordings of a case are byte
  identical. The grading slices add their programs, each by its script or by the name of its Python driver;
  slice 5 adds `claims` and slice 7 `python-grade`.
- **`fixture-git`.** A setup program every command map runs the same way. `fixture-git [--directory DIR] [FILE ...]`
  commits DIR, by default the case root, with a fixed identity and clock and no compression. It leaves no config or
  message, rewrites the index from the commit without file timestamps, and writes the commit into each named file
  in place of `<freeze commit>`. A command that refreshes the index afterwards, such as `git status`, writes host
  timestamps into it.
- **Real-record mode.** `--real-record [LIST] --root ROOT --differential MAP` runs each command
  [`grading-real-record.v1.json`](../../../tools/behavior_contracts/grading-real-record.v1.json) lists against the
  primary implementation and the candidate on ROOT, by default this checkout. It compares exit status, stdout,
  stderr and each named output byte for byte. An `{"output": NAME}` argument names a file in a disposable
  directory. Every entry below ROOT, tracked, ignored or Git's own, is compared by type, permissions, size, inode
  and change time before and after each run. A run that changes one fails and stops the mode, so no later run
  reads changed inputs. Before the first run the mode creates `.cache/current-record.lock`, the file
  `current_grading.record_lock` creates on first use, if it is absent, and leaves an existing one unchanged, so a
  command that takes the lock on a fresh checkout does not read as a change. #156 slice 3 listed `current-grading check`, `status`, `inventory --out` and
  `fingerprints`, slice 4 added `methodology --out`, slice 5 added `claims check` on each tracked registry, slice 6 added `calibration check`,
  `calibration queue` and `evaluator-audit check`, and the Pages build job runs the mode after its evidence and freeze-commit fetches.
- **Fixture roots.** A case built from the grading fixture names it with `"fixture": "grading/<variant>"`. Its
  `initial` entries and leading setup steps must equal what
  [`grading_fixture.rs`](../../../rust/bench-tools/tests/support/grading_fixture.rs) builds, or
  `cargo test --test grading_fixture` fails.

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
- `initial`, a list of `file`, `directory`, `symlink` or `special` entries with base64
  relative `path_b64`. Files include `bytes_b64`, symlinks `target_b64`; a special entry
  is a FIFO made with `mkfifo`. Files, directories and FIFOs take an optional `mode`.
  Parent directories are created.
- `expected.exit`, the process exit status, and `stdout` and `stderr` comparators.
- `expected.filesystem`, the complete resulting tree, including directories,
  files with base64 bytes and SHA-256, and symlinks with base64 targets.
- `derivation`, the independent accounting or protocol argument for the expectation.
- Optional `baseline_defect`, with an exact `observed` comparator and a resolution link.
- Instead of `argv`, `stdin_b64` and `expected`, optional `steps` that share one root, in
  order. A command step holds `argv`, `stdin_b64`, `expected` and optionally `program`; a
  `remove` step deletes the listed paths that exist; a `write` step adds `initial`-style
  entries, replacing files; a `shell` step runs its command line with `/bin/sh -c` in the root
  and must exit 0. Every command step's `expected.filesystem` is the whole tree
  after it. A step case carries no `baseline_defect` of its own; a command step may carry one, and
  `--allow-known-defects` accepts the case when every differing step matches its own.

A corpus may opt in to two settings. `snapshot_modes: true` adds each file's and
directory's permission bits to the snapshot and runs setup and commands with umask 022;
corpora without it keep their snapshots and digests. `gh_stub_b64` names a state directory
in each case and puts a stub `gh` first on `PATH`. The stub appends every argument list
to `calls.jsonl` there and answers only the release reads, upload and asset download of
evidence storage, from `release.json` (repository, tag, ID, `draft`, `filler_assets` and
`faults`) and `assets/<id>/<name>`. The faults are `lost-upload-response`,
`partial-upload`, `rejected-upload`, `truncated-download` and `oversized-download`. A
missing `release.json` answers 404; other arguments exit 2.

A case may declare comparisons for outputs that legitimately vary:

- `archives`, glob and `tar.gz`, `zip` or `gzip` format: a matching file is compared by its
  members' names, sizes, modes and SHA-256 instead of its bytes, or a `gzip` file by its decompressed size, SHA-256
  and bytes, and its SHA-256 is
  replaced by `<archive N>` in every path, file and stream. Restored and pre-existing
  files match no declaration and keep byte comparison.
- `digests`, globs whose files' SHA-256 is replaced by `<sha256 of PATH>` the same way.
- `manifests`, evidence manifests whose every package must name the archive beside it by
  asset name, `sha256` and `bytes`, or the case fails with a `consistency:` difference.
  A declared archive's `bytes` is masked only after that check.
- `references`, a storage record path and its top-level `fields`, each an object whose
  `path` and `sha256` must name a file written beside the record, or the case fails with
  a `consistency:` difference. The check runs before any digest becomes a label.
- `volatile`, a file path and the RFC 6901 pointers of clock fields such as `verified_at`
  and `at`, replaced in place by `"<volatile>"`.
- `mask_root`, which replaces the absolute case root with `<root>`, and `mask_patterns`,
  regular expressions over stdout and stderr, such as Python's temporary directory names.
The runner creates a fresh temporary directory and an empty `.home` for every
implementation and case. It sets that directory as cwd, gives the process a minimal
environment without inherited credentials, captures output bytes, and removes the
root afterward. Commands time out after ten seconds. These are trusted local test
programs; the disposable cwd is not an OS sandbox. Commands must not access external
state or spawn surviving descendants. No model, network, or paid operations belong
in this corpus. Filesystem comparison covers the disposable tree, not arbitrary
external effects, hard-link identity, or file metadata other than the opted-in
permission bits. A special file is recorded by kind and mode and never opened.

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
