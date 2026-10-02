# Connection lifecycle and mutex ownership

The review found no actionable defects or maintainability regressions in the pinned change. The single changed subsystem is connection admission and transport restart. This report supplies the evidence behind that judgment and evaluates structural alternatives without proposing a checkout edit.

## Scope and measurements

The base is `daab56344e612097fd50c46c433de5d9b6013837`; the head is `76ef33f44a600c3ed1a385979fd1dfbcade3fbb6`. Local `main` and `review-head` resolve to those SHAs. `git diff main...review-head` changes only `clientconn.go`, with six added lines and seven removed lines. `git diff --check main...review-head` reports no whitespace errors.

`git show main:clientconn.go | wc -l` returns 1,839. `wc -l clientconn.go` returns 1,838. The file was already above 1,000 lines; this PR makes it smaller. The change removes three caller-side unlock lines and one initializer-side lock line, renames the initializer at both call sites and its declaration, and adds its contract comment. There are no new conditions, fields, optional modes, casts, or forwarding abstractions.

Source was inspected with `nl -ba clientconn.go` over the caller and transport lifecycle sections, and with `git show main:clientconn.go` for direct comparison. `rg -n 'resetTransport|\.connect\(|updateAddrs\(' --glob '*.go'` establishes that the renamed initializer has exactly two callers, at head lines 922 and 996. The underlying `connect` method is invoked by `acBalancerWrapper.Connect` in a goroutine at `balancer_wrapper.go:275–276`; address updates reach `updateAddrs` through `balancer_wrapper.go:271–272`.

## Connection admission

At the base, `connect` locks `ac.mu`, rejects Shutdown and non-Idle states, unlocks, and calls `resetTransport`, which locks again. Two callers can both pass the Idle check before either initializer publishes Connecting. This is a logical interleaving defect even though the state accesses are individually protected by the mutex.

At the head, `clientconn.go:905–923` keeps the guard and initialization in one uninterrupted critical section. `resetTransportAndUnlock` reads the attempt context and address slice, calculates its backoff and connection deadline, and publishes Connecting at lines 1235–1261. Only then does it release the mutex. A competing `connect` therefore either precedes this admission or sees the updated state after admission. It cannot pass through the former gap.

The canceled-context path at lines 1236–1238 unlocks and returns without publishing Connecting. This is preserved behavior; a canceled connection should not perform new dial work. The initializer itself does not need a duplicate state check because its callers already validate under the inherited lock. Shutdown publishes its state and cancels the attempt while holding that same mutex at lines 1546–1556.

The canonical state updater remains `updateConnectivityState` at lines 1199–1215. It updates the state channel, stored state, channelz state, and balancer notification together. The PR does not move connection admission into the balancer API or invent a separate in-progress flag. Keeping the check and this updater in a single critical section removes the incidental gap rather than compensating for it with another state model.

## Address updates and the goroutine handoff

`updateAddrs` locks at line 948. Equal-address, inactive-state, and still-valid-Ready-address returns explicitly unlock at lines 950, 960, and 971. These paths do not call the initializer.

The restart path cancels the old attempt and replaces its context at lines 980–981. It captures any old transport for deferred graceful closure and clears `ac.transport` at lines 985–987. It then launches `resetTransportAndUnlock` at line 996 while the mutex remains held. Consequently, another address update, shutdown, or explicit connection request cannot alter the current context and address list between restart validation and the new initializer's snapshot.

The lock is handed to a different goroutine, but no concurrent unprotected use follows the handoff in the caller. The caller reaches its return and deferred cleanup; the initializer is responsible for releasing the inherited lock. The doc comment at lines 1231–1233 and the `AndUnlock` suffix explicitly describe this contract. It is a private, bounded contract with both users visible in the same file. The source already uses a consume-and-release naming convention for `updateResolverStateAndUnlock` at line 728, although that function does not itself demonstrate a goroutine handoff.

