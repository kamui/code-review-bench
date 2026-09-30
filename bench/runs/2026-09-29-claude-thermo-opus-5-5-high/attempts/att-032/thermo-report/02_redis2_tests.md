# 02 — `weed/filer/redis2/universal_redis_store_test.go` (new, 143 lines)

## Measurements and commands

- `go test -count=1 -v ./weed/filer/redis2`, run offline with the attempt's caches, passed with every case **skipped**. `RUN_REDIS_TESTS` is unset, and this environment has no Redis server. The results claimed in the PR body (fail on master, pass on the head, against `redis-server 8.2.1`) could not be reproduced here.
- `go vet ./weed/filer/redis2` and `gofmt -l weed/filer/redis2` are both clean.
- `grep -rn 'RUN_[A-Z_]*_TESTS' weed` shows that the only prior gate of this shape is `weed/filer/tarantool/tarantool_store_test.go:16`. The new gate, `RUN_REDIS_TESTS` with `REDIS_ADDR`, follows it.
- `go.mod` has no in-process Redis fake such as `miniredis`. `github.com/stvp/tempredis` is present, and the PR body reports that it hangs on current `redis-server` builds. So the live-server gate is a reasonable choice, and I am not raising a canonical-helper finding.

## Assessment

The tests are well aimed. They cover:

- orphan removal both with and without `keyPrefix`;
- the Redis-expired TTL path end to end;
- the recreate guard, tested directly at the helper, which is the only deterministic way to hit that window.

Each test works in a uniquely named directory and cleans up after itself, which makes it safe to run against a shared instance. The 1.5 s sleep is deterministic rather than flaky: an expired key is reported absent on access (lazy expiry), whether or not the active expiry cycle has run yet.

Two structural points, both already covered by Finding B in `01_redis2_store.md`:

1. **Hand-built keys.** Line 112 calls the helper with `store.getKey(genDirectoryListKey(string(dir)))`, and lines 76, 91 and 130 build keys the same way. The tests re-implement the store's key layout rather than asking the store for it. If the helper took only a path, and the store exposed `valueKey`/`dirListKey`, those four compositions would disappear. Tests and production would then share a single definition of the prefixing rule, which the `keyPrefix=sw:` subtest exists to protect.
2. **Repeated assertions.** The `len(x) != 1 || x[0] != "…"` pattern appears four times, and `len(x) != 0` twice. `slices.Equal(names, []string{"alive"})` would express each check in one call. This is a legibility nit and does not rise to a summary finding.

No other findings.
