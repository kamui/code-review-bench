# Code review — grpc/grpc-go#7390

"grpc: Hold mutex while calling resetTransport to prevent concurrent connection attempts"
Range: daab56344e612097fd50c46c433de5d9b6013837..76ef33f44a600c3ed1a385979fd1dfbcade3fbb6 (`main...review-head`)

## Result

No issues found. Checked for bugs and CLAUDE.md compliance.

Step 1 did not stop the review: the PR is not a draft, and it is a non-trivial concurrency change. Step 6 kept no issue. Step 4's review agents reported no issues, so step 5 had nothing to validate.

No actionable findings.

## What was checked

- CLAUDE.md compliance: the repository has no CLAUDE.md, either at the root or in the directory of the changed file `clientconn.go`, so there are no rules to apply.
- Bugs in `clientconn.go`:
  - `connect()` and `updateAddrs()` now pass the held `ac.mu` to `resetTransportAndUnlock()`. Every path locks and unlocks exactly once.
  - The goroutine started in `updateAddrs()` is allowed to unlock a lock it did not take, because Go's `sync.Mutex` is not owned by a goroutine.
  - The deferred `GracefulClose()` in `updateAddrs()` may wait briefly on `ac.mu`, but it cannot deadlock.
  - No code still calls the old `resetTransport()`.
