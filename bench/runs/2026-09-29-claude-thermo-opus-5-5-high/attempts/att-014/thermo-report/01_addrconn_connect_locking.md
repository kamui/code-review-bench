# 01 — addrConn connect / resetTransport locking (`clientconn.go`)

Review range: `daab56344e612097fd50c46c433de5d9b6013837..76ef33f44a600c3ed1a385979fd1dfbcade3fbb6`
(`git diff main...review-head`). One file changed, +6 / −7.

## What the diff does

Before the PR, both `addrConn.connect()` and `addrConn.updateAddrs()` validated
`ac.state` under `ac.mu`, released `ac.mu`, and then called `resetTransport()`,
which re-acquired `ac.mu` and moved the subchannel to `CONNECTING`. The gap between
the unlock and the re-lock let two `connect()` calls (the balancer wrapper runs
`go acbw.ac.connect()` at `balancer_wrapper.go:276`) both observe `IDLE` and both
start `tryAllAddrs`, orphaning one transport. That is the root cause stated in
issue #7365.

The PR renames `resetTransport` to `resetTransportAndUnlock`, drops the
`ac.mu.Lock()` at its top, and makes both callers hand over a held `ac.mu`:

- `clientconn.go:922` — `connect()` calls `ac.resetTransportAndUnlock()` synchronously.
- `clientconn.go:996` — `updateAddrs()` calls `go ac.resetTransportAndUnlock()`.
- `clientconn.go:1231-1234` — new doc comment: "unconditionally connects the
  addrConn. ac.mu must be held by the caller, and this function will guarantee it
  is released."

The fix is behaviourally correct for the race it targets: the `IDLE` check in
`connect()` and the `CONNECTING` transition in the callee now happen in one
critical section.

## Measurements and commands

| Check | Command | Result |
| --- | --- | --- |
| Diff | `git diff main...review-head` | 1 file, +6/−7 |
| File size | `git show main:clientconn.go \| wc -l` / `wc -l clientconn.go` | 1839 → 1838 (already over 1k before the PR; the PR does not cause it) |
| Callers of the renamed function | `git grep -n "resetTransport\|AndUnlock" -- '*.go'` | only `clientconn.go:922` and `:996`; the other hits are log strings in tests |
| Existing `*AndUnlock` precedent | same grep | `ClientConn.updateResolverStateAndUnlock` (`clientconn.go:728`), called synchronously from `resolver_wrapper.go` |
| Transport close path | `internal/transport/http2_client.go:1045-1064` | `GracefulClose` takes `t.mu` and calls `t.onClose(...)` **synchronously**; `onClose` (`clientconn.go:1351-1352`) does `ac.mu.Lock()` |
| State-listener dispatch | `balancer_wrapper.go` `acBalancerWrapper.updateState`, `internal/grpcsync/callback_serializer.go:64` | `serializer.Schedule` puts onto an unbounded buffer and does not block |
| Root package, race detector | `go test -race -count=1 .` (offline env from the policy) | `ok google.golang.org/grpc 13.941s` |
| Originally flaky test | `go test -count=300 -run 'Test/AuthorityRevive' ./xds/internal/xdsclient/tests/` | `ok … 37.602s` (300/300 pass; this is not enough runs to prove the ~0.4% flake is gone, but nothing regressed) |

The clone was left unchanged (`git status --porcelain` was empty after each run).

---

## Finding 1 — `updateAddrs` now passes a held mutex to a goroutine that does not exist yet, and this breaks the invariant its own `defer` relies on

**Severity:** structural regression / missed code-judo move. **Verification:** confirmed by
reading the code along the full call path; no deadlock found; the added latency and
lock coupling come from the code structure and were not measured.

`clientconn.go:996` is now `go ac.resetTransportAndUnlock()`. `updateAddrs` locks
`ac.mu`, and the unlock happens later in another goroutine, whenever the scheduler
starts it. Go permits this (a `sync.Mutex` is not bound to a goroutine), but it is
the least legible locking pattern in the file. Nothing at the lock site shows who
releases the lock, and the `*AndUnlock` convention's single precedent
(`updateResolverStateAndUnlock`) is always called synchronously. After this PR a
reader has to follow the release into a goroutine to know when it happens.

The handoff also silently invalidates code a few lines up in the same function:

```go
// We have to defer here because GracefulClose => onClose, which requires
// locking ac.mu.
if ac.transport != nil {
    defer ac.transport.GracefulClose()
    ac.transport = nil
}
...
go ac.resetTransportAndUnlock()
}
```

That `defer` exists only to run `GracefulClose` *after* `ac.mu` has been released.
Before the PR that held, because `ac.mu.Unlock()` ran before the function returned.
Now `ac.mu` is still held when the deferred `GracefulClose` runs, unless the new
goroutine has already been scheduled. `GracefulClose` takes the old transport's
`t.mu` and calls `onClose` synchronously (`http2_client.go:1055`), and `onClose`
calls `ac.mu.Lock()`. So the caller of `updateAddrs` blocks, holding the old
transport's `t.mu`, until the spawned goroutine runs, computes backoff and the
deadline, calls `updateConnectivityState(Connecting)`, and unlocks. The caller is
the LB policy, through `acBalancerWrapper.UpdateAddresses` (`balancer_wrapper.go:272`),
running on the balancer serializer goroutine. I found no deadlock: the spawned
goroutine only makes a non-blocking `serializer.Schedule` while it holds `ac.mu`.
But the comment is now false, the `defer` no longer does what it says, and
`ac.mu` stays held across a goroutine-scheduling gap. The struct comment at
`clientconn.go:1181-1184` asks for the opposite: "This mutex is used on the RPC
path, so its usage should be minimized as much as possible."

