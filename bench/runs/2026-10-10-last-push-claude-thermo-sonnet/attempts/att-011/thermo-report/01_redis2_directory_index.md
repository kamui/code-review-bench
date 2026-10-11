# Detail report 01: redis2 directory-index orphan cleanup

Scope: `git diff main...review-head` (db5a086d0..6c8fde664), two files: `weed/filer/redis2/universal_redis_store.go` (+17) and `weed/filer/redis2/universal_redis_store_test.go` (+143, new). No file approaches 1k lines (store is 259 lines, test 143), so the size bar is not in play.

## Measurements and commands

- `git diff main...review-head -- weed/filer/redis2/universal_redis_store.go` shows one new call at line 208 inside the `err == filer_pb.ErrNotFound` branch of `ListDirectoryEntries`, and a new method `removeOrphanedDirectoryListMember` at lines 237-251.
- `grep -n "ZRem" weed/filer/redis2/universal_redis_store.go` shows three ZREM sites: `DeleteEntry` (136), the logical-expiry branch inline (217), and the new helper (238). Only the new one has compensating logic.
- `wc -l weed/filer/redis2/*.go`: store 259 lines, test 143.
- I did not run the tests: no Redis server is available, so every test in the package skips (`RUN_REDIS_TESTS` gate). Findings below come from reading the code and reasoning through interleavings.

## Finding 1: the cleanup is a remove-then-compensate protocol, not an atomic operation, and its compensating step swallows errors

Evidence: `universal_redis_store.go:237-251`. The helper issues `ZREM`, then `EXISTS`, then conditionally `ZADD NX`. The result of the final `ZAddNX` is discarded, and an `Exists` error is deliberately treated as "present" and falls through to the restore.

Analysis of interleavings (writer = `InsertEntry`: SET then ZADD NX; lister = this helper after `FindEntry` returned not-found):

- Writer SET and ZADD both land before lister ZREM: ZREM drops the member, EXISTS sees the value, restore re-adds it. Correct.
- Writer SET lands before EXISTS, ZADD after ZREM: either ZADD NX or the restore wins. Correct.
- Writer SET lands after EXISTS: ZREM already ran, so the writer's own ZADD NX adds the member. Correct.

So the insert race that Greptile and CodeRabbit raised is closed, but the structure leaves three residual weaknesses, none of which are documented:

