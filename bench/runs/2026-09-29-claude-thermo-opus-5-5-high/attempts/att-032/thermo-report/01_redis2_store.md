# 01 — `weed/filer/redis2/universal_redis_store.go`

Scope: the +17 lines in `ListDirectoryEntries` and the new `removeOrphanedDirectoryListMember` helper, read against the rest of the store (`InsertEntry`, `UpdateEntry`, `DeleteEntry`, `DeleteFolderChildren`) and the filer-side callers (`weed/filer/filer.go`, `weed/filer/filerstore_wrapper.go`).

## Measurements and commands

All commands ran from the clone root, offline, with the attempt's module cache (`GOMODCACHE`, `GOCACHE`, `GOFLAGS=-mod=mod`, `GOPROXY=off`, `GOTOOLCHAIN=local`).

- `git diff main...review-head` shows 2 files, +160/−0. The store file gains one call site (line 208) and one 15-line helper (lines 237–251).
- `wc -l weed/filer/redis2/*.go` puts `universal_redis_store.go` at 259 lines, far below the 1k threshold, so file size is not a concern.
- `go vet ./weed/filer/redis2` is clean, and `gofmt -l weed/filer/redis2` prints nothing.
- `go test -count=1 -v ./weed/filer/redis2` passes, but all four test cases **skip** because no Redis server is available in this environment (`RUN_REDIS_TESTS` unset). The behaviour of the new code is therefore verified by reading it, not by running it.
- A grep for `getKey(` in `weed/filer/redis2` finds 12 key compositions in `universal_redis_store.go`: six `store.getKey(genDirectoryListKey(...))` (lines 68, 121, 136, 151, 166, 178) and six `store.getKey(string(path))` (lines 86, 99, 126, 161, 216, 245). The new test file adds four more, at test lines 76, 91, 112 and 130.
- A grep for `WithoutCancel` in `weed/filer/filerstore_wrapper.go` shows that `FilerStoreWrapper.ListDirectoryEntries` (line 324) and `ListDirectoryPrefixedEntries` (line 341) already detach cancellation before calling the store. Because of that, a torn repair caused by context cancellation cannot happen in production, and I have dropped that concern (see "Checked and dismissed" below).
- A grep for the logical-expiry expression finds the same `Crtime.Add(time.Duration(entry.TtlSec) * time.Second).Before(time.Now())` check in `weed/filer/filer.go:434` and `:473`, in `redis/`, `redis2/` and `redis3/` `universal_redis_store.go`, in `tikv_store.go:303` and in `rocksdb_ttl.go:31`.

## Correctness of the compensating protocol (verified by reasoning)

I walked through the helper's sequence (`ZREM member` → `EXISTS value` → `ZADDNX member` unless the value is confirmed absent) against every concurrent writer.

- **Concurrent `InsertEntry`** runs `SET value` and then `ZADDNX member`. If its `SET` lands before our `EXISTS`, we restore the member. If it lands after, its own `ZADDNX` comes after our `ZREM` and restores the member. Both orders converge.
- **Concurrent `DeleteEntry`** runs `DEL value` and then `ZREM member`. If our `EXISTS` sees the value before the delete's `DEL`, and our `ZADDNX` then lands after the delete's `ZREM`, the member is resurrected as an orphan. That orphan is removed by the next listing, so the state self-heals. This is acceptable.
- **Two concurrent listers** both `ZREM`, and each re-checks independently. This is idempotent.

The protocol is sound. The authors are also right that a Lua script or `MULTI` spanning both keys would be `CROSSSLOT` under `redis_cluster2`, because the keys carry no hash tag. The single-key compensating form is the right primitive. My findings are about how the primitive is wired into the store, not about the primitive itself.

---

## Finding A — Two divergent "dead member" cleanup protocols now sit in the same loop; collapse them into one

**Where:** `weed/filer/redis2/universal_redis_store.go:205-220` (both branches) and `:237-251` (the helper).

**Verification:** confirmed by reading. There is no runtime evidence because Redis is unavailable.

After this PR, the loop in `ListDirectoryEntries` handles a dead index member in two different ways, ten lines apart.

- **Value missing** (line 207). The loop calls `removeOrphanedDirectoryListMember`: `ZREM`, re-check, and restore when the value is present again. This path has a comment, a test, and an explanation of why a plain `ZREM` is unsafe.
- **Value present but logically expired** (lines 214–219). The loop runs a bare `Del(value)` and a bare `ZRem(member)`, discarding both results. This is exactly the unconditional `ZREM` pattern the second commit (`6c8fde664`) calls unsafe.

