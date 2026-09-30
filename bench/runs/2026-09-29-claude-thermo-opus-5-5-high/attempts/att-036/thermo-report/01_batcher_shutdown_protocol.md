# 01 — `lib/batcher/batcher.go`: admission / shutdown protocol

Scope: the six added lines in `lib/batcher/batcher.go` (the `admitMu` field, the lock/unlock pair in `Shutdown`, and the lock plus two unlocks in `Commit`), read against the whole file as it stands at `review-head` (288 lines, well clear of any size threshold).

## What the PR does

Before the PR, `Commit` checked `b.closed` and then sent on `b.in`. `Shutdown` could close `b.closed` and enqueue the `{quit: true}` sentinel between those two steps, so a request could land behind the sentinel. `commitLoop` stops at the sentinel, which means that request was never read. In sync mode the caller then blocks forever. In async mode the upload is silently dropped.

The PR adds `admitMu sync.Mutex`. `Commit` holds it from the closed-check through the send. `Shutdown` holds it across `close(b.closed)` and the sentinel send. This closes the race: a request is either sent before the sentinel, or it sees `b.closed` and is rejected.

Correctness verdict: **the fix is correct.** Verified as follows:

- `go test -count=1 -race ./lib/batcher` at `review-head` passes (`ok … 5.345s`).
- The new `TestBatcherCommitRacingShutdown` fails against the unfixed base `batcher.go`. I substituted the base file with `go test -overlay` using the file from `git show main:lib/batcher/batcher.go`. The sync subtest failed with `commit hung while racing shutdown` and the async subtest with `accepted commit was dropped during shutdown`.
- Deadlock check: the only other goroutine that must make progress while `Commit` holds `admitMu` and is blocked on a full `b.in` is `commitLoop`. `commitLoop` never takes `admitMu`, and it cannot have exited, because it only exits after the sentinel, and the sentinel is only enqueued under `admitMu`. The two callers (`backend/dropbox/dropbox.go:2122` and `backend/googlephotos/googlephotos.go:1226`) do not call back into `Commit` or `Shutdown` from their `commitBatch` functions.

The findings below are about the *shape* of the fix, not its correctness.

---

## Finding 1.1 — The lock makes the quit-sentinel protocol deletable, but the PR layers the lock on top of it and leaves a now-false comment

**Where:** `lib/batcher/batcher.go:60` (`quit bool` field), `:208-211` (sentinel check in `commitLoop`), `:243-252` (`Shutdown`, including the comment at `:246-250`).

**The problem.** The in-band `{quit: true}` sentinel exists for exactly one reason, and the code says so at `:248-250`: "we don't close b.in because that will cause write to closed channel in Commit when we are exiting due to a signal." That reasoning was true when `Commit` could send without coordinating with `Shutdown`. This PR removes that possibility: every send on `b.in` now happens under `admitMu`, after a check of `b.closed`, and `Shutdown` closes `b.closed` under the same lock. After this PR, `Commit` can no longer write to a closed `b.in`. The comment justifies a hazard the PR has just removed.

The result is a two-mechanism design where one would do:

1. A mutex that orders admission against shutdown.
2. An in-band sentinel value carried in the same `request` type as real work. Because of that, `request` has a mode flag (`quit bool`) that is meaningless for every real request, and `commitLoop` has a special-case branch to check for it.

A reader now has to understand both mechanisms and see that the mutex is what actually makes the sentinel safe. The comment points them in the wrong direction.

**The code-judo move.** Close the channel under the lock instead of sending a sentinel. Go already has an end-of-stream signal for channels: close. A `receive, ok` on a closed buffered channel returns every buffered value before reporting `!ok`. So "commitLoop drains every admitted request before exiting" stops being a property you have to reason out from ordering. It follows directly from channel semantics. Concretely:

- Delete the `quit bool` field from `request` (`:60`). `request` goes back to carrying only real work.
- `commitLoop`: `case req, ok := <-b.in: if !ok { break outer }` replaces the sentinel check.
- `Shutdown`: `close(b.closed); close(b.in)` under `admitMu`, replacing the sentinel send and the stale four-line comment.

There is also a side benefit. If a future change adds a send on `b.in` that skips the lock, closing the channel makes it **panic** (`send on closed channel`) right away. With the sentinel design, the same mistake reproduces this bug: a silent hang or a lost upload, which the maintainer's issue comment notes the race detector cannot see.

`Shutdown` also no longer blocks on a full `b.in` while holding `admitMu`, because `close` does not block. With the sentinel, `Shutdown` can hold the lock while waiting for `commitLoop` to make room in the buffer.

**Worked proposal (verified).** I applied this to a scratch copy (`clone-work/scratch/batcher_v2.go`) and ran the package tests through `go test -overlay` without touching the clone:

```
go test -count=3 -race -overlay=<work>/scratch/overlay_v2.json ./lib/batcher
ok  	github.com/rclone/rclone/lib/batcher	13.979s
```

The full suite passed three times under `-race`, including the PR's own `TestBatcherCommitRacingShutdown`. The diff against `review-head`:

