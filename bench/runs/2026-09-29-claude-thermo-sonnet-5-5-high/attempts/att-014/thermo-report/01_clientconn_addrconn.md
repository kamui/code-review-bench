# Detail: `clientconn.go` — addrConn connect / resetTransport locking

Range reviewed: `daab56344e612097fd50c46c433de5d9b6013837..76ef33f44a600c3ed1a385979fd1dfbcade3fbb6` (`git diff main...review-head`), one file, +6/−7.

## Measurements and commands

- `git diff main...review-head` shows three edits: `connect()` drops its `ac.mu.Unlock()` before the call, `updateAddrs()` drops its `ac.mu.Unlock()` and turns `go ac.resetTransport()` into `go ac.resetTransportAndUnlock()`, and `resetTransport` loses its `ac.mu.Lock()` and is renamed with a doc comment.
- `clientconn.go` is 1839 lines on `main` and 1838 lines on the head. It was already over 1000 lines, so the 1k-line rule is not triggered. The diff makes the file one line shorter.
- `go vet .` (offline, focused allowance) is clean. The clone tree is unchanged.
- I read the transport code for `GracefulClose` (`internal/transport/http2_client.go:1045-1060`), `onClose` (`clientconn.go:1351-1377`) and `acBalancerWrapper.updateState` (`balancer_wrapper.go:255-265`). These bear on findings 1 and 2.

## Verification of the fix itself

The root cause in the issue is a real check-then-act gap. `connect()` checks `state == Idle`, unlocks, and `resetTransport` re-locks and only then moves to `Connecting`. Two concurrent `connect()` calls can both pass the Idle check. Keeping `ac.mu` held from the Idle check through `updateConnectivityState(Connecting)` closes the gap. `updateAddrs` needed the same treatment, and it got it. I found no correctness defect in the change, and I did not construct a deadlock:

- The deferred `ac.transport.GracefulClose()` in `updateAddrs` runs `onClose` synchronously, and `onClose` takes `ac.mu`. That call now blocks until the spawned goroutine reaches one of its unlocks. The goroutine's path from lock to unlock does not depend on `updateAddrs` returning. `updateConnectivityState` reaches `acbw.updateState`, which only calls `serializer.Schedule` and does not block. So the wait is bounded.
- `onClose` on the old transport sees `ctx.Err() != nil` (the old context was cancelled) and returns after `adjustParams`. The behaviour matches what it did before the change.

The remaining findings are design and maintainability concerns about how the fix is expressed. None of them is a bug.

## Finding 1 — Lock ownership is handed to a different goroutine (`go ac.resetTransportAndUnlock()`), and the "…AndUnlock" naming and contract are a smell

Location: `clientconn.go` `updateAddrs` (about line 996) and `resetTransportAndUnlock` (about line 1231).

`updateAddrs` now returns with `ac.mu` still held, and the spawned goroutine unlocks it. The Go race detector and vet do not model this. It only works because Go mutexes are not goroutine-affine, and it is easy to break. Any future early return added to `resetTransportAndUnlock` before the first unlock, or any new caller, must remember the release contract. The failure mode is a wedged subchannel, because everything on `ac` takes `ac.mu`. It is not a compile error and not a clean panic.

There is a second consequence in the same function. The `defer ac.transport.GracefulClose()` has to acquire `ac.mu` in `onClose`. It now waits for the spawned goroutine to be scheduled and to reach `Unlock`. The function-level comment ("we have to defer here because GracefulClose => onClose requires locking ac.mu") still reads as if the defer runs after the lock is free. In the changed flow, the lock is held by another goroutine at that point. The comment is now misleading, and `updateAddrs` is coupled to goroutine scheduling in a way it was not before.

The "AndUnlock" function shape has three properties that make it fragile:

- The function's first lines run under a caller-owned lock.
- It contains six lock/unlock cycles afterwards, so the reader must track "held on entry, released once, re-taken later" across about 70 lines.
- Its doc comment says "unconditionally connects", but the body returns early when `acCtx.Err() != nil`. The comment is slightly inaccurate.

The team's existing convention for functions that need the lock held is the `…Locked` suffix, where the function neither takes nor releases it. This function breaks that convention. That is why the review thread had to argue about whether to enforce locking in code (threads 11–17). Those threads were resolved by "the name and comment should be sufficient".

