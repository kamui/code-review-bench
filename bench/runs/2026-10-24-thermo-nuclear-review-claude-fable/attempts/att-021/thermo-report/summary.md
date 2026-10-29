# Thermo Nuclear Review — grpc/grpc-go#7390

Range reviewed: `daab56344e612097fd50c46c433de5d9b6013837..76ef33f44a600c3ed1a385979fd1dfbcade3fbb6` (`main...review-head`).
One file changed: `clientconn.go` (+6 / −7).

## Verdict

No actionable findings.

## What the change does

`addrConn.resetTransport()` used to acquire `ac.mu` itself. Its two callers, `addrConn.connect()` and `addrConn.updateAddrs()`, released `ac.mu` just before invoking it. That left a window between the caller's state check (for example `connect()` verifying the subchannel is `Idle`) and `resetTransport()` moving the state to `Connecting`. Two `connect()` calls racing through that window both saw `Idle` and both launched a connection attempt, producing two transports with only one tracked in `ac.transport` (the symptom in issue #7365).

The PR renames the function to `resetTransportAndUnlock()`, removes its internal `ac.mu.Lock()`, and has both callers keep holding `ac.mu` into the call. The state transition to `Connecting` now happens inside the same critical section as the caller's validation, so a second `connect()` sees `Connecting` and returns early.

## Evidence traced

**Caller 1, `connect()` (`clientconn.go:905-924`).** Locks `ac.mu`, returns early on `Shutdown` or non-`Idle` (unlocking on each path), then calls `resetTransportAndUnlock()` with the lock still held. Every path out of `connect()` releases the mutex exactly once: the two early returns unlock explicitly; the fall-through path delegates the unlock. The window described in the issue is closed.

**Caller 2, `updateAddrs()` (`clientconn.go:940-997`).** Locks `ac.mu`, and every early-return path unlocks explicitly (lines 950, 960, 971). The fall-through path now spawns `go ac.resetTransportAndUnlock()` without unlocking first, so the lock is released by the spawned goroutine. `sync.Mutex` is not goroutine-owned, so unlocking from a different goroutine is valid Go. The `defer ac.transport.GracefulClose()` registered at line 986 runs when `updateAddrs` returns, and `GracefulClose` calls `onClose` synchronously (`internal/transport/http2_client.go:1045-1064`), which locks `ac.mu` (`clientconn.go:1351-1353`). That `Lock()` now waits until the spawned goroutine releases the mutex. I traced what the spawned goroutine does before unlocking (`clientconn.go:1235-1262`): reads `ac.ctx`, `ac.addrs`, `ac.backoffIdx`, computes the backoff and deadline, and calls `updateConnectivityState(Connecting)`. `updateConnectivityState` closes/recreates `stateChan`, logs via channelz, and calls `acbw.updateState`, which only enqueues onto the balancer `CallbackSerializer` (`balancer_wrapper.go:255-265`, `internal/grpcsync/callback_serializer.go:64-66`, unbounded buffer, non-blocking). Nothing in that prefix acquires the transport mutex, `cc.mu`, or any lock the `updateAddrs` caller could hold, so the wait is bounded by goroutine scheduling and cannot deadlock. The relative ordering of `GracefulClose` and the new connection attempt was already arbitrary before the change (the goroutine was spawned before the deferred close ran), so no new ordering hazard is introduced.

**`resetTransportAndUnlock()` body (`clientconn.go:1231-1304`).** Both exits from the locked prefix unlock: the `acCtx.Err() != nil` early return at line 1237, and the normal path at line 1262. The rest of the function's lock/unlock pairs are unchanged from the base. The doc comment accurately states the contract. No other callers exist in the repository; the only other textual matches for `resetTransport` are log-message strings in `test/goaway_test.go` and `test/end2end_test.go` that refer to a message emitted elsewhere and are not affected by the rename.

**Lock ordering.** The change does not add any new lock acquisition under `ac.mu`. The pre-unlock prefix of `resetTransportAndUnlock` is identical to what the old `resetTransport` executed under the lock it took itself. The only difference is that the lock is now held continuously from the caller's validation through the state transition, which is the intended fix.

**Semantics of the `len(addrs) == 0` path in `updateAddrs`.** The code sets `Idle` and then launches a connection attempt that sets `Connecting` and returns success with no addresses tried. This is pre-existing behaviour on the base and is not altered by the diff, so it is out of scope.

## Verification run

- `go vet .` on the root package: clean.
- `go test -race -count=1 -run 'Test/(.*Connect.*|.*UpdateAddr.*|.*Idle.*|.*Backoff.*|.*ResetConnectBackoff.*|.*AddrConn.*|.*Ready.*|.*Dial.*)' .` : ok.
- `go test -race -count=20 -run 'Test/AuthorityRevive' ./xds/internal/xdsclient/tests/` (the originally flaky test): ok, no race reports.
- `go test -race -count=1 -run 'Test/(.*GoAway.*|.*PickFirst.*|.*Pickfirst.*|.*UpdateAddresses.*|.*StateTransitions.*)' ./test/` (exercises the `updateAddrs` → deferred `GracefulClose` path and `connect()`): ok.

## Non-finding notes

- Because `updateAddrs` now returns while `ac.mu` is logically held until the spawned goroutine runs, the caller's deferred `GracefulClose` briefly blocks on `ac.mu` until that goroutine is scheduled and reaches its `Unlock`. This is a scheduling-latency effect measured in microseconds, not a deadlock or correctness issue, and I do not consider it actionable.
- Security, devex, and feature-gate surfaces are untouched: no environment variables, ports, secrets, build steps, or flags change.