```diff
@@ type request
 	item   Item
 	name   string
 	result chan<- response[Result]
-	quit   bool // if set then quit
 }
@@ func (b *Batcher[Item, Result]) commitLoop
-		case req := <-b.in:
-			if req.quit {
+		case req, ok := <-b.in:
+			if !ok {
 				break outer
 			}
@@ func (b *Batcher[Item, Result]) Shutdown
 		b.admitMu.Lock()
-		// show that batcher is shutting down
+		// Stop admission and let commitLoop drain everything
+		// already admitted. Commit only sends while holding
+		// admitMu after checking b.closed, so closing b.in here
+		// cannot race a send.
 		close(b.closed)
-		// quit the commitLoop by sending a quitRequest message
-		//
-		// Note that we don't close b.in because that will
-		// cause write to closed channel in Commit when we are
-		// exiting due to a signal.
-		b.in <- request[Item, Result]{quit: true}
+		close(b.in)
 		b.admitMu.Unlock()
```

(The `Commit` side of the same scratch file is Finding 1.2.)

**Follow-on simplification (not verified by execution; reasoned).** Once `b.closed` is only ever read under `admitMu`, it does not need to be a channel at all. A `closing bool` guarded by `admitMu` says the same thing without the `select { case <-b.closed: … default: }` idiom. The only other reader is the new test (`batcher_test.go:288`), which uses the channel as a timing hint. This step is optional. The sentinel deletion above is the part that matters.

---

## Finding 1.2 — `Commit` threads manual Lock/Unlock through its body, with an early-return unlock and unrelated work inside the critical section

**Where:** `lib/batcher/batcher.go:267-281`.

**The problem.** The admission critical section is written inline as `Lock` at the top of `Commit`, a second `Unlock` in the early-return branch of the `select`, and a final `Unlock` after the send. Every future edit to `Commit` must keep two unlock paths balanced by hand. A new early return between `:267` and `:281` would leak the lock and deadlock the next `Commit` and `Shutdown`. The region also holds work that has nothing to do with admission: `fs.Debugf` (which formats and calls `String()` on the backend's `Fs`) and `make(chan response[Result], 1)`. Admission and waiting for the result are two different jobs, and the current shape mixes them in one function body.

**Remedy.** Extract the atomic step into a small helper whose whole body is the critical section, and use `defer`:

```go
// admit enqueues req unless the batcher is shutting down.
//
// Every request admitted here is drained by commitLoop before it
// exits, because Shutdown closes b.in under the same lock.
func (b *Batcher[Item, Result]) admit(req request[Item, Result]) error {
	b.admitMu.Lock()
	defer b.admitMu.Unlock()
	select {
	case <-b.closed:
		return fserrors.FatalError(errors.New("batcher is shutting down"))
	default:
	}
	fs.Debugf(b.f, "Adding %q to batch", req.name)
	b.in <- req
	return nil
}
```

`Commit` then becomes: build `resp`, call `b.admit(...)`, return early on error, and otherwise return immediately (async) or wait on `resp` (sync). The lock is visible in one six-line function, next to the comment stating the invariant it protects. `Commit` itself no longer touches the mutex.

The `fs.Debugf` stays inside `admit` in the verified version above only because the PR's regression test depends on it being there (see `02_regression_test.md`, Finding 2.1). On its own merits it belongs outside the lock.

**Verification.** This is part of the same scratch file (`batcher_v2.go`) that passed `go test -count=3 -race` above.

---

## Considered and not raised as findings

- **Shutdown priority under a plain `sync.Mutex`.** New `Commit` callers that arrive after `Shutdown` starts can take `admitMu` ahead of `Shutdown` while Go's mutex is in normal (non-starvation) mode. Before the PR, `close(b.closed)` took effect immediately. A `sync.RWMutex` (`Commit` takes `RLock`, `Shutdown` takes `Lock`) would let admissions run concurrently and block new readers once shutdown is waiting. This was reasoned, not measured. The effect is bounded, and in practice sends on `b.in` were already serialized by the channel. I mention it here so the author can decide whether to use `RWMutex` when adopting Finding 1.1. It is not a blocker.
- **`ctx` is not honoured while waiting to be admitted.** A cancelled context cannot interrupt the wait for `admitMu` or the send on `b.in`. This is pre-existing: the unguarded send ignored `ctx` before the PR too.
- **File size.** `batcher.go` is 288 lines and `batcher_test.go` is 364 lines. There is no decomposition concern.
- **Drain-after-quit alternative (from the issue thread).** Draining `b.in` after the sentinel with a non-blocking loop is *not* sufficient on its own. A straggler that passed the closed-check but has not yet sent can land after the drain finishes. Some admission gate is needed, so the PR's choice of a lock is right. The finding is only about what the lock makes deletable.

## Commands used

```
git diff main...review-head
GOMODCACHE=… GOCACHE=… GOFLAGS=-mod=mod GOPROXY=off GOTOOLCHAIN=local go test -count=1 -race ./lib/batcher
    → ok (head)
… go test -count=1 -overlay=<work>/scratch/overlay_base.json -run TestBatcherCommitRacingShutdown ./lib/batcher
    → FAIL sync "commit hung while racing shutdown"; FAIL async "accepted commit was dropped during shutdown" (base code)
… go test -count=3 -race -overlay=<work>/scratch/overlay_v2.json ./lib/batcher
    → ok (proposed close(b.in) + admit() redesign)
git status --short   → clean after every run
```
