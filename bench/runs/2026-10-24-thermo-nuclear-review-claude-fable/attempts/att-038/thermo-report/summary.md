# Thermo Nuclear Review — grpc/grpc-go#7390

**Range:** `daab56344e612097fd50c46c433de5d9b6013837..76ef33f44a600c3ed1a385979fd1dfbcade3fbb6` (`main...review-head`)
**Diff:** `clientconn.go` only, +6 / −7.

## Verdict

No actionable findings.

## What the change does

`addrConn.resetTransport()` used to acquire `ac.mu` itself. Its two callers, `addrConn.connect()` and `addrConn.updateAddrs()`, both validated state under the lock, released it, and then invoked `resetTransport`, which re-acquired the lock before setting the state to `Connecting`. The window between release and re-acquire allowed a second `connect()` (the balancer wrapper's `Connect()` runs `ac.connect()` on a fresh goroutine) to also observe `Idle` and start a second, parallel `tryAllAddrs` loop. That is the orphaned-transport flake in issue #7365.

The PR renames the function to `resetTransportAndUnlock()` (`clientconn.go:1234`), removes its internal `ac.mu.Lock()`, and has both callers keep holding the lock when they call it. The function unlocks on every path: the early `ctx.Err()` return (`clientconn.go:1237`) and after the transition to `Connecting` (`clientconn.go:1262`). The remainder of the function (backoff, `tryAllAddrs`, TRANSIENT_FAILURE handling) is unchanged.

## What I traced

**The race is actually closed.** In `connect()` (`clientconn.go:905-924`) the state check `ac.state != connectivity.Idle` and the transition to `Connecting` inside `resetTransportAndUnlock` now happen in one critical section. A concurrent `connect()` blocks on `ac.mu` and then sees `Connecting`, returning `nil` without starting another attempt. The only path that unlocks without changing state is the `acCtx.Err() != nil` branch. With the lock held and `ac.state == Idle`, `ac.ctx` can only be cancelled by `tearDown()` (which sets `Shutdown` under the same lock first, so `connect()` would have rejected it), by `updateAddrs()` (which replaces `ac.ctx` with a fresh context under the same lock), or by the parent `ClientConn` context being cancelled during `ClientConn.Close()`. In the last case the addrConn is about to be torn down anyway, and this behaviour is identical to the pre-change `resetTransport`.

**Lock handoff to a goroutine in `updateAddrs` is safe.** `updateAddrs()` (`clientconn.go:940-997`) now executes `go ac.resetTransportAndUnlock()` while still holding `ac.mu`, so the lock is released by the child goroutine rather than by `updateAddrs` itself. `sync.Mutex` is not goroutine-bound, so this is legal, and `go vet` on the root package is clean. The practical consequence is that the deferred `ac.transport.GracefulClose()` registered at `clientconn.go:986` now runs while the lock is still logically held. `http2Client.GracefulClose` (`internal/transport/http2_client.go:1045-1064`) takes `t.mu` and calls `onClose`, and `onClose` (`clientconn.go:1351-1377`) blocks on `ac.mu.Lock()`. I checked that this cannot deadlock: the goroutine that owns the lock only reads `ac.ctx`, `ac.addrs`, computes backoff, calls `updateConnectivityState(Connecting)`, and unlocks. `updateConnectivityState` fans out through `acBalancerWrapper.updateState` (`balancer_wrapper.go:255-265`), which enqueues onto a `CallbackSerializer` backed by an unbounded buffer (`internal/grpcsync/callback_serializer.go:64-66`) and never blocks, and it never touches the old transport's `t.mu`. So `onClose` waits briefly, acquires the lock, observes the old (cancelled) context captured in its closure, and returns without touching state. Net ordering is the same as before the change, where `go resetTransport()` was also spawned before the deferred `GracefulClose` ran.

**Other paths unchanged.** `tearDown()` (`clientconn.go:1545-1580`) and `createTransport`'s post-dial `ctx.Err()` / `hctx.Err()` checks are untouched and still interact with the new context handling exactly as before. `len(addrs) == 0` in `updateAddrs` behaves as before (sets `Idle`, then the goroutine transitions to `Connecting` and `tryAllAddrs` returns `nil` on an empty list); that pre-existing quirk is not introduced by this PR and is out of scope.

**No other callers.** A repository-wide search finds no other call site of `resetTransport`; the only remaining mentions are expected-log strings in `test/goaway_test.go` and `test/end2end_test.go` that refer to a log line which already did not exist before this PR, so they are unaffected.

**No devex, feature-gate, or API surface impact.** The change is confined to unexported methods of `addrConn`; no exported API, environment variable, configuration, or build step changes.

## Tests executed (offline, race detector on)

| Command | Result |
| --- | --- |
| `go test -race -run 'Test/(UpdateAddresses\|ResetConnectBackoff\|ConnectParams\|WithConnectParams\|Balancer_StateListenerBeforeConnect\|EnterIdle\|CloseConnectionWhenServerPrefaceNotReceived\|Backoff)' .` | ok (8.5s) |
| `go test -race -run 'Test/(StateTransitions\|SubConn\|RoundRobin_UpdateAddressAttributes\|PickFirst\|HealthCheckWithAddrConnDrain\|HealthCheckWithoutSetConnectivityState\|GoAway\|ConnectivityStateSubscriber)' ./test/` | ok (12.7s) |
| `go test -race -count=200 -run 'Test/AuthorityRevive' ./xds/internal/xdsclient/tests/` | ok, 200/200 passes (26.4s) |
| `go vet .` | clean |

The clone tree was unchanged after the review.

## Notes (not findings)

- The comment at `clientconn.go:983-984` ("We have to defer here because GracefulClose => onClose, which requires locking ac.mu") is now slightly imprecise: deferring is still required, but the lock is released by the spawned `resetTransportAndUnlock` goroutine rather than by `updateAddrs` before the defer runs. This is a documentation nit with no behavioural effect and does not rise to an actionable finding.
