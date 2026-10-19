# Grading migration record

This records [#156](https://github.com/kamui/code-review-bench/issues/156), part of
[#149](https://github.com/kamui/code-review-bench/issues/149), under its
[delivery plan](https://github.com/kamui/code-review-bench/issues/156#issuecomment-6265525205).
Slice 0, [#230](https://github.com/kamui/code-review-bench/issues/230), starts it: the comparisons, the fixture
root and the replay inventory every later slice runs, and the research-helper audit. It ports no grading code and
changes no grade, claim decision, ruling or scorecard number. `src/lib/scoring.ts` stays the only owner of
scorecard arithmetic.

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
- `check_manifest.py`'s command-line callers move to `bench-tools check-manifest` in slice 4; the library stays for
  its importers until #160.

## Comparison features and fixture root

[The contract reference](contracts/README.md#grading-comparison-features) describes the harness features: the
structured comparison, hand-stated grading outcomes, the Python entropy driver, the `fixture-git` setup program and
real-record mode. `grading-real-record.v1.json` lists no command yet; slice 3 adds the first and runs it in Pages.

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

- `ruling_record.py`'s command line resolves pins against its own checkout. The fixture test calls its checks with
  the fixture root, so slice 2 settles how ruling commands read one.
- A command that runs `git status` in the clone rewrites its index with host timestamps, so slice 8's cleanup cases
  need an index the snapshot can compare.
- The structured comparison orders dossier sections but not JSON members. The dossier's provenance key lists its
  review tokens in shuffle order, so slice 5 either records that order by decision or declares a member ordering.

## Replay coverage

[`grading-replay.v1.json`](grading-replay.v1.json) accounts for the 376 receipts under
`bench/grading/current/assessments/`. `rust/bench-tools/tests/grading_replay.rs` writes it after the evidence fetch,
and the Pages build job fails when the committed file differs.

| Verdict contract | Receipts | Filed by | Validator replays | Mapping replays |
| --- | --- | --- | --- | --- |
| `current-verdicts/v1` | 333 | `5521884a8` (325), `8738936bc` (8) | 333 | 333 |
| `current-verdicts/v2` | 29 | `9be17965f` | 0 | 0 |
| `current-verdicts/v3` | 14 | `3cbcbd061` | 0 | 0 |

Each v1 receipt matches exactly one of the 751 archives under `bench/regrading/`. The archive restores by hash
through `repository-payloads-2026-10-08-v1.json`, its key, validator inputs and verdicts hash as `evidence.json`
lists, and the key's fields equal the receipt's. The 43 v2 and v3 receipts were filed with a receipt and no
verdicts. No archive or manifest in the repository holds their key, inputs or verdicts, so their contract comes
from the validation policy at the filing commit.

Gap for slice 8: v2 and v3 have no replay. The plan's remedy, a fixture case built from the archived key and
verdicts, has no archive to build from, so slice 8 builds those cases on the fixture root.

## Slice 1: claim grading rules and the blinded validator

[#231](https://github.com/kamui/code-review-bench/issues/231) ports `claim_grading.py` (all but `scoring_items`),
`pre_existing_grading.py` and `grading_validation.py` to `bench_tools::claim_grading`, `bench_tools::pre_existing`
and `bench_tools::grading_validation`, and adds `bench-tools grading validate [VERDICTS] --inputs FILE`. Its stdout
and exit status match the script's; any input it cannot read prints
`cannot validate: unreadable or malformed JSON input`, exit 1. No caller moves until slice 10, and the three Python
modules stay as the baseline until slice 11.

- [`grading-validation.v1.json`](../../tools/behavior_contracts/grading-validation.v1.json) covers every message,
  each verdict contract and safety policy, a duplicate key, non-UTF-8 and non-object input, and the fixture's
  `pending`, `proposed` and `disputed-without-ruling` variants. Its nine decided differences are in
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
helper's output, and the helpers have no port. `current_grading.py check` covers only their consistency part. Two
need a later step:

- The staged inventory pin's live derivation is the inventory `current_grading.py check` compares. It goes to the
  real-record list with slice 3's `current-grading check`.
- The rule that a comment of a filed claim has no equivalent link is stricter than `current_grading.py`, which
  refuses only a comment equivalent to two claims. Making it a validator case is a new rule for the owner.

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

No bridge exists yet; slices 7 to 10 add them.
