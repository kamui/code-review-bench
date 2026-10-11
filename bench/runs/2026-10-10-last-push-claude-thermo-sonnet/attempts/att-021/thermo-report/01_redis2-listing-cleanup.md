# 01 — redis2 listing orphan cleanup (detail)

Scope: `git diff main...review-head` (range db5a086d0..6c8fde664), files `weed/filer/redis2/universal_redis_store.go` (+17) and `weed/filer/redis2/universal_redis_store_test.go` (+143, new).

## Measurements and commands

- `wc -l weed/filer/redis2/*.go`: `universal_redis_store.go` is 259 lines after the change, the test file is 143. No file approaches the 1k-line threshold, so the file-size rule does not apply.
- `go test -count=1 -v ./weed/filer/redis2` (offline, per policy): PASS, but every case reports SKIP ("redis2 tests are disabled..."). No live Redis was available, so none of the new behaviour was executed by this review. The PR's claim that the tests fail on master and pass with the change could not be re-verified here. Status: unverified by execution; verified by reading.
- `grep miniredis|redismock go.mod`: no hits. The repo has no in-process Redis fake, so the author's opt-in-live-Redis gating follows the existing store-test convention. No workflow in `.github/workflows` mentions Redis for these tests, so CI will skip them.

## Reading of the change

The not-found branch of `ListDirectoryEntries` (universal_redis_store.go:205-211) now calls `removeOrphanedDirectoryListMember` (lines 237-251). That helper does: `ZREM` the member; on ZREM error return; `EXISTS` the value key; if EXISTS succeeded and returned 0 return; otherwise `ZADD NX` the member back, discarding the result. The second commit added the EXISTS/re-add step in response to review comments about a concurrent `InsertEntry` racing the `ZREM`.

I traced the interleavings. With the compensation, the lost-member case from the review (GET nil, concurrent SET + no-op ZADD NX, then ZREM) is repaired. Remaining interleavings are benign or self-healing, with the exceptions below.

## Finding 1 — Remove-then-repair is a non-atomic compensating write whose repair failure is silent (Boundary / atomicity)

Evidence: universal_redis_store.go:237-250. The helper first destroys the index member and then, if the value is back, tries to restore it with `ZAddNX(...)` whose result is not examined (line 250). The step before it (ZREM error) returns early, but the step that matters most — undoing a destructive write on a live entry — has no error handling and no log.

Failure scenario: a listing sees `ErrNotFound`, a concurrent `InsertEntry` recreates the value (its ZADD NX is a no-op because the member is still present), the listing ZREMs the member, EXISTS returns 1, and the restoring ZADD NX fails (timeout, connection reset, failover). The live entry is now permanently missing from listings, nothing is logged, and `UpdateEntry` never re-adds it (the commit message itself says so). This is exactly the failure the second commit set out to prevent; it is now narrower but still reachable and invisible. A second, smaller effect: between ZREM and the restore, concurrent listings of the same directory omit a live entry, which also skews pagination (`lastFileName` / limit).

When `EXISTS` itself errors, the code falls through to the restore (`err == nil && exists == 0` is false), which resurrects a possibly dead member. That is a safe direction, but it is implicit control flow through a compound condition and is undocumented.

Remediation: keep the compensation but make it explicit and observable. Log (at least `glog.Warningf`) when the restoring ZADD NX fails, and write the three outcomes as a small switch (member removed / value reappeared and member restored / repair failed) rather than a compound `err == nil && exists == 0` guard. If the cluster-slot constraint can be sidestepped, a single-slot Lua `if redis.call('EXISTS', KEYS[1]) == 0 then ZREM ...` is the real atomic answer; the commit message argues it cannot be used for the cluster store, which is a fair reason to keep the compensation, but then the failure of the compensation must not be swallowed.

Verification status: confirmed by code reading; not reproduced (no Redis).

## Finding 2 — Two divergent eviction policies now live in the same loop (Spaghetti / missed code-judo)

