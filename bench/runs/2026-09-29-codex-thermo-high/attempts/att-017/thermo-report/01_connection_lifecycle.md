# Connection lifecycle and lock boundary

## Finding: asynchronous lock handoff can deadlock `updateAddrs`

The change makes both `connect` and `updateAddrs` call `resetTransportAndUnlock` while holding `ac.mu`. In `connect`, this is a direct call: the helper checks the context, updates connectivity to `Connecting`, and unlocks before dialing. That closes the original race where two callers could both observe `Idle` before either established the connecting state.

The `updateAddrs` call site is different. At `clientconn.go:983-987`, it defers `ac.transport.GracefulClose()` while holding `ac.mu`; at line 996 it starts `go ac.resetTransportAndUnlock()` without releasing that mutex. The helper's first possible unlock is at lines 1236-1238 (if the context has ended) or 1261-1262 (after calculating the deadline and setting `Connecting`). The deferred `GracefulClose` runs as `updateAddrs` returns, with no ordering guarantee that the newly launched goroutine has executed either unlock.

This is a real lock cycle, not just an unusual ownership convention. `http2Client.GracefulClose` invokes its `onClose` callback inline. The callback created in `addrConn.createTransport` at `clientconn.go:1351-1354` immediately calls `ac.mu.Lock()`. If the deferred close runs before the helper unlocks, the update goroutine waits for `ac.mu`; the helper waits for the mutex held by the update goroutine. The cycle is avoidable but can hang the address update and leave the subchannel unable to progress.

The local code itself documents the constraint: `tearDown` unlocks `ac.mu` before calling `GracefulClose` or `Close` because those operations invoke `onClose`, which locks `ac.mu` (`clientconn.go:1573-1574`). The new goroutine handoff bypasses that safe ordering in `updateAddrs`.

## Worked code-judo proposal

Make the state change and work scheduling explicit instead of passing a held mutex between goroutines. Under `ac.mu`, each entry point should validate its current state, cancel/detach the previous attempt as needed, and commit `Connecting` before unlocking. Then capture the connection inputs (`ctx`, address slice, backoff/deadline inputs) as a work item. After unlocking, close any detached transport and run the transport attempt. `connect` and `updateAddrs` can share a small locked transition/snapshot helper, while the actual dial routine should own no caller-held lock and should never unlock a mutex it did not acquire itself.

For `updateAddrs`, the ordering should be: lock; update address/context and detach old transport; commit the state that prevents another idle `connect`; capture new-attempt inputs; unlock; close the old transport; start the replacement work. If empty-address behavior intentionally reports `Idle`, preserve that state rule and make the no-address worker path explicit rather than relying on a later unconditional `Connecting` update. This keeps the check-to-transition atomic, preserves callback freedom from the mutex, and removes the caller obligation encoded by `resetTransportAndUnlock`.

A minimal patch that only moves `go ac.resetTransportAndUnlock()` after an unlock would reintroduce the original check/update gap for paths that are still `Idle`. The state transition must happen before releasing the lock; the cleanup callback and network work must happen after.

## Evidence and measurements

The review range is `daab56344e612097fd50c46c433de5d9b6013837..76ef33f44a600c3ed1a385979fd1dfbcade3fbb6`. `git diff --stat main...review-head` reports one file changed with 6 insertions and 7 deletions. The changed file remains a single large source file, but the patch does not push it across a size boundary; file growth is not a finding here.

Relevant inspection commands:

- `git diff --find-renames --unified=80 main...review-head -- clientconn.go`
- `rg -n "resetTransport|resetBackoff|func \\(ac \\*addrConn\\) connect|updateAddrs" clientconn.go`
- `rg -n "GracefulClose\\(|func \\(.*\\) onClose|resetTransportAndUnlock|resetBackoff\\(" --glob '*.go'`
- `nl -ba clientconn.go | sed -n '970,1002p;1225,1268p;1568,1592p'`

`git diff --check main...review-head` passed. No tests were executed. Static review confirms the deadlock interleaving; a focused regression test should hold the helper before its first unlock while `updateAddrs` returns with an old transport whose `GracefulClose` callback needs `ac.mu`, then verify the update completes. That test is suggested verification, not a test run in this review.

## Verification status

The finding is based on the actual synchronous callback path in the transport implementation and the `addrConn` callback in `clientconn.go`. The deadlock requires the close defer to execute before the new goroutine unlocks, so it is scheduling-dependent, but the code provides no synchronization that rules this order out. The checkout remained unchanged; `git status --short` was empty after inspection.
