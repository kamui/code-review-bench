# addrConn connection admission and restart

## Scope and judgment

There are no actionable findings in this subsystem. The change makes the existing state-machine transition atomic with its caller's admission decision. It does not add a second synchronization mechanism or a special case. Approval follows both source inspection and focused test execution; it is not based on the pull request's recorded approval or merged status.

The source examined was the committed `main...review-head` range at head `76ef33f44a600c3ed1a385979fd1dfbcade3fbb6`, against base `daab56344e612097fd50c46c433de5d9b6013837`. The only changed file is `clientconn.go`. Supporting evidence came from `balancer_wrapper.go`, `internal/transport/http2_client.go`, `internal/grpcsync/callback_serializer.go`, backoff and dial-option implementations, and the selected test sources. Repository guidance was not loaded as instructions. No network lookup, child reviewer, prior review session, or alternate-model review was used.

## Measurements and commands

`git diff main...review-head` shows three changes: retain the lock in `connect`, retain it in the restart tail of `updateAddrs`, and rename/document the reset helper while removing its initial lock acquisition. `git diff main...review-head --numstat` reports `6 7 clientconn.go`. `git diff main...review-head --check` passes.

`wc -l clientconn.go` reports 1,838 lines; `git show main:clientconn.go | wc -l` reports 1,839. There is no crossing of the 1,000-line threshold. The existing file size is substantial, but adding a decomposition requirement to this small deletion would need a demonstrated structural benefit. The patch does not enlarge the state model or add a feature to the file.

`rg -n 'resetTransport|updateAddrs|\.connect\(' clientconn.go clientconn_test.go balancer_wrapper.go` identifies the two production reset call sites, at head lines 922 and 996. A repository-wide Go-source search found no other caller of the reset helper. Occurrences in test log strings are not calls. `nl -ba clientconn.go` supplied the head line anchors below. `git show main:clientconn.go` supplied the before-change comparison.

## Connection admission evidence

`clientconn.go:905–923` locks `ac.mu`, rejects Shutdown, and returns without connecting for every non-Idle state. Both rejected paths explicitly unlock. On the accepted path, the helper is now invoked with the mutex still held. Its locked prefix reads `ac.ctx` and `ac.addrs`, computes the attempt's backoff/deadline, and calls `updateConnectivityState(Connecting, nil)` before unlocking at lines 1261–1262.

Previously, the successful admission path unlocked before invoking a helper that acquired the lock itself. Two goroutines could each pass the Idle check before either published Connecting. Holding the same mutex across these operations deletes that gap. A second `connect` call must wait and then sees Connecting unless the first attempt has already completed or legitimately changed state. A later retry after a completed attempt is a separate state transition, not the duplicate admission being fixed.

`balancer_wrapper.go:275–277` already launches `connect` in a goroutine. The change does not move network dialing into a synchronous balancer call. `updateConnectivityState` still uses the canonical state-change channel, channelz metrics, and balancer notification path. `acBalancerWrapper.updateState` schedules its listener through `CallbackSerializer`; `Schedule` enqueues the callback rather than invoking the listener inline. Publishing Connecting under `ac.mu` therefore does not newly require a synchronous balancer callback to complete.

## Address-update evidence and lock ownership

`clientconn.go:948–973` acquires the same mutex and explicitly unlocks on unchanged addresses, Shutdown/TransientFailure/Idle, and a Ready transport whose address remains valid. These branches are unchanged. Only the path that cancels and replaces a connection attempt hands the still-held mutex to `go ac.resetTransportAndUnlock()` at line 996.

That transfer is legal for a Go mutex: it need not be unlocked by the goroutine that locked it. The important local invariant is that the original goroutine does no further protected-field work after the launch and does not also unlock. At the head, the launch is its last statement. The worker reads the current context and addresses before releasing the lock, so another address update or `connect` cannot enter between installing the new attempt context and establishing the new attempt's initial state. Each later restart cancels its predecessor's context before installing a replacement.

