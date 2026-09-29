# Detail: lib/batcher (batcher.go, batcher_test.go)

## Measurements and commands

- `git diff main...review-head`: 2 files, +95/-0 (6 lines production, 89 lines test).
- `wc -l lib/batcher/batcher.go lib/batcher/batcher_test.go`: 288 and 364 lines. No 1k-line concern.
- `go test -count=1 -race ./lib/batcher` (offline, env per policy): `ok github.com/rclone/rclone/lib/batcher 5.340s`.
- `b.in` is created at batcher.go:107 as `make(chan request, opt.Size)`, a buffered channel, so `b.in <-` can block when the buffer is full and the commit loop is busy.

## Production change analysis

Before: `Commit` checked `b.closed` with a non-blocking select, then sent to `b.in`. `Shutdown` did `close(b.closed)` then `b.in <- quit`. A Commit could pass the check, then lose the scheduling race so its request landed after the quit marker; the loop returned on quit and never read it. Sync callers hang on `<-resp`; async requests are dropped.

After: `admitMu` is taken in `Commit` before the check and released after the send, and in `Shutdown` before `close(b.closed)` and released after the quit send. Any Commit either enqueues wholly before the quit marker or observes `closed` and returns the fatal error. Correctness is fine for the stated invariant.

Considered alternative: drain `b.in` in `commitLoop` after quit and reply with the shutdown error. Rejected as a simplification: a sender that passed the check can still enqueue after the drain finishes, so the race survives without a lock. The mutex is the minimal correct primitive. An `RWMutex` (Commit takes RLock, Shutdown takes Lock) would allow concurrent admission but the buffered channel already serializes the send, so it buys nothing.

Behavior note: `close(b.closed)` is now delayed until any Commit blocked on a full `b.in` completes its send. Shutdown waits for the loop regardless, so this is benign.

## Finding 1 detail: manual unlock (batcher.go:267-281)

Current shape:

```go
b.admitMu.Lock()
select {
case <-b.closed:
    b.admitMu.Unlock()
    return entry, fserrors.FatalError(errors.New("batcher is shutting down"))
default:
}
fs.Debugf(b.f, "Adding %q to batch", name)
resp := make(chan response[Result], 1)
b.in <- request{...}
b.admitMu.Unlock()
```

Worked code-judo proposal (behavior-preserving):

```go
// admit enqueues the request unless the batcher is shutting down.
// The lock spans the send so that an accepted request is always
// queued ahead of the shutdown marker.
func (b *Batcher[Item, Result]) admit(name string, item Item) (chan response[Result], error) {
    b.admitMu.Lock()
    defer b.admitMu.Unlock()
    select {
    case <-b.closed:
        return nil, fserrors.FatalError(errors.New("batcher is shutting down"))
    default:
    }
    fs.Debugf(b.f, "Adding %q to batch", name)
    resp := make(chan response[Result], 1)
    b.in <- request[Item, Result]{item: item, name: name, result: resp}
    return resp, nil
}

func (b *Batcher[Item, Result]) Commit(ctx context.Context, name string, item Item) (entry Result, err error) {
    resp, err := b.admit(name, item)
    if err != nil {
        return entry, err
    }
    if b.async {
        return entry, nil
    }
    result := <-resp
    return result.entry, result.err
}
```

Verification: reasoned from source, not compiled. Note the debug log stays inside the lock, which the existing test depends on (finding 3); if the log line is moved, the test must change with it.

Risk if unaddressed: a future early return between Lock and the final Unlock leaks the mutex, blocking all later Commit and Shutdown calls silently. Current code has no such path, so this is preventive.

## Finding 2 detail: undocumented lock scope

The field comment at batcher.go:50 states purpose but not the scope rule (lock spans a possibly-blocking send). Suggested comment text: "held across the send to b.in so a request accepted here is always queued before the quit marker sent by Shutdown". Verification: source reading.

## Finding 3 detail: test seam

Test at batcher_test.go:239 uses `blockingStringer` (lines 22-35) as the `fs.Fs` value. Sequence: debug log level set (245), Commit goroutine blocks in `String()` called from `fs.Debugf` inside the lock, Shutdown started (line ~281), 100 ms wait (287-290; with the fix `closed` cannot close, so the timer always fires), release closed, then assertions on commit, shutdown and the batch callback all completing.

Fragilities: (a) relies on `Debugf` calling `String()` synchronously between check and send; (b) mutates the global config log level, restored twice (deferred at 246 and inline at 279); (c) fixed 100 ms sleep per subtest; (d) `require.NoError` on the commit result will fail with a non-obvious message if the seam moves. Author reports the test fails on unfixed code, which is consistent with my reading, and I did not re-run against the unfixed code. Alternatives: an unexported test hook called after the closed check, or testing the extracted `admit` helper. Otherwise document the dependency on `blockingStringer`.

## Finding 4 detail: rejection path

The new test asserts only the accepted-before-shutdown outcome. Adding, after `shutdownDone`, `_, err := b.Commit(ctx, "late", Item("late")); require.Error(t, err)` would cover the other half of the contract in both modes. `TestBatcherCommitShutdown` was not inspected for existing coverage of the rejection error message, so treat this as "consider" rather than a confirmed gap.
