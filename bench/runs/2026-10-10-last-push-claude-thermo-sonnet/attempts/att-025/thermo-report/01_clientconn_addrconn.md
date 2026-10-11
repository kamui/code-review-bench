# Detail: clientconn.go — addrConn connection-attempt locking

Scope: `git diff main...review-head` (one file, `clientconn.go`, +6/−7). Commands used: `git diff main...review-head`, reads of `clientconn.go` lines 899–1000, 1195–1310, 1347–1440, 1545–1580, and `internal/transport/http2_client.go` `GracefulClose` (line 1045). No tests were run (read-only review, focused analysis only). File size: `clientconn.go` is 1838 lines both before and after (net −1), so the 1k-line rule is not triggered (it was already over).

## What the change does

`connect()` used to release `ac.mu` after the Idle check and then call `resetTransport()`, which re-took the lock and moved the state to Connecting. Two concurrent `connect()` callers could both observe Idle in that gap, so two attempts ran. The PR renames the function to `resetTransportAndUnlock`, drops the lock acquisition at its top, and has both callers (`connect`, `updateAddrs`) hand over a held lock. The race is closed because the Idle check and the transition to Connecting now happen in one critical section. This is a correct fix for the race in #7365.

## Finding 1 — updateAddrs hands a held mutex to a new goroutine (ownership transfer across goroutines)

Evidence: `clientconn.go:976-996`. `updateAddrs` takes `ac.mu`, mutates `ac.ctx`/`ac.transport`, registers `defer ac.transport.GracefulClose()` (line 984), and finishes with `go ac.resetTransportAndUnlock()`. The mutex is now unlocked by a different goroutine than the one that locked it.

Consequences, verified by reading the code:

- The comment at lines 982–983 ("We have to defer here because GracefulClose => onClose, which requires locking ac.mu") described a deferral that ran after an explicit `ac.mu.Unlock()`. That explicit unlock is gone. The deferred `GracefulClose` (`http2_client.go:1045-1055`) calls `t.onClose(...)` synchronously, and `onClose` (`createTransport`, line 1352) does `ac.mu.Lock()`. So the deferred call now blocks on a lock that is released only when the newly spawned goroutine gets scheduled and reaches its `Unlock`. There is no deadlock (every path in `resetTransportAndUnlock` unlocks before any blocking work), but correctness now rests on scheduler progress of a goroutine started a few lines earlier, and the comment no longer explains the real ordering.
- `updateAddrs` returns with the lock still logically held by someone else. Any reader of this function has to follow the lock into `resetTransportAndUnlock` to see where it is released, including the early return at line 1237 and the four later unlock sites.
- `Unlock` from a goroutine other than the locker is legal in Go, but nothing else in this file does it, and the race detector / vet tooling cannot validate the pairing.

Code-judo proposal: split the function by lock state rather than pass the lock around. A small `ac.startConnectAttemptLocked() (acCtx, addrs, backoffFor, connectDeadline)` runs under the lock and does the ctx check, computes the dial parameters, and sets Connecting. Both callers then `Unlock()` themselves (same goroutine as the `Lock()`), and either call `ac.runConnectAttempt(...)` inline (`connect`) or `go ac.runConnectAttempt(...)` (`updateAddrs`). The Idle/Connecting transition stays atomic with the validation, so the race fix is preserved. The `AndUnlock` convention (already used once for `cc.updateResolverStateAndUnlock`, but there for a short function) disappears for addrConn, and the original deferred-`GracefulClose`-after-unlock ordering and its comment stay true.

Verification status: confirmed by code reading; a runtime hang was not reproduced and is not claimed.

## Finding 2 — resetTransportAndUnlock has a lock-release contract enforced only by a comment, across six exit paths

Evidence: `clientconn.go:1231-1308`. The function now begins without the lock but must release it on the early-return path (line 1237), after setting Connecting (line ~1262), and then re-locks and unlocks four more times in the failure/backoff/success tails. A reviewer asked for runtime enforcement; the author answered that a double-unlock panics and the race detector covers misuse. That covers only the "forgot to lock" direction, and only when a test exercises it. The "forgot to unlock" direction (a future early return added above line 1262) produces a silent deadlock of the whole subchannel (`ac.mu` guards `getTransport`, `updateAddrs`, `tearDown`).

This is the cost of Finding 1's design: a ~75-line function that is half "caller's lock" and half "my own locking". The split proposed in Finding 1 removes the hazard by making the locked part a short straight-line function that contains no early exits other than the ctx check, and leaving the long backoff loop with plain self-contained `Lock/Unlock` pairs.

The `connect()` doc comment (line 903–905) is also now incomplete: it says only "starts creating a transport", but it now also performs the Idle→Connecting transition atomically with the check, which is the whole point of the fix. The invariant belongs there or in the new locked helper's comment.

Verification status: confirmed by code reading.

## Finding 3 — the race fix has no deterministic regression test

Evidence: the diff touches only `clientconn.go`. The PR description cites 100000 reruns of `Test/AuthorityRevive` (an xDS test whose pass/fail depends on 0.4% scheduler timing) as the validation. Nothing pins the new invariant "two concurrent `connect()` calls produce exactly one `tryAllAddrs` / one transport". A focused test in `clientconn_test.go` or `test/` would call `ac.connect()` from N goroutines against a listener that counts accepted connections (or use `internal/testutils` listener wrapper), and assert one accept. Without it, the fix can be undone by the refactor in Finding 1 or any later reshuffle of the lock and nobody will notice except through the same 0.4% flake in an unrelated xDS test.

Verification status: absence of a test confirmed from the diff (`git diff main...review-head --stat` shows one file).

## Things checked and not flagged

- File size: unchanged threshold situation (1838 lines before and after); no new decomposition obligation from this PR.
- No new conditionals or flags were added; the diff deletes code (net −1 line).
- The `acCtx.Err()` check at the top of `resetTransportAndUnlock` is not dead: `ac.ctx` derives from `cc.ctx`, which `cc.Close` cancels without first moving every addrConn out of Idle, so the check remains meaningful for `connect()`.
- Behavior of `connect()` with Shutdown/non-Idle state is unchanged.
