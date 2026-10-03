# Scheduler event-note limit

This detail report covers `pkg/scheduler/schedule_one.go` in the committed range `6bb42350227f0a714f4730165e7ba622c69bd99e..d9cf69d74a25a209e79c7061ff86a14ab4b4b634`. It applies the frozen thermo-nuclear code-quality skill to the implementation and its immediate callers. The checkout was not edited, and no additional reviewer was used.

## Judgment and source evidence

There are no actionable findings in this subsystem. The new package-private `noteLengthLimit` at `pkg/scheduler/schedule_one.go:58–61` replaces a dependency on the internal core-validation package with the same numeric value, 1024. The comment identifies the source of the number and explains the compatibility assumption. It is a producer ceiling for scheduler event notes, not a reimplementation of API validation.

The pinned server value is `NoteLengthLimit = 1024` at `pkg/apis/core/validation/events.go:37`. The server compares `len(event.Message)` to that limit at lines 183–184. The public event type independently documents the note's maximum length as 1kB at `staging/src/k8s.io/api/events/v1/types.go:81–85`. Both the old and new scheduler implementations therefore use the same byte-based ceiling as the pinned validator. No lightweight shared numeric constant was found in the inspected public event APIs or client-go events code.

`truncateMessage` at `pkg/scheduler/schedule_one.go:852–858` retains one length comparison, one early return, and one slice-plus-suffix expression. Removing `max := validation.NoteLengthLimit` changes neither the type of the evaluated bound nor its value. For every Go string input, substituting 1024 for both old occurrences of `max` gives exactly the new function. This is a direct equivalence argument, including empty strings, boundary inputs, long inputs, and multibyte strings.

For an input of at most 1024 bytes, both versions return the original string. For a longer input, both versions return its first 1020 bytes followed by the four-byte suffix `" ..."`, yielding 1024 bytes. The existing byte-slicing behavior can split a multibyte encoding; that behavior is unchanged by this PR and is not raised as a new finding. No rune-based rewrite is proposed because it would change behavior and is outside the dependency change.

The production caller at lines 838–845 continues to truncate only the warning event's note and retain the full error message in the pod condition. The caller itself is unchanged. `staging/src/k8s.io/client-go/tools/events/event_recorder.go:74` formats the note and line 128 assigns it to the event. The inspected recorder does not supply an equivalent truncation helper that this PR should reuse. This assessment establishes helper equivalence; it does not claim that unchanged event formatting handles every possible diagnostic string correctly.

## Structural measurements

`git diff --numstat main...review-head` reports seven additions and five deletions in `schedule_one.go`; the other changed file loses two lines. The scheduler file grows from 890 lines at base to 892 at head. It remains below the skill's 1000-line threshold, with no threshold crossing or unjustified file-size explosion. `truncateMessage` shrinks from eight lines to seven, counting its function declaration through closing brace. The new constant and comment live in the existing constants block rather than introducing another file or module.

The change introduces no conditional, control-flow mode, mutable state, optional value, wrapper, cast, generic mechanism, asynchronous orchestration, or state update. The helper's branch count stays at one. Scheduling failure handling and pod status updates are unchanged. There is no new feature check scattered through shared scheduling paths, and no new atomicity issue to remedy.

## Worked code-judo proposals

The strongest simplification for this scope is the one the PR already performs. The old implementation imports a package of server-side validators just to obtain one integer. The new implementation declares the scheduler's conservative event-output ceiling locally and removes that dependency and its policy exception. Its essential shape is:

```go
const noteLengthLimit = 1024 // Explained by the existing provenance comment.

func truncateMessage(message string) string {
    if len(message) <= noteLengthLimit {
        return message
    }
    suffix := " ..."
    return message[:noteLengthLimit-len(suffix)] + suffix
}
```

This snippet illustrates the submitted structure; it is not a requested replacement or an applied remedy. It leaves the real invariant visible at the use site and removes the intermediate `max` binding.

