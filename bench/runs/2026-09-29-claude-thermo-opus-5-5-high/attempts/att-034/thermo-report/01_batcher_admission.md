# 01 — `lib/batcher/batcher.go`: admission vs. shutdown

Scope: the production half of `main...review-head` (commit `0b87bc478`), +6 / −0 in
`lib/batcher/batcher.go`. The file is 288 lines after the change, so there is no size concern.

## What the diff does

The PR adds a field `admitMu sync.Mutex` ("serializes Commit admission with shutdown") and
holds it in two places:

- `Shutdown` (lines 243–252) takes `admitMu`, closes `b.closed`, sends the sentinel
  `request{quit: true}` on `b.in`, then releases the lock and waits on `b.wg`.
- `Commit` (lines 267–281) takes `admitMu`, does a non-blocking `select` on `b.closed`,
  unlocks by hand on the early-return path, logs, makes the reply channel, sends on `b.in`
  (a buffered channel that can block while `commitLoop` is inside `commitBatch`), and then
  unlocks by hand.

## Correctness verdict

The fix is correct. Admission is now linearizable against shutdown. Any `Commit` that sees
`b.closed` open finishes its send before `Shutdown` can close `b.closed` and enqueue the quit
sentinel, so every accepted request sits ahead of the sentinel and gets drained by
`commitLoop`, including the "commit any remaining items" tail at lines 227–230. A `Commit`
that gets the lock after `Shutdown` sees `b.closed` and returns the existing fatal
"batcher is shutting down" error. I found no deadlock. `commitLoop` never takes `admitMu`, so
a `Commit` holding the lock while blocked on a full `b.in` always makes progress once the
current `commitBatch` returns. `Shutdown` holding the lock across its own sentinel send is
safe for the same reason.

Verification: `go test -count=1 -race -v -run 'TestBatcherCommitRacingShutdown|TestBatcherCommitShutdown' ./lib/batcher`
passes on `review-head`: both subtests PASS, each taking 0.10s.

## Finding F1 — the fix bolts a mutex onto the old shutdown protocol instead of letting it collapse

The mutex is the right primitive, but the PR puts it on top of the pre-existing shutdown
machinery and leaves that machinery unchanged. The result has three overlapping mechanisms
expressing one fact ("the batcher is no longer accepting work"):

1. `closed chan struct{}`. After this PR, its only production reader is the `select` in
   `Commit`, and that read now always happens under `admitMu`. A channel used purely as a
   flag that is read under a mutex is just a `bool` with extra ceremony. `grep -rn "b\.closed" lib/batcher/`
   finds only `batcher.go:245` (close), `batcher.go:269` (read) and the new test at
   `batcher_test.go:288`.
2. The `quit bool` field on `request` (line 60) and the sentinel send at line 251. The sentinel
   only existed because `b.in` could not be closed.
3. The comment at lines 246–250 is now stale and misleading: "Note that we don't close b.in
   because that will cause write to closed channel in Commit when we are exiting due to a
   signal." That was true when admission was unsynchronised. It is exactly the hazard that
   `admitMu` now removes. Under the lock, `Commit` checks the flag before it sends, and
   `Shutdown` sets the flag and closes the channel in one critical section. Nobody can send
   on a closed `b.in`.

So the code-judo move is available and the PR stops one step short of it. Once admission is
serialized with shutdown, **closing `b.in` is the shutdown signal**. The `closed` channel
becomes a `bool`, the `quit` field and the sentinel request go away, and `commitLoop` exits on
`req, ok := <-b.in; !ok`, which is the idiomatic Go "producer is done" signal. The stale
comment gets deleted instead of contradicting the code beside it.

Two smaller structural issues go away in the same rewrite:

- **Manual unlock on two exit paths in `Commit`** (lines 270 and 281). The lock covers a check,
  a log call, an allocation and a potentially blocking send, and each exit unlocks by hand.
  If someone later adds a return or a `ctx.Done()` case between Lock and Unlock and forgets the
  unlock, the whole batcher wedges: every future `Commit` and `Shutdown` blocks. Moving the
  critical section into a small `admit` helper with `defer` removes that trap. It also gives
  the invariant a name and a single doc comment.
