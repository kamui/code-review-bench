# Regression-test orchestration

## Scope and verdict

Reviewed the complete batcher test file and the logging path used by the new
blockingStringer. The new test exercises both synchronous hangs and asynchronous
dropped requests, but its shutdown-release handshake permits a false negative.
This is the sole actionable finding in the summary.

## Finding and source evidence

### [P2] Wait for shutdown completion before releasing the closed-admission path

At `lib/batcher/batcher_test.go:287–291`, the select either observes closure of
`b.closed` or allows 100 ms for shutdown to contend, then immediately releases
the Commit blocked in String. The closed-channel arm does not wait for the quit
marker. Shutdown's source order is close at `batcher.go:245`, marker send at
line 251, and worker wait at line 253. The marker is therefore a separate event
from admission closure, with an ordinary scheduler preemption point between
them. The new test reintroduces a race between the two sends at that point.

Here is a valid execution against the unfixed production code:

1. Commit passes the closed check and blocks in blockingStringer.String.
2. Shutdown closes b.closed and is descheduled before its marker send.
3. The test receives from b.closed and closes blocker.release.
4. Commit sends its request; the consumer processes it. A synchronous caller
   gets its result, and an asynchronous caller's accepted item is committed.
5. Shutdown resumes, sends its marker, and finishes. The test observes all
   three success signals, despite production still permitting sends after a
   quit marker on another schedule.

The pause in the logging identity is precise and useful: it reliably places
Commit after its admission check. The defect is in what the test observes on
the shutdown side. It uses an intermediate notification where it needs proof
that shutdown has actually passed its terminal marker.

## Executed verification

All commands ran once per distinct flag set from the clone root, offline, with
the cache environment recorded in 01_admission_and_shutdown.md and a 300-second
command limit. No target source was edited.

Scratch evidence directory:
`/home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high/att-025/clone-work/review-evidence`.
The base source was obtained directly with
`git show main:lib/batcher/batcher.go`. A Go overlay replaced only that source
file while retaining the head's test file.

```sh
timeout 300s go test -count=1 -race \
  -run '^TestBatcherCommitRacingShutdown$' \
  -overlay=/home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high/att-025/clone-work/review-evidence/base-overlay.json \
  ./lib/batcher
```

This exited 1 and produced:

```text
--- FAIL: TestBatcherCommitRacingShutdown (2.01s)
    --- FAIL: TestBatcherCommitRacingShutdown/sync (1.01s)
        batcher_test.go:297: commit hung while racing shutdown
    --- FAIL: TestBatcherCommitRacingShutdown/async (1.01s)
        batcher_test.go:307: accepted commit was dropped during shutdown
FAIL github.com/rclone/rclone/lib/batcher 2.052s
```

A second scratch source contained exactly that base implementation with a
`time.Sleep(10 * time.Millisecond)` immediately after `close(b.closed)`.
The existing time import was reused. This imposes a scheduling pause at the
identified preemption point; it does not add the missing admission mutex or
fix the original send-ordering bug.

```sh
timeout 300s go test -count=1 -race \
  -run '^TestBatcherCommitRacingShutdown$' \
  -overlay=/home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high/att-025/clone-work/review-evidence/base-delayed-marker-overlay.json \
  ./lib/batcher
```

This exited zero and reported:

```text
ok github.com/rclone/rclone/lib/batcher 1.049s
```

Both modes passed on production code without the fix. This verifies the
false-negative interleaving, rather than merely conjecturing a loaded-runner
timing problem. It does not measure how often an uninstrumented scheduler would
select that interleaving. The unchanged head passed the full race-enabled
package suite separately.

## Worked code-judo proposal

Use the existing shutdownDone channel as the authoritative completion signal
in the arm that detects closed admission. Replace the current contention select
with the following structure, retaining the release afterward:

```go
select {
case <-b.closed:
    select {
    case <-shutdownDone:
    case <-time.After(time.Second):
        t.Fatal("shutdown did not complete after closing admission")
    }
case <-time.After(100 * time.Millisecond):
}
close(blocker.release)
```

On the fixed head, Commit still holds admitMu while paused, so Shutdown cannot
close b.closed. The existing contention timeout releases Commit and the normal
success assertions remain applicable. On the base, shutdown can close admission
while Commit is paused; waiting for shutdownDone ensures its marker has been
consumed and its loop has exited before the request is released. The synchronous
hang or asynchronous drop then becomes the forced schedule even when shutdown
pauses before its marker send.

This proposal reuses a completion channel already in the test. It does not
introduce a production hook, new generic helper, or runtime scheduling primitive
into the shipping code. It adds one local bounded wait at the boundary whose
ordering the test actually needs to prove. Merely lengthening the existing
100 ms timeout would not repair the closed-channel arm's missing ordering.

The proposal has been checked against the source-level lock and channel order;
it was not applied or executed. Remediation verification should run the fixed
head and both scratch base schedules with the revised test.

## Other quality checks

The new test adds 89 lines to a 275-line test file. It stays in the canonical
package and uses a table over the two actual batch modes rather than duplicated
test bodies. No file decomposition or additional mode abstraction is justified.

blockingStringer's sync.Once bounds its pause to the first formatting call.
The test temporarily enables the global debug guard to reach that call and
restores the previous log level before starting shutdown. Under this package's
default Notice configuration, shutdown's Info log is disabled, so it does not
contend on the stringer's Once before reaching the admission protocol. The
test's dependence on this log location is explicit in its comment and did
successfully expose both failures in the unchanged-base run.

The test uses sequential subtests, with no t.Parallel. The first debug-level
read precedes closing blocker.started, and the test observes that channel
before restoring the level. Normal completion waits for shutdown before the
deferred final restoration. The race-enabled head run found no data race in
this setup. These observations do not erase the separately reproduced
channel-ordering weakness.

No additional actionable wrapper, cast, optionality, canonical-helper,
boundary, or file-size issue was found. The requested change is confined to
the regression test's shutdown handshake.
