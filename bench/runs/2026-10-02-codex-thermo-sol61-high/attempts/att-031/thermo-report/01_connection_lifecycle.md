# Connection lifecycle and mutex ownership

## Scope and judgment

The only modified subsystem is addrConn connection initiation in `clientconn.go`. There are no actionable findings. The PR improves atomicity using the existing state machine and removes a synchronization gap with very little structural churn.

This is a source-based review of the pinned base and head, supported by focused offline race tests. Historical approvals and comments in the supplied packet were not used as evidence of correctness. No external sources were fetched. The frozen skill did not require a child call or reference resource, so neither was used.

## Measurements and reproducible inspection

The inspection commands included:

```sh
git diff main...review-head
git diff --numstat main...review-head
git diff --check main...review-head
git rev-parse main review-head 'HEAD^{tree}'
git show main:clientconn.go | wc -l
wc -l clientconn.go
rg -n 'resetTransport|func .*connect|func .*updateState|func .*updateConnectivityState|func .*tearDown|func .*ResolveNow|func .*resetBackoff' clientconn.go balancer_wrapper.go resolver_wrapper.go
rg -n 'AndUnlock|go .*Unlock' --glob '*.go' --glob '!**/*test.go' .
```

The range modifies one file with six added and seven removed lines. `clientconn.go` measures 1,839 lines at the base and 1,838 at the head. The whitespace check passes. Base and head resolve to the packet's pinned SHAs. The head tree is `ca9417ec8ca05f79c8bd3511936a4876e4881f83`.

Exactly two production call sites invoke the renamed helper: the synchronous call in `connect` at line 922, and the goroutine launch in `updateAddrs` at line 996. There are no remaining calls to the old name. The diff changes no conditions, state enum, fields, transport API, or backoff algorithm. It removes the helper's initial lock acquisition and the callers' preceding unlocks, and documents the resulting contract.

Source inspection covered `connect`, all exits from `updateAddrs`, the complete reset helper, `tryAllAddrs`, `createTransport` and its `onClose`, health-check state updates, backoff reset, transport retrieval, and `tearDown`. Related source evidence includes `acBalancerWrapper.Connect` and `updateState`, `CallbackSerializer.Schedule`, `ClientConn.Close`, and `http2Client.Close`/`GracefulClose`.

## Atomicity evidence

At the base, two calls to `connect` can both observe `Idle`: caller A locks, checks, and unlocks; caller B locks, checks, and unlocks before A's reset helper reacquires the mutex. Both then enter the unconditional initializer and can create transports for the same addrConn. Merely locking the initializer does not validate that its caller's earlier observation is still current.

At the head, `connect` acquires `ac.mu` at line 906. Its shutdown and non-idle exits release it at lines 911 and 918. Its remaining path calls `resetTransportAndUnlock` without releasing the mutex. The helper copies the context and addresses, calculates timing, publishes `Connecting` at line 1261, and unlocks at line 1262. A second `connect` can only inspect the state after that transition, and returns through the non-idle branch. The failed-context exit at lines 1236–1238 unlocks without publishing a new state.

This keeps the attempt's context, address snapshot, and initial state publication in one critical section. The helper remains unconditional because address replacement also needs to initiate an attempt while replacing an existing connection or attempt. Moving the idle predicate into this helper would require distinguishing the callers' different initiation rules.

## Address replacement and asynchronous handoff

`updateAddrs` acquires the mutex at line 948. All early exits release it: unchanged addresses at line 950; shutdown, transient failure, or idle at line 960; a still-valid ready address at line 971. The remaining path cancels the old attempt, replaces the context, clears the current transport if present, optionally publishes `Idle` for an empty list, then starts the worker at line 996 while retaining the mutex.

The worker inherits the obligation to release the mutex. Go mutexes do not bind unlock to the locking goroutine, so the handoff itself is valid. Until the worker releases the mutex, another update, connect request, shutdown, or old transport callback cannot inspect half-applied replacement state. The worker captures the new context before releasing the mutex, so a later replacement can cancel that specific attempt rather than having two delayed workers both capture the newest context.