The PR body acknowledges that the expiry branch "has a pre-existing instance of the same race". It leaves that branch alone because fully closing the race needs a compare-and-delete on the value. That is fair for the value-loss half of the race. It does not justify keeping two cleanup protocols. After the expiry branch has `DEL`ed the value, the member is in exactly the state the new helper exists to handle: an index member whose value is gone. The branch should hand off to the helper instead of running its own weaker copy.

That is the code-judo move. Treat "logically expired" as "delete the value, then it is not found". The loop then has a single dead-member path, the `if … else` nesting collapses into guard clauses, and the store has one cleanup protocol to reason about instead of two that disagree.

### Worked proposal

```go
// fetch entry meta
var entry *filer.Entry
for _, fileName := range members {
	path := util.NewFullPath(string(dirPath), fileName)
	lastFileName = fileName

	entry, err = store.FindEntry(ctx, path)
	if err == nil && entry.TtlSec > 0 &&
		entry.Attr.Crtime.Add(time.Duration(entry.TtlSec)*time.Second).Before(time.Now()) {
		// logically expired: drop the value, then clean the index like any other orphan
		store.Client.Del(ctx, store.getKey(string(path)))
		err = filer_pb.ErrNotFound
	}
	if err == filer_pb.ErrNotFound {
		store.removeOrphanedDirectoryListMember(ctx, path)
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

This proposal also uses the path-only helper signature from Finding B.

### Behaviour deltas, stated honestly

1. **The expiry branch's `ZREM` gains the re-check and restore.** This is strictly safer. If a concurrent `InsertEntry` recreates the path between our `GET` and our `DEL`, the value is still lost; that is the pre-existing race the authors deliberately scoped out. However, if the recreate lands after the `DEL`, its member is no longer stripped. The same applies if the `DEL` itself fails on a transport error: the helper's `EXISTS` then sees the still-present value and keeps the member. Today the bare `ZRem` strips the member off a still-present value in that case. The cost is one `EXISTS` per expired member, once per member, on the repair path only.
2. **The V(0) log moves after the not-found check.** Orphan repairs no longer log at V(0). The PR body itself cites that line as default-on noise emitted per dead member. If the team wants the repair to stay visible, log it at `V(1)` inside the helper instead. Either way, the choice should be deliberate. Today the log line fires for every repair as a side effect of where it sits.
3. Nothing else changes. `lastFileName` is still set for every member, including skipped ones. Transport errors still break the loop with the wrapped error.

### Ambitious follow-up (outside this diff, noted for the owners)

The store-level logical-expiry branch duplicates policy the filer already owns. `Filer.doListDirectoryEntries` (`weed/filer/filer.go:465-478`) applies the same `Crtime + TtlSec` check to every listed entry and deletes expired entries through `Store.DeleteOneEntry`, which goes through `DeleteEntry` and does a clean `DEL` + `ZREM`. `Filer.FindEntry` does the same at `:434-437`. Removing the expiry branch from the store would leave it with exactly one concept, "value missing ⇒ repair the index", and let the filer own TTL as it does for every other store. I have not verified that no caller reaches `UniversalRedis2Store.ListDirectoryEntries` except through the filer, so this is a suggestion for a separate change, not a request for this PR.

---

## Finding B — The helper's signature mixes a pre-prefixed key with a raw path, which is the same `keyPrefix` trap the PR body warns about

**Where:** `weed/filer/redis2/universal_redis_store.go:237` (signature), `:245` (it prefixes `path` itself), `:208` (call site), and `weed/filer/redis2/universal_redis_store_test.go:112` (the test rebuilds the prefixed key by hand).

**Verification:** confirmed by reading.

`removeOrphanedDirectoryListMember(ctx, dirListKey string, path util.FullPath, fileName string)` takes three parameters that describe one member, in two key-spaces:

- `dirListKey` must already be prefixed with `store.getKey(...)`;
- `path` must **not** be prefixed, because the helper applies `store.getKey(string(path))` itself at line 245;
- `fileName` is redundant, since it is the name component of `path`.

The PR body spends a paragraph on the danger here: rebuilding the index key without the prefix "would have been a silent no-op for every deployment that sets `keyPrefix`". Yet the new helper's contract makes each caller responsible for that exact decision, one argument at a time. The one caller outside the loop is the test at `universal_redis_store_test.go:112`, and it has to hand-compose `store.getKey(genDirectoryListKey(string(dir)))` to call the helper. That is the pattern the body warns against, now needed just to use the API. A future caller who passes `genDirectoryListKey(dir)` without the prefix gets a helper that silently `ZREM`s from a non-existent key and then, because the value exists, `ZADDNX`es a member into that wrong key. That leaves a stray ZSET behind.

The underlying cause is older than this PR. The store has no names for its two key shapes. `store.getKey(genDirectoryListKey(x))` is spelled out six times and `store.getKey(string(p))` six times in `universal_redis_store.go`, and the PR adds four more hand-built keys in the test file. The helper's awkward signature is a symptom of that missing vocabulary.

### Worked proposal

Give the two key shapes names once, and have the helper take only the path:

```go
func (store *UniversalRedis2Store) valueKey(p util.FullPath) string {
	return store.getKey(string(p))
}

