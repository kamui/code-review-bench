# Connection lifecycle: evidence and structural review

## Scope and measurements

The review covers the committed diff `main...review-head`, whose endpoints are base `daab56344e612097fd50c46c433de5d9b6013837` and head `76ef33f44a600c3ed1a385979fd1dfbcade3fbb6`. No checkout files were edited. No child reviewer was required by the frozen skill, and none was started. This report's judgment comes from the committed source and the focused executions below.

`git diff --numstat main...review-head` reports `6 7 clientconn.go`. `wc -l clientconn.go` reports 1,838 lines, and `git show main:clientconn.go | wc -l` reports 1,839. The three hunks remove the unlock in `connect`, remove the unlock before the asynchronous restart in `updateAddrs`, and replace the helper's initial lock acquisition with a documented incoming-lock contract. There are exactly two helper call sites: lines 922 and 996. No new conditional, state field, parameter, package dependency, or wrapper is introduced.

Source inspection used `nl -ba clientconn.go` over lines 840–1040 and 1150–1585, targeted `rg` searches for `resetTransport`, `updateAddrs`, `connect`, and backoff operations, and reads of `balancer_wrapper.go`, `internal/transport/http2_client.go`, and `internal/grpcsync/callback_serializer.go`. Existing tests were read at `clientconn_test.go:859–919`, the remainder of the address-update test, `xds/internal/xdsclient/tests/authority_test.go:278–314`, and the relevant SubConn and state-transition tests. Repository or ancestor guidance was not loaded.

## Admission invariant and interleavings

At `clientconn.go:906`, `connect` takes `ac.mu`. A shutdown connection unlocks and returns an error at lines 907–912. Any other non-idle state unlocks and returns at lines 914–919. Only the idle branch enters `resetTransportAndUnlock` at line 922, carrying the same mutex. At lines 1235–1262, the helper checks cancellation, snapshots the context and addresses, computes timing, publishes `Connecting`, and unlocks. This is the existing initialization sequence, with its entry ownership changed.

On the base, call A can observe `Idle`, unlock, and pause before `resetTransport` acquires the mutex. Call B can also observe `Idle` and enter the helper. The helper has no second idle admission check, so both calls can snapshot the same live context and attempt a transport. At head, call B must wait until call A has either published `Connecting` and unlocked, or observed cancellation and unlocked. In the live case, B then exits through the non-idle guard. In the canceled case, no transport attempt has been admitted. This is a source-level proof that the specific admission gap is removed, independent of how often a stress test schedules it.

`updateAddrs` takes the mutex at line 948. Identical addresses return after unlocking at line 950. Shutdown, transient-failure, and idle connections update the list and return after unlocking at line 960. A ready connection whose current address remains valid returns after unlocking at line 971. The remaining cases cancel the old context, create a replacement context, detach any current transport, and initiate a restart. At line 996, the newly started goroutine inherits responsibility for the held mutex. Competing updates, calls to `connect`, transport callbacks, and shutdown cannot enter the protected state between that replacement and the helper's snapshot.

The handoff is legal for a Go mutex, which has no goroutine ownership. After starting the goroutine, `updateAddrs` does not access protected fields except through its deferred transport close. That deferred receiver was evaluated when the defer was registered at line 986, while the caller held the mutex. It refers to the detached old transport rather than a later replacement.

## Unlock and callback audit

| Path | Incoming-lock release | Subsequent behavior |
| --- | --- | --- |
| Helper sees a canceled context | Line 1237 | Returns without dialing or changing state |
| Live initialization | Line 1262 | Begins `tryAllAddrs` without the incoming lock |
| Failed attempt, then canceled context | Fresh acquisition at line 1266; release at line 1269 | Returns without publishing a stale failure |
| Failed attempt with backoff | Fresh acquisition at line 1266; release at line 1278 | Timer and context waits run unlocked |
| Backoff timer fires | Fresh acquisition at line 1283; release at line 1285 | Advances the existing backoff index |
| Backoff finishes | Fresh acquisition at line 1293; release at line 1297 | Publishes idle only if the attempt context is live |
| Successful attempt | Fresh acquisition at line 1301; release at line 1303 | Resets the existing backoff index |

