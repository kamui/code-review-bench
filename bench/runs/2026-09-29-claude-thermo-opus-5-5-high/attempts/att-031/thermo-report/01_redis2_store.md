# 01 — redis2 store: `ListDirectoryEntries` and `removeOrphanedDirectoryListMember`

Scope: `weed/filer/redis2/universal_redis_store.go` at `review-head` (`6c8fde664`), compared with `main` (`db5a086d0`).

## Measurements

| Item | Value | How obtained |
| --- | --- | --- |
| File size after PR | 259 lines (was 242) | `wc -l weed/filer/redis2/universal_redis_store.go` |
| Lines changed | +17 / −0 | `git diff --stat main...review-head` |
| New helper | `removeOrphanedDirectoryListMember`, lines 237-251 | read at head |
| Call site | line 208, inside `if err != nil { ... if err == filer_pb.ErrNotFound { ... } }` | read at head |
| Pre-existing expiry reap | lines 214-219 (`Del` value + `ZRem` member, both results discarded) | read at head |
| Build | `go test -count=1 -v ./weed/filer/redis2` (offline, with the provided cache): package compiles, all three new tests SKIP because there is no Redis | run in this review |

The file stays far below the 1k-line boundary, so size is not a concern.

## Finding S1 — The loop now has two different "drop this child from the index" procedures, and only one is race-guarded

**Verification status: confirmed by reading the code at head.** The behavioural consequence of the unguarded path is pre-existing and is acknowledged in the PR body. The structural duplication is what this PR introduces.

After this PR, `ListDirectoryEntries` reaps dead children in two places, each written differently:

- The not-found branch (line 208) calls the new helper. The helper runs `ZREM`, re-checks the value with `EXISTS`, and restores the member with `ZADDNX` unless the value is confirmed absent. Its comment says why: a concurrent `InsertEntry` whose `ZAddNX` was a no-op would otherwise lose its index member.
- The logical-expiry branch (lines 214-219) still runs an inline `Del(value)` and then a bare `ZRem(member)`, discarding both results.

Both branches do the same thing to the index: they remove a dead child's member. After the value `DEL`, the expiry branch is in exactly the state the helper was written for (value absent, member present, and a concurrent insert could land in between). It is exposed to the same index-member loss that the second commit fixed for the other branch. The PR body calls the expiry branch's *value* race out of scope, and that is reasonable. But the diff leaves the reader with two procedures for one concept, and the unguarded one sits nine lines below the guarded one. The next person to touch this loop has to work out why they differ, and the answer is "history", not "design".

The helper's name adds to the problem. `removeOrphanedDirectoryListMember` describes one caller's situation (the value was already gone when we looked). It does not describe what the function guarantees: remove the member unless the value is present, and fail towards keeping it. That guarantee is exactly what the expiry branch needs too.

There is also a nesting problem. The new call is buried two levels deep in the `if err != nil { log; if err == ErrNotFound { ...; continue }; break } else { if TtlSec > 0 { if expired { ...; continue } } ... }` ladder. The PR adds one more statement to the busiest spot in that ladder instead of flattening it. The `glog.V(0)` "list %s : %v" line also still fires before the repair. What is now a normal, self-healing event is therefore logged at default verbosity just like a real store error. The PR body lists log noise as one of the symptoms being fixed, so each orphan still logs once, on the listing that reaps it.

### Worked code-judo proposal

Rename the helper to describe the guarantee, and route both dead-child cases through it. Then flatten the loop into a `switch` so that each outcome of `FindEntry` is one visible case:

```go
	for _, fileName := range members {
		path := util.NewFullPath(string(dirPath), fileName)
		entry, err = store.FindEntry(ctx, path)
		lastFileName = fileName
		switch {
		case err == filer_pb.ErrNotFound:
			// value dropped by Redis TTL, eviction, or an out-of-band DEL
			store.dropDirectoryListMember(ctx, dirListKey, path, fileName)
			err = nil
			continue
		case err != nil:
			glog.V(0).InfofCtx(ctx, "list %s : %v", path, err)
		case entry.TtlSec > 0 && entry.Attr.Crtime.Add(time.Duration(entry.TtlSec)*time.Second).Before(time.Now()):
			store.Client.Del(ctx, store.getKey(string(path)))
			store.dropDirectoryListMember(ctx, dirListKey, path, fileName)
			continue
		default:
			ok, cbErr := eachEntryFunc(entry)
			if cbErr != nil {
				err = fmt.Errorf("failed to process eachEntryFunc: %w", cbErr)
			} else if ok {
				continue
			}
		}
		break
	}
```

