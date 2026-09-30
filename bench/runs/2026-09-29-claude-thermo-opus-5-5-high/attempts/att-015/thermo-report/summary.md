# Thermo-nuclear code quality review — grpc/grpc-go#7390

Range: `daab5634..76ef33f4` (`git diff main...review-head`). One file, `clientconn.go`, +6/−7.
Detail file: [`01_addrconn_connect_locking.md`](01_addrconn_connect_locking.md).

## Verdict

The fix is correct and worth having. `addrConn.connect()` used to drop `ac.mu` between its `Idle` check and the move to `Connecting`, so two connection attempts could run at once and orphan a transport. The PR closes that window. `go vet .` is clean, and `Test/AuthorityRevive` passed 200 of 200 runs under `-race` here.

The structure the fix uses is weaker than it needs to be. It hands ownership of `ac.mu` from the caller to `resetTransportAndUnlock`, and in `updateAddrs` that means handing it to a newly spawned goroutine. That is the only take-the-caller's-lock-and-release-it helper in the repository. It leaves a stale lock-ordering comment in `updateAddrs`. It also hides a simple fix ("do the state transition in the same critical section as the check") behind lock handoff. A split at the function's natural seam would keep behavior identical, keep every lock on the goroutine that took it, and remove the special contract. I recommend merging the bug fix, but I'd push for the restructure below rather than accepting the handoff shape as the long-term design.

File size isn't a concern for this PR. `clientconn.go` was already 1839 lines and the PR shrinks it by one.

## Findings

**1. Mutex ownership now crosses goroutines; split the locked prologue from the unlocked attempt instead (`clientconn.go:922`, `clientconn.go:996`, `clientconn.go:1231-1262`).**
In `updateAddrs`, `ac.mu` is locked at line 948 and released only inside `go ac.resetTransportAndUnlock()`, on a different goroutine, whenever the scheduler runs it. Go allows this, but lock balance can no longer be checked by reading one function. The mutex is also documented as sitting on the RPC hot path and meant to be held as briefly as possible, and it's now held across a `go` statement. `resetTransportAndUnlock` mixes a caller-owned-lock prologue with the file's usual lock-locally body. The `...AndUnlock` name breaks the repository-wide `...Locked` convention, where a helper returns with the lock still held. The real invariant is small: the `Idle` check and the move to `Connecting` must share one critical section. The code-judo move is to split the function at line 1262. A `startConnectingLocked()` would check `ac.ctx`, snapshot addrs, backoff and deadline, move to `Connecting`, and return a `*connectAttempt`. A `runConnectAttempt(a)` would run today's unlocked tail unchanged. `connect()` becomes `a := ac.startConnectingLocked(); ac.mu.Unlock(); if a != nil { ac.runConnectAttempt(a) }`. `updateAddrs` does the same with `go`. The race stays closed, every unlock is back on the goroutine that locked, and the special contract goes away. The worked sketch is in the detail file.

**2. `updateAddrs`' deferred `GracefulClose` now runs while `ac.mu` is still held, and its justifying comment is stale (`clientconn.go:983-988`).**
The comment says the `GracefulClose` is deferred because it calls `onClose`, which locks `ac.mu`. At the merge-base that worked, because `updateAddrs` unlocked before returning. Now `updateAddrs` returns with the lock handed to the spawned goroutine. The deferred `http2Client.GracefulClose` takes the old transport's `t.mu` and calls `onClose` inline (`internal/transport/http2_client.go:1046,1055`), which then blocks on `ac.mu` until the new goroutine reaches its unlock. I traced the spawned goroutine's locked section, and it only calls the non-blocking `serializer.Schedule`, so there's no deadlock today. Correctness now depends on that non-local fact, and the comment claims an ordering the code no longer provides. Adopting Finding 1 restores the documented ordering for free. If the handoff stays, the comment has to be rewritten to describe the new wait-on-another-goroutine dependency.

**3. The doc comment's "unconditionally connects" contradicts the early return (`clientconn.go:1231-1239`).**
The new comment says the function "unconditionally connects the addrConn". It actually returns without connecting when `ac.ctx` is already canceled, and that can happen before the state reaches `Shutdown`. The comment should describe the real contract ("starts a connection attempt unless ac.ctx is canceled"). Finding 1's nil `*connectAttempt` would make that outcome explicit in the type.

## Questions

**Q1. Should `updateAddrs` skip the connection attempt when the new address list is empty (`clientconn.go:990-996`)?**
With zero addresses it sets `Idle` and then starts an attempt anyway. `tryAllAddrs` returns a nil error for an empty list, so the attempt counts as a success and the subchannel is left in `Connecting` with nothing running. This predates the PR and is not a finding against it, but the PR edits exactly this tail.

## Proposed remediation sequence

1. Split `resetTransportAndUnlock` into `startConnectingLocked` (locked, returns `*connectAttempt` or nil) and `runConnectAttempt` (unlocked, today's lines 1264–1303), as sketched in the detail file (Finding 1).
2. Make `connect()` and `updateAddrs()` each unlock on their own goroutine before running or spawning the attempt. This puts the deferred `GracefulClose` back after the unlock, which resolves Finding 2 without editing the comment.
3. Give the locked helper a doc comment that states the nil-on-canceled-context outcome, which resolves Finding 3.
4. Re-run `Test/AuthorityRevive` under stress with `-race`, and run the `test/` subchannel/pickfirst suites, to confirm the double-attempt race stays closed.
5. Separately, decide on Q1 (the empty-address `updateAddrs` path).

## Verification status

All three findings were verified by reading the code at `review-head` and the transport and balancer-wrapper callees. The remediation sketch is a design proposal and hasn't been compiled here, because the clone is read-only. Evidence, commands and the full worked proposal are in [`01_addrconn_connect_locking.md`](01_addrconn_connect_locking.md).
