# Connection lifecycle and lock handoff

## Scope and evidence

The reviewed range is `main...review-head` (`daab56344e612097fd50c46c433de5d9b6013837..76ef33f44a600c3ed1a385979fd1dfbcade3fbb6`). It changes only `clientconn.go` (+6/−7). The change replaces the lock-release/reacquire gap between `addrConn.connect`'s idle check and `resetTransport`'s `Connecting` transition. `connect` now calls `resetTransportAndUnlock` while holding `ac.mu`; `updateAddrs` likewise starts that helper in a goroutine before relinquishing the mutex. The helper's doc comment states that callers must hold the mutex and that the helper releases it.

The two uses are consistent with the contract. In `connect`, both early exits unlock directly and the eligible idle path transfers lock release to the helper. In `updateAddrs`, address mutation, context replacement, transport detachment, and any empty-address state update happen under the mutex; the helper then observes the updated context and address list before it sets `Connecting` and unlocks. The previous gap allowed another `connect` call to observe `Idle` and begin an overlapping attempt.

`updateAddrs` still defers `GracefulClose` until function return. The new goroutine may be responsible for releasing `ac.mu` before the close callback can acquire it. This can make the deferred close wait briefly for the helper's setup, but it preserves progress: the helper checks cancellation and unlocks on both its early return and normal path before beginning address attempts. The callback captured by the old transport also sees the canceled context and returns without mutating the replacement connection state. The coupling is visible from the `AndUnlock` helper name and its contract; replacing it with a broader refactor would add intermediate state or split the transition without removing meaningful complexity.

## Code-judo assessment

A more elaborate split could separate the locked transition/captured connection inputs from the later dial work, letting `updateAddrs` explicitly close the old transport between those phases. That introduces another representation of a pending attempt and a second-stage API solely to preserve an ordering that the current lock-transfer helper already expresses. No simpler restructuring was evident that both preserves the atomic idle-to-connecting transition and reduces the number of concepts. The current change is the smaller direct expression of the required invariant.

The PR adds no conditionals, does not duplicate transport-selection logic, and adds no file-size growth beyond six lines net. `clientconn.go` is already over 1,000 lines at both range endpoints, so the diff does not cross the skill's file-size threshold.

## Verification status

The following read-only inspection commands were used from the clone root:

- `git diff --stat main...review-head` and `git diff main...review-head -- clientconn.go` to establish the changed surface.
- `rg -n "resetTransport(AndUnlock)?\\(" clientconn.go` and surrounding source reads to verify all helper call sites and the transition behavior.
- `git diff --check main...review-head`, which passed.
- `wc -l clientconn.go`, which reported 1,838 lines at the reviewed head.

Focused verification passed with the policy-prescribed offline environment:

`GOMODCACHE=<cache>/gomodcache GOCACHE=<cache>/gocache GOFLAGS=-mod=mod GOPROXY=off GOTOOLCHAIN=local go test .`

Result: `ok google.golang.org/grpc 10.918s`. The full grpc-go suite and the originating xDS flake stress run were outside this verification; no finding depends on claiming those passed.

## Finding status

No actionable code-quality finding. No implementation change is requested.
