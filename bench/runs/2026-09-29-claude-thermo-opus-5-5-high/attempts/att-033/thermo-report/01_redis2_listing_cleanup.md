# 01 — redis2 listing cleanup (`weed/filer/redis2/universal_redis_store.go`)

Scope: the one production file touched by `main...review-head`
(`db5a086d0..6c8fde664`), +17 / −0. The file is 259 lines at head (`wc -l`), so there
is no file-size concern.

## What the diff does

`ListDirectoryEntries` (`universal_redis_store.go:176-235`) iterates the members of the
per-directory ZSET `<prefix><dir>\x00` and calls `FindEntry` for each one. Before the PR,
a member whose value key was gone (`filer_pb.ErrNotFound`) was logged and skipped, and the
member stayed in the index for good. The PR adds one call at `:208` into a new helper,
`removeOrphanedDirectoryListMember` (`:237-251`). The helper runs `ZREM`, then `EXISTS` on
the value key, and runs `ZADD NX` to put the member back if the value is present again or
if the `EXISTS` check itself failed.

## Correctness check of the compensating protocol (verified by reasoning against the code)

I walked through the interleavings against `InsertEntry` (`:56-74`: `SET` first, then
`ZADD NX`), `UpdateEntry` (`:92-95`: `SET` only) and `DeleteEntry` (`:119-143`: `DEL`
value, then `ZREM`):

- Insert `SET` lands between `FindEntry` and the helper's `ZREM`, and the insert's `ZADD NX`
  runs before the `ZREM`. The `ZADD NX` is a no-op, the `ZREM` removes the member, `EXISTS`
  returns 1, and the member is restored. Correct.
- Insert `SET` lands before `EXISTS`, and its `ZADD NX` runs after the `ZREM`. Both paths
  re-add the member, and `NX` makes the second add harmless. Correct.
- Insert `SET` lands after `EXISTS`. Its `ZADD NX` necessarily runs after our `ZREM`, so
  the member is re-added. Correct.
- The value is recreated, we restore the member, and then a concurrent `DeleteEntry`
  finishes its `DEL` and `ZREM` before our `ZADD NX`. The member comes back as an orphan.
  That is benign: the next listing cleans it up. This is the "bias towards a stale
  member" the PR body claims.
- Two listers race on the same orphan. Both `ZREM`, both see `EXISTS == 0`, and the end
  state is correct.

Cluster: every command in the helper touches a single key, so `redis_cluster2` sees no
`CROSSSLOT` error. The PR body is right that a Lua script or `MULTI` across the value key
and the index key is not an option here. `WATCH` + `MULTI` hits the same problem.

I found no correctness regression. The findings below are about structure.

---

## Finding 1.1 — The loop now carries two different "remove a dead child" protocols eight lines apart; normalize expiry into not-found so there is one

**Evidence.** At head, `ListDirectoryEntries` handles a dead child in two places:

```go
// :207-211  value key gone
if err == filer_pb.ErrNotFound {
    store.removeOrphanedDirectoryListMember(ctx, dirListKey, path, fileName)
    err = nil
    continue
}
...
// :214-220  value present but logically expired
if entry.TtlSec > 0 {
    if entry.Attr.Crtime.Add(time.Duration(entry.TtlSec) * time.Second).Before(time.Now()) {
        store.Client.Del(ctx, store.getKey(string(path))).Result()
        store.Client.ZRem(ctx, dirListKey, fileName).Result()
        continue
    }
}
```

Both branches reach the same conclusion: "this member no longer names a live entry, drop
it from the index." The PR makes them implement that conclusion differently. The first
branch uses the new guarded protocol (`ZREM` → `EXISTS` → maybe `ZADD NX`). The second
keeps the unguarded, results-discarded `ZREM`. So after this PR a reader has to hold two
cleanup idioms in mind and work out why they differ. They should not differ: the
logical-expiry branch has exactly the recreate race the second commit was written to
close. An `InsertEntry` on the same path can land between that branch's `DEL` and `ZREM`.
The PR body calls the `DEL` half of that race out of scope ("needs a compare-and-delete"),
but the index-member half is the same bug the helper already fixes, and the fix is one
call away.

