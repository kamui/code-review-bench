# Redis2 directory-index cleanup

## Finding: Unreported repair failures can permanently de-index a live entry

### Evidence

`ListDirectoryEntries` sees `filer_pb.ErrNotFound` and invokes `removeOrphanedDirectoryListMember`, then clears the listing error and continues. The helper at `weed/filer/redis2/universal_redis_store.go:237-250` removes the ZSET member first, checks the value key, and attempts `ZAddNX` when the value exists again (or the existence check itself errors).

Both sides of the repair can fail without reaching the caller. An error from `ZRem` causes an immediate return. Because a Redis response can be lost after the command was applied, that error does not establish that the index member remains. If a concurrent insert has recreated the value, the helper may then leave it unindexed. If `ZRem` succeeds but the value is present and the restoring `ZAddNX` fails, its result is discarded and the same inconsistency remains. The listing continues as though cleanup succeeded. Since subsequent listings enumerate index members before reading values, a live path whose member was lost is not visited again for lazy repair.

This is a narrow failure-path defect, not a broad maintainability regression. The production file has 259 lines after the change and gains 17 lines; the new test file has 143 lines. The helper centralizes the extra cleanup protocol, and the packet explains why one atomic operation across the value and index keys is unavailable to the cluster store. There is no convincing code-judo simplification that removes that cross-key coordination while retaining cleanup and recreation safety.

### Worked repair proposal

Keep the helper as the owner of this protocol, but give it an `error` result and make its states explicit. After attempting `ZRem`, continue to the existence check even if `ZRem` returned an error, because the removal may have been applied. If the value is absent and removal is known to have succeeded, cleanup is complete. If the value exists, or the existence check is indeterminate, attempt `ZAddNX`; check that result rather than dropping it. Return an error whenever the helper cannot establish either that the stale member was removed or that a live value's member was restored. `ListDirectoryEntries` should then return that repair error (or wrap it with the path) rather than silently claiming a complete listing.

A useful focused test seam is a small client interface or injected command function for the three Redis operations. Exercise: (1) `ZRem` applies but returns an error while the value is live, then verify the helper attempts restoration; (2) successful removal plus live value followed by restoration failure, then verify listing returns an error; and (3) a normal absent-value case, which still removes the orphan. Keep the real Redis integration tests for prefix and TTL behavior.

This reporting behavior cannot make a persistently unavailable Redis command succeed. It does prevent an uncertain partial repair from being silently represented as a successful listing and makes the failure actionable for retries or operational recovery.

### Verification status

- Inspected `git diff main...review-head` and the surrounding insert, update, delete, find, and list flows.
- `git diff --check main...review-head`: passed.
- Checkout status after review: clean.
- Go tests: not run. The execution packet states there is no Redis server; the added test cases require one.
