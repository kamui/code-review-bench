# Thermo-nuclear code quality review: kubernetes/kubernetes#141463

Range reviewed: `6bb42350227f..d9cf69d74a25` (`git diff main...review-head`), one commit, two files, +7/-7.
Reviewer model: `claude-opus-5-5` at `high`, single primary context. No cross-model or alternate-model review was run and no child reviewers were started.

Detail file with the measurements, commands and worked proposals: [01_scheduler_note_limit.md](01_scheduler_note_limit.md).

## Verdict

Approve. There is no blocking finding.

The change does what it says. `pkg/scheduler/schedule_one.go` used `k8s.io/kubernetes/pkg/apis/core/validation` for exactly one symbol, `NoteLengthLimit`, inside `truncateMessage`. The PR replaces it with an unexported `noteLengthLimit = 1024` and deletes the matching allow rule from `pkg/scheduler/.import-restrictions`. I confirmed that nothing else under `pkg/scheduler/...` (tests included) imports that package, so the deleted rule is dead rather than merely unused by this file, and that the scheduler root package no longer reaches the validation package even transitively. The package builds and `TestSchedulerScheduleOne` passes.

Against the skill's approval bar: `schedule_one.go` goes from 890 to 892 lines, well under 1k. No branch, flag, wrapper, cast or helper is added; `truncateMessage` actually loses a local that shadowed the builtin `max`. The safety argument in the new comment is sound: a stale copy can only be lower than the server's limit, which means over-truncating, never a rejected event.

What remains are three low-severity items. None changes behaviour and none should hold the merge on its own.

## Findings

### Finding 1: the limit is now pinned only by a comment

`pkg/scheduler/schedule_one.go:58-61` and `pkg/scheduler/schedule_one.go:851-858`. Before this PR the compiler tied the scheduler's truncation length to the validator's limit. After it, the only thing connecting `noteLengthLimit = 1024` to `NoteLengthLimit` in `pkg/apis/core/validation/events.go:37` is the prose in the comment, and nothing in the tree would fail if either number were edited. `truncateMessage` has no test of its own: the two references in `schedule_one_test.go` (lines 2098 and 5062) call it from replacement failure handlers to build the message, so they would agree with any value. The drift risk is small, because the limit is also written into the `events.k8s.io/v1` `Note` field documentation and lowering it would break clients, but the PR trades a compile-time guarantee for an unchecked one and should say so with a test rather than a sentence. The cheapest fix is a table test for `truncateMessage` at 1023, 1024 and 1025 bytes that asserts the output length and the ` ...` suffix. That pins the behaviour inside the package without needing the forbidden import. If the authors want the two constants compared directly, that check has to live outside `pkg/scheduler`, for example in `test/integration/scheduler`, whose import rules do not forbid `pkg/apis/core/validation`.

### Finding 2: the constant has a better home than a private scheduler copy

`pkg/scheduler/schedule_one.go:58-61`. The note limit is part of the `events.k8s.io/v1` API contract, not scheduler policy, and `staging/src/k8s.io/api/events/v1/types.go:81-83` already documents it in prose ("Maximal length of the note is 1kB"). `k8s.io/api` is where the tree keeps this kind of number: `ResourceHealthMessageMaxLength = 1024` and `MaxSecretSize` in `core/v1/types.go`, and the `*MaxSize` and `*MaxLength` family in `resource/v1/types.go`. The code-judo move is to declare the note limit once next to the `Note` field, have `pkg/apis/core/validation/events.go` use that declaration, and have the scheduler import `k8s.io/api/events/v1`, which is a staging module and therefore compatible with the move this PR is preparing. That removes the k/k import and the duplicated literal at the same time, and the "safe even if stale" paragraph is no longer needed because nothing can go stale. A further step would be to truncate inside the `client-go/tools/events` recorder, since the scheduler is the only in-tree caller that truncates at all and every other user of that recorder has the same exposure; that would delete `truncateMessage` from the scheduler outright. Both steps add exported surface or change shared behaviour in staging modules and need API review, so I would accept the local copy for this PR and treat the relocation as a follow-up. It is worth a `TODO` on the constant so the copy is not mistaken for the end state.

### Finding 3: the commit message carries leftover rebase text

Commit `d9cf69d74a25`. The message body ends with `# Conflicts:` and `#	pkg/scheduler/schedule_one.go`, which is the residue git appends when a cherry-pick or rebase stops on a conflict. This repository merges commits without squashing, so those lines would land in history. Amend the message to drop them.

## Remediation sequence

1. Amend the commit message to remove the two `# Conflicts:` lines (Finding 3).
2. Add the boundary test for `truncateMessage` in `pkg/scheduler/schedule_one_test.go` (Finding 1). This fits in the same PR.
3. Add a `TODO` on `noteLengthLimit` pointing at a shared declaration in `k8s.io/api/events/v1`, and do the relocation in a separate PR with API review (Finding 2).

## Observed but outside this diff

These are pre-existing properties of the code around the change. They are not findings against this PR and are not listed in the finding index. Details are in the detail file.

The truncated message is passed to `Eventf` as the format string with no arguments (`schedule_one.go:839`), and the recorder runs `fmt.Sprintf(note, args...)` on it, so a `%` in a scheduling error would be expanded after truncation. `truncateMessage` also slices by byte, so it can cut a multi-byte rune in half.

## Verification status

Verified by running: `go build ./pkg/scheduler/` succeeds; `go list -deps ./pkg/scheduler/` contains no `k8s.io/kubernetes/pkg/apis/core/validation`; `go list -test` over `./pkg/scheduler/...` shows no package or test importing it; `go test ./pkg/scheduler/ -run 'TestSchedulerScheduleOne$' -count=1` passes; `git diff --check` is clean.

Not run: `hack/verify-import-boss.sh` itself (the rule removal was checked through the import lists instead), and the rest of the scheduler unit and integration suites.
