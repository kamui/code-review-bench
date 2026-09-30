# Thermo-nuclear code quality review — seaweedfs#10735

"fix(redis2): remove orphaned directory index members on listing". Range `db5a086d0..6c8fde664`, 2 files, +160 / −0.

## Verdict

Changes requested, but narrow ones. The fix is small and correct in its central claim: a member whose value key is confirmed absent is removed, and a concurrently recreated value keeps its member. It uses only single-key commands, so it is safe under the cluster store. It adds no file-size problem (`universal_redis_store.go` goes from 242 to 259 lines) and no new layer. There are two actionable findings. First, the PR introduces a second, race-guarded way to drop a dead child from the directory index while leaving the original unguarded one nine lines below it. The loop now encodes one concept two ways, and the obvious code-judo move collapses them. Second, the test that protects the new race guard never runs under a key prefix, so the one prefix-sensitive line the PR adds is effectively untested. A third item is a question for maintainers, not a demand on this PR: whether the store should be doing logical TTL expiry at all, given that the filer already does it canonically.

The package builds offline, and `go test -count=1 -v ./weed/filer/redis2` shows all three new tests skipping because no Redis server is available. Nothing in this review was verified against a live Redis.

## Findings

### S1 — Two divergent "drop this child from the index" procedures in one loop

In `weed/filer/redis2/universal_redis_store.go`, `ListDirectoryEntries` now reaps dead children in two places. The not-found branch (line 208) calls the new `removeOrphanedDirectoryListMember`, which runs `ZREM`, re-checks the value with `EXISTS`, and restores the member with `ZADDNX` unless the value is confirmed absent. The logical-expiry branch (lines 214-219) still runs an inline `Del` + bare `ZRem` with both results discarded. After its `DEL`, the expiry branch is in exactly the state the helper was written for, so it has the same index-member loss that the second commit fixed next door. The PR also adds its call two levels deep in the existing `if err != nil { log; if ErrNotFound {...} ; break } else { if ttl { if expired {...} } }` ladder. It leaves the default-verbosity `glog.V(0)` line firing for what is now a normal, self-healing event. Remedy: rename the helper for the guarantee it provides (for example `dropDirectoryListMember`: remove unless the value exists, and keep the member on error), and state that invariant in its doc comment. Call it from both the not-found and the expiry branches, the latter after its value `DEL`. Then flatten the loop into a four-case `switch` over the `FindEntry` outcomes: not-found, error, expired, live. That gives one procedure with one comment and no nested ladder, and the error log stays with real errors. The worked rewrite, and the reason for keeping `fileName` as a verbatim parameter rather than deriving it via `DirAndName`, are in `01_redis2_store.md`.

### T1 — The race guard's prefixed key lookup is not covered by any prefixed test

In `weed/filer/redis2/universal_redis_store_test.go`, only `TestListDirectoryEntriesRemovesOrphanedIndexMembers` loops over `keyPrefix` `""` and `"sw:"`. `TestRemoveOrphanedDirectoryListMemberKeepsRecreatedEntry` and `TestListDirectoryEntriesRemovesIndexMembersExpiredByRedis` run only with `""`. The only key the new helper builds itself is `store.getKey(string(path))` in the `EXISTS` re-check (`universal_redis_store.go:245`). If that `getKey` were dropped, every test would still pass. The prefixed orphan test expects removal, and the unprefixed `EXISTS` returns 0 anyway. The recreate test uses an empty prefix, where both keys coincide. Meanwhile every production deployment with `keyPrefix` set would lose the recreate guard silently. That is the exact failure class the PR body warns about. Remedy: hoist the prefix loop into a shared `forEachKeyPrefix(t, fn)` runner and run all three tests through it. Also state the prefixed directory-index key once in a test helper instead of rebuilding it at lines 74 and 110. I verified this by mutation reasoning, not execution, since no Redis was available. Details in `02_redis2_tests.md`.

## Questions

### Q1 — Should logical TTL expiry live in the store at all?

`Filer.doListDirectoryEntries` (`weed/filer/filer.go:461-499`) and `Filer.FindEntry` (`filer.go:426-437`) already apply the canonical `Crtime + TtlSec` policy and delete expired entries through `Store.DeleteOneEntry`. That makes the redis2 store's own expiry branch (`universal_redis_store.go:214-219`) a second copy of filer policy inside a store adapter. It is also where the pre-existing value-loss race the PR body scopes out comes from. If every production caller of `ListDirectoryEntries` goes through the filer, deleting that branch would leave the loop with a single dead-child case and remove the race rather than needing a compare-and-delete. Direct store callers (metadata tooling that iterates a `FilerStore` without the filer) would start seeing logically expired entries, so this needs a caller audit and is not a blocker here. See `01_redis2_store.md`.

## Checked and sound

The branch guard uses identity against the bare `filer_pb.ErrNotFound`, so transport errors cannot trigger a repair. `dirListKey` is reused already prefixed. Every interleaving with a concurrent `InsertEntry` converges to "value present ⇒ member present". A concurrent `DeleteEntry` can leave a stale member, which is the intended failure bias and is reaped on the next listing. Single-key commands are correct for `redis_cluster2`, where a multi-key script would hit `CROSSSLOT`. Super-large directories never reach the new code. The test gating and per-test directory isolation follow existing store-test conventions.

## Proposed remediation sequence

1. Hoist the prefix loop into a shared runner and run all three tests under `""` and `"sw:"` (T1). Doing this first means the next step is refactored under a test that can actually catch a prefix slip.
2. Rename the helper after its guarantee, document the invariant, and route the expiry branch's index removal through it (S1).
3. Flatten the listing loop into a `switch` over `FindEntry` outcomes, and take the not-found case off the `glog.V(0)` path (S1).
4. As a follow-up, not in this PR: audit direct `FilerStore.ListDirectoryEntries` callers and decide whether the store-level expiry branch can be deleted in favour of the filer's (Q1).

## Detail files

- `01_redis2_store.md` covers the store change: measurements, finding S1 with the worked `switch` + helper rewrite, question Q1, and the interleaving and cluster checks.
- `02_redis2_tests.md` covers the test file: finding T1 with the mutation argument and the `forEachKeyPrefix` runner, plus a minor key-duplication note.
