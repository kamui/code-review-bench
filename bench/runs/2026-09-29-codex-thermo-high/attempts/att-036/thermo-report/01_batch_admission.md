# Batch admission and shutdown

## Scope and measurements

Reviewed the committed range `main...review-head` for `lib/batcher/batcher.go` and `lib/batcher/batcher_test.go`.

The diff adds 6 production lines and 89 test lines. At the reviewed head, `batcher.go` is 288 lines and `batcher_test.go` is 364 lines. Neither file is near the skill's 1,000-line decomposition threshold. The reviewed tree was clean at the outset (`git status --short --branch` showed only `## review-head`).

## Assessment

In `lib/batcher/batcher.go`, `Batcher.admitMu` protects the full admission operation in `Commit`: check `closed`, log, and send the request to `in`. `Shutdown` takes that same mutex before closing `closed` and sending the quit request. This creates a simple ordering rule: a commit that gets the lock first must enqueue before shutdown can enqueue quit; shutdown that gets it first closes admission before a later commit checks `closed`. The lock is released before synchronous callers wait for their result, so waiting for a batch response does not unnecessarily serialize other callers.

This is a focused and legible state-boundary fix. It does not scatter shutdown checks through the commit loop, add nullable state, or introduce a second queue protocol. The mutex is not an unnecessary abstraction: it directly expresses the mutual exclusion needed between request admission and shutdown-marker insertion. A more ambitious rewrite does not show a code-judo benefit here. Draining after quit would introduce a second rejection path and require the loop to interpret requests after its termination marker; a new state machine or admission helper would mainly relocate the same synchronization rule.

The regression test in `lib/batcher/batcher_test.go` uses `blockingStringer.String` as a pause during `Commit`'s debug log, then starts shutdown before releasing that pause. It runs in both `sync` and `async` modes, waits for commit and shutdown completion, and checks that the batch callback ran. The test does use a 100 ms contention opportunity rather than an explicit acknowledgement that the shutdown goroutine has reached the lock. That leaves a small scheduler dependency in how reliably the pre-fix interleaving is forced. However, the test synchronizes the commit at the critical admission point, observes `closed` if shutdown wins, and has bounded assertions for the hang and dropped-upload outcomes. I do not consider this a high-confidence defect warranting a finding; no production redesign or test rewrite is required by this review.

## Verification status

- Inspected `git diff --find-renames main...review-head` for both changed files and the surrounding implementation in `batcher.go`.
- Inspected line counts and initial worktree status.
- Did not execute tests, `go vet`, or builds. The review request did not ask for verification runs, and this review reports code inspection rather than independently reproducing the race.
- The PR packet records the author's test and build commands and an approval that says the regression test was run against the unfixed code; those are packet facts, not independent verification in this review.

## Worked code-judo proposal

Keep the current design. The tempting alternative is to add a drain/reject phase after receiving quit, but that retains admission-versus-quit ordering as an implicit protocol and adds loop behavior to handle requests that should never have been accepted. The shared mutex instead makes the existing two operations indivisible relative to each other. No helper extraction is warranted for the few lock/check/send lines, and moving them into a wrapper would obscure the boundary rather than simplify it.

## Remediation

None. There is no actionable maintainability regression in this change. If the test is later refactored, preserve a synchronization point between admission's closed check and enqueue; avoid replacing that point with a sleep-only race attempt.
