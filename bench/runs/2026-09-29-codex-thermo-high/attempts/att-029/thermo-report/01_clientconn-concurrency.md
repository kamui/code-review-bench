# Client connection synchronization

## Scope and measurements

The reviewed change is `clientconn.go` in the pinned range `daab56344e612097fd50c46c433de5d9b6013837..76ef33f44a600c3ed1a385979fd1dfbcade3fbb6`. The patch changes six lines and removes seven. The file is 1,838 lines at the reviewed head, and was already above 1,000 lines at the base; this patch adds no lines net and does not create a new file-size regression.

The change removes the unlock/relock gap between `addrConn.connect` validating `Idle` and `resetTransport` updating the state to `Connecting`. It also changes `updateAddrs` to start the helper goroutine while the mutex is still held. Both paths now call `resetTransportAndUnlock`, whose comment states that the caller must hold `ac.mu` and that the helper releases it.

## Finding assessment

No actionable maintainability issue found. The helper has exactly two call sites in this file, and both acquire `ac.mu` before calling it. The helper releases the mutex before the potentially slow address dialing work. Its early context-canceled return also releases the mutex. This gives callers a single clear contract and keeps state setup and the transition into connection work in one path.

The goroutine call in `updateAddrs` deserves scrutiny because it transfers the responsibility to unlock a mutex across goroutines. Go's `sync.Mutex` does not require the locking and unlocking goroutines to be the same. More importantly for this code, the invocation is visibly adjacent to the lock-preserving state updates, and `resetTransportAndUnlock` documents the handoff. The previous pattern unlocked before starting the goroutine, leaving a window where another `connect` could also observe `Idle`; the patch removes that window.

## Code-judo alternative considered

One alternative is to change callers to set the state to `Connecting` under `ac.mu`, unlock, and then invoke a helper that acquires its own lock or begins dialing. That could avoid the cross-goroutine lock handoff. It would also split the operation's invariant across caller-specific setup and a separate connection-attempt function: `connect` and `updateAddrs` would each need to establish the right state before calling the new helper, while the helper would need a distinct contract for already-transitioned state. Given only these two call sites and the fact that state setup already lives in `resetTransport`, this is a larger protocol with more moving pieces, not a simplification.

Another option is to inline the state transition into `connect` and `updateAddrs`. That duplicates the transition logic and moves transport setup details into both callers. It is less cohesive than the current helper. The current name is somewhat mechanical, but accurately signals both the synchronization precondition and its postcondition; the doc comment makes the non-obvious behavior explicit.

## Evidence and commands

Commands used from the repository root:

- `git diff --stat main...review-head` and `git diff --find-renames --find-copies main...review-head -- clientconn.go` to inspect the exact patch.
- `rg -n "resetTransportAndUnlock|resetTransport\\(|resetBackoff" --glob '*.go' .` to enumerate references.
- `sed -n '880,1020p' clientconn.go` and `sed -n '1200,1310p' clientconn.go` to inspect callers, locking, and early-return paths.
- `wc -l clientconn.go` to measure file size.
- `git diff --check main...review-head`, which passed.

## Verification status

Static review only. No tests were run, so the report makes no runtime or race-detector claim. The patch's lock ownership and exit paths were inspected in source. No actionable finding remains.
