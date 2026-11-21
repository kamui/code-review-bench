# Thermo-nuclear review — seaweedfs/seaweedfs#10735

"fix(redis2): remove orphaned directory index members on listing"
Range `db5a086d0..6c8fde664` (`main...review-head`), 2 files, +160 / −0.

One actionable finding (Medium). On the default routing (standalone, sentinel, and cluster reading from masters) the change holds up, including the concurrent-insert race that commit 2 addresses.

## Findings

### 1. [Medium] Orphan cleanup treats a replica's stale "not found" as authoritative, permanently dropping live entries from the directory index on `redis_cluster2` with `useReadOnly` / `routeByLatency`

**Where:** `weed/filer/redis2/universal_redis_store.go:207-211` (new call in `ListDirectoryEntries`) and `weed/filer/redis2/universal_redis_store.go:237-251` (`removeOrphanedDirectoryListMember`).

**Evidence traced:**

- `RedisCluster2Store` passes `useReadOnly` and `routeByLatency` straight into `redis.ClusterOptions{ReadOnly, RouteByLatency}` (`weed/filer/redis2/redis_cluster_store.go:27-28,40-41,49-56`). Both are supported, documented options (`weed/command/scaffold/filer.toml:287-290`), off by default.
- In go-redis v9.21.0 (the version pinned in `go.mod`), `RouteByLatency` forces `ReadOnly = true` (`osscluster.go:205-206`), and with `ReadOnly` set every command the server flags read-only is routed by `cmdNode` → `slotReadOnlyNode` to a replica (`slotSlaveNode`) or to the lowest-latency node (`osscluster.go:2399-2405, 2438-2450, 935-961`). `ZRANGEBYLEX`, `GET` and `EXISTS` are all read-only commands; `ZREM` and `ZADD` go to the master.
- The directory index `<dir>\x00` and the value key `<dir>/<name>` hash to different slots (the PR's second commit message says so itself), so in general they sit on different shards whose replicas lag independently. Redis replication is asynchronous.
- `InsertEntry` writes `SET` then `ZADDNX` to the masters (`universal_redis_store.go:56-74`). A listing shortly afterwards can read the member from an up-to-date replica of the index shard while `FindEntry`'s `GET` (`:99-102`) hits a replica of the value shard that has not yet applied the `SET`. That returns `redis.Nil` → `filer_pb.ErrNotFound` for an entry that is fully present on the masters.
- Before this PR that branch only `continue`d: the entry was missing from that one listing and reappeared as soon as the replica caught up. After this PR the branch issues `ZREM` (`:238`) against the master, removing the real member.
- The recheck added in commit 2 does not catch this: `Exists` (`:245`) is itself a read-only command, so it is routed to the same class of lagging replica and returns 0, and the function returns at `:246-248` without the restoring `ZAddNX`. The recheck is only authoritative when reads go to the master.
- Nothing puts the member back afterwards. `UpdateEntry` is `doInsertEntry` only (`:92-95`), and `Filer.CreateEntry` takes the `UpdateEntry` path whenever the entry is found (`weed/filer/filer.go:253-281`), which it is, since the value key exists. Only a delete followed by a fresh create restores it.

**Failure:** With `useReadOnly = true` or `routeByLatency = true`, any listing that lands inside the replica-lag window after a create turns a transient stale read into durable index damage. The entry stays readable by exact path (GET/HEAD succeed) but is permanently absent from every directory listing (filer UI, S3 `ListObjects`, mount `readdir`, `filer.sync`/backup walks). Downstream effects of an invisible child: `doBatchDeleteFolderMetaAndData` decides emptiness and collects chunks from the listing (`weed/filer/filer_delete_entry.go:83-129`), so a non-recursive delete of the parent succeeds as "empty", and both it and a recursive delete leave the hidden child's value key in Redis and its chunks on the volume servers, since `DeleteFolderChildren` also iterates index members only (`universal_redis_store.go:151-167`).

The window is normally milliseconds per created file, which makes it likely under "write then list" or concurrent-ingest-plus-listing workloads at volume. It is unbounded when a replica is stale but still reachable (broken replication link with the default `replica-serve-stale-data yes`, or a replica far behind under load): then every listing strips every recently created name whose value lives on that shard. In all of these cases the pre-PR behaviour healed on its own once the replica caught up; the new behaviour does not.

The PR body's safety argument ("a member cannot legitimately outlive a missing value", "transient errors cannot trigger it") assumes `redis.Nil` is an authoritative answer. That holds for standalone, sentinel (`NewFailoverClient` reads from the master; `redis_conf` sets no replica routing) and cluster with default routing, but not for the replica-read modes this store exposes. The new tests cannot show this: they run against a single standalone server and are skipped without `RUN_REDIS_TESTS=1`.

**Direction:** make the destructive decision from the master. For example, confirm absence with a command that is not routed to replicas before the `ZREM`, and do the post-`ZREM` recheck the same way; or skip the cleanup when the client is configured for replica reads. The `ZREM` itself already goes to the master, so only the two existence checks need rerouting.

## Checked and not reported

- **Concurrent `InsertEntry` race (prior review threads on commit 1).** With reads on the master, commit 2 closes it. If the insert's `SET` lands before the `Exists`, the lister re-adds the member; if it lands after, the insert's own `ZAddNX` runs after the `ZRem` and adds it. Interleavings with two concurrent listers and with `DeleteEntry` leave at worst an orphan member, which the next listing removes. A concurrent listing can miss a recreated entry in the gap between `ZRem` and the restoring `ZAddNX`; that is transient.
- **`UpdateEntry` after cleanup.** `UpdateEntry` never re-adds a member, so a `SET` from an update that lands after a completed cleanup would leave a live value with no member. It needs the value key to vanish (Redis expiry or eviction) and a listing to run inside the few-millisecond gap between the filer's `FindEntry` and `Store.UpdateEntry`; and because `Filer.UpdateEntry` preserves `Crtime` (`filer.go:378`), an entry revived that way with an unchanged TTL is already logically expired and deleted on next access. Not actionable.
- **Context cancellation between `ZRem` and the restore.** Not reachable through the filer: `FilerStoreWrapper.ListDirectoryEntries` / `ListDirectoryPrefixedEntries` call the store with `context.WithoutCancel` (`weed/filer/filerstore_wrapper.go:324,341`).
- **`keyPrefix`, super-large directories, error identity.** `dirListKey` is already prefixed and the value key is rebuilt with `store.getKey`, matching `FindEntry`. Super-large directories have no index, so the new code is unreachable there. Only the bare `filer_pb.ErrNotFound` sentinel (from `redis.Nil`) reaches the new call; wrapped errors still break out of the loop.
- **Directory entries.** `CreateEntry` zeroes `TtlSec` for directories and `ensureParentDirectoryEntry` creates parents without a TTL, so directory value keys do not expire out from under their children.
- **Tests / devex.** `RUN_REDIS_TESTS` and `REDIS_ADDR` are new and referenced nowhere else in the tree; they only gate the new opt-in tests. `go test -count=1 -v ./weed/filer/redis2` builds and passes with all four test cases skipped (no Redis server is available in this environment, so the live-Redis assertions were not executed). `go vet ./weed/filer/redis2` is clean.
