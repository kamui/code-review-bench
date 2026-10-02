# Admission and shutdown

## Scope and verdict

Reviewed the complete `lib/batcher/batcher.go`, its options model, atexit
registration and unregistration, the logger's object formatting, and the
Dropbox and Google Photos call sites relevant to admission and shutdown.
No actionable production finding was identified. The change makes the existing
queue protocol atomic without expanding the protocol's state model.

## Measurements and evidence

`git diff main...review-head` shows six added production lines and no deletions.
`wc -l lib/batcher/*.go` reports 288 lines in `batcher.go`, 364 in
`batcher_test.go`, and 74 in `options.go`. Reading the base blobs with
`git show main:lib/batcher/batcher.go | wc -l` and the equivalent test-file
command gives 282 and 275 lines respectively. No file crosses 1,000 lines.

`Batcher.admitMu` at line 50 names the transaction it owns. `Commit` acquires it
at line 267, checks admission at lines 268–273, enqueues at lines 276–280, and
unlocks at line 281. Its rejection arm unlocks at line 270. There are two
explicit unlock paths, both in a single short method; no unrelated conditional
or new execution mode was inserted.

`Shutdown` acquires the same mutex at line 243, closes admission at line 245,
sends the quit request at line 251, and unlocks at line 252. The once guard
already owns the one-time shutdown lifecycle. The new mutex owns ordering with
producers, which is a distinct responsibility.

## Ordering proof

If Commit acquires the lock first, its closed check and input send complete
before shutdown can close admission or send its marker. Since the two sends
are serialized, the channel presents the accepted request before the marker.
The consumer processes it either as a full batch or as the final pending batch.

If Shutdown acquires the lock first, it closes admission and sends the marker
before releasing the lock. The next Commit observes the closed channel and
returns the existing `fserrors.FatalError(errors.New("batcher is shutting down"))`.
It never sends a request behind the marker.

Concurrent shutdown callers remain serialized by `shutOnce`. The `Batching()`
guard continues to bypass shutdown for the off mode. No new off-mode behavior
is introduced, and Commit's documented requirement to enable batching remains.

## Progress and ownership

The input channel is buffered to the batch size at line 107. Holding admitMu
across a potentially blocked send is necessary to order it before the marker.
The receiver at lines 188–231 does not acquire admitMu and can free capacity.
Backend batch completion can still delay channel progress, as it could before
this patch; there is no new consumer dependency on the admission lock.

The synchronous response wait is at line 286, after admission unlocks. The
worker wait in Shutdown is at line 253, also after unlocking. Moving either wait
inside the lock would unnecessarily retain admission ownership, but this patch
does neither. Atexit unregistration finishes before locking admitMu, and the
consumer does not acquire the atexit registration mutex.

The existing debug log now executes while admission is locked. Formatting can
call the logging identity's String method: `fs/log.go:150–153` explicitly uses
`fmt.Sprint` for stringers. The actual Dropbox and Google Photos identities
simply format their roots (`backend/dropbox/dropbox.go:439–441` and
`backend/googlephotos/googlephotos.go:314–316`). No concrete reentrant lock
cycle was found in these consumers. A general logging redesign is not required
on a hypothetical callback assumption. The test intentionally uses this
formatting point as its pause; its ordering defect is covered separately.

The mutex remains private to the canonical batching package. Dropbox and
Google Photos continue to call the same Commit and Shutdown APIs. No backend
policy, cast, option, or feature condition is introduced into shared code.

## Worked code-judo alternatives

One possible reframe would close the work stream and let the receiver detect
end of input. Its key operations would be:

```go
// Fields: in, admitMu, shuttingDown bool; no request.quit or closed channel.
// Commit, under admitMu:
if b.shuttingDown {
    // Unlock and return the existing fatal shutdown error.
}
b.in <- req
// Unlock before waiting for the response.

// Shutdown, inside shutOnce and under admitMu:
b.shuttingDown = true
close(b.in)
// Unlock before waiting for the worker.

// Consumer's channel case, retaining the timer case and final flush:
case req, ok := <-b.in:
    if !ok {
        break outer
    }
    // Existing batching work follows.
```

With send and close serialized, the old send-on-closed-channel objection would
be resolved. This would remove the explicit marker and its request field.
However, it still needs an admission state, a lock, and a receiver termination
branch. It changes construction, request shape, shutdown, admission, receiver
handling, and tests. It is a viable alternative protocol, not a dramatic
reduction of this six-line fix's conceptual complexity. No rewrite is requested.

A narrower alternative is an `admit(req) error` helper that locks, defers its
unlock, checks closure, and sends. This can reduce manual-unlock repetition, but
would introduce a new helper to serve one caller and would still require the
shutdown transaction. Deferring unlock in the existing Commit method without
extracting admission would hold the mutex across the synchronous response
wait. The current small, explicit critical section is preferable here.

Using an RWMutex would let producers share admission ownership while shutdown
takes exclusive ownership. Nothing in the inspected evidence establishes that
producer lock contention is a material bottleneck; an additional lock mode is
not justified as a maintainability remedy.

## Verification status

The prescribed offline environment was used for all Go checks:

```sh
GOMODCACHE=/home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high/att-025/clone-cache/gomodcache
GOCACHE=/home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high/att-025/clone-cache/gocache
GOFLAGS=-mod=mod
GOPROXY=off
GOTOOLCHAIN=local
```

These variables were set on each command invocation, from the clone root:

```sh
timeout 300s go test -count=1 -race ./lib/batcher
timeout 300s go vet ./lib/batcher
git diff --check main...review-head
```

The race-enabled package test exited zero and reported
`ok github.com/rclone/rclone/lib/batcher 5.347s`. Vet and diff checking exited
zero without diagnostics. These establish focused verification, not backend
integration or whole-repository build verification.

The initially clean checkout remained clean after execution. HEAD remained
`0b87bc4788cd50664b82dd7e74c78aa2c7fc3b1c`; its tracked tree remained
`513ed2d594fb1979990d396fdd4253d6867ea9b8`. The changed-file index blobs remained
`ace41f9524abf2a4438b396afcdf7fb7275e11fa` for batcher.go and
`72873f12ca8fa0f1fa214d49fcf00b8b1b92b22c` for batcher_test.go. Worktree and
index diffs were empty. All scratch sources and overlays live outside the clone.
