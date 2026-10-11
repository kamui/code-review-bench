This subsystem review covers connection-attempt reservation, address replacement, cancellation, and transport-close interactions for `grpc/grpc-go#7390`. It uses the committed `main...review-head` diff and local source evidence. There are no actionable findings and no open questions. No independent reviewer was started: the frozen skill does not require a child call, and this task prohibits discretionary delegation.

The checkout head is `76ef33f44a600c3ed1a385979fd1dfbcade3fbb6`; `main` is `daab56344e612097fd50c46c433de5d9b6013837`. The review read the explicitly selected frozen skill. It loaded no ambient repository guidance, personal configuration, additional skill, upstream discussion, or reference answer. The conclusions below come from the diff, supporting source, and tests, rather than prior review judgments in the packet.

Measurements were obtained with `git diff main...review-head`, `git diff --numstat main...review-head`, `wc -l clientconn.go`, and `git show main:clientconn.go | wc -l`. Function locations were obtained with `rg -n '^func |^type ' clientconn.go`; relevant call sites were checked with `rg -n 'resetTransportAndUnlock|resetTransport\(' --glob '*.go' .`. Source excerpts were read using `nl -ba` and `sed`, and compared with `git show main:clientconn.go`.

| Measurement | Base | Head | Interpretation |
| --- | --- | --- | --- |
| `clientconn.go` physical lines | 1,839 | 1,838 | Already over 1,000; shrinks by one line |
| Changed files | — | 1 | Only `clientconn.go` |
| Diff additions/deletions | — | +6 / −7 | Includes three documentation lines |
| Reset callers | 2 | 2 | `connect` and `updateAddrs` |
| Reset implementation bodies | 1 | 1 | Renamed, with entry lock removed |
| Added conditionals, loops, types, public APIs | — | 0 | Existing control flow retained |

The changed helper is an existing implementation, not a new pass-through abstraction. The three changed regions are the successful path of `connect`, the restart path of `updateAddrs`, and the reset method's entry contract. No signature changes outside this private implementation occur. No new optionality, casts, generic mechanism, policy flag, or package dependency is introduced.

At the base, `connect` released `ac.mu` after checking Idle and called a reset method that acquired the mutex again. An interleaving was possible in which callers A and B both observed Idle before either reset operation set Connecting. The reset implementation was unconditional, so both could proceed to transport creation. Serializing the reset entries alone did not serialize the callers' state eligibility checks with attempt reservation.

At the head, `clientconn.go:906` acquires `ac.mu`. Shutdown and non-Idle exits explicitly unlock at lines 911 and 918. The Idle path reaches `resetTransportAndUnlock` at line 922 without unlocking. That method captures the attempt context at line 1235, returns with an unlock on cancellation at lines 1236–1238, captures addresses and backoff settings at lines 1241–1259, publishes Connecting at line 1261, and unlocks at line 1262. A competing `connect` cannot acquire the mutex during this sequence. Once it can acquire the mutex, it sees the changed state unless another legitimate lifecycle event has subsequently changed it.

This is a coherent atomic boundary: eligibility, attempt snapshot, and state reservation belong to one critical section. The implementation removes the lock gap instead of adding a second mechanism for recording an in-progress attempt. It retains `updateConnectivityState` as the canonical publisher, including its state channel and channelz updates at `clientconn.go:1199–1215`.

The canceled-context path also preserves the old behavior. It releases the mutex and returns before setting Connecting. `tearDown` obtains the same mutex and sets Shutdown before canceling the context at `clientconn.go:1546–1556`. A completed subconnection teardown therefore cannot be followed by an admitted `connect` setting Connecting. Cancellation inherited from the channel can occur independently, but this change retains the existing context checks and does not invent a different state policy for that case.

The other caller, `updateAddrs`, needs an unconditional reset after replacing an active attempt or an unsuitable connected address. Its existing early returns remain: equal addresses, inactive states, and a Ready connection whose current address remains valid all unlock locally at `clientconn.go:948–974`. The restart path cancels the old context, creates a new context, detaches the old transport, and may publish Idle for an empty address list at lines 980–991. The call at line 996 starts the worker while retaining the mutex. That worker consumes the new attempt context and addresses before another address update can interleave.

The cross-goroutine mutex handoff is the most subtle part of the diff. The parent executes no further protected-field accesses after starting the worker; any deferred close operates on the transport receiver already captured by the defer statement. The child does not acquire the mutex on entry, because it receives the locked critical section. It releases it on both initial exit routes. There is no leftover parent unlock or defer that could double-unlock it. All subsequent lock acquisitions and releases inside the reset implementation remain as at the base.

