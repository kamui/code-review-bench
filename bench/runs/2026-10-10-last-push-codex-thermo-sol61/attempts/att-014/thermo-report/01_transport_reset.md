# Address-connection admission and transport reset

This subsystem report covers every changed hunk in `clientconn.go` and the
unchanged code needed to assess its synchronization contract. There are no
actionable findings and no unresolved questions. The verdict is based on a
single primary review context using the frozen review skill. No child review
was required by that skill, and no review was delegated.

## Scope and measurements

The pinned base is `daab56344e612097fd50c46c433de5d9b6013837`; the pinned head is
`76ef33f44a600c3ed1a385979fd1dfbcade3fbb6`. `git diff main...review-head` contains
three hunks: a synchronous call at `clientconn.go:922`, an asynchronous call at
line 996, and the renamed helper plus its contract at lines 1231–1234.

`git diff --stat main...review-head` reports one changed file, six insertions,
and seven deletions. `git diff --numstat main...review-head` independently reports
`6 7 clientconn.go`. `git show main:clientconn.go | wc -l` reports 1,839 lines;
`wc -l clientconn.go` reports 1,838. The 1,000-line threshold was crossed before
this change. This PR decreases total file length and adds no conditional.

At the head, `connect` spans lines 905–924, `updateAddrs` spans lines 940–997,
and `resetTransportAndUnlock` spans lines 1234–1304. These are respectively 20,
58, and 71 lines including signature and closing brace. The base helper spans
lines 1234–1305, or 72 lines. Its body loses one acquisition; its branches,
network work, and retry policy are retained.

The supporting sources inspected were `balancer_wrapper.go:253–280`,
`clientconn.go:1199–1215`, `clientconn.go:1306–1425`,
`clientconn.go:1500–1595`, `internal/transport/http2_client.go:982–1064`,
`internal/grpcsync/callback_serializer.go:56–93`, and
`internal/buffer/unbounded.go:54–69`. Focused tests and their fixtures were also
read to confirm what the selected executions exercise.

Source was read with numbered `nl -ba ... | sed -n ...` slices. The whole-code
search `rg -n 'resetTransport' --glob '*.go' .` found exactly two executable
calls to the renamed helper. Other matches outside the implementation were
existing string literals in test logging allowances, not unresolved calls.
`rg -n 'func .*Locked|AndUnlock' clientconn.go` showed the established
`updateResolverStateAndUnlock` ownership convention at line 728 alongside
ordinary Locked helpers. No helper already implemented the required atomic
connection preparation.

## Atomicity proof for connect

The base performs its Idle check while holding `ac.mu`, releases the mutex, and
then calls `resetTransport`, which acquires it again. Consider caller A passing
the Idle check and releasing the mutex. Caller B can acquire it, observe Idle,
and also commit to calling the helper. Both workers can then capture the same
live context and address list. The helper sets Connecting without repeating the
admission check, so B's earlier successful admission remains effective.

The head instead has A call the helper with `ac.mu` held. The helper captures
`ac.ctx`, checks cancellation, captures `ac.addrs`, prepares backoff/deadline,
and publishes Connecting before its first unlock at line 1262. B can only
acquire the mutex after that transition and fails its Idle check. This removes
the overlapping admission window without a second flag or duplicate check.

If the context is already canceled, line 1237 unlocks and returns before state
publication. The change therefore preserves the old canceled-context path.
`tearDown` holds the same mutex while publishing Shutdown and canceling at
lines 1546–1556, so it cannot insert Shutdown between this caller's admission
and the live-context preparation. Parent cancellation can still happen
independently, as it could at the base; the downstream context checks remain.

The guarantee is one admitted attempt for the live context at this boundary.
It is not a claim that replacement generations can never overlap while canceled
network work is completing, or that a connection cannot legitimately go Idle
and admit a subsequent attempt. Those existing lifecycle cases are handled by
context cancellation and transport installation checks.

## Address replacement and the goroutine handoff

Every return before `updateAddrs` launches a reset still unlocks locally:
equal addresses at lines 949–951, inactive states at lines 956–961, and retaining
the connected address at lines 964–973. The remaining path owns `ac.mu` from
line 948 through context cancellation/replacement at lines 980–981, transport
detachment at lines 985–987, and the launch at line 996.

The launched function does not call Lock on entry. The parent goroutine's
already held mutex protects the child prologue until the child unlocks it.
Go's `sync.Mutex` does not bind unlocking to the goroutine that locked it;
starting a goroutine also orders the preceding initialization before its work.
The parent performs no further access to mutable addrConn fields on this path.

