# 02 — `lib/batcher/batcher_test.go`: `TestBatcherCommitRacingShutdown`

Scope: the 89 added test lines, meaning the `blockingStringer` type
(`batcher_test.go:22-35`) and `TestBatcherCommitRacingShutdown`
(`batcher_test.go:239-311`).

## What the test does

1. It sets the **global** `fs.GetConfig(ctx).LogLevel` to debug
   (`batcher_test.go:243-246`), so that `Commit`'s
   `fs.Debugf(b.f, "Adding %q to batch", name)` formats `b.f`.
2. It passes a `blockingStringer` as the batcher's logging identity `f`. The
   first `String()` call closes `started` and blocks until `release`. That
   pauses `Commit` inside the debug log, which sits after the closed check and
   before the send.
3. It resets the log level (`batcher_test.go:279`), starts `Shutdown` in a
   goroutine, then waits either for `<-b.closed` or 100 ms
   (`batcher_test.go:286-290`) before releasing the paused `Commit`.
4. It asserts that `Commit` returns no error, that `Shutdown` returns, and that
   the commit callback ran.

## Verification

- At head, `go test -count=1 -race ./lib/batcher` passes (5.35 s).
- With base `batcher.go` swapped in via `-overlay`, both subtests fail
  (`sync`: "commit hung while racing shutdown"; `async`: "accepted commit was
  dropped during shutdown"). The test genuinely detects the original race. It
  does so deterministically on the unfixed code, because there `Shutdown` is not
  blocked, so `b.closed` closes and the quit marker is queued before release.

## Finding 2.1 — The regression test is coupled to an incidental log line, global log-level state, and a private field, and its "contention" wait is a dead arm under the fix (verified by reading and running)

**Where:** `lib/batcher/batcher_test.go:22-35`, `243-246`, `279`, `286-290`.

**Problem.** The test works, and the reviewer at merge confirmed it fails
without the fix, which I reproduced. But its seam is magic. The only reason it
can pause `Commit` inside the race window is that `Commit` happens to call
`fs.Debugf(b.f, ...)` between the closed check and the send, that `fs.Debugf`
only stringifies `b.f` when the global log level is debug, and that nothing
before `Commit` stringifies `f` (the `sync.Once` swallows only the first call).
None of this is written down at the production site. `batcher.go:274` carries
no hint that a test depends on this line staying exactly where it is. Most
edits there fail loudly: deleting the log trips "commit did not reach the
admission point", and hoisting it above the check makes `require.NoError` fail.
But moving it below the send or the unlock turns the test into one that
passes without exercising the window at all.

Second, the "give Shutdown a chance to contend" step (`batcher_test.go:286-290`)
waits on the private `b.closed` channel **or** 100 ms. Under the fixed code,
`Shutdown` blocks on `admitMu` before `close(b.closed)`, so the `<-b.closed` arm
can never fire. The test always burns the full 100 ms per subtest, and it
cannot confirm that `Shutdown` actually reached the lock before `release`. The
arm only does anything on the buggy code. So the test body mixes a
deterministic regression detector with a timing guess whose success case is
unobservable. It also reaches into the private `closed` field, which ties the
test to exactly the representation that finding 1.1 recommends deleting.

Third, the test mutates the process-global `ci.LogLevel` twice: once via
`defer` restore, and once in the middle of the test to stop other log calls
from consuming the `sync.Once`. That is safe only because of a happens-before
chain through `release` and `b.in`, which a reader must reconstruct to be
convinced. Any future `t.Parallel()` in this package would turn it into a data
race.

**Remedy.**

- Make the seam explicit where it lives. Add a one-line comment above
  `fs.Debugf(b.f, "Adding %q to batch", name)` in `Commit` (or in the
  `enqueue` helper proposed in finding 1.1) saying that
  `TestBatcherCommitRacingShutdown` pauses here via `f.String()` and that the
  log must stay between the closed check and the send.
- Replace the `select { case <-b.closed: case <-time.After(100ms) }` with a
  plain, commented `time.Sleep(100 * time.Millisecond)`. The honest statement
  is "we cannot observe `Shutdown` waiting on the lock, so give it time". This
  also removes the test's dependency on the private field.
- Complement (not replace) the seam test with a short randomized stress test
  that does not depend on log placement. Start N concurrent `Commit`s racing one
  `Shutdown` for a few hundred iterations. Assert that each `Commit` either
  returns the shutdown error or has its item seen by `commitBatch`, and that
  nothing hangs past a deadline. This asserts the actual invariant ("every
  accepted request is processed or explicitly rejected") rather than one
  hand-picked interleaving, and it keeps protecting the property if the
  logging seam ever moves.

**Severity:** low-to-moderate test-maintainability issue. The test's value is
real (verified against the unfixed code); the concern is brittleness and
reliance on incidental structure, not that it is wrong today.