Evidence: universal_redis_store.go:205-220. The not-found branch calls the guarded helper. Four lines below, the logical-expiry branch still does `store.Client.Del(value)` followed by an unconditional `store.Client.ZRem(dirListKey, fileName)` with both results discarded. The PR body says the new line "mirrors what the expiry branch already does", and commit 2 then made the mirror stop being a mirror.

Consequences: (a) the race the second commit fixed for one branch is still present in its sibling — and worse there, because that branch also DELs the value, so a concurrent recreate between `FindEntry` and `DEL` loses the fresh value as well as the index member; (b) a reader of `ListDirectoryEntries` now has to understand why one eviction is guarded and the other is not; (c) the "remove a stale child from the index" concept has two implementations.

Code-judo proposal: introduce a single `evictStaleChild(ctx, dirPath, fileName, deleteValue bool)` (or two thin callers of one helper) that owns "remove the index member, and repair if the value came back", and have both branches call it. For the expiry branch, `deleteValue` is true. The loop then reads: not found -> evict; expired -> evict with delete; otherwise emit. That deletes the duplicated ZREM and puts the race policy in exactly one place. Pre-existing behaviour of the expiry branch need not change beyond adopting the guard, which keeps the PR's spirit of minimal behaviour change while removing the inconsistency the PR itself introduced.

Verification status: confirmed by code reading.

## Finding 3 — Helper takes three redundant, mutually dependent parameters (Boundary cleanliness)

Evidence: signature at universal_redis_store.go:237, `(ctx, dirListKey string, path util.FullPath, fileName string)`. `dirListKey` is already-prefixed (`getKey(genDirectoryListKey(dir))`), `path` is `dirPath/fileName`, and the helper separately calls `store.getKey(string(path))`. Three arguments encode two facts (directory, name) and rely on the caller having built them consistently, including the keyPrefix rule the PR body explicitly worries about ("rebuilding it unprefixed would have been a silent no-op"). The test at line 112 has to hand-build the same triple.

Remediation: take `(ctx, dirPath util.FullPath, fileName string)` and derive both keys inside, using the same `getKey`/`genDirectoryListKey` pair used by `InsertEntry` and `DeleteEntry`. The keyPrefix hazard then lives in one place rather than being a calling convention. This also lets the helper sit next to `InsertEntry`/`DeleteEntry`, which are the other writers of this index, instead of being appended after the listing function.

Verification status: confirmed by code reading.

## Finding 4 — Verification surface is thin and never runs in CI (Tests)

Evidence: `newTestStore` (test file:16-45) skips unless `RUN_REDIS_TESTS=1`; this review's run shows all four subtests SKIP. `TestRemoveOrphanedDirectoryListMemberKeepsRecreatedEntry` (line 106) calls the helper directly with the value already present, which exercises only the restore branch deterministically; it does not cover the `exists == 0` no-restore branch in isolation, a ZREM error, an EXISTS error, or the restoring ZADD NX failing (Finding 1). No test covers the expiry branch's behaviour after Finding 2 is addressed.

The test file also adds generic package-level helper names (`newTestStore`, `insertTestEntry`, `listNames`, `indexMembers`) in package `redis2`, which will collide with any later test in that package; the Redis-expiry test also sleeps 1.5s of wall-clock.

Remediation: narrow the Redis-facing surface so the decision logic can be tested without a server — e.g. have the helper's three outcomes be a small pure function of (`zremErr`, `exists`, `existsErr`) or put the three Redis calls behind a minimal interface the test can fake — and prefix the helpers (`redis2Test...`) or place them in a `_test` helper file. Keep the live-Redis tests as the integration layer.

Verification status: skip behaviour confirmed by executing; coverage gaps confirmed by reading.

## Non-findings (checked)

- keyPrefix handling is right: the helper receives the prefixed list key and prefixes the value key.
- `err == filer_pb.ErrNotFound` identity comparison remains correct because `FindEntry` returns the bare sentinel only for `redis.Nil`.
- Super-large directories never reach the helper (no index key).
- The `redis3` out-of-scope note in the PR body is reasonable for a store marked desupported.