This matters beyond the originating connect race. At the base, address update A
can replace the context, unlock, and schedule worker A. Before A captures its
context, address update B can replace it again and schedule worker B. Both
workers can capture B's context and addresses. The head holds the mutex until
A's worker has captured A's context and published Connecting. B's later
replacement cancels that captured context, preserving the intended generation
boundary. `createTransport` checks the captured context at lines 1394–1412
before installing a transport and closes a canceled attempt's transport.

The contract is private, named `AndUnlock`, and documented at lines 1231–1233.
An ownership check or TryLock assertion would not establish that the correct
operation holds the mutex and would add another mechanism to this small fix.
The complete call-site audit, rather than a speculative assertion, supplies
the evidence that both callers follow the contract.

## Deferred close and callback ordering

`updateAddrs` evaluates the receiver of `defer ac.transport.GracefulClose()`
before clearing `ac.transport`. Thus the deferred call still targets the old
transport even if the child later installs a replacement. `GracefulClose`
takes the old transport's mutex and calls `onClose` synchronously at
`internal/transport/http2_client.go:1055`. That callback needs `ac.mu`.

With this PR, that deferred call can begin while the child still owns `ac.mu`.
It waits until the child finishes its prologue. This is a real scheduling
dependency, but there is no corresponding wait for the old transport in the
prologue at `clientconn.go:1235–1262`. The duration hooks and backoff calculation
were already called under this mutex at the base. No new user callback is
introduced inside the critical section.

State publication invokes `acBalancerWrapper.updateState`, whose implementation
only schedules work. `CallbackSerializer.Schedule` calls the unbounded buffer's
Put; Put acquires its own mutex, sends non-blockingly when possible, or appends
to its backlog. It does not wait for the balancer callback to run. Therefore a
balancer serializer currently executing an address update does not prevent the
child from reaching its unlock.

After unlocking, the child calls `tryAllAddrs`. Even if the old close callback
has not run yet, its captured context was canceled by the update. Its check at
`clientconn.go:1356–1361` returns without touching the replacement transport or
publishing Idle. The cancellation and checks existed at the base and remain
the canonical stale-transport protection. The examined paths provide no
deadlock or replacement-transport corruption finding.

## Complete return and ownership audit

| Path | Evidence at head | Ownership disposition |
| --- | --- | --- |
| Context canceled before preparation | 1235–1238 | Releases the caller's mutex and returns. |
| Live attempt begins | 1241–1264 | Captures protected inputs and publishes Connecting, then unlocks before dialing. |
| Dial failure after cancellation | 1266–1270 | Acquires, checks context, releases, returns. |
| Dial failure with live context | 1274–1278 | Publishes failure and captures the backoff channel, then releases before waiting. |
| Backoff timer | 1283–1285 | Acquires and releases around the index increment. |
| Cancellation during backoff | 1288–1290 | Returns while the mutex is already released. |
| End of backoff | 1293–1298 | Acquires, conditionally publishes Idle, releases, returns. |
| Successful dial | 1301–1303 | Acquires and releases around the backoff reset. |

There is no double unlock or missing normal-path unlock. A single entry defer
would hold the mutex across network work or double-unlock the existing release
points. A panic in an existing hook is outside this normal-return guarantee;
this patch does not introduce such a hook or a new panic path.

## Worked code-judo alternatives

The accepted change itself deletes a synchronization gap and a redundant
acquisition. It extends the existing canonical reset path rather than scattering
new state transitions across callers. Three more ambitious alternatives were
evaluated instead of treating minimal line count as sufficient evidence.

One alternative moves the Connecting transition into `connect`, immediately
before its former unlock, and leaves the reset helper acquiring its own mutex.
This is locally small but repeats the state publication responsibility. More
substantively, it leaves the address-replacement snapshot race described above
unaddressed. Repairing both callers then needs a second preparation mechanism.
This is not a simplification of the common invariant.

A stronger alternative separates preparation from execution. Its concrete
shape could be the following, using the existing deadline calculation and
the existing post-unlock reset body:

```go
type transportAttempt struct {
    ctx        context.Context
    addrs      []resolver.Address
    backoffFor time.Duration
    deadline   time.Time
}

// prepareTransportLocked reads and updates ac without releasing ac.mu.
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
        ctx: ac.ctx, addrs: ac.addrs,
        backoffFor: backoffFor, deadline: time.Now().Add(dialDuration),
    }
    ac.updateConnectivityState(connectivity.Connecting, nil)
    return attempt, true
}
```

The successful tail of `connect` would become:

```go
attempt, ok := ac.prepareTransportLocked()
ac.mu.Unlock()
if ok {
    ac.runTransportAttempt(attempt)
}
return nil
```