The deferred `ac.transport.GracefulClose()` at lines 983–988 is the delicate boundary. `internal/transport/http2_client.go:1045–1065` calls `onClose` synchronously while closing an active transport. The `onClose` closure in `clientconn.go:1352` takes `ac.mu`, so the deferred close may temporarily block until the worker unlocks. This is not a cyclic dependency: the worker's locked prefix performs context checks, deadline/backoff calculation, and queued state publication, then unlocks before `tryAllAddrs`. It does not wait for the old transport to close. The canceled old context also makes the old callback return without clearing a replacement transport or publishing an obsolete state.

The backoff strategy and optional minimum-timeout calculation were already inside the reset helper's original critical section. Their placement is not a new call-under-lock regression. The production backoff implementation calculates a duration; the arbitrary minimum-timeout callback is a private testing option. Source inspection did not establish a callback dependency on the deferred old-transport close.

## Unlock and cancellation audit

At `clientconn.go:1235–1238`, an already-canceled attempt context causes an immediate unlock and return. Otherwise the first unlock is at line 1262, before transport construction. Subsequent exits belong to the helper's existing ordinary lock/unlock sections: cancellation following address failure unlocks at line 1269; the backoff timer/select runs after the unlock at line 1278; the context-done select branch returns while unlocked; the final Idle publication unlocks at line 1297; and success resets the backoff index and unlocks at line 1303. No return path retains the caller's initial lock.

`tearDown` continues to publish Shutdown and cancel the context while holding `ac.mu`. It cannot race between admission and Connecting publication. After the reset helper unlocks, shutdown can proceed normally. `createTransport` still checks the attempt context before installing a newly created transport and asynchronously closes an obsolete one, avoiding inline close-callback lock recursion. The helper's post-failure state update also checks that the captured attempt context is still live.

The name `resetTransportAndUnlock` and its comment at lines 1231–1234 make the exceptional ownership boundary explicit. `updateResolverStateAndUnlock` is an existing analogous naming convention in this same file. A custom mutex or an additional lock-state boolean would add another invariant rather than improve the state-machine model. There is no evidence of an unlocked caller that would justify such machinery.

## Worked code-judo alternatives

The patch itself performs the strongest local simplification: it uses the existing mutex and Connecting state to make admission atomic, deleting an unnecessary unlock/relock pair. It leaves all transport retry/backoff behavior in its current canonical implementation.

One alternative is to separate preparation under the lock from execution outside it. A preparation helper could capture the same context, addresses, backoff, and deadline, publish Connecting, and return an execution closure. The following is a design sketch, not an applied or compiled replacement:

```go
// Caller holds ac.mu; this helper leaves it held.
func (ac *addrConn) prepareTransportAttemptLocked() func() {
    acCtx := ac.ctx
    if acCtx.Err() != nil {
        return nil
    }
    addrs := ac.addrs
    backoffFor := ac.dopts.bs.Backoff(ac.backoffIdx)
    // Compute connectDeadline using the existing timeout/backoff rules.
    connectDeadline := /* existing calculation */
    ac.updateConnectivityState(connectivity.Connecting, nil)
    return func() {
        ac.runTransportAttempt(acCtx, addrs, connectDeadline, backoffFor)
    }
}

// connect's accepted Idle path:
attempt := ac.prepareTransportAttemptLocked()
ac.mu.Unlock()
if attempt != nil {
    attempt() // Preserve connect's synchronous private-method behavior.
}

// updateAddrs' restart tail:
attempt := ac.prepareTransportAttemptLocked()
ac.mu.Unlock()
if attempt != nil {
    go attempt()
}
```

`runTransportAttempt` would contain the unchanged portion starting at the existing `tryAllAddrs` call, including error resolution, cancellation checks, TransientFailure publication, backoff, Idle publication, and success bookkeeping. It must use the captured context and address slice, not reacquire them from the live `addrConn`, to preserve restart isolation. Moving the state transition into the preparation step makes its placement explicit and eliminates cross-goroutine unlocking.