### Worked code-judo proposal

Split the function at the natural seam. The seam is the point where the original code already called `ac.mu.Unlock()`, just before `tryAllAddrs`. The "prepare" half is pure bookkeeping under the lock. The "run" half is the unlocked dial-and-backoff loop.

```go
// connectAttempt captures everything the unlocked half needs.
type connectAttempt struct {
    ctx      context.Context
    addrs    []resolver.Address
    deadline time.Time
    backoff  time.Duration
}

// startConnectLocked moves ac to CONNECTING and snapshots the attempt.
// ac.mu must be held.
func (ac *addrConn) startConnectLocked() (connectAttempt, bool) {
    if ac.ctx.Err() != nil {
        return connectAttempt{}, false
    }
    ...compute backoffFor, dialDuration, deadline...
    ac.updateConnectivityState(connectivity.Connecting, nil)
    return connectAttempt{ac.ctx, ac.addrs, deadline, backoffFor}, true
}

// runConnect performs the dial/backoff loop; ac.mu must NOT be held.
func (ac *addrConn) runConnect(a connectAttempt) { ...existing body after Unlock... }
```

Callers:

```go
// connect()
ac.mu.Lock()
... state checks (each unlocks and returns) ...
a, ok := ac.startConnectLocked()
ac.mu.Unlock()
if ok { ac.runConnect(a) }

// updateAddrs()
a, ok := ac.startConnectLocked()
ac.mu.Unlock()
if ok { go ac.runConnect(a) }
```

Every function then follows the standard contract: `…Locked` runs under the lock, and everything else runs without it. Every `Lock` is paired with an `Unlock` in the same function. The race is still closed, because the transition to `Connecting` happens in the same critical section as the Idle check. No lock crosses a goroutine boundary, and the defer coupling in `updateAddrs` disappears. The size is comparable to the merged diff, and the code reads as a two-phase operation instead of a mutex handoff.

Verification status: analysis only, and I did not implement the proposal. The equivalence argument is that `resetTransportAndUnlock` already does exactly this split internally, with `Unlock()` between the state update and `tryAllAddrs`.

## Finding 2 — No regression test locks in the fix

Location: `clientconn.go` (no test file in the diff).

The PR changes a concurrency invariant in the core channel type and adds no test. The only evidence is the description's claim that `Test/AuthorityRevive` did not flake in 100000 runs. That test lives in `xds/internal/xdsclient/tests`, and it detects the problem only indirectly, through an unexpected second transport to the xDS server. My search of the top-level `*_test.go` files and `test/*_test.go` found no test that asserts a single `connect()` attempt under concurrent `connect()` calls. The 0.4% baseline flake rate shows the race is narrow and would probably reappear silently after any refactor.

A focused test would call `ac.connect()` from N goroutines against a listener that counts accepted connections (or a `dopts.copts.Dialer` counter), with a barrier to maximise overlap, and assert exactly one dial. The same pattern with `updateAddrs` running alongside `connect()` would cover the second call site.

Verification status: I searched for existing tests by grep only and did not run any.

## Finding 3 — Stale and incomplete documentation around the changed contract

Location: `clientconn.go` `connect()` doc comment (about line 903) and the `updateAddrs` deferred-`GracefulClose` comment.

The `connect()` doc still says only "starts creating a transport. It does nothing if the ac is not IDLE". It says nothing about the guarantee this PR establishes, which is that the state is `Connecting` by the time the lock is first released. That guarantee is the whole reason the lock now spans the call. The `updateAddrs` comment about deferring `GracefulClose` (Finding 1) no longer describes the actual lock situation. Both are cheap to fix and they record the invariant that the bug violated.

Verification status: by inspection.

## Not flagged

- File size: `clientconn.go` stays over 1000 lines, but the diff shrinks it slightly and does not cross the threshold.
- No new conditionals, flags, casts or wrappers were added.
- The early `acCtx.Err()` return in `resetTransportAndUnlock` is not dead code. `ac.ctx` derives from `cc.ctx`, which `ClientConn.Close` cancels before it tears down the addrConns, so the state may not yet be `Shutdown` when the context is already cancelled.
