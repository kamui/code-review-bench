# Detail report 01: lib/batcher (batcher.go, batcher_test.go)

Range reviewed: `862ed2b7a..0b87bc478` (`git diff main...review-head`), 2 files, +95/-0.
Verification: `go test -count=1 -race ./lib/batcher` passes on the head (offline, ~5s). No files approach 1000 lines (batcher.go 288, batcher_test.go 364).

## Baseline

The change adds `admitMu sync.Mutex`. `Shutdown` locks it around `close(b.closed)` and the `b.in <- quit` send. `Commit` locks it around the closed-check, the debug log, and the `b.in <- request` send. This does close the race (a Commit is either wholly before the quit marker or sees `closed`). Behaviour is correct; the findings below are structural.

## Finding 1: closed-check plus channel plus mutex now encode one fact three ways (missed simplification)

Evidence: `grep -n closed lib/batcher/batcher.go` shows `b.closed` is only created (line 108), closed (245) and read (269, a non-blocking select). It is never selected on alongside anything else, and nothing outside `Commit` observes it. Before the PR a channel was the right tool because the check was lock-free. Once every reader and the single writer hold `admitMu`, the channel provides nothing a `closed bool` guarded by `admitMu` would not, and the `select { case <-b.closed: ... default: }` idiom (5 lines) becomes `if b.closed { ... }`.

Worked proposal:

```go
type Batcher struct {
    ...
    admitMu sync.Mutex // guards closed and admission to b.in
    closed  bool       // set by Shutdown under admitMu
}

// enqueue admits req unless the batcher has shut down.
func (b *Batcher[Item, Result]) enqueue(req request[Item, Result]) error {
    b.admitMu.Lock()
    defer b.admitMu.Unlock()
    if b.closed {
        return fserrors.FatalError(errors.New("batcher is shutting down"))
    }
    b.in <- req
    return nil
}
```

`Shutdown` becomes lock; `b.closed = true`; send quit; unlock (or the same via a small `closeAdmission()` helper). This removes one channel, the select, and the manual triple `Unlock()` pairing.

Verification status: read-only analysis; not compiled. The existing test reads `<-b.closed` and would need adjusting.

## Finding 2: hand-paired Unlock on two exit paths in Commit; lock spans a blocking send

Evidence: `Commit` (batcher.go ~267-284) has `Lock()` at the top, an `Unlock()` in the early-return branch, and a second `Unlock()` after the send. `Shutdown` likewise unlocks by hand. The lock is held across `b.in <- request`, a send on a channel whose buffer is `opt.Size` (line 107); when the commit loop is busy in a slow backend `commit()`, the buffer fills and every other producer plus `Shutdown` queue on `admitMu` rather than on the channel. That is not a deadlock (the loop still drains), and it was already effectively serialised by the channel, but it hides where blocking happens and any future early return added between Lock and Unlock (e.g. honouring `ctx` on the send) will leak the lock. It also means `Commit` cannot become ctx-aware without rewriting the critical section.

The remedy is the `enqueue` helper above: `defer Unlock` makes the lock scope a single function, the response wait stays outside it (as the PR already intends), and `Commit` reads as "admit, then wait". If concurrent admission ever matters, `sync.RWMutex` (Commit takes `RLock`, Shutdown `Lock`) states the intent directly, since the only exclusion required is "no admission concurrent with shutdown", not "no admission concurrent with other admission"; the channel already serialises the sends themselves.

Verification status: read-only analysis.

## Finding 3: the regression test steers the code through a log-formatting side channel (89 lines for a 6-line fix)

Evidence: `blockingStringer` (batcher_test.go, added lines 22-35) blocks inside `String()`, which is invoked only because `fs.Debugf(b.f, "Adding %q to batch", name)` in `Commit` formats `b.f` when the global log level is Debug. The test therefore (a) mutates the process-global `ci.LogLevel` (restored in a defer and also manually mid-test), (b) depends on the Debugf call staying inside the critical section and on `%v`-formatting of `b.f` continuing to call `String()`, and (c) uses a 100 ms `select` on `b.closed` as a "give Shutdown time to contend" sleep. After the fix `b.closed` cannot close until `Commit` releases the lock, so that branch always waits the full 100 ms; the wait is a timing heuristic, not a synchronisation. If someone moves or removes the debug line, the test silently stops exercising the race window (it would pass regardless) instead of failing. The sync/async table is a good idea; the pause mechanism is the fragile part.

Remedy options: (1) add a tiny unexported test hook on the batcher (e.g. a nil-by-default `beforeSend func()` invoked inside the critical section) so the pause point is explicit; or (2) keep the interleaving-free approach: a stress test with N concurrent Commits racing one Shutdown asserting every call returns and every accepted async item is committed (this is how the issue reporter reproduced it, hanging by iteration 6 of 2000). Either removes the global log-level mutation and the sleep. The `errors` parameter name in the `commitBatch` literal also shadows the imported `errors` package in that closure; harmless but avoidable.

Verification status: the test passes under `-race` on the head. I did not run it against the unfixed code (the clone is read-only).

## Not flagged

No file-size threshold crossed; no new conditionals scattered in unrelated flows; the preserved error text and `FatalError` wrapping match the existing behaviour.