An empty replacement list can temporarily publish `Idle`, as at the base, but competing `connect` calls cannot observe that state in the gap before the replacement worker publishes `Connecting`. No extra branch is needed to close that gap.

## Deferred close and lock ordering

At lines 983–987, `updateAddrs` defers the old transport's `GracefulClose`. `http2Client.GracefulClose` invokes `onClose` synchronously while holding the transport mutex (`internal/transport/http2_client.go:1045` onward); `createTransport`'s callback then acquires `ac.mu` (`clientconn.go:1351` onward). At the head, the defer can start before the replacement worker has unlocked `ac.mu`, so the callback may wait for that worker.

The worker's initial critical section does not need the old transport mutex or wait for the deferred close. State publication calls `acBalancerWrapper.updateState`, which schedules a callback via `CallbackSerializer.Schedule`; it does not synchronously invoke the balancer's listener. The worker then unlocks at line 1262 before calling `tryAllAddrs` or creating a transport. Alternatively, its canceled-context path unlocks at line 1237. Consequently there is no circular wait introduced between replacement initialization and old transport closure.

The old callback sees its captured context canceled and returns without clearing the replacement transport or changing replacement state. No transport work was newly moved under the initial addrConn critical section. Backoff and timeout calculation were already inside that critical section in the base helper.

## Completion, cancellation, and shutdown

After the initial unlock, the unchanged helper reacquires `ac.mu` around failure state publication, backoff accounting, return to idle, and successful backoff reset. Its error exit releases the mutex at line 1269; its transient-failure path releases at line 1278; both timer increment and final idle publication have paired unlocks. The cancellation select returns after the prior unlock. The success path pairs its lock at line 1301 with its unlock at line 1303.

`tearDown` publishes shutdown and cancels the attempt while holding `ac.mu`, then releases it before transport closure. A competing connect call either wins the lock and begins a cancellable attempt before teardown, or sees shutdown afterward. A canceled worker returns without overwriting shutdown. `createTransport` checks the attempt context after acquiring the mutex and closes a transport created for a canceled attempt in a goroutine, preserving its existing callback-safe closure pattern.

Other unchanged lifecycle subtleties, such as backoff reset after an old attempt completes, were inspected for interaction with the new boundary. They are not new behaviors caused by this diff. No concrete correctness regression was established.

## Worked code-judo proposals

### Retain the implemented ownership move

The useful restructuring is to remove the release/reacquire boundary and let the shared initializer consume the caller's lock:

```go
// connect: after verifying Idle with ac.mu held
ac.resetTransportAndUnlock()

// updateAddrs: after replacing context and clearing transport with ac.mu held
go ac.resetTransportAndUnlock()

// shared initializer: capture one attempt and publish its state atomically
acCtx := ac.ctx
if acCtx.Err() != nil {
    ac.mu.Unlock()
    return
}
// Existing address/backoff/deadline preparation stays here.
ac.updateConnectivityState(connectivity.Connecting, nil)
ac.mu.Unlock()
// Existing dial/backoff completion stays outside the initial critical section.
```

This sketch describes the actual change and deliberately leaves the existing timing code in place. It deletes a lock gap without adding a concept. It retains different caller eligibility rules while sharing the canonical attempt initialization. No further remediation is needed.

### Separate preparation from execution: evaluated, not recommended here

A possible alternative would introduce `prepareAttemptLocked() (connectionAttempt, bool)` and `runAttempt(connectionAttempt)`. The attempt value would carry the context, address snapshot, backoff duration, and connection deadline. Each caller would prepare under its own lock, explicitly unlock, and then synchronously run or launch the returned attempt. Address replacement could close its old transport after the local unlock without transferring mutex ownership to a goroutine.