- **An exclusive lock across a blocking send.** With `sync.Mutex`, concurrent `Commit` callers
  now queue on the mutex instead of on the channel, and the log formatting and `make(chan)`
  run under the exclusive lock. Throughput is basically unchanged because the channel admits
  one item at a time anyway. Still, the lock's real contract is "many admitters, one closer",
  and `sync.RWMutex` expresses that directly: admitters take `RLock` and send concurrently,
  and `Shutdown` takes `Lock` to close. That also makes the reason the lock is held across the
  send obvious: the writer must not close `in` while a reader might still send on it.

### Worked proposal (verified)

I applied this to scratch copies under `clone-work/judo/` via `go test -overlay`. The clone was
not modified. Full diff against `review-head`:

```diff
@@ type Batcher
 	in       chan request[Item, Result]  // incoming items to batch
-	closed   chan struct{}               // close to indicate batcher shut down
 	atexit   atexit.FnHandle             // atexit handle
-	admitMu  sync.Mutex                  // serializes Commit admission with shutdown
+	mu       sync.RWMutex                // held for reading while sending on in, for writing to close it
+	closed   bool                        // set under mu when in has been closed
@@ type request
 	result chan<- response[Result]
-	quit   bool // if set then quit
 }
@@ New
 		in:     make(chan request[Item, Result], opt.Size),
-		closed: make(chan struct{}),
@@ commitLoop
-		case req := <-b.in:
-			if req.quit {
+		case req, ok := <-b.in:
+			if !ok {
 				break outer
 			}
@@ Shutdown
-		b.admitMu.Lock()
-		// show that batcher is shutting down
-		close(b.closed)
-		// quit the commitLoop by sending a quitRequest message
-		//
-		// Note that we don't close b.in because that will
-		// cause write to closed channel in Commit when we are
-		// exiting due to a signal.
-		b.in <- request[Item, Result]{quit: true}
-		b.admitMu.Unlock()
+		b.mu.Lock()
+		b.closed = true
+		close(b.in) // commitLoop drains queued requests then exits
+		b.mu.Unlock()
 		b.wg.Wait()
@@ Commit
-	b.admitMu.Lock()
-	select {
-	case <-b.closed:
-		b.admitMu.Unlock()
-		return entry, fserrors.FatalError(errors.New("batcher is shutting down"))
-	default:
-	}
-	fs.Debugf(b.f, "Adding %q to batch", name)
 	resp := make(chan response[Result], 1)
-	b.in <- request[Item, Result]{
-		item:   item,
-		name:   name,
-		result: resp,
+	if err := b.admit(request[Item, Result]{item: item, name: name, result: resp}); err != nil {
+		return entry, err
 	}
-	b.admitMu.Unlock()
@@ new helper
+// admit queues req for the commitLoop unless Shutdown has closed the
+// input. Holding mu for reading across the send means Shutdown cannot
+// close b.in until every admitted request is in the channel.
+func (b *Batcher[Item, Result]) admit(req request[Item, Result]) error {
+	b.mu.RLock()
+	defer b.mu.RUnlock()
+	if b.closed {
+		return fserrors.FatalError(errors.New("batcher is shutting down"))
+	}
+	fs.Debugf(b.f, "Adding %q to batch", req.name)
+	b.in <- req
+	return nil
+}
```

The only test change needed was replacing the `select { case <-b.closed: case <-time.After(100ms) }`
at `batcher_test.go:287-290` with `time.Sleep(100 * time.Millisecond)`, because `closed` is no
longer a channel. See 02 for why that line is a problem anyway.

Verification status: **verified**. With the overlay above,
`go test -overlay=<work>/judo/overlay.json -count=1 -race ./lib/batcher` gives
`ok github.com/rclone/rclone/lib/batcher 5.344s`. That is the full package, including the
PR's new `TestBatcherCommitRacingShutdown` sync and async subtests. The `debugf` call stays
between the closed-check and the send, so the PR's regression test still exercises the same
window.

Behavior preserved: same error value and type (`fserrors.FatalError`) for late commits, same
drain-then-exit semantics in `commitLoop` (a closed channel still yields all buffered items
before `!ok`), same `shutOnce`/`wg` lifecycle. One subtle difference makes things simpler:
`Shutdown` no longer blocks while holding the lock waiting for buffer space for the sentinel,
because `close` never blocks.

Severity: medium. It is a missed simplification, not a regression. The PR's version works, but
it leaves a comment that now argues against the code beside it, plus two shutdown signals
(`closed` and `quit`) where one would do. The next person to touch this has to rediscover why
both exist.
