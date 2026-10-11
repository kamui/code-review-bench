# 01 — redis2 store: orphaned index-member cleanup

Scope: `weed/filer/redis2/universal_redis_store.go`, range `db5a086d0..6c8fde664` (+17 / −0 net; the second commit rewrote the first commit's one-line `ZREM` into a 15-line helper).

All line numbers below refer to the head (`review-head`, `6c8fde664`) unless stated otherwise.

## What the change does

`ListDirectoryEntries` iterates the members of the per-directory ZSET `<dir>\x00` and issues one `GET` per member through `FindEntry`. Before this PR a member whose value key was gone was logged and skipped; now it is handed to a new helper:

```go
// universal_redis_store.go:237-251
func (store *UniversalRedis2Store) removeOrphanedDirectoryListMember(ctx context.Context, dirListKey string, path util.FullPath, fileName string) {
	if err := store.Client.ZRem(ctx, dirListKey, fileName).Err(); err != nil {
		return
	}

	// InsertEntry writes the value before adding the member, so a value present
	// again here may belong to an insert that found the member still in place
	// and whose ZAddNX was therefore a no-op.
	exists, err := store.Client.Exists(ctx, store.getKey(string(path))).Result()
	if err == nil && exists == 0 {
		return
	}

	store.Client.ZAddNX(ctx, dirListKey, redis.Z{Score: 0, Member: fileName})
}
```

The shape is remove, re-read, compensate. Call the commands of the listing `G` (GET, returns nil), `R` (ZREM), `E` (EXISTS), `A'` (restoring ZADD NX), and those of a concurrent `InsertEntry` `S` (SET) and `A` (ZADD NX), with `G < R < E < A'`, `S < A`, and `G < S` because `G` saw nothing.

- If `S < E`, `E` sees the value and `A'` re-adds the member after `R`. The member survives.
- If `E < S`, then `R < E < S < A`, so the insert's own `A` lands after `R`. The member survives.

So, given linearizable reads and a successful `A'`, the ordering argument in the second commit is sound, and it is the right ordering for a store that cannot run a cross-slot script. Both of those preconditions are where the findings below come from.

## Measurements and commands

| Check | Command | Result |
| --- | --- | --- |
| File size | `wc -l weed/filer/redis2/*.go` | `universal_redis_store.go` is 259 lines at head; no size concern |
| Package tests | `go test -count=1 -v ./weed/filer/redis2` (offline, cached modules) | `ok`; all four test functions/subtests **SKIP** (no Redis available) |
| Vet | `go vet ./weed/filer/redis2` | clean |
| Format | `gofmt -l weed/filer/redis2` | clean |
| Index write sites | `grep -n "ZRem\|ZAddNX" weed/filer/redis2/*.go` | five sites in the store: `:68` (insert add), `:136` (delete remove), `:217` (expiry remove), `:238` (orphan remove), `:250` (orphan restore) |
| Read routing | read `go-redis/v9@v9.21.0/osscluster.go` in the module cache | see F1 |

Nothing below was reproduced against a live Redis or a Redis Cluster; none is available in this environment. Each finding states whether it rests on code reading or on execution.

## F1 — the orphan decision trusts reads that may come from a replica

**Evidence.** `RedisCluster2Store.Initialize` reads `useReadOnly` and `routeByLatency` (`redis_cluster_store.go:28-29`) and passes them straight into `redis.ClusterOptions{ReadOnly, RouteByLatency}` (`:53-54`). In go-redis v9.21.0:

```go
// osscluster.go:205-207
if opt.RouteByLatency || opt.RouteRandomly {
	opt.ReadOnly = true
}

// osscluster.go:2399-2405  (cmdNode)
if c.opt.ReadOnly {
	cmdInfo := c.cmdInfo(ctx, cmdName)
	if cmdInfo != nil && cmdInfo.ReadOnly {
		return c.slotReadOnlyNode(state, slot)
	}
}
return state.slotMasterNode(slot)
```

`slotReadOnlyNode` picks the closest node, a random node, or a replica (`osscluster.go:935-961` prefers `nodes[1:]`). `GET`, `EXISTS` and `ZRANGEBYLEX` are all flagged read-only by Redis; `SET`, `ZADD`, `ZREM` are not. The value key `<dir>/<name>` and the index key `<dir>\x00` hash to different slots (the second commit message says so itself), so they generally live on different shards with independent replication lag.

**Failure.** With either option on:

1. `InsertEntry` runs `SET <dir>/<name>` on the primary of shard V and `ZADD NX <dir>\x00` on the primary of shard D.
2. A listing reads `ZRANGEBYLEX` from a replica of shard D that already has the member, then `GET` from a replica of shard V that has not applied the `SET` yet. `FindEntry` returns `filer_pb.ErrNotFound`.
3. New in this PR: `ZREM` goes to the primary of shard D and removes the member of a live entry.
4. `EXISTS` is routed like the `GET` was — with `routeByLatency`, very likely to the very same lagging node — and returns 0. The restore is skipped.

The value is live on the primary and absent from the index. As the second commit message notes, `UpdateEntry` does not re-add the member (`:92-95` only calls `doInsertEntry`), so the entry stays invisible to listings until something calls `InsertEntry` on that exact path again.

Before this PR the same replica lag produced a log line and a one-listing miss that healed by itself. After it, replica lag is converted into durable index loss, and the trigger is the ordinary create-then-list pattern, not a rare three-way race. The PR body's premise — "a member returned by `ZRangeByLex` whose value is absent has had that value deleted or expired — it is never an entry mid-creation" — holds only when the `GET` is answered by the primary.

**Reach.** `redis2` (`redis.NewClient`) and `redis2_sentinel` (`redis.NewFailoverClient`, no replica option set in `redis_sentinel_store.go` or `redis_conf`) read from the primary and are unaffected. `redis_cluster2` is affected when `useReadOnly` or `routeByLatency` is set; both default to `false`. `routeByLatency` is documented in `weed/command/scaffold/filer.toml:289-290`. (The scaffold spells the other key `readOnly` at `:287-288` while the code reads `useReadOnly`; that mismatch predates this PR and is not part of this review.)

**Verification status.** Established by reading the store and the go-redis routing code in the module cache. Not executed: no Redis Cluster is available here, and the PR's tests build a single-node `redis.NewClient` (`universal_redis_store_test.go:29`), so no test in the PR can observe this.

**Remedy.** The read that licenses a destructive write must be answered by the primary. Only the `EXISTS` needs it: if the confirming read is primary-consistent, a stale first `GET` is repaired by the restore step. The store already has the right place to decide this — the three `initialize` functions, which hold the concrete client type:

```go
type UniversalRedis2Store struct {
	Client    redis.UniversalClient
	keyPrefix string
	// primaryFor returns a client whose reads for key are never served by a
	// replica. Nil when Client itself only reads from the primary.
	primaryFor              func(ctx context.Context, key string) (redis.Cmdable, error)
	superLargeDirectoryHash map[string]bool
}

// redis_cluster_store.go, initialize
client := redis.NewClusterClient(options)
store.Client = client
if options.ReadOnly || options.RouteByLatency {
	store.primaryFor = func(ctx context.Context, key string) (redis.Cmdable, error) {
		return client.MasterForKey(ctx, key)
	}
}
```

`ClusterClient.MasterForKey` exists in v9.21.0 (`osscluster.go:2481`) and returns a `*redis.Client`. The helper then confirms through `primaryFor` when it is set and through `store.Client` otherwise. No type assertion, no mode flag in the listing loop, and the single-node and sentinel stores are untouched. The cheaper alternative — skip the cleanup entirely when replica reads are enabled — is also defensible, but it leaves the leak in place for exactly the deployments large enough to run a cluster.

## F2 — two "this member is dead" exits, only one of them repaired

**Evidence.** The loop body at head:

```go
// universal_redis_store.go:203-220
entry, err = store.FindEntry(ctx, path)
lastFileName = fileName
if err != nil {
	glog.V(0).InfofCtx(ctx, "list %s : %v", path, err)
	if err == filer_pb.ErrNotFound {
		store.removeOrphanedDirectoryListMember(ctx, dirListKey, path, fileName)
		err = nil
		continue
	}
	break
} else {
	if entry.TtlSec > 0 {
		if entry.Attr.Crtime.Add(time.Duration(entry.TtlSec) * time.Second).Before(time.Now()) {
			store.Client.Del(ctx, store.getKey(string(path))).Result()
			store.Client.ZRem(ctx, dirListKey, fileName).Result()
			continue
		}
	}
```

The first commit justified its `ZREM` as "mirroring what the expiry branch already does four lines below". The second commit then decided that an unconditional `ZREM` is a bug — "an InsertEntry landing between FindEntry and the ZRem left a live value with no index member ... the loss persisted" — and replaced it with the helper. The expiry branch it was mirroring still does the unconditional `ZREM` at `:217`, and it has the same hole: an `InsertEntry` that lands between the `DEL` at `:216` and the `ZREM` at `:217` writes a fresh value, its `ZADD NX` is a no-op because the member is still there, and the `ZREM` then removes it.

So the function now has two exits that mean the same thing ("the value behind this member is gone; drop the member"), nested at different depths of an `if err != nil { ... } else { if { if { ... } } }`, with two different removal procedures, one of which the PR itself has declared unsafe. A reader has to diff the two by eye to learn that the difference is an accident of history rather than a requirement.

**Verification status.** Code reading. The expiry branch's race is the same interleaving the second commit describes, shifted by one command.

**Worked code-judo.** After the expiry branch's `DEL`, the entry is in exactly the state the not-found branch handles. Make "logically expired" produce "not found" and there is one exit and one removal procedure:

```go
for _, fileName := range members {
	path := util.NewFullPath(string(dirPath), fileName)
	entry, err = store.FindEntry(ctx, path)
	lastFileName = fileName

	if err == nil && isExpired(entry) {
		store.Client.Del(ctx, store.getKey(string(path)))
		err = filer_pb.ErrNotFound
	}
	if err != nil {
		glog.V(0).InfofCtx(ctx, "list %s : %v", path, err)
		if err != filer_pb.ErrNotFound {
			break
		}
		store.removeOrphanedDirectoryListMember(ctx, dirListKey, path, fileName)
		err = nil
		continue
	}

	resEachEntryFunc, resEachEntryFuncErr := eachEntryFunc(entry)
	...
}

func isExpired(entry *filer.Entry) bool {
	return entry.TtlSec > 0 && entry.Crtime.Add(time.Duration(entry.TtlSec)*time.Second).Before(time.Now())
}
```

The `else` arm and two levels of nesting disappear, the raw `ZREM` at `:217` disappears, and the race-safe procedure has one caller-visible meaning. (The log line would now also fire for logically-expired members; keep it under the not-found test only if that matters.)

**A larger move, noted but not requested.** The filer already owns logical expiry one layer up: `Filer.doListDirectoryEntries` (`weed/filer/filer.go:465-500`) checks the same `Crtime + TtlSec` condition, distinguishes S3-expiry entries, deletes through `Store.DeleteOneEntry` with errors logged, and reports `expiredCount` so `doListValidEntries` (`filer_search.go:118-124`) can refill the page. The store-level branch pre-empts all of that. Deleting it outright would leave the helper as the only cleanup in the store. That is a behaviour change for S3-expiry entries and page refill and belongs in its own PR; it is recorded here as a question, not a finding.

## F3 — a compensating write whose failure is invisible

**Evidence.** `universal_redis_store.go:237-251`, quoted at the top of this file. Three things about its error handling:

1. The helper has no return value and logs nothing. If the `ZREM` succeeds, `EXISTS` reports the value present, and the restoring `ZADD NX` at `:250` fails, the store is left in precisely the state the second commit was written to prevent — live value, no member, not repaired by `UpdateEntry` — and nothing anywhere records that it happened. The adjacent filer-level code logs every failed cleanup (`filer.go:493`, `:498`).
2. The early return at `:238-240` treats a `ZREM` error as "nothing was removed". A timeout or a dropped connection after the server applied the command returns an error too. In that case the member is gone and the recheck that would have restored it is skipped.
3. The guard at `:246` is written as the negation of what the following line does. `if err == nil && exists == 0 { return }` followed by an unconditional restore reads as "bail out unless ...", and the reader has to invert it to see that an `EXISTS` error deliberately falls through to the restore. That is the correct, conservative choice, but it is expressed as incidental control flow rather than stated.

There is also a duplicated invariant: `redis.Z{Score: 0, Member: ...}` now appears at `:68` and `:250`. `ZRANGEBYLEX` is only defined when every member has the same score, so those two literals must stay identical forever; nothing ties them together.

**Verification status.** Code reading. `go vet` does not flag discarded command results.

**Worked proposal.** The restore step is idempotent and only ever adds a member for a value that exists, so it is always safe to attempt. That removes the early return rather than complicating it:

```go
// addDirectoryListMember is the only place a member is written to a directory
// index. Every member carries score 0 so that ZRANGEBYLEX orders by name.
func (store *UniversalRedis2Store) addDirectoryListMember(ctx context.Context, dirListKey, name string) error {
	return store.Client.ZAddNX(ctx, dirListKey, redis.Z{Score: 0, Member: name}).Err()
}

func (store *UniversalRedis2Store) removeOrphanedDirectoryListMember(ctx context.Context, dirListKey string, path util.FullPath, fileName string) {
	store.Client.ZRem(ctx, dirListKey, fileName)

	// InsertEntry writes the value before adding the member, so an insert that
	// ran since FindEntry may have found the member in place and added nothing.
	// Put the member back unless the value is known to be absent.
	if exists, err := store.existsOnPrimary(ctx, store.getKey(string(path))); err == nil && !exists {
		return
	}
	if err := store.addDirectoryListMember(ctx, dirListKey, fileName); err != nil {
		glog.ErrorfCtx(ctx, "restore %s in directory index: %v", path, err)
	}
}
```

`InsertEntry` at `:68` calls `addDirectoryListMember` too. `existsOnPrimary` is the F1 seam. One further hardening worth considering: the restore runs on the listing request's `ctx`; if that request is abandoned between the `ZREM` and the `ZADD NX`, the compensation is what gets dropped. `context.WithoutCancel(ctx)` for the restore alone costs one line.

## F4 — three serial round trips per dead member on the listing path

**Evidence.** For each dead member the loop now issues `GET` (`:203` → `:99`), `ZREM` (`:238`) and `EXISTS` (`:245`), one after another, before it moves to the next member. The PR body names the cost of dead members as its first motivation: "`LIST` costs one `GET` round trip per name ever created in that directory". The dead members are now removed, which fixes the steady state, but each one costs three round trips on the way out instead of one, inside a user-facing listing. A directory that has accumulated a backlog — the population this PR exists for — pays `3 × min(limit, backlog)` serial round trips per page until it is drained, and a directory under continuous TTL churn pays three per entry expired since the previous listing, where it used to pay one.

**Verification status.** Counted from the code; not timed.

**Proposal.** `ZREM` is variadic, and the ordering argument at the top of this file only needs `G < R < E < A'` per member, not adjacency. Collect the dead names in the loop and clean up once after it:

```go
var dead []any
...
	if err == filer_pb.ErrNotFound {
		dead = append(dead, fileName)
		err = nil
		continue
	}
...
store.removeOrphanedDirectoryListMembers(ctx, dirPath, dirListKey, dead)
return lastFileName, err
```

with one `ZREM dirListKey dead...`, then the existence checks, then one `ZADD NX` for the survivors. On `redis2` and `redis2_sentinel` the existence checks can go in one pipeline. On a cluster with replica reads they cannot: a pipeline made only of read-only commands is routed to replicas (`osscluster.go:1634`), so there the checks stay per key through the F1 seam. Even so the extra cost falls from `2N` round trips to `N + 2`. This composes with F2: both dead-member exits append to the same slice.

This is lower priority than F1–F3. It is listed because the change makes the hot path it set out to relieve temporarily more expensive, and because the batched form is also the simpler loop.

## Things checked and found sound

- `dirListKey` is built once at `:178` with `store.getKey(...)`, and the helper receives that variable; the value key is prefixed at `:245`. The `keyPrefix` handling is correct on both keys.
- The branch guard is identity comparison against `filer_pb.ErrNotFound`, and `FindEntry` returns the bare sentinel only for `redis.Nil` (`:100-102`). A transient `GET` error cannot reach the helper.
- Super-large directories never get an index key (`:63-65`, `:132-134`, `:147-149`), so the loop body, and with it the helper, is unreachable for them.
- A restore that races with a `DeleteEntry` can leave an orphaned member behind; the next listing removes it. That is self-healing and not a concern.
- File size: 259 lines. No decomposition concern.
- The same not-found `continue` with no index repair exists in the `redis` store (`weed/filer/redis/universal_redis_store.go:184-187`) and, as the PR body says, in `redis3`. The PR is explicitly scoped to `redis2`; this is context, not a finding.
