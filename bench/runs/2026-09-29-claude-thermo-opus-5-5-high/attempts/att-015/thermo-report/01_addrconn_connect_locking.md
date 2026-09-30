# 01 — addrConn connect / resetTransport locking (`clientconn.go`)

Scope: `git diff main...review-head` (daab5634..76ef33f4), one file, `clientconn.go` +6/−7.
Every line reference below is to `clientconn.go` at `review-head` unless another file is named.

## What the diff does

Before the change, `addrConn.connect()` checked `ac.state == Idle` under `ac.mu` and then unlocked. After that it called `resetTransport()`, which took `ac.mu` again, computed the dial parameters, and moved to `Connecting`. A second `connect()` that ran between the unlock and the relock also saw `Idle`, so two `tryAllAddrs` loops could run at the same time. `updateAddrs` then closed only one of the two resulting transports. The other became an orphan, which is the flake in issue #7365.

The PR renames `resetTransport` to `resetTransportAndUnlock` (line 1234). The function now expects `ac.mu` to be held when it's called, and both callers keep the lock across the call:

- `connect()` calls it on the same goroutine (line 922).
- `updateAddrs()` calls it as `go ac.resetTransportAndUnlock()` (line 996), with `ac.mu` still held by the calling goroutine.

This fixes the race. The `Idle` check in `connect()` and the move to `Connecting` at line 1261 now happen in one critical section.

## Verification performed

- I read the full `connect`, `updateAddrs`, `resetTransportAndUnlock`, `tryAllAddrs`, `createTransport` and `tearDown` bodies (lines 905–997, 1231–1425, 1545–1580).
- I read `acBalancerWrapper.Connect` / `UpdateAddresses` / `updateState` (`balancer_wrapper.go:255–277`) and `http2Client.GracefulClose` (`internal/transport/http2_client.go:1045–1064`).
- `go vet .` (offline flags from the allowance) reported nothing.
- `go test -race -count=200 -run 'Test/AuthorityRevive' ./xds/internal/xdsclient/tests/` → `ok` (26.6s). This supports the fix but doesn't prove it, because the flake rate reported upstream was about 0.4%.
- File size: `clientconn.go` is 1839 lines at `main` and 1838 at `review-head`. The PR doesn't cross the 1k threshold (the file was already over it) and doesn't make it worse, so there is no size finding.
- `git status --porcelain` was empty after the run.

## Finding 1 — Mutex ownership now spans goroutines; split the locked prologue from the unlocked attempt instead

**Evidence.** In `updateAddrs`, `ac.mu` is taken at line 948 and never released on the calling goroutine. The only release on the connecting path is inside `go ac.resetTransportAndUnlock()` (line 996), at line 1237 or line 1262 of a different goroutine. That goroutine runs whenever the scheduler gets to it. Go's `sync.Mutex` allows this, but a reader can no longer confirm lock balance by looking at one function. They have to follow the lock into a goroutine launch. `resetTransportAndUnlock` also mixes two contracts. Its first 28 lines run under a lock the caller acquired, and the remaining ~40 lines follow the file's usual lock-locally discipline, with four more `Lock`/`Unlock` pairs (lines 1266–1303). The name `...AndUnlock` describes only the first part. Across the rest of the repository, lock-held helpers use the `...Locked` suffix and return with the lock still held (`server.go:1931`, `balancer/rls/balancer.go:482`, `orca/producer.go:156`). This PR adds the codebase's only helper that takes the caller's lock and releases it.

**Why it matters.** The struct comment at lines 1181–1184 says `ac.mu` "is used on the RPC path, so its usage should be minimized as much as possible". The PR now holds it across a `go` statement and the scheduling delay that follows. It also creates the defer-ordering problem in Finding 2. The race was really about one thing: the `Idle`→`Connecting` transition has to happen in the same critical section as the `Idle` check. Nothing else needs to change owners, and the network work in `tryAllAddrs` runs unlocked anyway.

**Code-judo proposal.** Split the function at its natural seam, line 1262. The locked half checks the context, snapshots the dial inputs, moves to `Connecting`, and returns a value describing the attempt. The unlocked half runs that attempt. Each caller then unlocks on its own goroutine, and the "AndUnlock" contract goes away.

```go
// connectAttempt is a snapshot of the inputs for one pass over ac.addrs.
type connectAttempt struct {
	ctx        context.Context
	addrs      []resolver.Address
	backoffFor time.Duration
	deadline   time.Time
}

// startConnectingLocked transitions ac to Connecting and returns the attempt
// to run without ac.mu held. It returns nil if ac.ctx is already canceled.
//
// Caller must hold ac.mu.
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
	return &connectAttempt{ac.ctx, ac.addrs, backoffFor, time.Now().Add(dialDuration)}
}

// runConnectAttempt is today's lines 1264–1303, unchanged, reading from a.
func (ac *addrConn) runConnectAttempt(a *connectAttempt) { ... }
```

