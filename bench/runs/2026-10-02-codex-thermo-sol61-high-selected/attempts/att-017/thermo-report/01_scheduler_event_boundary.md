# Scheduler event boundary

## Scope and judgment

This subsystem review covers the import removal, `noteLengthLimit` declaration, and `truncateMessage` rewrite in `pkg/scheduler/schedule_one.go`, plus the existing event call sites and validation contract needed to assess them. There are no actionable findings in this subsystem.

The old code imported `k8s.io/kubernetes/pkg/apis/core/validation` solely to obtain `NoteLengthLimit`. The reviewed code gives the event-producing client a documented conservative limit, instead of coupling its implementation to the server's validation package. The server retains ownership of acceptance policy. A client can impose a stricter output limit without implementing or owning server validation.

## Source evidence and measurements

`pkg/apis/core/validation/events.go:33–38` defines `NoteLengthLimit = 1024`. Its legacy event validator rejects `len(event.Message) > NoteLengthLimit` at lines 183–184 for events with EventTime set. `staging/src/k8s.io/api/events/v1/types.go:81–85` documents a maximal note length of 1kB and advises libraries to handle values up to 64kB. That advice concerns handling received values; it does not require the scheduler to emit longer notes.

`pkg/scheduler/schedule_one.go:58–61` defines the same untyped integer constant and explains both its source and the compatibility rationale. The future non-decreasing validation assertion is the comment's stated design premise, not something proved by the focused tests. At this pinned revision, the public contract and concrete validator both substantiate the value independently of that future premise.

At `schedule_one.go:838–845`, only the failure Event message goes through `truncateMessage`; the PodScheduled condition keeps `errMsg`. Neither call site changes. The successful scheduling Event at line 719 is likewise unchanged. This refactor does not expand truncation into shared scheduling flows.

The before-and-after helper behavior is exact for every Go string at the pinned revisions. Up to and including 1024 bytes, it returns the input unchanged. Above 1024 bytes, it returns `message[:1020] + " ..."`, which has 1024 bytes. `len` and slicing operate on bytes in both revisions, and the suffix is unchanged. Removing `max := validation.NoteLengthLimit` changes neither the threshold, suffix, index, nor result type. There is no new underflow or panic path because the over-limit branch implies an input longer than the constant slice index.

The source file measures 890 lines at base and 892 at head. The diff adds a four-line constant declaration/comment, removes the import and intermediate variable, and updates the existing expressions and comment. The helper keeps one `if`, one return for short strings, and one return for long strings. It adds no dependency injection, generic parameter, policy object, nullable contract, mutable state, asynchronous sequencing, or partial update. The skill's 1000-line threshold is not crossed.

## Worked code-judo analysis

The useful structural move is the one already made: remove the validation package from the client's direct dependency boundary, and keep the conservative producer limit explicit. The resulting algorithm is already small and direct:

```go
const noteLengthLimit = 1024

func truncateMessage(message string) string {
    if len(message) <= noteLengthLimit {
        return message
    }
    suffix := " ..."
    return message[:noteLengthLimit-len(suffix)] + suffix
}
```

The PR also keeps the compatibility explanation beside the constant; the abbreviated example above is an analysis illustration, not a request to delete that documentation. No change to this implementation is proposed.

One alternative would move the canonical constant into a public API or component helper, re-export it from validation, and import it in the scheduler. That would replace numeric duplication with a shared exported contract, but it would add a new public symbol and migrate a server implementation detail across module boundaries. The inspected public Event API and client-go event helpers have no existing exported note-limit constant or matching truncation utility to reuse. A project-wide search for `NoteLengthLimit` and `noteLengthLimit` found only the validator and the new scheduler declaration and use sites. A shared-contract migration is not an obvious simplification for this two-file change: it creates more ownership and migration work than it deletes.

Another alternative would centralize truncation in the event recorder and delete the scheduler helper. `staging/src/k8s.io/client-go/tools/events/event_recorder.go:72–128` formats the note and forwards it into the Event without applying this scheduler truncation policy. The legacy adapter in `tools/record/event.go:175–182` also forwards the note. Changing those shared producers would affect other clients and would require separate behavior decisions and coverage. It cannot be justified as a behavior-preserving local cleanup here.

Extracting this eight-line helper and its constant into a new file or package would not eliminate a concept or branch, and the affected file remains below the skill's threshold. Generalizing it to a limit parameter or policy object would add indirection for one production caller. Restoring the server validation import would undo the architectural improvement. These alternatives were evaluated and rejected as unnecessary; they are not actionable findings or deferred remediation requests.

## Commands and verification status

The source evidence was obtained with `git diff main...review-head`, `git diff --numstat main...review-head`, numbered reads of `schedule_one.go`, reads of `pkg/apis/core/validation/events.go` and the public Event type, and `rg -n 'NoteLengthLimit|noteLengthLimit' --glob '*.go' --glob '!vendor/**'`. File measurements came from:

```sh
git show main:pkg/scheduler/schedule_one.go | wc -l
wc -l pkg/scheduler/schedule_one.go pkg/scheduler/.import-restrictions
```

The focused test was executed from the clone root with the following environment. Here `CACHE` abbreviates `/home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high-selected/att-017/clone-cache` in the reproduction command; the actual invocation used absolute paths.

```sh
timeout 300s env \
  GOROOT="$CACHE/toolchain" \
  GOMODCACHE="$CACHE/gomodcache" \
  GOCACHE="$CACHE/gocache" \
  GOFLAGS=-mod=vendor GOPROXY=off GOSUMDB=off GOTOOLCHAIN=local \
  "$CACHE/toolchain/bin/go" test ./pkg/scheduler \
  -run '^(TestSchedulerScheduleOne|TestSchedulerFailedSchedulingReasons|TestHandleSchedulingFailure.*)$' \
  -count=1 -timeout=4m
```

The command exited zero and returned `ok k8s.io/kubernetes/pkg/scheduler 18.461s`. Compilation and the selected existing scheduling/failure-handler tests passed. The tests use fake clients and event facilities and do not establish API-server acceptance at every string boundary. The exhaustive equivalence assertion above is a source-level deduction from the identical constant values and operations, not a claim that an exhaustive test was executed.

The checked-in Go workspace was preserved. No dependencies were fetched, no tests were added, and no remedy or source edit was applied. Pre-existing byte slicing and format-string handling were considered when assessing equivalence; neither changed and neither supplies a PR-introduced finding. No additional remediation or question remains for this subsystem.
