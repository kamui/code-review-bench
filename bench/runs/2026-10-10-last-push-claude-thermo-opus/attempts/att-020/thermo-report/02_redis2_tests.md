# 02 — redis2 tests: `universal_redis_store_test.go`

Subject: `weed/filer/redis2/universal_redis_store_test.go` at `6c8fde664`, new file, 143 lines.
Carries the evidence for summary Finding 5.

## What the file contains

Four helpers (`newTestStore`, `insertTestEntry`, `listNames`, `indexMembers`) and three test functions, one of which has two sub-cases:

- `TestListDirectoryEntriesRemovesOrphanedIndexMembers` with `keyPrefix` empty and `"sw:"`: insert two entries, `DEL` one value key directly, list, assert both the listing and the raw index.
- `TestRemoveOrphanedDirectoryListMemberKeepsRecreatedEntry`: insert an entry, call the helper on it while the value is present, assert the member is still there.
- `TestListDirectoryEntriesRemovesIndexMembersExpiredByRedis`: insert with `TtlSec: 1`, sleep 1.5 seconds, assert Redis dropped the value, list, assert the index is empty.

## Verification: the tests do not run

Command, from the clone root with the offline environment from the execution policy:

```
go test -count=1 -v ./weed/filer/redis2
```

Output (trimmed to the result lines):

```
--- PASS: TestListDirectoryEntriesRemovesOrphanedIndexMembers (0.00s)
    --- SKIP: TestListDirectoryEntriesRemovesOrphanedIndexMembers/keyPrefix= (0.00s)
    --- SKIP: TestListDirectoryEntriesRemovesOrphanedIndexMembers/keyPrefix=sw: (0.00s)
--- SKIP: TestRemoveOrphanedDirectoryListMemberKeepsRecreatedEntry (0.00s)
--- SKIP: TestListDirectoryEntriesRemovesIndexMembersExpiredByRedis (0.00s)
PASS
ok  	github.com/seaweedfs/seaweedfs/weed/filer/redis2	0.024s
```

The gate is lines 19–21 of the test file. A search of the repository for `RUN_REDIS_TESTS` finds only those two lines; a search of `.github/` for `redis` finds nothing. There is no workflow that starts a Redis service for the Go unit tests. The only evidence that the tests pass is the author's local run quoted in the PR body, which covers two of the three functions (the third arrived with the second commit and has no reported run) and cannot be repeated by CI.

The convention the PR cites is real: `weed/filer/tarantool/tarantool_store_test.go:16` gates on `RUN_TARANTOOL_TESTS`. That test delegates to the shared `store_test.TestFilerStore` harness and needs a real database to mean anything. The situation here differs because the logic under test is command sequencing in Go, and the store reaches Redis through an interface.

## Verification: a hermetic seam exists and needs no new dependency

`UniversalRedis2Store.Client` is `redis.UniversalClient`. go-redis v9 lets a `redis.Hook` wrap `Process`; a hook that fills in the command's result and returns without calling `next` means no connection is ever opened. The scratch fake used for the experiments in `01_redis2_index_cleanup.md` is built this way:

```go
func (f *fakeRedis) ProcessHook(next redis.ProcessHook) redis.ProcessHook {
	return func(ctx context.Context, cmd redis.Cmder) error {
		args := cmd.Args()
		name, key := strings.ToLower(args[0].(string)), args[1].(string)
		switch name {
		case "get":
			if v, ok := f.values[key]; ok {
				cmd.(*redis.StringCmd).SetVal(v)
			} else {
				cmd.SetErr(redis.Nil)
			}
		case "exists": ...
		case "set": ...
		case "del": ...
		case "zadd": ...
		case "zrem": ...
		case "zrangebylex": ...
		}
		if f.after != nil {
			f.after(name, key)
		}
		return cmd.Err()
	}
}
```

with the store constructed as

```go
client := redis.NewClient(&redis.Options{Addr: "127.0.0.1:1"})
client.AddHook(f)
store := &UniversalRedis2Store{Client: client}
store.loadSuperLargeDirectories(nil)
```

Status: verified by execution. Three tests driving `InsertEntry`, `UpdateEntry`, `FindEntry` and `ListDirectoryEntries` through this fake ran in 0.02s in an environment with no Redis server and no network. `miniredis` is not in `go.mod` and is not needed; `github.com/stvp/tempredis` is in `go.mod` but, as the PR body notes, depends on a `redis-server` binary.

The fake is about seventy lines. It does not model `ZRANGEBYLEX` bounds or `LIMIT`, nor TTLs; a version meant to stay in the tree should model the lexical range at least, since the paging arguments are the other half of `ListDirectoryEntries`.

## Coverage of the second commit

The second commit's claim is that a concurrent `InsertEntry` between `FindEntry` and the `ZREM` no longer loses the member. The only test added for it, lines 105–121, does this:

```go
insertTestEntry(t, store, path, 0)
store.removeOrphanedDirectoryListMember(context.Background(), store.getKey(genDirectoryListKey(string(dir))), path, "recreated")
```

The value and the member both exist before the helper runs. That is the ordering "insert completed, then `ZREM`", and it would also pass for an implementation that checked `EXISTS` first and never removed anything. It does not exercise:

- an insert whose `SET` lands between the helper's `ZREM` and `EXISTS`;
- an `UpdateEntry` after the `EXISTS` (the case that fails, see detail file 01, experiment B);
- an `EXISTS` error, where the helper is meant to restore the member;
- a `ZADD NX` failure on the restore;
- the helper being reached through `ListDirectoryEntries` rather than called directly.

With the `after` callback on the fake each of these is a few lines: set the callback to perform the write when it sees the command of interest, run the listing, clear the callback, list again and compare.

## Smaller points

`time.Sleep(1500 * time.Millisecond)` at line 126 makes this the slowest test in the package by three orders of magnitude when it does run. It is the right thing for the one end-to-end case whose purpose is to show that Redis' own expiry beats the logical check; it should not be the pattern for the rest.

The assertion idiom `len(x) != 1 || x[0] != "name"` appears four times and `len(x) != 0` twice. The shared harness in `weed/filer/store_test/test_suite.go` uses `testify/assert`; either `assert.Equal(t, []string{"alive"}, names)` or `slices.Equal` would state the expectation once.

`newTestStore` registers cleanup that calls `DeleteFolderChildren` and `DeleteEntry` and ignores both errors. Against a shared server a failed cleanup leaves `/redis2_test_<nanos>` keys and a member in the root index behind with no signal. Logging the error with `t.Logf` is enough.
