# Completion of the host evidence migration

Refs [#81](https://github.com/kamui/code-review-bench/issues/81). This work uses a worktree attached to the fresh, rewritten repository. It preserves frozen runs, previous publication mappings, replacement receipts and earlier findings.

## Publication status

The dependency, supplemental and inventory packages are prepared locally. Their public upload awaits explicit approval. They are not yet independently retrievable from this branch's discovery manifests. The [publication plan](publication-plan.json) pins every prepared package identity. The [prepared verification](prepared-verification.json) is a local package exercise, not a remote publication receipt. Issue #81 remains open until publication, cold remote verification and final review complete.

The recovery copy is published at [evidence-recovery-2026-10-08-v1](https://github.com/kamui/code-review-bench/releases/tag/evidence-recovery-2026-10-08-v1). Its [index](../../../bench/evidence/recovery/2026-10-08-v1.json) is about 2 KiB; it references existing manifests instead of duplicating their large member lists in Git. The [exercise receipt](recovery-verification.json) verifies all 33,196 members with original releases deliberately unavailable, an empty evidence cache and the originating `.t3` roots hidden. Offline restoration of a removed sample passed for each source.

## Host inventory and classifications

The [inventory index](inventory-index.json) covers 32 roots and 2,094,470 non-directory entries. It includes `.t3/bench-runs`, `.t3/bench-cache`, every direct worktree directory present during the scan, and the previously inventoried nested `finish-it-7` worktree. Parent and nested-root counts overlap. These are inventory rows, not unique files or a reclaim estimate.

Every entry has a classification, reason, identity when hashed, permissions and storage status. The compressed per-path records are prepared outside Git. `bench/tools/reconcile_evidence.py` validates them against `bench/schema/evidence-reconciliation.schema.json` and matches published evidence by hash, size and mode. Matching bytes never authorize retirement. Development files retain their development classification; authentication remains excluded; links and unresolved files remain protected.

The snapshot predates the supplemental publication. It identifies 519,952 rows backed by existing release manifests, 1,574,513 retained locally and five excluded authentication files. There are 30,726 unknown rows, principally symlinks and remaining unclassified work. The records explicitly preserve those blockers. The snapshot is read-only and not atomic across active writers; retirement still requires a fresh inventory in an exclusive maintenance window.

The earlier removal of `t3code-356b9051` remains documented by its [retirement receipt](../repository-storage-2026-10-08/retirement/t3code-356b9051-retired.json). This completion step removes no production workspace or cache. The old repository still owns protected worktrees.

## Dependencies

The [dependency catalog](dependency-catalog.json) covers every archive-backed frozen target. All 15 available archives are the replacements already recorded by the [original cohort](../original-cache-rebuild-2026-10-02/cache-replacements.v1.json) and [selected cohort](../selected-cache-rebuild-2026-10-02/cache-replacements.v1.json). Their original archives were deleted before this migration. No original identity is relabeled, no recipe is rebuilt, and no frozen target is edited.

The five prepared release packages total 2,091,406,948 bytes. Archives retain their original filenames under `artifacts/reproducibility/archives/`. Once the published manifest is present, select a target with:

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

After publication, the existing command works for these paths too:

```sh
python3 docs/research/evidence-migration-2026-10-08/restore-local.py \
  bench-cache archives/i-requests-6667.tar.gz /tmp/restored-i-requests-6667.tar.gz
```

Publication review is not investigation closure or deletion authorization. Shared archives do not clear accounting dependencies, certify source trees as unchanged, or establish that a failed attempt is closed.

## Accounting and validation

The [accounting snapshot](accounting-before.json) retains 417 reservations across 21 controlling queues. Independently rediscovering the queues from the fresh full inventory produced the identical [after snapshot](accounting-after.json). One unknown reservation remains outstanding. No benchmark model calls or settlement changes are part of this migration.

Storage, inventory, retirement, pruning, replacement, reconciliation and resolution tests passed, 55 tests total. Three recovery tests also passed, including retrieval while the original release is unavailable and refusal of changed discovery metadata. Remote verification of the new packages and an independent final review remain pending.

Local package and scan artifacts are retained at `/tmp/issue81-completion-20261008` until publication and review finish. They are staging copies, not a shared evidence location or a durable recovery guarantee.