1. If the restore `ZAddNX` fails (timeout, failover, connection blip), the error is discarded and a live value is left with no index member. `UpdateEntry` does not re-add the member (commit 2's own message says "the loss persisted"), so this is the exact permanent-invisibility failure the second commit set out to prevent, now silent and merely less likely. The first-step `ZRem` error is also dropped, which is defensible (retry next listing); the restore error is not equivalent, because there is no next attempt that repairs it.
2. A concurrent `DeleteEntry` between the `EXISTS` (value present) and the restore `ZADD NX` leaves a member whose value is gone again. It self-heals on the next listing, so it is bounded, but the helper's comment claims only the insert scenario.
3. Between ZREM and restore there is a window in which a live entry is missing from the index; a concurrent listing in that window omits it. Transient, but it is a new visible inconsistency that did not exist before the PR for live entries.

Code-judo options considered:

- Make the check-and-remove atomic with a Lua script or WATCH/MULTI. Rejected by the author for cluster mode (value key and index key are in different slots), and that is correct; the same constraint blocks WATCH.
- Reorder `InsertEntry` to add the member first. This makes a mid-creation member observable to listings as an orphan, which is worse.
- Keep the shape but make the failure mode honest: log the restore error at least at `glog.V(0)` (the surrounding loop already logs per-orphan at V(0)), and extend the comment to state the delete-race and the transient window. Alternatively return the restore error out of the helper so the listing can surface it rather than silently continuing.

Recommendation: do not discard the restore error; at minimum log it, and document the residual races next to the helper. If the project wants real atomicity for redis2, that is a store-wide design question (hash-tagging the value key with its parent directory so one Lua script can run in cluster mode), which should be raised separately rather than patched with a compensating step.

Verification status: reasoning-verified from code; not executed (no Redis).

## Finding 2: two ad-hoc stale-member cleanups in one loop, with an awkward helper signature, instead of one eviction helper

Evidence: `universal_redis_store.go:208` (call into the helper), `:215-218` (the pre-existing inline `Del` + `ZRem` for logically expired entries), and the helper signature at `:237`: `(ctx, dirListKey string, path util.FullPath, fileName string)`.

The loop now has two branches that both mean "this index member is stale, remove it", implemented two different ways: one through a new named method that does ZREM plus an existence probe and restore, the other through two inline discarded calls. A reader must now hold two cleanup protocols in mind, and the safer one is applied only to the branch that was added. The expiry branch has the same race the PR fixed for the not-found branch: a concurrent `InsertEntry` that recreates the value after `FindEntry` and before the inline `DEL` has its fresh value deleted. That is arguably worse, because it destroys data rather than only an index member, and the PR's argument that a recreate is a real hazard applies there with more force. It is pre-existing, so it is not a regression, but the PR introduced the asymmetry and the opportunity to unify.

The helper signature also passes redundant data. `path` is `dirPath` joined with `fileName`, both of which the helper's caller already holds, and `dirListKey` is derived from `dirPath`. Passing the three loosely-related values lets a caller pass a mismatched trio; it also forces the helper to recompute `store.getKey(string(path))` that `FindEntry` just computed. Taking `(ctx, dirPath util.FullPath, fileName string)` and deriving the other two inside would be a narrower contract, at the cost of one extra `getKey` call.

Code-judo proposal: introduce one `dropStaleIndexMember(ctx, dirPath, fileName)` used by both branches, with the expiry branch calling it after its `Del`. That deletes the inline `ZRem` line, gives the loop a single concept ("stale member"), and gives the compensating logic exactly one home and one test target. If the expiry branch should not get the existence probe (it has just deleted the value, so the probe would always report absent and the restore would be skipped), the helper handles it naturally.

Verification status: read from code; the unfixed expiry-branch race is reasoned, not reproduced.

## Finding 3: the regression test for the concurrent-recreate fix is sequential, white-box, and unprefixed

Evidence: `universal_redis_store_test.go:106-121`, `TestRemoveOrphanedDirectoryListMemberKeepsRecreatedEntry`.

The test inserts an entry normally, then calls the private helper directly with a hand-built key. It exercises only the "value already exists when the helper runs" interleaving in one thread; it does not test that `ListDirectoryEntries` itself keeps a recreated entry, and it covers none of the restore failure paths in Finding 1. It also uses `keyPrefix == ""` only. The PR description says the key-prefix handling is the specific silent-no-op risk and runs the first test under both prefixes, but the new restore path builds its own `store.getKey(string(path))` for `EXISTS` and reuses `dirListKey` for the restore, and a prefix mistake there would pass this test. Looping over `[]string{"", "sw:"}` as the first test does would close that.

A second, structural note: every test in this file is gated on `RUN_REDIS_TESTS=1` and needs a live server, so none of the new coverage, including the concurrency fix, runs in ordinary CI. That follows existing repo convention and was disclosed, but it means a regression of this fix would not be caught automatically.

Recommendation: run the restore test under both prefixes; call it through `ListDirectoryEntries` (for example by seeding an orphan whose value is then recreated via a hook or ordering) if feasible; add the TTL sleep assertion's reliance (1.5 s sleep for a 1 s TTL) into a note that it is wall-clock dependent.

Verification status: read from code; tests not run (skip without Redis).

## Out of scope but noted

`weed/filer/redis3/universal_redis_store.go:163` has the same inline ZREM pattern, which the PR body explains it chose not to touch because redis3 is described as unsupported. Not raised as a finding.