Deeper problem: `resetTransportAndUnlock` mixes two jobs. One is a short locked
**prelude**: snapshot `ac.ctx`, `ac.addrs`, and the backoff, compute the deadline,
move to `CONNECTING`. The other is a long unlocked **attempt loop**: `tryAllAddrs`,
backoff timer, and the transition to `TRANSIENT_FAILURE` and then `IDLE`. The race fix only
needs the prelude to run in the caller's critical section. Passing the whole function
the lock is the blunt way to get that. It is why the lock crosses a `go` statement,
why the function carries an ownership-transfer contract in its name and doc comment,
and why it has an early-return path whose only job is `ac.mu.Unlock()`.

### Worked code-judo proposal

Split the function where the lock boundary already is. The prelude becomes a
`...Locked` helper, following the file's existing convention. It returns the
immutable snapshot the attempt needs. The attempt loop takes that snapshot and
never inherits a lock.

```go
// connectAttempt is the state captured when an addrConn transitions to
// CONNECTING; it is consumed without holding ac.mu.
type connectAttempt struct {
	ctx        context.Context
	addrs      []resolver.Address
	backoffFor time.Duration
	deadline   time.Time
}

// startConnectingLocked moves ac to CONNECTING and snapshots what the attempt
// needs. It returns false if ac.ctx is already canceled. ac.mu must be held.
func (ac *addrConn) startConnectingLocked() (connectAttempt, bool) {
	if ac.ctx.Err() != nil {
		return connectAttempt{}, false
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
	return connectAttempt{ac.ctx, ac.addrs, backoffFor, time.Now().Add(dialDuration)}, true
}

// runConnectAttempt tries all addresses and handles failure/backoff.
// ac.mu must NOT be held.
func (ac *addrConn) runConnectAttempt(a connectAttempt) { /* body of today's
	resetTransportAndUnlock from tryAllAddrs onward, using a.ctx etc. */ }
```

Callers:

```go
// connect()
	...state checks...
	a, ok := ac.startConnectingLocked()
	ac.mu.Unlock()
	if ok {
		ac.runConnectAttempt(a)
	}
	return nil

// updateAddrs()
	...
	a, ok := ac.startConnectingLocked()
	ac.mu.Unlock()
	if ok {
		go ac.runConnectAttempt(a)
	}
}
```

Result:

- The `IDLE`→`CONNECTING` transition stays atomic with the caller's validation,
  so the #7365 fix is kept in both callers. (It matters in `updateAddrs` too. When
  `len(addrs) == 0` that function sets `IDLE` just before starting the attempt, so
  a concurrent `connect()` could otherwise see `IDLE` and start a second attempt.)
- Every `Lock` in the file is again paired with an `Unlock` in the same goroutine
  and function. No lock crosses a `go` statement.
- `ac.mu` is released before `updateAddrs` returns, so the "defer because
  GracefulClose ⇒ onClose locks ac.mu" comment is true again. It needs no edits.
- The ownership-transfer contract (`AndUnlock` suffix, "this function will
  guarantee it is released", the early-return unlock) goes away completely. The
  reviewer thread asked how to enforce the handoff, and this deletes the question
  instead of documenting it.
- `runConnectAttempt` becomes a function with a lock-free entry and exit. Today it
  is a 60-line function that starts locked and then unlocks and relocks five times.

This is roughly the same amount of code as the PR, split at the natural seam
rather than at the function boundary that happened to exist.

---

## Finding 2 — the new doc comment misstates the contract ("unconditionally connects")

**Severity:** legibility / contract. **Verification:** confirmed by reading
`clientconn.go:1231-1240`.

The comment added at `clientconn.go:1231` says `resetTransportAndUnlock`
"unconditionally connects the addrConn." The first thing the body does is return
without connecting when `ac.ctx.Err() != nil`. The comment actually means "this
does not re-check `ac.state`; the caller must have already validated that a
transition to CONNECTING is legal." That precondition is the one that matters
(the race was a missing state check), and the comment leaves it out. A future
caller reading it would learn only about the mutex and nothing about the state
precondition that the fix relies on.

**Remedy:** if Finding 1's split is adopted, put the precondition on
`startConnectingLocked`: "ac.mu must be held; the caller must have verified ac.state
permits a transition to CONNECTING (i.e. not SHUTDOWN, and IDLE for connect())."
Say that it returns false, and connects nothing, when `ac.ctx` is already canceled.
If the PR's shape is kept, at least replace "unconditionally connects" with that
precondition and mention the canceled-context early return.

---

## Non-findings checked

- **File size:** `clientconn.go` was 1839 lines before and is 1838 after. It is well
  over 1k, but this PR shrinks it, so the 1k rule does not apply here. Splitting
  the `addrConn` section (about lines 1166–1600) into its own file is still worth
  doing some day, as the existing `TODO(bar) Move this to the addrConn section`
  at `clientconn.go:904` suggests, but that is outside this PR's scope.
- **Deadlock risk from holding `ac.mu` longer:** `connect()`'s callee does the same
  work under the lock as before (the old `resetTransport` also held `ac.mu` for the
  prelude), and the only outward call under the lock is the non-blocking
  `serializer.Schedule`. No new deadlock found.
- **`acCtx.Err()` early return leaving state `IDLE`:** unchanged from before the PR,
  and reachable only when `cc.ctx` is canceled, because `updateAddrs` replaces
  `ac.ctx` under the same lock and `tearDown` sets `SHUTDOWN`, which `connect()`
  rejects. Not a regression.
