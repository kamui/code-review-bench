# 01 — addrConn connect lifecycle (`clientconn.go`)

Scope: the entire diff of `grpc/grpc-go#7390` (`main...review-head`, `daab5634..76ef33f4`), which touches only `clientconn.go` (+6 / −7). The change renames `addrConn.resetTransport` to `resetTransportAndUnlock`, moves the initial `ac.mu.Lock()` out of it and into its two callers (`addrConn.connect` and `addrConn.updateAddrs`), and deletes the callers' `ac.mu.Unlock()` so the lock is held continuously from the caller's state check until the transition to `Connecting`.

## What the PR is fixing, restated

`acBalancerWrapper.Connect` runs `go acbw.ac.connect()` (`balancer_wrapper.go:276`). Before the PR, `connect` locked `ac.mu`, verified `ac.state == Idle`, unlocked, and then called `resetTransport`, which re-locked and moved the state to `Connecting`. Two concurrent `connect` calls could both see `Idle` in that gap and both run `tryAllAddrs`, orphaning one transport. The PR closes the gap by making the Idle check and the `Connecting` transition a single critical section. That diagnosis is right, and the fix does remove the race. The problems below are about how the critical section was extended, not whether it needed extending.

## Measurements and commands

- `git diff main...review-head` — one hunk each in `connect` (`clientconn.go:918-923`), `updateAddrs` (`clientconn.go:991-996`) and the function header (`clientconn.go:1231-1234`).
- `wc -l clientconn.go` — 1838 lines at head; the diff is net −1 line. No file-size threshold is crossed (the file was already well past 1000 lines before this PR). No finding is raised on size.
- `git diff --stat main...review-head -- '*_test.go'` — empty: no test file changed.
- `go vet .` (offline, per execution policy) — clean.
- `go test -race -count=300 -run 'Test/AuthorityRevive' ./xds/internal/xdsclient/tests/` — `ok` (39.4s). This matches the PR's claim that the flake is gone, at a much smaller sample size than the author's 100000 runs.
- `go test -race -run 'Test/(ClientConn|AddrConn|SubConn|Connect|Backoff|ResetConnectBackoff|UpdateAddresses|DialWith|CloseConnectionWhenServerPrefaceNotReceived)' .` — `ok` (7.2s).
- Relevant call-chain lookups: `internal/transport/http2_client.go:1045-1064` (`GracefulClose`), `clientconn.go:1351-1352` (`onClose` locks `ac.mu`), `balancer_wrapper.go:255-265` (`acbw.updateState` → `serializer.Schedule`), `internal/grpcsync/callback_serializer.go:64-66` (`Schedule` is an unbounded `Put`, so it does not block).

## Finding A — `updateAddrs` now hands a held mutex to a different goroutine, which silently breaks the deferred-`GracefulClose` invariant right above it

Verification: the broken invariant is CONFIRMED by reading the code; the practical impact (a stall today, a deadlock if the pre-unlock prefix ever blocks) is PLAUSIBLE and was not reproduced dynamically.

In `updateAddrs` the PR replaces `ac.mu.Unlock(); go ac.resetTransport()` with `go ac.resetTransportAndUnlock()` (`clientconn.go:996`). The goroutine that calls `updateAddrs` takes `ac.mu`, and a different goroutine, scheduled at some later point, releases it. Go allows this (`sync.Mutex` is not tied to a goroutine), but it is the least legible way to hold a lock. Neither the race detector nor a reader can pair the `Lock` and `Unlock` from the source, and while the lock is held no running code owns it. For the whole window between the `go` statement and the goroutine being scheduled, every other `ac.mu` user blocks: `getReadyTransport`, `tearDown`, `resetConnectBackoff`, transport `onClose` callbacks, and channelz reads.

The problem gets worse a few lines up. `updateAddrs` has this block (`clientconn.go:983-988`):

```go
// We have to defer here because GracefulClose => onClose, which requires
// locking ac.mu.
if ac.transport != nil {
    defer ac.transport.GracefulClose()
    ac.transport = nil
}
```

