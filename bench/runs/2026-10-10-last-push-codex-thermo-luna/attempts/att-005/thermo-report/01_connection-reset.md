# Connection reset and mutex handoff

## Review result

No actionable maintainability or structural finding was identified in the changed connection-reset path. The patch changes one cohesive synchronization boundary in `clientconn.go` and does not grow the file across a size threshold: the file is 1,838 lines at the reviewed head. It adds no conditionals, state fields, or branch-specific behavior.

## Evidence

In `connect`, the mutex is acquired at line 906 and the shutdown/non-idle exits release it directly. On the idle path, the method now calls `resetTransportAndUnlock` at line 922 without first unlocking. That helper checks the captured context and, if canceled, unlocks at lines 1235–1238. Otherwise it transitions the address connection to `Connecting` while still holding the mutex at line 1261, then unlocks at line 1262 before dialing addresses. Concurrent `connect` calls therefore cannot both observe `Idle` before this state transition.

In `updateAddrs`, the existing mutex protects address replacement, cancellation of the old connection context, clearing the transport, and any transition to `Idle` (lines 948–992). When a restart is needed, the method now launches `resetTransportAndUnlock` at line 996 while still holding that same mutex. This preserves the update-to-restart handoff: releasing the mutex before the goroutine enters the helper would permit observers to see the old connectivity state after the transport and context had already been replaced. The deferred `GracefulClose` remains outside the critical section in effect: its callback may wait for the new goroutine to set `Connecting` and unlock, after which it can acquire `ac.mu` normally.

The helper name and doc comment at lines 1231–1234 state its two important properties: it starts a connection attempt unconditionally and requires the caller to hold `ac.mu`, which it guarantees to release. The function also releases the mutex before `tryAllAddrs`, so network work does not run under the lock. The use of one helper for both synchronous and asynchronous entry points keeps this lock contract in one place.

## Structural alternatives considered

A direct extraction of a short “set `Connecting` and unlock” helper would leave the rest of `resetTransport` responsible for context checks, dial setup, errors, and backoff. That would divide one lifecycle operation across two helpers without reducing state or control flow. Keeping the full operation in `resetTransportAndUnlock` is more cohesive.

Making `updateAddrs` unlock before starting its goroutine and having that goroutine reacquire the mutex would create a gap between replacing the transport/context and marking the connection `Connecting`. Preserving that behavior would require another explicit pending/restarting state or another synchronization mechanism. Neither is a simpler code-judo move than the current documented mutex handoff. Calling the helper synchronously would also make address updates perform the connection attempt inline rather than preserving the existing asynchronous restart.

The method name is longer than the former `resetTransport`, but it communicates an observable lock effect at each call site. Renaming it to a generic `Locked` suffix would be less accurate because the method deliberately releases the lock before returning and while doing the dial attempt. No rename or further decomposition is recommended.

## Verification status

Reviewed the committed range `main...review-head` with `git diff --find-renames --find-copies main...review-head -- clientconn.go`, inspected the changed methods and adjacent connection lifecycle with `nl -ba clientconn.go`, and searched the repository for helper call sites and related lock/unlock naming with `rg`. The diff is six insertions and seven deletions in one file; `git diff --check main...review-head` reported no whitespace errors. No tests were run. The packet records the author's 100,000-attempt flake check, but that report was not independently reproduced in this review.
