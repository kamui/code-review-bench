# Batcher admission and shutdown

## Scope and measurements

The committed diff changes `lib/batcher/batcher.go` and `lib/batcher/batcher_test.go`, adding 6 and 89 lines respectively. At the reviewed head the files are 288 and 364 lines, so neither approaches the skill's 1,000-line decomposition threshold. The production change adds one mutex field and acquires it in `Commit` and `Shutdown`; it adds no new condition tree, mode, wrapper, or helper layer.

The relevant production sequence is in `lib/batcher/batcher.go:243-253` and `lib/batcher/batcher.go:266-287`. Shutdown takes `admitMu`, closes `closed`, queues the quit request, then releases the mutex. Commit takes the same mutex, rejects an already closed batcher, or enqueues its request before releasing the mutex. Thus every accepted request is ahead of the quit marker, and every caller arriving after shutdown acquires the lock sees the closed signal and returns the existing fatal shutdown error. A synchronous caller cannot be left awaiting a response for a request stranded behind the marker; an asynchronous request cannot be silently dropped in that way.

## Maintainability assessment

The mutex is the smallest direct expression of the required ordering invariant. It is located beside the batcher's existing channel and lifecycle state, and it protects just the admission/check/send transaction and the corresponding shutdown close/marker transaction. The implementation does not scatter special cases into the commit loop or alter request semantics.

The tempting code-judo alternative of having the commit loop drain requests after seeing the quit marker would require deciding how to answer sync requests and how to reject async work, creating extra shutdown paths. Closing the input channel would make concurrent sends panic. The current mutual exclusion keeps the established quit-marker protocol and removes the race with one explicit boundary, so there is no clearer restructuring that deletes complexity while preserving behavior.

The lock also spans the debug log formatting in `Commit`. Narrowing it by moving the log before the admission check can produce an “Adding” log for a request rejected during shutdown; moving it after enqueue can reorder the log relative to processing. This is not a compelling maintainability improvement for the current change, and does not justify a finding.

## Regression coverage

`lib/batcher/batcher_test.go:239-311` adds a focused test in both sync and async modes. Its `blockingStringer` pauses the debug log after Commit has passed the closed check, which forces Shutdown to contend at the vulnerable admission boundary. The test then checks that the caller returns, shutdown completes, and the accepted batch reaches the callback. The test adds no production abstraction and confines its synchronization helper to the test file.

## Verification status

Source inspection only; no tests or builds were run. The execution allowance permits a focused package test, but no test run was requested. `git diff --check main...review-head` completed with no output, and the working tree was clean when inspected. The review inspected the exact `main...review-head` change and confirmed the local `main` and `review-head` SHAs match the packet's merge-base and head.
