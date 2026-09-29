# Detail 01: `clientconn.go` addrConn locking (PR #7390)

## Scope and commands

Diff inspected with `git diff main...review-head` (1 file, +6/−7). Surrounding code read at `clientconn.go` lines 895-1000 (`connect`, `updateAddrs`), 1225-1345 (`resetTransportAndUnlock`, `tryAllAddrs`), 1345-1420 (`createTransport`, `onClose`), and `internal/transport/http2_client.go` `GracefulClose` (line 1045). Caller search: `grep -rn "AndUnlock\|resetTransport" --include='*.go' .`. Only two callers of the renamed function exist (lines 922 and 996). The other hits are stale log strings in `test/`. Line count: `wc -l clientconn.go` gives 1838, so no threshold is crossed. No tests were run: the change is a single-file lock refactor and no focused test in the allowance targets it deterministically.

## What the change does

Before the change, `connect()` did check-state, unlock, `resetTransport()`. `resetTransport` re-locked and set `Connecting`. Two goroutines could both pass the `Idle` check before either set `Connecting`. `updateAddrs` had the same gap around `go ac.resetTransport()`. After the change, the lock stays held from the state check until the `Connecting` transition inside `resetTransportAndUnlock`, which unlocks just before `tryAllAddrs`. The fix is correct for the reported race, and every early-return path of `resetTransportAndUnlock` unlocks (the `acCtx.Err()` early return unlocks explicitly, and the main path unlocks after `updateConnectivityState(Connecting)`).

## 1. F1: ownership transfer instead of splitting prologue from loop (verified by reading)

Evidence.
- `resetTransportAndUnlock` (1234-1301) has a locked prologue of about 20 lines (ctx check, address snapshot, backoff, deadline, `updateConnectivityState(Connecting)`), followed by an unlocked loop containing six further `Lock/Unlock` pairs.
- `connect()` at 921 no longer holds the lock via a scoped pattern; it has three explicit `Unlock` sites and one implicit release inside the callee.
- `updateAddrs` at 994-996: `go ac.resetTransportAndUnlock()` runs with `ac.mu` held. The earlier `defer ac.transport.GracefulClose()` fires at return of `updateAddrs`. `GracefulClose` calls `t.onClose(GoAwayInvalid)` inline (http2_client.go:1055). `onClose` in `createTransport` does `ac.mu.Lock()`. So the deferred call waits for the spawned goroutine to be scheduled and reach `Unlock`. Because the goroutine never waits for `updateAddrs`, there is no deadlock, but the comment "We have to defer here because GracefulClose => onClose, which requires locking ac.mu" was written for a world in which `updateAddrs` had already unlocked.
- Sync semantics: Go allows unlocking from another goroutine, and the race detector accepts it, so this is not a data race, only a readability and fragility issue.

Worked code-judo proposal.

```go
// connectAttempt is the state snapshotted under ac.mu for one connection attempt.
type connectAttempt struct {
	ctx         context.Context
	addrs       []resolver.Address
	backoffFor  time.Duration
	deadline    time.Time
}

// prepareConnectLocked moves ac to CONNECTING and snapshots what the attempt needs.
// Caller must hold ac.mu. Returns false if ac was torn down.
func (ac *addrConn) prepareConnectLocked() (connectAttempt, bool) { ... }

// runAttempt dials and handles backoff. It takes ac.mu itself when needed.
func (ac *addrConn) runAttempt(a connectAttempt) { ... }
```

`connect()` then reads: lock; check Shutdown and non-Idle; `a, ok := ac.prepareConnectLocked()`; unlock; `if ok { ac.runAttempt(a) }`. `updateAddrs` does the same under its existing lock, then `go ac.runAttempt(a)` after unlocking normally, so the `defer GracefulClose` runs with no lock held, exactly as before the PR. The atomicity property is retained because the state moves to `Connecting` inside the same critical section as the `Idle` check. The `AndUnlock` name, the callee-unlocks contract and the goroutine lock handoff all go away, and `runAttempt` has no lock precondition. The diff would probably be larger than 13 lines, but the net concept count goes down.

Risk to check before adopting: the `ctx.Err()` early return currently happens inside the callee; in the split version `prepareConnectLocked` returns `ok=false` in that case, so the behavior is preserved.

## 2. F2: undocumented invariant and imprecise doc comment (verified by reading)

- `connect()` (905-923) doc comment says only "connect starts creating a transport. It does nothing if the ac is not IDLE." It does not state that the transition out of `Idle` must occur under the same lock hold as the check. This is the property the PR exists to guarantee.
- The comment at 1231-1233 says "unconditionally connects the addrConn". The body returns early if the addrConn context is already cancelled. It also does not say that the lock is dropped before the first dial, after which the function manages locking itself.
- Convention: `grep` shows `updateResolverStateAndUnlock` (clientconn.go:728) as the one precedent for the suffix. `startHealthCheck` documents its precondition as "Caller must hold ac.mu." Consistency would favor stating the precondition in that same phrasing.
- The review thread concluded that enforcing the lock precondition in code is not worthwhile (private function, panics on unlock of an unlocked mutex, race-detector runs). I agree and do not raise it again.

Suggested text for `connect()`: "The check that ac is Idle and the move to Connecting must happen in one critical section, or concurrent callers can start parallel connection attempts."

## 3. F3: no deterministic regression test (verified by inspection of the diff; no test files changed)

The diff touches only `clientconn.go`. The evidence for the fix is a manual 100000-iteration run of `Test/AuthorityRevive` (flake rate before: 399 of 100000, per the issue thread). A direct test would use the existing test dialer hooks (`WithContextDialer` with an atomic counter and a blocking dial), create a `ClientConn` with a single address, obtain the subchannel, and call `connect()` from N goroutines released together by a barrier. It would assert the counter is exactly 1. A sibling test would race `updateAddrs` with `connect()`. Both run in milliseconds and should fail on the pre-PR code with high probability under `-race -count=N`. These tests were not run here because they do not exist; this is a recommendation only.

## Measurements summary

| Item | Value |
| --- | --- |
| Files changed | 1 |
| Lines | +6 / −7 |
| clientconn.go length | 1838 lines (already above 1k; PR reduces by 1) |
| Callers of renamed function | 2 |
| New conditionals | 0 |
| Lock/Unlock sites in `resetTransportAndUnlock` after the prologue | 6 lock/unlock pairs (unchanged) |