This proposal has a real ownership benefit, but it introduces a closure boundary, a new execution helper, nullable preparation results, and handling at both call sites. It also moves address-update state publication from the worker into the caller, requiring assessment of notification timing. A typed snapshot would replace the closure with a new data model and similar orchestration. Neither alternative deletes enough concepts to establish a dramatic maintainability improvement over this 13-line diff. These are evaluated alternatives, not requested remediation or hidden actionable findings.

Merely setting Connecting in `connect` before calling the old reset helper is a smaller-looking alternative, but it duplicates state publication and leaves the address-update unlock/reacquire window in place. Address updates can replace contexts and addresses while another restart waits to acquire the mutex, allowing workers to capture the same latest attempt. Repairing only one caller is an incomplete restructuring. Adding an in-flight boolean or a second mutex would create extra state that the existing connectivity model already provides.

A broader extraction of `addrConn` into its own source file could improve navigation in the pre-existing 1,839-line file. It would move the connection-state implementation, its connection/admission helpers, and supporting utilities together; it would not reduce the synchronization concepts or branch count. The current PR reduces the file size and adds no responsibility, so that independent organizational change is not an approval blocker here.

## Verification

All commands ran from the clone root with the required offline environment. The environment prefix used on each command was:

```sh
GOMODCACHE=/home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high/att-021/clone-cache/gomodcache
GOCACHE=/home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high/att-021/clone-cache/gocache
GOFLAGS=-mod=mod
GOPROXY=off
GOTOOLCHAIN=local
```

The following three focused invocations completed successfully, each executed once with its listed flags:

```sh
go test -race . -run '^Test/(UpdateAddresses_NoopIfCalledWithSameAddresses|ResolverEmptyUpdateNotPanic|BackoffCancel|ResetConnectBackoff|DialContextCancel)$' -count=1 -timeout=240s
go test -race ./xds/internal/xdsclient/tests -run '^Test/AuthorityRevive$' -count=20 -timeout=240s
go test -race ./test -run '^Test/(StateTransitions_ReadyToConnecting|PickFirst_NewAddressWhileBlocking|PickFirst_AddressesRemoved|PickFirst_ResolverError_ZeroAddresses_WithPreviousUpdate)$' -count=1 -timeout=240s
```

The reported package durations were 2.220 seconds, 3.667 seconds, and 1.777 seconds respectively. No command approached the five-minute allowance. AuthorityRevive observes accepted connections, so it checks the duplicate-transport symptom in addition to data-race detection. The root tests cover cancellation/backoff and unchanged-address behavior. The `./test` selections cover reconnection, replacement of Ready addresses, and empty-address recovery/error handling. Passing repetitions do not prove schedule completeness; logical duplicate admissions can be race-detector-clean, so the mutex/state analysis remains essential.

No replacement implementation, new test, or checkout edit was made. No full-suite run, upstream discussion fetch, or network dependency download occurred. The packet's 100,000-run claim was not independently repeated.

After testing, `git diff HEAD --exit-code` passed and `git status --porcelain=v1 --untracked-files=all` was empty. `git hash-object clientconn.go go.mod go.sum` matched their previously recorded index blob hashes: `c3cafbc1e55ced7b76e68984c550b5a4b9ee4501`, `ba6b97947e15b5ebd64b3c8638dbfea7f1a77f17`, and `7b87042c68700abf5e3304d62b95107f13efb281`. The head tree remained `ca9417ec8ca05f79c8bd3511936a4876e4881f83`. Reports were written only under the requested work-directory report path.

## Remediation disposition

No actionable remediation is needed for this PR. Keep the caller-held lock until Connecting is published and keep the helper's lock-consuming contract explicit. Any future preparation/execution split should preserve the captured attempt context, both caller paths, close-callback ordering, and the existing cancellation/backoff behavior. The source evidence and verification above satisfy the strict approval bar for this narrow change.
