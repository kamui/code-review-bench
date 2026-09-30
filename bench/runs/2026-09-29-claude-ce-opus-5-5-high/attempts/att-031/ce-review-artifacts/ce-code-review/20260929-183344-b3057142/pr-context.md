Title: fix(redis2): remove orphaned directory index members on listing
URL: https://github.com/seaweedfs/seaweedfs/pull/10735

Body:
# What problem are we solving?

The `redis2` per-directory child index — the ZSET at `<dir>\x00` — accumulates members whose value key no longer exists, and nothing ever removes them.

`ListDirectoryEntries` reads the members, then calls `FindEntry` per member (`weed/filer/redis2/universal_redis_store.go:189-231`). Two outcomes matter:

- the value key is gone → `filer_pb.ErrNotFound` → the loop logs and `continue`s, **leaving the member in the index** (`:205-211` before this change);
- the value key is there but the entry is logically expired (`Crtime + TtlSec < now`) → `DEL` the value **and `ZREM` the member** (`:213-219`).

The second branch is the intended behaviour — the code already means to keep the index consistent. The first branch is the one that actually runs for TTL entries, because the two expiry deadlines are on different clocks:

- `doInsertEntry` writes `SET <key> <value> EX <TtlSec>` (`:86`), so Redis drops the key at `set_time + TtlSec`;
- the filer's logical check is `Crtime + TtlSec < now`.

`Crtime` is stamped before the store write, so `Crtime <= set_time` and the logical deadline is the *earlier* of the two — by the sub-millisecond gap between stamping the attribute and Redis processing the `SET`. The `else` branch can therefore only run if a listing lands inside that gap. Every listing after it finds `GET` returning `redis.Nil` and takes the not-found branch, the one that leaves the member behind. (An entry that is later *updated* re-arms the Redis TTL from the update time while `Crtime` stays put, which is the only case where the working branch is reliably reachable — so the leak is specific to entries created once and never touched again, which is the common TTL case.)

Under a TTL workload the index therefore grows without bound, and two things get worse with it:

- `LIST` costs one `GET` round trip per name *ever created* in that directory, not per live entry;
- the skip logs at `glog.V(0)`, which is on by default, so every listing emits one log line per dead member.

The same orphan is produced by any other route that removes a value key without the matching `ZREM` — an out-of-band `DEL`, `maxmemory` eviction, or a `DeleteEntry` that fails between its `DEL` and its `ZREM` (`:119-143`). TTL is just the one that does it continuously.

# How are we solving the problem?

`ZREM` the member in the not-found branch, mirroring what the expiry branch already does below and against the same `dirListKey` variable — guarded against a concurrent recreate, per the review point below.

Things I checked rather than assumed:

**A member cannot legitimately outlive a missing value.** `InsertEntry` writes the value first and adds the member second (`:56-74`): `doInsertEntry`'s `SET` has returned before `ZAddNX` is issued. So "member present" implies "value was written at some point", and a member returned by `ZRangeByLex` whose value is absent has had that value deleted or expired — it is never an entry mid-creation. The opposite window (value written, member not yet added) does exist, but a listing cannot observe it, because it only iterates members.

**The value can, however, come back before the `ZREM` lands** — which the paragraph above does not cover, and which both review bots caught. If an `InsertEntry` for the same path completes between `FindEntry` returning `ErrNotFound` and the `ZREM`, its `ZAddNX` is a no-op, because the member is still in place; the `ZREM` then strips the index member off a live value. `UpdateEntry` only calls `doInsertEntry` (`:92-95`) and never re-adds the member, so the entry stays invisible to `ListDirectoryEntries` and `DeleteFolderChildren` until another `InsertEntry` hits that exact path. That is a real regression, so the cleanup compensates rather than removing unconditionally: `ZREM`, re-check the value key, and `ZAddNX` the member back when the value is present again or when the check itself failed.

That converges in both interleavings. If the insert's `SET` lands before the re-check, the re-check sees it and restores the member; if it lands after, the insert's own `ZAddNX` is necessarily after our `ZREM` and restores the member itself. Only a positively confirmed absent value leaves the member removed, so the repair still converges for genuine orphans at the cost of one extra `EXISTS` — on the repair path only, which is one-time per orphan.

The remedy both reviews suggested — a Lua script or `MULTI` spanning the value key and the index key — is not available here. `RedisCluster2Store` embeds `UniversalRedis2Store`, and the two keys (`<prefix></dir/name>` and `<prefix></dir>\x00`) carry no hash tag, so a multi-key script would be `CROSSSLOT` under `redis_cluster2`. The compensating form uses only single-key commands and behaves identically on all three transports.

