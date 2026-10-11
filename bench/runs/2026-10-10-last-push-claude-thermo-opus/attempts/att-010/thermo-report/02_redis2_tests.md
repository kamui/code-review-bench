# 02 — redis2 tests

Scope: `weed/filer/redis2/universal_redis_store_test.go` (new, 143 lines), range `db5a086d0..6c8fde664`.

## What was added

Four helpers (`newTestStore`, `insertTestEntry`, `listNames`, `indexMembers`) and three tests:

| Test | Lines | What it asserts |
| --- | --- | --- |
| `TestListDirectoryEntriesRemovesOrphanedIndexMembers` | 83-104 | after an out-of-band `DEL` of one value key, listing returns the other entry and the index holds only it; run with `keyPrefix` `""` and `"sw:"` |
| `TestRemoveOrphanedDirectoryListMemberKeepsRecreatedEntry` | 106-121 | calling the helper on an entry whose value and member both exist leaves the member in place |
| `TestListDirectoryEntriesRemovesIndexMembersExpiredByRedis` | 123-143 | an entry with `TtlSec: 1` is gone from Redis after 1.5 s and listing then empties the index |

## Measurements and commands

```
$ go test -count=1 -v ./weed/filer/redis2        # offline, cached modules, no Redis
--- PASS: TestListDirectoryEntriesRemovesOrphanedIndexMembers (0.00s)
    --- SKIP: .../keyPrefix= (0.00s)
    --- SKIP: .../keyPrefix=sw: (0.00s)
--- SKIP: TestRemoveOrphanedDirectoryListMemberKeepsRecreatedEntry (0.00s)
--- SKIP: TestListDirectoryEntriesRemovesIndexMembersExpiredByRedis (0.00s)
ok  	github.com/seaweedfs/seaweedfs/weed/filer/redis2	0.025s

$ grep -rIl "RUN_REDIS_TESTS\|RUN_TARANTOOL_TESTS" . --exclude-dir=.git
./weed/filer/redis2/universal_redis_store_test.go
./weed/filer/tarantool/tarantool_store_test.go
./weed/filer/tarantool/readme.md

$ grep -rn "redis" .github/workflows
(no output)
```

Every test body skipped here, so nothing in this file was executed by this review. The PR body's claim that the three cases fail on the base and pass on the head is not independently confirmed.

## F5 — the race test cannot fail for the bug it is named after, and nothing runs any of it

**The gate.** `newTestStore` skips unless `RUN_REDIS_TESTS=1` (`:19-21`). No workflow, Makefile or script in the repository sets that variable; the only other match in the tree is the tarantool test this convention was copied from, and its variable is not set by any workflow either. The 143 lines therefore compile in CI and assert nothing. The parent test even reports `PASS` when both of its subtests skip. That is tolerable for a one-line fix whose behaviour is obvious; it is much less so after the second commit, whose entire content is an ordering argument about three Redis commands.

**The race test.** `TestRemoveOrphanedDirectoryListMemberKeepsRecreatedEntry` (`:106-121`) inserts an entry and then calls the private helper directly:

```go
insertTestEntry(t, store, path, 0)
store.removeOrphanedDirectoryListMember(context.Background(), store.getKey(genDirectoryListKey(string(dir))), path, "recreated")
```

That is a static state — value present, member present — which `ListDirectoryEntries` never hands to the helper, since it only calls it after `FindEntry` returned not-found. The property the commit is about is an ordering: the existence check must come after the `ZREM`. Rewrite the helper the racy way —

```go
if n, err := store.Client.Exists(ctx, valueKey).Result(); err == nil && n == 0 {
	store.Client.ZRem(ctx, dirListKey, fileName)
}
```

— and this test still passes, because with a present value the `ZREM` is simply never issued. The test pins "do not remove a member whose value exists", not "survive an insert that lands between the check and the removal". To reach the helper at all it also has to rebuild the prefixed index key by hand and pass a `(dirListKey, path, fileName)` triple that nothing keeps consistent, which is a small sign that the helper's signature mixes a physical key with a logical path.

**Worked proposal.** go-redis exposes command hooks on every client (`redis.Hook` in `redis.go:54-58`; `AddHook` is on `UniversalClient`, `universal.go:351`). A hook can place the concurrent insert at an exact point in the command stream, which turns the race into a deterministic test that goes through `ListDirectoryEntries`:

```go
type beforeCmdHook struct {
	name string
	once sync.Once
	do   func()
}

func (h *beforeCmdHook) DialHook(next redis.DialHook) redis.DialHook { return next }
func (h *beforeCmdHook) ProcessPipelineHook(next redis.ProcessPipelineHook) redis.ProcessPipelineHook {
	return next
}
func (h *beforeCmdHook) ProcessHook(next redis.ProcessHook) redis.ProcessHook {
	return func(ctx context.Context, cmd redis.Cmder) error {
		if cmd.Name() == h.name {
			h.once.Do(h.do)
		}
		return next(ctx, cmd)
	}
}

func TestListDirectoryEntriesKeepsEntryRecreatedDuringOrphanCleanup(t *testing.T) {
	store, dir := newTestStore(t, "")
	path := dir.Child("recreated")

	insertTestEntry(t, store, path, 0)
	if err := store.Client.Del(context.Background(), store.getKey(string(path))).Err(); err != nil {
		t.Fatalf("drop value key: %v", err)
	}

	// The insert lands after the listing's GET saw nothing and before its ZREM.
	store.Client.AddHook(&beforeCmdHook{name: "zrem", do: func() { insertTestEntry(t, store, path, 0) }})

	listNames(t, store, dir)

	if members := indexMembers(t, store, dir); len(members) != 1 || members[0] != "recreated" {
		t.Fatalf("directory index holds %v, want [recreated]", members)
	}
}
```

Against the head this passes: insert, `ZREM`, `EXISTS` = 1, restore. Against the first commit's unconditional `ZREM` it fails, and against the check-then-remove rewrite above it fails as well (`EXISTS` = 0, insert, `ZREM`). The same hook with `name: "exists"` covers the other interleaving. The hook's own insert issues `SET` and `ZADD`, not `ZREM`, so it does not re-enter. This sketch was written against the v9.21.0 hook API in the module cache and has not been compiled or run.

For the gate, the proportionate fix is a workflow step with a `redis` service container that sets `RUN_REDIS_TESTS=1` for `./weed/filer/redis2`; that is a few lines of YAML and no new module dependency. An in-process fake would also work but adds a dependency the repository does not have today (`go.mod` carries only `stvp/tempredis`, which the PR body correctly reports as unusable against current servers).

**Verification status.** The skip behaviour and the absence of any CI reference were observed directly (commands above). The claim that the current race test passes against a check-then-remove implementation follows from reading the test; it was not executed.

## Smaller observations (not findings)

- `TestListDirectoryEntriesRemovesIndexMembersExpiredByRedis` sleeps 1.5 s of wall-clock time for a 1 s TTL (`:128`). It is the only way to exercise real server-side expiry and the 500 ms margin is adequate; an out-of-band `DEL` is already covered by the first test, so this one is the end-to-end reproduction and is worth its cost.
- `newTestStore` builds the struct literally (`:34-35`) instead of going through `Redis2Store.initialize`. That keeps the test independent of configuration parsing and is fine, but it means any field added to the store for F1 (`primaryFor`) must tolerate its zero value.
- Each test uses a unique directory and removes it in `t.Cleanup` (`:37-42`), so running against a shared instance is safe, as the PR body says.
