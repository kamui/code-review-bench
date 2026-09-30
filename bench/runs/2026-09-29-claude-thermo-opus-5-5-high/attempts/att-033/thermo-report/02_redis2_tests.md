# 02 — redis2 tests (`weed/filer/redis2/universal_redis_store_test.go`)

Scope: the new 143-line test file added by the PR.

## Measurements

Command (offline, from the clone root, using the attempt's dependency cache):

```
GOMODCACHE=<cache>/gomodcache GOCACHE=<cache>/gocache GOFLAGS=-mod=mod GOPROXY=off \
GOTOOLCHAIN=local go test -count=1 -v ./weed/filer/redis2
```

Result: the package builds, and all four test cases report `SKIP` with
`redis2 tests are disabled. Start a redis-server and set RUN_REDIS_TESTS=1 ...`. The
package reports `ok`.

CI check: `git grep -n -i redis -- .github` returns nothing, and
`git grep -n RUN_TARANTOOL_TESTS -- .github Makefile '*.yml'` returns nothing. No workflow
in the repository provides a Redis server or sets `RUN_REDIS_TESTS`.

`go.mod` does not contain `miniredis`. The only in-tree Redis test helper is
`github.com/stvp/tempredis`, which the PR body reports hangs on current `redis-server`.

---

## Finding 2.1 — The regression tests, including the one guarding the concurrency fix, run nowhere automatically, and the recreate test also passes against a no-op helper

**Evidence.** `newTestStore` (`universal_redis_store_test.go:16-45`) skips unless
`RUN_REDIS_TESTS=1` (`:19-21`), and nothing in `.github` sets it or starts Redis
(measurement above). The three tests are therefore documentation of a manual run the
author did against `redis-server 8.2.1`. They are not a guard. The most delicate part of
the PR is the restore-after-`ZREM` protocol added in the second commit, and it is exactly
the part a later "simplification" would remove. Nothing in CI would notice.

Separately, `TestRemoveOrphanedDirectoryListMemberKeepsRecreatedEntry` (`:106-121`) calls
the helper with the value key present and asserts that the member survives. That
assertion also holds if the helper does nothing at all. The test only pins the restore
behavior in combination with
`TestListDirectoryEntriesRemovesOrphanedIndexMembers`, which pins that the `ZREM`
happens. The PR body's claim that "it fails if the restore is removed" is true, but it
does not fail if the removal is removed. Stated plainly: the pair of tests covers the
protocol, and neither test covers it alone.

**Remedy.** In order of preference:

1. Add a Redis service container to an existing Go test workflow and set
   `RUN_REDIS_TESTS=1` for `./weed/filer/redis2/...`. That is a few lines of YAML, and it
   turns the three tests into real guards.
2. If CI cannot take a service, say so in the test file's skip message or in a package
   doc comment, so the next reader does not assume the concurrency guard is enforced.
3. Make the recreate test self-sufficient: assert that the member was removed *and* then
   restored, for example by running a `MONITOR`/keyspace check, or by first asserting the
   helper removes the member when the value is absent in the same test. That way a no-op
   helper fails it.

**Verification status:** skip behavior verified by running the package tests offline.
The CI absence was verified by `git grep`. The no-op-helper claim was verified by
reading the assertions at `:114-120`.

---

## Checked and not flagged

- **Fixture isolation:** each test uses a `time.Now().UnixNano()`-named directory and
  cleans it up in `t.Cleanup`. That is safe against a shared instance.
- **`TestListDirectoryEntriesRemovesIndexMembersExpiredByRedis`** sleeps 1.5 s for a 1 s
  TTL. Redis expires keys lazily on access, so the following `EXISTS` is deterministic.
  The test also asserts its own precondition (the value key is already gone), which is
  good practice.
- **Helper duplication inside the test file:** `indexMembers` rebuilds
  `store.getKey(genDirectoryListKey(...))` by hand. That follows from Finding 1.2 in
  `01_redis2_listing_cleanup.md` and is not a separate issue.
