# Thermo-nuclear code quality review — `grpc/grpc-go#7390`

"grpc: Hold mutex while calling resetTransport to prevent concurrent
connection attempts". Review range `daab5634..76ef33f4`
(`git diff main...review-head`): one file, `clientconn.go`, +6/−7. Line
numbers below refer to the PR head.

Full evidence, commands, measurements and the worked proposal are in
[`01_addrconn_connect_locking.md`](01_addrconn_connect_locking.md).

## Verdict

**Request changes.** The fix is correct for the bug it targets, and the
distance to an approvable version is about twenty lines, but as written it
does not clear this review's bar.

The race is real and the PR removes it. In a scratch stress test, eight
concurrent `connect()` calls on an idle subchannel dialled more than once in
2 of 1500 iterations on the merge-base and in 0 of 1500 on the PR head. In
`connect()` the change is exactly right: the idle check and the move to
`Connecting` are now one critical section.

The problem is how the same mechanism is applied to the second call site.
`updateAddrs` now returns with `ac.mu` still locked and leaves the unlock to
a goroutine that has not started yet. That is a structural regression in a
file that otherwise unlocks before it hands off, and it falsifies a comment
a few lines above. There is also a visible restructuring that keeps the fix
and deletes the lock-transfer contract altogether, which the PR does not
take.

There are two findings and one question. Nothing here is a crash, hang or
user-visible failure; I looked for a deadlock and did not find one.

## Findings

### Finding 1 — `updateAddrs` now returns with `ac.mu` locked, and its deferred `GracefulClose` runs under that lock