No normal helper exit leaves its incoming mutex locked. Every post-dial acquisition remains paired as before. A broad deferred unlock at helper entry would be incorrect: the helper deliberately unlocks before I/O and subsequently acquires and releases the mutex several times. The explicit ownership-consuming name accurately signals this distinction from a conventional helper that remains locked throughout.

`internal/transport/http2_client.go:1045–1063` shows that `GracefulClose` takes the transport mutex and invokes `onClose` inline. The callback in `clientconn.go:1351–1377` takes `ac.mu`, confirming why `updateAddrs` cannot synchronously run a whole connection attempt while keeping the mutex and defer transport closure behind that attempt. With the reviewed handoff, the deferred close may wait for `ac.mu`, but the worker's initial section does not acquire the old transport mutex or require old-transport closure. It unlocks at line 1237 or 1262, allowing the callback to proceed. The callback observes the canceled old context and returns at lines 1356–1361. No new wait cycle is present in this path.

State notifications at `clientconn.go:1214` call `acBalancerWrapper.updateState`. `balancer_wrapper.go:255–264` queues the listener through `CallbackSerializer.Schedule`; `internal/grpcsync/callback_serializer.go:64–65` inserts the callback rather than executing it inline. A listener that calls `Connect`, `UpdateAddresses`, or shutdown therefore does not recursively take `ac.mu` from the helper's state publication stack. A concurrent listener can wait briefly for the worker's initialization to finish, which is precisely the protected admission interval.

Shutdown is still mediated by `tearDown`, which takes `ac.mu`, publishes `Shutdown`, cancels the context, and unlocks before closing a transport (`clientconn.go:1545–1583`). The updated admission interval prevents it from interleaving between `connect`'s state validation and the helper's initial transition. Cancellation after the helper unlocks continues to be checked in the existing attempt and transport-installation paths. `createTransport` checks the captured context before installing the transport at lines 1394–1413 and closes an obsolete newly created transport asynchronously, preserving the callback locking rule.

## Worked code-judo alternative: prepare an attempt, then execute

The most credible ownership alternative separates the helper's locked prefix from its unlocked tail. A candidate model would hold exactly the snapshot currently made under the mutex:

```go
type connectionAttempt struct {
    ctx      context.Context
    addrs    []resolver.Address
    backoff  time.Duration
    deadline time.Time
}

// Caller holds ac.mu; this helper leaves it held.
func (ac *addrConn) prepareAttemptLocked() *connectionAttempt {
    if ac.ctx.Err() != nil {
        return nil
    }
    // Compute the same backoff and deadline as the current locked prefix.
    attempt := &connectionAttempt{
        ctx: ac.ctx, addrs: ac.addrs,
        backoff: backoffFor, deadline: connectDeadline,
    }
    ac.updateConnectivityState(connectivity.Connecting, nil)
    return attempt
}
```

This is a design sketch, not compilable replacement code: the timing calculation is intentionally represented by the existing `backoffFor` and `connectDeadline` values. The current tail would become `runAttempt(attempt)`, using the snapshot for `tryAllAddrs`, cancellation checks, and backoff. `connect` would prepare the attempt, unlock itself, and synchronously run a non-nil attempt. `updateAddrs` would prepare the attempt, unlock itself, and launch a non-nil attempt in a goroutine before its deferred close runs.

That alternative removes mutex ownership transfer between goroutines. To preserve behavior it must retain the canceled-context early return, compute the same timing under the lock, capture the replaced context rather than reread `ac.ctx` later, publish state in the same order, and keep transport closure outside the mutex. It must also retain the current unlocked I/O and post-I/O synchronization. It adds an attempt model, a second helper, and a nil-result branch at each caller. It moves synchronous state notification scheduling into `updateAddrs` before spawning the worker, which is an observable ordering choice to verify rather than assume away.

