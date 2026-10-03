# 01 — Scheduler: event-note limit and import restrictions

Subsystem: `pkg/scheduler` (root package), specifically `schedule_one.go` and `.import-restrictions`.

Range: `6bb42350227f..d9cf69d74a25`, inspected with `git diff main...review-head`.

## What the change does

The diff touches two files. In `pkg/scheduler/schedule_one.go` it removes the import of `k8s.io/kubernetes/pkg/apis/core/validation`, adds `noteLengthLimit = 1024` with a three-line comment to the file-level const block (lines 58–61), and rewrites `truncateMessage` (lines 851–858) to use the new constant instead of a local `max := validation.NoteLengthLimit`. In `pkg/scheduler/.import-restrictions` it removes the rule that allowed `^k8s[.]io/kubernetes/pkg/apis/core/validation$`, so the catch-all "forbid any other k8s.io/kubernetes imports" rule now covers that package.

The commit message states the motivation: `pkg/scheduler` only used the validation package for `NoteLengthLimit`, a local copy stays safe even if stale, and this is part of removing k/k imports from `pkg/scheduler` so it can move to staging.

## Measurements

File size. `pkg/scheduler/schedule_one.go` is 890 lines at `main` and 892 lines at `review-head` (`git show main:pkg/scheduler/schedule_one.go | wc -l`, `wc -l pkg/scheduler/schedule_one.go`). The 1000-line rule is not in play.

Constant occurrences. Searching the non-vendored tree for `NoteLengthLimit|noteLengthLimit` gives the definition and its two uses in `pkg/apis/core/validation/events.go` (lines 37, 183, 184), the new copy and its three uses in `pkg/scheduler/schedule_one.go` (lines 58, 61, 851, 853, 857), and CHANGELOG mentions. There is no third copy, and no shared staging definition.

Consumers of the scheduler copy. `truncateMessage` has one production caller, `handleSchedulingFailure` at `schedule_one.go:838`, and two test callers at `schedule_one_test.go:2098` and `schedule_one_test.go:5062`. The constant is declared at line 61; the function that uses it starts at line 852.

Remaining importers of the validation package. A grep for `pkg/apis/core/validation` under `pkg/scheduler` returns only comments (two in `framework/plugins/dynamicresources/nodeallocatabledynamicresources.go`, one in `apis/config/validation/validation_pluginargs.go`, and the new comment in `schedule_one.go`). `go list -test` over `./pkg/scheduler/...` shows no package whose imports, test imports or external test imports include it. `go list -deps ./pkg/scheduler/` shows zero transitive occurrences from the root package.

Staging precedent. `staging/src/k8s.io/api/events/v1/types.go:81-83` documents the limit in prose only ("Maximal length of the note is 1kB, but libraries should be prepared to handle values up to 64kB"). The only constant block in that package is the group name in `register.go`. By contrast `staging/src/k8s.io/api/core/v1/types.go:8403` exports `MaxSecretSize`, an API size limit that validation consumes through the internal mirror `core.MaxSecretSize` (`pkg/apis/core/validation/validation.go:8095`), so exporting an API limit from `k8s.io/api` is an established pattern.

Recorder behaviour. `staging/src/k8s.io/client-go/tools/events/event_recorder.go:72-74` computes `message := fmt.Sprintf(note, args...)` and performs no length handling; a grep of `tools/events` and `tools/record` for `1024`, `1kB` or `runcat` finds nothing outside tests.

Server enforcement. `pkg/apis/core/validation/events.go:183` rejects `len(event.Message) > NoteLengthLimit` only on the strict path; `ValidateEventCreate` returns early for `v1` and `events.k8s.io/v1beta1` requests. The scheduler records through the `events.k8s.io/v1` recorder, so the comment's claim that the server enforces this on notes the scheduler records is accurate.

## Verification status

