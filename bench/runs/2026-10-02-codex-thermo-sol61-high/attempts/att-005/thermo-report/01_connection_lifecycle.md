# Connection lifecycle and mutex ownership

## Scope and verdict

There are no actionable findings in this subsystem. The change removes a gap between validating connection state and reserving a connection attempt. Its central invariant is that the Idle check, the attempt's context/address snapshot, and the transition to Connecting happen under one uninterrupted hold of `ac.mu`.

The target is `grpc/grpc-go#7390`, head `76ef33f44a600c3ed1a385979fd1dfbcade3fbb6`, base `daab56344e612097fd50c46c433de5d9b6013837`. The committed diff was inspected with `git diff main...review-head`, not against working-tree changes. Source inspection covered the complete changed functions and their callers, state notification, context cancellation, transport installation, transport-close callbacks, backoff, and teardown. Previous review approvals and discussions supplied in the packet were not treated as independent correctness evidence.

One primary reviewer conducted this review. No child reviewers, cross-model checks, external discussions, ambient guidance, or other skills were used. The frozen skill has no required reference-resource or child-review step for this task.

## Measurements and commands

`git diff main...review-head --stat` reports one changed file and six insertions against seven deletions. `git diff main...review-head --unified=0` isolates the two call-site edits and the helper's rename/documentation/entry-lock change. `git diff --check main...review-head` exits successfully without output.

The line counts come from `wc -l clientconn.go` and `git show main:clientconn.go | wc -l`. A read-only Python inspection of `git show main:clientconn.go` and `git show review-head:clientconn.go` located each function declaration and its unindented closing brace, then counted function lines and explicit mutex calls. The conditional counts below count lines starting with `if ` after stripping indentation, including logging guards; they are a source measurement, not a cyclomatic-complexity claim.

| Item | Base | Head |
| --- | --- | --- |
| File lines | 1,839 | 1,838 |
| `connect` lines | 21 | 20 |
| `updateAddrs` lines | 60 | 58 |
| Reset helper lines, excluding doc comment | 72 | 71 |
| `connect` Lock / Unlock calls | 1 / 3 | 1 / 2 |
| `updateAddrs` Lock / Unlock calls | 1 / 4 | 1 / 3 |
| Reset helper Lock / Unlock calls | 5 / 7 | 4 / 7 |
| `connect` conditional count | 4 | 4 |
| `updateAddrs` conditional count | 7 | 7 |
| Reset helper conditional count | 6 | 6 |

Unequal textual Lock/Unlock counts are expected because mutually exclusive returns release a lock at different locations and the new helper receives its initial hold from a caller. The lock-flow audit below checks paths, rather than interpreting these counts as a balance test.

The changed-file manifest was verified against the local diff. Whole-repository searches for `.resetTransportAndUnlock(`, `.updateAddrs(`, and `.connect(` identify exactly two reset callers in `clientconn.go`, the address-update wrapper at `balancer_wrapper.go:271–273`, and the asynchronous connect wrapper at `balancer_wrapper.go:275–277`. No stale call to the old reset method remains.

## Atomic connection reservation

`clientconn.go:905–924` acquires the mutex, handles Shutdown, and ignores other non-Idle states. Each early return explicitly releases the mutex. The remaining path calls `resetTransportAndUnlock` while still holding it.

The base can interleave two calls as follows: caller A observes Idle and unlocks; caller B observes Idle and unlocks; A and B each enter the self-locking reset function. That function does not recheck Idle, so both can take a snapshot and dial using the same uncanceled attempt context. The transport installation in `createTransport`, at head lines 1394–1424, checks context cancellation and handshake closure but does not make duplicate same-context attempts mutually exclusive. Serializing only installation would therefore be an inadequate repair.

The head holds the mutex continuously until line 1261 records Connecting. A waiting connect caller then sees a non-Idle state and returns. The canceled-context exit at lines 1235–1238 unlocks without reserving an attempt. The PR does not move the state change before that check, so it preserves the original canceled-context behavior.

This is the effective code-judo move already implemented by the PR: delete the unlock/relock boundary instead of introducing another Connecting update, another guard in transport installation, or a new boolean that duplicates connectivity state.

```go
// connect: after the existing Shutdown and non-Idle checks,
// ac.mu is still held.
ac.resetTransportAndUnlock()

// The reset helper snapshots its context and addresses while held,
// computes the existing deadline, then reserves the attempt:
ac.updateConnectivityState(connectivity.Connecting, nil)
ac.mu.Unlock()
// Existing transport creation and backoff continue outside this hold.
```

The snapshot is coherent with state validation because `updateAddrs` and `tearDown` also use this mutex to replace/cancel the attempt context. There can still be transient overlap with an older canceled address-replacement attempt. That is supported by the existing context checks and cleanup; the fix does not promise that no two TCP handshakes can ever overlap across different attempt generations.

