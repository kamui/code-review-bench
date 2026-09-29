# 01 — redis2 store production change

Files: `weed/filer/redis2/universal_redis_store.go`. Command used: `git diff main...review-head -- weed/filer/redis2/universal_redis_store.go`; `wc -l weed/filer/redis2/*.go` (store file 259 lines after the change, +17). Verification: static reading only; no Redis server was available and I did not execute the tests.

## Measurements

The diff adds one call at line 208 and a 16-line helper at lines 237-252. `ListDirectoryEntries` (lines 176-235) now has three exits inside its per-member `if err != nil` block: not-found (repair, continue), other error (break), and, in the `else` arm, logical expiry (Del+ZRem, continue). The helper issues up to three commands per orphan: ZREM, EXISTS, and (if the value exists or the check failed) ZAddNX.

## Finding 1 — two reaping policies (verified by reading)

Lines 205-211 versus 214-219. The not-found branch uses the guarded protocol; the expiry branch discards both results and has no guard. The PR body concedes the expiry branch loses the freshly written value on a concurrent insert. Verified that both branches are reachable from the same loop iteration type and share `dirListKey`.

Worked code-judo proposal: replace both bodies with one helper.

```go
// reapDeadMember removes a listed child whose value is gone or logically expired.
func (store *UniversalRedis2Store) reapDeadMember(ctx context.Context, dirListKey string, path util.FullPath, fileName string, expired bool) error
```

The loop becomes: on ErrNotFound call reap(expired=false) and continue; on decoded entry with `store.isLogicallyExpired(entry)` call reap(expired=true) and continue. The `expired` case can use a compare-and-delete on the value later without touching the loop. Branch count in the loop is unchanged but the policy lives in one place and the known race has a single home.

## Finding 2 — swallowed errors and non-atomic sequence (verified by reading)

Lines 237-252. ZREM failure returns silently; the ZAddNX result is unused; no log. The glog line at 206 runs before the repair so its text ("list path: not found") does not describe what happened next. Residual interleaving: EXISTS returns 1, a `DeleteEntry` runs DEL and ZREM, then our ZAddNX re-adds an orphan member. Self-heals on next listing, so benign, but it is not mentioned in the comment. `path` and `dirListKey` are both derivable from `(dirPath, fileName)`.

Alternative proposal (deletes the restore step): make `UpdateEntry` call the same member-add as `InsertEntry` (a shared `ensureIndexed(dir, name)` guarded by `isSuperLargeDirectory`). Then a member wrongly removed by any cleanup is re-established on the next update of that path, which removes the reason for the EXISTS/ZAddNX pair. Cost: one extra ZAddNX per update. Trade-off to check: update-heavy workloads on large directories; ZAddNX on an existing member is O(log N).

## Finding 3 — accidental design (verified by reading, plus the PR body's own timing argument)

Line 86 sets the physical expiry with `time.Duration(entry.TtlSec)*time.Second`; line 215 compares against `Crtime` plus TtlSec. The listed helper is the only reaper that actually runs for never-updated TTL entries. The follow-up (physical TTL plus grace) would restore the intended logical path. Recommendation: file it, and add a one-line comment at the `Set` call pointing at it.