The deferred cleanup merits a specific deadlock check. `internal/transport/http2_client.go:1045–1055` shows that `GracefulClose` takes the transport mutex and calls `onClose` inline. `clientconn.go:1351–1353` shows that `onClose` takes `ac.mu`. After the PR, deferred graceful closure may reach this callback before the new goroutine runs and temporarily wait for `ac.mu`.

There is no opposing dependency before the initializer's first unlock: lines 1235–1262 read attempt configuration, calculate timing, and queue a connectivity update. `acBalancerWrapper.updateState` at `balancer_wrapper.go:255–264` schedules a callback instead of invoking the balancer inline; `internal/grpcsync/callback_serializer.go:64–65` enqueues it without waiting for its execution. The initializer does not acquire the old transport mutex or wait for old graceful closure before releasing `ac.mu`. The deferred callback can proceed after that release. Dialing and transport acquisition happen afterward.

The old transport callback also sees its old attempt context, canceled by `updateAddrs`. Its check at lines 1356–1361 prevents it from clearing the replacement transport or reverting the new attempt's state. `createTransport` checks the captured context under the mutex at lines 1394–1412 before installing a newly created transport. Those existing generation-by-context protections remain intact.

## Release-path audit

Every reachable return from `resetTransportAndUnlock` leaves `ac.mu` released. The initial canceled-context return unlocks at line 1237. The normal path unlocks at line 1262 before `tryAllAddrs`. On a failed attempt, the canceled-context return unlocks at line 1269; the backoff wait begins after the unlock at line 1278; the context-done branch returns without an outstanding lock. The post-backoff Idle transition unlocks at line 1297 before returning. Successful completion resets backoff under a fresh lock and unlocks at line 1303.

Replacing the contract with a function-wide `defer ac.mu.Unlock()` would be incorrect: most dialing and backoff work must run without the mutex, and the function reacquires the lock in later phases. The current explicit releases express those phase boundaries. The PR removes an entry acquisition; it does not add a new release branch or increase the number of conditional paths that must be audited.

## Worked code-judo alternative: prepare an attempt, then execute it

A possible structural split would keep each lock and its release in the calling goroutine. A small immutable attempt value would carry the existing initial snapshot:

```go
type transportAttempt struct {
    ctx             context.Context
    addrs           []resolver.Address
    backoffFor      time.Duration
    connectDeadline time.Time
}

// Caller holds ac.mu; this function leaves it held.
func (ac *addrConn) prepareTransportLocked() (transportAttempt, bool) {
    if ac.ctx.Err() != nil {
        return transportAttempt{}, false
    }
    backoffFor := ac.dopts.bs.Backoff(ac.backoffIdx)
    dialDuration := minConnectTimeout
    if ac.dopts.minConnectTimeout != nil {
        dialDuration = ac.dopts.minConnectTimeout()
    }
    if dialDuration < backoffFor {
        dialDuration = backoffFor
    }
    attempt := transportAttempt{
        ctx:             ac.ctx,
        addrs:           ac.addrs,
        backoffFor:      backoffFor,
        connectDeadline: time.Now().Add(dialDuration),
    }
    ac.updateConnectivityState(connectivity.Connecting, nil)
    return attempt, true
}
```

After its existing guards, `connect` would prepare, unlock, and synchronously execute the returned attempt. After its existing restart mutations, `updateAddrs` would prepare, unlock, and launch execution in a goroutine. The executor would contain the existing `tryAllAddrs`, failure, backoff, and success suffix, using the four captured values instead of reacquiring an initial snapshot. Cancellation checks in that suffix and in `createTransport` would remain essential.

This design removes the cross-goroutine mutex handoff and the inherited-lock precondition from the long-running executor. It does not delete the guard branches, cancellation model, backoff state, or transport cleanup. It adds an attempt type, a preparation helper, and success checks in both callers. Address-update preparation, the timeout callback, the Connecting notification, and deadline calculation would run earlier in the caller rather than in the scheduled goroutine. Equivalence of those timing changes would need verification; this is a worked design candidate, not an applied or tested remedy.

For this thirteen-line diff, that trade is not a demonstrated dramatic simplification. The current private contract has two callers, explicit documentation, no extra state, and a complete release-path audit. Requiring the split would substitute a larger abstraction change for a small atomicity improvement. It is therefore not an actionable finding in this review.