`updateAddrs` defers the old transport's `GracefulClose` because `internal/transport/http2_client.go:1045–1063` invokes `onClose` synchronously. That callback takes `ac.mu` at `clientconn.go:1351–1353`. Under the head implementation, the close callback may wait until the newly scheduled reset worker runs its initial section. This changes the scheduling dependency but does not create a cycle: the worker's initial section does not wait for the old transport or for the parent to finish closing it. It unlocks before calling `tryAllAddrs` at line 1264. The old callback then sees its canceled attempt context and returns at lines 1356–1361 without clearing the replacement transport.

The notification path was inspected for a hidden dependency on the balancer that could invalidate this reasoning. `updateConnectivityState` calls `acBalancerWrapper.updateState`, which queues a callback through `CallbackSerializer.Schedule` at `balancer_wrapper.go:255–264`. `internal/grpcsync/callback_serializer.go:64–65` enqueues the callback; it does not wait for the balancer's state listener to execute. The reset worker therefore does not wait for the serializer callback that might itself be performing the address update. Dialing and transport creation remain outside the initial critical section.

Later attempt completion remains protected by the existing context checks. Failed attempts recheck their captured context before publishing TransientFailure at `clientconn.go:1266–1274`, and recheck it before publishing Idle after backoff at lines 1293–1297. A transport established after its attempt was canceled is closed instead of installed at lines 1394–1412. The successful transport assignment and health-check startup still occur under `ac.mu` at lines 1421–1423. These are unchanged protections, not newly added correctness mechanisms.

The contract comment at `clientconn.go:1231–1234` states both the caller's lock obligation and the callee's release obligation. The method name also advertises the release. The repository already has a private `AndUnlock` ownership convention in `ClientConn.updateResolverStateAndUnlock` at line 728 and its callers in `resolver_wrapper.go`. Both actual reset callers were audited; no old `resetTransport()` call remains. The race detector is not a general lock-ownership checker, so the source audit is the evidence that these two callers satisfy the contract.

The file-size concern was assessed separately from the correctness argument. `clientconn.go` contains channel creation, resolver/service-configuration integration, the `addrConn` lifecycle, and other existing support code. Extracting the connection lifecycle could make that existing organization easier to scan, but this diff neither introduces another responsibility nor enlarges the file. Requiring that extraction for this change would not remove any of the locking or cancellation obligations under review. No size-threshold crossing or unjustified growth is present.

The first worked code-judo candidate is the implementation already chosen: make attempt reservation part of the caller's existing critical section, then release before network work. Its essential flow is:

```go
// connect holds ac.mu and has already checked Idle.
ac.resetTransportAndUnlock()

// resetTransportAndUnlock retains the existing preparation body.
acCtx := ac.ctx
if acCtx.Err() != nil {
    ac.mu.Unlock()
    return
}
// Snapshot addresses/backoff and compute the deadline, as in the source.
ac.updateConnectivityState(connectivity.Connecting, nil)
ac.mu.Unlock()
// The existing dialing and backoff body follows.
```

This is an explanatory excerpt, not a replacement patch. It eliminates the reacquisition gap and three obsolete unlock/lock operations across the two callers and reset entry. The cancellation check and single state transition stay together. No additional branch or model is needed. This is the strongest simplification visible in the changed code.

A second candidate would separate locked preparation from unlocked execution so that `updateAddrs` unlocks in its own goroutine before launching the worker. A concrete design would capture an attempt with these fields:

```go
type transportAttempt struct {
    ctx             context.Context
    addrs           []resolver.Address
    backoffFor      time.Duration
    connectDeadline time.Time
}

// prepareTransportLocked would perform the current context check,
// capture the four fields, and publish Connecting without unlocking.
// runTransportAttempt would contain the unchanged dialing/backoff body.

// At the restart tail of updateAddrs:
attempt, ok := ac.prepareTransportLocked()
ac.mu.Unlock()
if ok {
    go ac.runTransportAttempt(attempt)
}

// At the admitted path of connect:
attempt, ok := ac.prepareTransportLocked()
ac.mu.Unlock()
if ok {
    ac.runTransportAttempt(attempt)
}
```

