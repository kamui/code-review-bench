# Admission and shutdown

## Scope and measurements

Reviewed the whole of `lib/batcher/batcher.go`, the committed diff, `lib/batcher/options.go`, the atexit implementation, and the relevant Dropbox/Google Photos construction, shutdown, logging identity, and batch callbacks. The production change is six added lines, with no removed lines. `git show main:lib/batcher/batcher.go | wc -l` reports 282; `wc -l lib/batcher/batcher.go` reports 288. The mutex stays within the canonical batcher layer; callers do not need a new public contract.

## Finding: release admission before waiting on the shutdown marker

In `lib/batcher/batcher.go:251–252`, `Shutdown` holds the new admission mutex while sending its quit marker to the bounded input channel. When a backend commit is stalled and that channel is full, admission has already closed, but every subsequent `Commit` blocks acquiring the mutex instead of returning the existing fatal shutdown error. This makes rejection depend on backend progress and can leave these callers blocked indefinitely; the same backpressure probe returns the error immediately on the base revision. Release `admitMu` immediately after `close(b.closed)` and enqueue the marker afterward: every earlier accepted request has already been sent under the mutex, and every later caller sees the closed channel, so the ordering fix remains intact without holding admission hostage to draining. Add regression coverage for a full queue with a blocked backend callback.

The shutdown critical section starts at line 243, closes `b.closed` at 245, sends at 251, and unlocks at 252. `Commit` starts by locking at 267, and its existing closed-channel rejection is at 268–271. `New` makes the input channel with capacity `opt.Size` at 107. The commit loop synchronously calls the user callback at 150, so it does not consume more queue entries while that callback is stalled. These facts make a full queue during a backend operation a normal backpressure condition, without any data race or hypothetical reentrant callback.

The worker never needs `admitMu`. The original fix correctly orders accepted sends ahead of the marker and releases the mutex before synchronous response waiting. The regression is specifically the shutdown send’s inclusion in the critical section: rejection becomes coupled to the downstream worker even after the shutdown signal has become visible.

## Reproduction and verification status

The review-only `TestReviewClosedAdmissionUnderBackpressure` constructs an async batcher with size one. Its first commit enters a callback that waits on a release channel. A second commit fills the input buffer. Shutdown then closes admission and cannot enqueue its marker. The probe waits for `b.closed`, calls a third commit, and allows 250 milliseconds for the shutdown error while the callback remains deliberately blocked. It then releases the callback, joins shutdown, and checks the error, keeping the experiment finite.

The submitted head fails with `Commit after b.closed was closed waited for backend drain instead of rejecting`. The base passes in 0.00 seconds. Moving the unlock before the marker send also passes in 0.00 seconds. The fixed overlay subsequently passed the full race-enabled package. This is a verified behavioral regression under controlled backpressure; an indefinitely stalled real backend was not exercised.

The async fixture makes it easy to fill the channel from one goroutine. The same admission lock and closed check run before the sync/async branch, so late synchronous callers also suffer the delay. A sync fixture could use concurrent calls for the active and queued requests; it does not require a different production remedy.

## Worked code-judo proposal

Make the shutdown state transition the end of admission coordination:

```go
b.admitMu.Lock()
close(b.closed)
b.admitMu.Unlock()
b.in <- request[Item, Result]{quit: true}
b.wg.Wait()
```

An accepted caller held the same mutex through its send, so acquiring the mutex in shutdown proves all already accepted sends have completed. Any caller that acquires the mutex after closure rejects before sending. Therefore no request can overtake the shutdown marker, even though the marker is sent outside the mutex. The marker’s wait for queue space remains part of shutdown’s intended drain, while rejection no longer needs that drain to progress. `shutOnce` continues to protect the single closure and marker insertion. No extra helper, branch, state, or public interface is needed.

This proposal was tested as `review-scratch/proposed.go`; it was not applied to the checkout. The report does not recommend a whole-method deferred unlock in `Commit`, since that would hold admission while waiting for synchronous batch results and could prevent a batch from filling.

There is also a broader channel-based simplification now that all producers are serialized. Within the shutdown lock, close `b.closed` and `b.in`, then unlock and wait. Delete `request.quit`, and change the worker receive to `req, ok := <-b.in` with termination when `!ok`. Closed buffered channels drain their preceding items before reporting exhaustion, and late producers reject under the mutex before sending. This would eliminate the quit marker and its queue-space wait, and supersede the old comment warning about unsynchronized sends to a closed channel. It is a plausible private-protocol cleanup, assessed by source reasoning only, not an additional finding or a runtime-verified proposal. The narrower unlock move already addresses the observed regression with less change.

## Commands and evidence

All test commands ran from the clone root with this prefix:

```sh
env GOMODCACHE=/home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high/att-028/clone-cache/gomodcache GOCACHE=/home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high/att-028/clone-cache/gocache GOFLAGS=-mod=mod GOPROXY=off GOTOOLCHAIN=local timeout 300s
```

In the following commands, `$scratch` denotes `/home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high/att-028/clone-work/review-scratch`. This notation abbreviates the report; the executed commands used literal absolute paths.

```sh
go test -count=1 -race ./lib/batcher
go test -count=1 -race -v -overlay=$scratch/head.json -run '^TestReview' ./lib/batcher
go test -count=1 -race -v -overlay=$scratch/base.json -run '^TestReview' ./lib/batcher
go test -count=1 -race -v -overlay=$scratch/proposed.json -run '^TestReview|^TestBatcherCommitRacingShutdown$' ./lib/batcher
go test -count=1 -race -overlay=$scratch/proposed-repaired.json ./lib/batcher
```

The unmodified package passes in 5.344 seconds. The head probe command exits 1 solely for the backpressure assertion, with the debug regression subtests passing. The base probe command exits 0 in 1.228 seconds; its misleading debug regression passes are analyzed in the second detail file. The narrowed-lock proposal command exits 0 in 1.431 seconds. The combined production and test proposals pass the complete package in 5.548 seconds.

`head.json` overlays only the test file with the original test source plus the two review-only probes. `base.json` additionally overlays production with the blob from `main`. `proposed.json` overlays production with the head source with its shutdown unlock moved before the marker send. `proposed-repaired.json` uses that same production proposal and the test logging repair detailed in `02_regression_tests.md`. The overlay files and source copies remain under the work directory for reproduction.

## Review boundaries

The existing synchronous callback invocation, response channel behavior, and batching-off precondition were inspected but are not introduced by this PR and are not reported as regressions. The new mutex does cover the debug formatting call; the real Dropbox and Google Photos identity methods format immutable identifying fields, so source inspection did not establish a concrete reentrancy defect worth an extra finding. No public backend contract, new policy layer, giant-file crossing, or branching growth was found.
