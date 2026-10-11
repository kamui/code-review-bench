# Detail report 01: clientconn.go (addrConn connect / updateAddrs / resetTransport)

Scope: `git diff main...review-head`, one file, +6/-7. Commits c3a3d1c4, 6214c9dd, ff977b39, and the merge 76ef33f4.

## What the diff does

`addrConn.resetTransport()` used to take `ac.mu` itself. Its callers, `connect()` and `updateAddrs()`, released the lock before calling it. That left a window in which a second `connect()` still saw `Idle` and started a second attempt, as described in issue #7365.

The diff renames the function to `resetTransportAndUnlock()` and drops its leading `ac.mu.Lock()`. Callers now enter it still holding `ac.mu`. The function sets `Connecting` under that lock and then releases it itself. In `connect()` that is a straight handoff. In `updateAddrs()` the call is `go ac.resetTransportAndUnlock()`, so the lock is taken in one goroutine and released in another.

The fix is correct for the reported race. The `Idle` check and the `Connecting` transition now happen in one critical section. Everything below is about structure, not correctness of the race fix.

## Measurements and commands

- `git diff main...review-head` showed 1 file, +6/-7.
- `wc -l clientconn.go` gives 1838 lines. The file was already over 1k lines and the net change is -1, so the 1k-line rule is not triggered.
- `grep -n resetTransport *.go` shows exactly two call sites (clientconn.go:922 and :996) and the definition (:1234). The rename touches nothing else.
- `grep -n "Locked()" clientconn.go` shows the file's own convention for "caller holds the lock" is a `...Locked` suffix, for example `initIdleStateLocked`.
- I read `http2Client.GracefulClose` (internal/transport/http2_client.go:1045-1064). It calls `t.onClose(GoAwayInvalid)` synchronously. The `onClose` closure in `createTransport` (clientconn.go:1351) takes `ac.mu.Lock()`.
- I ran no tests. The packet's flake verification (100000 runs of Test/AuthorityRevive) was done upstream and is not reproduced here.

## Finding 1 (structural): lock ownership is handed to another goroutine in updateAddrs

Location: clientconn.go:983-996, with the definition at :1234.

`updateAddrs` locks `ac.mu` at the top of the function and never unlocks it on the restart path. It ends with `go ac.resetTransportAndUnlock()`, and the unlock now happens in the spawned goroutine. A mutex locked in one goroutine and unlocked in another is legal in Go. It still breaks the usual rule that a function which locks also unlocks, and a reader can no longer verify lock balance by reading `updateAddrs` alone.

There is a concrete consequence in the same function. Lines 983-986 contain `defer ac.transport.GracefulClose()` with the comment "We have to defer here because GracefulClose => onClose, which requires locking ac.mu". The old code made that comment true by calling `ac.mu.Unlock()` explicitly before the function returned, so the deferred call ran with the lock free.

After the diff the deferred call runs while `ac.mu` is still held on behalf of a goroutine that may not have started. `GracefulClose` calls `onClose` synchronously, and `onClose` calls `ac.mu.Lock()`. So `updateAddrs` now blocks until the new goroutine has been scheduled and has reached the `ac.mu.Unlock()` inside `resetTransportAndUnlock`.

I found no deadlock. The goroutine does not wait on `updateAddrs`, and the old `onClose` closure returns early because its `ctx` was cancelled by `ac.cancel()`. But the stale comment is now misleading, and correctness depends on a scheduling dependency that no one wrote down. If `resetTransportAndUnlock` ever blocks before its unlock, `updateAddrs` blocks with it.

This works, but it makes the locking harder to reason about than it was before the fix. I did not run an interleaving test to try to expose it.

### Code-judo proposal: split the attempt into a locked "begin" step and an unlocked "run" step

The reviewers discussed and rejected renaming to `resetTransportLocked`. The stated worry was that `resetTransport` releases the lock midway and `updateAddrs` runs it in a goroutine. That worry assumes the whole body must stay in one function. It does not.

The body has two parts:

- **A locked prelude (the first ~20 lines).** It checks `ac.ctx.Err()`, snapshots `ac.addrs`, computes `backoffFor`, `dialDuration` and `connectDeadline`, and calls `updateConnectivityState(Connecting)`. Everything it touches is guarded by `ac.mu`.
- **An unlocked remainder.** It runs `tryAllAddrs` and the backoff and retry logic, and takes `ac.mu` itself only in short critical sections.

