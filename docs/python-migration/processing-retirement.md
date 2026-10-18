# Processing retirement

This is the first slice of the processing stage, [#192](https://github.com/kamui/code-review-bench/issues/192),
part of [#154](https://github.com/kamui/code-review-bench/issues/154). It follows the
[delivery plan](https://github.com/kamui/code-review-bench/issues/154#issuecomment-6253814508).
It removes modules with no current caller and records classifications. It ports nothing.

The [v2 inventory](README.md) stays as recorded. `python_inventory.py --check` rebuilds v2 with
today's assignment tables, so those tables stay unchanged until the next census version records
the changes below.

## Retired modules

`bench/tools/score.py` and `bench/tools/scoreboard_svg.py` are deleted from main. Commit `bd1fde436`
is the last main commit that holds them. Neither is hashed by `grade.runner_files()`, and no package
script, workflow, container builder or current schema runs them.

| Module | Current callers removed or updated | Callers that keep working, and why |
| --- | --- | --- |
| `score.py` | `test_claim_grading.HistoricalProjection.test_mixed_detection_and_refutation_both_survive_scoring`, deleted. Prose in `bench/README.md` and `docs/methodology-integration.md`, updated. | Frozen: `bench/runs/2026-09-29-codex-sol61-high-clean/deviations/grade.sol61-v{1,2}.py` import it and run at that run's `freeze_commit`. Imported: `artifacts/import-source/bench/tools/{grade,scoreboard,test_answers}.py` import the preserved `score.py`. Research: `codex-calibration-2026-10-02/runner/grade.v{1,2}.py` import it, and three publication helpers name it; they replay at their own commit. Authorization records under `methodology-integration-2026-09-30` and `codex-grading-readiness-2026-10-02` pin it at `89f8fc25…`. Main already held `d89db2d4…`, so `regrade.py` refuses those authorizations before and after this change. |
| `scoreboard_svg.py` | The `bench/README.md` tools table. | Imported: `artifacts/import-source/bench/tools/scoreboard.py` imports the preserved copy. Frozen: two run runtime records name its path and hash. `docs/design-interview.md` describes the source repository it inspected, not main. |

To replay either module, resolve it at the caller's commit, as [runtime resolution](README.md#runtime-resolution-and-unfinished-work)
describes, for example `git show bd1fde436:bench/tools/score.py`. Imported originals keep their
`preserved_path` in `bench/import-manifest.json`.

## Kept: `compare.py`

`bench/tools/compare.py` has a current caller, so it stays, byte-unchanged. Three tests in the
historical group of `tools/run_bench_tests.py` load #156 filing scripts:
`last-push-rulings-2026-10-11/filing/move.py` and `last-push-review-code-2026-10-12/selection/selection.py`.
Both import `last-push-recut-2026-10-07/stage_edition.py`. Its `staged_registry()` executes
`replacement_runs.py` and `skill_replacement_runs.py`, which `import compare`. With `compare.py`
removed, these fail with `ModuleNotFoundError: No module named 'compare'`:

- `test_last_push_move.OneGenerationTests` (class setup)
- `test_review_code_selection.OneGenerationTests` (class setup)
- `test_review_code_selection.RecordTests.test_the_committed_record_is_the_one_the_script_derives`

`compare.provisioning_hash` is also the only implementation of the run manifest's
`provisioning_sha256`, which the run-definition helpers `replacement_runs.py`,
`skill_replacement_runs.py` and `last-push-review-code-2026-10-12/review_code_run.py` call.

`compare` stays with #154, as the inventory assigns it. Remove it when no current test or command
runs those helpers from the current checkout, and another command computes `provisioning_sha256`
for new run freezes.

## Research helpers

| Helper | Classification | Evidence |
| --- | --- | --- |
| `builtin-review-benchmark-2026-09-24/answers.py` | Historical replay | Its only caller that ran it was `bench/tools/test_answers.py`, deleted here. `tools/import_benchmark.py` lists it as a path to copy on import, and `tools/test_python_inventory.py` writes a fixture with the same path; neither runs it. |
| `last-push-recut-2026-10-07/plan.py`, `recut.py`, `verify.py` | Historical replay | Their only caller that ran them was the audited loader in `bench/tools/test_packet_replacements.py`, deleted here. Research and grading docs name them as record commands. |
| `last-push-recut-2026-10-07/stage_edition.py` | Not historical replay yet | The #156 filing scripts above still import and run it in the historical test group. No #154 port is assigned; it follows those filing scripts under #156. |

The v2 census misses the `stage_edition` import from `move.py` and `selection.py`, because each
script inserts the other directory into `sys.path` first. A census generated after this change
therefore lists `stage_edition.py`, `replacement_runs.py` and `skill_replacement_runs.py` as
historical replay, although the historical tests run them. Its next version should record that
edge; `stage_edition`'s path references to the two run helpers then keep all three live.

Helper bytes are unchanged.

### Packet-pin drift without `verify.py`

`test_packet_replacements.py` ran `verify.py` against eight edits of a disposable copy. Each was
applied to a disposable worktree at `bd1fde436`, one at a time, after restoring the pinned evidence:

| Edit | `current_grading.py check` |
| --- | --- |
| `i-requests-6667/packet.v2.md` bytes changed | exit 1: `replacement packet differs from replacement manifest` |
| `i-requests-6667/packet.md` bytes changed | exit 1: `packet bytes changed` |
| Registry pins the merge-cut packet of `i-requests-6667` | exit 1: `packet differs: the run's reviews read d16600156792 and the task revision pins 061c9288a59f` |
| References pin the re-cut packet of `u-grpc-go-6919` | exit 1: `reference revision differs` |
| `i-requests-6667/target.json` pins the re-cut packet | exit 1: `packet bytes changed` |
| Three edits of `decisions.v1.json` | exit 0 |

`current_grading.py check` is part of `bun run verify:current`, so packet-pin drift stays caught.
`decisions.v1.json` is a research record that current tooling does not read; immutable-record
policy protects it, and `verify.py` still checks it when replayed.

## Ownership changes

- `methodology` moves to #156 and `collect_run` to #155. Both issues already list them.
- Retiring `score` and `scoreboard_svg` settles #156's item to classify legacy score and report
  responsibilities for those modules. `compare` is classified above.
