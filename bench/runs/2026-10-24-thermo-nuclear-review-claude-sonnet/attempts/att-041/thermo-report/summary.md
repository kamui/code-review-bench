# Thermo nuclear review: grpc/grpc-go#7390 (`clientconn.go`, +6/−7)

No actionable findings.

## What was traced

- `connect()` (clientconn.go:905-924) now keeps `ac.mu` held from the Idle check through `resetTransportAndUnlock()`, which sets CONNECTING before it releases the lock. A second concurrent `connect()` therefore sees a non-Idle state and returns. That closes the double-`tryAllAddrs` race from the issue.
- Every exit of `resetTransportAndUnlock()` (clientconn.go:1234) releases the lock exactly once:
  - the early `acCtx.Err()` return unlocks;
  - the normal path unlocks right after `updateConnectivityState(Connecting)`;
  - the later paths re-lock and unlock themselves, as before.
  I found no double-unlock and no leaked lock.
- `updateAddrs()` (clientconn.go:940-997) hands the held lock to `go ac.resetTransportAndUnlock()`. Every earlier return in that function unlocks itself. Go mutexes have no owner, so unlocking from the spawned goroutine is legal.
- The deferred `ac.transport.GracefulClose()` in `updateAddrs` now runs while the spawned goroutine may still hold `ac.mu`.
  - `GracefulClose` calls `onClose` inline (internal/transport/http2_client.go:1055). `onClose` locks `ac.mu` (clientconn.go:1352).
  - It can only wait briefly. The goroutine does no I/O or blocking while it holds the lock: `updateConnectivityState` calls `acbw.updateState`, which uses the non-blocking `serializer.Schedule`. The goroutine's unlock does not depend on the caller, so there is no deadlock.
  - The old transport's `onClose` sees the cancelled `ctx` and returns early, so it does not clobber the new CONNECTING state.
- The `len(addrs)==0` path sets Idle and then CONNECTING under one critical section. The Idle window in which a concurrent `connect()` could start a second attempt is closed.
- No other callers of `resetTransport` exist. The only remaining references are log-message strings in tests, which are unaffected.
- `go vet .` passes on the head.

## Residual observations (not reported)

- If `createTransport` sets Idle through the `hctx.Err()` branch (clientconn.go:1414-1419), a new `connect()` can start while the old attempt is still about to reset `backoffIdx`. This window existed before the PR, the effect is a benign backoff reset, and the PR does not make it worse.
- The thread about enforcing the lock precondition in code was answered by the doc comment. That matches the existing `...Locked` convention, so I do not count it as a defect.
