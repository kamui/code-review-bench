# Thermo Nuclear Review — grpc/grpc-go#7390

Range: `daab56344e612097fd50c46c433de5d9b6013837..76ef33f44a600c3ed1a385979fd1dfbcade3fbb6` (`git diff main...review-head`). One file changed: `clientconn.go` (+6/−7).

## What the change does

`addrConn.resetTransport()` becomes `resetTransportAndUnlock()`. It no longer takes `ac.mu` itself. The caller must already hold the lock, and the function releases it. There are two call sites:

- `addrConn.connect()` (`clientconn.go:905-924`) checks `state == Idle` while holding the lock and then calls `resetTransportAndUnlock()` without releasing the lock first. That function moves the state to `Connecting` before it unlocks (`clientconn.go:1262-1263`). So the check-then-act window that let two concurrent `connect()` calls both see `Idle` and both run `tryAllAddrs` is gone. This was the root cause in #7365.
- `addrConn.updateAddrs()` (`clientconn.go:940-997`) now hands the held lock to `go ac.resetTransportAndUnlock()`, and the spawned goroutine releases it. This closes the same window between `updateAddrs` unlocking and the goroutine locking again. One case matters most: `len(addrs) == 0`, where `updateAddrs` sets the state to `Idle` and a concurrent `connect()` could otherwise start a second attempt.

## Evidence traced

1. **All callers.** `git grep resetTransport` finds only the two call sites above. The other hits are log strings in tests. `connect()` is only called from `acBalancerWrapper.Connect` (`balancer_wrapper.go:276`, via `go`), and `updateAddrs` is only called from `acBalancerWrapper.UpdateAddresses` (`balancer_wrapper.go:272`). No caller is left that would double-lock or fail to unlock.
2. **Every return path releases the lock.** `resetTransportAndUnlock` unlocks on the `acCtx.Err() != nil` early return (line 1237) and on the main path after `updateConnectivityState(Connecting)` (line 1263). Every later lock/unlock pair in the function is balanced, the same as before.
3. **Lock ordering in `connect()`.** The newly held section (between the `Idle` check and the old `Lock()` at the top of `resetTransport`) contains no new work. Everything `resetTransportAndUnlock` does before unlocking (`ac.dopts.bs.Backoff`, `minConnectTimeout()`, `updateConnectivityState`) already ran under `ac.mu` before this change. `updateConnectivityState` → `acbw.updateState` → `CallbackSerializer.Schedule` is just an unbounded `Put`, so it never blocks (`internal/grpcsync/callback_serializer.go:64-66`).
4. **Handing the lock across goroutines in `updateAddrs`.** `sync.Mutex` is not tied to a goroutine, so unlocking it in the spawned goroutine is legal and race-detector-clean.
   - One subtle interaction: `updateAddrs` still does `defer ac.transport.GracefulClose()`. That deferred call now runs while the spawned goroutine logically owns `ac.mu`. `http2Client.GracefulClose` calls `t.onClose` synchronously while holding the old transport's `t.mu` (`internal/transport/http2_client.go:1045-1064`), and `onClose` does `ac.mu.Lock()` (`clientconn.go:1352`). So the deferred close blocks until the goroutine releases `ac.mu`.
   - I checked whether this can deadlock. The goroutine's locked section never touches the old transport's `t.mu` (`ac.transport` was already set to nil) and never needs anything the `updateAddrs` caller holds (the serializer schedule does not block). So the wait is bounded and short. After that, `onClose` sees the cancelled old `ctx` and returns without changing state (`clientconn.go:1355-1361`). There is no deadlock and no change in state.
5. **The state machine stays correct.** With `updateAddrs` holding the lock until the new goroutine sets `Connecting`, nothing can run between the context swap (`ac.ctx, ac.cancel = context.WithCancel(...)`) and the state change. `tearDown` still cancels `ac.ctx` under the lock, so a goroutine that runs after teardown takes the `acCtx.Err()` early return as before.
   - One case is unchanged: if `cc.ctx` is cancelled while the state is still `Idle`, `connect()` returns with the state left at `Idle`. The old code behaved the same way, and no connection is attempted in that case.
6. **Tests run (offline, focused).**
   - `go test -race .` (root package `google.golang.org/grpc`): `ok` (13.9s).
   - `go test -race -run 'Test/AuthorityRevive' -count=300 ./xds/internal/xdsclient/tests/`, which is the originating flaky test: `ok` (39.4s), with no failures.

No feature gates, environment variables, ports, secrets or build steps are touched. There is no devex impact.

## Findings

No actionable findings.

A minor non-actionable note: the new doc comment says the function "unconditionally connects the addrConn". That is not literally true, because it returns early without connecting when `ac.ctx` is already cancelled. The comment's statement about the lock contract is accurate, though, and the mismatch causes no failure.
