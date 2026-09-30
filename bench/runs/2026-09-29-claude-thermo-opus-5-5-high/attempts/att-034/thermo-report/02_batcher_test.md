# 02 — `lib/batcher/batcher_test.go`: the racing-shutdown regression test

Scope: the test half of `main...review-head`, +89 / −0. The file goes from 275 to 364 lines, so
there is no size concern.

## What the test does

`blockingStringer` (lines 22–35) is passed as the batcher's logging identity `f`. The test
sets the global `ci.LogLevel` to debug, so the `fs.Debugf(b.f, "Adding %q to batch", name)`
call in `Commit` (batcher.go:274) formats `b.f`. That calls `String()`, which closes `started`
and then blocks on `release`. This pauses `Commit` right after its closed-check and just
before its send. The test then restores the log level, starts `Shutdown` in a goroutine, and
waits up to 100ms for `b.closed` (lines 287–290) before releasing the commit. It then asserts
that the commit returns nil, `Shutdown` finishes, and the commit function ran.

Confirmed from source: `fs.Debugf` → `LogLevelPrintf`, which is gated on
`GetConfig(context.TODO()).LogLevel >= level` (fs/log.go:191–195) → `LogPrintf` →
`logSlogWithObject(level, o, ...)`, which stringifies `o`. So the hook is real, but it only
works because of the log gate.

## Finding F2 — the regression test pins behavior to a logging side effect and a timer

The trick is clever, and it does catch the bug on unfixed code (the maintainer's approval
reports verifying this). As a long-lived regression test, though, it depends on several
things the test does not own:

1. **Where a debug log line sits.** The pause only happens because `Commit` happens to call
   `fs.Debugf(b.f, …)` between the closed-check and the send. If that log line moves above
   the check, gets dropped, or stops formatting `b.f` (a routine logging cleanup), the test
   keeps passing on both fixed and unfixed code. It turns into a silent no-op instead of
   failing. Nothing in `batcher.go` marks that line as load-bearing.
2. **Global mutable config.** It flips the process-wide `fs.GetConfig(ctx).LogLevel` twice
   (lines 243–246 and 279) to reach the hook and then to stop `Shutdown`'s logging from
   re-entering it. This is safe today only because no test in the package uses `t.Parallel`
   and because of the particular happens-before chain through `started` and the `go`
   statement.
3. **An unexported channel.** `case <-b.closed:` (line 288) reads a private field. Any change
   to how shutdown is represented breaks the test. The simplification in 01 (a `bool` under an
   `RWMutex`) needed exactly that edit.
4. **A timing heuristic presented as determinism.** On the fixed code, `b.closed` can never
   close while `Commit` holds `admitMu`, so the `select` always falls through to the 100ms
   timer. Measured: each subtest takes exactly 0.10s (`--- PASS: …/sync (0.10s)`,
   `…/async (0.10s)`). More importantly, if the `Shutdown` goroutine has not reached
   `admitMu.Lock()` within 100ms (a slow or loaded CI runner under `-race`), the test releases
   the commit before shutdown even contends. On the *buggy* code it then passes trivially. The
   pause between check and send is deterministic. The ordering against `Shutdown` is not.

Remedy, in order of preference:

- Keep the test's structure, but make its dependencies explicit rather than incidental. Add
  a comment at the `fs.Debugf` in `Commit` (or in the proposed `admit` helper from 01)
  saying that `TestBatcherCommitRacingShutdown` relies on it running between the
  closed-check and the send. Replace `case <-b.closed:` with a plain
  `time.Sleep(100 * time.Millisecond)`. That is honest about what the wait is, and it stops
  the test depending on the representation.
- Add a small black-box stress companion, like the maintainer's reproduction in #9687:
  several goroutines calling `Commit` in a loop against one `Shutdown`, with the assertion
  that every call returns either nil (and the commit function saw the item) or the
  "shutting down" fatal error, all within a deadline. This touches only the public API, so it
  survives refactors of both the log line and the shutdown representation. It also covers
  the property the fix actually guarantees: every accepted request is processed or rejected.

Verification status: points 1–3 are source-proven. The 0.10s fixed cost in point 4 is
measured. The false-pass-under-scheduling-delay scenario in point 4 is reasoned from the code
and was not reproduced.

Severity: low–medium. The production fix does not depend on this, but a regression test that
can silently stop testing anything is a maintainability liability.
