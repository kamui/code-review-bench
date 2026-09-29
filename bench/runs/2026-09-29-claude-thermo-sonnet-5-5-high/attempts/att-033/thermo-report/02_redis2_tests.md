# 02 — redis2 tests

File: `weed/filer/redis2/universal_redis_store_test.go` (143 lines, new). Commands: `git diff main...review-head --stat`; `grep -rn "RUN_REDIS_TESTS\|redis-server\|redis:" .github/workflows` returned no matches. Verification: static reading; the tests skip in this environment because no Redis is available, and I did not run them.

## Finding 4 — opt-in gating, and the restore test does not test the race

Every test goes through `newTestStore` (lines 16-45), which skips unless `RUN_REDIS_TESTS=1`. No workflow sets that variable, so in CI these tests always skip. The two orphan-removal tests (lines 83-104 and 123-143) are good end-to-end checks of the reported behavior, including the keyPrefix matrix, and depend on a 1.5 s sleep that real Redis needs (the physical TTL has one-second granularity).

`TestRemoveOrphanedDirectoryListMemberKeepsRecreatedEntry` (lines 106-121) calls the helper on a live entry. An implementation that never removes anything passes this test, so it detects only removal of the restore step in the specific shape of "ZREM then always fail to restore", not the interleaving. It also couples the test to an unexported helper name and argument list.

Worked proposal:

1. Add an in-process fake (miniredis) or a Redis service in the CI job, and drop the env gate for the fake-backed cases while keeping a live-Redis variant for TTL expiry, which miniredis models only with explicit `FastForward`.
2. Drive the race deterministically with a `redis.Hook` on the client that, when it sees the ZREM for the target member, first runs `store.InsertEntry` for the same path (value SET then no-op ZAddNX), then lets the ZREM proceed. Assert that `ListDirectoryEntries` afterwards still returns the entry. This test fails if the restore is removed and passes with it, and it goes through the public API.
3. Split `newTestStore` into `connectRedis(t)` and `uniqueTestDir(t, store)` so gating, connection and fixture naming can be reused independently.