Proposed shape:

```go
// beginConnectLocked validates the addrConn, moves it to CONNECTING and
// snapshots what the dial needs. ac.mu must be held.
func (ac *addrConn) beginConnectLocked() (a connectAttempt, ok bool) { ... }

// runConnectAttempt dials and handles backoff. ac.mu must NOT be held.
func (ac *addrConn) runConnectAttempt(a connectAttempt) { ... }
```

`connect()` would then read:

```go
ac.mu.Lock()
... idle checks ...
a, ok := ac.beginConnectLocked()
ac.mu.Unlock()
if ok { ac.runConnectAttempt(a) }
```

`updateAddrs()` would call `beginConnectLocked()` under its existing lock. Its tail would then be `ac.mu.Unlock(); go ac.runConnectAttempt(a)`, or use `defer ac.mu.Unlock()` with the `GracefulClose` defer ordered correctly.

What this removes:

- the "AndUnlock" function that unlocks someone else's lock;
- the cross-goroutine unlock;
- the stale defer comment (the lock is released by a plain unlock before the deferred `GracefulClose` runs, as before);
- the `acCtx.Err()` early-return unlock in the middle of the function, which becomes an `ok == false` result.

The race fix is unchanged: the idle check and the state change still share one critical section, which is the actual invariant. I have not built or tested this proposal. It needs a compile and a run of the clientconn tests with `-race` before anyone relies on it.

## Finding 2 (boundary and legibility): the "AndUnlock" contract is documented, not structural, and the doc comment is imprecise

Location: clientconn.go:1231-1234.

The doc comment says "unconditionally connects the addrConn". Two things are wrong with that:

- The function returns early without connecting when `acCtx.Err() != nil`.
- It does not state the second half of its contract, that it holds the lock only until partway through.

The function has now become a half-locked routine. It starts locked, unlocks, then re-locks four more times (the TransientFailure path, the backoff increment, the Idle transition, and the success reset). It mixes "caller holds the lock" at the top with "I take it myself" below.

The file's established convention is a `...Locked` suffix for "caller holds the lock" and no suffix for "I lock". `...AndUnlock` is a third convention introduced for one function. The reviewer discussion (purnesh42H, arjan-bal) acknowledged that nothing enforces the precondition, and the author's answer was that unlocking an unlocked mutex panics. That is true, but it only catches a missing lock at runtime on the path that runs. It does not catch a caller who locks and then forgets that the callee unlocks, which is a double-unlock and also panics, or a future early return in a caller.

The split proposed under Finding 1 would resolve this one as well. If the split is not wanted, the minimum is to fix the doc comment so it names the early-return case and says where in the function the lock is released.

## Finding 3 (test coverage, lower priority): the race has no deterministic regression test

The only evidence for the fix is the existing flaky xDS test (`Test/AuthorityRevive`, roughly 0.4% failures before the change, 0 in 100000 runs after). That is a good empirical check, but it is a single indirect test in a different package. Nothing in the `grpc` package tests that two concurrent `ac.connect()` calls produce one transport.

A small test would be enough: call `connect()` from N goroutines against a listener that counts accepted connections, and assert that exactly one is accepted. Without it, a later refactor of `connect` or `resetTransport` can reintroduce the race and only a rare flake will show it. The diff modifies only `clientconn.go`, so no such test was added.

## Minor notes, not findings

- Commit 6214c9dd is titled "Make callers of resetBackoff() lock the mutex", but the code it changes is `resetTransport`. The history would be easier to read with the right name. This does not matter once squashed.
- The diff does not enlarge the file (1838 lines, net -1), so the file-size rule does not apply.

## Verification status

| Item | Status |
| --- | --- |
| Race fix is logically correct | Verified by reading: idle check and Connecting transition share one critical section |
| Deferred `GracefulClose` now runs with `ac.mu` held | Verified by reading `GracefulClose` and `onClose` |
| No deadlock from the above | Reasoned, not tested |
| Code-judo split compiles and passes tests | Not attempted; the review is read-only |
