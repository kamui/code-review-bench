# Completion of the host evidence migration

Refs [#81](https://github.com/kamui/code-review-bench/issues/81). This work uses a worktree attached to the fresh, rewritten repository. It preserves frozen runs, previous publication mappings, replacement receipts and earlier findings.

## Publication status

The dependency, supplemental and inventory packages are published at [evidence-completion-2026-10-08-v1](https://github.com/kamui/code-review-bench/releases/tag/evidence-completion-2026-10-08-v1). The user approved all three public uploads, including the path listings and hash-only audit records. Authentication files remain excluded. The [publication plan](publication-plan.json) pins the prepared identities and their published discovery manifests. All seven packages passed remote download, hash, member and permission verification. The [prepared verification](prepared-verification.json) records the earlier local exercise.

The recovery copy is published at [evidence-recovery-2026-10-08-v1](https://github.com/kamui/code-review-bench/releases/tag/evidence-recovery-2026-10-08-v1). Its [index](../../../bench/evidence/recovery/2026-10-08-v1.json) is about 2 KiB; it references existing manifests instead of duplicating their large member lists in Git. The [exercise receipt](recovery-verification.json) verifies all 33,196 members with original releases deliberately unavailable, an empty evidence cache and the originating `.t3` roots hidden. Offline restoration of a removed sample passed for each source.

## Host inventory and classifications

The [inventory index](inventory-index.json) covers 32 roots and 2,094,470 non-directory entries. It includes `.t3/bench-runs`, `.t3/bench-cache`, every direct worktree directory present during the scan, and the previously inventoried nested `finish-it-7` worktree. Parent and nested-root counts overlap. These are inventory rows, not unique files or a reclaim estimate.

Every entry has a classification, reason, identity when hashed, permissions and storage status. The compressed per-path records and raw snapshots are shared through the [inventory manifest](../../../bench/evidence/manifests/host-inventory-2026-10-08-v1.json). Git stores the compact index; the 169.7 MiB release package holds the full inventories and audit metadata. `bench/tools/reconcile_evidence.py` validates them against `bench/schema/evidence-reconciliation.schema.json` and matches published evidence by hash, size and mode. Matching bytes never authorize retirement. Development files retain their development classification; authentication remains excluded; links and unresolved files remain protected.

The snapshot predates the supplemental publication. It identifies 519,952 rows backed by existing release manifests, 1,574,513 retained locally and five excluded authentication files. There are 30,726 unknown rows, principally symlinks and remaining unclassified work. The records explicitly preserve those blockers. The snapshot is read-only and not atomic across active writers; retirement still requires a fresh inventory in an exclusive maintenance window.

The earlier removal of `t3code-356b9051` remains documented by its [retirement receipt](../repository-storage-2026-10-08/retirement/t3code-356b9051-retired.json). This completion step removes no production workspace or cache. The old repository still owns protected worktrees.

## Dependencies

The [dependency catalog](dependency-catalog.json) covers every archive-backed frozen target. All 15 available archives are the replacements already recorded by the [original cohort](../original-cache-rebuild-2026-10-02/cache-replacements.v1.json) and [selected cohort](../selected-cache-rebuild-2026-10-02/cache-replacements.v1.json). Their original archives were deleted before this migration. No original identity is relabeled, no recipe is rebuilt, and no frozen target is edited.

The five published release packages total 2,091,406,948 bytes. Archives retain their original filenames under `artifacts/reproducibility/archives/`. Select a target with:

```sh
python3 bench/tools/evidence_store.py fetch \
  --manifest bench/evidence/manifests/dependencies-2026-10-08-v1.json \
  --path artifacts/reproducibility/archives/i-requests-6667.tar.gz
```

Pass `artifacts/reproducibility` as the existing `--cache-root`, together with the target's cataloged `--cache-replacements` manifest. To verify all archives and exercise the existing cache restorer:

```sh
python3 docs/research/evidence-completion-2026-10-08/verify-dependencies.py \
  --out /tmp/dependency-restoration.json
```

Add `--offline` after fetching. Source mirrors, pinned external interpreters, the frozen platform and recorded smoke limitations remain separate requirements. The local exercise restored all 15 caches with networking disabled and the originating `.t3` directories hidden. It did not dispatch reviews, grade results or rerun benchmarks.

## Previously withheld paths

The first capture withheld 843 paths after a conservative scan. Recursive inspection now distinguishes public source/package fixtures, local TLS test fixtures, public preview QR links, already-public session account metadata, random prefixes in encrypted session fields and malformed JWT-shaped compiler bytes. Authentication secrets and authentication files are excluded. The saved test-generation scripts establish the origin of 14 local TLS fixture keys.

The supplemental package is 128,402,625 bytes. It contains 784 distinct additional objects and a versioned mapping for all 843 paths; repeated dependency archives resolve to the dependency packages. The [mapping reference](withheld-summary.json) pins the compressed map. Original captures and their withheld decisions remain unchanged. `restore-local.py` consults the new map only for a previously blocked row and requires an exact original root, path, hash, size and mode match.

The existing command works for these paths too:

```sh
python3 docs/research/evidence-migration-2026-10-08/restore-local.py \
  bench-cache archives/i-requests-6667.tar.gz /tmp/restored-i-requests-6667.tar.gz
```

Publication review is not investigation closure or deletion authorization. Shared archives do not clear accounting dependencies, certify source trees as unchanged, or establish that a failed attempt is closed.

## Accounting and validation

The [accounting snapshot](accounting-before.json) retains 417 reservations across 21 controlling queues. Independently rediscovering the queues from the fresh full inventory produced the identical [after snapshot](accounting-after.json). One unknown reservation remains outstanding. No benchmark model calls or settlement changes are part of this migration.

Storage, inventory, retirement, pruning, replacement, reconciliation and resolution tests passed, 55 tests total. Three recovery tests also passed, including retrieval while the original release is unavailable and refusal of changed discovery metadata. The [cold remote receipt](completion-remote-verification.json) records retrieval of all new package members from an empty checkout with the original `.t3` roots and local preparation directories hidden. It validates all 32 inventory records and all 843 resolved paths, then exercises `restore-local.py` on a formerly withheld dependency. The [dependency receipt](dependency-restoration.json) records successful restoration of all 15 replacement caches with networking disabled. Independent review gates PR publication.

To fetch all inventory and audit records:

```sh
python3 bench/tools/evidence_store.py fetch \
  --manifest bench/evidence/manifests/host-inventory-2026-10-08-v1.json
```

Run `verify-completion.py --out /tmp/completion-verification.json` in an empty checkout with the originating `.t3` roots hidden to repeat the remote exercise. Run `verify-dependencies.py --offline --out /tmp/dependency-restoration.json` after fetching to repeat dependency restoration without networking.

This completes the remaining inventory and dependency-publication criteria in #81. Incomplete, unknown, active and accounting-dependent work remains local and protected. Publication authorizes no deletion. Future retirement still requires its own fresh exclusive-window inventory, closure and restoration receipts. Local staging copies are temporary and are not a durable recovery guarantee.