All commands were run from the clone root with the prescribed toolchain environment (`GOFLAGS=-mod=vendor GOPROXY=off GOSUMDB=off GOTOOLCHAIN=local`), each under five minutes, and the clone was left unmodified (`git status --porcelain` empty afterwards).

- `go build ./pkg/scheduler/` — passed.
- `go vet ./pkg/scheduler/` — passed.
- `go run ./cmd/import-boss ./pkg/scheduler/...` — exit 0, no violations, confirming the tightened restriction file is satisfied by the whole scheduler tree.
- `go test ./pkg/scheduler/ -run 'TestSchedulerScheduleOne|TestSchedulerNoPhantomPodAfterExpire|TestSchedulerFailedSchedulingReasons' -count=1` — `ok k8s.io/kubernetes/pkg/scheduler 18.224s`.

Not run: the full scheduler unit suite, integration tests, and `hack/verify-*` scripts other than the direct `import-boss` invocation; these are outside the focused-command allowance.

One gap worth stating plainly: no test pins the truncation length. `TestSchedulerFailedSchedulingReasons` asserts that the error text is not longer than 150 characters ("too spammy"), which never reaches the truncation branch, and there is no dedicated test of `truncateMessage`. That was equally true before the PR, when the compile-time reference to `validation.NoteLengthLimit` was the only link between the two values. After the PR nothing mechanical links them. Because of import-boss, a unit test inside `pkg/scheduler` cannot import the validation package to assert equality either, so the comment is the entire contract.

## Finding 1 — duplicated limit, declared far from its only consumer

### Evidence

Before:

```go
// truncateMessage truncates a message if it hits the NoteLengthLimit.
func truncateMessage(message string) string {
	max := validation.NoteLengthLimit
	if len(message) <= max {
		return message
	}
	suffix := " ..."
	return message[:max-len(suffix)] + suffix
}
```

After (lines 58–61 and 851–858):

```go
const (
	// Percentage of plugin metrics to be sampled.
	pluginMetricsSamplePercent = 10
	// minFeasibleNodesToFind is ...
	minFeasibleNodesToFind = 100
	// minFeasibleNodesPercentageToFind is ...
	minFeasibleNodesPercentageToFind = 5
	// noteLengthLimit is a local copy of NoteLengthLimit in k8s.io/kubernetes/pkg/apis/core/validation, which the API
	// server enforces on notes the scheduler records. A local copy is safe because API validation limits will never
	// become lower than an already released version.
	noteLengthLimit = 1024
)
...
// truncateMessage truncates a message if it hits the noteLengthLimit.
func truncateMessage(message string) string {
	if len(message) <= noteLengthLimit {
		return message
	}
	suffix := " ..."
	return message[:noteLengthLimit-len(suffix)] + suffix
}
```

### Why it matters

There are two separate issues folded into this finding, and they have different weights.

The lighter one is locality. The const block at the top of `schedule_one.go` is a set of scheduling-cycle tunables: a metrics sample rate and two feasibility-search floors. An API-imposed event-note cap is a different kind of thing, used by exactly one twelve-line helper near the bottom of the file. The old code kept the value inside the function; the new code moves it roughly 790 lines away. The new comment lines are also about 120 columns wide while every neighbouring comment in the block wraps near 80.

The heavier one is ownership. The PR's goal is legitimate: `pkg/scheduler` must stop importing `k8s.io/kubernetes/pkg/...` so it can move to staging. But the limit is not a scheduler concept, it is a property of the `events.k8s.io/v1` API. Copying the number into the consumer resolves the import problem while leaving the real defect, which is that this API limit has no importable home outside k/k internals. The comment's argument that a stale copy is safe holds (limits may rise but not fall, so the worst case is truncating a little earlier than needed), which is why I do not treat this as a blocker. It does, however, set the pattern for the rest of the series, and "copy the literal and write a paragraph about why that is fine" is the pattern I would least like to see repeated for constants where staleness is not harmless.