The behavior-preserving sequence would be: validate caller eligibility; cancel and replace the old context where needed; check the new context; capture all four attempt inputs; publish `Connecting`; unlock; execute dialing and backoff using those inputs. The empty-address transition and canceled-context behavior must remain intact. A separate attempt value must not reread `ac.ctx` after the unlock.

This alternative makes ownership more local, but adds a type, helper, validity result, and caller orchestration. It does not delete the connection state machine, backoff branches, cancellation model, or callback constraints. It would also change the deadline's relation to goroutine scheduling unless preparation timing were preserved deliberately. For two small internal call sites, this is a tradeoff rather than an obvious dramatic simplification. It is not an actionable finding or approval blocker.

### Move Connecting into the callers: rejected

Setting `Connecting` directly in `connect` would reserve the connection before unlocking, but address replacement still needs its own reservation, context snapshot, and reset path. To preserve the current canceled-context behavior, callers would need the helper's context predicate as well. Duplicating these rules would scatter the attempt contract across the callers and the shared helper. Adding a generic reservation flag would create another state variable that must agree with `ac.state`. The implemented single critical section is simpler.

### File decomposition: evaluated, not warranted by this diff

The file was already larger than 1,000 lines and shrinks here. Extracting the addrConn lifecycle into a separate Go source file is possible without changing package visibility, but would only relocate the same ownership protocol. No new subsystem or distinct policy introduced by this diff warrants extraction, and moving the entire pre-existing lifecycle would greatly enlarge the review surface. No file-size finding is raised.

## Verification record

All three commands ran from the clone root. Each package was tested once for its selected flag set, and each command had a five-minute outer limit. The environment was exactly:

```sh
GOMODCACHE=/home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high/att-031/clone-cache/gomodcache
GOCACHE=/home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high/att-031/clone-cache/gocache
GOFLAGS=-mod=mod
GOPROXY=off
GOTOOLCHAIN=local
```

With these variables supplied through `env`, the executed test commands were:

```sh
timeout 300s go test -race -count=10 -timeout=240s -run '^Test/(UpdateAddresses_NoopIfCalledWithSameAddresses|ResetConnectBackoff|Balancer_StateListenerBeforeConnect|CloseConnectionWhenServerPrefaceNotReceived)$' .
timeout 300s go test -race -count=20 -timeout=240s -run '^Test/AuthorityRevive$' ./xds/internal/xdsclient/tests
timeout 300s go test -race -count=5 -timeout=240s -run '^Test/(PickFirst_NewAddressWhileBlocking|RoundRobin_NewAddressWhileBlocking|SubConnEmpty|SubConnShutdown|StateTransitions_ReadyToConnecting)$' ./test
```

All exited zero. The root package reported `ok google.golang.org/grpc 32.603s`; xDS reported `ok google.golang.org/grpc/xds/internal/xdsclient/tests 3.697s`; integration tests reported `ok google.golang.org/grpc/test 4.082s`. No race, panic, deadlock, or test failure was reported.

`AuthorityRevive` asserts that reviving an authority creates no extra management-server transport. The root address test covers the unchanged early exit while connecting; the integration `SubConnEmpty` test directly exercises removal and readdition on the same SubConn. The other selections support shutdown, reconnect, backoff, and empty-address lifecycle compatibility. Their subtest names were checked against `grpctest.RunSubTests`, which strips the `Test` method prefix.

No new tests or scratch reproductions were added to the checkout. No full-suite run or 100,000-attempt stress reproduction was performed. Race instrumentation cannot detect every logical connection race, so the exact mutex/control-flow analysis above is necessary alongside these successful runs.

Final tracked and untracked status checks were clean, and both worktree and index diffs were empty. The repository was not edited. Reports and the locator index were written only under the authorized work directory.

## Actionable remediation disposition

None. Preserve the atomic initialization boundary and explicit ownership contract in this PR. The alternatives above are worked design assessments, not requested changes or unresolved questions.