Callers:

```go
// connect()
	a := ac.startConnectingLocked()
	ac.mu.Unlock()
	if a != nil {
		ac.runConnectAttempt(a)
	}
	return nil

// updateAddrs()
	a := ac.startConnectingLocked()
	ac.mu.Unlock()
	if a != nil {
		go ac.runConnectAttempt(a)
	}
	// deferred GracefulClose now runs after the unlock, as its comment claims.
```

Behavior is the same as the PR. The `Idle`/state check and the move to `Connecting` stay in one critical section on both paths, so the double-attempt race stays closed. After the split:
- lock acquire and release are back on the same goroutine;
- `updateAddrs` again releases `ac.mu` before its deferred `GracefulClose`;
- the lock is no longer held across a `go` statement;
- the unusual `...AndUnlock` contract is gone.

The locked half also becomes a `...Locked` helper that matches the repository's existing naming. The PR's first commit (`c3a3d1c4`, "Update conn state to prevent concurrent connection attempts") already set `Connecting` inside `connect()`. This proposal applies that same idea to both callers without duplicating the dial-parameter math.

**Status.** Verified by reading. The proposal is a design sketch and hasn't been compiled here, because the clone is read-only.

## Finding 2 — `updateAddrs`' deferred `GracefulClose` now runs while `ac.mu` is still held, and its justifying comment is stale

**Evidence.** Lines 983–988:

```go
	// We have to defer here because GracefulClose => onClose, which requires
	// locking ac.mu.
	if ac.transport != nil {
		defer ac.transport.GracefulClose()
		ac.transport = nil
	}
```

At `main`, `updateAddrs` ended with `ac.mu.Unlock(); go ac.resetTransport()`, so the deferred `GracefulClose` ran after the unlock. That ordering is the whole point of the comment. At `review-head`, `updateAddrs` returns with `ac.mu` still held, because the lock was handed to the spawned goroutine. The defer then runs `http2Client.GracefulClose`, which takes the old transport's `t.mu` and calls `t.onClose(GoAwayInvalid)` inline (`internal/transport/http2_client.go:1046, 1055`). `onClose` (lines 1351–1353) calls `ac.mu.Lock()`. So the goroutine that called `UpdateAddresses` now blocks while holding `t.mu`, until the new goroutine is scheduled and reaches line 1237 or 1262.

**Deadlock analysis.** I found no deadlock. While the spawned goroutine holds `ac.mu`, it only reads `ac` fields, calls `ac.dopts.bs.Backoff` / `minConnectTimeout`, and calls `updateConnectivityState`. That call only does `acbw.ccb.serializer.Schedule(...)`, which doesn't block (`balancer_wrapper.go:255–264`). It never touches the old transport's `t.mu`. The code works, but only because of that non-local argument. The comment still states an invariant ("defer so that GracefulClose runs without ac.mu") that the code no longer upholds. Anyone who later adds work under `ac.mu` at the top of `resetTransportAndUnlock` that touches the old transport or waits on the balancer serializer would bring back a lock-order deadlock (`t.mu` → `ac.mu` on one side, `ac.mu` → … on the other), and the stale comment gives them no warning.

**Remedy.** Finding 1's split fixes this for free: `updateAddrs` unlocks before returning, so the defer runs unlocked again as documented. If the handoff shape is kept anyway, the comment at lines 983–984 has to be rewritten to explain that `GracefulClose` blocks until the spawned goroutine releases `ac.mu`. That is a new synchronization dependency, and it should be written down rather than left implicit.

**Status.** Verified by reading both call chains.

## Finding 3 — The doc comment's "unconditionally connects" contradicts the early return

**Evidence.** Line 1231 says `resetTransportAndUnlock unconditionally connects the addrConn.` Lines 1235–1239 return without connecting when `ac.ctx` is already canceled. That can happen while `ac.state` isn't `Shutdown`, for example after the parent `ClientConn` context is canceled but before `tearDown` runs. The comment tells callers they don't need to think about that case, but they do. With the current handoff design, the caller can't tell afterwards whether a connection attempt started.

**Remedy.** Say what the function actually does: "starts a connection attempt unless ac.ctx is canceled". Or adopt Finding 1, where a nil `*connectAttempt` makes this outcome explicit in the type.

**Status.** Verified by reading.

## Observation outside the diff (question, not a finding)

When `updateAddrs` receives an empty address list while `Connecting` or `Ready`, it sets `Idle` (line 991) and then starts an attempt anyway (line 996). `tryAllAddrs` over zero addresses returns `firstConnErr == nil` (line 1341). `resetTransportAndUnlock` treats that as success (lines 1300–1303) and leaves the subchannel in `Connecting` with no attempt running. This was true before the PR too, so it's out of scope. The PR does touch this exact tail, though, so it's worth asking whether the empty-address case should skip the attempt.