For two local callers and a small diff, this does not remove enough machinery to establish an obvious dramatic simplification. The current code already has a single initialization implementation and an explicit contract. The alternative is therefore not a finding or required remediation. It would need an independent reason, such as a broader lifecycle redesign, to justify the larger change.

## Other simplifications considered

Moving just `updateConnectivityState(Connecting, nil)` into `connect` appears smaller but distributes admission and initialization across callers. It needs care around canceled contexts and does not itself protect `updateAddrs`' replacement context and address snapshot. Duplicating the transition at both callers would increase state-machine knowledge at the call sites. The reviewed helper keeps that knowledge canonical and fixes both admission windows with fewer moving pieces.

Extracting the roughly 440-line `addrConn` lifecycle section into another file would reduce the size of `clientconn.go` without changing package boundaries, but would not simplify this PR's ownership problem. Most of that implementation predates the change. The file is already large, yet the patch reduces its size and adds no unrelated feature logic. No PR-induced file-size blocker or clear decomposition opportunity was found.

Adding a `TryLock` probe would not prove that a particular caller holds the mutex: failure can indicate any goroutine holds it, while success mutates lock state. No new probing or runtime ownership abstraction is needed to express this private method's two audited call sites. The name and comment state the actual contract directly.

## Focused execution and verification status

All commands were launched from the clone root. Define the following literal cache paths in the command environment:

```sh
GOMODCACHE=/home/jack/.t3/bench-runs-lanes/codex-thermo-sol61/2026-10-10-last-push-codex-thermo-sol61/att-015/clone-cache/gomodcache
GOCACHE=/home/jack/.t3/bench-runs-lanes/codex-thermo-sol61/2026-10-10-last-push-codex-thermo-sol61/att-015/clone-cache/gocache
GOFLAGS=-mod=mod
GOPROXY=off
GOTOOLCHAIN=local
```

Each command used `timeout 300s env` with those five assignments followed by the exact Go invocation below. Each package was executed once for its flag set. No module proxy or upstream network access was used.

```sh
go test -race -run '^Test/(UpdateAddresses_NoopIfCalledWithSameAddresses|ResetConnectBackoff|BackoffCancel|ResolverEmptyUpdateNotPanic|Balancer_StateListenerBeforeConnect)$' -count=10 -timeout=240s .
go test -race -run '^Test/AuthorityRevive$' -count=50 -timeout=240s ./xds/internal/xdsclient/tests
go test -race -run '^Test/(SubConnEmpty|SubConnShutdown|StateTransitions_ReadyToConnecting|StateTransitions_TriesAllAddrsBeforeTransientFailure|StateTransitions_MultipleAddrsEntersReady)$' -count=10 -timeout=240s ./test
```

The root package passed in 12.486 seconds. The xDS client test package passed in 7.422 seconds. The focused end-to-end package passed in 2.019 seconds. All three commands exited with status zero, and no race reports appeared. Test selectors were checked against the source declarations. The xDS test explicitly rejects an unexpected new management-server transport and an unexpected close after authority revival. The end-to-end tests cover empty-address replacement, shutdown from a state listener, reconnection after ready, trying addresses before transient failure, and selecting a viable address. These tests complement the source audit but do not exhaust possible interleavings.

`git diff --check main...review-head` exited successfully without output. Before and after execution, `git status --porcelain=v1 --untracked-files=all` was empty and `git rev-parse HEAD^{tree}` returned `ca9417ec8ca05f79c8bd3511936a4876e4881f83`. The worktree and pinned commit tree remained unchanged.

The full repository suite was not run because the policy allows focused packages only. The author's 100,000-attempt result was not reproduced. The alternative designs above were reviewed as sketches and were not applied or tested; no clone mutation or remedy was authorized or performed.

## Findings and remediation

There are no actionable findings or unresolved review questions in this subsystem. The change removes a non-atomic interval, keeps transport initialization in the existing owning layer, introduces no spaghetti growth, and documents its ownership boundary. No remediation is required. The structural alternatives above are considered and rejected as blockers, rather than being hidden additional recommendations.