## Address replacement and the goroutine handoff

`clientconn.go:940–997` copies resolver addresses before locking. Equal addresses return after unlocking. Shutdown, TransientFailure, and Idle update the address list and return after unlocking. A Ready connection whose current address remains valid also returns after unlocking. These paths and their behavior are unchanged.

The restart path cancels the old attempt context and replaces it at lines 980–981, captures any old transport for deferred graceful close at lines 985–987, and optionally reports Idle for an empty list at lines 990–992. The final `go ac.resetTransportAndUnlock()` now transfers responsibility for releasing the existing mutex hold to the newly launched goroutine.

This transfer is valid for Go's mutex model; a mutex is not bound to the goroutine that acquired it. The source evidence is stronger than the language permission alone: the reset helper's prefix only reads its protected inputs, computes timing, records Connecting, and unlocks. Its asynchronous launch prevents another connect/update/teardown from intervening between the restart decision and the new reservation.

The deferred old transport close calls `http2Client.GracefulClose`, at `internal/transport/http2_client.go:1045–1063`. That method synchronously invokes `onClose` at line 1055. The callback in `clientconn.go:1351–1377` acquires `ac.mu`, so it may wait for the new reset worker to release the transferred hold. This introduces a scheduling dependency but no identified cycle: the reset prefix neither waits for that close nor acquires the old transport's mutex.

The state notification at `clientconn.go:1214` calls `acBalancerWrapper.updateState`, whose implementation at `balancer_wrapper.go:255–265` schedules a callback rather than invoking the balancer inline. `internal/grpcsync/callback_serializer.go:64–65` enqueues through `buffer.Unbounded.Put`; `internal/buffer/unbounded.go:55–69` does not wait for callback execution. Thus a balancer serializer that is currently executing the address update cannot prevent the reset prefix from reaching its unlock.

Once the old onClose callback acquires the mutex, it sees its canceled context at lines 1356–1361 and avoids clearing the replacement transport or reverting its state. If an older dial finishes after cancellation, `createTransport` checks its context at lines 1396–1412 and closes the newly created stale transport in another goroutine. Neither behavior is newly introduced by this diff.

## Release-path audit

The normal entry-context-canceled path releases the caller's hold at line 1237 and returns. The active-context path releases it at line 1262 before `tryAllAddrs`. This includes empty address lists; the empty-list behavior inside `tryAllAddrs` predates the PR and was not changed as part of this audit.

On a failed address iteration, the helper reacquires at line 1266. A canceled attempt releases at line 1269 and returns. An active attempt records TransientFailure, snapshots the reset-backoff channel, and releases at line 1278 before waiting. Timer expiry increments the backoff index inside a separate Lock/Unlock pair. Reset and cancellation cases do not retain a mutex hold. The final Idle transition reacquires at line 1293 and releases at line 1297. The success path resets the backoff index inside the pair at lines 1301–1303.

`tearDown`, at lines 1545–1595, first takes the same mutex and records Shutdown before canceling the attempt context. Keeping the caller's lock through reservation means teardown cannot interpose between the checked state and a subsequent Connecting update. `connect` also retains its explicit Shutdown rejection.

The helper's name and lines 1231–1233 document the incoming lock and guaranteed normal-flow release. No call site adds a defer-unlock around the call, and the asynchronous caller does not access mutex-protected fields after launching it. A custom lock-owner boolean would itself require synchronization and would duplicate the existing mutex contract. No runtime ownership checker is warranted by the two audited callers.

## Worked structural alternatives

The most credible alternative separates locked preparation from unlocked execution. A schematic typed boundary is:

```go
type transportAttempt struct {
    ctx        context.Context
    addrs      []resolver.Address
    backoffFor time.Duration
    deadline   time.Time
}

// prepareTransportLocked snapshots inputs and records Connecting.
// Its caller retains responsibility for unlocking.
func (ac *addrConn) prepareTransportLocked() (transportAttempt, bool)

// runTransportAttempt contains the existing tryAllAddrs, failure,
// backoff, and success paths, starting with ac.mu unlocked.
func (ac *addrConn) runTransportAttempt(a transportAttempt)

// In connect, after the existing state checks:
a, ok := ac.prepareTransportLocked()
ac.mu.Unlock()
if ok {
    ac.runTransportAttempt(a)
}

// In updateAddrs, after cancellation/replacement and existing state work:
a, ok := ac.prepareTransportLocked()
ac.mu.Unlock()
if ok {
    go ac.runTransportAttempt(a)
}
```

This is an unimplemented design sketch, not compilable replacement code or a requested remedy. Preparation would copy the current context check, backoff/deadline calculation, and Connecting update exactly. The execution helper would retain the original context checks, backoff-channel snapshot timing, and stale-transport cleanup. The addresses must remain the copied slice associated with the prepared attempt; execution must not reload them from a later update.

