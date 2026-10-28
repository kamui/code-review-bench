# Thermo Nuclear Review: grpc/grpc-go#7390

Range: `main...review-head`, `clientconn.go` only (+6/−7).

No actionable findings.

## What I traced

- `connect()` (clientconn.go:~908-924). It now keeps `ac.mu` held from the Idle check through `resetTransportAndUnlock()`. `resetTransportAndUnlock` runs `updateConnectivityState(Connecting)` before it unlocks. A second concurrent `connect()` therefore sees `Connecting` and returns. This closes the window described in issue #7365, where two callers both saw Idle and ran `tryAllAddrs` twice.
- `resetTransportAndUnlock()` (clientconn.go:~1231). Every exit path before the first `ac.mu.Unlock()` unlocks exactly once. These are the early return on `acCtx.Err()` and the normal path that unlocks after the Connecting transition. Everything after that point locks and unlocks in balanced pairs, as before. I found no double unlock and no path that returns with the mutex held.
- `updateAddrs()` (clientconn.go:~990-996). The lock stays held across `go ac.resetTransportAndUnlock()`, and the goroutine releases it. `sync.Mutex` allows unlocking from a different goroutine, and the race detector handles it. Holding the lock also means the goroutine reads the `ac.ctx` that this call just installed. Before, a second `updateAddrs` could swap `ctx` between the unlock and the goroutine start.
- The deferred `ac.transport.GracefulClose()` in `updateAddrs` runs after return. It calls `onClose`, which takes `ac.mu`. It can now block until the spawned goroutine unlocks. That goroutine does not wait on the caller or on the old transport's `t.mu`, so there is no lock cycle. `onClose` returns early because the old `ctx` is cancelled.
- All callers of the renamed function are updated. A grep for `resetTransport` finds no stale references, including in tests.
- `updateConnectivityState` was already called under `ac.mu` in the old `resetTransport`, so calling `acbw.updateState` while the lock is held is not a new behaviour.
- The `connect()` early-return paths still unlock. The `acCtx.Err()` short-circuit in `connect()` cannot be reached with a cancelled context while the state is Idle, because `tearDown` sets Shutdown under the same mutex.

I did not run tests. This was a read-only trace.