This sketch is not executable as written and was not tested. Preserving behavior would require moving the current preparation calculations intact, retaining the canceled-context no-transition exit, maintaining context checks in the worker, and keeping old-transport closing outside the critical section. Preparing synchronously also advances deadline calculation and state notification relative to the current asynchronous restart. The sketch makes the ownership boundary more conventional, but it adds a snapshot type, a method boundary, and caller-level success branches without deleting the attempt lifecycle's existing complexity. With only two private callers and an explicit release contract, there is no demonstrated dramatic simplification that warrants demanding this refactor. It is an assessed alternative, not an actionable finding or remediation request.

A third candidate would set Connecting in `connect` before unlocking and leave reset independently locking. That would split reservation from context validation and snapshot capture. Applying the same scheme to the address-update restart would require more caller-level state handling, especially around the empty-address transition. It would spread the reset invariant across callers rather than remove concepts. I do not recommend that restructuring.

Verification used only focused offline Go tests from the clone root. Every command had a five-minute outer limit and the required cache and module settings. Each package was tested once with its specified flag set. The exact shared invocation prefix was:

```sh
timeout 300s env \
  GOMODCACHE=/home/jack/.t3/bench-runs-lanes/codex-thermo-sol61/2026-10-10-last-push-codex-thermo-sol61/att-004/clone-cache/gomodcache \
  GOCACHE=/home/jack/.t3/bench-runs-lanes/codex-thermo-sol61/2026-10-10-last-push-codex-thermo-sol61/att-004/clone-cache/gocache \
  GOFLAGS=-mod=mod GOPROXY=off GOTOOLCHAIN=local \
  go test
```

The root-package command appended:

```sh
-race -count=1 -timeout=240s \
  -run '^Test/(UpdateAddresses_NoopIfCalledWithSameAddresses|ResetConnectBackoff|BackoffCancel|ConnectParamsWithMinConnectTimeout)$' .
```

It exited 0 with `ok google.golang.org/grpc 1.247s`. The selected tests exercise address-list stability while connecting, backoff reset, cancellation, and configured connection timeouts. Their source was inspected in `clientconn_test.go`, including the restart-related test at lines 916 onward.

The integration-package command appended:

```sh
-race -count=1 -timeout=240s \
  -run '^Test/(StateTransitions_.*|SubConnShutdown|PickFirst_ResolverError_ZeroAddresses_WithPreviousUpdate|PickFirst_AddressUpdateWithAttributes|PickFirst_AddressUpdateWithBalancerAttributes)$' ./test
```

It exited 0 with `ok google.golang.org/grpc/test 2.396s`. The selection includes the four state-transition tests in `test/clientconn_state_transition_test.go`, address-attribute updates, empty resolver results, and subconnection shutdown. `TestSubConnShutdown` also exercises shutdown requested from a state listener. These tests are supporting evidence; they do not force every possible mutex-handoff schedule.

The originating regression-test command appended:

```sh
-race -count=10 -timeout=240s \
  -run '^Test/AuthorityRevive$' ./xds/internal/xdsclient/tests
```

It exited 0 with `ok google.golang.org/grpc/xds/internal/xdsclient/tests 2.371s`. The existing test observes a connection being created, cancels and restarts an authority watch, and rejects an additional transport at `authority_test.go:278–313`. Ten successful runs are a focused check, not reproduction of the author's much larger experiment or a proof of schedule coverage. This PR adds no new tests; that fact alone does not establish an actionable maintainability regression.

`git diff --check main...review-head` exited 0. The source audit verified the initial reset method releases `ac.mu` on both exits, and that subsequent release/reacquisition paths match the base implementation. The full grpc-go suite was not run, and no proxy, upstream network source, or scratch-module experiment was used. The alternative designs above were analyzed rather than implemented or benchmarked.

Checkout integrity was checked before and after test execution using `git status --porcelain=v1`, `git rev-parse HEAD HEAD^{tree}`, and `git ls-files -z | xargs -0 sha256sum | sha256sum`. Status was empty. The head remained `76ef33f44a600c3ed1a385979fd1dfbcade3fbb6`; its tree remained `ca9417ec8ca05f79c8bd3511936a4876e4881f83`; the tracked-file content digest remained `1ed466288b45ad9e7002fd5e3996a12d761ee7da9e652ed59c6eb363a7f6b506`. Report artifacts are outside the checkout.

No actionable remediation is warranted: the diff removes a non-atomic admission boundary, leaves branching unchanged, documents its private ownership contract, and retains the existing connection lifecycle. The approval is limited to this pinned change and the evidence above. No repository edits or remedies were applied.
