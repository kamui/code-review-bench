# Selected-cohort cache rebuild and readiness

Part of [issue 9](https://github.com/kamui/code-review-bench/issues/9). All five mirrors and dependency archives are rebuilt from the unchanged frozen target recipes. All five base/head smoke recipes and the full 45-review offline preflight pass. Paid calibration and rollout remain blocked by the Codex grading client and budget gates below.

The [original recovery report](../grading-evidence-selected-2026-10-02/README.md), frozen targets, saved reviews and earlier execution plans remain unchanged. These are replacement cache bytes. The [replacement manifest](cache-replacements.v1.json) pins each original archive hash, replacement hash, frozen target hash and build receipt. [Cache verification](cache-verification.v1.json) checks the source dependency identities, both revisions and all ten smoke checks. [Mirror receipts](mirrors/), [build receipts](build/) and [smoke receipts](smoke/) retain the measured results.

## Execution inputs

[Execution inputs v2](execution-inputs.v2.json) pin the current controller and grading runner files, schemas, command policy, rubric, template, claim registries, selected evidence, references, replacement manifest and receipts. The three v2 execution plans preserve each v1 plan's workspace root, archive root and target order, and add `.local/selected-cache-rebuild-2026-10-02/cache` as `cacheRoot`. Control calibration omits selected extracts; enriched calibration and rollout use the pinned selected extracts. The record is readiness evidence, with no execution authorization or chosen grader model.

The runner accepts `--cache-replacements` in preparation, preflight and provisioning smoke checks. It substitutes the archive identity in memory and keeps target revisions, recipes and command profiles frozen. Preparation embeds the exact manifest and build-receipt bytes, their hashes and the schema hash in the private key's provisioning deviation. Those records remain outside grader inputs. Controller authorization can pin `cacheReplacements`; the execution plan passes `cacheRoot` to both preflight and preparation. Paid dispatch checks remain in place.

Keep the rebuilt archives and mirrors at the pinned cache root. Archives total about 642 MiB and mirrors about 1.7 GiB. [The dependency inventory](dependency-inventory.v1.json) records resolved versions and verified frozen dependency identities. The original Python and Node resolved inventories are unavailable. Rebuilding from version ranges can resolve newer dependencies, so these receipts prove focused execution readiness. They do not establish byte identity with deleted archives or calibration equivalence.

## Verification

The [offline preflight log](logs/full-cohort-preflight.log) verifies nine saved reviews for each of five targets. The [smoke driver](checks/offline_smoke.py) runs the frozen base/head recipes in a bubblewrap namespace with external networking disabled. It refuses existing logs to preserve evidence. Smoke coverage includes PostgreSQL backend imports, not a running PostgreSQL service.

The [complete preparation and sandbox receipt](preparation-v1/prepared-sandbox.v1.json) passes for all five targets, with nine reviews each. The [saved preparation driver](checks/prepare_check.py) verifies replacement snapshots, policy confinement and focused head-revision commands. Tracked trees remain clean; no grading client, context home or model call was created. The first check was interrupted after four target logs. The [interruption receipt](preparation-interruption.v1.json) preserves that partial attempt alongside the complete rerun.

Saved test runs pass [cache replacement tests](logs/cache-replacements.final.log), 6 tests; [grading tests](logs/grade.final.log), 46 tests with one existing fixture-specific skip; [claim tests](logs/claims-tests.log), 31 tests; [controller tests](logs/regrade.final.log), 23 tests; [sandbox policy tests](logs/grading-policy-tests.log), 13 tests; and [provisioning self-test](logs/provision.final.log). The dependent claim-grading, profiling and workspace-pruning suites also pass. [Verification context](verification-context.v1.json) pins the tested runner inputs and command/log results. These local checks make no paid model calls. The original cohort verifier output is byte-identical to [the prior recovery result](../grading-evidence-selected-2026-10-02/cohort-verification.v1.json).

Recheck archive/source identities and the full offline cohort from the repository root:

```sh
python3 docs/research/selected-cache-rebuild-2026-10-02/verify.py
python3 bench/tools/grade.py preflight \
  --run bench/runs/2026-09-30-selected-prs-review-only \
  --work-root /tmp/selected-readiness-work --key-root /tmp/selected-readiness-keys \
  --rubric-version 2 --offline \
  --claim-registry bench/claims/registry.selected-pr-intake-v3.json \
  --claim-evidence bench/claims/evidence-extracts.selected-pr.v1.json \
  --reference u-grpc-go-6919=2 \
  --cache-root .local/selected-cache-rebuild-2026-10-02/cache \
  --cache-replacements docs/research/selected-cache-rebuild-2026-10-02/cache-replacements.v1.json
```

Rerun actual preparation and sandbox checks with a new output directory on a host that permits Linux namespaces and bubblewrap:

```sh
python3 docs/research/selected-cache-rebuild-2026-10-02/checks/prepare_check.py \
  --out /tmp/selected-readiness-preparation-fresh
```

## Remaining gates

[User constraints](user-constraints.v1.md) save the Codex-only restriction and reaffirmed $300 aggregate cap. The current paid grader invokes Claude Code. It has no supported Codex grading dispatch path, so no paid session can start under the current restriction. Implement and verify a Codex path, then pin a specific model, effort, client version and concrete execution authorization.

Saved review and setup usage totals $10.169001 in list-price equivalents. Other applicable charges remain unreconciled. The cap includes prior activity, calibration, failed attempts, replacements and retries; remaining budget and reservations are unknown. Unknown usage is not zero. Reconcile that usage before assigning any paid reservation.

The retained calibration design uses four fresh sessions, two control and two enriched, on grpc-go and GraphQL with one worker. The prior Opus selection cannot run under the Codex-only constraint. Token cost, judgment equivalence and calibration reuse remain unmeasured. Rollout requires verified calibration and any reuse requires verified context identity plus saved authorization. Publishing scores remains a separate step.
