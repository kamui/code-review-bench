# Redis2 directory-index cleanup

## Finding: the recreation repair misses `UpdateEntry`

The new orphan cleanup is called only after `FindEntry` returns `ErrNotFound` in `ListDirectoryEntries` ([`universal_redis_store.go:205-210`](../../clone/weed/filer/redis2/universal_redis_store.go)). `removeOrphanedDirectoryListMember` issues `ZREM`, then `EXISTS`, returns if the value is absent, and otherwise runs `ZAddNX` ([`universal_redis_store.go:237-250`](../../clone/weed/filer/redis2/universal_redis_store.go)). This is an attempt to repair the race in which `InsertEntry` writes the value after the removal: `InsertEntry` writes the value and then adds the index member ([`universal_redis_store.go:56-73`](../../clone/weed/filer/redis2/universal_redis_store.go)).

The check is not sufficient for every supported value writer. `UpdateEntry` invokes only `doInsertEntry` and does not add the parent member ([`universal_redis_store.go:92-95`](../../clone/weed/filer/redis2/universal_redis_store.go)). In this interleaving, listing observes a missing value; cleanup removes the member; cleanup's `EXISTS` returns zero; then an `UpdateEntry` writes the value. The helper has already returned, and the update has no `ZAddNX`, so the live value remains absent from directory listings. This is the same lost-index outcome the repair is intended to prevent, through an existing store API rather than an exotic Redis failure. The subsequent `UpdateEntry` call need not coincide with the helper: any update that rewrites a missing or concurrently expired key during that interval has the same result.

### Worked code-judo proposal

Move index maintenance to the common successful value-write boundary, or make both public write methods share one helper that writes the value and then ensures the parent index membership for ordinary directories. `InsertEntry` already has the desired ordering; `UpdateEntry` can adopt the same membership step, preserving `isSuperLargeDirectory` semantics. With every successful recreation responsible for leaving the index present, the post-`EXISTS` restore remains useful for an insert whose `ZAddNX` ran while the old member still existed, while update recreation no longer falls through the gap. This makes the invariant local to all writers instead of relying on the read-repair helper to predict which writer may race it.

Add a Redis-backed case that creates the member, removes the value, then coordinates an update write after cleanup has observed absence; assert both that the value exists and that the member is indexed. The current `TestRemoveOrphanedDirectoryListMemberKeepsRecreatedEntry` starts with the value already present and calls the helper directly, so it verifies the restoration branch but not this absent-check interleaving or `UpdateEntry` behavior.

## Verification and measurements

The reviewed source is the committed range `main...review-head`. The production file is 259 lines after the change (the patch adds 17 lines); it does not approach the skill's 1,000-line decomposition threshold. The implementation adds a focused helper rather than growing the listing branch with the recheck steps, and it reuses the already-prefixed `dirListKey`. Those are good local structure choices, but they do not remove the writer-invariant gap described above.

`git diff --check main...review-head` completed cleanly, and `git status --short` showed no working-tree changes. No tests were run. The packet says a Redis server is unavailable, and the static issue follows directly from the existing call paths at `UpdateEntry` and the new cleanup helper.
