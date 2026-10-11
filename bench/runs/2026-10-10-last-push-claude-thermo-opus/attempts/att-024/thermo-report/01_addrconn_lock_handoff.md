# 01 — addrConn lock handoff (`clientconn.go`)

Carries the evidence for Findings 1 and 3 and Question 1 of `summary.md`.

## What the PR changed

Before the PR, both callers of `resetTransport` released `ac.mu` and the callee took it again:

```go
// connect(), merge-base
	ac.mu.Unlock()

	ac.resetTransport()
	return nil

// updateAddrs(), merge-base
	ac.mu.Unlock()

	// Since we were connecting/connected, we should start a new connection
	// attempt.
	go ac.resetTransport()

// merge-base
func (ac *addrConn) resetTransport() {
	ac.mu.Lock()
	acCtx := ac.ctx
```

After the PR the callers keep the lock and the callee, renamed `resetTransportAndUnlock`, starts with it held. In `connect()` (`clientconn.go:905-924`) that is a synchronous call and reads fine. In `updateAddrs()` (`clientconn.go:940-997`) the call is behind `go`, so the lock taken at line 948 by the balancer's goroutine is released at line 1237 or line 1262 by a different goroutine, at some point after `updateAddrs` has returned.

## Why the handoff is a structural regression and not a style point

`sync.Mutex` allows unlock from another goroutine, so the handoff is not a bug by itself. The problem is what it does to local reasoning in `updateAddrs`.

The function has four early exits that each write `ac.mu.Unlock(); return` (lines 949-952, 956-962, 968-973). A reader builds the rule "every exit unlocks" from those, and the final exit breaks it without any visible marker other than the callee's name. Anything executed after the `go` statement runs inside the critical section. Today there is exactly one such thing, and it is the one thing the surrounding comment says must not run there:

```go
	// We have to defer here because GracefulClose => onClose, which requires
	// locking ac.mu.
	if ac.transport != nil {
		defer ac.transport.GracefulClose()
		ac.transport = nil
	}
```

`http2Client.GracefulClose` (`internal/transport/http2_client.go:1045-1064`) takes `t.mu` and calls `t.onClose` synchronously. `onClose` is the closure built in `createTransport` (`clientconn.go:1351-1377`), and its first statement is `ac.mu.Lock()`. So after the PR the sequence on the balancer's goroutine is: `updateAddrs` returns with `ac.mu` held, the defer runs, `GracefulClose` takes `t.mu`, `onClose` blocks on `ac.mu`, and it stays blocked until the scheduler runs the new goroutine as far as line 1262.

### Measurement

I confirmed the ordering with a probe, in scratch copies of both trees under `clone-work/scratch/instr-base` and `clone-work/scratch/instr-head`. The only edit is to the defer:

```go
	if ac.transport != nil {
		tr := ac.transport
		defer func() {
			if ac.mu.TryLock() {
				ac.mu.Unlock()
				println("SCRATCH-PROBE deferred GracefulClose: ac.mu FREE")
			} else {
				println("SCRATCH-PROBE deferred GracefulClose: ac.mu HELD")
			}
			tr.GracefulClose()
		}()
		ac.transport = nil
	}
```

Command, run once in each scratch tree:

```
go test -v -count=1 -run 'Test/(.*UpdateAddresses.*|.*SubConn.*|.*PickFirst.*|.*Pickfirst.*)' ./test . \
  2>&1 | grep -E "SCRATCH-PROBE|^ok|^FAIL" | sort | uniq -c
```

Result:

| Tree | `ac.mu FREE` | `ac.mu HELD` | Packages |
| --- | --- | --- | --- |
| merge-base (`daab5634`) | 7 | 0 | `grpc` ok, `grpc/test` ok |
| head (`76ef33f4`) | 0 | 7 | `grpc` ok, `grpc/test` ok |

### Is it a deadlock?

No, as far as I can establish by reading. The spawned goroutine's locked prefix (`clientconn.go:1235-1262`) reads `ac.ctx` and `ac.addrs`, calls `ac.dopts.bs.Backoff` and the optional `ac.dopts.minConnectTimeout` hook, and calls `updateConnectivityState`, which closes a channel, writes a channelz trace, and calls `acbw.updateState`. `acbw.updateState` (`balancer_wrapper.go:255-265`) only schedules onto the serializer. None of that needs `t.mu` or anything else the blocked balancer goroutine holds. The lock order `t.mu → ac.mu` used by `GracefulClose` and `Close` is the same order as before the PR, and I found no path that takes `ac.mu` and then `t.mu`.

