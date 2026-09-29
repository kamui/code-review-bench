# 02 redis2 tests: weed/filer/redis2/universal_redis_store_test.go

Scope: new file, 143 lines, three tests plus four helpers.

## Commands run and result

`go test -count=1 -v ./weed/filer/redis2` with the offline env: PASS, but every test reports SKIP ("redis2 tests are disabled..."). `RUN_REDIS_TESTS` is unset and no Redis is reachable here. A grep of .github for RUN_REDIS_TESTS, redis-server or a redis service found nothing. Verification status: compiled and skipped; test bodies not executed.

## Finding 3: guard never runs by default; branch coverage thin

1. Opt-in gating means the normal `go test ./weed/...` and CI runs never exercise the fix. The convention cited (tarantool, foundationdb) is real, but it means this cleanup logic, including a concurrency compensation, has no default regression guard. Options: start a redis service in an existing workflow and set RUN_REDIS_TESTS=1 there; or add an in-process fake (miniredis is not in go.mod, so that would be a new test dependency).
2. TestRemoveOrphanedDirectoryListMemberKeepsRecreatedEntry calls the private helper with hand-built arguments (`store.getKey(genDirectoryListKey(...))`). It proves the helper's present-value branch, but not that ListDirectoryEntries still routes through it. A listing-level variant would seed value plus member and assert the member survives a listing that hits a not-found path; hard to make deterministic, so a hook or small interface for the client would be needed, which is a reason to keep the helper narrow.
3. Untested branches: ZREM error return, EXISTS error restore, ZAddNX failure. A fake client (the store holds `redis.UniversalClient`) that injects errors per command would cover them and would pin down Finding 1.
4. TTL test uses `time.Sleep(1500ms)` then asserts the key is gone. Prefer polling EXISTS with a deadline so it is not flaky on slow Redis expiry cycles (Redis active-expire is not guaranteed at the exact millisecond, though GET treats an expired key as absent).
5. newTestStore's cleanup ignores errors from DeleteFolderChildren/DeleteEntry; acceptable for tests but a leak would be silent on a shared instance.

### Worked proposal

Add a small `failingClient` wrapper embedding redis.UniversalClient overriding ZAddNX/Exists/ZRem to return errors, use it to assert (a) ZRem error: nothing else called, (b) Exists error: ZAddNX called, (c) ZAddNX error: logged/not panicking. These run with miniredis or a Redis service, and the (c) case pins the logging fix from Finding 1.
