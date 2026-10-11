# addrConn transport initiation: detailed review

This review covers `grpc/grpc-go#7390` at head `76ef33f44a600c3ed1a385979fd1dfbcade3fbb6`, against merge-base `daab56344e612097fd50c46c433de5d9b6013837`. The committed range is `main...review-head`. The frozen thermo-nuclear-code-quality-review skill was applied by one reviewer, with no delegated review. Repository guidance and external discussions were not loaded. The supplied packet describes intent; the conclusions below come from the pinned source and local focused tests.

There are no actionable findings in this subsystem. The change improves atomicity without adding a state, policy, branch, or abstraction. The unusual mutex handoff deserves inspection, but the actual control flow satisfies its documented contract.

## Measurements and source evidence

`git diff --stat main...review-head` and `git diff --numstat main...review-head` show one changed file, `clientconn.go`, with six additions and seven deletions. `git show main:clientconn.go | wc -l` reports 1,839 lines; `wc -l clientconn.go` reports 1,838. The file was already above 1,000 lines and shrinks by one line. This PR neither crosses the skill's size threshold nor expands the existing file's responsibilities.

`git diff main...review-head -- clientconn.go` contains three hunks. In `connect`, the premature unlock disappears and the transport method is renamed. In `updateAddrs`, the unlock before goroutine creation disappears and the same renamed method is launched. At the transport method's definition, its initial lock disappears and its caller-held, callee-released contract is documented. No conditional is added or removed; connection preparation, dialing, backoff, cancellation, and state notification remain in their existing canonical layers.

`rg -n 'resetTransport' --glob '*.go'` identifies exactly two live callers of `resetTransportAndUnlock`, at `clientconn.go:922` and `clientconn.go:996`, and its definition at line 1234. Other matches are pre-existing expected log strings in tests, not calls to a removed method. `rg -n '\.connect\(' --glob '*.go'` identifies `balancer_wrapper.go:276` as the addrConn entry point; unrelated balancergroup methods share the spelling.

The relevant source was read with line numbers in `clientconn.go:902`, `clientconn.go:938`, and `clientconn.go:1165` through the teardown section, together with `balancer_wrapper.go:253`, `internal/grpcsync/callback_serializer.go`, and `internal/transport/http2_client.go:985` and line 1045. Tests were inspected in `clientconn_test.go`, `balancer_wrapper_test.go`, `test/subconn_test.go`, `test/clientconn_state_transition_test.go`, `test/balancer_test.go`, and `xds/internal/xdsclient/tests/authority_test.go:278`.

## Atomicity and locking analysis

At `clientconn.go:906`, `connect` acquires `ac.mu`. Its Shutdown and non-Idle cases each release it before returning. Its remaining path calls `resetTransportAndUnlock` while still holding the mutex. That method snapshots `ac.ctx` and its addresses, calculates the timeout/backoff, and publishes Connecting at line 1261 before its first normal unlock at line 1262. A second `connect` cannot observe the same Idle state during this interval. This deletes the check-then-act gap rather than adding a second admission flag or duplicating the transition in another layer.

At `clientconn.go:948`, `updateAddrs` acquires the same mutex. Equal addresses, inactive states, and a Ready connection whose address remains valid all release it locally and return. The restart path cancels the previous attempt's context and creates a replacement while holding the mutex. It clears the current transport, optionally publishes Idle for an empty address list, and launches `resetTransportAndUnlock` with the mutex still held. Starting a goroutine establishes visibility of the preceding updates; Go's mutex permits another goroutine to unlock it. This path transfers the lock intentionally, rather than attempting a second acquisition.

That transfer prevents another address update or `connect` from interposing between replacement-context setup and the new worker's initial snapshot/state transition. Once the worker releases the lock, subsequent address updates may cancel its captured context normally. The existing `createTransport` context check at `clientconn.go:1396` prevents a superseded successful transport from being installed. The change does not require serializing the network attempt under the mutex.

At `clientconn.go:1236`, an already canceled context causes an immediate unlock and return. Otherwise the lock is released at line 1262, before `tryAllAddrs`. Later error, backoff, and success paths retain their own balanced acquisitions and releases: the canceled error path releases at line 1269; failure state/backoff setup releases at line 1278; timer bookkeeping releases at line 1285; the final Idle transition releases at line 1297; success bookkeeping releases at line 1303. Returns from the backoff select occur while the lock is already released. There is no normal return path that leaks the handed-off mutex and no caller that unlocks it a second time.

The deferred `GracefulClose` at `clientconn.go:986` is the main callback interaction. `http2Client.GracefulClose` invokes `onClose` inline, and that callback acquires `ac.mu` at `clientconn.go:1352`. With this PR, the deferred call may wait for the new worker's initial unlock. The new worker does not wait for that graceful close before unlocking: its initial section reads connection configuration and schedules a state update, then releases the mutex before dialing. `acBalancerWrapper.updateState` uses the callback serializer's queue; it does not invoke the balancer's listener synchronously in this section. These paths do not introduce a circular wait.

No runtime ownership check is necessary to make the current two call sites correct. A TryLock probe would not prove which caller owns a mutex or improve this protocol. Adding such a branch would introduce machinery without repairing an observed defect. Similarly, adding a deferred unlock around the whole transport function would be incorrect because that function deliberately unlocks before network work and reacquires later.

## Worked code-judo alternative: separate preparation from execution

The strongest alternative considered was to separate the initial locked snapshot/state transition from the unlocked attempt and backoff work. This would remove cross-goroutine mutex transfer. A concrete design would introduce this private attempt value:

