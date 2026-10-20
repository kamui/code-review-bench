# Grading migration record

This records [#156](https://github.com/kamui/code-review-bench/issues/156), part of
[#149](https://github.com/kamui/code-review-bench/issues/149), under its
[delivery plan](https://github.com/kamui/code-review-bench/issues/156#issuecomment-6265525205).
Slice 0, [#230](https://github.com/kamui/code-review-bench/issues/230), starts it: the comparisons, the fixture
root and the replay inventory every later slice runs, and the research-helper audit. It ports no grading code and
changes no grade, claim decision, ruling or scorecard number. `src/lib/scoring.ts` stays the only owner of
scorecard arithmetic. Slice 2, [#232](https://github.com/kamui/code-review-bench/issues/232), ports the ruling
records and delegation routing, as [ruling records and delegation](#ruling-records-and-delegation) describes.
Slice 3, [#233](https://github.com/kamui/code-review-bench/issues/233), ports the current grading record and packet
selection, as [slice 3](#slice-3-the-current-grading-record-and-packet-selection) describes. Slice 4,
[#234](https://github.com/kamui/code-review-bench/issues/234), moves the explorer export's facts, `methodology` and
the schema-check callers, as [slice 4](#slice-4-the-explorer-exports-facts-methodology-and-schema-check-callers)
describes. Slice 5, [#235](https://github.com/kamui/code-review-bench/issues/235), ports the shared claims and
upstream collection, as [slice 5](#slice-5-shared-claims-and-upstream-collection) describes. Slice 6,
[#236](https://github.com/kamui/code-review-bench/issues/236), ports the calibration and evaluator audit checks, as
[slice 6](#slice-6-calibration-and-evaluator-audit-checks) describes. Slice 7,
[#237](https://github.com/kamui/code-review-bench/issues/237), ports grading preparation under runner identity v2, as
[slice 7](#slice-7-grading-preparation-and-runner-identity-v2) describes.

## The split with #157

Recorded on [#157](https://github.com/kamui/code-review-bench/issues/157#issuecomment-6265554250) before any
implementation.

| Code | #156 | #157 |
| --- | --- | --- |
| `grade.py` | `prepare`, `preflight-inputs`, `check-prepared`, `validate`, `inventory`, `map`, `invalidate`; runner identity v2 | `dispatch`, `dispatch_codex`, online `preflight`, `meter`, `audit`. After slice 10, `grade.py` holds only these |
| `regrade.py` | Its reads of grading records and `archive_attempt`. Slice 10 points those calls and its `grade` calls at `bench-tools`, and adds the `runtimes` pin | `execute`, workers, reservations and ledger, meters, client pin, lock, `status.json` |
| `regrade_queues.sh` | `status` (slice 9) | `run` |
| `grading_policy.py`, `grading-hosts.v1` | The command its `validate` tool runs (slice 10) | The confined tool server and sandbox |
| `attempt_audit.py` | — | The attempt isolation audit |
| `grading_profile.py` | — | Ports it on the Rust usage parser |
| `transcript_usage.py`, `codex_usage.py` | — | Proposed: the production usage parser |
| `packet_selection.py` | Rust port for `current_grading` | Retained library until its callers move |

`delegation.py` and `audit_log.py` stay with #156.

## Reassignments

- `attempt_audit`, `grading_policy`, `grading-hosts.v1` and `grading_profile` move to #157. They stay
  byte-unchanged until slice 10, where `grading_policy.py` and `grading_client_probe.py` change only in how the
  validator is invoked.
- `claim_grading.scoring_items` has no caller and is not ported.
- `check_manifest.py`'s command-line callers moved to `bench-tools check-manifest` in slice 4; the library stays for
  its importers until #160.

## Comparison features and fixture root

[The contract reference](contracts/README.md#grading-comparison-features) describes the harness features: the
structured comparison, hand-stated grading outcomes, the Python entropy driver, the `fixture-git` setup program and
real-record mode. `grading-real-record.v1.json` lists slice 3's four `current-grading` commands, slice 4's
`methodology` command, slice 5's `claims check` on each tracked registry and slice 6's `calibration check`,
`calibration queue` and `evaluator-audit check`, which the Pages build job runs.

[`grading_fixture.rs`](../../rust/bench-tools/tests/support/grading_fixture.rs) is the one fixture root. It mirrors
the cohort of `test_grade.py`: one task, a selected and an unselected arm, an invalid attempt and its replacement, a
stopped attempt, and current records under verdict contract v3 with a remedy policy. It writes inputs only; a case
derives `inventory.json` with the program it runs. `cargo test --test grading_fixture` checks that the output is the
same on every run, that every corpus case naming `grading/<variant>` holds those entries, and that the retained
`current_grading.py` and `ruling_record.py` read each variant as named:

| Variant | What it holds | What `current_grading.py` reports |
| --- | --- | --- |
| `pending` | Both families await an eligibility ruling | Valid, 0/2 admitted reviews assessed, incomplete |
| `proposed` | An automated eligibility decision, not approved | Valid and incomplete |
| `approved-without-receipt` | An approved decision whose pinned receipt was never saved | Refused, naming the receipt |
| `disputed-without-ruling` | An equivalent link whose blind parties disagree, with a before-record and no after-record | Valid and incomplete; the before-record has no fault and awaits a ruling |
| `ruled` | The same link after the owner's saved ruling, with an after-record pinning the before-record | Valid and incomplete; the before-record and after-record have no fault |
| `stale-fingerprint` | A saved grade whose input fingerprint does not match | Refused: grade fingerprint is stale |
| `changed-pinned-file` | `packet.md` edited after `target.json` pinned it | Inventory refused: packet bytes changed |
| `modified-clone` | A grading clone, committed by `fixture-git`, with an uncommitted change | `git status` shows ` M main.go` |
| `packet-replacements` | A run frozen on a re-cut packet, committed by `fixture-git` | Valid; the inventory selects `packet.v2.md` |

Limits for later slices:

- A command that runs `git status` in the clone rewrites its index with host timestamps, so slice 8's cleanup cases
  need an index the snapshot can compare.
- The structured comparison orders dossier sections but not JSON members. The dossier's provenance key lists its
  review tokens in shuffle order, so slice 5 either records that order by decision or declares a member ordering.

## Ruling records and delegation

Slice 2 ports `ruling_record.py`, `ruling_summary.py`, `ruling_dossier.py` and `delegation.py` to
`bench_tools::ruling` (`record`, `summary`, `dossier`) and `bench_tools::delegation`: record checks, routing,
summaries, precedent sheets, round openings and closed-round pins, trust misses, model replacement, per-round levels
and the filing check `current_grading` runs through `filing_faults`. `bench-tools ruling` runs them with the Python
arguments, stdout and exit codes:

```sh
python3 bench/tools/rust_runtime.py exec bench-tools -- ruling check <ruling>.before.json [<ruling>.after.json]
python3 bench/tools/rust_runtime.py exec bench-tools -- ruling route <ruling>.before.json [--policy PATH]
python3 bench/tools/rust_runtime.py exec bench-tools -- ruling summary [--round NAME] [--json]
python3 bench/tools/rust_runtime.py exec bench-tools -- ruling precedents --type TYPE --exclude GROUP --out SHEET
python3 bench/tools/rust_runtime.py exec bench-tools -- ruling dossier DIRECTORY
```

`check` is `ruling_record.py` without a command word. Each command reads the working directory as the repository,
or `--root DIR`; record paths stay relative to the working directory. Diagnostics keep the program names
`ruling_record.py` and `ruling_dossier.py`. No command writes a ruling, receipt, record or decision; `precedents`
writes only a new sheet. The callers in `docs/adjudication-record.md`, `docs/claim-adjudication.md`,
`docs/ruling-dossier-brief.md` and `docs/ruling-preparation-evidence.md` moved, and
`tools/test_moved_cli_callers.py` fails on a current caller of the Python command lines.

[`ruling-record.v1.json`](../../tools/behavior_contracts/ruling-record.v1.json) and
[`ruling-dossier.v1.json`](../../tools/behavior_contracts/ruling-dossier.v1.json) pin the commands. They cover the
`pending`, `proposed`, `approved-without-receipt`, `disputed-without-ruling` and `ruled` fixture roots, where no command
resolves a state, and synthetic rounds under an adopted policy v2. `cargo test --test grading_fixture` fails when either
corpus lacks one of the four unresolved variants. Route settles only an open decision type that
earned its level in the closed rounds the round's opening pins: level 1 after 15 agreed owner decisions on 4 pull
requests, level 2 after 30 on 5. It asks for a disputed answer, a credit fact nobody could tell, a changed model in a
pending record, a miss inside the round, a round with no opening pin and a level-2 history the opening does not pin.
It never settles a pre-existing decision type, and `summary` refuses an agent settlement of one.

The decisions record each difference from the retained Python:

- an exception the Python command line did not catch, a traceback with exit 1, exits 2 with one line naming its
  class; usage errors and help are clap's;
- a record holding a lone surrogate, and a non-UTF-8 `--exclude`, are refused with exit 2;
- `\d` follows Python 3.14's Unicode 16.0 digits in versioned rule names, round dates and natural ordering, and
  quoted paths follow its printable characters;
- `Path.stem`, `**` inside a glob component and `is_file` under an unsearchable directory follow Python 3.14's
  pathlib.

`tests/ruling_filing.rs` compares `filing_faults` with `delegation.py` for each refusal, since no command reaches it.

The four Python modules stay byte-unchanged as the baseline: `current_grading.py` imports `delegation`, and the
behavior corpora and `tests/ruling_filing.rs` run them. Every tracked record was compared with the Python on
2026-10-19: `check` with and without its after-record, and `route`, for each of the 200 before-records; `summary`, with
and without `--json`, for the whole record and each of the 6 rounds; `precedents` for each decision type; and
`dossier` for every directory holding a `summary.json`. All 692 commands gave identical exit status, stdout, stderr
and sheets under Python 3.12 and 3.14, except four `dossier` runs on research directories whose `summary.json` is not
a dossier, where the Python crashed and Rust exits 2 as decided. The real-record list runs only `current-grading`,
`methodology`, `claims`, `calibration` and `evaluator-audit` commands; no `ruling` command repeats this comparison in Pages yet.

## Replay coverage

[`grading-replay.v1.json`](grading-replay.v1.json) accounts for the 376 receipts under
`bench/grading/current/assessments/`. `rust/bench-tools/tests/grading_replay.rs` writes it after the evidence fetch,
and the Pages build job fails when the committed file differs.

| Verdict contract | Receipts | Filed by | Validator replays | Mapping replays |
| --- | --- | --- | --- | --- |
| `current-verdicts/v1` | 333 | `5521884a8` (325), `8738936bc` (8) | 333 | 333, all refused alike |
| `current-verdicts/v2` | 29 | `9be17965f` | 0 | 0 |
| `current-verdicts/v3` | 14 | `3cbcbd061` | 0 | 0 |

Each v1 receipt matches exactly one of the 751 archives under `bench/regrading/`. The archive restores by hash
through `repository-payloads-2026-10-08-v1.json`, its key, validator inputs and verdicts hash as `evidence.json`
lists, and the key's fields equal the receipt's. The 43 v2 and v3 receipts were filed with a receipt and no
verdicts. No archive or manifest in the repository holds their key, inputs or verdicts, so their contract comes
from the validation policy at the filing commit.

The mapping replay, `the_mapping_replays_every_restorable_archive_identically` in the same file, gives each
restorable archive's workspace and key to `grade.py map` and to `bench-tools grade map` with a fixture `--assessor`,
through `--root`, on a disposable worktree of the commit that filed its receipt. Each must end the same way in both:
refused with identical lines and nothing written but `.cache/current-record.lock`, which both create, or accepted
with identical lines and records that are equal once the mapping clock, `runner.mapping` and the receipt's own
digest are masked. The Pages workflow runs it in its own `Mapping replay` job, outside the build job's
time limit. On 2026-10-20 all 333 v1 replays were refused alike, exit 2: neither filing commit has
`bench/grading/current/credits.json`, which `grade.py map` reads before it writes and `43bd77593` added later. No
replay accepted, so the comparison of accepted records runs only in the fixture cases.

No contract has an accepted replay, so each contract's verdicts map in a case of
[`grade-map.v1.json`](../../tools/behavior_contracts/grade-map.v1.json). v2 and v3 have no archive to build a case
from, so `grade-map-verdicts-v2` and, for v3, `grade-map-pending-candidate` are built on the fixture root. The test
fails on a contract with no accepted replay and no case.

`grade-map-verdicts-v1` maps an archived batch: requests (`2026-09-24-builtin-baseline/i-requests-6667`) from
`bench/regrading/cohort-rebuild-2026-10-04-rebuilt-v4/.../attempt-1`, restored through the evidence store into a
disposable copy. It keeps the archived key, the six blinded reviews and the other prepared files, the verdicts, the
validator and the dispatch receipt with its command audits. Each of these files keeps the bytes `evidence.json` pins,
and `test_v1_mapping_fallback_holds_the_archived_key_and_work_bytes` checks them. The case adapts the archive in
these ways only:

- The record is the one the key was prepared against, at `94a99d397`, not the filing commit `5521884a8`, which
  changed the record after preparation. It is trimmed to the batch: the task, its three configurations and six
  attempts without runner-only logs (`audit.json`, `usage-requests.jsonl`, `dispatch.txt`, `timing.json`), its
  reference with the decisions and evidence those pin, no claims or candidates (the target had none), and an
  inventory regenerated from the trimmed record.
- An empty `credits.json` is added, which the current mapper reads and `94a99d397` lacked.
- The key's `input_fingerprint` is recomputed over the same inputs plus the empty credits. Every other key field is
  archived.

Both implementations map it to 17 caught, 5 missed and 2 unresolved, with two new candidates. The archive's
`map.log`, which the `94a99d397` mapper reproduces, reads 17, 2 and 5. The difference comes from `5521884a8`: since
then, an unresolved claim that names no family no longer holds every family open. That moves three family
recoveries of `att-001` and `att-056` from unresolved to missed.

## Slice 1: claim grading rules and the blinded validator

[#231](https://github.com/kamui/code-review-bench/issues/231) ports `claim_grading.py` (all but `scoring_items`),
`pre_existing_grading.py` and `grading_validation.py` to `bench_tools::claim_grading`, `bench_tools::pre_existing`
and `bench_tools::grading_validation`, and adds `bench-tools grading validate [VERDICTS] --inputs FILE`. Its stdout
and exit status match the script's; any input it cannot read prints
`cannot validate: unreadable or malformed JSON input`, exit 1. No caller moves until slice 10, and the three Python
modules stay as the baseline until slice 11.

- [`grading-validation.v1.json`](../../tools/behavior_contracts/grading-validation.v1.json) covers every message,
  each verdict contract and safety policy, a duplicate key, non-UTF-8 and non-object input, and the fixture's
  `pending`, `proposed` and `disputed-without-ruling` variants. Its twelve decided differences are in
  [the contract reference](contracts/README.md#run-the-grading-validation-corpus). The Rust runtime workflow runs it
  as primary and with `--differential` on four targets.
- `rust/bench-tools/tests/claim_grading.rs` states the unresolved outcomes by hand and compares every rule that no
  command runs yet with the retained Python, including which exception each raises.
- The Pages build job replays the validator on every restorable archive in the replay inventory: the 333
  `current-verdicts/v1` archives give byte-identical stdout and exit status from both implementations. The run
  prints its count per contract and fails when it is zero. v2 and v3 have no archive, as above, so the corpus covers
  them.

Pending, proposed and disputed-without-ruling inputs stay unresolved in both implementations: no claim is pinned in
those fixture variants, `pinned-pending` and `pinned-proposed` hold an `unresolved` pin, and the cargo tests check
that pending and proposed eligibility give `unresolved` recovery. `approved-without-receipt` has no validator case,
because `current_grading.py` refuses it before validator inputs exist.

## Slice 3: the current grading record and packet selection

[#233](https://github.com/kamui/code-review-bench/issues/233) ports `packet_selection.py` to
`bench_tools::packet_selection` and `current_grading.py` to `bench_tools::current_grading`: the inventory, pin
checks, document and grade validation, grading inputs and fingerprints, `load_current`, `control_state`,
`coverage_status` and `record_lock`. `bench-tools current-grading` runs them:

```sh
python3 bench/tools/rust_runtime.py exec bench-tools -- current-grading inventory [--out FILE]
python3 bench/tools/rust_runtime.py exec bench-tools -- current-grading check
python3 bench/tools/rust_runtime.py exec bench-tools -- current-grading status
python3 bench/tools/rust_runtime.py exec bench-tools -- current-grading fingerprints
```

Each reads the working directory as the repository, or `--root DIR`, and the record under `--current DIR`. Stdout,
stderr and exit codes are the Python's, and diagnostics keep the program name `current_grading.py`. `fingerprints`
has no Python command: it prints each selected batch with its input fingerprint and state as JSON, for `regrade.py`
in slice 10, without checking saved grades. The schemas the command validates against are copies of `bench/schema`
under `rust/bench-tools/src/current_grading/`, and a unit test fails when a copy differs. `verify:current` and the
commands in `docs/current-grading.md`, `docs/claim-adjudication.md` and `docs/evaluator-audit.md` moved.
`tools/test_moved_cli_callers.py` fails on a current caller of `inventory`, `check` or `status`, except
`test_grade.py` and `test_regrade.py`, which check the retained Python `grade.py` and `regrade.py` against their
baseline.

[`current-grading.v1.json`](../../tools/behavior_contracts/current-grading.v1.json) pins the commands on the nine
fixture variants and on these cases: a missing attempt record or review, an arm file changed after inventory, a
frozen run without its Git history, an agent-settled credit whose before-record binds another family, and the
command line. The decided differences:

- an exception the Python did not catch exits 2 with `current_grading.py: <type>: <message>` instead of a
  traceback with exit 1; usage errors and help are clap's;
- a lone surrogate in JSON and a UTF-16 replacement manifest are refused with exit 2;
- messages follow Python 3.14: `repr` of Unicode 15.1 and 16.0 characters, Unicode 16.0 digits and exponents in
  dash-led values, and the `TypeError` wording for `in` on a number and for an unhashable set element.

`tests/current_grading.rs` holds the record lock in each implementation while the other waits. No `current-grading`
command takes the lock, as no `current_grading.py` command does, so this is a cargo test, not a corpus step case.
Slice 8's `grade map` and `grade invalidate` take it; see below.

The Pages build job runs [`grading-real-record.v1.json`](../../tools/behavior_contracts/grading-real-record.v1.json),
`check`, `status`, `inventory --out` to a temporary file and `fingerprints`, against the checked-out record after
the evidence and freeze-commit fetches. On 2026-10-19 both implementations gave identical output on the record: 16
tasks, 198 batches, 623 required reviews and none assessed, every batch `missing` with the same input fingerprint in
both, and the same dataset hash.

`current-grading check` is stricter than the Python for a comment linked to several claims. After the owner
[ruled on #230](https://github.com/kamui/code-review-bench/issues/230#issuecomment-6274330595),
[#256](https://github.com/kamui/code-review-bench/issues/256) added the rule that a comment linked to two or more
claims has no equivalent link. `current_grading.py` keeps the older rule and stays byte-unchanged as the baseline
until slice 11 deletes it; `packet_selection.py` stays for #157's callers.

## Slice 4: the explorer export's facts, methodology and schema-check callers

[#234](https://github.com/kamui/code-review-bench/issues/234) moves three callers off the Python:

- `bench-tools export-explorer` with no command runs the whole export. It loads the current facts in process
  through `bench_tools::current_grading::load_current` and `coverage_status`, so the temporary facts file and
  `tools/export_explorer.py` are gone. `bun run data` runs it through `rust_runtime.py exec`, which prints the
  install command and exits 1 when no `bench-tools` is selected for `HEAD`, as the processing record explains. [The explorer
  export](processing.md#the-explorer-export) describes the command and its decisions.
- `bench_tools::methodology` ports `methodology.plan`. `bench-tools methodology --out FILE [--root DIR]` writes the
  queue, and `plan:methodology` and `docs/current-grading.md` run it. `methodology.py` stays byte-unchanged as the
  baseline until slice 11 deletes it. `regrade:methodology` still runs `regrade.py`.
- `check_manifest.py`'s command line has no caller left: `bench/README.md`, the rates workflow and
  `docs/current-grading.md` run `bench-tools check-manifest`, and `test:bench` runs
  [`check-manifest.v1.json`](../../tools/behavior_contracts/check-manifest.v1.json) against Rust in place of
  `--self-test`. The Rust runtime workflow runs the same corpus with a differential against the retained Python.

`tools/test_moved_cli_callers.py` fails on a current caller of `methodology.py`, `check_manifest.py` or
`tools/export_explorer.py`, except `test_grade.py` and `test_regrade.py`, which check the retained Python against
its baseline.

[`methodology.v1.json`](../../tools/behavior_contracts/methodology.v1.json) pins the command on the six fixture
variants that plan and the two the record refuses, and on `--out` and the command line. The decided differences
follow slice 3's:

- an exception the Python did not catch prints one line instead of a traceback: an inconsistent record keeps exit 1
  with `methodology.py: current_grading.Inconsistent: <message>`; an input or output it cannot read or write, such
  as an `--out` that exists, and any other exception exit 2;
- usage errors and help are clap's;
- Python 3.14's readings: `repr` of Unicode 15.1 and 16.0 characters, and a dash-led `--root` in exponent form or
  led by a Unicode 16.0 digit, which Python 3.12's argparse refused.

[`export-explorer.v1.json`](../../tools/behavior_contracts/export-explorer.v1.json) gains the whole export's cases,
which agree with the deleted front at the base commit except six decided in `export-explorer-decisions.v7.json`: a
record the export refuses prints the traceback's last line instead of the traceback, keeping exit 1
(`*-billing-receipt`, `*-missing-inventory`, `*-unicode-15-1-record`, which also quotes through Python 3.14's
`repr`); `--r` is ambiguous with the hidden `--restore` (`*-abbreviated-root`); `--facts`, `--base-path`,
`--stage` or `--restore` without a command is refused (`*-facts-without-command`); and `--public` led by `-` and a
Unicode 16.0 digit is taken as Python 3.14 took it (`*-unicode-16-digit-public`). Dash-led `--root` and `--public`
values such as `-.5` and `'-published dir'` agree with the front (`*-dash-led-*`, `*-negative-number-public`).
`tests/export_explorer.rs` covers what a corpus cannot reach: a record changed after staging, a failed publication,
`SIGINT` during publication and `BASE_PATH`.

On 2026-10-19, read-only against the checked-out record, `methodology --out` wrote the same queue as
`methodology.py` under Python 3.12.15 and 3.14.7: 663 selected reviews, 198 batches and 126 shared-claim links. The
whole export wrote the same `public/` tree as the base commit's front, with `BASE_PATH` unset and set to
`/code-review-bench/`.

## Slice 5: shared claims and upstream collection

[#235](https://github.com/kamui/code-review-bench/issues/235) ports `claims.py` to `bench_tools::claims`, extending
the registry checks #197 ported: the inventory, blinded dossiers, the grading context, identity and exposure checks,
evidence extracts and packets, allowed assignments and `mapping_problems`. It extends `bench_tools::upstream` with
`validate_assessment`, `route` and `collect`. The commands:

```sh
python3 bench/tools/rust_runtime.py exec bench-tools -- claims [--registry FILE] check
python3 bench/tools/rust_runtime.py exec bench-tools -- claims inventory --target TARGET [--pattern RE] [--unlinked]
python3 bench/tools/rust_runtime.py exec bench-tools -- claims dossier --out FILE --key FILE
python3 bench/tools/rust_runtime.py exec bench-tools -- claims evidence --out DIR [--extracts FILE] [--target TARGET]
python3 bench/tools/rust_runtime.py exec bench-tools -- upstream collect (--target TARGET | --all) [--related-pr N]...
```

Diagnostics keep the program names `claims.py` and `upstream.py`. Dossier tokens and the link shuffle come from
`/dev/urandom`; cargo tests inject the entropy and check that the shuffle is applied. `collect` runs `gh` with
`upstream.py`'s argument lists, four reads at a time. `verify:claims` and the commands in
`docs/claim-adjudication.md` and `docs/maintainer-adjudication.md` moved, and `tools/test_moved_cli_callers.py`
fails on a current caller of `claims.py`'s `check`, `inventory`, `dossier` or `evidence` command, or of
`upstream.py`'s `collect` command. `grade.py prepare --claim-evidence` still imports `claims.py`, until slice 10 moves its caller.

[`claims.v1.json`](../../tools/behavior_contracts/claims.v1.json) pins the four commands on the fixture variants and
on hand-built records, with dossiers and packets compared structurally. Evidence cases state their claim outcomes
by hand: the approved packet's sources and withheld records, and no resolved claim for a proposed decision or in the
`pending`, `proposed` and `disputed-without-ruling` variants. An approved decision without a saved receipt or
receipt passage is refused. Three mutants of `claims.py` in `test_behavior_contract.py`, a
shifted quote, a withheld record that keeps its hash and a zeroed source pin, each fail `claims-evidence-approved`.
[`upstream.v1.json`](../../tools/behavior_contracts/upstream.v1.json) adds 15 `upstream-collect-*` cases against the
stub `gh`, with failed, undecodable and paged responses injected. `test_behavior_contract.py` checks that every
recorded call is a paginated GET of the case's repository that the stub answers. The decided differences:

- An exception `claims.py` did not catch, such as a malformed target record's `TypeError` or an invalid
  `--pattern`, prints `claims.py: <message>`, exit 1, instead of a traceback (`claims-check-uncaught-type-error`,
  `claims-inventory-invalid-pattern`).
- `--pattern` compiles through `pyregex`, which refuses lookarounds, back-references and other syntax it does not
  support, exit 1, where Python's `re` ran them (`claims-inventory-unsupported-pattern`).
- Values follow Python 3.14 where versions differ: Unicode 16.0 digits in list indexes, attempt numbers and
  `--related-pr`, word characters for `\b` and case folding for identities (`claims-evidence-unicode-16-*`,
  `upstream-collect-unicode-16-related-pr`), and dash-led values (`claims-dash-led-*`).
- `collect` saves a response holding a lone surrogate as an incomplete endpoint, exit 1, where Python saved it
  (`upstream-collect-lone-surrogate`).
- Usage errors are clap's (`claims-inventory-missing-target`, `upstream-collect-choice-required`,
  `upstream-collect-target-and-all`, `upstream-collect-invalid-related-pr`).
- `inventory` and `evidence` validate the current record through the Rust validator, so they refuse a comment
  linked to two or more claims with an `equivalent` link, #256's rule, where `claims.py` accepted it
  (`claims-inventory-comment-equivalent-and-related`, `claims-evidence-comment-equivalent-and-related`).

#235 says a failed or partial response leaves no snapshot. `upstream.py` saves a failed endpoint as incomplete,
with its error, and exits 1; only a response that fails snapshot validation, such as another pull request's identity
or a paged pull-request read, leaves none. The port keeps Python's behavior, which
[the maintainer workflow](../maintainer-adjudication.md#capture-upstream-evidence) documents and
[the #235 clarification](https://github.com/kamui/code-review-bench/issues/235#issuecomment-6279079746) confirms.

The Pages build job runs `claims check` on `registry.json` and the four other tracked registries in real-record mode.
On 2026-10-19, `claims evidence` for each tracked extracts manifest, `evidence-extracts.v1.json` and
`evidence-extracts.selected-pr.v1.json`, wrote the same 126 packets, byte for byte, with the same stdout, from both
implementations. `claims.py` and `upstream.py` stay byte-unchanged as baselines until slice 11, with the `upstream`
and `claims` corpus drivers.

## Slice 6: calibration and evaluator audit checks

[#236](https://github.com/kamui/code-review-bench/issues/236) ports `calibration.py` to `bench_tools::calibration`
and every step of `evaluator_audit.py` but `second` to `bench_tools::evaluator_audit`. Both validate through the
`current_grading` port, whose schemas gain copies of `current-impact-card` and `current-audit`. The commands:

```sh
python3 bench/tools/rust_runtime.py exec bench-tools -- calibration check|queue [--out FILE]
python3 bench/tools/rust_runtime.py exec bench-tools -- calibration cards --out DIR --key FILE
python3 bench/tools/rust_runtime.py exec bench-tools -- evaluator-audit check|draw|compare
python3 bench/tools/rust_runtime.py exec bench-tools -- evaluator-audit claims --run RUN --target TARGET --out FILE
python3 bench/tools/rust_runtime.py exec bench-tools -- evaluator-audit conclude --reconciliation FILE
```

Diagnostics keep the program names `calibration.py` and `evaluator_audit.py`. `second` runs a grading client, so it
stayed in Python until slice 8 moved it. `verify:current` and the commands in
`docs/current-grading.md`, `docs/impact-calibration.md`, `docs/claim-adjudication.md`,
`docs/pre-existing-problems.md` and `docs/evaluator-audit.md` moved, and `tools/test_moved_cli_callers.py` fails on
a current caller of `calibration.py`'s `check`, `cards` or `queue`, or of `evaluator_audit.py`'s `check`, `draw`,
`claims`, `compare` or `conclude`.

[`calibration.v1.json`](../../tools/behavior_contracts/calibration.v1.json) and
[`evaluator-audit.v1.json`](../../tools/behavior_contracts/evaluator-audit.v1.json) replay the scenarios of
`test_calibration.py` and `test_evaluator_audit.py`, the command-line forms, and fixture cases on the `pending`,
`proposed`, `approved-without-receipt` and `disputed-without-ruling` variants. They include a whole receipt refused as
a scope (`calibration-whole-receipt-is-not-a-scope`), a family with no card or decision reported
(`calibration-every-family-needs-one-impact-decision`), a pending candidate keeping an audited-clean control
provisional (`calibration-provisional-control`), `draw` refusing a second sample for its seed
(`evaluator-audit-draw-twice`), and `claims`, `compare` and `conclude` refusing once `grades.json` changed after the
draw (`evaluator-audit-a-batch-graded-again-after-the-draw-is-refused`). Round files are compared with their clock
fields masked. The decided differences:

- Values follow Python 3.14 where versions differ: Unicode 16.0 word characters in the superseded-seed search and in
  the blinded cards' private-path check (`calibration-unicode-seed`, `calibration-unicode-exposure`), digits in a
  second assessment's number (`evaluator-audit-unicode-assessment-number`), and dash-led values (`*-cli-dash-led-*`).
- An exception the Python did not catch, a pinned receipt that is not UTF-8 or a second-assessment receipt that is
  not an object, prints `<program>: <class>: <message>`, exit 2, instead of a traceback, exit 1
  (`calibration-receipt-not-utf8`, `evaluator-audit-receipt-not-an-object`).
- `evaluator-audit second` is refused as an invalid operation, exit 2 (`evaluator-audit-cli-second-*`).
- Usage errors and help are clap's (`calibration-cli-*`, `evaluator-audit-cli-*`).

The Pages build job runs `calibration check`, `calibration queue` and `evaluator-audit check` in real-record mode.
The saved round `evaluator-audit-v1-2026-10-03` cannot be drawn again by either implementation. Its sample pins the
`grades.json` of [#58](https://github.com/kamui/code-review-bench/pull/58)'s commit `0a35e127e`, whose 199 batches
were graded under `current-verdicts/v1` and which predates `credits.json`. On 2026-10-19, on a disposable worktree of
that commit with the round removed and an empty `credits.json` added, `evaluator_audit.py` and `bench-tools` both
refused `check` and `draw` with the same output, exit 1: 199 batches were not graded under `current-verdicts/v2`.
`calibration.py` and `evaluator_audit.py` stay byte-unchanged as baselines until slice 11.

## Slice 7: grading preparation and runner identity v2

[#237](https://github.com/kamui/code-review-bench/issues/237) ports the preparation half of `grade.py` to
`bench_tools::grade`: attempt and input checks, review selection with blind tokens from `/dev/urandom` and their
collision retry, rendering through `bench_tools::normalize_review`, source views without the private attempt
directory, references, rubric, prompt, claim context and evidence packets, the command policy, cache replacements,
the remedy policy and `--inventory`. The commands:

```sh
python3 bench/tools/rust_runtime.py exec bench-tools -- grade prepare --run RUN --target TARGET --work WORK --key KEY [...]
python3 bench/tools/rust_runtime.py exec bench-tools -- grade preflight-inputs --run RUN --work-root DIR --key-root DIR [--target TARGET]...
python3 bench/tools/rust_runtime.py exec bench-tools -- grade check-prepared --work WORK --key KEY [--dispatching]
```

`prepare` takes `grade.py prepare`'s options. `--provision SCRIPT` runs `python3 SCRIPT prepare` into `WORK/clone`,
the bridge #157 removes; a failed provisioner leaves no `clone`, `clone-cache` or `clone-work` and reports its exit and
output tail. The key is created with mode 0600 outside WORK, and WORK must be new or empty. `preflight-inputs` makes
the checks of `grade.py preflight --offline`. `check-prepared` makes `check_prepared`'s checks of a version 1 key.

A Rust-prepared key is version 2, as plan decision 6 gives it. `validator` holds only `inputs.json`, and
`validator/tools/` is not created, since the validator is part of the executable. `runner_deviation` is
`{"version": 2, "files", "bench_tools", "execution_policy_sha256", "codex_catalog_sha256"}` with an optional
`provisioning`: `files` holds the 23 runner files and `rust/bench-tools`, the running executable's SHA-256, and
`bench_tools` is that executable's `rust_runtime.py` pin. `prepare` and `check-prepared` refuse a receipt that
`rust_runtime.check` finds incomplete. `check-prepared` refuses a v2 key whose pinned executable or receipt is
missing, changed or incomplete, one problem naming it, and a changed or deleted `files` entry as a runner change. The
retained `grade.py map` accepts a v2 key; `grade.py dispatch` refuses it with the runner-changed line after its
client preflight, before any home, session or charge.
[`current-grading-key.v1.schema.json`](../../bench/schema/current-grading-key.v1.schema.json) describes both
versions. It cannot require `bench_tools` for version 2 only; `check-prepared` refuses a version 2 key without a valid pin.

[`grade-prepare.v1.json`](../../tools/behavior_contracts/grade-prepare.v1.json) pins the three commands on the
fixture variants, with the reviews, items and families each case expects stated by hand, the compatibility rows,
the refusals and the identity leaks. [`grade-prepare-decisions.v3.json`](../../tools/behavior_contracts/grade-prepare-decisions.v3.json),
which carries [v2](../../tools/behavior_contracts/grade-prepare-decisions.v2.json) and
[v1](../../tools/behavior_contracts/grade-prepare-decisions.v1.json) forward, records the decided differences:

- `grade-prepare-runner-identity-v2`, on the `pending`, `proposed` and `disputed-without-ruling` fixture cases: only
  `runner_deviation`, the missing `validator/tools/` and the hashes that pin them differ.
- Source views read `\d` from the Unicode 16.0 tables of Python 3.14 (`grade-prepare-unicode-16-digit-source-link`).
  Python 3.12 (Unicode 15.0) misses the 80 digits Unicode 16.0 added, and Python 3.10 (Unicode 13.0) misses 110; the
  case records 3.12's result, as CI runs it. Attempt ids are `att-` and ASCII digits, so no saved review changes.
- Usage errors are clap's, exit 2 (`grade-prepare-arguments`).
- Help is clap's. An earlier `--help`, or a prefix such as `--hel`, wins over a later `-h=1` and exits 0, as in
  argparse (`grade-prepare-*-before-invalid-short-help`, [#268](https://github.com/kamui/code-review-bench/issues/268)).
- A failing provisioner whose output is not UTF-8 crashes `grade.py` with a traceback before its clean-up, leaving
  `clone`, `clone-cache` and `clone-work`. `bench-tools` removes all three and reports the exit and the output tail,
  each undecodable byte written as `\udcXX` (`grade-prepare-provisioner-invalid-output`).

No caller moves: `grade.py prepare` stays the production command until slice 10, and slice 11 deletes the Python
preparation.

## Slice 8: grade mapping, invalidation and second assessments

[#238](https://github.com/kamui/code-review-bench/issues/238) ports the filing half of `grade.py` to
`bench_tools::grade::filing`, and the `second` step of `evaluator_audit.py` to `bench_tools::evaluator_audit`.
The commands:

```sh
python3 bench/tools/rust_runtime.py exec bench-tools -- grade validate --work WORK
python3 bench/tools/rust_runtime.py exec bench-tools -- grade inventory --work WORK --key KEY --out OUT
python3 bench/tools/rust_runtime.py exec bench-tools -- grade map --work WORK --key KEY [--assessor FILE] [--safety-checks FILE]... [--root ROOT] [--current DIR]
python3 bench/tools/rust_runtime.py exec bench-tools -- grade invalidate [--root ROOT] [--current DIR]
python3 bench/tools/rust_runtime.py exec bench-tools -- evaluator-audit second --work WORK --key KEY [--assessor FILE]
```

`map` and `invalidate` hold the record lock, `.cache/current-record.lock` under the root, as `grade.py` does.
`tests/current_grading.rs` starts both commands of each implementation while the other holds the lock and checks
that they wait. Each refusal writes nothing, a modified clone stops the mapping, a raised candidate stays pending, an
unresolved claim stays unresolved, and fix sufficiency is never copied from another review. The cleanup after a map runs the checkout's `python3 bench/tools/prune_workspace.py --grading-work WORK
--target TARGET --receipt RECEIPT --apply`, the bridge [#159](https://github.com/kamui/code-review-bench/issues/159)
removes; an exit or output outside its contract is a cleanup failure that keeps no grades.

[`grade-map.v1.json`](../../tools/behavior_contracts/grade-map.v1.json) pins the five commands on the fixture
variants, a Python v1 key mapped in Rust and a Rust v2 key mapped in Python, the refusals, the remedy policy paths of
`test_remedy_policy.py` for a first and a second grader, and a failed write at each step, including between the
renames, with the remap after it. [`grade-map-decisions.v2.json`](../../tools/behavior_contracts/grade-map-decisions.v2.json)
records the decided differences:

- `grade-map-runner-identity-v2`: `runner.mapping` in each receipt the mapping writes is the version 2 runner
  identity `grade prepare` writes, and each pin of the receipt's SHA-256 changes with it. `runner.prepared` is still
  copied from the key.
- A failed read or write the Python did not catch prints `grade.py: <message>`, exit 2, instead of a traceback, exit
  1 (`grade-map-target-without-head`, `grade-map-write-faults`).
- The cleanup bridge runs the checkout's `prune_workspace.py` as a command, so a broken script fails the mapping
  (`grade-map-cleanup-bridge-*`).
- Assessment numbers read Unicode 16.0 digits, as Python 3.14 does (`grade-map-unicode-16-digit-assessment-number`).
- Usage errors and help are clap's (`grade-map-arguments-refused`).

[`evaluator-audit-decisions.v3.json`](../../tools/behavior_contracts/evaluator-audit-decisions.v3.json) carries v2
forward and ends its `evaluator-audit-cli-second-is-not-ported` difference: `bench-tools evaluator-audit second` now
saves the second assessment, and its help lists `--work`, `--key` and `--assessor`.

`docs/evaluator-audit.md` names `bench-tools evaluator-audit second`, and `tools/test_moved_cli_callers.py` fails on a
current caller of `evaluator_audit.py`'s `second`. No `grade.py` caller moves: `grade.py` stays the production
command until slice 10, and slice 11 deletes `grade.py` and `evaluator_audit.py`.

## Research helpers

The audit covered the `DYNAMIC_ROOTS` loaders, `test_last_push_move.py`, `test_regrade_filing.py`,
`test_review_code_selection.py`, `test_packet_replacements.StagedSelectionTests`, `test_v10_filing.py` and the
maintained docs. The helpers stay byte-unchanged, and no maintained `bench/tools` or `tools` module imports or runs
any of them.

### Loaders

| Loader | Assertions about the current record | Disposition |
| --- | --- | --- |
| `test_packet_replacements.StagedSelectionTests` | None. All eight tests check the frozen staged-edition record and the script's refusals | Left the suite. Replay `stage_edition.py --root <staging> --check` at `8b390d6a7`, staging from a worktree at `751a969a0` |
| `test_last_push_move.py` | The move rebuild compares the live record, with five later filings undone, with `move.py`'s output | Stays |
| `test_regrade_filing.py` | The regrade rebuild over the live record; no equivalent link on a comment of a filed claim | Stays |
| `test_review_code_selection.py` | The staged inventory pin, the selection rebuild, the committed `selection.v1.json`, packets and collector labels over the live record | Stays |

No current-record assertion moved in this slice. None fits a fixture case: each compares the live record with a
helper's output, and the helpers have no port. `current-grading check` covers only their consistency part. The
staged inventory pin's live derivation needs a later step: it is the inventory `current-grading check` compares.
Since slice 3, the real-record list runs that check in Pages.

`current-grading check` also refuses an equivalent link on a comment linked to two or more claims, a rule #256
added. `current_grading.py` refuses only a comment equivalent to two claims.

Replaying the rebuild assertions at their filing commits, so the loaders could leave, would drop today's guard that
each later filing stays undoable. That needs the owner's ruling; until then the loaders stay and their helpers stay
active.

### Helpers

Replay commands run in a worktree at the replay commit, with its evidence restored and freeze commits fetched,
because each helper imports `bench/tools` from its own checkout.

| Helper | Disposition | Reason | Replay commit | Replay command |
| --- | --- | --- | --- | --- |
| `cohort-rebuild-2026-10-05/regrade/plan.py` | Active | `docs/grading-readiness.md` names it for sizing the next regrade | `1727db272` | `python3 docs/research/cohort-rebuild-2026-10-05/regrade/plan.py` |
| `last-push-recut-2026-10-07/stage_edition.py` | Active | `OwnerDecisionTests` imports it, and `move.py` and `selection.py` run it | `8b390d6a7` | `… stage_edition.py --root <staging> --check` |
| `last-push-review-code-2026-10-12/selection/collect.py` | Active | `test_review_code_selection.IntakeTests` runs it | `aa6ff1eae` | `… selection/collect.py PACKETS KEY`; its matcher outputs were never archived |
| `last-push-review-code-2026-10-12/selection/packets.py` | Active | `test_review_code_selection` runs it on the live record | `aa6ff1eae` | `… selection/packets.py OUT KEY` |
| `last-push-review-code-2026-10-12/selection/selection.py` | Active | `test_review_code_selection` runs it on the live record | `aa6ff1eae` | `… selection/selection.py`, then `--record`; expect no diff |
| `last-push-rulings-2026-10-11/filing/collect.py` | Historical replay | No loader runs it; only the census directory rule names it | `0900d5dc9` | `… filing/collect.py PACKETS KEY SESSION`, inputs in `last-push-filing-links-2026-10-11-v1.json` |
| `last-push-rulings-2026-10-11/filing/move.py` | Active | `test_last_push_move` runs it on the live record | `8b390d6a7` | `… filing/move.py`; expect a clean `git status` |
| `last-push-rulings-2026-10-11/filing/packets.py` | Historical replay | No loader runs it; only the census directory rule names it | `0900d5dc9` | `… filing/packets.py OUT KEY` at `0900d5dc9^` |
| `last-push-rulings-2026-10-11/filing/record.py` | Active | The regrade and v10 filing scripts import it, and `test_v10_filing` runs them | `8b390d6a7` | `… filing/record.py` from the records at `0900d5dc9` |
| `older-problem-activation-2026-10-13/filing/admit.py` | Active | The activation undo in the loaders' setup runs it | `1727db272` | `… filing/admit.py`; expect no diff |
| `older-problem-activation-2026-10-13/filing/receipts.py` | Active | `admit.py` and the activation `record.py` run it in that undo | `1727db272` | `… filing/receipts.py`; expect no diff |
| `older-problem-activation-2026-10-13/filing/record.py` | Active | `test_last_push_move` undoes the activation through it | `1727db272` | `… filing/record.py --undo`, then `admit.py` |
| `older-problem-activation-2026-10-13/filing/wordings.py` | Historical replay | No loader runs it; only `admit.py`'s docstring names it | `1727db272` | `… filing/wordings.py`; expect no diff |
| `reference-calibration-2026-10-03/blind_items.py` | Active | `docs/impact-calibration.md` makes it step 1 of every empty-reference control audit | `d393beb53` | `… blind_items.py --target T --out ITEMS --key KEY` |
| `regrade-rulings-2026-10-13/filing/record.py` | Active | `test_regrade_filing` checks its rebuild and link rule on the live record | `9be17965f` | `… filing/record.py`; expect no change |

The three historical helpers keep their inventory v3 class, `active-outside-tooling`, through the census
directory and docstring rules until the next inventory version narrows those rules.

## Slice 9: queue status, attempt archives and the audit log

[#239](https://github.com/kamui/code-review-bench/issues/239) ports three read-mostly reports. `grading_profile.py`
is not one of them; it moved to #157.

| Python | Rust | Caller | Python implementation |
| --- | --- | --- | --- |
| The `status` heredoc of `bench/tools/regrade_queues.sh` | `bench-tools regrade status QUEUE_DIRECTORY...`, `bench_tools::grading_reports::status` | `regrade_queues.sh status`, through `rust_runtime.py exec` | Removed from the script |
| `regrade.archive_attempt` | `bench-tools regrade archive-attempt ATTEMPT ARCHIVE_DIR`, through `bench_tools::archive` | None until slice 10 | Retained; `regrade.py` still calls it |
| `bench/tools/audit_log.py` | `bench-tools audit-log`, with its options and outputs | None outside the docs | Deleted with its `--self-test`, whose fixture is `audit-log-self-test` |

[`regrade-reports.v1.json`](../../tools/behavior_contracts/regrade-reports.v1.json) and
[`audit-log.v1.json`](../../tools/behavior_contracts/audit-log.v1.json) pin the commands. Each case's expected
output is the port's; every case where Python 3.12.15 or 3.14.7 printed something else carries Python's output as a
`baseline_defect` and a decision record. The decided differences:

- An exception the Python did not catch prints `<program>: <type>: <message>` and keeps exit 1, instead of a
  traceback (`*-missing-batches`, `*-invalid-json`, `*-unhashable-state`, `*-uncaught-error`,
  `*-uncaught-type-error`, `*-zero-reviews`).
- Values follow Python 3.14 where versions differ: `sum` (`*-mixed-wall-seconds`, `*-mixed-sum`), hour 24
  (`*-hour-24`), Unicode 16.0 digits in `float()` (`*-unicode-16-digit-charge`), `repr` of Unicode 15.1 and 16.0
  characters (`*-unicode-15-1-*`), the absolute path in a missing run's message (`*-missing-run`), and argparse 3.14's
  `-1e5` (`*-exponent-out`).
- Refusals: a lone surrogate in JSON, a name that is not UTF-8, an archive tarfile could read but
  `bench_tools::archive` refuses (`*-plain-tar-refused`), including a directory and a regular member of one name in
  either order (`audit-log-*-then-*-refused`), a UTF-16 dispatch record, and in `archive-attempt` a symbolic link,
  directory or special mode bits before anything is written, so no partial archive is left.
- `audit-log` parses with clap, as every subcommand does (`audit-log-usage-*`, `*-self-test-removed`,
  `*-run-after-option`, `*-dot-digit-out`, `*-help*`).

Three decided cases record Python 3.14's wording as the old output, so the deleted Python reproduces them only under
3.14: `regrade-status-unhashable-state` and `regrade-status-unicode-15-1-queue`, whose 3.12 messages differ, and
`audit-log-zero-reviews`, where 3.12 says `float division by zero`. CI runs no Python for those programs.
`summary.md` still says it was derived by `bench/tools/audit_log.py`, byte for byte as before.

Read-only runs on 2026-10-19 compared the port with the removed Python, under Python 3.14.7 and 3.12.15:

- `regrade status` over the 14 saved queues under `.local/regrade` of the regrade worktree: identical output.
- `audit-log` over the 69 runs under `bench/runs`, with that worktree's 37 local grading archive roots (1,148
  archives): identical files and stdout, 1,405 attempts in 19 setups and 1,145 grading sessions.
- `regrade archive-attempt` on the 399 saved attempts that still have a workspace, into scratch directories:
  identical exit, stdout, receipt and archive members, sizes, modes and SHA-256, 13,391 members in all.

The audit-log decisions v1 to v3 cite `bench_tools::pyvalue::fromisoformat`, `python_sum` and `parse_float`, and the
regrade-reports decisions v1 cite `bench_tools::pyvalue::python_sum`, by that module's name before #249 renamed it
`bench_tools::pysemantics`. `bench_tools::pyvalue` is now #251's ruling module. The pinned decision files stay
unchanged.

## Retained after #156

| Module | Kept for | Removed by |
| --- | --- | --- |
| `packet_selection.py` | `run_cell.py`, `codex_skill_runner.py` | #157 |
| `grade.py` (dispatch, online preflight) | `regrade.py`, manual dispatch | #157 |
| `grading_profile.py` | Waits for the shared Rust usage parser | #157 |
| `check_manifest.py` (library only) | `rates`, `provision`, `run_cell`, `file_attempt`, `evidence_store` | #160 |
| `normalize_review.py`, `diff_identity.py` | As the processing record lists | #158, #157 |
| `tools/behavior_contract.py` features added here | The contract harness | #160 |
| `stage_edition.py` and the other research helpers | Research records | never deleted |

Until slice 11, every Python module in the recurring real-record differential, or imported by the Python grading
runner, stays as the baseline. `grading_fixture.rs` runs `current_grading.py` as that baseline and moves to the
Rust check by then.

## Bridges

| Bridge | Removal condition | Removed by |
| --- | --- | --- |
| `grade prepare` runs `provision.py prepare` | Provisioning moves to Rust | #157 |
| `grade map` and `evaluator-audit second` run `prune_workspace.py --grading-work` before publishing | Pruning moves to Rust | #159 |
| `grade.py` dispatch and preflight run `grade check-prepared` and `preflight-inputs` | Dispatch moves to Rust | #157 |
| `grading_policy.py`'s `validate` runs the pinned `grading validate` | The tool server moves to Rust | #157 |
| `regrade.py` runs `bench-tools` under its `runtimes` pin | The controller moves to Rust | #157 |

Slice 7 adds the first, `grade prepare`'s provisioner; slices 8 to 10 add the rest.
