# Detail: lib/batcher

## Measurements and commands

- `git diff main...review-head`: `batcher.go` +6, `batcher_test.go` +89.
- File sizes after the change: `lib/batcher/batcher.go` 288 lines, `lib/batcher/batcher_test.go` 364 lines. No 1000-line crossing.
- `go test -count=1 -race ./lib/batcher` (offline, GOFLAGS=-mod=mod GOPROXY=off): `ok` in about 5s.
- `b.in` is created with capacity `opt.Size` (batcher.go:107), so `Commit`'s send can block while the commit loop is inside a batch commit.

## Verification status

Behaviour of the fix: verified by reading. Under `admitMu`, Commit's closed-check and send are atomic with Shutdown's `close(b.closed)` and quit send. Any Commit that gets the lock first enqueues ahead of the quit marker, so the loop's post-loop `commit()` handles it. Any Commit that gets the lock second sees `closed` and returns the fatal error. No deadlock path found: the commit loop never takes `admitMu`, and it drains `b.in` until quit, so a Shutdown or Commit blocked on a full channel while holding the lock always makes progress. F1 to F4 are maintainability findings and are not behavioural bugs; I did not run the test against unfixed code, and the packet reports that the approving reviewer did.

## F1 worked proposal: extract `admit` with defer

```go
// admit adds the request to the batch unless shutdown has started.
// Holding admitMu across the check and the send guarantees the
// request lands before Shutdown's quit marker.
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
	if err != nil || b.async {
		return entry, err
	}
	result := <-resp
	return result.entry, result.err
}
```

This removes both manual unlock sites, and `Commit` returns to its original three-line shape.

## F2 note: RWMutex

`Commit` would use `RLock`/`RUnlock` and `Shutdown` would use `Lock`/`Unlock`. Concurrent committers stay concurrent, and the type expresses that only Shutdown is exclusive. The behaviour is otherwise identical, since the shutdown path is only entered once (`shutOnce`).

## F3 worked proposal: explicit hook instead of a log-argument trick

Add an unexported field `afterClosedCheck func()` (nil in production), invoked inside `admit` after the closed check and before the send. The test sets it to close `started` and wait on `release`. This removes `blockingStringer`, the `ci.LogLevel` juggling and the dependency on `Debugf` argument evaluation. To replace the 100ms wait, poll until `Shutdown` is observably blocked, or accept the hook plus a short bounded wait and document it as best-effort.

## F4 note

Add a comment at the quit send: the send is under `admitMu` so that no admitted request can follow the quit marker.