### Minimal remedy (in this PR)

Keep the value with the code that uses it and say precisely whose limit it is:

```go
// noteLengthLimit is the maximum length of an events.k8s.io/v1 Event note
// accepted by the API server. It mirrors NoteLengthLimit in
// k8s.io/kubernetes/pkg/apis/core/validation (see also the Note field doc in
// k8s.io/api/events/v1), which pkg/scheduler must not import. A stale copy is
// safe: API validation limits are never lowered, so the worst case is
// truncating earlier than strictly necessary.
const noteLengthLimit = 1024

// truncateMessage truncates a message if it exceeds noteLengthLimit.
func truncateMessage(message string) string {
	if len(message) <= noteLengthLimit {
		return message
	}
	suffix := " ..."
	return message[:noteLengthLimit-len(suffix)] + suffix
}
```

This leaves the top-of-file const block as it was at `main`, makes the diff to that block empty, and puts constant, rationale and consumer in one screen.

### Code-judo remedy (follow-up)

Give the limit a home in staging and delete the copy:

1. Export the limit from a package both sides can import. The most natural spot is next to the field it constrains, in `k8s.io/api/events/v1` (precedent: `k8s.io/api/core/v1.MaxSecretSize`). An alternative that avoids growing API surface is `k8s.io/client-go/tools/events`.
2. Have `pkg/apis/core/validation/events.go` define `NoteLengthLimit` in terms of that export (or reference it directly) so there is a single literal.
3. Have the scheduler reference the staging constant. `noteLengthLimit` and its justification comment disappear, and the "is a stale copy safe?" question never needs to be asked again.

This needs API-review sign-off and is correctly out of scope for a mechanical import-removal PR, which is why it is proposed as a follow-up.

## Finding 2 — conflict residue in the commit message

### Evidence

`git log -1 --format=%B review-head` prints:

```
scheduler: replace pkg/apis/core/validation dependency with local const

pkg/scheduler only uses k8s.io/kubernetes/pkg/apis/core/validation for
NoteLengthLimit, for which a local copy remains safe even if stale.

Part of removal of k/k imports in pkg/scheduler to support move to staging.

# Conflicts:
#	pkg/scheduler/schedule_one.go
```

### Why it matters

The last two lines are git's conflict note from a rebase or cherry-pick. Git normally strips `#` comment lines when the message is edited interactively, but they survive when the commit is completed non-interactively or with a cleanup mode that keeps them. They add nothing and will be permanent history for a heavily-read file.

### Remedy

`git commit --amend` and delete the two trailing lines (and the blank line before them). Keep the rest of the message unchanged.

## Question Q1 — ownership of truncation

`handleSchedulingFailure` truncates and then calls `Eventf`. The `tools/events` recorder does not truncate. If the recorder (or a helper beside it) owned "fit this note into what the server accepts", the scheduler would need neither `truncateMessage` nor any knowledge of the limit, and other emitters of potentially long notes would get the same protection. I am raising it as a question rather than a finding because it changes client-go behaviour and is a design decision for the owners of that package, not something this PR should be asked to carry.

## Adjacent observations, not findings against this PR

Both of the following are on lines the diff does not modify (or modifies only by renaming the limit) and behave identically before and after the change. They are recorded so that whoever next touches `truncateMessage` has them in view.

`truncateMessage` slices at a byte offset (`message[:noteLengthLimit-len(suffix)]`). The server check is also in bytes, so the length arithmetic is consistent, but the cut can land inside a multi-byte UTF-8 sequence when a failure message contains non-ASCII text.

At `schedule_one.go:838-839` the truncated message is passed in the `note` position of `Eventf`, which the recorder treats as a format string (`fmt.Sprintf(note, args...)`), rather than as an argument to a `"%s"` note. A failure message containing a `%` would be rendered with `%!…(MISSING)` artefacts, and the formatted result is not re-checked against the limit.
