# Thermo-nuclear code quality review — `rclone/rclone#9699` "lib/batcher: prevent commits racing shutdown"

Range: `862ed2b7a..0b87bc478` (`git diff main...review-head`). The diff touches 2 files, +95/−0: `lib/batcher/batcher.go` (+6) and `lib/batcher/batcher_test.go` (+89).

## Verdict

**The fix is correct, but it does not meet the approval bar as written.** The PR closes a real channel-ordering race by adding a mutex that serializes `Commit` admission with `Shutdown`. I checked the claim directly. The package tests pass under `-race` at head. The new regression test fails, as intended, when run against the unfixed base code. I found no deadlock path.

The problem is shape, not behaviour. The new lock makes the old in-band quit-sentinel protocol unnecessary, yet the PR bolts the lock on top of that protocol. It leaves in place a comment whose only justification the PR itself has just invalidated. There is a visible code-judo move here: close the channel under the lock and delete the sentinel. I applied that move to a scratch copy and it passes the full suite, including the PR's own test. The regression test is effective today, but it relies on an undocumented log-formatting seam and checks for one particular interleaving rather than the actual invariant.

Neither file approaches a size threshold (288 and 364 lines).

## Findings

### 1. The lock makes the quit-sentinel protocol deletable, but the PR layers the lock on top of it and leaves a now-false comment

`lib/batcher/batcher.go:243-252` (with the sentinel at `:60` and `:208-211`). `Shutdown` still stops `commitLoop` by sending an in-band `{quit: true}` request, and the comment at `:248-250` explains why: closing `b.in` "will cause write to closed channel in Commit". This PR is exactly what makes that false. Every send on `b.in` now happens under `admitMu`, after checking `b.closed`, and `Shutdown` closes `b.closed` under the same lock. So `Commit` can no longer write to a closed `b.in`.

What remains is two overlapping mechanisms: a mutex that does the real ordering, and a sentinel that needs a mode flag (`quit bool`) on every real `request` plus a special-case branch in `commitLoop`. The comment actively misleads the reader about which of the two provides safety.

The remedy is to replace the sentinel send with `close(b.in)` under `admitMu`. That lets you delete the `quit` field, the `if req.quit` branch and the stale comment, and have `commitLoop` exit on `req, ok := <-b.in; !ok`. Channel close semantics then guarantee that every admitted request is drained before the loop exits, which is exactly the invariant issue #9687 asked for. Any future unguarded send would panic loudly instead of reproducing today's silent hang or lost upload. `Shutdown` also stops blocking on a full buffer while holding the lock.

I verified this in a scratch copy through `go test -overlay`: `go test -count=3 -race` passed the whole package. Once `b.closed` is only read under the lock, it could also become a plain `bool` (reasoned, not executed). Full diff and reasoning: `01_batcher_shutdown_protocol.md`, Finding 1.1.

### 2. `Commit` threads manual Lock/Unlock through its body, with an early-return unlock and unrelated work inside the critical section

`lib/batcher/batcher.go:267-281`. The admission critical section is spread across `Commit`: a `Lock` at the top, an extra `Unlock` in the `select` early-return branch, and a final `Unlock` after the send. Any future early return added in between leaks the lock and deadlocks both later `Commit` calls and `Shutdown`. The locked region also contains `fs.Debugf`, which formats the message and calls the backend `Fs`'s `String()`, and a `make(chan …)`, neither of which is part of admission.

The remedy is to extract `func (b *Batcher) admit(req request) error` whose entire body is `Lock; defer Unlock; check closed; send`, documented with the invariant it protects. `Commit` then only builds the request, calls `admit`, and waits on the response. This was verified as part of the same scratch redesign in finding 1. Worked code: `01_batcher_shutdown_protocol.md`, Finding 1.2.

### 3. The regression test depends on where a debug log line sits, and asserts only one of the two legal outcomes of the race

`lib/batcher/batcher_test.go:22-35`, `:239-311`, which depends on `batcher.go:274`. The test pauses `Commit` mid-admission by setting the process-global log level to Debug and passing a `blockingStringer` as the batcher's logging identity. It relies on `fs.Debugf` inside `Commit` calling `String()` between the closed-check and the send. Nothing on the production side records that dependency.

I moved the log line out of the critical section, which is a behaviour-preserving cleanup that finding 2 would naturally invite. Both subtests then failed at `:295` with `batcher is shutting down`, even though that is the correct result for a commit that loses the race. Moving the log line after the send instead would make the test pass on the unfixed code too (reasoned from control flow). The deeper issue is the assertion: `require.NoError` treats one interleaving as the only correct result, while the real contract is "processed or explicitly rejected, never hung or dropped". The test also mutates global config mid-test, and on fixed code its 100 ms wait always runs out, so it acts as a fixed sleep.

The remedy is to assert the either/or invariant instead. Add a comment at the production `Debugf` (or inside `admit`) naming the test that depends on it. Consider adding a bounded, hook-free stress test modelled on the maintainer's 8-concurrent-Commit reproduction. Details and verification: `02_regression_test.md`, Finding 2.1.

## Proposed remediation sequence

1. Extract `admit()` with `defer`-based unlocking (finding 2). This is mechanical, and it gives the invariant one home and one comment.
2. Inside that structure, replace the quit sentinel with `close(b.in)`. Delete `request.quit`, the `commitLoop` sentinel branch and the stale Shutdown comment (finding 1). Optionally turn `b.closed` into a lock-guarded `bool`, and optionally use `sync.RWMutex` so concurrent admissions don't serialize and a waiting `Shutdown` takes priority (see the "considered" notes in `01`).
3. Relax `TestBatcherCommitRacingShutdown` to assert "processed or rejected, never hung or dropped". Annotate the production `Debugf` seam, and add a bounded concurrent stress test (finding 3).

Steps 1 and 2 together are a net deletion from `batcher.go`, and they make the fix's guarantee follow from channel semantics instead of from careful ordering. The PR's existing test already passes against that version.

## Detail files

- `01_batcher_shutdown_protocol.md`: correctness verification of the fix, findings 1 and 2 with the verified scratch diff, and points considered but not raised (mutex fairness, `ctx` handling, file size, the drain-only alternative).
- `02_regression_test.md`: test effectiveness check against the base code, and finding 3 with the executed counter-example and remedies.

## Verification summary

- `go test -count=1 -race ./lib/batcher` at head: pass.
- New test against base `batcher.go` (overlay): sync fails "commit hung while racing shutdown", async fails "accepted commit was dropped during shutdown". The test is effective.
- Proposed close-`b.in` + `admit()` redesign (overlay), `go test -count=3 -race`: pass.
- Same redesign with `Debugf` moved outside the lock (overlay): the new test fails with `batcher is shutting down`. This confirms finding 3's coupling.
- The clone's working tree was clean after every run. All scratch files are under `clone-work/scratch/`.
