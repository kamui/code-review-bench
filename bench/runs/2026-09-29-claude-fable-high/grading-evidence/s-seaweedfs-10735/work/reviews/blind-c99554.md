# Review blind-c99554

### Item 1
Location: weed/filer/redis2/universal_redis_store.go:245
Claim: With `redis_cluster2` `useReadOnly`/`routeByLatency`, both the `GET` in `FindEntry` and the new `EXISTS` re-check are routed to replicas, so a lagging replica makes the cleanup permanently `ZREM` the index member of a live entry.
Consequence: Cluster store with useReadOnly=true. An entry is inserted or recreated: SET lands on master A, ZADDNX on master B (no-op if an old orphan member is still there). A listing reads the member, then GET goes to a replica of A that has not replicated the SET yet and returns nil -> ErrNotFound. ZREM goes to master B and removes the member; EXISTS is read-only, hits the same lagging replica, returns 0, so nothing is restored. The live entry is now missing from ListDirectoryEntries and DeleteFolderChildren until another InsertEntry on that exact path. Before this change the same stale read only skipped the entry for one listing.
Fix: —

### Item 2
Location: weed/filer/redis2/universal_redis_store.go:238
Claim: The compensation reuses the request ctx and treats any `ZREM` error as 'nothing removed', so cancellation or a read timeout between the steps leaves the member removed from a live value with no restore.
Consequence: A concurrent InsertEntry recreates the path (its ZADDNX is a no-op because the orphan member is still present). The listing's ZREM succeeds, then the client disconnects or the deadline fires: EXISTS returns a ctx error, the code falls through to ZAddNX with the same dead ctx, which also fails and whose error is discarded. Alternatively ZREM is applied server-side but the client sees a read timeout and returns early without the re-check. Either way the live entry stays unlisted; the PR's claim that failures bias toward a stale member does not hold for these errors.
Fix: —

### Item 3
Location: weed/filer/redis2/universal_redis_store.go:250
Claim: The remove-then-restore sequence is not atomic, so a live recreated entry is briefly absent from the index, and the late `ZAddNX` can resurrect a member or a whole `<dir>\x00` ZSET after a concurrent delete.
Consequence: Lister: GET nil; inserter recreates the value (ZADDNX no-op); lister: ZREM, EXISTS=1. In that window a recursive delete of the directory runs DeleteFolderChildren, which iterates only index members, misses the child and leaves its value key in Redis; DeleteEntry(dir) then deletes `<dir>\x00`. The lister's ZAddNX now recreates `<dir>\x00` with one member for a deleted directory, so a later recreation of that directory shows a ghost child. A concurrent listing in the same window also omits a live entry.
Fix: —

### Item 4
Location: weed/filer/redis2/universal_redis_store.go:208
Claim: Removing the member for an orphan that is a directory (value key evicted or deleted out of band) detaches its `<dir>/sub\x00` child index and descendants from the only structure `DeleteFolderChildren` uses to reach them.
Consequence: maxmemory allkeys-* eviction drops the value key of directory /a/sub while /a/sub\x00 and its children survive. Listing /a now ZREMs 'sub' from /a\x00. A later recursive delete of /a calls DeleteFolderChildren(/a), which no longer sees 'sub' and so never deletes /a/sub\x00 (previously it did, line 166). The child index and values leak, and when /a/sub is created again the old children reappear in its listing.
Fix: —

### Item 5
Location: weed/filer/redis2/universal_redis_store.go:216
Claim: The logical-expiry branch in the same loop still does an unguarded `DEL` plus `ZREM`, so the race this PR guards against in the not-found branch remains there with a worse outcome (data loss).
Consequence: Listing reads an entry whose Crtime+TtlSec has passed (reachable whenever the entry was updated, which re-arms the Redis TTL). Between FindEntry and line 216 a concurrent InsertEntry writes a fresh value for the same path. The listing then DELs the new value and ZREMs the member, silently destroying a just-written entry. The new helper fixes only one of the two cleanup paths in the function.
Fix: —

### Item 6
Location: weed/filer/redis2/universal_redis_store_test.go:19
Claim: All new tests skip unless `RUN_REDIS_TESTS=1` with a live Redis, and no workflow sets it, so the change ships with no coverage that runs in CI.
Consequence: `go test ./weed/filer/redis2` reports PASS with every test skipped (confirmed locally: 3 SKIP). A regression such as dropping the restore or building the index key without the prefix merges green. Nothing in .github/workflows references redis, so the gate is never enabled.
Fix: —

### Item 7
Location: weed/filer/redis2/universal_redis_store_test.go:105
Claim: The recreate test calls the helper with the value already present and only with an empty `keyPrefix`, so it exercises neither the actual interleaving nor the prefixed `EXISTS` key.
Consequence: If the `EXISTS` at line 245 were changed to use `string(path)` without `store.getKey`, every deployment with keyPrefix set would see EXISTS=0 and lose live members under a concurrent recreate, yet this test still passes because it runs only with keyPrefix "". It also cannot detect ordering bugs such as the re-check running before the ZREM.
Fix: —

### Item 8
Location: weed/filer/redis2/universal_redis_store.go:206
Claim: Each orphan now costs up to four sequential round trips (GET, ZREM, EXISTS, optional ZADDNX) inside the listing loop, still with a `glog.V(0)` line per orphan, instead of batching the cleanup.
Consequence: A directory with a large backlog of TTL orphans (the case the PR targets) is listed with limit 1000: the request does about 3000 serial Redis calls and 1000 default-level log lines and may return zero entries for the page. Collecting orphan names and issuing one variadic ZREM plus a pipelined EXISTS after the loop would cut this to a couple of round trips.
Fix: —

### Item 9
Location: weed/filer/redis/universal_redis_store.go:184
Claim: The sibling `redis` store has the identical not-found branch that leaves the member in the directory set, and the fix was applied only to `redis2`.
Consequence: A deployment on the `redis` store with TTL entries: values expire via SET EX, the listing hits ErrNotFound and `continue`s without removing the member, so the directory set grows without bound with one GET and one V(0) log line per dead name on every listing. This is the same defect the PR describes, left in place; a shared cleanup helper would cover both stores.
Fix: —
