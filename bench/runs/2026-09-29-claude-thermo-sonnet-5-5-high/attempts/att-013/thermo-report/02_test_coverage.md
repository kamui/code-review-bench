# Detail 02: test coverage (finding F4)

## What the PR ships

The diff contains no test files. The PR body states the fix was verified by running `Test/AuthorityRevive` (xDS client tests, `xds/internal/xdsclient/tests`) 100000 times without failure, against a baseline of 399 failures in 100000 (about 0.4 percent) reported in the issue thread. Codecov reported no uncovered modified lines, which only says the changed lines are executed by existing tests.

## Why this is not enough

The race is in the `grpc` package: two goroutines call `addrConn.connect()` on an `Idle` addrConn; both observe `Idle` because the first released `ac.mu` before `resetTransport` set `Connecting`. The invariant that should be pinned is local and cheap to state: concurrent `connect()` calls produce exactly one connection attempt.

## Suggested test (not written; the clone must stay unchanged)

In package `grpc` (internal test), create a `ClientConn` with a custom dialer that increments a counter and blocks on a channel, obtain an idle `addrConn` (or use a channel with `pick_first` and `Connect()` invoked twice through `SubConn`), release N goroutines simultaneously into `ac.connect()`, wait until the dialer count stabilises, assert it equals 1, then unblock. Repeat in a loop with `-race`. With the pre-fix code the window between the `Unlock` and the re-`Lock` in `resetTransport` is wide enough that this fails much more often than 0.4 percent, which makes it a real guard.

A second case should cover `updateAddrs` racing with `connect()` from the `go` spawned in `updateAddrs`, since that is the path where lock ownership is handed to another goroutine (F1).

Verification status: analysis by reading only; no test was run or created.
