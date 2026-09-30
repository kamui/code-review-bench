# 01 — `lib/batcher/batcher.go`: admission vs. shutdown

Scope: the six production lines added by `0b87bc478` (range
`862ed2b7a..0b87bc478`) and the surrounding `Batcher` struct, `Shutdown`, and
`Commit`.

## Measurements

| File | Lines at base | Lines at head | Delta |
| --- | --- | --- | --- |
| `lib/batcher/batcher.go` | 282 | 288 | +6 / −0 |
| `lib/batcher/batcher_test.go` | 275 | 364 | +89 / −0 |

Produced with `git show main:<path> | wc -l`, `wc -l <path>`, and
`git diff --stat main...review-head`. Neither file is anywhere near the 1000-line
threshold; there is no file-size concern.

Usage of the shutdown state, from a grep of `lib/batcher`, `backend/dropbox`, and
`backend/googlephotos` for `.closed|admitMu`:

- `b.closed` is read in exactly one production place, the non-blocking `select`
  in `Commit` (`batcher.go:268-273`), and closed in exactly one, `Shutdown`
  (`batcher.go:245`). The only other reference is the new test
  (`batcher_test.go:288`).
- `admitMu` is locked/unlocked at `batcher.go:243`, `252`, `267`, `270`, `281`.
- External callers (`backend/dropbox/dropbox.go:556,1763`,
  `backend/googlephotos/googlephotos.go:417,891`) only use `New`, `Commit`, and
  `Shutdown`. The shutdown state is fully private, so changing how it is
  represented is a purely local refactor.

## Correctness check (baseline for the quality discussion)

The fix is correct. Holding `admitMu` from the closed check through the send in
`Commit`, and from `close(b.closed)` through the quit send in `Shutdown`, means
every non-quit request that reaches `b.in` is ahead of the quit marker in FIFO
order. `commitLoop` processes everything it reads before the marker and then
commits the remainder (`batcher.go:227-230`), so every admitted request is
answered (sync) or committed (async). Any `Commit` that takes the lock after
`Shutdown` sees `closed` and gets the existing `FatalError("batcher is shutting
down")`.

I checked for new deadlocks. `Commit` now holds the mutex across a potentially
blocking send when `b.in` (capacity `opt.Size`) is full. That only makes progress
if `commitLoop` drains the channel, and `commitLoop` never takes `admitMu`, so it
cannot deadlock. Re-entrant use from inside a `CommitBatchFn` (calling `Commit`
or `Shutdown` from the commit callback) would deadlock, but it already did before
this PR: a full-buffer send, or `wg.Wait` from inside the loop. So that is not a
regression.

Verification (all run offline from the clone root with the prescribed
GOMODCACHE/GOCACHE/GOFLAGS/GOPROXY/GOTOOLCHAIN environment):

- `go test -count=1 -race ./lib/batcher` at head: `ok  github.com/rclone/rclone/lib/batcher  5.347s`.
- The new test run against the **unfixed** base `batcher.go`, swapped in with a
  `-overlay` under the work directory (`go test -count=1 -overlay=base.json -run
  TestBatcherCommitRacingShutdown -v ./lib/batcher`): both subtests fail, with
  `sync` reporting "commit hung while racing shutdown" and `async` reporting
  "accepted commit was dropped during shutdown". So the regression test does
  detect the original bug.

## Finding 1.1 — The fix layers a mutex on top of a channel-as-flag, leaving two representations of "shut down" and a hand-balanced lock (verified)

**Where:** `lib/batcher/batcher.go:48-50` (struct fields), `243-252`
(`Shutdown`), `267-281` (`Commit`).

**Problem.** Before this PR, `closed chan struct{}` plus a non-blocking
`select { case <-b.closed: ... default: }` was the batcher's lock-free way to ask
"has shutdown started?". The PR correctly identifies that the check and the send
must be atomic with respect to shutdown, and it adds `admitMu` for that. But it
keeps the channel. Every read and write of `closed` now happens with `admitMu`
held, so the channel no longer does any synchronization. It is a boolean dressed
up as a channel, and the reader has to work out that the `select`/`default` idiom
has become a plain flag test. The struct now holds two fields that together mean
one thing, "admission is closed". Only one of them (`admitMu`) carries the
actual invariant, and nothing in the code connects the two except their
placement.

