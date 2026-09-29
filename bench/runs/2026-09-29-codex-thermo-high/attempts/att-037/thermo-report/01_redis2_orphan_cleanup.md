# Redis 2 orphan cleanup

## Scope and measurements

The production change adds one helper and one call in `weed/filer/redis2/universal_redis_store.go`. The file is 259 lines after the change (the new helper is at lines 237–251); the added test file is 143 lines. Neither approaches the skill’s 1,000-line threshold. The listing loop has one new call in the existing `ErrNotFound` branch and no new nested branch there.

The helper implements the intended best-effort repair sequence: remove the orphan member, check whether the value reappeared, and restore membership unless the value was positively confirmed absent. The packet explains why a cross-key atomic script or transaction is unavailable for Redis Cluster’s different hash slots. Given that boundary, I do not see a credible restructuring that removes the check-and-restore steps while keeping the concurrent-recreation guarantee. Keeping this sequence in a focused helper is clearer than expanding the already-busy listing loop.

## Finding — failed restore is silently discarded

In `weed/filer/redis2/universal_redis_store.go:250`, `removeOrphanedDirectoryListMember` invokes `ZAddNX` without inspecting its result. The helper has already successfully removed the member at line 238. When `Exists` returns a positive result, that final command is the only repair for the interleaving where `InsertEntry` wrote the value and its `ZAddNX` ran before the cleanup’s `ZRem`. If the helper’s restoring `ZAddNX` fails, the live value has no index member. The original listing continues after the helper call and returns success, and later listings cannot rediscover the path because they enumerate index members. The error is lost, so operators and callers have no signal that membership repair failed.

### Worked remediation

Have `removeOrphanedDirectoryListMember` return an error for the final `ZAddNX` failure and handle that error at the listing call site instead of unconditionally continuing. Choose and document a recovery contract: for example, return a contextual listing error and retry the restore within a bounded policy, or use a retry mechanism that keeps the incomplete repair observable. Preserve the existing conservative choice when `Exists` itself errors: attempt to add the member back, since a stale member is safer than losing a live entry. Do not silently convert a failed restoration into successful listing completion.

This keeps the Redis Cluster-compatible single-key commands and the current helper boundary; it makes the failure visible without pretending the three-command sequence is atomic. A test should inject a `ZAddNX` error after a successful `ZRem` and positive `Exists`, then assert the selected error/retry contract. The current direct helper test exercises the positive-exists repair path only when every Redis operation succeeds.

## Verification

Command run from the clone root with the execution policy’s offline Go environment:

```text
go test -count=1 ./weed/filer/redis2
```

Result: package passed (`ok`, 0.019s). The packet states that no Redis server is available, and the added integration tests skip unless `RUN_REDIS_TESTS=1`; this run therefore does not verify their live Redis assertions or the failure interleaving above. No edits were made to the checkout. No child or alternate-model review was performed, as directed by the task packet.