Verification status: the "no deadlock" conclusion is by reading, backed by the focused test runs passing at head. It is not proven by execution.

### What it costs even without a deadlock

- `GracefulClose` holds the old transport's `t.mu` while it waits for a goroutine to be scheduled. Other work on that transport that needs `t.mu` waits with it.
- `UpdateAddresses` is called by LB policies, often from inside the balancer serializer. Its return now depends on goroutine scheduling.
- The comment at lines 983-984 states a requirement the code no longer meets. Comments about lock requirements are what people rely on when they edit this file.
- `resetTransportAndUnlock` has two unlock points for the handed-off lock (lines 1237 and 1262). Any future early return added to the locked prefix must remember to unlock, on behalf of a caller in a different goroutine, with no `defer` available.

## Finding 3 evidence — the doc comment

```go
// resetTransportAndUnlock unconditionally connects the addrConn.
//
// ac.mu must be held by the caller, and this function will guarantee it is released.
func (ac *addrConn) resetTransportAndUnlock() {
	acCtx := ac.ctx
	if acCtx.Err() != nil {
		ac.mu.Unlock()
		return
	}
```

"Unconditionally" is meant in contrast to `connect()`, which checks for IDLE first. Read cold it says the function always connects, and the next four lines show a condition under which it does not.

The lock sentence is accurate but incomplete. After line 1262 the function re-locks and unlocks at 1266/1278, at 1283/1285 and at 1293/1297 on the failure path, or at 1301/1303 on success, and between those it runs `tryAllAddrs` (bounded by the connect deadline) and a backoff timer. "AndUnlock" has one precedent, `updateResolverStateAndUnlock` (`clientconn.go:728`), whose three call sites in `resolver_wrapper.go` are all synchronous. A search for `go …Locked(` or `go …AndUnlock(` across the repository returns only `clientconn.go:996`.

## Question 1 evidence — empty address list

`updateAddrs` lines 990-992:

```go
	if len(addrs) == 0 {
		ac.updateConnectivityState(connectivity.Idle, nil)
	}
```

followed, in the same critical section at head, by `updateConnectivityState(connectivity.Connecting, nil)` at line 1261. `tryAllAddrs` (`clientconn.go:1309-1342`) returns `firstConnErr`, which is nil when the loop body never runs, so `resetTransportAndUnlock` takes the success branch and returns with the state at CONNECTING and `ac.transport` nil. The merge-base publishes the same two states in two critical sections. This was not exercised by execution.

## Worked code-judo proposal

Split the function where the lock is released today. The locked half is a pure state transition; the unlocked half is the attempt.

Diff against the head (`clientconn.go`), as applied in `clone-work/scratch/judo-tree`:

```diff
@@ -919,7 +919,12 @@
 		return nil
 	}
 
-	ac.resetTransportAndUnlock()
+	attempt := ac.startConnectingLocked()
+	ac.mu.Unlock()
+
+	if attempt != nil {
+		ac.runConnectAttempt(attempt)
+	}
 	return nil
 }
 
@@ -993,7 +998,12 @@
 
 	// Since we were connecting/connected, we should start a new connection
 	// attempt.
-	go ac.resetTransportAndUnlock()
+	attempt := ac.startConnectingLocked()
+	ac.mu.Unlock()
+
+	if attempt != nil {
+		go ac.runConnectAttempt(attempt)
+	}
 }
 
 // getServerName determines the serverName to be used in the connection
@@ -1228,14 +1238,25 @@
 	}
 }
 
-// resetTransportAndUnlock unconditionally connects the addrConn.
+// connectAttempt is the snapshot of addrConn state that one connection attempt
+// runs against. It is taken under ac.mu in the same critical section that moves
+// the addrConn to CONNECTING.
+type connectAttempt struct {
+	ctx        context.Context
+	addrs      []resolver.Address
+	backoffFor time.Duration
+	deadline   time.Time
+}
+
+// startConnectingLocked unconditionally moves the addrConn to CONNECTING and
+// returns the attempt to pass to runConnectAttempt once ac.mu is released. It
+// returns nil, and changes nothing, if the addrConn was torn down.
 //
-// ac.mu must be held by the caller, and this function will guarantee it is released.
-func (ac *addrConn) resetTransportAndUnlock() {
+// Caller must hold ac.mu.
+func (ac *addrConn) startConnectingLocked() *connectAttempt {
 	acCtx := ac.ctx
 	if acCtx.Err() != nil {
-		ac.mu.Unlock()
-		return
+		return nil
 	}
 
 	addrs := ac.addrs
@@ -1259,9 +1280,16 @@
 	connectDeadline := time.Now().Add(dialDuration)
 
 	ac.updateConnectivityState(connectivity.Connecting, nil)
-	ac.mu.Unlock()
+	return &connectAttempt{ctx: acCtx, addrs: addrs, backoffFor: backoffFor, deadline: connectDeadline}
+}
 
-	if err := ac.tryAllAddrs(acCtx, addrs, connectDeadline); err != nil {
+// runConnectAttempt tries the attempt's addresses and, if none connects, moves
+// the addrConn to TRANSIENT_FAILURE, backs off, and returns it to IDLE.
+//
+// Caller must not hold ac.mu.
+func (ac *addrConn) runConnectAttempt(a *connectAttempt) {
+	acCtx, backoffFor := a.ctx, a.backoffFor
+	if err := ac.tryAllAddrs(acCtx, a.addrs, a.deadline); err != nil {
 		ac.cc.resolveNow(resolver.ResolveNowOptions{})
 		ac.mu.Lock()
 		if acCtx.Err() != nil {
```

