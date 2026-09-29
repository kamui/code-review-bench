# Detail 01: clientconn.go lock handoff (findings F1 to F3)

## Change inventory

`git diff main...review-head` touches only `clientconn.go`:

- `connect()`: drops `ac.mu.Unlock()` after the Idle check; calls `ac.resetTransportAndUnlock()` with the lock held.
- `updateAddrs()`: drops the explicit `ac.mu.Unlock()` before the tail; `go ac.resetTransport()` becomes `go ac.resetTransportAndUnlock()`.
- `resetTransport` is renamed `resetTransportAndUnlock`, its leading `ac.mu.Lock()` is removed, and a three-line doc comment is added.

Only two call sites exist (`grep -n resetTransport *.go` gives lines 922, 996 and the definition at 1234). `connect()` itself is called only from `acBalancerWrapper.Connect` as `go acbw.ac.connect()` (`balancer_wrapper.go:276`). File length is 1838 lines, so the 1k-line rule is not triggered by this change.

## F1 evidence: the deferred GracefulClose now runs under the handed-off lock

Trace, all read-only:

1. `updateAddrs` takes `ac.mu`, cancels the old context, creates a new one, and registers `defer ac.transport.GracefulClose()` with `ac.transport = nil` (`clientconn.go` around lines 980 to 987). The comment says the defer is needed because `GracefulClose => onClose` requires `ac.mu`.
2. Before the PR, `ac.mu.Unlock()` ran explicitly before `go ac.resetTransport()`, so when the deferred `GracefulClose` ran at function return the lock was free.
3. After the PR, the function returns with `ac.mu` held; the new goroutine is responsible for the unlock. The deferred `GracefulClose` runs first, takes `t.mu`, and calls `t.onClose(GoAwayInvalid)` synchronously (`internal/transport/http2_client.go:1045 to 1055`). `onClose` (`clientconn.go:1351`) does `ac.mu.Lock()`, which blocks until the new goroutine calls `ac.mu.Unlock()` at the end of its locked prefix (after `updateConnectivityState(Connecting)`).
4. Progress argument: the goroutine's locked prefix reads `ac.ctx`, `ac.addrs`, `ac.dopts`, calls `updateConnectivityState`, whose only outward effect is `acbw.updateState`, which just schedules onto the balancer serializer (`balancer_wrapper.go:255`). It needs neither `t.mu` nor anything held by the `updateAddrs` caller, so no deadlock. When `onClose` finally runs, the old context is cancelled, so it returns early after `adjustParams`.

Consequences: (a) the balancer's `UpdateAddresses` call now waits on goroutine scheduling; (b) the comment justifying the defer is stale; (c) lock/unlock are in different goroutines, which no linter or `-race` run will flag as misuse unless the unlock is missed at runtime. The reviewer discussion noted that unlocking an unlocked mutex panics, which covers the "callee unlocks twice or without lock" case but not "caller forgot that the callee owns the unlock" (double-unlock by a later edit of the caller), which would panic only on that path.

I did not run any test to confirm timing; this is a source-reading conclusion.

## F2: worked code-judo proposal

Current shape:

```go
func (ac *addrConn) resetTransportAndUnlock() {
    acCtx := ac.ctx
    if acCtx.Err() != nil { ac.mu.Unlock(); return }
    addrs := ac.addrs
    backoffFor := ...
    dialDuration := ...
    connectDeadline := time.Now().Add(dialDuration)
    ac.updateConnectivityState(connectivity.Connecting, nil)
    ac.mu.Unlock()
    if err := ac.tryAllAddrs(acCtx, addrs, connectDeadline); err != nil { ... backoff ... }
    ...
}
```

Proposed shape (behavior identical, sketch only, not applied):

```go
// startConnectingLocked transitions the addrConn to CONNECTING and returns the
// parameters for one connection attempt. ac.mu must be held.
func (ac *addrConn) startConnectingLocked() (a connectAttempt, ok bool) {
    if ac.ctx.Err() != nil { return connectAttempt{}, false }
    ... compute dialDuration, connectDeadline, backoffFor ...
    ac.updateConnectivityState(connectivity.Connecting, nil)
    return connectAttempt{ctx: ac.ctx, addrs: ac.addrs, deadline: ..., backoffFor: ...}, true
}

// runAttempt dials and handles backoff. ac.mu must NOT be held.
func (ac *addrConn) runAttempt(a connectAttempt) { /* existing body from tryAllAddrs on */ }
```

`connect()`:

```go
ac.mu.Lock()
... shutdown / non-idle checks with Unlock + return ...
a, ok := ac.startConnectingLocked()
ac.mu.Unlock()
if ok { ac.runAttempt(a) }
return nil
```

`updateAddrs` keeps its existing explicit `ac.mu.Unlock()` and does `if ok { go ac.runAttempt(a) }`, so the deferred `GracefulClose` runs with the lock free, exactly as before the PR. Every `Lock` is paired with an `Unlock` in the same function. The single-transition atomicity that the PR wants is owned by one small function with a name that says what it does. If a small struct is thought heavy, returning `(ctx, addrs, deadline, backoffFor, ok)` also works. This is also the natural home for the `Locked`-suffix name (F3).

Tradeoff to state honestly: the split adds one type or a multi-value return, and it touches more lines than the PR did. In exchange it removes a cross-goroutine lock transfer, which is not something the current call sites can keep safe automatically.

## F3 evidence

- `grep` for functions named `*Locked` finds several in the tree (`internal/idle/idle.go:86`, `server.go:1931/1940/1952`, `internal/transport/http2_server.go:1006`, `orca/producer.go:156`, `controlbuf.go:422`), all meaning "caller holds the lock, function leaves it held". No other `AndUnlock` function exists.
- The comment "unconditionally connects the addrConn" sits above a function whose first branch is `if acCtx.Err() != nil { ac.mu.Unlock(); return }`. It also states "ac.mu must be held by the caller" without noting the `updateAddrs` case in which the holder is not the goroutine that runs the function.
