# Thermo-nuclear review: seaweedfs/seaweedfs#10735

"fix(redis2): remove orphaned directory index members on listing"

Range reviewed: `db5a086d048c5c2d6e51e82bb070d20df04d688d..6c8fde6642cd428e884cc9c9d88c9aaa189c4bf1` (`git diff main...review-head`), 2 files, +160 / -0.

## Verdict

Two actionable findings: one High, one Low.

The change is correct for the default deployments (`redis2`, `redis2_sentinel`, and `redis_cluster2` with replica reads off). The High finding applies only to `redis_cluster2` when reads are routed to replicas; there the new cleanup turns a transient stale read into a permanent loss of a live entry from its directory index.

Neither finding was reproduced at runtime, because no Redis server is available in this environment. Both are traced through the code at the head SHA and through go-redis v9.21.0 in the module cache. `go test -count=1 -v ./weed/filer/redis2` builds and passes, with all three new tests (four cases) skipping on the `RUN_REDIS_TESTS` gate.

## Findings

### 1. [High] Orphan cleanup removes live entries from the directory index when `redis_cluster2` reads from replicas

**File:** `weed/filer/redis2/universal_redis_store.go` lines 237-251 (`removeOrphanedDirectoryListMember`), called from line 208.

**Evidence.**

- The new branch takes a single `GET` returning `redis.Nil` (`FindEntry`, lines 99-102, called at line 203) as proof that the value is gone, and issues `ZREM` on the index (line 238). The PR's argument that "member present implies the value was written, so a missing value was deleted or expired" only holds when the `GET` is answered by the node that accepted the `SET`.
- `RedisCluster2Store` passes `useReadOnly` and `routeByLatency` into `redis.ClusterOptions{ReadOnly, RouteByLatency}` (`weed/filer/redis2/redis_cluster_store.go` lines 27-28, 40-41, 53-54). `routeByLatency` is a documented option in `weed/command/scaffold/filer.toml` lines 289-290. (The scaffold's `readOnly` key on line 288 is not the key the code reads, which is `useReadOnly`; `routeByLatency` does match.)
- In go-redis v9.21.0, `RouteByLatency` forces `ReadOnly = true` (`osscluster.go` lines 205-207). With `ReadOnly` set, every command the server flags read-only is routed through `slotReadOnlyNode` to a replica, or to the lowest-latency node of the slot (`osscluster.go` lines 2399-2404, 2438-2450, 935-961). Writes still go to the slot master. `ZRANGEBYLEX`, `GET` and `EXISTS` are read-only commands; `ZREM` and `ZADD` are writes.
- The value key `<dir>/<name>` and the index key `<dir>\x00` hash to different slots (the second commit message says so itself), so they normally live on different shards whose replicas lag independently.

**Failure.**

1. `InsertEntry` runs `SET <dir>/<name>` on shard X's master, then `ZADD NX <dir>\x00 <name>` on shard Y's master (lines 58-68).
2. A listing reads `ZRANGEBYLEX` from Y's replica, which already has the member, and `GET` from X's replica, which has not applied the `SET` yet. The result is `redis.Nil`, so `ErrNotFound`.
3. The new code sends `ZREM` to Y's master and removes the member of a live entry. The recheck `EXISTS` (line 245) is also a read-only command, so it goes back to X's read node; if that node is still behind it returns 0 and the function returns without restoring the member.
4. The value now exists with no index member, and nothing puts it back: `UpdateEntry` is a bare `SET` (lines 92-95), and `Filer.CreateEntry` on an existing path takes the `UpdateEntry` route (`weed/filer/filer.go` lines 275-277).

The entry stays readable by exact path but is permanently absent from directory listings (`ls` on a mount, S3 ListObjects). `DeleteFolderChildren` (lines 151-167) iterates the index, so a recursive delete of the parent leaves the value key behind.

Before this PR, step 3 was a plain `continue`: the entry was missing from that one listing and reappeared once the replica caught up. The PR makes the same stale read destructive.

The trigger is a listing shortly after a create while the value shard's replica is a few milliseconds further behind than the index shard's replica. A replica serving stale data during a resync, or a remote replica (the case `routeByLatency` exists for), widens that to every entry created inside the lag window, for every listing of their directories.

**Scope.** Unaffected: the default configuration (`useReadOnly` and `routeByLatency` both false), `redis2`, and `redis2_sentinel`, which all read from the master.

**Fix direction.** Confirm absence on the master before removing, for example by running the recheck through `ClusterClient.MasterForKey` (available in go-redis v9.21.0) and removing only after that check; or skip the repair when the client is configured to read from replicas.

### 2. [Low] An `UpdateEntry` that lands after the recheck still leaves a live value with no index member

**File:** `weed/filer/redis2/universal_redis_store.go` lines 245-248.

**Evidence.** The `EXISTS` recheck added in the second commit only protects writers that add the member after writing the value, which is `InsertEntry`. `UpdateEntry` is `SET` only (lines 92-95) and never touches the index. The remaining interleaving is:

1. `Filer.CreateEntry` reads the old entry (`weed/filer/filer.go` lines 240-243) and will therefore take the update route.
2. Redis expires the value key.
3. A listing gets `redis.Nil`, runs `ZREM`, and `EXISTS` returns 0.
4. `Store.UpdateEntry` re-creates the value with a fresh Redis TTL (`weed/filer/filer.go` lines 277 and 397).

Before this PR the member survived step 3, so the re-created value stayed listed. `maybeLazyListFromRemote` has the same find-then-update shape (`weed/filer/filer_lazy_remote_listing.go` lines 96-118).

**Failure.** The re-created entry is live but invisible to listings until something calls `InsertEntry` on that path again.

- For plain TTL entries this is harmless: `Filer.UpdateEntry` keeps the old `Crtime` (`weed/filer/filer.go` line 378), so the re-created value is already logically expired and the filer discards it on the next lookup.
- For S3-expiry entries it is not: their expiry is computed from `Mtime` (`weed/filer/entry.go` lines 215-222), so an overwritten object lives a full `TtlSec` without appearing in listings.

The window is the filer's find-to-update interval coinciding with the Redis expiry instant and a concurrent listing, so it is narrow. It is reported because the second commit message describes this exact loss ("`UpdateEntry` does not re-add it, so the loss persisted") as closed, and for the update path it is only narrowed.

## Checked, no finding

- **Insert race from the earlier review threads.** Every interleaving of `InsertEntry` (`SET`, then `ZADD NX`) against `ZREM` / `EXISTS` / `ZADD NX` ends with the member present when the value is present, as long as reads come from the master.
- **Delete racing the re-add.** A `DeleteEntry` between `EXISTS` and the restoring `ZADD NX` can leave an orphan member, but the next listing removes it.
- **Error handling.** A failed `ZREM` returns without further action. A failed `EXISTS` restores the member, which is the conservative choice. Transient `FindEntry` errors are wrapped, so they cannot match the `err == filer_pb.ErrNotFound` identity check and still break out of the loop.
- **`keyPrefix`.** `dirListKey` is already prefixed at line 178, and the recheck uses `store.getKey(string(path))`, the same key `FindEntry` reads.
- **Super-large directories.** They have no index key, so the new code is unreachable for them. `redis2_sentinel` never loads the map, and a lookup on the nil map is safe.
- **Path-specific stores.** `FilerStorePathTranslator` translates insert, find and list consistently, so the listing rebuilds the same value key the insert wrote.
- **Directory entries.** `CreateEntry` forces `TtlSec = 0` for directories and `ensureParentDirectoryEntry` creates them without a TTL, so the cleanup does not routinely remove directories that still have children.
- **Tests.** The new test file compiles and skips cleanly without `RUN_REDIS_TESTS=1`. It needs no new environment variable for an ordinary `go test` run, cleans up its own keys under a unique directory, and does not change developer workflow. No workflow under `.github` sets `RUN_REDIS_TESTS`, so the tests are opt-in only.
- **Security, feature gates, devex.** No new input reaches a Redis command (member names and paths were already used as keys), and no feature gates or build or run requirements change.