```go
type transportAttempt struct {
    ctx             context.Context
    addrs           []resolver.Address
    backoffFor      time.Duration
    connectDeadline time.Time
}

// Caller holds ac.mu; this method leaves it held.
func (ac *addrConn) prepareTransportLocked() *transportAttempt {
    ctx := ac.ctx
    if ctx.Err() != nil {
        return nil
    }
    backoffFor := ac.dopts.bs.Backoff(ac.backoffIdx)
    dialDuration := minConnectTimeout
    if ac.dopts.minConnectTimeout != nil {
        dialDuration = ac.dopts.minConnectTimeout()
    }
    if dialDuration < backoffFor {
        dialDuration = backoffFor
    }
    attempt := &transportAttempt{
        ctx: ctx,
        addrs: ac.addrs,
        backoffFor: backoffFor,
        connectDeadline: time.Now().Add(dialDuration),
    }
    ac.updateConnectivityState(connectivity.Connecting, nil)
    return attempt
}
```

An unlocked `runTransportAttempt(attempt)` would contain the current code from `tryAllAddrs` through final backoff reset, substituting the captured fields for its existing local variables. Its cancellation checks must continue using the captured context, not a later value of `ac.ctx`. Its backoff timer must remain tied to the captured backoff duration. The address slice is already replaced rather than edited in place by `updateAddrs`, so the existing snapshot behavior can be preserved.

After their existing validation, the synchronous and asynchronous callers would become, respectively:

```go
attempt := ac.prepareTransportLocked()
ac.mu.Unlock()
if attempt != nil {
    ac.runTransportAttempt(attempt)
}
```

```go
attempt := ac.prepareTransportLocked()
ac.mu.Unlock()
if attempt != nil {
    go ac.runTransportAttempt(attempt)
}
```

This is a worked proposal, not an applied remedy or a verified replacement. It preserves the required atomic state transition in the caller's critical section and avoids transferring the lock to the worker. It also adds an attempt type, a helper boundary, and a cancellation result check at both call sites. It moves configuration callbacks and deadline calculation into the address-update caller, changing their scheduling relative to the deferred graceful close. Those timing effects would need validation before claiming full behavioral equivalence.

For the reviewed diff, this alternative does not clearly delete enough complexity to justify requiring it. The existing code already has one canonical transition/preparation implementation, both callers are adjacent in the same file, the method name describes the ownership effect, and the new comment states its precondition. The alternative would be reasonable if connection initiation gained more callers or distinct preparation modes. It is not an actionable missed simplification in this PR.

Moving the cohesive addrConn implementation into a dedicated file was also considered. That would improve navigation in this already large file, but would not remove states, branches, or ownership obligations. The PR reduces the file's size and does not expand its architectural role. Requiring a broad relocation for this locking fix would not address a structural regression introduced by the change.

## Verification

All Go commands ran from the clone root with the required offline environment. The exact cache root was `/home/jack/.t3/bench-runs-lanes/codex-thermo-sol61/2026-10-10-last-push-codex-thermo-sol61/att-025/clone-cache`; `GOMODCACHE` was its `gomodcache` subdirectory and `GOCACHE` its `gocache` subdirectory. Each command also set `GOFLAGS=-mod=mod GOPROXY=off GOTOOLCHAIN=local`. Each package was tested once for its flag set. No scratch module, source edit, or upstream access was used.

```sh
timeout 290s go test -race . \
  -run '^Test/(Balancer_StateListenerBeforeConnect|UpdateAddresses_NoopIfCalledWithSameAddresses|ResolverEmptyUpdateNotPanic|ResetConnectBackoff|BackoffCancel|ConnectParamsWithMinConnectTimeout)$' \
  -count=10 -timeout=270s
```

This completed with exit code 0: `ok google.golang.org/grpc 12.787s`. These selected tests exercise close/NewSubConn ordering, address-list handling, backoff cancellation/reset, and timeout configuration. They supplement source analysis; they do not deterministically force the original double-Connect schedule.

```sh
timeout 290s go test -race ./xds/internal/xdsclient/tests \
  -run '^Test/AuthorityRevive$' -count=20 -timeout=270s
```

This completed with exit code 0: `ok google.golang.org/grpc/xds/internal/xdsclient/tests 3.650s`. The original issue's test ran twenty times with the race detector and did not create an unexpected additional management-server transport. This local run does not independently reproduce the author's much larger stress count.

```sh
timeout 290s go test -race ./test \
  -run '^Test/(SubConnEmpty|SubConnShutdown|StateTransitions_ReadyToConnecting)$' \
  -count=10 -timeout=270s
```

This completed with exit code 0: `ok google.golang.org/grpc/test 1.420s`. These selected integration tests exercise removing/re-adding a SubConn's addresses, shutdown through both public paths, and reconnect state ordering. They specifically extend verification to the asynchronous address-update path and its interaction with closing transports.

`git diff --check main...review-head` completed with exit code 0 and no output. `git status --porcelain=v1 --untracked-files=all` was empty before and after test execution. HEAD remained the pinned head SHA, whose tree is `ca9417ec8ca05f79c8bd3511936a4876e4881f83`; the base tree is `41a1bf36da7d241b404899fd5c73a1ccabc6e930`. The full suite was not run, in accordance with the execution allowance. Passing race tests cannot prove absence of all logical races; the admission fix is supported by the explicit critical-section analysis above.

## Remediation assessment

No change is requested before approval. If a later refactor adopts the worked preparation/execution split, first preserve the captured-context and backoff semantics, then move only the unlocked execution phase, and verify both synchronous initiation and asynchronous address replacement. A meaningful added regression test for that work would block a counting dialer after the first attempt starts, issue concurrent SubConn.Connect calls, and assert that only one attempt is admitted; address replacement and shutdown should be exercised while that dial is blocked. This is a future verification suggestion, not a finding or an unfulfilled requirement of the present review.