func (store *UniversalRedis2Store) dirListKey(dir string) string {
	return store.getKey(genDirectoryListKey(dir))
}

// removeOrphanedDirectoryListMember drops path's index member when its value is
// gone. InsertEntry writes the value before adding the member, so a value present
// again after the ZREM may belong to an insert whose ZAddNX was a no-op; put the
// member back in that case.
func (store *UniversalRedis2Store) removeOrphanedDirectoryListMember(ctx context.Context, path util.FullPath) {
	dir, name := path.DirAndName()
	listKey := store.dirListKey(dir)

	if err := store.Client.ZRem(ctx, listKey, name).Err(); err != nil {
		return
	}
	exists, err := store.Client.Exists(ctx, store.valueKey(path)).Result()
	if err == nil && exists == 0 {
		return
	}
	store.Client.ZAddNX(ctx, listKey, redis.Z{Score: 0, Member: name})
}
```

With these helpers in place:

- The call site becomes `store.removeOrphanedDirectoryListMember(ctx, path)`.
- The test's hand-built key at line 112 disappears.
- The test helpers `indexMembers` (line 76) and the two `Exists`/`Del` probes (lines 91, 130) can use `store.dirListKey(string(dir))` and `store.valueKey(...)`, so tests and production cannot disagree about prefixing.

Adopting `valueKey`/`dirListKey` across the other twelve sites is a mechanical follow-up. It is worth doing in this PR only if the owners want it, but the helper and its test should use the named form from the start.

`path.DirAndName()` on `util.NewFullPath(string(dirPath), fileName)` gives back `(string(dirPath), fileName)` for every member the loop can produce, root directory included. That makes the path-only signature behaviour-preserving.

---

## Finding C — The one repair failure that hides a live entry is silently discarded

**Where:** `weed/filer/redis2/universal_redis_store.go:238-240` and `:250`.

**Verification:** plausible, by reading. I could not exercise it because no Redis is available.

The PR body says cleanup errors "are no longer discarded outright, since the restore decision depends on them". That holds for the `ZREM` and `EXISTS` results, which steer control flow. It does not hold for the step that matters most. At line 250, the `ZAddNX` restore runs only when the value is present or its presence is unknown, meaning a live entry may be losing its index member, and its error is thrown away. If that call fails on a connection reset or a failover, the member stays removed from a live value. The entry is then invisible to `ListDirectoryEntries` and `DeleteFolderChildren` until some later `InsertEntry` on that exact path. That is precisely the regression the second commit exists to prevent, and it leaves no log line. The early `return` on a failed `ZREM` (line 239) is also silent. That one is harmless, since nothing was removed, but it means the repair path has no observability at all.

I checked whether context cancellation widens this window. It does not in production: `FilerStoreWrapper.ListDirectoryEntries` detaches cancellation with `context.WithoutCancel` (`weed/filer/filerstore_wrapper.go:324`, and `:341` for the prefixed variant) before reaching the store. So only genuine transport errors trigger this failure. That keeps it narrow, but the outcome is the harmful direction, and it should not be invisible.

### Remedy

Log a failed restore at a level that is on by default, naming the path, because it is the one outcome that leaves a live entry unlisted. For example:

```go
if err := store.Client.ZAddNX(ctx, listKey, redis.Z{Score: 0, Member: name}).Err(); err != nil {
	glog.WarningfCtx(ctx, "restore directory index member %s: %v", path, err)
}
```

The same `glog` call can optionally sit on the `ZREM` error at `V(1)`. With Finding A adopted, this is the single place any dead-member repair can go wrong, so one log statement covers both branches.

---

## Checked and dismissed

- **Context cancellation tearing the repair between `ZREM` and `ZADDNX`.** Dismissed, because the store wrapper detaches cancellation for listings (see Finding C).
- **Super-large directories.** When a directory is listed in `superLargeDirectoryHash`, `InsertEntry` skips the `ZAddNX` and `DeleteEntry` skips the `ZRem`, so the index key never exists. `ZRangeByLex` returns nothing and the helper is unreachable. Confirmed by reading lines 63–65, 132–134 and 189–201.
- **Transient errors reaching the repair.** `FindEntry` returns the bare `filer_pb.ErrNotFound` only for `redis.Nil` (lines 99–102) and wraps every other error with `fmt.Errorf`. The branch test is `==`, so only a genuine miss reaches the repair.
- **File size.** 259 lines. Not a concern.