```go
// dropDirectoryListMember removes fileName from the directory index unless its
// value key exists. The check runs after the ZREM because InsertEntry writes the
// value before its ZAddNX, and that ZAddNX is a no-op while the member is still
// present; a failed check keeps the member, so errors leave a stale member
// (reaped on a later listing) rather than hiding a live entry.
func (store *UniversalRedis2Store) dropDirectoryListMember(ctx context.Context, dirListKey string, path util.FullPath, fileName string) {
	if store.Client.ZRem(ctx, dirListKey, fileName).Err() != nil {
		return
	}
	if exists, err := store.Client.Exists(ctx, store.getKey(string(path))).Result(); err == nil && exists == 0 {
		return
	}
	store.Client.ZAddNX(ctx, dirListKey, redis.Z{Score: 0, Member: fileName})
}
```

What this buys:

- One index-removal procedure instead of two, with its invariant written next to it. The expiry branch picks up the member-restore guard for free. The only behaviour change is inside the concurrent-recreate window, and it is the same correction the second commit already made for the other branch. The value-`DEL` race the author scoped out stays out of scope.
- The `err`/`else`/`if`/`if` ladder becomes a four-way `switch` whose cases correspond one-to-one to the outcomes of `FindEntry`. The not-found case no longer passes through the error log.
- The `glog.V(0)` line only fires for real store errors. If the author wants a trace of repairs, a `glog.V(1)` line inside the not-found case keeps it without the default-verbosity noise.

I considered deriving `dirListKey` and `fileName` inside the helper from `path` alone, the way `DeleteEntry` does via `DirAndName()`, and rejected it. `DirAndName` passes the name through `util.SanitizeUTF8Name` (`weed/util/fullpath.go`). A member written before that sanitisation, containing invalid UTF-8, would no longer match, so `ZREM` would silently miss it. Passing the member name through verbatim, as the PR does, is correct.

## Question Q1 — Should the store be doing logical TTL expiry at all?

**Verification status: plausible; needs a caller audit before acting.**

`Filer.doListDirectoryEntries` (`weed/filer/filer.go:461-499`) already applies the canonical `Crtime + TtlSec` policy to every entry the store yields. It collects expired entries and deletes them through `Store.DeleteOneEntry`, which reaches `UniversalRedis2Store.DeleteEntry` (value `DEL` + index `ZREM`). `Filer.FindEntry` (`filer.go:426-437`) does the same on point lookups. The store-level expiry branch at `universal_redis_store.go:214-219` is therefore a second copy of filer policy in a store adapter. It is also the source of the out-of-scope value race the PR body describes.

If every production caller of `ListDirectoryEntries` goes through the filer, the bigger code-judo move is to delete the store's expiry branch outright. The loop would then have one dead-child case (not-found) and one helper. The pre-existing value-loss race would leave the store instead of needing a compare-and-delete. I have not audited direct store callers (for example, metadata backup or migration tooling that iterates `FilerStore` without the filer). Those callers would start seeing logically expired entries, so this is a question for the maintainers rather than a demand on this PR.

## Things checked and found sound

- **Branch guard identity.** `FindEntry` returns the bare `filer_pb.ErrNotFound` only for `redis.Nil` (line 100-101). Transport errors are wrapped with `fmt.Errorf` and do not match `err == filer_pb.ErrNotFound`, so a connection blip cannot trigger the cleanup.
- **Prefix handling.** `dirListKey` is already prefixed (line 178) and is reused. The value key is re-derived with `store.getKey(string(path))`, which matches `FindEntry`.
- **Interleavings with a concurrent `InsertEntry`.** I traced all orderings of {L: ZREM, L: EXISTS, L: ZADDNX} against {I: SET, I: ZADDNX}. The member always ends up present when the value is present. A concurrent `DeleteEntry` interleaved after L's `EXISTS` can leave a stale member. That is the bias the author chose, and it is reaped on the next listing.
- **Cluster.** `ZREM`, `EXISTS` and `ZADDNX` are single-key, so they are safe under `RedisCluster2Store`. The author's argument that a multi-key Lua script would be `CROSSSLOT` holds, because keys carry no hash tag.
- **Super-large directories.** The index key does not exist, so the loop body never runs.
