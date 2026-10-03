# Detail: scheduler note-length limit (`pkg/scheduler`)

Covers both files in the range: `pkg/scheduler/schedule_one.go` and `pkg/scheduler/.import-restrictions`.

All Go commands were run from the clone root with the packet's toolchain and environment (`GOFLAGS=-mod=vendor GOPROXY=off GOSUMDB=off GOTOOLCHAIN=local`). Nothing was written to the clone; `git status --porcelain` was empty afterwards.

## What the diff does

`git diff main...review-head --stat` reports two files, 7 insertions and 7 deletions.

In `schedule_one.go` the import of `k8s.io/kubernetes/pkg/apis/core/validation` is removed, a new unexported constant is added to the existing `const` block at lines 58-61, and `truncateMessage` (lines 851-858) uses it:

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

In `.import-restrictions` the allow rule for `^k8s[.]io/kubernetes/pkg/apis/core/validation$` is deleted, so the catch-all `forbiddenPrefixes` rule at the bottom of the file now covers it.

## Measurements

| Check | Command | Result |
| --- | --- | --- |
| File size | `wc -l pkg/scheduler/schedule_one.go`; `git show main:pkg/scheduler/schedule_one.go \| wc -l` | 892 after, 890 before |
| Package builds | `go build ./pkg/scheduler/` | success |
| Transitive dependency gone | `go list -deps ./pkg/scheduler/ \| grep -c 'k8s.io/kubernetes/pkg/apis/core/validation$'` | 0 |
| No other importer in the tree, tests included | `go list -test -f '{{.ImportPath}}: {{join .Imports " "}} {{join .TestImports " "}} {{join .XTestImports " "}}' ./pkg/scheduler/...` filtered for the validation path | no package listed |
| Text search agrees | `grep -rn 'pkg/apis/core/validation' pkg/scheduler` | three comments and the new constant's comment; no import |
| Focused test | `go test ./pkg/scheduler/ -run 'TestSchedulerScheduleOne$' -count=1` | `ok k8s.io/kubernetes/pkg/scheduler 18.191s` |
| Whitespace | `git diff main...review-head --check` | clean |

After the change the scheduler root package's direct k/k imports outside `pkg/scheduler` are `pkg/api/v1/pod`, `pkg/apis/scheduling` and `pkg/features`.

`hack/verify-import-boss.sh` was not executed. The claim that the removed rule is dead rests on the two import-list checks above, which cover production, test and external-test imports for every package under `pkg/scheduler`.

## Is the "safe even if stale" argument correct?

Yes. The server-side check is in `pkg/apis/core/validation/events.go:183`:

```go
		if len(event.Message) > NoteLengthLimit {
			allErrs = append(allErrs, field.Invalid(field.NewPath("message"), "", fmt.Sprintf("can have at most %v characters", NoteLengthLimit)))
		}
```

Both sides count bytes with `len`. If the server limit were raised, the scheduler would truncate to 1024 and the event would still be accepted. Only a lowered server limit would cause rejections, and that would break every existing client of the events API, not just the scheduler.

The comment's phrase "notes the scheduler records" is also accurate. In `handleSchedulingFailure` (lines 838-846) the truncated string is used only for the event; the `PodScheduled` condition gets the untruncated `errMsg`.

## Finding 1 evidence: nothing tests the limit

`grep -rn truncateMessage pkg/scheduler` returns the definition, the one production call at line 838, and two test references:

- `schedule_one_test.go:2098`, inside a replacement `sched.FailureHandler` in `TestSchedulerScheduleOne`
- `schedule_one_test.go:5062`, inside the replacement failure handler in the shared test scheduler setup

Both call `truncateMessage` to produce the message they then emit. Neither asserts a length or a suffix, and both would produce the same pass/fail result for any value of the constant.

`grep -rnI 'NoteLengthLimit\|noteLengthLimit' --include='*.go'` across the non-vendored tree returns only `pkg/apis/core/validation/events.go` (declaration and the check quoted above) and the lines this PR adds. So there is no test anywhere that relates the two numbers.

Worked proposal, in the style of the surrounding table tests:

```go
func TestTruncateMessage(t *testing.T) {
	tests := []struct {
		name    string
		length  int
		wantLen int
		wantCut bool
	}{
		{name: "below limit", length: noteLengthLimit - 1, wantLen: noteLengthLimit - 1},
		{name: "at limit", length: noteLengthLimit, wantLen: noteLengthLimit},
		{name: "above limit", length: noteLengthLimit + 1, wantLen: noteLengthLimit, wantCut: true},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := truncateMessage(strings.Repeat("a", tt.length))
			if len(got) != tt.wantLen {
				t.Errorf("got length %d, want %d", len(got), tt.wantLen)
			}
			if gotCut := strings.HasSuffix(got, " ..."); gotCut != tt.wantCut {
				t.Errorf("truncation suffix present = %v, want %v", gotCut, tt.wantCut)
			}
		})
	}
}
```

