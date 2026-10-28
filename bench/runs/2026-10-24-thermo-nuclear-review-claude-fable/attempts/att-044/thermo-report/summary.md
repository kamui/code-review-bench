# Thermo Nuclear Review — seaweedfs/seaweedfs#10735

Range reviewed: `db5a086d048c5c2d6e51e82bb070d20df04d688d..6c8fde6642cd428e884cc9c9d88c9aaa189c4bf1` (`main...review-head`).

Files changed:

- `weed/filer/redis2/universal_redis_store.go` (+17)
- `weed/filer/redis2/universal_redis_store_test.go` (new, +143)

## Verdict

No actionable findings.

## What the change does

`ListDirectoryEntries` in the redis2 store iterates the per-directory ZSET index and calls `FindEntry` for each member. Before this change, a member whose value key had disappeared (Redis-side TTL expiry, out-of-band `DEL`, eviction, or a `DeleteEntry` that failed between its `DEL` and its `ZREM`) was logged and skipped but never removed, so the index grew without bound under TTL workloads.

The change adds one call in the not-found branch (`universal_redis_store.go:208`) to a new helper `removeOrphanedDirectoryListMember` (`universal_redis_store.go:237-251`) that:

1. `ZREM`s the member from the already-prefixed `dirListKey`; on error it returns and leaves the member for the next listing.
2. Runs `EXISTS` on the prefixed value key. If the key is confirmed absent, it returns.
3. Otherwise (value present again, or `EXISTS` itself failed) it `ZADDNX`es the member back.

The second commit adds step 2 and 3 to close the concurrent-recreate race that the first commit opened: an `InsertEntry` landing between the listing's `GET` and its `ZREM` would `SET` the value and then find the member still present, so its own `ZADDNX` would be a no-op, and the listing's `ZREM` would then strip the only index member for a live value.

## What I traced

**Key construction.** `dirListKey` is built once at `universal_redis_store.go:178` via `store.getKey(genDirectoryListKey(...))` and the helper reuses that variable. The value key is rebuilt as `store.getKey(string(path))`, where `path` is `util.NewFullPath(dirPath, fileName)`, which is the same expression `doInsertEntry` uses at `:86` and `DeleteEntry` uses at `:126`. Prefixed and unprefixed deployments behave identically; the test covers both.

**Branch guard.** `FindEntry` (`:97-117`) returns the bare `filer_pb.ErrNotFound` sentinel only on `redis.Nil`; every other error is wrapped with `fmt.Errorf`. The guard at `:207` is an identity comparison, so transport or decode errors still `break` out of the loop as before and never reach the cleanup.

**Recreate race, all interleavings.** Writing L for the listing and I for a concurrent `InsertEntry`:

- L:GET nil → I:SET → I:ZADDNX(no-op, member present) → L:ZREM → L:EXISTS=1 → L:ZADDNX re-adds. Correct.
- L:GET nil → L:ZREM → L:EXISTS=0 → return → I:SET → I:ZADDNX adds. Correct.
- L:GET nil → L:ZREM → I:SET → L:EXISTS=1 → I:ZADDNX adds → L:ZADDNX no-op. Correct.

A concurrent `DeleteEntry` interleaved after `L:EXISTS=1` but before `L:ZADDNX` leaves a member with no value, which is exactly the orphan state this PR repairs on the next listing, and is the pre-change steady state. Not a regression.

**Error handling in the helper.** A failed `EXISTS` falls through to `ZADDNX`, which restores the member just removed. That is the conservative direction: the worst case is one orphan surviving until the next listing. A failed `ZREM` returns early, same outcome. The `ZADDNX` result is discarded, matching the adjacent expiry branch at `:216-217`; a lost re-add can only occur if `EXISTS` and `ZADDNX` both fail after `ZREM` succeeded *and* an insert landed in the same window.

**Context cancellation.** `FilerStoreWrapper.ListDirectoryEntries` and `ListDirectoryPrefixedEntries` (`weed/filer/filerstore_wrapper.go:324`, `:341`) wrap the context in `context.WithoutCancel`, so a client disconnect cannot cut the three-command sequence short mid-way.

**Super-large directories.** `InsertEntry` and `DeleteEntry` skip the ZSET for directories in `superLargeDirectoryHash`, so the index key normally does not exist and the loop body never runs. If a directory was promoted to super-large after members already existed, the helper can at most re-add a member it just removed. Net state unchanged.

**Pagination.** `lastFileName` is assigned at `:204` before the branch and the next page starts exclusive of it, so removing the member behind the cursor does not affect continuation. The short-page behaviour when a page is mostly orphans is unchanged from before (orphans were skipped then too), and `prefixFilterEntries` (`filerstore_wrapper.go:364-409`) drives continuation off `lastFileName`, not off page fullness.

**Pre-existing race shape, not introduced here.** `UpdateEntry` (`:92-95`) only `SET`s and never `ZADDNX`es. If a value expires at Redis in the sub-second gap between the filer's `FindEntry` and the resulting `Store.UpdateEntry`, and a listing's cleanup runs in that same gap, the re-written value has no index member. The identical shape already exists on `main` between the logical-expiry branch's `DEL`+`ZREM` (`:216-217`) and `UpdateEntry`, and between `DeleteEntry`'s `ZREM` and `UpdateEntry`. Because the Redis-side TTL deadline is later than the logical deadline the filer checks, the filer's `FindEntry` cannot succeed on a value that Redis is about to drop except within the sub-millisecond skew between stamping `Crtime` and the `SET`. I do not consider this actionable for this PR; it is a property of the store's non-atomic two-key design that this change neither widens materially nor could fix alone.

**Security.** No new input reaches Redis beyond `fileName`, which was already read from the ZSET and already used as a key component on the existing path. No new network surface, permissions, or secrets.

**Devex.** The new test file skips unless `RUN_REDIS_TESTS=1`, with `REDIS_ADDR` defaulting to `127.0.0.1:6379`. Neither variable is referenced by any Makefile or CI workflow in the tree, so the default `go test ./weed/...` path is unchanged. Confirmed offline:

```
gofmt -l weed/filer/redis2/        -> clean
go vet ./weed/filer/redis2/        -> clean
go test -count=1 -v ./weed/filer/redis2
    TestListDirectoryEntriesRemovesOrphanedIndexMembers/keyPrefix=     SKIP
    TestListDirectoryEntriesRemovesOrphanedIndexMembers/keyPrefix=sw:  SKIP
    TestRemoveOrphanedDirectoryListMemberKeepsRecreatedEntry           SKIP
    TestListDirectoryEntriesRemovesIndexMembersExpiredByRedis          SKIP
ok  github.com/seaweedfs/seaweedfs/weed/filer/redis2
```

No Redis server was available in this environment, so the live-Redis assertions were not executed here.

**Test cleanup.** `newTestStore` registers `DeleteFolderChildren(dir)` then `DeleteEntry(dir)`; `DeleteEntry` also `ZREM`s the test directory name from the `/` index, where it was never added, so it is a harmless no-op on a shared instance.

**Other embedders.** `UniversalRedis2Store` is embedded only by `redis_store.go`, `redis_cluster_store.go`, and `redis_sentinel_store.go` within the same package. The cluster store issues `ZREM`, `EXISTS`, and `ZADDNX` as independent commands, so the two keys hashing to different slots is not a problem. No other package references the type.

## Cost note (not a finding)

Each orphan now costs two extra round trips (`ZREM`, `EXISTS`) on the listing that finds it, versus zero before. That is a one-time cost per orphan that replaces an unbounded per-listing `GET` and a per-listing `glog.V(0)` line, so it is a net reduction.