The defer only works if `ac.mu` has been released by the time `updateAddrs` returns. Before the PR that held, because the explicit `ac.mu.Unlock()` ran before the function returned. After the PR it does not. `http2Client.GracefulClose` calls `t.onClose(GoAwayInvalid)` inline (`internal/transport/http2_client.go:1055`), and the `onClose` closure built in `createTransport` begins with `ac.mu.Lock()` (`clientconn.go:1351-1352`). In the Ready-to-a-different-address path, the deferred call now runs while the spawned goroutine may still hold `ac.mu`. The caller of `updateAddrs` then blocks inside `GracefulClose` while holding the transport's `t.mu`. That caller is the LB policy, running inside the balancer wrapper's callback serializer via `acBalancerWrapper.UpdateAddresses`. It waits until the scheduler gets around to the goroutine and the goroutine reaches the `ac.mu.Unlock()` after `updateConnectivityState(Connecting)`.

This does not deadlock today only because of a non-local fact: nothing in `resetTransportAndUnlock`'s pre-unlock prefix blocks on anything the `updateAddrs` caller holds. `acbw.updateState` uses `serializer.Schedule`, which is an unbounded buffer `Put`. `dopts.bs.Backoff` and `dopts.minConnectTimeout` are cheap. If anyone later adds a blocking call to that prefix (a synchronous balancer notification, a `cc.mu` acquisition, a channelz write that takes a lock the serializer holds), it becomes a deadlock that only shows up on the Ready→re-address path. The comment that is supposed to warn about exactly this hazard now says something false.

Remedy: stop handing the lock across goroutines. Do the state transition while the caller still holds the lock, then release it in the same function that acquired it, and only then spawn the unlocked connection loop. That keeps the `defer ac.transport.GracefulClose()` comment true and makes the lock scope readable from one function body. Finding B shows the worked version.

## Finding B — code-judo: split "begin connecting (locked)" from "run the attempt (unlocked)" so the `...AndUnlock` contract disappears

Verification: worked proposal, reasoned against the current code paths. Not compiled into the clone (the clone is read-only).

The PR moves one `Lock()` across a function boundary and adds a naming convention (`AndUnlock`) plus a doc comment to explain an asymmetric lock contract. That spreads the complexity around without removing any. Also, `resetTransportAndUnlock` does not have one lock phase. It enters locked, unlocks, re-locks after `tryAllAddrs`, unlocks, re-locks in the backoff timer arm, unlocks, re-locks for the Idle transition, and unlocks again. The "and unlock" in the name describes only the first of five lock/unlock episodes.

The race fix needs one invariant: the `Idle` check and the `Connecting` transition happen in one critical section. Everything else in `resetTransport` already runs unlocked. So the natural cut is:

```go
// connectAttempt captures the parameters of one pass over ac.addrs.
type connectAttempt struct {
	ctx             context.Context
	addrs           []resolver.Address
	backoffFor      time.Duration
	connectDeadline time.Time
}

// startConnectingLocked moves ac to CONNECTING and returns the parameters for
// the attempt, or nil if ac has been torn down. ac.mu must be held.
func (ac *addrConn) startConnectingLocked() *connectAttempt {
	if ac.ctx.Err() != nil {
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
	ac.updateConnectivityState(connectivity.Connecting, nil)
	return &connectAttempt{
		ctx:             ac.ctx,
		addrs:           ac.addrs,
		backoffFor:      backoffFor,
		connectDeadline: time.Now().Add(dialDuration),
	}
}

// resetTransport runs a connection attempt started by startConnectingLocked.
// ac.mu must not be held.
func (ac *addrConn) resetTransport(a *connectAttempt) {
	if err := ac.tryAllAddrs(a.ctx, a.addrs, a.connectDeadline); err != nil {
		// ... unchanged failure / backoff / Idle handling, using a.ctx and a.backoffFor ...
	}
	// ... unchanged success path ...
}
```

The callers then read top to bottom, with `Lock` and `Unlock` paired in one body:

```go
func (ac *addrConn) connect() error {
	ac.mu.Lock()
	// ... existing Shutdown / non-Idle checks, unchanged ...
	attempt := ac.startConnectingLocked()
	ac.mu.Unlock()
	if attempt != nil {
		ac.resetTransport(attempt)
	}
	return nil
}
```