This pins behaviour but still uses the scheduler's own constant. A direct comparison with `validation.NoteLengthLimit` cannot live in `pkg/scheduler` because the import rules apply to test files too (the same file allows `test/utils/ktesting` and a `persistentvolume/testing` package explicitly). `test/integration/.import-restrictions` only restricts `test/e2e` imports, so an integration test may import the validation package. `noteLengthLimit` is unexported, so such a test would have to assert on the recorded event's note length rather than on the constant.

## Finding 2 evidence: where the constant belongs

The API type documents the limit in prose at `staging/src/k8s.io/api/events/v1/types.go:81-85`:

```go
	// note is a human-readable description of the status of this operation.
	// Maximal length of the note is 1kB, but libraries should be prepared to
	// handle values up to 64kB.
	// +optional
	Note string `json:"note,omitempty" protobuf:"bytes,10,opt,name=note"`
```

Precedent for exported limit constants in `k8s.io/api`:

- `core/v1/types.go:3726-3728`: `ResourceHealthMessageMaxLength = 1024`, documented as "Messages longer than this will be truncated"
- `core/v1/types.go:8403`: `MaxSecretSize = 1 * 1024 * 1024`
- `resource/v1/types.go`: `DriverNameMaxLength`, `PoolNameMaxLength`, `DeviceTaintsMaxLength`, `DeviceRequestsMaxSize` and others

Worked proposal:

```go
// staging/src/k8s.io/api/events/v1/types.go
// EventNoteMaxLength is the maximum length in bytes of Event.Note accepted by the API server.
const EventNoteMaxLength = 1024
```

```go
// pkg/apis/core/validation/events.go
NoteLengthLimit = eventsv1.EventNoteMaxLength
```

```go
// pkg/scheduler/schedule_one.go
if len(message) <= eventsv1.EventNoteMaxLength {
```

`schedule_one.go` does not currently import `k8s.io/api/events/v1`, so this adds one staging import and removes the local constant and its three-line justification. `pkg/apis/core/validation/events.go` already imports `k8s.io/api/events/v1beta1` and `k8s.io/api/core/v1`, so the validation side gains no new module dependency.

The deeper variant is to truncate in the recorder. `staging/src/k8s.io/client-go/tools/events/event_recorder.go:72-74` builds the note with `fmt.Sprintf(note, args...)` and passes it to `makeEvent` unchanged; a case-insensitive search of `client-go/tools/events` and `client-go/tools/record` for truncation or a note-length limit finds nothing. The scheduler is the only in-tree consumer of `NoteLengthLimit` outside the validator, so other users of this recorder send overlong notes as they are. Truncating once in the recorder would remove `truncateMessage` and its constant from the scheduler. That is a behaviour change in a published library and is out of scope for a dependency-trimming PR.

Why this is not a blocker: the PR is one step in a series (`git log -- pkg/scheduler/.import-restrictions` shows the lock-down commit `8210ce334f2` followed by this one), the local copy is correct, and both alternatives need API or client-go review. The cost of the local copy is one duplicated literal and Finding 1.

## Finding 3 evidence: commit message

`git log main..review-head --format='%s%n%b'`:

```
scheduler: replace pkg/apis/core/validation dependency with local const
pkg/scheduler only uses k8s.io/kubernetes/pkg/apis/core/validation for
NoteLengthLimit, for which a local copy remains safe even if stale.

Part of removal of k/k imports in pkg/scheduler to support move to staging.

# Conflicts:
#	pkg/scheduler/schedule_one.go
```

The last two lines are git's conflict template text.

## Pre-existing observations, not findings against this PR

Format-string use. `schedule_one.go:839` calls `Eventf(pod, nil, v1.EventTypeWarning, "FailedScheduling", "Scheduling", msg)`, with `msg` in the `note` format position and no arguments. The recorder evaluates `fmt.Sprintf(note, args...)`. A `%` in a scheduling error message would be interpreted as a verb and rendered as a `%!x(MISSING)`-style token, which is both wrong text and longer than the input, so the result could exceed the limit the truncation just enforced. Passing `"%s", msg` would avoid it. I did not construct a scheduling error containing `%` to demonstrate this; it follows from reading the two call sites.

Byte slicing. `message[:noteLengthLimit-len(suffix)]` cuts at a byte offset, so a multi-byte UTF-8 character straddling byte 1020 would be split. The validator also counts bytes, so the length is right, but the note could contain an invalid sequence.

Comment wording. "truncates a message if it hits the noteLengthLimit" describes the function loosely: a message exactly at the limit is returned unchanged. The PR only renamed the identifier in that sentence.

## Checklist against the skill's presumptive blockers

- Incidental complexity preserved where a judo move would delete it: a move exists (Finding 2) but it sits in other modules and needs separate review; not a blocker here.
- File pushed past 1000 lines: no, 890 to 892.
- Ad-hoc branching added: none.
- Feature checks scattered across shared code: none.
- Unnecessary abstraction, wrapper or cast: none added; one shadowing local removed.
- Duplicated helper or wrong layer: one duplicated literal, justified in a comment; see Finding 2.
