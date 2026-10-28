# Thermo Nuclear Review — grpc/grpc-go#7390

**Range:** `daab56344e612097fd50c46c433de5d9b6013837..76ef33f44a600c3ed1a385979fd1dfbcade3fbb6` (`main...review-head`)
**Change:** `clientconn.go` only, +6 / −7.

## Verdict

No actionable findings.

## What the change does

`addrConn.resetTransport()` used to acquire `ac.mu` itself. Its two callers released the mutex
just before invoking it, which left a window in which a second caller could observe the
addrConn still `Idle` and start a second, concurrent connection attempt (the root cause of the
`Test/AuthorityRevive` flake in issue #7365). The PR renames the function to
`resetTransportAndUnlock()`, makes both callers keep `ac.mu` held across the call, and documents
that the callee is responsible for releasing it.

## Evidence traced

**1. Every exit of `resetTransportAndUnlock` releases `ac.mu` exactly once** (`clientconn.go:1234-1304`).
The function is entered with the lock held. It unlocks on the early `acCtx.Err() != nil`
return (line 1237) and otherwise unlocks at line 1262 after moving the state to `Connecting`.
Every later `ac.mu.Lock()` in the function (lines 1266, 1283, 1293, 1301) is paired with an
unlock on all paths, including the `acCtx.Done()` branch of the backoff `select`, which returns
after the lock was already released at line 1278. There is no path that returns while holding
the lock, and no path that unlocks twice.

**2. `connect()` caller** (`clientconn.go:905-924`). The `Shutdown` and non-`Idle` early returns
unlock before returning, and the fall-through now calls `resetTransportAndUnlock()` with the
lock still held. The `Idle -> Connecting` transition therefore happens inside the same critical
section as the idleness check, which closes the race described in the issue. The only caller of
`connect()` is `acBalancerWrapper.Connect()` (`balancer_wrapper.go:276`), which runs it in a
fresh goroutine and holds no locks, so holding `ac.mu` longer inside `connect()` cannot
deadlock against the caller.

**3. `updateAddrs()` caller** (`clientconn.go:940-997`). All early returns still unlock. The
fall-through now does `go ac.resetTransportAndUnlock()` while `ac.mu` is still locked, so the
lock is handed to the new goroutine. `sync.Mutex` is not goroutine-affine, so unlocking in a
different goroutine is legal; `go vet` on the package is clean and the race detector accepts it
(see tests below).

I specifically checked the interaction with the deferred `ac.transport.GracefulClose()` at
line 986, because `http2Client.GracefulClose` (`internal/transport/http2_client.go:1045-1064`)
calls `t.onClose(...)` synchronously and the `onClose` closure built in `createTransport`
(`clientconn.go:1351-1377`) begins with `ac.mu.Lock()`. With this change, that deferred call
now runs while `ac.mu` is still held (pending release by the spawned goroutine), so the
`updateAddrs` caller blocks until the spawned goroutine reaches line 1262. This is not a
deadlock: before releasing the mutex the spawned goroutine only reads `ac.ctx`, `ac.addrs`,
`ac.dopts`, computes the backoff, and calls `updateConnectivityState` (`clientconn.go:1200-1215`),
whose only external call is `acbw.updateState`, which enqueues onto an unbounded
`CallbackSerializer` (`internal/grpcsync/callback_serializer.go:64`) and never blocks. The
blocked `UpdateAddresses` caller (a balancer running on that same serializer) therefore cannot
be something the spawned goroutine waits on. The cost is a brief scheduling stall for the
balancer's `UpdateAddresses` call, which is behavioural noise rather than a defect, and it
also makes the ordering deterministic in a helpful way: `onClose` now always observes the
already-cancelled old `ac.ctx` and returns without touching state.

**4. No other callers or stale references.** `resetTransport` no longer appears anywhere in
non-test Go source. The remaining string matches in `test/goaway_test.go` and
`test/end2end_test.go` are pre-existing expected-log-message filters unrelated to the
function name.

**5. Behavioural scope.** State transitions, backoff handling, `tearDown` interplay and the
`ctx.Err()` checks in `createTransport` are unchanged. Because `tearDown` must take `ac.mu`,
it now cannot interleave between the `updateAddrs` critical section and the spawned
goroutine's `Connecting` transition, which only removes a race window; after the spawned
goroutine unlocks, `tearDown` proceeds as before and the in-flight `tryAllAddrs` observes the
cancelled context. No feature gates, environment variables, ports, build steps, or public APIs
are touched, so there is no devex or feature-leak impact.

## Tests executed (offline, with `-race`)

| Package | Selection | Result |
| --- | --- | --- |
| `google.golang.org/grpc` | `go vet .` | clean |
| `google.golang.org/grpc` | `Test/(UpdateAddresses…, Dial…, Backoff…, EnterIdle…, Balancer_StateListenerBeforeConnect, …)` — 14 subtests | all pass |
| `google.golang.org/grpc/test` | `Test/(StateTransitions…, PickFirst…, ConnectivityStateSubscriber, HealthCheckWithAddrConnDrain, RoundRobin_UpdateAddressAttributes, GracefulClientOnGoAway)` — 28 subtests, exercising `updateAddrs` with live transports | all pass |
| `google.golang.org/grpc/xds/internal/xdsclient/tests` | `Test/AuthorityRevive`, `-count=200` | 200/200 pass, no race reports |

No data races were reported in any run.