A shared public event-limit constant was considered. In a change with several production consumers or an existing public constant, that could establish useful ownership. Here it would require changing the server validator, public or shared packaging, and the scheduler to replace one fixed producer ceiling. The inspected code has no such existing constant. That alternative adds a cross-package contract without removing a branch or meaningful implementation complexity beyond what the PR already deletes, so it is not an obvious missed simplification.

Moving truncation into the client-go recorder was also considered. It would change a shared recorder's output for all users rather than preserving this scheduler-specific policy. Its effect on event formatting, identity, and consumers would require a separate behavioral review. It is not a behavior-preserving restructuring of this narrow PR.

Extracting the constant and helper into a new scheduler utility file or package would similarly move seven lines without simplifying the model. There is one production caller, no new size threshold crossing, and no duplicated helper introduced by the diff. Decomposing the much larger scheduling workflow is not justified as a prerequisite for this two-line net file growth.

The local copy is intentionally asymmetric with server validation: if a later server accepts larger notes, a scheduler emitting at most 1024 bytes remains within that byte limit. Exact equality to every future server maximum is unnecessary. The claim about future validation limits not decreasing is a compatibility assumption documented by the PR; future release policy was not independently verified, and no future-version test is claimed. At the pinned revisions, the public API documentation and validator agree with the local value.

## Verification status and reproducible commands

Source inspection and measurements used:

```sh
git diff main...review-head
git show main:pkg/scheduler/schedule_one.go | wc -l
wc -l pkg/scheduler/schedule_one.go
git show main:pkg/scheduler/schedule_one.go | sed -n '840,860p'
rg -n 'NoteLengthLimit|noteLengthLimit|truncateMessage' pkg staging/src/k8s.io/api staging/src/k8s.io/apimachinery
sed -n '1,220p' pkg/apis/core/validation/events.go
sed -n '72,90p' staging/src/k8s.io/api/events/v1/types.go
rg -n 'max.*[Mm]essage|[Mm]ax.*[Nn]ote|[Nn]ote.*[Ll]imit|[Nn]ote.*1024' staging/src/k8s.io/client-go/tools/events staging/src/k8s.io/api/events staging/src/k8s.io/api/core/v1
```

One exploratory search included the nonexistent `staging/src/k8s.io/code-generator/cmd/import-boss` path and returned exit code 2. The actual implementation was subsequently inspected at `cmd/import-boss`; the focused verification used that correct path. No conclusion relies on the nonexistent path.

The focused scheduler test command used exactly the cached-toolchain environment recorded in [the import-boundary detail](02_import_boundary.md), including vendored dependencies, disabled proxies, the local toolchain, and the preserved checked-in workspace. The Go command under the same 300-second outer limit was:

```sh
go test ./pkg/scheduler \
  -run '^(TestSchedulerScheduleOne|TestSchedulerFailedSchedulingReasons|TestHandleSchedulingFailure.*)$' \
  -count=1 -timeout=240s
```

It passed with exit code 0 and output `ok k8s.io/kubernetes/pkg/scheduler 18.176s`. The flag set was run once. These tests exercise scheduling and failure-handling paths; they are not claimed to be dedicated assertions of the 1024-byte boundary. Boundary preservation follows from the source equivalence above. No new tests, overlays, dependency downloads, live API server validation, race run, or full Kubernetes suite were used.

The scheduler import check also passed; its full invocation and interpretation are in [the import-boundary detail](02_import_boundary.md). `git diff --check main...review-head` passed. Before and after test execution, the checkout remained clean; tracked working-tree and index diffs were empty. HEAD remained `d9cf69d74a25a209e79c7061ff86a14ab4b4b634`, with tree `f352e4a83a44eebca7557612450597686a4ddb4d`.

## Remediation judgment

No remediation is required. Preserve the documented local ceiling, the existing direct truncation flow, and the matching removal of the import exception. There is no demonstrated defect or structural regression that would justify expanding this PR into recorder changes, an exported shared contract, or a scheduling-workflow decomposition.