What this buys:

- `connect()` and `updateAddrs()` each lock and unlock `ac.mu` themselves. The deferred `GracefulClose` runs after the unlock, as its comment requires.
- The check-then-transition atomicity that the PR is about is kept: `startConnectingLocked` runs in the caller's critical section, so CONNECTING is published before the lock is released, in both callers.
- `updateAddrs` publishes CONNECTING before it returns instead of at a later time chosen by the scheduler.
- The locked half has no unlock statements, so an early return added to it cannot leak the lock.
- The helper name follows the `…Locked` convention, which 33 functions in this repository use.

What it costs: `clientconn.go` goes from 1838 to 1866 lines. The extra lines are a four-field struct, two doc comments, and two `if attempt != nil` guards. A variant that returns a `func()` closure from `startConnectingLocked` instead of a struct saves about ten of those lines at the price of hiding what the attempt captures; I prefer the struct.

Behaviour is intended to be identical to the head apart from the timing of the unlock. The context check, the state published, the snapshot of `addrs`, the backoff computation and the deadline are the same statements in the same order.

### Verification of the proposal

All in `clone-work/scratch/judo-tree`, offline, with the pinned Go environment.

| Check | Command | Result |
| --- | --- | --- |
| Formatting | `gofmt -l clientconn.go` | no output |
| Build and vet | `go build . && go vet .` | no output |
| Focused root tests, race detector | `go test -race -count=1 -run 'Test/(UpdateAddresses\|ResetConnectBackoff\|.*Backoff.*\|Dial.*\|.*Connect.*\|.*SubConn.*)' .` | `ok google.golang.org/grpc 7.874s` |
| Scratch race test (see `02_regression_test_gap.md`) | `go test -count=1 -v -run TestConcurrentConnectDialsOnce .` in `scratch/race-judo` | 0 of 3000 iterations with dial count != 1, PASS |

The same focused root-package command at the unmodified head returned `ok google.golang.org/grpc 8.008s`, and `go test -count=1 -run 'Test/(.*UpdateAddresses.*|.*SubConn.*|.*PickFirst.*|.*Pickfirst.*|.*GoAway.*)' ./test` at the head returned `ok google.golang.org/grpc/test 11.417s`. The `./test` package and the xDS packages were not run against the proposal.

## Things checked and not raised

- File size: `git show main:clientconn.go | wc -l` gives 1839 and `wc -l clientconn.go` at head gives 1838. The file was already well past 1000 lines and the PR shrinks it by one.
- `connect()` now has two explicit unlocks and one delegated unlock. With a synchronous callee named `…AndUnlock` that is readable and has precedent in this file; I would not raise it without the `go` call site.
- Six references to the string `resetTransport` remain outside `clientconn.go`, in `declareLogNoise` lists in `test/goaway_test.go` and `test/end2end_test.go`. They name a log message that no longer exists in the source at either commit, so they were stale before this PR and the rename does not change that.
