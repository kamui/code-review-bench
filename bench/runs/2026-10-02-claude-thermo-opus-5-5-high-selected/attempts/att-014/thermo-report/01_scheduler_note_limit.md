# Detail 01: scheduler note-length limit and import restrictions

Subsystem: `pkg/scheduler` (root package) — `schedule_one.go`, `.import-restrictions`.
Range: `6bb42350227f..d9cf69d74a25`. Model: `claude-opus-5-5` at `high`. No alternate-model review.

## The change in full

`pkg/scheduler/.import-restrictions` loses the two-line allow rule for `^k8s[.]io/kubernetes/pkg/apis/core/validation$`.

`pkg/scheduler/schedule_one.go` drops the `k8s.io/kubernetes/pkg/apis/core/validation` import, adds a private constant to the leading `const` block, and rewrites `truncateMessage` to use it:

```go
	// noteLengthLimit is a local copy of NoteLengthLimit in k8s.io/kubernetes/pkg/apis/core/validation, which the API
	// server enforces on notes the scheduler records. A local copy is safe because API validation limits will never
	// become lower than an already released version.
	noteLengthLimit = 1024
```

```go
// truncateMessage truncates a message if it hits the noteLengthLimit.
func truncateMessage(message string) string {
	if len(message) <= noteLengthLimit {
		return message
	}
	suffix := " ..."
	return message[:noteLengthLimit-len(suffix)] + suffix
}
```

## Measurements

File size: `schedule_one.go` is 890 lines on `main` and 892 at head (`git show main:pkg/scheduler/schedule_one.go | wc -l`, `wc -l pkg/scheduler/schedule_one.go`). No threshold is crossed.

Branching: zero new conditionals. `truncateMessage` keeps its one `if`; the `max` local (which shadowed the Go builtin) is removed.

Concept count: one import and one allow-list rule deleted, one private constant added. Net coupling goes down: the root scheduler package no longer reaches `pkg/apis/core/validation` at all.

## Verification performed

All Go commands ran from the clone root with the provided toolchain and `GOFLAGS=-mod=vendor GOPROXY=off GOSUMDB=off GOTOOLCHAIN=local`. The clone was not modified (`git status --short` empty afterwards).

Build and vet. `go build ./pkg/scheduler/` succeeded and `go vet ./pkg/scheduler/` produced no output. Status: verified.

Dependency really removed. `go list -deps ./pkg/scheduler/ | grep -c 'k8s.io/kubernetes/pkg/apis/core/validation$'` prints `0`. The validation package is absent from the transitive closure, so no other allowed import (`pkg/api/v1/pod`, `pkg/apis/core/v1/helper`, `pkg/apis/scheduling`, `pkg/controller`, `pkg/features`) drags it back in. Only six non-scheduler `k8s.io/kubernetes/pkg/...` packages remain in the closure. Status: verified.

Allow-rule removal is safe. `.import-restrictions` ends in a catch-all `^k8s[.]io/kubernetes` forbid rule, so deleting an allow rule breaks `import-boss` for any remaining importer in the subtree. `go list -test ./pkg/scheduler/...` lists 147 packages and test variants; formatting each with `.Imports`, `.TestImports` and `.XTestImports` and grepping for the validation path yields zero matches. A text search of `pkg/scheduler` for `pkg/apis/core/validation` finds only comments (`schedule_one.go:58`, `apis/config/validation/validation_pluginargs.go:100`, and two in `framework/plugins/dynamicresources/nodeallocatabledynamicresources.go`). `cmd/import-boss/main.go:86` loads packages with `Tests: true`, so tests are in scope for the rule, and they were in scope for this check. Status: verified by equivalent means; `hack/verify-import-boss.sh` itself was not run because it writes build output into the tree.

Value parity. `pkg/apis/core/validation/events.go:37` defines `NoteLengthLimit = 1024`; `pkg/scheduler/schedule_one.go:61` defines `noteLengthLimit = 1024`. Status: verified by inspection.

Comment accuracy. The comment says the API server enforces the limit on notes the scheduler records. `events.go:183` rejects `len(event.Message) > NoteLengthLimit` for request versions other than core `v1` and `events.k8s.io/v1beta1`; the scheduler records through the `events.k8s.io/v1` recorder (`podFwk.EventRecorder()...Eventf`, `schedule_one.go:839`), whose `Note` maps to the internal `Message`. The claim is correct. The safety argument (limits never tighten after release, so a stale copy can only over-truncate) is also correct and is the only argument needed. Status: verified by reading.