Another candidate is to set Connecting directly in `connect` and `updateAddrs`, then retain the old self-locking initializer. This can close the first caller's Idle gap, but it duplicates context/state admission policy and leaves restart snapshots separated from address-update validation. Preserving cancellation behavior and restart ordering would require additional reasoning or checks. It does not provide a cleaner canonical boundary than the submitted change.

A file-only extraction could place `addrConn` and its connection methods in a separate Go file in the same package. That would reduce the inherited size of `clientconn.go`, but it would preserve all the lifecycle branches and ownership contracts above. The PR neither creates the existing size problem nor expands it. Physical decomposition is not a necessary remedy for this change.

## Verification and reproducible commands

Tests were run from the clone root, offline, using the provided dependency and build caches. Every command had an external five-minute cap and a four-minute test timeout. The local toolchain reported `go version go1.26.5 linux/amd64`; the target module declares Go 1.21, so these runs do not establish compatibility on every supported toolchain.

The environment prefix for each command was:

```sh
GOMODCACHE=/home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high/att-017/clone-cache/gomodcache \
GOCACHE=/home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high/att-017/clone-cache/gocache \
GOFLAGS=-mod=mod GOPROXY=off GOTOOLCHAIN=local
```

Each of the following commands was executed once with that prefix:

```sh
timeout 300s go test -race -count=20 -timeout=4m -run '^Test/(ResetConnectBackoff|BackoffCancel|UpdateAddresses_NoopIfCalledWithSameAddresses|Balancer_StateListenerBeforeConnect)$' .
timeout 300s go test -race -count=100 -timeout=4m -run '^Test/AuthorityRevive$' ./xds/internal/xdsclient/tests
timeout 300s go test -race -count=10 -timeout=4m -run '^Test/(StateTransitions_.*|SubConnShutdown|ClientConnClose_WithPendingRPC)$' ./test
timeout 300s go test -race -count=100 -timeout=4m -run '^Test/SubConnEmpty$' ./test
```

All returned exit code zero. The package results respectively reported `ok` in 3.866s, 13.942s, 3.800s, and 2.616s. No race-detector reports, test failures, or mutex panics occurred.

`AuthorityRevive` at `xds/internal/xdsclient/tests/authority_test.go:278–314` observes management-server connection creation and checks that revival creates no unexpected extra connection. The root address test exercises the equal-address early return, not the modified restart path. To address that coverage limitation, the separate `SubConnEmpty` run exercises removal of all addresses from an active SubConn and their restoration, through `test/subconn_test.go:83–85,117–124`, including deferred closure and asynchronous initialization. State-transition tests check reconnect sequences and address iteration; shutdown and backoff tests check adjacent lifecycle paths.

This is focused verification, not exhaustive scheduling exploration. The race detector detects unsynchronized memory access; it does not certify logical atomicity. The source-level check-to-transition argument is necessary even when race tests pass. The full suite, a deterministic barrier-driven admission test, and a 100,000-repetition experiment were not performed. Proposed refactor sketches were not compiled or applied.

## Checkout identity and disposition

Before and after test execution, `git status --porcelain=v1` was empty. `git hash-object clientconn.go go.mod go.sum` after execution matched the initial index blob values:

| File | Blob |
| --- | --- |
| `clientconn.go` | `c3cafbc1e55ced7b76e68984c550b5a4b9ee4501` |
| `go.mod` | `ba6b97947e15b5ebd64b3c8638dbfea7f1a77f17` |
| `go.sum` | `7b87042c68700abf5e3304d62b95107f13efb281` |

`git rev-parse 'HEAD^{tree}'` returned `ca9417ec8ca05f79c8bd3511936a4876e4881f83`. No source remedies were applied. All report artifacts were written outside the clone.

Approval is supported by the atomic admission boundary, unchanged canonical lifecycle model, explicit private unlock contract, complete release-path audit, shrinking file, and passing focused checks. There is no required remediation and no unresolved question to index.