**`keyPrefix`.** `dirListKey` is `store.getKey(genDirectoryListKey(...))` at `:178` — already prefixed — and the new `ZREM` reuses that variable instead of rebuilding the key. Rebuilding it unprefixed would have been a silent no-op for every deployment that sets `keyPrefix`, which is why the test runs both with and without one.

**Transient errors cannot trigger it.** `FindEntry` returns the bare `filer_pb.ErrNotFound` sentinel only for `redis.Nil` (`:99-102`); every other failure is wrapped with `fmt.Errorf`. The branch guard is `err == filer_pb.ErrNotFound` — identity, not `errors.Is` — so a connection blip cannot match it and still breaks out of the loop as before.

**Super-large directories.** `isSuperLargeDirectory` makes `InsertEntry` skip the `ZAdd` and `DeleteEntry` skip the `ZRem`, so the index key does not exist for those directories at all. `ZRangeByLex` returns nothing, the loop body never runs, and the new call is unreachable. The feature is unaffected.

Cleanup failures still do not fail the listing — a failed repair is simply retried on the next one — but the errors are no longer discarded outright, since the restore decision depends on them: a `ZREM` that errors removes nothing and returns, and a re-check that errors restores the member, so the failure modes bias towards a stale member rather than a lost one.

**Deliberately not included:** making the physical TTL a backstop behind the logical one (`SET ... EX TtlSec + grace`) so the logical branch wins the race and performs the clean two-part delete. That changes when data actually disappears from Redis for every `redis2` user, and it would not remove the need for this fix, since the non-TTL routes above produce the same orphan. Happy to raise it separately if you want it.

**Also deliberately not included:** the expiry branch at `:214-219` has a pre-existing instance of the same race, with a worse outcome — a concurrent `InsertEntry` landing between `FindEntry` and the `DEL` loses the freshly written value, not just the index member. Closing it needs a compare-and-delete on the value rather than the compensating restore used here, so it is a separate change and not one I want to smuggle into this one.

# How is the PR tested?

New `weed/filer/redis2/universal_redis_store_test.go`. It needs a live Redis, so it follows the existing convention for store tests that need a server — `tarantool_store_test.go` gating on `RUN_TARANTOOL_TESTS=1`, `foundationdb_store_test.go` skipping when the cluster is absent — and skips unless `RUN_REDIS_TESTS=1`, with `REDIS_ADDR` defaulting to `127.0.0.1:6379`. Each test works inside a uniquely named directory and cleans up after itself, so it is safe against a shared instance.

I did try the in-tree `tempredis` helper used by `redis3/kv_directory_children_test.go` first. It hangs on any current `redis-server`: it blocks waiting for the literal line `The server is now ready to accept connections`, which Redis stopped printing years ago. That helper is only reachable from a `Benchmark` today, so nothing noticed. Worth knowing if anyone tries to build on it.

- `TestListDirectoryEntriesRemovesOrphanedIndexMembers` — two entries, one's value key dropped; asserts the listing returns only the live entry *and* that the index is left holding only the live name. Run with `keyPrefix` empty and set.
- `TestListDirectoryEntriesRemovesIndexMembersExpiredByRedis` — inserts with `TtlSec: 1`, waits past it, asserts Redis has already dropped the value key, then asserts the listing empties the index. This is the reported path end to end.
- `TestRemoveOrphanedDirectoryListMemberKeepsRecreatedEntry` — added for the review point above. It drives the cleanup with the value key present, which is exactly the state a concurrent recreate leaves behind, and asserts the member survives and the entry still lists. Asserted at the helper rather than through `ListDirectoryEntries`, because the window between the `GET` and the `ZREM` cannot be hit deterministically from outside. It fails if the restore is removed.

Against `redis-server 8.2.1`, the first three cases fail on master and pass with the change.

`go build ./weed/...`, `go vet ./weed/filer/redis2/`, `gofmt` all clean, and `go test ./weed/filer/redis2/... -race` passes against a live Redis.

## Note on `redis3`, which I am not touching

`redis3` has the same shape at `universal_redis_store.go:140-166`, and is worse off: its not-found branch removes nothing at all, and its expiry branch calls `ZRem` on `<dir>\x00` — which in `redis3` is a **string** holding the serialised skiplist root, not a ZSET. Flagging rather than fixing, because `weed/filer/redis3/README.md` reads `Desuppported.`

## Related

#10736 proposes giving lazily-created remote-mount entries a TTL so the filer's existing expiry machinery reclaims them.