Tests. `go test ./pkg/scheduler/ -run 'TestSchedulerScheduleOne$|TestSchedulerBinding$' -count=1` reports `ok  k8s.io/kubernetes/pkg/scheduler 18.140s`. These exercise `truncateMessage` on short messages via `schedule_one_test.go:2098` and `:5062`. Status: verified. The full package suite was not run (outside the allowance).

## Was there a canonical home to reuse instead of copying?

No. A tree-wide search for `NoteLengthLimit` outside `vendor/` returns only its definition and two uses in `pkg/apis/core/validation/events.go`, plus changelog text. No staging module exports the value: `staging/src/k8s.io/api/events/v1/types.go:82` says only "Maximal length of the note is 1kB" in a doc comment, and `staging/src/k8s.io/client-go/tools/events` has no length constant or truncation. A drift-guard unit test asserting `noteLengthLimit == validation.NoteLengthLimit` cannot live in `pkg/scheduler` either, because the import restriction applies to tests. Given the monotonic-limit argument, a guard is not needed. The duplication is therefore justified and is not raised as a finding.

## Finding 1: constant placement

Evidence: the constant is at `schedule_one.go:58-61`; its single consumer is `truncateMessage` at `:851-858`; the single production call is `:838`. The other members of that `const` block are `pluginMetricsSamplePercent`, `minFeasibleNodesToFind` and `minFeasibleNodesPercentageToFind`, all scheduling-cycle tunables.

Worked proposal — move the declaration to the point of use, leaving the tunables block untouched:

```go
// noteLengthLimit is a local copy of NoteLengthLimit in k8s.io/kubernetes/pkg/apis/core/validation, which the API
// server enforces on notes the scheduler records. A local copy is safe because API validation limits will never
// become lower than an already released version.
const noteLengthLimit = 1024

// truncateMessage truncates a message if it hits the noteLengthLimit.
func truncateMessage(message string) string {
	if len(message) <= noteLengthLimit {
		return message
	}
	suffix := " ..."
	return message[:noteLengthLimit-len(suffix)] + suffix
}
```

Same line count, same behaviour, and the reason for `1024` is read in the same screen as the arithmetic. Severity is low; this is legibility only.

## Finding 2: commit message residue

Evidence: `git log --format=%B main..review-head` ends with

```
# Conflicts:
#	pkg/scheduler/schedule_one.go
```

These lines are git's conflict template, normally stripped as comments but retained here (typical when the commit was completed with a non-default `core.commentChar`/cleanup mode or via a tool that passes `--cleanup=verbatim`). Remedy: `git commit --amend` and delete the two lines. Severity is low; no code impact.

## Question 1: ownership of note truncation

Evidence: `staging/src/k8s.io/client-go/tools/events/event_recorder.go:74` builds `message := fmt.Sprintf(note, args...)` and `:128` assigns it to `Note` with no length handling. In `pkg/scheduler`, five production `Eventf` call sites exist (`schedule_one.go:475`, `:719`, `:839`; `framework/preemption/executor.go:159`; `framework/plugins/dynamicresources/dynamicresources.go:1720`) and only `:839` truncates. Two test failure handlers re-implement the truncate-then-record sequence (`schedule_one_test.go:2098`, `:5062`).

The code-judo move, if the project wanted it, is to make the recorder responsible for the `Note` size it is about to submit. That deletes `noteLengthLimit`, `truncateMessage`, and the duplicated test-handler calls from the scheduler, and removes the need for any component to mirror a validation constant. It changes client-go behaviour for every consumer and needs API-machinery sign-off, so it is posed as a question about the intended end state rather than a request on this PR.

## Out-of-scope observations (pre-existing, unchanged by this PR)

Byte-offset truncation. `message[:noteLengthLimit-len(suffix)]` slices by byte and can split a multi-byte UTF-8 sequence if a plugin or extender message contains non-ASCII text. The behaviour is identical on `main`.

No boundary test. A search of `schedule_one_test.go` finds no message longer than the limit being passed to `truncateMessage` and no assertion on the truncated length or suffix. Also identical on `main`.

Neither was introduced or worsened by the change under review.
