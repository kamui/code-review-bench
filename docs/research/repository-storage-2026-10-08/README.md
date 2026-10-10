# Repository payload relocation, 2026-10-08

Part of #81. The [release](https://github.com/kamui/code-review-bench/releases/tag/evidence-repository-payloads-2026-10-08-v1) preserves 3,508 previously tracked files at their original hashes and permissions. Their 568,109,862 bytes occupy 366,965,147 bytes in nine packages, each containing at most 64 MiB of uncompressed members. The [manifest](../../../bench/evidence/manifests/repository-payloads-2026-10-08-v1.json) supplies their original paths. The [restoration receipt](../../../bench/evidence/receipts/repository-payloads-2026-10-08-v1-restoration.json) records independent remote verification.

| Selection | Files | Original bytes |
| --- | ---: | ---: |
| Tracked archives | 1,818 | 313,436,392 |
| Run stdout logs | 379 | 192,429,361 |
| Upstream research JSON snapshots | 1,311 | 62,244,109 |

Small records, decisions, indexes and frozen references stay in Git. No scientific record, original archive bytes or recorded limitation changes. The selection was compared with the public GitHub tree at `4c72200c5fd3b59867d41491c65c2ba30c517ebb` before publication. `prepare.py` reproduces the snapshot and packing; `detach.py` checks every published member against the Git index and source commit before untracking it. Existing local copies remain available as ignored materialized evidence.

## Retrieval

`bun run evidence:fetch` restores all repository payloads. Development, build, benchmark tests and verification commands invoke it before their consumers. CI fetches with its read-only GitHub token. For a direct Python command, fetch explicitly first. One selected archive can instead be fetched with `evidence_store.py fetch --manifest bench/evidence/manifests/repository-payloads-2026-10-08-v1.json --path PATH`. Use `--offline` after retrieval; hashes and modes are still verified.

A first full fetch downloads about 350 MiB and materializes about 542 MiB. Package cache files add the downloaded size again. Smaller clones therefore do not mean that a full local evidence build needs no disk space. Ignore entries name only these exact materialized paths. Changing an existing file causes retrieval to refuse overwrite.

## History and recovery

Removing tracked copies reduces the current Git tree. It does not shrink earlier history. History removal is a separate operation authorized by the repository owner, who will replace existing clones. It must preserve release assets and metadata, back up shared refs, rewrite all retaining branches and tags, and verify a new clone before replacing shared refs. Frozen source commit identities remain historical provenance; never rewrite evidence content to make an old observation look newly generated.

The additional [Git fallback manifest](../../../bench/evidence/manifests/local-git-fallbacks-2026-10-08-v1.json) preserves every distinct payload referenced by the first local migration's 42,255 Git-backed path mappings. `restore-local.py` can retrieve those bytes without old commit IDs. Its 9,215 distinct objects occupy 42,943,699 compressed bytes. The original mappings remain immutable.

The [recovery manifest](../../../bench/evidence/manifests/history-recovery-2026-10-08-v1.json) preserves a complete Git bundle of the pre-rewrite shared branches and tags. Its publication receipt verifies remote package and bundle bytes. Fetch its single member to a disposable directory and run `git clone PATH_TO_BUNDLE recovered-repository` to inspect that history without adding it back to normal clones. A branch deleted after this backup remains in the bundle for recovery only; it must not be recreated on GitHub.

A [second recovery manifest](../../../bench/evidence/manifests/history-recovery-2026-10-10-v1.json) preserves the nine pre-rewrite commits of `refs/recovery/issue-9-selected-cohort-2026-10-02` that the first bundle does not hold. They include `d27a8c24`, the `freeze_commit` of `2026-09-30-selected-prs-review-only`. Its bundle holds only those commits and requires commit `6906541d` from the first: clone the first bundle, then run `git fetch PATH_TO_SECOND_BUNDLE 'refs/recovery/*:refs/recovery/*'` in that clone. The same limit applies: the ref is for recovery only and must not be pushed to GitHub.

A [third recovery manifest](../../../bench/evidence/manifests/history-recovery-freeze-commits-2026-10-10-v1.json) preserves two more freeze commits in one package of two small bundles. `freeze-2026-10-03-codex-ce-sol61-high.bundle` holds `d19fee72`, the `freeze_commit` of `2026-10-03-codex-ce-sol61-high` and `2026-10-03-codex-ce-sol61-high-selected`, and its parent `034c2016`. It requires commit `70aa84b3` from the first bundle: clone the first bundle, then run `git fetch PATH_TO_BUNDLE 'refs/recovery/*:refs/recovery/*'` in that clone. `freeze-2026-09-26-x382-destroyed-tests.bundle` holds `61b659b3`, the `freeze_commit` of `2026-09-26-x382-destroyed-tests`, which was never pushed. It requires commit `f297f308` of `kamui/skills`, which only a pull request head keeps: clone `kamui/skills`, run `git fetch origin refs/pull/397/head`, then fetch the bundle the same way. The same limit applies to both refs.

## Local retirement

The complete inventory and closure plan under `retirement/` identify one redundant linked worktree, `t3code-356b9051`. Its 35,213 entries contained no unique untracked or ignored files; its commits were reachable from merged `main`. The [candidate restoration receipt](retirement/t3code-356b9051-restoration.json) verified all 35,212 payload files and their permissions from the GitHub-derived history backup, without reading the original workspace.

After the reviewed plan was published at `7b8df1f`, both [dry run](retirement/t3code-356b9051-dry-run.json) and [apply](retirement/t3code-356b9051-retired.json) passed the unchanged retirement guards. The worktree was removed. Its files occupied 907,976,704 allocated bytes, about 866 MiB. The tool measured a filesystem free-space increase of 962,166,784 bytes during removal; concurrent host activity can affect that whole-filesystem delta. The candidate directory is absent and Git no longer lists it as a worktree.

All 18 remaining run clone/cache candidates were refused: 17 belong to failed or invalid attempts and one has no filed attempt. Three other clean worktrees retain uncaptured Python bytecode. Other worktrees and the original dependency cache retain the blockers documented in the [first migration](../evidence-migration-2026-10-08/README.md). Publication alone does not authorize their removal or settle the outstanding reservation.

## Verification

The [cold-consumer receipt](../../../bench/evidence/receipts/repository-payloads-2026-10-08-v1-consumers.json) records a fresh checkout at `9c9cb4f56d2a4361c1fb0c9dcc73e0bc337696ea`, with `/home/jack/.t3` hidden and an empty package cache. All 3,508 relocated files and all 42,255 Git-backed paths verified. Fallback restoration preserved permissions with the original path absent. Offline fetch, import verification, current grading, calibration, claims and explorer export passed. The import still reports its original nine historical mismatches and zero unavailable archives. Export totals remain 16 tasks, 17 configurations and 746 attempts.

The relocation also passed 63 web tests, 21 exporter tests, 681 benchmark tests with 18 skips, the provisioning self-test, the static build and type checking. These results cover the retrieval/configuration tree committed at `9c9cb4f`; subsequent additions are receipts and documentation. The first sandboxed build could not start its local prerender listener; the retry with the required permission passed.
