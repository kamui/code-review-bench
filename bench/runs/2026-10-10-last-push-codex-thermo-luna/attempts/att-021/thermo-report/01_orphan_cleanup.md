# Orphan cleanup and concurrent recreation

## Scope and structure

The production diff adds a not-found call in `ListDirectoryEntries` and a dedicated `removeOrphanedDirectoryListMember` helper in `weed/filer/redis2/universal_redis_store.go`. The helper first removes the stale sorted-set member, checks whether the value key has appeared again, and restores the member if it has. That ordering addresses the insert ordering in this store: `InsertEntry` writes the value before `ZAddNX`, and `ZAddNX` may be a no-op when a listing has not yet removed the old index member.

This is a contained response to the cross-key race. The value and index keys may occupy different Redis cluster slots, so a transaction or Lua script over both is not a general atomic substitute. The helper is not an unnecessary layer: it gives the race repair a named boundary without spreading the extra commands through the listing loop. At 259 lines, the production file remains far below the skill's decomposition threshold, and the test file is a focused Redis integration test file.

## Finding: restoration failure is discarded

At `weed/filer/redis2/universal_redis_store.go:238-250`, the helper ignores the `ZRem` error by returning silently and also ignores the final `ZAddNX` error. Either failure can follow a state change that has already removed the member: Redis may have applied `ZRem` even if the client observes a transport error, or the restore `ZAddNX` may fail after the successful removal. If the following `Exists` sees a concurrent recreation, the helper's final operation is the only repair if the insert's `ZAddNX` was already issued while the old member still existed.

A concrete failure ordering is: an insert writes the value and runs `ZAddNX` while the old member is still present, so `NX` leaves the index unchanged; the listing then runs `ZRem`; its `Exists` sees the value; the helper's restoring `ZAddNX` fails transiently. The value is now live and the member is absent. Since the restore error is hidden and the listing continues, callers see success. A future `UpdateEntry` only rewrites the value, and a subsequent `InsertEntry` may likewise leave the missing membership unresolved if the member is again removed in the same race.

The helper should return errors from both state-changing commands and the caller should surface them from `ListDirectoryEntries`, wrapping them with the path and operation. That keeps the observable result honest: a listing that cannot complete its consistency repair does not silently claim success. Keep ignored failures for best-effort cleanup only where they cannot lose a live member. If the intended contract instead makes restoration retryable, implement a bounded retry or another explicit recovery mechanism and document its guarantee; silently discarding command results is not sufficient.

## Worked code-judo proposal

Keep the algorithm and its helper boundary, but make each state-changing command's outcome part of the helper contract. For example, change the helper to return `error`; return nil when `ZRem` succeeds and `Exists` confirms the key is absent; return wrapped errors for failed `ZRem`, `Exists`, or restoration `ZAddNX`; and have the not-found branch set `err` and break when repair fails. The error paths should include the affected path so operators can distinguish an incomplete cleanup from a failed entry read. This preserves the cluster-compatible separate-key operations while removing the hidden partial-success state.

The existing best-effort handling of cleanup in the logical-expiry branch is nearby but is not part of this finding: the specific restore write is materially different because it protects a live entry from being hidden after this helper has removed its index member.

## Verification status

The offline focused command `go test -count=1 ./weed/filer/redis2` completed successfully. The execution policy noted that no Redis server is available; the opt-in integration tests therefore did not exercise live key expiry, concurrent recreation, or injected Redis command failures. No files in the clone were edited.
