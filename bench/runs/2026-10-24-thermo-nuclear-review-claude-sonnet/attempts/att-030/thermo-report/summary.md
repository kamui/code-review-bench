# Thermo nuclear review: seaweedfs#10735 (redis2 orphaned index members)

No actionable findings.

## What I traced

- `ListDirectoryEntries` (`weed/filer/redis2/universal_redis_store.go:201-232`) calls the new `removeOrphanedDirectoryListMember` (`:237-251`) only when `FindEntry` returns the bare `filer_pb.ErrNotFound` sentinel. `FindEntry` returns that only for `redis.Nil`, so transient errors never reach the cleanup.
- `dirListKey` is already prefixed. The helper reuses it, and it rebuilds the value key with `store.getKey`. The `keyPrefix` handling is correct.
- Super-large directories have no index ZSET, so the loop body never runs for them and the cleanup is unreachable.
- Concurrent InsertEntry (the race raised in the earlier review threads): `InsertEntry` does SET and then `ZAddNX`.
  - If the insert's `ZAddNX` ran while the stale member was still present, it was a no-op. The listing's `ZREM` then removes the member, `EXISTS` sees the recreated value, and `ZAddNX` restores the member. This is handled.
  - If the insert's SET comes after the `ZREM`, its own `ZAddNX` also comes after the `ZREM`, so it re-adds the member.
  - If `EXISTS` itself errors, the code falls through to `ZAddNX`. That at worst leaves a harmless orphan, which a later listing removes.
- Concurrent DeleteEntry racing the restore: the listing's `ZREM` and `EXISTS` can run before the delete's `DEL`, and the restore's `ZAddNX` after the delete's `ZREM`. That recreates an orphan member. It is self-healing, because the next listing removes it through this same path. There is also a brief window between `ZREM` and `ZAddNX` where a concurrent listing may omit a live entry. Neither is worth reporting.
- The `ZREM` error is swallowed and the repair retried on the next listing, matching the existing expiry branch.
- The logical-expiry branch is unchanged. The TTL-clock reasoning in the PR body holds: `SET ... EX` and `Crtime` produce the not-found path.
- The tests gate on `RUN_REDIS_TESTS=1`, so no devex break. `go test ./weed/filer/redis2` offline passes, with the live-Redis tests skipped. I could not run them without a Redis server.
- The restore test calls the helper directly with a present value rather than exercising a real race. That is a coverage limitation, not a defect.
