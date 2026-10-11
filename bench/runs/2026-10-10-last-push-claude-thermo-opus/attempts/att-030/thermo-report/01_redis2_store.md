# 01 — redis2 store: listing loop and orphan helper

Subject: `weed/filer/redis2/universal_redis_store.go` at head `6c8fde664`.
Line numbers refer to the head tree unless stated otherwise.

## 1. What changed and how it was checked

The store diff is 17 added lines: one call at line 208 and the new helper at lines 237-251.

```go
// :207-211
if err == filer_pb.ErrNotFound {
	store.removeOrphanedDirectoryListMember(ctx, dirListKey, path, fileName)
	err = nil
	continue
}

// :237-251
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

History matters for reading it. Commit `9f9962800` added a single line,
`store.Client.ZRem(ctx, dirListKey, fileName).Result()`, mirroring the expiry
branch. Commit `6c8fde664` replaced that line with the helper after two review
comments on the concurrent-insert race.

Commands run, from the clone root with the offline Go environment the run
supplies:

| Command | Result |
| --- | --- |
| `git diff main...review-head` | 2 files, +160 / −0 |
| `wc -l weed/filer/redis2/*.go` | store 259 lines, test 143, package 620 |
| `go test -count=1 -v ./weed/filer/redis2` | `ok`, all 4 cases `SKIP` (no Redis) |
| `go vet ./weed/filer/redis2` | clean |
| `gofmt -l weed/filer/redis2` | no output |
| `git status --short` after the above | clean |

Verification status for this file: every claim is from reading source. No
interleaving was executed, because the environment has no Redis server. Where a
claim depends on go-redis behaviour, the line in
`github.com/redis/go-redis/v9@v9.21.0` is cited.

File size is not a concern. 259 lines, far from the 1000-line threshold.

## 2. F1 — replica reads make the absence observation non-authoritative

### Evidence

The cluster store accepts two routing options and passes them to go-redis:

```go
// weed/filer/redis2/redis_cluster_store.go
27:	configuration.SetDefault(prefix+"useReadOnly", false)
28:	configuration.SetDefault(prefix+"routeByLatency", false)
...
53:		ReadOnly:       readOnly,
54:		RouteByLatency: routeByLatency,
```

go-redis v9.21.0, `osscluster.go`:

```go
205:	if opt.RouteByLatency || opt.RouteRandomly {
206:		opt.ReadOnly = true

2399:	if c.opt.ReadOnly {
2400:		cmdInfo := c.cmdInfo(ctx, cmdName)
2401:		if cmdInfo != nil && cmdInfo.ReadOnly {
2402:			return c.slotReadOnlyNode(state, slot)
2403:		}
2404:	}
2405:	return state.slotMasterNode(slot)
```

`slotReadOnlyNode` (`:2438-2451`) returns the closest node under
`RouteByLatency` and otherwise a replica via `slotSlaveNode` (`:935`).

`GET`, `EXISTS` and `ZRANGEBYLEX` are read-only commands. `ZREM` and `ZADD` are
writes. So with either option on, the helper's commands split like this:

| Step | Command | Key | Served by |
| --- | --- | --- | --- |
| list members (`:189`) | `ZRANGEBYLEX` | `<dir>\x00` | replica of slot B |
| find entry (`:203`, via `:99`) | `GET` | `<dir>/<name>` | replica of slot A |
| remove (`:238`) | `ZREM` | `<dir>\x00` | master of slot B |
| re-check (`:245`) | `EXISTS` | `<dir>/<name>` | replica of slot A |
| restore (`:250`) | `ZADD NX` | `<dir>\x00` | master of slot B |

### Failure interleaving

Slots A and B are different (the second commit's message says so itself: "The
value key and the directory index key hash to different slots"), so they have
separate replication streams.

1. Writer: `InsertEntry` → `SET <dir>/<name>` on master A, then `ZADD NX` on master B. Both acknowledged.
2. Replica B has applied the `ZADD`. Replica A has not yet applied the `SET` (lag, a large value, a busy replica).
3. Lister: `ZRANGEBYLEX` on replica B returns `<name>`.
4. Lister: `GET` on replica A returns `redis.Nil` → `filer_pb.ErrNotFound`.
5. Lister: `ZREM` on master B removes the member of a live entry.
6. Lister: `EXISTS` on replica A, microseconds later, still returns 0 → helper returns.

End state: value present on master A, member absent on master B. No later step
repairs it:

- another listing never iterates the name, so the helper is never called for it;
- an overwrite through the filer goes `CreateEntry` → `FindEntry` finds the old
  entry (`weed/filer/filer.go:241-243`) → `f.UpdateEntry` (`:276-277`) →
  `store.UpdateEntry`, which is `SET` only (`universal_redis_store.go:92-95`);
- only delete-then-create, or the value vanishing and being inserted again,
  brings the member back.

Before this PR, step 4 led to `continue` and nothing else. The entry was missing
from that one listing and present in the next. The PR turns a stale read into a
write.

### Exposure

Both options default to false. `redis2` standalone and sentinel clients read
from the master. So the default configuration is not affected. The scaffold
documents `routeByLatency` (`weed/command/scaffold/filer.toml:290`) as a
supported key of `[redis_cluster2]`, and the code also honours `useReadOnly`.

### Worked remedy

A single-key transaction is legal in cluster mode and go-redis always sends it
to the slot master:

```go
// osscluster.go:1881, inside processTxPipeline
node, err := state.slotMasterNode(slot)
```

So the re-check can be made authoritative for all three client types without a
type assertion on `redis.UniversalClient`:

```go
// valueExistsOnMaster asks the master, never a replica: the answer decides
// whether an index member is destroyed.
func (store *UniversalRedis2Store) valueExistsOnMaster(ctx context.Context, path util.FullPath) (bool, error) {
	var exists *redis.IntCmd
	_, err := store.Client.TxPipelined(ctx, func(pipe redis.Pipeliner) error {
		exists = pipe.Exists(ctx, store.getKey(string(path)))
		return nil
	})
	if err != nil {
		return false, err
	}
	return exists.Val() > 0, nil
}
```

With that in place the stale-read sequence above ends differently: step 6 asks
master A, sees the value, and the member is restored. The cost is `MULTI` /
`EXEC` framing on one round trip, paid only for members whose `GET` already came
back empty.

This sketch is untested. It needs a run against a real cluster with replica
reads enabled, which this environment cannot provide.

## 3. F2 and F7 — collapse the two dead-member branches

### Current shape

```go
// :201-232, abridged
for _, fileName := range members {
	path := util.NewFullPath(string(dirPath), fileName)
	entry, err = store.FindEntry(ctx, path)
	lastFileName = fileName
	if err != nil {
		glog.V(0).InfofCtx(ctx, "list %s : %v", path, err)             // logs before classifying (F7)
		if err == filer_pb.ErrNotFound {
			store.removeOrphanedDirectoryListMember(ctx, dirListKey, path, fileName)   // guarded
			err = nil
			continue
		}
		break
	} else {
		if entry.TtlSec > 0 {
			if entry.Attr.Crtime.Add(time.Duration(entry.TtlSec) * time.Second).Before(time.Now()) {
				store.Client.Del(ctx, store.getKey(string(path))).Result()
				store.Client.ZRem(ctx, dirListKey, fileName).Result()                  // unguarded
				continue
			}
		}
		... eachEntryFunc ...
	}
}
```

Two branches decide "this member is dead". They are four lines apart, they
clean up differently, and only one is protected against a concurrent writer.

### The expiry branch has the same race, in a worse form

Take the second commit's scenario and move it down four lines:

1. Lister: `GET` returns an entry whose `Crtime + TtlSec` is in the past.
2. Writer: `InsertEntry` on the same path: `SET` (fresh value), `ZADD NX` (no-op, member present).
3. Lister: `DEL` removes the fresh value. `ZREM` removes the member.

The insert reported success and left nothing behind. If the writer's `ZADD NX`
lands after the lister's `ZREM` instead, the member survives and the value does
not, which is a new orphan. This race predates the PR. What the PR adds is the
inconsistency: one of two sibling paths is now defended and nothing in the code
says why the other is not.

The PR body notes that the expiry branch is reliably reachable for entries that
were updated after creation, because `SET ... EX` re-arms the Redis TTL while
`Crtime` stays put. So it is live code, not dead code.

### Code-judo proposal

Treat "logically expired" as a property of the lookup. The lookup drops the
expired value and reports not-found. One not-found path then owns index
cleanup.

```go
// findLiveEntry reports filer_pb.ErrNotFound both for a missing value key and
// for a value past its logical TTL, which it drops first.
func (store *UniversalRedis2Store) findLiveEntry(ctx context.Context, path util.FullPath) (*filer.Entry, error) {
	entry, err := store.FindEntry(ctx, path)
	if err != nil {
		return nil, err
	}
	if entry.TtlSec > 0 && entry.Attr.Crtime.Add(time.Duration(entry.TtlSec)*time.Second).Before(time.Now()) {
		store.Client.Del(ctx, store.getKey(string(path)))
		return nil, filer_pb.ErrNotFound
	}
	return entry, nil
}
```

```go
for _, fileName := range members {
	path := util.NewFullPath(string(dirPath), fileName)
	lastFileName = fileName

	entry, err = store.findLiveEntry(ctx, path)
	if err == filer_pb.ErrNotFound {
		store.dropDeadDirectoryListMember(ctx, path)
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

What this removes:

- the `else` arm and two levels of nesting;
- the second, unguarded cleanup implementation at `:216-217`;
- the `V(0)` line for the expected not-found case (F7), because the log now
  sits after classification and fires only for errors that break the loop.

What it changes in behaviour, deliberately: the expiry path now runs the
remove / re-check / restore sequence, so a writer whose `SET` lands after the
`DEL` gets its member back. It does not close the `DEL`-of-a-fresh-value half of
the expiry race; that needs a conditional delete on the value's own slot and is
a separate change. It should be stated as a known remainder in the helper's
comment.

The sketch is untested for the same reason as section 2.

## 4. F3 — `UpdateEntry` is a writer outside the protocol

### The invariant the helper relies on

The helper comment (`:242-244`) reasons about `InsertEntry` only. The implied
rule is: any write of a value key is followed by a `ZADD NX` of its member, so a
value that reappears after the `ZREM` will be re-indexed either by the writer
(if its `ZADD NX` comes after the `ZREM`) or by the helper (if the helper's
`EXISTS` sees the value).

`InsertEntry` (`:56-74`) follows that rule. `UpdateEntry` (`:92-95`) does not:

```go
func (store *UniversalRedis2Store) UpdateEntry(ctx context.Context, entry *filer.Entry) (err error) {
	return store.doInsertEntry(ctx, entry)
}
```

Before this PR nothing removed a member on the read path except the
barely-reachable expiry branch, so "the member is already there" was a safe
assumption for an update. The PR adds a remover that fires whenever a value is
absent.

### Interleaving

| # | Actor | Operation | Value | Member |
| --- | --- | --- | --- | --- |
| 1 | filer `CreateEntry` | `FindEntry` → found (TTL entry near expiry) | present | present |
| 2 | Redis | TTL fires | absent | present |
| 3 | lister | `GET` → Nil | absent | present |
| 4 | lister | `ZREM` | absent | absent |
| 5 | lister | `EXISTS` → 0, helper returns | absent | absent |
| 6 | filer `CreateEntry` | `UpdateEntry` → `SET` | present | **absent** |

At step 5 the helper is right: there is nothing to keep. Step 6 then writes a
value and no member. Before the PR, steps 4-5 did not happen and step 6 produced
a consistent entry.

Callers that go `FindEntry` → `UpdateEntry`: `Filer.CreateEntry`
(`weed/filer/filer.go:241-243`, `:276-277`), the file-to-directory promotion in
`ensureParentDirectoryEntry` (`:367`), and the gRPC handlers in
`weed/server/filer_grpc_server.go` (`:477`, `:582`, `:612`, `:674`).

### Severity, stated plainly

Narrow. Steps 2 through 5 all have to fall between a `FindEntry` and the
following `UpdateEntry`, which are normally microseconds to milliseconds apart.
For a TTL entry the invisible value also expires again after `TtlSec`. This is a
residual risk, not an expected incident. The reason it is a finding is
structural: the commit message already identifies `UpdateEntry` as the reason
the loss persists ("UpdateEntry does not re-add it, so the loss persisted") and
then builds a protocol that leaves `UpdateEntry` out of it.

### Options

Make the invariant true:

```go
func (store *UniversalRedis2Store) UpdateEntry(ctx context.Context, entry *filer.Entry) (err error) {
	return store.InsertEntry(ctx, entry)
}
```

This deletes the Insert / Update distinction in this store. `doInsertEntry`
can then be inlined into `InsertEntry`. The cost is one `ZADD NX` per update,
which is O(log N) on the server and one more round trip unless `SET` and
`ZADD NX` are pipelined (a plain pipeline is split per node by the cluster
client, so it works for all three client types).

Or keep updates at one `SET` and write the reliance down at `UpdateEntry`, so
the next person who touches the remover knows there is a writer that does not
re-index.

Summary question Q2 asks the maintainers which they prefer.

## 5. F4 — helper boundary and error handling

### Signature

```go
removeOrphanedDirectoryListMember(ctx, dirListKey string, path util.FullPath, fileName string)
```

- `dirListKey` is a physical key: already through `store.getKey` at `:178`.
- `path` is a logical path: the helper applies `store.getKey` itself at `:245`.
- `fileName` is the last element of `path`.

Three parameters, two conventions, one of them redundant. Every other method in
the file takes logical paths and derives physical keys inside
(`InsertEntry:62-68`, `DeleteEntry:121-136`, `DeleteFolderChildren:151-166`).
The PR body spends a paragraph on why an unprefixed key would be a silent no-op.
The signature makes that the caller's problem, and the only other caller so far,
the test at `universal_redis_store_test.go:112`, has to spell out
`store.getKey(genDirectoryListKey(string(dir)))` to call it correctly.

### Control flow and swallowed errors

```go
exists, err := store.Client.Exists(...).Result()
if err == nil && exists == 0 {
	return
}
store.Client.ZAddNX(ctx, dirListKey, redis.Z{Score: 0, Member: fileName})
```

Outcomes after a successful `ZREM`:

| `EXISTS` result | Action | Self-healing if the action fails? |
| --- | --- | --- |
| 0, no error | leave removed | n/a |
| ≥1 | restore member | **no** — live value, no member, until the path is inserted again |
| error | restore member | yes if the value is really gone (orphan again, next listing retries); **no** if it is live |

The "error means restore" rule is correct and conservative, but it is encoded as
the negation of the early-return condition and is not mentioned in the comment.
The restore's own result is discarded. A failed restore with a live value is the
single outcome in this helper that does not repair itself, and it produces no
log line.

The restore also uses the listing request's `ctx`. If that context is cancelled
or past its deadline, `EXISTS` fails with the context error, control falls
through to `ZAddNX` with the same dead context, and the restore fails too. That
needs a concurrent writer to matter, so it is rare, but the fix is one call.

### Rewritten helper

Combining sections 2 and 5 (name and comment are suggestions):

```go
// dropDeadDirectoryListMember removes path's name from its parent's index, and
// puts it back if the value key turns out to exist once the member is gone. A
// writer that set the value while the member was still present had its own
// ZAddNX turn into a no-op, so nobody else would re-add it.
func (store *UniversalRedis2Store) dropDeadDirectoryListMember(ctx context.Context, path util.FullPath) {
	dir, name := path.DirAndName()
	dirListKey := store.getKey(genDirectoryListKey(dir))

	if err := store.Client.ZRem(ctx, dirListKey, name).Err(); err != nil {
		return // member untouched, the next listing retries
	}

	exists, err := store.valueExistsOnMaster(ctx, path)
	if err == nil && !exists {
		return
	}

	// Unknown counts as present. The restore must not die with the listing request.
	if err := store.Client.ZAddNX(context.WithoutCancel(ctx), dirListKey, redis.Z{Score: 0, Member: name}).Err(); err != nil {
		glog.ErrorfCtx(ctx, "restore %s in parent dir index: %v", path, err)
	}
}
```

`go.mod` declares `go 1.25.8`, so `context.WithoutCancel` (Go 1.21) is
available. Recomputing `dirListKey` costs one string concatenation per dead
member, next to two or three network round trips.

## 6. Observations that are not findings

**Round trips.** Each dead member now costs `GET` + `ZREM` + `EXISTS`, issued
one after another, where it used to cost `GET`. A page of `limit` dead members
is up to three times the round trips on the first listing after deployment, and
then the backlog is gone. `ZREM` is variadic, so the removals could be batched
after the loop and the re-checks pipelined. That would widen the remove-to-check
window and add bookkeeping to the loop, and the cost is one-time and bounded by
the page size, so it is not worth pushing for in this PR.

**Sibling stores.** `weed/filer/redis/universal_redis_store.go:180-195` has the
same not-found `continue` with no cleanup, on a `SET`-typed index. The PR body
discusses `redis3` and not this one. Neither `redis` nor `redis3` has a section
in the scaffold, so leaving them alone is defensible. It is noted here only so
the next reader does not assume the fix is store-wide.

**The cross-slot constraint.** The commit message's reason for not using Lua or
`MULTI` across both keys is correct for the cluster store. It does not rule out
a single-slot transaction on the value key, which is what section 2 uses.