The nesting also got deeper, not flatter. The loop body is now
`if err != nil { log; if notFound { cleanup; continue }; break } else { if ttl { if expired { cleanup; continue } } ... }`,
which puts three levels of conditionals around what is really a three-way
classification: live / dead / error.

**Code-judo proposal (behavior-preserving on the non-concurrent path).** Turn logical
expiry into "not found" right after the lookup. Then there is exactly one dead-child exit,
and the `else` branch disappears:

```go
for _, fileName := range members {
    path := util.NewFullPath(string(dirPath), fileName)
    lastFileName = fileName
    entry, err = store.FindEntry(ctx, path)
    if err == nil && entry.TtlSec > 0 &&
        entry.Attr.Crtime.Add(time.Duration(entry.TtlSec)*time.Second).Before(time.Now()) {
        store.Client.Del(ctx, store.getKey(string(path)))
        err = filer_pb.ErrNotFound
    }
    if err == filer_pb.ErrNotFound {
        store.removeOrphanedDirectoryListMember(ctx, dirPath, fileName)
        err = nil
        continue
    }
    if err != nil {
        glog.V(0).InfofCtx(ctx, "list %s : %v", path, err)
        break
    }

    resEachEntryFunc, resEachEntryFuncErr := eachEntryFunc(entry)
    if resEachEntryFuncErr != nil {
        err = fmt.Errorf("failed to process eachEntryFunc: %w", resEachEntryFuncErr)
        break
    }
    if !resEachEntryFunc {
        break
    }
}
```

Behavior deltas, all in the safe direction or deliberate:

- Expired entries now go through the guarded member removal. Without concurrency the
  result is identical: `DEL` has just run, so `EXISTS` returns 0 and the member stays
  removed. Under a concurrent recreate, the index member is restored instead of lost.
