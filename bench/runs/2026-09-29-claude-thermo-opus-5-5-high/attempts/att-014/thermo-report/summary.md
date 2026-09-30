# Thermo-nuclear code quality review — grpc/grpc-go#7390

**PR:** "grpc: hold ac.mu while calling resetTransport to prevent concurrent connection attempts"
**Range:** `daab5634..76ef33f4` (`git diff main...review-head`), `clientconn.go` only, +6 / −7.
**Detail file:** [`01_addrconn_connect_locking.md`](01_addrconn_connect_locking.md)

## Verdict

**Request changes (structural, not behavioural).** The fix is correct for the
race in #7365. `connect()` now checks `IDLE` and moves to `CONNECTING` in one
critical section, so two concurrent `connect()` calls can no longer both start
`tryAllAddrs`. Root-package tests pass under `-race`, and `Test/AuthorityRevive`
passed 300 of 300 runs locally.

The fix works by handing a held `ac.mu` into `resetTransportAndUnlock`. In
`updateAddrs` that handoff crosses a `go` statement, so the mutex is released by a
goroutine that has not started yet. The race fix did not need that. It also makes
a nearby `defer` comment false, and it adds a lock-ownership-transfer contract that
reviewers then argued about how to enforce. There is a clear code-judo move: split
the function at its existing lock boundary. That keeps the fix and removes the
contract. The file-size rule does not apply: `clientconn.go` shrinks by one line,
from 1839 to 1838.

## Findings

### 1. `updateAddrs` hands a held mutex to a not-yet-running goroutine, which breaks its own `defer` invariant (structural / missed code-judo)

In `clientconn.go:996`, `updateAddrs` now ends with `go ac.resetTransportAndUnlock()`
while still holding `ac.mu`, so the lock is released later by another goroutine.
A few lines up (`clientconn.go:983-986`), the same function defers
`ac.transport.GracefulClose()`. Its comment says this is done "because GracefulClose
=> onClose, which requires locking ac.mu", which means the defer exists to run after
`ac.mu` is released. That is no longer guaranteed. `http2Client.GracefulClose`
takes the old transport's `t.mu` and calls `onClose` synchronously, and `onClose`
locks `ac.mu`. So the LB policy's goroutine (through
`acBalancerWrapper.UpdateAddresses`) now blocks, holding the old transport's
lock, until the spawned goroutine is scheduled and unlocks. I verified this by
reading the full call path. I found no deadlock, because the only outward call
under the lock is a non-blocking `serializer.Schedule`. Still, the comment is now
false, and the "RPC-path" mutex that the struct comment says to hold as briefly as
possible now stays locked across goroutine scheduling.

The underlying problem is that `resetTransportAndUnlock` mixes two jobs. One is a
short locked prelude: snapshot `ctx`, `addrs`, and backoff, compute the deadline,
move to `CONNECTING`. The other is a long unlocked attempt loop. The race fix only
needs the prelude inside the caller's critical section. **Remedy:** split it into
`startConnectingLocked() (connectAttempt, bool)`, which runs under the caller's
lock, and `runConnectAttempt(connectAttempt)`, which never holds the lock on entry.
Both callers then do `a, ok := ac.startConnectingLocked(); ac.mu.Unlock()`
followed by a direct or `go` call to `runConnectAttempt`. That keeps the atomic
`IDLE`→`CONNECTING` transition in both callers and makes every Lock/Unlock pair
local again. It also makes the `defer GracefulClose` comment true again, and it
removes the `AndUnlock` ownership contract and its unlock-only early return.
Full worked code is in the detail file.

### 2. The new doc comment misstates the contract ("unconditionally connects") (legibility / contract)

The comment added at `clientconn.go:1231` says `resetTransportAndUnlock`
"unconditionally connects the addrConn". The body returns without connecting when
`ac.ctx` is already canceled. The comment also leaves out the precondition the race
fix depends on: the caller must already have checked that `ac.state` allows a
transition to `CONNECTING`. **Remedy:** document that precondition and the
canceled-context early return. Ideally put them on the `startConnectingLocked`
helper from Finding 1.

## Proposed remediation sequence

1. Extract `startConnectingLocked` and `connectAttempt` from the prelude of
   `resetTransportAndUnlock`, and rename the rest to `runConnectAttempt`, entered
   with `ac.mu` not held (Finding 1).
2. Change `connect()` and `updateAddrs()` to call the locked helper, unlock
   locally, then run or `go` the attempt. Leave the `defer GracefulClose` block
   as is; its comment will be accurate again.
3. Write the precondition and early-return behaviour into the helper's doc comment
   (Finding 2).
4. Re-run `go test -race .` and a high-count `Test/AuthorityRevive` loop.

## Verification status

- Finding 1: confirmed by code reading along
  `updateAddrs` → deferred `GracefulClose` → `onClose` → `ac.mu.Lock()`. The extra
  blocking and lock coupling were not measured.
- Finding 2: confirmed by reading.
- Tests run (offline, policy environment): `go test -race -count=1 .` passed;
  `go test -count=300 -run 'Test/AuthorityRevive' ./xds/internal/xdsclient/tests/`
  passed.

## Non-findings

The file-size rule does not apply because the PR shrinks the file. No new deadlock
was found. The canceled-context early return that leaves the state `IDLE` is
unchanged from before the PR.
