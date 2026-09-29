# Review blind-98880d

### Item 1
Location: lib/batcher/batcher.go:251
Claim: admitMu is held across blocking channel sends (quit marker at 251, request at 276), so Commit calls arriving during shutdown no longer fail fast and wait on a mutex that cannot be cancelled by ctx.
Consequence: b.in is full while commitLoop is inside a slow b.commit (e.g. a Dropbox batch finish that polls for a long time). Shutdown takes admitMu and blocks sending the quit marker. Every Commit arriving now blocks in admitMu.Lock() until the batch finishes, ignoring ctx cancellation, where previously it returned 'batcher is shutting down' immediately from the lock-free closed check. Symmetrically, a Commit blocked on a full b.in holds admitMu, so Shutdown (the atexit signal handler path) cannot even close b.closed to reject new work until that send completes.
Fix: —

### Item 2
Location: lib/batcher/batcher.go:267
Claim: admitMu is locked and unlocked manually on two paths without defer, so a panic between Lock and Unlock leaves the mutex held permanently.
Consequence: fs.Debugf at line 274 calls b.f.String() and the log handler while holding admitMu. If either panics at -vv and the panic is recovered upstream (rc job handler turns panics into errors), admitMu is never released: every later Commit blocks forever on Lock, and Shutdown blocks forever at line 243, hanging process exit in the atexit handler.
Fix: —

### Item 3
Location: lib/batcher/batcher.go:274
Claim: fs.Debugf (object String() plus log formatting and I/O) runs inside the admission critical section on the upload hot path.
Consequence: With -vv and a slow log sink (blocked stderr pipe, syslog, slow --log-file), every concurrent Commit serializes on the log write while holding admitMu, and Shutdown has to queue behind them. The log call can move before admitMu.Lock() so that only the closed check and the send are under the lock.
Fix: —

### Item 4
Location: lib/batcher/batcher_test.go:279
Claim: The test assumes the ambient log level is below INFO; otherwise Shutdown's fs.Infof(b.f, ...) calls blocker.String() and blocks in once.Do, so Shutdown never contends with the paused Commit.
Consequence: If the global config LogLevel is INFO or DEBUG when the test starts, oldLogLevel is restored at line 279 and Shutdown's Infof formats b.f, blocking in sync.Once.Do until release is closed. Shutdown therefore never reaches close(b.closed) or the quit send while Commit is paused, the interleaving under test does not happen, and the test can pass on the unfixed code. It should set an explicit level below INFO rather than restoring the old one.
Fix: —

### Item 5
Location: lib/batcher/batcher_test.go:29
Claim: The regression test depends on the exact position of an incidental debug log line in Commit to pause between the closed check and the send.
Consequence: A legitimate refactor that moves fs.Debugf before admitMu.Lock() (the fix for the logging-under-lock issue) makes Shutdown complete first, so Commit returns 'shutting down' and require.NoError fails. Removing the log line makes the test fail with 'commit did not reach the admission point'. In both cases the test breaks for reasons unrelated to the race it guards; a test hook or seam would be less fragile than a Stringer side effect.
Fix: —

### Item 6
Location: lib/batcher/batcher_test.go:287
Claim: The contention window is a fixed 100ms wait that never confirms Shutdown actually reached the mutex, and the 1s result timeouts are tight for a loaded -race CI run.
Consequence: With the fix, b.closed cannot close while Commit is paused, so the select always sleeps the full 100ms per mode and the b.closed case is dead on fixed code. If the Shutdown goroutine is not scheduled within 100ms on a loaded runner, the test degenerates into sequential commit-then-shutdown and passes without exercising the race. Conversely, a slow runner can trip the 1s timeouts and report a spurious 'commit hung' or 'shutdown hung'.
Fix: —

### Item 7
Location: lib/batcher/batcher_test.go:276
Claim: Failure paths leak a blocked goroutine and a live batcher because blocker.release is never closed and Shutdown is never called when the test aborts early.
Consequence: If t.Fatal fires at line 276, or the sync case fails at line 297, the Commit goroutine stays blocked forever (in String() holding admitMu, or on <-resp). In the line 276 case the batcher's commitLoop goroutine and atexit registration are also left behind for the rest of the test binary. A t.Cleanup that closes release and calls b.Shutdown() would avoid this.
Fix: —

### Item 8
Location: lib/batcher/batcher_test.go:245
Claim: The test mutates the process-global config LogLevel to DEBUG, read without synchronisation by every fs.Debugf/Infof call.
Consequence: The write at 245, the second write at 279 and the deferred restore all touch the shared ConfigInfo returned by fs.GetConfig. It is race-free today only because no other goroutine logs at those instants; if these tests gain t.Parallel() or a background goroutine from another test is still logging, the race detector will flag it. It also prints debug output into the test log. The mid-test restore at 279 plus the deferred restore is redundant state handling.
Fix: —

### Item 9
Location: lib/batcher/batcher.go:50
Claim: An exclusive sync.Mutex serializes all Commit callers against each other when only Commit-versus-Shutdown exclusion is needed.
Consequence: All concurrent uploads finishing at once funnel through one exclusive lock that is held across a potentially blocking send, which adds contention and couples admission latency to the slowest sender. A sync.RWMutex (RLock in Commit, Lock in Shutdown), or draining b.in after the quit marker and replying to stragglers with the shutdown error as suggested in the issue, gives the same guarantee without serializing admitters.
Fix: —