- The `V(0)` log on not-found is gone. The PR body itself names that log as noise ("one
  log line per dead member"). If the log should stay, move it into the helper at
  `V(1)`. Don't keep it in the loop.
- `lastFileName` is still assigned for every member, as before.

Result: one dead-child concept, one cleanup call site, the `else` ladder deleted, and the
pre-existing results-discarded `ZRem` at `:217` removed from the file.

**Context (not a finding).** The filer already owns TTL expiry. `Filer.FindEntry`
(`weed/filer/filer.go:434`) and `doListDirectoryEntries` (`weed/filer/filer.go:473`)
apply the same `Crtime + TtlSec` check and delete through `Store.DeleteOneEntry`. The
store-level expiry branch is a second copy of that policy. Whether it can be deleted
outright is a larger question (see Question Q1 in the summary). The normalization above
does not depend on the answer.

**Verification status:** verified by reading the head source. The proposed loop compiles
in my head against the existing types (`entry.Attr.Crtime`, `entry.TtlSec`,
`filer_pb.ErrNotFound`). It was not built, because the clone is read-only.

---

## Finding 1.2 — The helper's signature mixes a pre-prefixed key with unprefixed paths and passes the same identity three times

**Evidence.** `universal_redis_store.go:237`:

```go
func (store *UniversalRedis2Store) removeOrphanedDirectoryListMember(ctx context.Context, dirListKey string, path util.FullPath, fileName string)
```

- `dirListKey` must already carry `keyPrefix`: it is the result of
  `store.getKey(genDirectoryListKey(...))`.
- `path` must **not** carry `keyPrefix`: the helper calls `store.getKey(string(path))` on it
  at `:245`.
- `fileName` is the last component of `path`, and `path` is itself
  `NewFullPath(dirPath, fileName)`. The three arguments encode one identity, the pair
  `(dirPath, fileName)`, in three shapes, and two of those shapes sit in different key
  namespaces.

The PR body defends passing the prefixed `dirListKey` as protection against a silent
no-op under `keyPrefix`. But the protection comes from computing keys through `getKey`,
not from where that happens. A mixed contract like this is exactly how the no-op bug
gets written: a future caller who builds `dirListKey` without `getKey`, or passes a
prefixed `path`, compiles fine and silently operates on the wrong key. The test already
has to reconstruct the prefixed key by hand to call the helper
(`universal_redis_store_test.go:112`:
`store.removeOrphanedDirectoryListMember(ctx, store.getKey(genDirectoryListKey(string(dir))), path, "recreated")`).

This is also the sixth site in the file that spells out
`store.getKey(genDirectoryListKey(...))` (`:68, :121, :136, :151, :166, :178`), and the
second that spells out `redis.Z{Score: 0, Member: name}` (`:68, :250`). The directory
index has no owner in this file. Every operation re-derives its key inline, and this PR
adds one more derivation instead of giving the index an owner.

**Remedy.** Give the helper one unprefixed identity and let it derive both keys itself:

```go
// removeOrphanedDirectoryListMember drops fileName from dirPath's child index. It puts the
// member back if the value key exists again afterwards (or cannot be checked), because a
// concurrent InsertEntry may have found the member still present and skipped its ZAddNX.
func (store *UniversalRedis2Store) removeOrphanedDirectoryListMember(ctx context.Context, dirPath util.FullPath, fileName string) {
    dirListKey := store.getKey(genDirectoryListKey(string(dirPath)))
    ...
    exists, err := store.Client.Exists(ctx, store.getKey(string(util.NewFullPath(string(dirPath), fileName)))).Result()
    ...
}
```

Keep `fileName` as the raw ZSET member. Do **not** re-derive it with
`path.DirAndName()`: `DirAndName` runs `SanitizeUTF8Name` (`weed/util/fullpath.go:44`),
so a member that was stored unsanitized would be `ZREM`'d under a different string and
never removed. The more ambitious version of the same move adds a
`dirListKey(dir string) string` method and a `directoryListMember(name)` constructor, and
routes all six sites through them. That is the natural owner of the concept and makes the
`keyPrefix` class of bug structurally impossible, not just avoided in this call.

**Verification status:** verified by reading the head source and the `git grep` counts
above. `DirAndName`'s sanitization was confirmed at `weed/util/fullpath.go:42-52`.

---

## Finding 1.3 — Every failure in the repair helper is silent, including the one that re-creates the bug the second commit fixes

**Evidence.** `universal_redis_store.go:238-250`:

- `ZRem(...).Err()` failure → `return`, no log.
- `Exists(...)` failure → treated as "present" and falls through to the restore, no log.
- `ZAddNX(...)` result → discarded entirely.

The restore `ZADD NX` is the step that exists to prevent "live value, no index member,
invisible to listings and to `DeleteFolderChildren` until another `InsertEntry` on that
path". If that step fails (connection blip, `OOM command not allowed` under `maxmemory`,
failover), that exact state is produced, and nothing records it. The PR body says the
errors "are no longer discarded outright". They are no longer ignored for control flow,
but they are still invisible to operators, and the single most consequential one is
still dropped on the floor.

**Remedy.** Log the restore failure at error level, and optionally the `ZREM` failure at
`V(1)`:

```go
if err := store.Client.ZAddNX(ctx, dirListKey, redis.Z{Score: 0, Member: fileName}).Err(); err != nil {
    glog.ErrorfCtx(ctx, "restore %s in dir list %s: %v", fileName, dirPath, err)
}
```

That costs one line and turns a silent index-loss path into a diagnosable one. It also
matches how the filer layer reports its own cleanup failures (`glog.ErrorfCtx` at
`weed/filer/filer.go:430`).

**Verification status:** verified by reading the head source.

---

## Checked and not flagged

- **1k-line rule:** the file is 259 lines at head. Not applicable.
- **Transient-error safety:** `FindEntry` returns the bare sentinel only for `redis.Nil`
  (`:100-102`). The branch uses `==`, so wrapped errors still `break`. Confirmed.
- **Super-large directories:** the index key never exists for them, so `ZRangeByLex`
  returns nothing and the helper cannot be reached. Confirmed by reading
  `InsertEntry`/`DeleteEntry`.
- **Sibling stores:** `weed/filer/redis/universal_redis_store.go:184-187` (v1, `SET`-based
  index) and `weed/filer/redis3/universal_redis_store.go:156` have the same leak. Neither
  has a section in `weed/command/scaffold/filer.toml` (`grep -n '^\[redis'` shows only
  `redis2*` and the codeless `redis_lua*`), so leaving them alone is a defensible scope
  choice. Not flagged.