The restart tail of `updateAddrs` would use the same preparation and local
unlock, followed by `go ac.runTransportAttempt(attempt)` when preparation
succeeds. `runTransportAttempt` would contain the existing lines 1264–1303,
using the snapshot fields for `acCtx`, `addrs`, `backoffFor`, and
`connectDeadline`. Later reacquisitions must remain in that worker; capturing
inputs does not make shared connectivity, transport, or backoff fields immutable.

This proposal removes transfer of the initial mutex between goroutines and
creates two phases with ordinary ownership boundaries. It does not delete the
state branches, retry select, stale-transport checks, or transport lifecycle.
It also adds a type, a function, and a success-result branch at both callers.
The slice snapshot remains safe only because address updates replace the slice
rather than mutate its elements. Copying only addresses and looking up `ac.ctx`
again in the worker would recreate the original generation race.

The sketch is evaluated source, not an applied or compiled remedy. Moving
preparation onto the updating goroutine advances when the state notification
and absolute deadline are created relative to scheduling. That ordering needs
an explicit behavior audit before adopting it as an equivalent refactor. For
this patch, retaining the existing prologue and post-unlock body is smaller,
more direct, and supported by a complete ownership proof. This alternative is
not a missed dramatic simplification or a required remediation.

The third alternative moves `addrConn` and its methods to a dedicated source
file. It could group the pre-existing connection implementation and eliminate
the old TODO near `connect`. It does not simplify the locking boundary or change
the number of branches and functions. The current PR reduces file size, and
requiring a broad relocation for this three-hunk repair would conflate existing
organization debt with a regression. No decomposition finding is raised.

## Executed verification

All commands ran from the clone root with this exact environment, using the
attempt's supplied cache:

```sh
GOMODCACHE=/home/jack/.t3/bench-runs-lanes/codex-thermo-sol61/2026-10-10-last-push-codex-thermo-sol61/att-014/clone-cache/gomodcache
GOCACHE=/home/jack/.t3/bench-runs-lanes/codex-thermo-sol61/2026-10-10-last-push-codex-thermo-sol61/att-014/clone-cache/gocache
GOFLAGS=-mod=mod
GOPROXY=off
GOTOOLCHAIN=local
```

These values were prefixed to each invocation. The focused commands were:

```sh
timeout 300s go test -race -count=10 -timeout=240s -run '^Test/(ResetConnectBackoff|BackoffCancel|UpdateAddresses_NoopIfCalledWithSameAddresses|CloseConnectionWhenServerPrefaceNotReceived|Balancer_StateListenerBeforeConnect)$' .
timeout 300s go test -race -count=5 -timeout=240s -run '^Test/(SubConnEmpty|SubConnShutdown|PickFirst_AddressesRemoved|StateTransitions_ReadyToConnecting|ConcurrentClientConnCloseAndServerGoAway)$' ./test
timeout 300s go test -race -count=100 -timeout=240s -run '^Test/AuthorityRevive$' ./xds/internal/xdsclient/tests
```

They exited zero and returned, respectively:

```text
ok      google.golang.org/grpc                            32.496s
ok      google.golang.org/grpc/test                       3.082s
ok      google.golang.org/grpc/xds/internal/xdsclient/tests 13.783s
```

`internal/grpctest/grpctest.go:97–103` confirms that suite methods named TestX
run as subtests named X, matching the command filters. The source contains all
five selected root tests and all five selected integration tests. The root
unchanged-address test checks its no-op branch; the integration address-removal
and empty-address tests exercise actual replacement paths. The xDS test asserts
that no additional transport appears after reviving an authority.

The race detector checks executed memory accesses; the original bug is also a
logical admission race and need not trigger it. One hundred passing stochastic
repetitions cannot establish the packet's reported 100,000-attempt result or
prove all possible interleavings. Static analysis of the uninterrupted critical
section establishes the intended admission property. The full suite was not
executed because only focused packages were allowed. No external review,
network lookup, reference answer, or historical verdict was used.

`git diff --check main...review-head` exited zero. Initial and final
`git status --porcelain=v1 --untracked-files=all` were empty, and final
`git diff --exit-code HEAD -- .` exited zero. The head commit remained
`76ef33f44a600c3ed1a385979fd1dfbcade3fbb6`, with tree
`ca9417ec8ca05f79c8bd3511936a4876e4881f83`; the base tree was
`41a1bf36da7d241b404899fd5c73a1ccabc6e930`. Reports were written outside the clone.

## Disposition

No mandatory changes are proposed. The boundary becomes more atomic without
state-machine growth, the private ownership contract is explicit and satisfied,
and actual I/O remains outside the initial critical section. The structural
alternatives above are worked evaluations, not additional actionable findings.
No remedy was applied, and no tests or source files were added to the checkout.
