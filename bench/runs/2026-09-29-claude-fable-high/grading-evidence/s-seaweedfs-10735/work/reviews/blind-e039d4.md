# Review blind-e039d4

### Item 1
Location: weed/filer/redis2/universal_redis_store.go:245
Claim: With redis_cluster2 `useReadOnly`/`routeByLatency`, the GET in FindEntry and the EXISTS re-check are served by replicas, so replication lag makes a live entry look absent and the ZREM (sent to the master) permanently strips its index member.
Consequence: Cluster store with ReadOnly=true. InsertEntry does SET and ZADD on two masters (the keys are in different slots). The index shard's replica has the member, the value shard's replica has not replicated the SET yet. Listing: ZRANGEBYLEX returns the name, GET on the lagging replica returns nil -> ErrNotFound -> ZREM on master -> EXISTS on the same lagging replica returns 0 -> member stays removed. The live entry is invisible to ListDirectoryEntries and DeleteFolderChildren until another InsertEntry on that path. Before this change the same lag only caused a transient skip.
Fix: —

### Item 2
Location: weed/filer/redis2/universal_redis_store.go:246
Claim: The convergence argument only covers InsertEntry; a concurrent UpdateEntry (SET only, no ZAddNX) that lands after the EXISTS re-check leaves a live value with no index member.
Consequence: Filer.CreateEntry/UpdateEntry on a TTL entry: FindEntry returns the old entry just before Redis expires the key, so the filer takes the UpdateEntry path. The key expires, a listing sees ErrNotFound, does ZREM, EXISTS returns 0, returns. UpdateEntry then does SET. The value is live but absent from `<dir>\x00`, so it never lists and is skipped by DeleteFolderChildren. On main the member stayed in place and the updated entry remained listed.
Fix: —

### Item 3
Location: weed/filer/redis2/universal_redis_store.go:208
Claim: When the orphaned member is a directory, the cleanup removes it from the parent index but leaves its own child index key `<path>\x00` and descendant values, making the subtree unreachable and undeletable.
Consequence: The value key of directory /a/b is lost (maxmemory eviction or out-of-band DEL, both named in the PR) while /a/b\x00 still holds children. Listing /a removes `b` from /a\x00. A later recursive delete of /a runs DeleteFolderChildren(/a), which iterates members and no longer sees `b`, so /a/b\x00 (no TTL) and the children's value keys leak permanently. On main the member stayed, so DeleteFolderChildren still deleted /a/b and /a/b\x00. DeleteEntry deletes the dir-list key; this helper does not.
Fix: —

### Item 4
Location: weed/filer/redis2/universal_redis_store.go:250
Claim: The restoring ZAddNX result is discarded and nothing is logged, so when the re-check failed or the restore fails, the member is already removed and the claimed bias towards a stale member does not hold.
Consequence: ZREM succeeds, then the connection drops or a failover/timeout occurs. EXISTS returns an error, the code falls through to ZAddNX on the same broken client, which also fails, and the error is ignored. If the value had been recreated by a concurrent InsertEntry (whose ZAddNX was a no-op), the live entry silently loses its index member with no log line. The destructive step runs first and the compensating step is unchecked.
Fix: —

### Item 5
Location: weed/filer/redis2/universal_redis_store.go:250
Claim: The check-then-restore is itself racy: a DeleteEntry completing between EXISTS and ZAddNX causes the helper to re-add a member for a deleted entry, and can recreate the index key of a deleted directory.
Consequence: Lister: ZREM, EXISTS returns 1 (value recreated). Concurrently a recursive delete removes the file (DEL value, ZREM member) and then the parent directory (DEL `<dir>\x00`). The lister's ZAddNX then recreates `<dir>\x00` with the stale name. The directory no longer exists, so it is never listed and the key leaks; if the directory is recreated later it starts with a phantom member that costs a GET and a repair.
Fix: —

### Item 6
Location: weed/filer/redis2/universal_redis_store.go:208
Claim: Repair runs synchronously inside the listing loop as separate round trips per orphan (GET + ZREM + EXISTS), tripling latency of the listings that hit the backlog this PR describes.
Consequence: A TTL directory that accumulated 1M dead members: each listing page of `limit` names now issues 3*limit sequential Redis commands instead of limit, on the request path, and still emits one V(0) log line per orphan. Collecting the orphans of a page and issuing one variadic ZREM plus a pipelined EXISTS (same slot for the ZSET key) would cost one or two extra round trips per page.
Fix: —

### Item 7
Location: weed/filer/redis2/universal_redis_store_test.go:19
Claim: All new tests skip unless RUN_REDIS_TESTS=1, and no workflow sets it or starts a Redis, so the change has no automated coverage in CI.
Consequence: `go test ./weed/filer/redis2` reports PASS with every test skipped (confirmed locally: all four cases SKIP). A future regression such as rebuilding the key without keyPrefix or dropping the restore would merge green.
Fix: —

### Item 8
Location: weed/filer/redis2/universal_redis_store_test.go:105
Claim: TestRemoveOrphanedDirectoryListMemberKeepsRecreatedEntry only checks ZREM followed by ZAddNX with the value present; it does not cover the absent-value, errored re-check, or UpdateEntry interleavings the helper's correctness depends on.
Consequence: An implementation that skips the ZREM whenever the value exists, or one that restores unconditionally, both pass this test. The error branches at lines 238 and 246 of the store and the SET-only writer race are untested, so the concurrency guarantee stated in the PR is not pinned by any test.
Fix: —

### Item 9
Location: weed/filer/redis2/universal_redis_store.go:237
Claim: Altitude: the fix patches the symptom in the read path instead of the cause (Redis EX deadline diverging from the filer's Crtime+TtlSec), and the store drops expired entries without the filer ever seeing them.
Consequence: Because redis2 removes Redis-expired entries silently, Filer.doListDirectoryEntries never counts them as expired, so DeleteEmptyParentDirectories (filer.go:504) never runs and empty TTL directories accumulate. Listing becomes a writer with non-atomic compensation. Setting the physical TTL as a backstop (EX TtlSec+grace) would let the existing logical-expiry path do the two-part delete.
Fix: —

### Item 10
Location: weed/filer/redis/universal_redis_store.go:184
Claim: The sibling redis (v1) store has the identical not-found branch that leaves the member in the directory SET, and it is not fixed or shared with the new helper.
Consequence: Deployments on the `redis` / `redis_cluster` stores keep growing the per-directory SET under TTL workloads, with one GET and one V(0) log line per dead name on every listing. The repair logic is duplicated per store instead of shared, so the fix must be re-implemented for each store.
Fix: —
