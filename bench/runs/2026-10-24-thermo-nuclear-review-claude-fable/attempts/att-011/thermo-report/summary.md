# Thermo-nuclear review: seaweedfs/seaweedfs#10735

Range reviewed: `db5a086d048c5c2d6e51e82bb070d20df04d688d..6c8fde6642cd428e884cc9c9d88c9aaa189c4bf1` (`git diff main...review-head`), two files: `weed/filer/redis2/universal_redis_store.go` (+17) and the new `weed/filer/redis2/universal_redis_store_test.go` (+143).

The change makes `ListDirectoryEntries` in the `redis2` store repair its per-directory ZSET index: when `FindEntry` for a member returns the bare `filer_pb.ErrNotFound` sentinel (only produced for `redis.Nil`), the new helper `removeOrphanedDirectoryListMember` issues `ZREM`, then `EXISTS` on the value key, and re-adds the member with `ZADDNX` if the value is present again or the `EXISTS` call failed.

What I verified and found sound:

- The not-found guard is identity on the sentinel, so wrapped transport errors still break out of the loop and never reach the new code.
- `dirListKey` is the prefixed key computed once at the top of `ListDirectoryEntries`, and the value key in the helper is built with `store.getKey`, so `keyPrefix` deployments are handled.
- Super-large directories never have the ZSET, so the loop body and the helper are unreachable for them.
- All three commands are single-key, so Redis Cluster cross-slot restrictions do not apply.
- The concurrent `InsertEntry` race raised against the first commit is closed for a single-node or master-only deployment: every interleaving of `SET`/`ZADDNX` with `ZREM`/`EXISTS`/`ZADDNX` ends with the member present when the value is present. A concurrent `DeleteEntry` can leave a member for a dead value, which the next listing cleans again, so that is self-healing.
- The filer wrapper calls the store with `context.WithoutCancel`, so a cancelled request cannot cut the helper off between `ZREM` and the re-add.
- `go vet ./weed/filer/redis2/` and `gofmt -l` are clean, and `go test -count=1 ./weed/filer/redis2` builds and skips all three new tests when `RUN_REDIS_TESTS` is unset, as intended. No Redis server was available, so the tests were not executed against a live instance.

## Findings

### Medium — stale replica reads now delete live index members under `redis_cluster2` with `useReadOnly`

**File and lines:** `weed/filer/redis2/universal_redis_store.go` line 208 (the call site) and lines 237–251 (`removeOrphanedDirectoryListMember`).

**Evidence traced.** `RedisCluster2Store.initialize` (`weed/filer/redis2/redis_cluster_store.go:51-62`) passes the `useReadOnly` and `routeByLatency` configuration keys straight into `redis.ClusterOptions.ReadOnly` and `RouteByLatency`. In go-redis v9.21.0, the version pinned in `go.mod`, `ClusterClient.cmdNode` (`osscluster.go:2389-2406`) routes any command whose `COMMAND INFO` carries the `readonly` flag to `slotReadOnlyNode`, which returns a replica (`slotSlaveNode`, `osscluster.go:935-947`) or the closest/random node; every other command goes to the slot master. `GET` and `EXISTS` are read-only commands, `ZREM` and `ZADD` are write commands. Redis Cluster replication is asynchronous, and the value key `<path>` and the index key `<dir>\x00` hash to different slots, so they are typically served by different master/replica pairs with independent lag.

With this routing, the new code path is: `ZRANGEBYLEX` (replica of the directory shard) returns the member; `GET` (replica of the value shard) returns `redis.Nil` because that replica has not yet applied the master's `SET`; the branch at line 207 fires and the helper issues `ZREM` to the directory shard's master, deleting the member for a live value; the recheck `EXISTS` goes to the same lagging replica of the value shard and returns 0, so the helper returns without re-adding. The author's safety argument ("a member cannot legitimately outlive a missing value", "transient errors cannot trigger it") holds for a master-only read path but not here: the value is not missing, it is just not visible yet on the node that was asked, and that staleness is reported as `redis.Nil`, not as an error.

**Failure caused.** Before this PR, a stale replica read during a listing produced a transient symptom: the freshly created entry was omitted from that one listing and appeared on the next. After this PR the same stale read issues a destructive write to the master. The live entry's member is gone from the ZSET, so the entry stops appearing in any directory listing (S3 `ListObjects`, FUSE `readdir`, `weed shell` `ls`, filer HTTP listings) while `FindEntry` by exact path still succeeds. Nothing restores the member: `UpdateEntry` only rewrites the value (`universal_redis_store.go:92-95`), and only a fresh `InsertEntry` on the identical path calls `ZADDNX`. The failure is silent apart from the pre-existing `glog.V(0)` "list ... not found" line. The window is a `PUT` followed by a `LIST` of the parent within the replica lag, which is a routine S3 client pattern (sync tools, multipart finalisers, listing after upload). The exposure becomes wide during a replica full resync: with the Redis default `replica-serve-stale-data yes` a resyncing replica keeps answering from its old dataset, so every listing routed to it during that period reports every value created since the replica disconnected as not found, and each such listing permanently drops those members from the master-side index.

Preconditions: store `redis_cluster2`, `useReadOnly = true` (or `routeByLatency = true`, which also routes reads off the master), replicas present. The option defaults to false, which is why this is Medium rather than High. For deployments that have it on, the fix turns a read-consistency wart they opted into deliberately into persistent index corruption.

A fix that does not depend on master-routed reads would be to perform the recheck in a way that is guaranteed to hit the master, for example `WATCH`/`MULTI` on the value key, or an explicit master-routed `EXISTS` (go-redis exposes `MasterForKey` on `ClusterClient`, but the store only holds a `redis.UniversalClient`), or to confine the cleanup to the single-node and sentinel stores where reads are master-served. At minimum the behaviour should be documented against `useReadOnly`/`routeByLatency`.

## Questions

None.
