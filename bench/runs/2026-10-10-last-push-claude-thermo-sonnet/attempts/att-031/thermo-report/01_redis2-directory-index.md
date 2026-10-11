# Detail: redis2 directory index pruning (`weed/filer/redis2`)

## Measurements and commands

- `git diff main...review-head --stat`: `universal_redis_store.go` +17, `universal_redis_store_test.go` +143 (new). Final `universal_redis_store.go` is 259 lines; the 1k-line threshold is not approached.
- `go vet ./weed/filer/redis2` (offline, GOFLAGS=-mod=mod GOPROXY=off): clean.
- `grep -n "ZRem\|ZAddNX" weed/filer/redis2/*.go`: ZAddNX at `InsertEntry` (line 68) and in the new helper (line 250); ZRem at `DeleteEntry` (136), expiry branch (217), new helper (238).
- `grep -rn "RUN_REDIS_TESTS\|redis-server\|services:" .github/workflows`: no match for Redis tests or a Redis service.
- Verification status: static reading only. No Redis server was available, so the live tests skip. Every behavioural claim below is an inference from the code, not an observed run.

## A. Two cleanup policies in one loop

Current code, `ListDirectoryEntries`:

- not found: `removeOrphanedDirectoryListMember` does `ZREM`, then `EXISTS`, then conditional `ZADDNX`.
- expired: `DEL value`, then `ZREM member`, both unconditional, results discarded.

Interleaving that the expiry branch still permits: listing `FindEntry` returns an entry whose `Crtime + TtlSec` is past; a concurrent `InsertEntry` runs `SET` (fresh value, new TTL) and `ZADDNX` (no-op, member present); the listing then runs `DEL` (destroys the fresh value) and `ZREM`. Here the loss is of the value itself, not only the index member, so it is worse than the case the second commit fixed. The PR's own reasoning says this branch is rarely reached for non-updated TTL entries, but updated entries are the one case the PR identifies as reaching it reliably.

Worked code-judo proposal: introduce one method, for example `pruneStaleListMember(ctx, dirListKey, dirPath, fileName)`, that is responsible only for the index: it removes the member and restores it if the value key exists. Not-found calls it directly. The expiry branch calls `Del` for the value and then the same method, which will find the value absent and leave the member removed. That deletes the second policy, so the loop has one rule for member removal. If the maintainers decide the expiry branch should stay as it was, a short comment there stating that the race is accepted is better than silent divergence.

## B. Silent failure in the write-back

Helper body:

1. `ZRem` error: return (member stays, next listing retries; acceptable).
2. `Exists`: if it succeeds and returns 0, return (member stays removed; correct).
3. Otherwise `ZAddNX`, result discarded.

Failure case: steps 1 and 2 succeed with the value present, step 3 fails. State: value present, member absent, no error surfaced, `UpdateEntry` does not re-add. The entry is invisible until another `InsertEntry`. The PR text claims "a failed repair is retried on the next listing", which holds for step 1 only.

Remedies, in order of preference:
- Log the step 3 error at `glog.V(0)` or `Errorf` level with the path, since it marks a state the code knows is wrong.
- Reorder so the dangerous state needs an extra failure: `Exists` first, and only `ZREM` when it returns 0. This does not close the race (an insert can still land between `Exists` and `ZREM`), but it avoids the removal in the common recreated case and removes the need for the restore in that case; the restore then remains only as the second line of defence for that narrow window.
- A fully atomic version needs a Lua script over both keys, which the author notes is unavailable in cluster mode because the keys hash to different slots. Hash-tagging the value key and the index key would be an on-disk key-format change and is rightly out of scope.

The benign mirror case (value deleted between `Exists` and `ZAddNX` resurrects an orphan) self-heals on the next listing, so I do not rate it as a defect.

## C. Signature and logging

- `path util.FullPath` and `fileName string` are redundant; `fileName` is the last path element and `dirListKey` is derivable from `path.DirAndName()`. Prefer `(ctx, dirPath, fileName)` or `(ctx, path)`.
- `getKey(genDirectoryListKey(...))` is built in several places already (lines 68, 130, 136, 178). The helper takes the pre-built key as an argument while recomputing the value key, which splits key construction across caller and callee.
- The V(0) log on every not-found member precedes the repair. After this change, a not-found member is an expected self-healing event, and the log line that the PR body criticises as noisy now fires exactly once per orphan. Log at a higher verbosity or only when the repair fails.

## D. Test coverage

- All three tests depend on `RUN_REDIS_TESTS=1` and a live Redis. No workflow provides one, so the suite is inert in CI.
- `TestRemoveOrphanedDirectoryListMemberKeepsRecreatedEntry` (line 106) inserts a normal entry and calls the helper on it. That exercises the restore branch, but there is no concurrency, and the failure paths in section B are untested.
- `TestListDirectoryEntriesRemovesIndexMembersExpiredByRedis` sleeps 1.5 seconds. Acceptable for an opt-in test; it would be the slowest test in the package if enabled in CI.
- Suggestion: add a Redis service container to an existing workflow, or inject a client double for the failure paths; rename the helper test to describe the single-threaded setup it actually performs.