```go
	// in updateAddrs, replacing `go ac.resetTransportAndUnlock()`:
	attempt := ac.startConnectingLocked()
	ac.mu.Unlock()
	if attempt != nil {
		go ac.resetTransport(attempt)
	}
}
```

What this deletes and restores:

- The cross-goroutine mutex handoff in `updateAddrs` goes away (Finding A), and `ac.mu` is free before the deferred `GracefulClose` runs, so that comment is true again.
- The `...AndUnlock` naming convention and its "must be held / will be released" doc comment are no longer needed. The codebase already has a convention for "caller holds the lock" (`...Locked`), and the proposal uses it instead of adding a second, asymmetric one.
- The helper gets a precise, testable contract: it returns nil only when the addrConn has been torn down. The current code hides that early-return inside a function documented as "unconditionally connects".
- Behavior is preserved. The Idle check and the `Connecting` transition still share one critical section. Parameters are still snapshotted under the lock at the same logical point. The unlocked connection loop is byte-for-byte the old tail of `resetTransport`.

This is the "make the cut where the invariant is" move. It is a few lines larger than the PR's diff but has fewer concepts and no lock ownership crossing a goroutine boundary. It also matches the issue discussion's first idea ("set the state to connecting while `addrConn.connect` has the mutex locked") without scattering the transition into both callers.

## Finding C — the new doc comment misdescribes the function

Verification: CONFIRMED by reading `clientconn.go:1231-1239`.

The comment added in `ff977b39` says `resetTransportAndUnlock` "unconditionally connects the addrConn". The first thing the function does is `if acCtx.Err() != nil { ac.mu.Unlock(); return }`, which returns without connecting. That is the tear-down / re-address case, which is exactly the case a caller needs to reason about. The second sentence, "this function will guarantee it is released", is true only of the caller's acquisition. The function then re-acquires and releases `ac.mu` several more times, which matters to anyone trying to reason about what state the addrConn is in when it returns. If Finding B is adopted, this comment goes away. Otherwise, reword it to say the function transitions to `Connecting` unless `ac.ctx` is already canceled, that it releases the caller's hold on `ac.mu` before dialing, and that it re-acquires the lock internally.

## Finding D — the fixed concurrency invariant is not pinned by any test in the package that owns it

Verification: CONFIRMED (`git diff --stat main...review-head -- '*_test.go'` is empty); the suggested test is a proposal, not executed.

The only evidence that the race is fixed is that an unrelated xDS-client test (`Test/AuthorityRevive` in `xds/internal/xdsclient/tests`) stops flaking at roughly a 0.4% rate. The invariant being fixed is "concurrent `SubConn.Connect()` calls on an Idle subchannel produce exactly one connection attempt". That belongs to the `grpc` package, and nothing there asserts it. A future refactor of `connect`/`resetTransport`, such as the restructuring suggested above or any other, could reintroduce the gap, and the only signal would be a low-rate flake in a distant package. Add a focused test in `test/subconn_test.go` or `clientconn_test.go`. Use a tiny test balancer that creates one SubConn and calls `sc.Connect()` from several goroutines at once, plus a `WithContextDialer` that counts dials and blocks until released. Assert that exactly one dial happens per address, and loop the scenario enough times inside the test to hit the window. The blocking dialer does not widen the pre-PR race window, which sits between `connect`'s unlock and `resetTransport`'s re-lock. What it does is make the dial count unambiguous and keep the orphaned attempt observable. With the fix, any failure of the assertion is a real regression rather than noise.

## Non-findings, recorded for completeness

- File size: `clientconn.go` is 1838 lines and the PR shrinks it by one line. The 1k-line rule is not triggered by this PR.
- No new branching, flags, casts, or feature-specific logic are introduced. The diff is a pure lock-scope change.
- The reviewer discussion about `acCtx.Err()` being checked after the state has moved to `Connecting` is not a quality concern. `tearDown` sets `Shutdown` under the same lock and cancels the context, so the ordering is sound.
