# Detail report 01: weed/filer/redis2 (universal_redis_store.go and its new test file)

Scope: `git diff main...review-head` (base db5a086d0, head 6c8fde664), two files, +160/-0. Nothing outside `weed/filer/redis2` changes.

## Measurements and commands

- `wc -l weed/filer/redis2/*.go`: `universal_redis_store.go` goes from 242 to 259 lines, and the new test file is 143 lines. No file gets near the 1k-line boundary, so the file-size rule is not triggered.
- `go test -count=1 -v ./weed/filer/redis2`, run offline as the policy allows: it builds and passes, but every test SKIPs because `RUN_REDIS_TESTS` is unset and no Redis server exists in this environment. So the behaviour claims (fails on master, passes with the change) could not be reproduced here. Verification status for every runtime claim below is "read from source only".
- `grep -rn redis .github/workflows/*.yml` returned nothing. No workflow starts a Redis server, so the new tests are opt-in and never run in CI. That is consistent with the repo's other server-backed store tests, but it means the regression protection is manual.

## Structure of the change

The production diff is one call site in the not-found branch of `ListDirectoryEntries` (line 208) and a new private method `removeOrphanedDirectoryListMember` (lines 237-251). The method does `ZREM`, then `EXISTS` on the value key, then `ZADD NX` to put the member back unless the value is confirmed absent. It is a compensating-action protocol, chosen because a Lua script or MULTI cannot span the two keys under `redis_cluster2` (the value key and `<dir>\x00` have no hash tag). That constraint is real, and the author documented it well. The comments in the code are short and explain the one non-obvious ordering fact.

## Finding 1: two cleanup protocols for the same stale index member in one loop (missed unification)

Location: `weed/filer/redis2/universal_redis_store.go`, lines 205-211 and 214-219 (call site at 208, helper at 237-251).

`ListDirectoryEntries` now has two branches that both mean "this index member is stale, drop it". The not-found branch calls the new helper: checked errors, a `ZREM`, a re-check, and a conditional restore. The logically-expired branch a few lines below still does `store.Client.Del(...).Result()` and `store.Client.ZRem(...).Result()` inline, unconditionally, with both errors discarded. A reader now has to hold two different rules for the same event and work out why the second is weaker. The PR body admits the expiry branch has the same race with a worse outcome (a concurrent insert loses the value itself). That is a disclosed deferral, but it leaves the file with an obviously inconsistent pair of protocols sitting next to each other.

Code-judo proposal: turn the helper into the single "drop this child from the listing" primitive, for example `dropStaleChild(ctx, dirListKey, valueKey, fileName)`. The not-found branch would call it as is. The expiry branch would call it after its `DEL`, so the same `ZREM`/re-check/restore applies to both. Both branches then collapse to "decide the child is stale, call one function". The value-loss race in the expiry branch is a separate change, but with a single primitive that follow-up touches one function instead of two divergent blocks. Even if the unification is deferred, a one-line comment at the expiry branch pointing at the helper would stop the two shapes from looking accidental.

Verification: read from source; behaviour of the expiry branch unchanged by the diff.

## Finding 2: the helper swallows both failure results silently, contrary to the PR's stated error handling

Location: `weed/filer/redis2/universal_redis_store.go`, lines 237-251.

The PR body says errors "are no longer discarded outright". In the code the `ZREM` error path is a bare `return` with no log, so a failing repair leaves no trace. The final `store.Client.ZAddNX(...)` result is discarded entirely, so if the restore itself fails, the exact state the method exists to prevent (a live value with no index member, invisible to listing and `DeleteFolderChildren`) is created silently and permanently, since `UpdateEntry` never re-adds the member. The method returns nothing, so neither the caller nor a test can observe that. At minimum the two failure paths should `glog.V(1)` (or V(0), matching the surrounding style) with the key and the error. Better, return an error and let the caller decide, which also makes the helper testable without a live race.

Verification: read from source.

## Finding 3: three-round-trip compensating sequence has a visible gap and can resurrect a deleted orphan

Location: `weed/filer/redis2/universal_redis_store.go`, lines 238-250.

Between the `ZREM` and the restoring `ZADD NX` the member is absent even though the value exists (in exactly the recreated case the code is repairing). A concurrent `ListDirectoryEntries` or `DeleteFolderChildren` in that gap skips a live entry; `DeleteFolderChildren` would then leave its value key behind. The gap is microseconds wide and self-heals for lists, so this is low severity. The mirror-image case is also possible. If a `DeleteEntry` for the recreated path runs its `DEL` after the helper's `EXISTS` returned 1, the helper's `ZADD NX` then re-adds a member for a value that is gone, which is a fresh orphan. The next listing repairs it, so it is not corruption, only churn. I am not claiming a bug that needs blocking. The point is that the author's convergence argument ("converges in both interleavings") is stated for the insert race only, and the protocol as written is not obviously convergent against delete.

Alternative worth considering: put the ordering burden on the writer. Making `UpdateEntry` (or `doInsertEntry`) idempotently ensure membership would remove the reason the restore exists, but it adds a write to every update, so it is a trade-off and not a clear win. The physical-TTL-behind-logical-TTL idea the author deferred would make the common TTL orphan disappear at its source. The compensating approach is defensible given the cluster constraint; I would just document the delete interleaving.

Verification: read from source; not reproducible without Redis.

## Finding 4: the not-found case is still logged at V(0) as if it were an error

Location: `weed/filer/redis2/universal_redis_store.go`, line 206.

The `glog.V(0).InfofCtx(ctx, "list %s : %v", path, err)` line runs before the `ErrNotFound` test. The PR rationale names per-listing log noise as one of the harms, and cleanup makes the not-found case a routine, self-repairing event (every TTL expiry produces exactly one). It still logs at the default verbosity as a failure-shaped line. Moving the log below the `ErrNotFound` check, or demoting it to `V(1)` for the not-found case, keeps the real error path loud and makes the new branch read as a normal path. Low severity.

## Finding 5: the "race" test does not exercise a race, and the error branches are untested

Location: `weed/filer/redis2/universal_redis_store_test.go`, lines 106-121.

`TestRemoveOrphanedDirectoryListMemberKeepsRecreatedEntry` inserts an entry (value and member both present) and calls the private helper directly with a hand-built `getKey(genDirectoryListKey(...))`. It correctly proves the restore branch fires when the value exists, and the PR notes it fails if the restore is deleted. But it is tied to the private helper's exact signature. No test covers the `ZREM`-error return or the `EXISTS`-error-restores branch that the PR body describes as a deliberate bias. A small interface seam or a client that errors on demand would cover them, and returning an error from the helper (Finding 2) makes them assertable. Also, with a fixed `time.Sleep(1500ms)` the TTL test is slower than needed and slightly timing-sensitive on a loaded server; polling `EXISTS` with a deadline would be steadier. All three tests skip without `RUN_REDIS_TESTS=1`, so they do not guard the change in CI. Low severity.

## What is fine

- Key prefix handling is correct: the helper reuses the already-prefixed `dirListKey` and calls `getKey` for the value key.
- The `err == filer_pb.ErrNotFound` identity check keeps transient errors out of the cleanup.
- Super-large directories are unaffected because the loop body never runs for them.
- Footprint is minimal and no new type, flag or mode was introduced into the existing flow.
