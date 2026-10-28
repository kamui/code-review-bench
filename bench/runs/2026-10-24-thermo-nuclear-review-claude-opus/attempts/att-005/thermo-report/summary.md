# Thermo Nuclear Review — grpc/grpc-go#7390

Range: `daab5634..76ef33f4` (`git diff main...review-head`), one file: `clientconn.go` (+6/−7).

## Result

No actionable findings.

## What changed

`addrConn.resetTransport()` became `resetTransportAndUnlock()`. The caller must now hold `ac.mu`, and the function always releases it. The function used to take the lock itself. There are two callers:

- `addrConn.connect()` (`clientconn.go:905-924`) now keeps holding `ac.mu` from its `Idle`/`Shutdown` check until `resetTransportAndUnlock` sets the state to `Connecting` (`clientconn.go:1261`). This closes the window in which a second `connect()` (started by `acBalancerWrapper.Connect` through `go acbw.ac.connect()`, `balancer_wrapper.go:276`) could also see `Idle` and start a second, concurrent `tryAllAddrs`. That window caused the orphaned-transport flake in #7365.
- `addrConn.updateAddrs()` (`clientconn.go:940-997`) now runs `go ac.resetTransportAndUnlock()` while still holding `ac.mu`. The new goroutine releases the lock.

## Traced concerns, each verified as not a defect

1. **The mutex is locked in one goroutine and unlocked in another (`updateAddrs`).** Go's `sync.Mutex` is not tied to a goroutine, so this is legal. The `go` statement orders the spawner before the goroutine, so the race detector accepts it. The lock is always released: every return path in `resetTransportAndUnlock` (`clientconn.go:1235-1262`) unlocks before it returns or blocks.

2. **Deadlock through the deferred `GracefulClose` in `updateAddrs`.** The deferred `ac.transport.GracefulClose()` now runs while `ac.mu` is still held by the spawned, not-yet-run goroutine. `http2Client.GracefulClose` (`internal/transport/http2_client.go:1045-1064`) calls `t.onClose` while holding `t.mu`. That callback (`clientconn.go:1352`) locks `ac.mu`, so the `updateAddrs` goroutine can block briefly. While the spawned goroutine holds `ac.mu`, it only does non-blocking work: it checks `ctx.Err()`, computes the backoff and connect timeout, and calls `updateConnectivityState`. `updateConnectivityState` goes to `acbw.updateState`, which only schedules on the serializer without blocking (`balancer_wrapper.go:255-265`). The goroutine never takes `t.mu` or any lock the caller might hold, so it always reaches `ac.mu.Unlock()` and the cycle cannot close. The old transport's `onClose` then sees its cancelled `ctx` and returns. This matches the old behavior.

3. **Behavior change from removing the unlocked window in `updateAddrs`.** Before, `tearDown` or `connect` could run between the unlock and the goroutine's re-lock. Now they cannot. With `tearDown`, the old code's `acCtx.Err()` check made the goroutine a no-op. With the new code, `tearDown` runs after `Connecting` is set, cancels `ac.ctx`, and `tryAllAddrs`/`createTransport` abort or close the new transport through their existing `ctx.Err()` checks. The final state is still `Shutdown`. With `connect`, the old code could start a second concurrent attempt when `len(addrs)==0` set the state to `Idle`. That was the same race, and it is now removed.

4. **`connect()` now moves to `Connecting` even when `ac.ctx` is already cancelled.** It cannot: `resetTransportAndUnlock` still checks `acCtx.Err()` before it calls `updateConnectivityState`, under the same lock. `ac.ctx` is only cancelled together with the state change to `Shutdown` (tearDown) or replaced (updateAddrs), both under `ac.mu`, and `connect` already returns early on `Shutdown`.

5. **Devex, feature gates, public API.** The change touches only unexported methods. It adds no configuration, environment variables or flags. No other package references `resetTransport` (checked with grep across the repository).

## Tests run (offline, focused)

- `go test -race -count=200 -run 'Test/AuthorityRevive' ./xds/internal/xdsclient/tests/` passed. This is the flaky test from #7365.
- `go test -race -count=1 .` (root `grpc` package, which contains the changed code) passed.
- `go test -race -count=1 ./test/... -run 'Test/.*(PickFirst|SubConn|UpdateAddr|Addr|GoAway|Reconnect|Backoff).*'` passed.
