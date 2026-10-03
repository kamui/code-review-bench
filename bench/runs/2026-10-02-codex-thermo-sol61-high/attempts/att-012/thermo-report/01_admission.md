# Admission and shutdown ownership

## Scope and measurements

Reviewed the complete head versions of `lib/batcher/batcher.go` and its tests, the committed diff, `lib/atexit/atexit.go`, the relevant logging/configuration functions, and the Dropbox and Google Photos call sites and logging identities. Repository guidance files were not loaded as instructions.

The commands `git diff main...review-head`, `nl -ba lib/batcher/batcher.go`, `git show main:lib/batcher/batcher.go | wc -l`, and `wc -l lib/batcher/batcher.go` establish that this production change adds six lines and takes the file from 282 to 288 lines. It adds one mutex field, two lock calls, and three unlock calls. There are no new production branches, helpers, casts, optional values, or external dependencies.

## Invariant and evidence

At head lines 267–281, Commit locks before checking `b.closed`, unlocks before returning the existing fatal shutdown error, and otherwise keeps the lock until its request has been sent. At lines 243–252, Shutdown holds the same lock while closing admission and sending the quit marker. Mutex exclusion and channel send order give two possible outcomes: Commit wins admission and completes its send before the quit marker, or Shutdown wins and Commit observes the closed channel after acquiring the mutex. There is no permitted enqueue after the marker.

The worker at lines 208–229 consumes the FIFO channel, exits on the quit marker, and flushes any accumulated partial batch. The mutex therefore strengthens ordering at the package that owns admission, without adding backend-specific policy or changing result distribution.

The lock is released before Commit waits for a response at line 286 and before Shutdown waits for the worker at line 253. This is essential: holding it during a response wait could prevent the shutdown flush of an incomplete batch. The proposed patch correctly avoids that cycle.

The input channel is buffered to the batch size at construction. A send can block under backpressure while holding `admitMu`, but the consuming worker never needs that mutex. The design retains the existing dependence on the backend commit callback making progress. The gate serializes admission and shutdown intentionally; this is related work that must be ordered, rather than independent work unnecessarily serialized.

Shutdown's `sync.Once` still owns idempotence and completion waiting. The closed channel owns rejection state, the mutex owns ordering, and the wait group owns worker completion. Each mechanism has a separate, justified role. Atexit unregistering and informational logging occur before the admission lock; no new atexit lock ordering was introduced.

The generic logging identity is formatted inside Commit's newly locked region. The current Dropbox and Google Photos String methods simply format their root strings. No source evidence shows a new production lock cycle through those identities. The test deliberately supplies a blocking identity, which makes the logging boundary significant for test orchestration; see [F1's detail](02_regression_tests.md).

## Structural alternatives and code-judo assessment

A producer helper could encapsulate the check and send with a deferred unlock. For this single admission path it would add a function and an error/response handoff while preserving the same state and branching. It does not delete enough complexity to justify extracting these few lines. A whole-Commit deferred unlock would be incorrect because the response wait must be outside the critical section.

One broader alternative would use the newly serialized send/close boundary to close `b.in`, remove the `quit` request field and marker send, and make the worker exit on a closed input channel after draining it. The worked shape would be:

```go
// Under admitMu, Shutdown would close admission and then input.
close(b.closed)
close(b.in)

// The worker would distinguish end-of-input from an ordinary request.
case req, ok := <-b.in:
    if !ok {
        break outer
    }
    // Existing accumulation and batch commit continue here.
```

This would remove one control flag and the potentially blocking marker send, but it changes the worker protocol and the existing deliberate avoidance of input-channel closure. It is a modest alternate representation, not an obvious dramatic simplification missed by this six-line fix. It needs its own evaluation and is not an actionable request for this review. Draining after the existing marker would instead introduce a second termination path and rejection machinery; it is less direct than making admission atomic.

The production patch meets the structural approval bar: the mutex expresses exactly the missing invariant, remains local to its owner, and does not scatter checks or require a new policy abstraction. No actionable production finding was identified.

## Executed verification

Every command used this environment from the clone root:

```sh
GOMODCACHE=/home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high/att-012/clone-cache/gomodcache
GOCACHE=/home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high/att-012/clone-cache/gocache
GOFLAGS=-mod=mod
GOPROXY=off
GOTOOLCHAIN=local
```

`timeout 300s env <the variables above> go test -count=1 -race ./lib/batcher` passed, with package execution reported as 5.353 seconds. This exercises the existing success, failure, shutdown, async, and new concurrent-admission tests. The DEBUG focused head run also passed; its full command and negative-control comparison appear in the test detail.

No production remedy was applied. No network or backend integration testing was performed. The worker-ordering proof is source-derived, and passing the race detector alone is not proof of channel-order correctness; the merge-base negative control provides the additional behavioral evidence.

Before and after execution, `git status --porcelain=v1` was empty, `git diff --exit-code` and `git diff --cached --exit-code` passed, and `git ls-tree -r HEAD | sha256sum` produced `adfff9ed533b77a4602d97f4d66347164014993cb39d5cfe1b304f28d860f66e`. HEAD and main resolved to the pinned head and base SHAs respectively.
