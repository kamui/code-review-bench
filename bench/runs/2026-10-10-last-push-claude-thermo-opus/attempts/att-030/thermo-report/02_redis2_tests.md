# 02 — redis2 tests

Subject: `weed/filer/redis2/universal_redis_store_test.go` at head `6c8fde664`
(new file, 143 lines).

## 1. What the file contains and what was run

Four helpers (`newTestStore`, `insertTestEntry`, `listNames`, `indexMembers`)
and three tests:

| Test | Lines | Prefixes | Covers |
| --- | --- | --- | --- |
| `TestListDirectoryEntriesRemovesOrphanedIndexMembers` | 83-104 | `""`, `"sw:"` | value `DEL`ed out of band → listing skips it and removes the member |
| `TestRemoveOrphanedDirectoryListMemberKeepsRecreatedEntry` | 106-121 | `""` only | helper called with value and member both present → member survives |
| `TestListDirectoryEntriesRemovesIndexMembersExpiredByRedis` | 123-143 | `""` only | `TtlSec: 1`, 1.5 s sleep, Redis drops the value → listing empties the index |

Command run from the clone root with the offline Go environment:

```
go test -count=1 -v ./weed/filer/redis2
```

Result:

```
--- PASS: TestListDirectoryEntriesRemovesOrphanedIndexMembers (0.00s)
    --- SKIP: TestListDirectoryEntriesRemovesOrphanedIndexMembers/keyPrefix= (0.00s)
    --- SKIP: TestListDirectoryEntriesRemovesOrphanedIndexMembers/keyPrefix=sw: (0.00s)
--- SKIP: TestRemoveOrphanedDirectoryListMemberKeepsRecreatedEntry (0.00s)
--- SKIP: TestListDirectoryEntriesRemovesIndexMembersExpiredByRedis (0.00s)
PASS
ok  	github.com/seaweedfs/seaweedfs/weed/filer/redis2	0.019s
```

Verification status: the tests compile and vet clean. Their assertions were not
executed in this review, because no Redis server is available. The PR body's
"fail on master, pass with the change" output could not be confirmed or refuted
here. That output also lists three failing cases, which matches the first
commit; the recreate test from the second commit is not in it.

## 2. F5 — the tests do not run automatically anywhere

### Evidence

```go
// :19-21
if os.Getenv("RUN_REDIS_TESTS") != "1" {
	t.Skip("redis2 tests are disabled. Start a redis-server and set RUN_REDIS_TESTS=1 to enable, REDIS_ADDR defaults to 127.0.0.1:6379.")
}
```

Searches run:

- `grep -rn "RUN_REDIS_TESTS\|RUN_TARANTOOL_TESTS" .` excluding this test file:
  the only hits are `weed/filer/tarantool/tarantool_store_test.go:16-17` and
  `weed/filer/tarantool/readme.md:11`. Nothing sets `RUN_REDIS_TESTS`.
- `grep -rl -i redis .github`: no files. None of the 70 workflow files starts a
  Redis service.
- `go.mod` has no in-process Redis fake. The only Redis test helper is
  `github.com/stvp/tempredis` (`go.mod:74`), which the PR body reports as
  hanging on current `redis-server` builds.

### Why it matters

The package reports `ok` with every assertion skipped. A later change that
breaks the orphan cleanup, the prefix handling, or the restore path will pass
CI. The second commit is a concurrency repair made in response to review; of
everything in this PR it is the part most likely to be simplified away by
someone who does not see why three commands are needed, and it has the least
protection.

The tarantool precedent is real, and it has the same weakness. Matching it is
consistent with the repository; it is not the same as being tested.

### Remedy

A CI job (or a step in an existing Go test job) with a `redis` service
container, `RUN_REDIS_TESTS=1`, and `go test ./weed/filer/redis2`. The tests
already isolate themselves in a uniquely named directory and clean up, so they
are ready for that.

An in-process fake such as miniredis would make the tests hermetic and let the
TTL test advance a fake clock instead of sleeping 1.5 seconds. It is a new
module dependency, so it is the maintainers' call and is not pushed here. If
the suite stays on a real server, the sleep is the honest way to test Redis
expiry and should stay.

## 3. F6 — the recreate test covers one prefix

### Evidence

```go
// :106-112
func TestRemoveOrphanedDirectoryListMemberKeepsRecreatedEntry(t *testing.T) {
	store, dir := newTestStore(t, "")

	path := dir.Child("recreated")
	insertTestEntry(t, store, path, 0)

	store.removeOrphanedDirectoryListMember(context.Background(), store.getKey(genDirectoryListKey(string(dir))), path, "recreated")
```

The helper under test has its own prefix application, independent of the one
the first test exercises:

```go
// universal_redis_store.go:245
exists, err := store.Client.Exists(ctx, store.getKey(string(path))).Result()
```

The first test (both prefixes) covers `dirListKey` reaching `ZREM` with the
prefix. It never reaches a state where `EXISTS` returns 1, so it says nothing
about line 245. The recreate test does reach that state, with an empty prefix
only, where `getKey` is the identity.

### Mutation that survives

Replace `store.getKey(string(path))` with `string(path)` at line 245. With
`keyPrefix = ""` nothing changes. With `keyPrefix = "sw:"`, `EXISTS` looks up a
key that is never written, returns 0, and the restore never runs. All four test
cases pass (given a Redis to run them on), and prefixed deployments have the
first commit's race back.

This is the same class of mistake the PR body calls out as the reason for the
prefix matrix.

### Remedy

Run the recreate test over the same table. The two tests can share the loop:

```go
func forEachKeyPrefix(t *testing.T, run func(t *testing.T, store *UniversalRedis2Store, dir util.FullPath)) {
	for _, keyPrefix := range []string{"", "sw:"} {
		t.Run("keyPrefix="+keyPrefix, func(t *testing.T) {
			store, dir := newTestStore(t, keyPrefix)
			run(t, store, dir)
		})
	}
}
```

If the helper's signature changes as proposed in `01_redis2_store.md`,
section 5, the call at line 112 shrinks to
`store.dropDeadDirectoryListMember(ctx, path)` and the test no longer has to
rebuild a physical key by hand.

## 4. Observations that are not findings

**The recreate test stages the race instead of running it.** It puts the store
in the state a concurrent insert would leave (value present, member present) and
calls the private helper. That is a reasonable deterministic unit test of the
restore path, and the first test proves the listing calls the helper. Driving
the real interleaving through `ListDirectoryEntries` would need a go-redis hook
that injects an `InsertEntry` when it sees the `ZREM`; that is more machinery
than this PR needs.

**No test for the replica-read or `UpdateEntry` sequences** in
`01_redis2_store.md`, sections 2 and 4. Both need either a cluster with
replicas or an injected interleaving. If F1 is fixed by routing the re-check to
the master, the routing itself is the thing to assert, and that is a go-redis
property and not something a single-node test can show.

**Helpers are fine.** `newTestStore` builds the struct directly and calls
`loadSuperLargeDirectories(nil)`, which is the minimum the methods need. The
cleanup removes the child values, the child index keys and the directory's own
index key, under the same prefix the test used.