`clientconn.go:983-996`, in `addrConn.updateAddrs`. The PR deletes the
`ac.mu.Unlock()` that used to come before the goroutine launch and turns
`go ac.resetTransport()` into `go ac.resetTransportAndUnlock()`. So
`updateAddrs` now returns with the mutex held, and the only code that will
release it is a goroutine that has been created but not yet scheduled. The
first consequence is that the comment at lines 983-984 is no longer true. It
says the `GracefulClose` call is deferred "because GracefulClose => onClose,
which requires locking ac.mu", and deferring it was how it got to run after
the unlock. With the unlock gone, the deferred call runs while the mutex is
still held: `GracefulClose` takes the old transport's `t.mu`
(`internal/transport/http2_client.go:1046`), calls `onClose` inline (line
1055), and parks on `ac.mu.Lock()` at `clientconn.go:1352` until the new
goroutine is scheduled and reaches its unlock. I confirmed this with a block
profile: on the PR head the stack `sync.(*Mutex).Lock` ←
`createTransport.func1` (`clientconn.go:1352`) ← `updateAddrs.deferwrap1`
(`clientconn.go:986`) ← `updateAddrs` is recorded, and on the merge-base it
is not. The second consequence is that this is not a deadlock only because of
an invariant nobody wrote down: nothing in lines 1235-1262 may block or touch
the old transport. The `defer` that relies on that is 250 lines away from the
code that has to uphold it, and neither end points at the other. The third is
that `ac.mu` is the mutex whose own comment (lines 1181-1184) asks for it to
be held as briefly as possible because `getReadyTransport` takes it on the
RPC path (`picker_wrapper.go:183`); one of its holds now lasts for a
goroutine start, so its length is set by the scheduler instead of by the
code. The author raised this risk in the review thread ("ac.updateAddrs calls
`resetTransport` in a new go routine so it can't hold the lock till
`resetTransport` completes. It feels a little risky to make that change") and
the final revision does it regardless. Holding the lock through the
transition does buy something: no other goroutine can see `Ready` with a nil
transport, or the `Idle` published at line 991. The remedy keeps that. Do the
locked work on the caller's goroutine, unlock in `updateAddrs` as before, and
give the new goroutine only the part that runs unlocked. Finding 2 shows the
change, which ends `updateAddrs` with
`attempt := ac.startConnectingLocked(); ac.mu.Unlock(); go attempt()` and
makes the `defer` comment true again.

### Finding 2 — missed code-judo: `resetTransportAndUnlock` is two functions joined at an `Unlock`; splitting there deletes the lock-transfer contract

`clientconn.go:1231-1304`, with its callers at lines 922 and 996. The
function has a seam at the `ac.mu.Unlock()` on line 1262. Lines 1235-1261 are
a short, non-blocking step that must run under the lock: snapshot `ac.ctx`
and `ac.addrs`, compute the backoff and the dial deadline, set the state to
`Connecting`. Lines 1264-1303 are a long, blocking step that must not run
under the lock: dial every address and, on failure, wait out the backoff,
with four `Lock`/`Unlock` pairs of its own. The race only needs the first
step to share a critical section with the caller's state check. The PR gets
there by having the whole function start locked and stop being locked
part-way through. That is why it needs a name that describes a locking side
effect instead of the function's job, a doc comment to explain the hand-off,
and a review thread about whether to assert that the lock is held. The doc
comment is also inaccurate: "unconditionally connects" is contradicted by the
early return on line 1236 when `ac.ctx` is done, and "will guarantee it is
released" reads as "released on return" when the lock is in fact dropped
early and re-taken, and the call blocks for the whole dial and backoff. I
think there is a code-judo move here that removes the contract instead of
documenting it: make the seam a function boundary. A
`startConnectingLocked()` does the first step under the caller's lock,
following the file's existing `…Locked` convention, and returns the attempt
as a `func()`; a plain
`resetTransport(acCtx, addrs, connectDeadline, backoffFor)` does the second
and takes no lock on entry. Every `Lock` then has its `Unlock` in the same
function, the mutex never crosses a goroutine, the two call sites differ only
in `attempt()` versus `go attempt()`, and the `…AndUnlock` name, its doc
comment and Finding 1 all disappear. I built this as an overlay, 19 lines
added and 8 removed against the PR head, and ran it: the root package tests,
`go vet`, a focused slice of `./test` and 50 runs of the originally flaky
`Test/AuthorityRevive` pass, the lock probes from Finding 1 come back clean,
and the stress test shows 0 double-dials in 1500 iterations, the same as the
PR head. The full diff is in section 4.2 of the detail file.

## Question for the author

### Question 1 — is there a regression test that can pin this invariant?

The PR adds no test, and the evidence in its description is a 100000-run loop
of an xDS test in another package. The invariant being restored, at most one
connection attempt per `addrConn` at a time, is enforced only by which lines
sit between a `Lock` and an `Unlock` in two functions. I could reach the race
from a small test in package `grpc` (a balancer that leaves its SubConn idle,
then eight concurrent `connect()` calls against a listener that counts
accepts), but only at 2 hits in 1500 iterations on the merge-base, and an
attempt to widen the window by queuing the callers on `ac.mu` did not
reproduce it at all (0 of 200). So I do not have a reliable test to propose
and am not raising this as a finding. Do you see a way to make it
deterministic, or would the maintainers accept a looped stress test in
`clientconn_test.go` as the guard?

## Proposed remediation sequence

1. Split `resetTransportAndUnlock` at line 1262 into
   `startConnectingLocked()` and an unlocked `resetTransport(...)`, as in
   section 4.2 of the detail file. This is the whole of Finding 2.
2. In `connect()` and `updateAddrs`, call `startConnectingLocked()` under the
   lock, unlock in the same function, then run the returned attempt
   (`attempt()` and `go attempt()` respectively). This resolves Finding 1 and
   restores the meaning of the `defer` comment at lines 983-984 without
   editing it.
3. Drop the `resetTransportAndUnlock` doc comment along with the function;
   the replacement comments in the proposal state what each half does and
   that the attempt blocks.
4. Decide Question 1. If a looped test is acceptable, the scratch test at
   `clone-work/scratch/stress/zz_stress2_test.go` is a starting point.

Steps 1 to 3 are one small commit and do not change `connect()`'s behaviour
at all. For `updateAddrs`, the only difference from the PR head is that the
`Connecting` state is published on the caller's goroutine before the function
returns, instead of on the spawned goroutine shortly after.

## Checked and not flagged

The `…AndUnlock` name has precedent in the same file,
`updateResolverStateAndUnlock` at `clientconn.go:728`, but all three of its
callers invoke it synchronously on the goroutine that took the lock, so it
supports the `connect()` call site and not `go …AndUnlock()`. The file-size
rule is not engaged: `clientconn.go` goes from 1839 to 1838 lines and was
already well past 1000. No path through the new function leaves the mutex
locked. I found no place where `ac.mu` is held while a transport's `t.mu` is
taken, which is the reverse of the new `t.mu` → `ac.mu` wait and would be
needed for a deadlock.

## Verification status

Verified by experiment, using `go test -overlay` so that nothing was written
to the clone: the lock is carried out of `updateAddrs` and the deferred close
blocks on it on the PR head, and neither happens on the merge-base or with
the proposal; the double-dial race reproduces on the merge-base and not on
the PR head or the proposal; the proposal passes the existing tests listed
under Finding 2.

Verified by reading only: that the hand-off cannot deadlock today, and the
lock-ordering check above.

Not verified: no `-race` runs and not the full grpc-go suite. The
existing-test runs were made against the proposal only, not separately
against the unmodified PR head. An attempt to rerun `Test/AuthorityRevive`
4000 times on each tree ran out of time and produced no result. The stress
numbers come from one run per tree and are thin: 2 hits in 1500 on the
merge-base.

No cross-model or alternate-model review was run, and no part of this review
was delegated.
