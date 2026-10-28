# Thermo nuclear review: seaweedfs#10735 (redis2 orphaned directory index members)

No actionable findings.

## What I traced

- `ListDirectoryEntries` (`weed/filer/redis2/universal_redis_store.go:201-232`) calls the new `removeOrphanedDirectoryListMember` only when `err == filer_pb.ErrNotFound`. `FindEntry` returns that bare sentinel only for `redis.Nil` (lines 99-102). Every other error is wrapped, so a transient Redis failure still breaks out of the loop and never removes a member.
- The helper reuses the already-prefixed `dirListKey` and builds the value key with `store.getKey(string(path))`. That matches the prefixing in `InsertEntry`/`doInsertEntry`, so `keyPrefix` deployments are handled.
- Super-large directories have no index ZSET, so `ZRangeByLex` returns nothing and the helper is never reached.
- Race with a concurrent `InsertEntry` (value `SET`, then `ZADD NX`):
  - If the insert's `ZADD NX` lands before the listing's `ZREM`, it is a no-op against the existing member. The helper's `EXISTS` then sees the value and `ZADD NX` restores the member.
  - If the insert lands after the `ZREM`, the insert adds the member itself, and the restore is a harmless no-op.
  - If the value is still absent at the `EXISTS`, the member stays removed and any later insert re-adds it.
- If `EXISTS` errors, the code falls through to the restore. That is the conservative choice: it never loses a live entry's membership.
- Cluster mode: the `ZREM`, `EXISTS` and `ZADD` calls each touch one key, so there are no cross-slot operations.
- `UpdateEntry` does not touch the index, so no new interaction there.
- The tests are gated behind `RUN_REDIS_TESTS=1` and skip without a server. They are self-contained, use unique directories and clean up afterwards. I couldn't run them, because no Redis server is available.

## Residual notes (not reported as findings)

- If a concurrent `DeleteEntry` removes the value between the helper's `EXISTS` and its `ZADD NX`, a stale member is re-added. The next listing removes it through the same path, so it is self-healing and bounded.
- If the `ZADD NX` fails right after a successful `ZREM` while a racing insert has recreated the value, that entry would be missing from listings. This needs a narrow race plus a Redis error at that exact moment, and the error is silently discarded. It is not worth a priority rating.
