# Thermo-nuclear code quality review — rclone/rclone#9699 "lib/batcher: prevent commits racing shutdown"

Range: `862ed2b7a..0b87bc478` (`main...review-head`), 2 files, +95 / −0.
Files: `lib/batcher/batcher.go` (now 288 lines), `lib/batcher/batcher_test.go` (275 → 364 lines).
No file gets close to the 1k-line threshold.

## Verdict

The fix is correct and small. `admitMu` makes the closed-check and the channel send in
`Commit` atomic with respect to `Shutdown`'s close-and-sentinel step. Every accepted request
now lands ahead of the quit marker and gets processed, and late commits get the existing
"batcher is shutting down" fatal error. I found no deadlock path, because `commitLoop` never
takes the lock. The package's race tests pass on the head.

It is not the simplest version of this fix, though. Once admission is serialized with
shutdown, the old workaround of a separate `closed` channel plus a `quit` sentinel request
(kept only because `b.in` "couldn't be closed") is no longer needed, and the PR leaves a
comment in place that now contradicts the code. There is a verified code-judo rewrite that
deletes the sentinel, turns the channel-used-as-flag into a `bool`, and makes `close(b.in)`
the shutdown signal. I would push for it before calling the design settled. The regression
test is clever, but it depends on a debug-log call site, global log config, a private field
and a 100ms timer.

Overall: **approve-with-changes on structure.** Neither finding is a correctness blocker. F1
is a clear missed simplification that also leaves a misleading comment behind.

## Findings

### F1. The mutex is bolted onto the old shutdown protocol instead of letting it collapse (medium)

In `lib/batcher/batcher.go`, the PR adds `admitMu` and wraps both the `closed`-channel check
in `Commit` (lines 267–281) and the `close(b.closed)` plus `b.in <- request{quit: true}`
sequence in `Shutdown` (lines 243–252). That fixes the race, but the batcher now has three
overlapping pieces expressing "no longer accepting work". The first is a `closed chan
struct{}` whose only production read now happens under the mutex, so it is really a `bool`.
The second is a `quit` field on every `request`, with a sentinel send. The third is a comment
at lines 246–250 claiming `b.in` must not be closed because `Commit` could write to a closed
channel. That comment is exactly the hazard `admitMu` now rules out, so it argues against the
code next to it. The code-judo move is to let the lock pay for itself. Replace `closed` with a
`bool` guarded by a `sync.RWMutex`. Admitters take `RLock` across the check-and-send in a small
`admit` helper with `defer`, which also removes the two hand-written unlocks that a future
early return could miss and wedge the batcher. `Shutdown` takes `Lock`, sets the flag and calls
`close(b.in)`. `commitLoop` exits on `req, ok := <-b.in; !ok`. The `quit` field, the sentinel,
and the stale comment are all deleted, and `Shutdown` no longer blocks under the lock waiting
for buffer space. I applied this rewrite via a scratch `-overlay` and the full `./lib/batcher`
suite passes under `-race`, including the PR's new regression test. The worked diff and the
behavior-preservation argument are in `01_batcher_admission.md`.

### F2. The regression test pins behavior to a logging side effect and a timer (low–medium)

`TestBatcherCommitRacingShutdown` in `lib/batcher/batcher_test.go` (lines 239–311, with
`blockingStringer` at lines 22–35) pauses `Commit` by passing a `String()` method that blocks
as the batcher's logging identity. To make it work, it turns the process-global `LogLevel` up
to debug, so that the `fs.Debugf(b.f, …)` between the closed-check and the send formats it.
It then waits on the unexported `b.closed` channel with a 100ms fallback. On the fixed code,
that channel can never close while `Commit` holds the lock, so every subtest pays the full
100ms (measured: 0.10s each). If the `Shutdown` goroutine has not reached the lock within that
window on a loaded runner, the test would pass on unfixed code too. More importantly, if
anyone moves or removes that debug log line, the test quietly stops testing anything, because
nothing marks the log call as load-bearing. The remedy is to state those dependencies
explicitly. Add a comment at the `Debugf` (or in the `admit` helper from F1) naming the test
that relies on it, and replace the private-field `select` with a plain sleep. Also add a
black-box stress companion: several goroutines looping `Commit` against one `Shutdown`,
asserting each call either succeeds with its item committed or gets the shutting-down error,
within a deadline. That protects the actual invariant through the public API and survives
refactors. Details and verification status are in `02_batcher_test.md`.

## Remediation sequence

1. Adopt the F1 rewrite in `batcher.go`: `RWMutex` + `closed bool`, `admit` helper with
   `defer`, `close(b.in)` in `Shutdown`, `!ok` exit in `commitLoop`. Delete `request.quit`
   and the stale "don't close b.in" comment.
2. In the test, replace `case <-b.closed:` with a plain sleep and annotate the load-bearing
   `Debugf` call (F2). This is needed for step 1 anyway.
3. Add the public-API stress test from F2 so the "every accepted request is processed or
   rejected" invariant stays protected even if the log line moves.

## Verification performed

- `go test -count=1 -race -v -run 'TestBatcherCommitRacingShutdown|TestBatcherCommitShutdown' ./lib/batcher`
  on `review-head`: PASS (sync 0.10s, async 0.10s).
- F1 rewrite applied through `-overlay` to scratch copies in `clone-work/judo/`, then
  `go test -count=1 -race ./lib/batcher`: `ok … 5.344s`. The clone was verified unchanged
  (`git status --short` empty).

## Detail files

- `01_batcher_admission.md`: correctness analysis of the lock, F1 evidence, full worked diff, overlay verification.
- `02_batcher_test.md`: how the test's hook works (traced through `fs/log.go`), F2 evidence, remedies.