The lock handling makes this worse. `Commit` takes the lock at line 267 and
releases it on two separate paths: inside the `select` case at line 270, and
after the send at line 281. Between those points sit a debug log, an allocation,
and a potentially blocking channel send. This is the manual multi-exit
lock-balancing that `defer` exists to prevent. A future edit that adds an early
return, for example a `ctx.Done()` case on the send, which is a natural next
change because `Commit` already takes a `ctx` it ignores, will silently leak the
lock and wedge both every later `Commit` and `Shutdown`. `Shutdown` also
open-codes the same critical section (lock, mark closed, send, unlock) without
naming it. So "admit a request into the queue unless we are shut down" has no
single home. It exists as two hand-synchronized halves.

The field comment `// serializes Commit admission with shutdown` also leaves out
the one non-obvious property of this lock: it is deliberately held across a
blocking send on `b.in`. So `commitLoop` (and any `CommitBatchFn`) must never
take it, and a full buffer makes `Shutdown` wait for the lock rather than
rejecting new work immediately. That invariant is the whole design, and it
should be written down where the lock is declared.

**Code-judo remedy.** Delete the channel and make admission one named critical
section. `closed` becomes a plain `bool` guarded by the mutex. The lock is taken
in exactly one helper with `defer`, and `Commit` stops knowing anything about
locking:

```go
type Batcher[Item, Result any] struct {
	...
	in       chan request[Item, Result]  // incoming items to batch
	atexit   atexit.FnHandle             // atexit handle
	mu       sync.Mutex                  // held across sends to in (may block) so nothing is queued behind the quit request
	closed   bool                        // set under mu when shutdown starts
	shutOnce sync.Once                   // make sure we shutdown once only
	wg       sync.WaitGroup              // wait for shutdown
}

// enqueue sends req to the commitLoop unless shutdown has started.
//
// Every request accepted here is ahead of the quit request in b.in,
// so the commitLoop always processes it.
func (b *Batcher[Item, Result]) enqueue(req request[Item, Result]) error {
	b.mu.Lock()
	defer b.mu.Unlock()
	if b.closed {
		return fserrors.FatalError(errors.New("batcher is shutting down"))
	}
	fs.Debugf(b.f, "Adding %q to batch", req.name)
	b.in <- req
	return nil
}

func (b *Batcher[Item, Result]) Shutdown() {
	...
	b.shutOnce.Do(func() {
		atexit.Unregister(b.atexit)
		fs.Infof(b.f, "Committing uploads - please wait...")
		// Mark closed and queue the quit request atomically so no
		// Commit can be admitted behind it.
		//
		// Note that we don't close b.in because that will
		// cause write to closed channel in Commit when we are
		// exiting due to a signal.
		b.mu.Lock()
		b.closed = true
		b.in <- request[Item, Result]{quit: true}
		b.mu.Unlock()
		b.wg.Wait()
	})
}

func (b *Batcher[Item, Result]) Commit(ctx context.Context, name string, item Item) (entry Result, err error) {
	resp := make(chan response[Result], 1)
	err = b.enqueue(request[Item, Result]{item: item, name: name, result: resp})
	if err != nil {
		return entry, err
	}
	// If running async then don't wait for the result
	if b.async {
		return entry, nil
	}
	result := <-resp
	return result.entry, result.err
}
```

This removes a concept (the `closed` channel and its `make` in `New`), removes a
`select` statement, removes the two-exit manual unlock, and gives the invariant
("nothing queued behind quit") a named function with a comment. Behavior is
unchanged. The debug log stays between the check and the send, so the PR's
`blockingStringer` test seam still works.

**Verification status: verified.** I built a variant of this in the work
directory and ran it with `-overlay` (production file plus test file, where the
test's `case <-b.closed` wait became `time.Sleep(100 * time.Millisecond)`,
because the channel no longer exists):
`go test -count=1 -race -overlay=judo.json ./lib/batcher` gave
`ok  github.com/rclone/rclone/lib/batcher  5.342s`. The overlaid variant routed
the quit request through `enqueue` with an `if req.quit { b.closed = true }`
branch. The form shown above, where `Shutdown` open-codes the four-line
lock/flag/send/unlock, is what I recommend instead. It avoids putting a
quit-specific branch in the admission helper and is a trivially equivalent
rearrangement of the verified code. The clone was left unmodified
(`git status --short` empty).

**Severity:** moderate maintainability issue, not a blocker. The PR is small
and correct. But it fixes an atomicity bug by bolting a lock around an idiom
that was designed to avoid locks, rather than replacing it, and it leaves
exactly the multi-exit unlock shape that tends to regress next.