This design removes the cross-goroutine mutex handoff and would let the old deferred close run after an unlock performed by its own caller. Its cost is an additional data type, a success/cancellation result, two helpers, and repeated dispatch at two callers. It leaves all existing transport/backoff branches in place. It also records Connecting synchronously in the address-update caller rather than in its worker. The coherence benefit is real, but the current two-site contract is short, explicit, and verified. The sketch is not a dramatic reduction in concepts or a presumptive blocker for this PR.

A second alternative keeps the current helper's locked prefix but starts the remaining work in an internal goroutine immediately after its unlock. `updateAddrs` could then call it directly instead of handing a held mutex to a worker. However, `connect` would also return before the attempt's current synchronous execution finishes. Although the public SubConn wrapper already invokes `connect` asynchronously, preserving the private method's completion behavior would require a separate execution entry point or a mode argument. A mode would obscure a simple contract, and another entry point converges on the first alternative. This option was not selected as a required change.

A third alternative sets Connecting in `connect` before releasing the mutex and leaves reset self-locking. That avoids the duplicate Idle check but splits ownership of the transition, requires careful treatment of the canceled-context early return, and does not repair the corresponding address-restart boundary by itself. The reviewed implementation is more canonical and deletes the acquisition gap at both callers with fewer changes.

A broad `addrConn` file extraction would move the connection lifecycle out of the already-large `clientconn.go`, but it would not change these ownership contracts or remove any branch. The file has no new responsibility and shrinks. Its preexisting size is not a PR-introduced threshold violation.

## Executed verification

Each package was tested once with its listed flag set. The commands ran from the clone root, offline, with a five-minute outer limit and a four-minute Go test timeout. No scratch test module, generated test, source change, or dependency-file edit was used.

All commands used this environment:

```sh
GOMODCACHE=/home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high/att-005/clone-cache/gomodcache
GOCACHE=/home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high/att-005/clone-cache/gocache
GOFLAGS=-mod=mod
GOPROXY=off
GOTOOLCHAIN=local
```

The root package command was `timeout 300s env` with that environment followed by:

```sh
go test -race -count=5 -timeout=240s -run '^Test/(Balancer_StateListenerBeforeConnect|UpdateAddresses_NoopIfCalledWithSameAddresses|ResetConnectBackoff|BackoffCancel|ResolverEmptyUpdateNotPanic|DialContextCancel)$' .
```

It exited 0: `ok google.golang.org/grpc 7.046s`. The selection covers cancellation, backoff reset, an unchanged address list during Connecting, empty resolver input, and the state-listener-before-connect case. The unchanged-address test takes an early return, so it alone would not validate the new restart handoff.

The connection-lifecycle package command was `timeout 300s env` with the same environment followed by:

```sh
go test -race -count=5 -timeout=240s -run '^Test/(SubConnEmpty|SubConnShutdown|ServersSwap|StateTransitions_ReadyToConnecting|PickFirst_NewAddressWhileBlocking|PickFirst_AddressesRemoved)$' ./test
```

It exited 0: `ok google.golang.org/grpc/test 4.240s`. `SubConnEmpty` exercises removal and re-addition on the same SubConn; `ServersSwap` and `PickFirst_AddressesRemoved` exercise active address replacement; the other selections cover shutdown, reconnection state order, and blocked-RPC recovery. This selection was added specifically to exercise the changed restart path rather than relying on the root package's no-op address-update test.

The originating flake command was `timeout 300s env` with the same environment followed by:

```sh
go test -race -count=10 -timeout=240s -run '^Test/AuthorityRevive$' ./xds/internal/xdsclient/tests
```

It exited 0: `ok google.golang.org/grpc/xds/internal/xdsclient/tests 2.389s`. The test at `authority_test.go:278–314` checks that reviving an authority does not create another management-server connection and that the existing connection survives the authority-idle interval.

The repository's `grpctest.RunSubTests` entry points expose these method names under `Test/…`, matching the selections. All three commands passed without race reports. Ten runs of the flake are a focused sanity check, not statistical evidence matching the packet's much larger experiment. Tests do not establish the mutex contract by themselves; the explicit path and callback audit supplies that evidence. No full-suite or base-version run is claimed.

`git status --porcelain=v1 --untracked-files=all` and `git diff --no-ext-diff --exit-code` were clean before completion. Head tree identity remained `ca9417ec8ca05f79c8bd3511936a4876e4881f83`; base tree identity is `41a1bf36da7d241b404899fd5c73a1ccabc6e930`. Reports were written only under the requested work directory.

## Remediation judgment

No change request follows from the evidence. Keep the single state-reservation path and its documented lock-consuming boundary. The structural alternatives above record the design audit and its tradeoffs; none is an actionable finding or an unverified defect allegation.
