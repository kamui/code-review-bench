# Thermo-nuclear code quality review: kubernetes/kubernetes#141463

Range reviewed: `6bb42350227f..d9cf69d74a25` (`git diff main...review-head`), one commit, two files, +7/−7.

Review setup: one primary review context, model `claude-opus-5-5` at `high`. No delegated reviewers, no cross-model or alternate-model review.

## Verdict

Acceptable, with two small things to fix before merge. There is no structural regression here: the change deletes a `k8s.io/kubernetes/pkg/apis/core/validation` import from `pkg/scheduler/schedule_one.go`, removes the matching allowance from `pkg/scheduler/.import-restrictions`, and replaces the one symbol it used (`validation.NoteLengthLimit`) with a local `noteLengthLimit = 1024`. No file crosses a size boundary (`schedule_one.go` goes from 890 to 892 lines), no branch is added, no wrapper or cast appears, and behaviour is byte-for-byte unchanged.

What I verified: the scheduler package builds and vets cleanly, `import-boss` passes over `./pkg/scheduler/...` with the tightened rules, nothing else under `pkg/scheduler` imports the validation package (directly, in tests, or transitively from the root package), and the focused scheduling-failure tests pass. Full evidence is in [01_scheduler_note_limit.md](01_scheduler_note_limit.md).

The one design point worth pushing on is that the PR removes a layering violation by forking a number rather than by giving that number a home both sides can import. That is defensible for this constant, and the author's comment explains why a stale copy is harmless, but the copy is placed and documented in a way that makes it harder to find than it needs to be.

## Findings

### 1. The event-note limit is now a second, unlinked source of truth, declared about 790 lines from its only consumer

In `pkg/scheduler/schedule_one.go` lines 58–61 the PR adds `noteLengthLimit = 1024` to the file-level const block that otherwise holds scheduling-cycle tunables (`pluginMetricsSamplePercent`, `minFeasibleNodesToFind`, `minFeasibleNodesPercentageToFind`), while its only consumer is `truncateMessage` at lines 851–858, which in turn has a single production caller at line 838. Before the change the limit was a function-local (`max := validation.NoteLengthLimit`) that read directly off the canonical definition in `pkg/apis/core/validation/events.go:37`; after it, the value `1024` exists in two places with nothing but a comment connecting them, and the copy sits among unrelated knobs where a reader of `truncateMessage` has to go hunting for it. The comment's safety argument is sound (API validation limits do not shrink, so a stale copy can only truncate more than strictly necessary), so this is not a correctness risk, but it is a small step backwards in locality and it normalises "copy the number" as the way to shed k/k imports. The minimal fix inside this PR is to keep the concept in one place: declare the constant immediately above `truncateMessage` (or as a `const` inside it), wrap the comment to the surrounding ~80-column style, and make it name the real owner precisely, which is the `events.k8s.io/v1` `Event.note` limit enforced in `pkg/apis/core/validation/events.go` and documented as "1kB" at `staging/src/k8s.io/api/events/v1/types.go:82`. The code-judo version, worth doing as a follow-up rather than blocking this PR, is to stop copying altogether: export the limit from a staging package that both the apiserver validation and the scheduler may import (there is precedent in `k8s.io/api/core/v1.MaxSecretSize`), have `pkg/apis/core/validation` consume it, and let the scheduler reference it, at which point the local constant and its justification comment disappear. Evidence and a worked sketch are in [01_scheduler_note_limit.md](01_scheduler_note_limit.md).

### 2. The commit message carries leftover conflict residue

The single commit under review (`d9cf69d74a2`) ends its message with the lines `# Conflicts:` and `#	pkg/scheduler/schedule_one.go`. That is the comment block git inserts during a conflicted rebase or cherry-pick, and it was committed without being stripped. It describes nothing about the change, it will be preserved verbatim in `git log` for `pkg/scheduler/schedule_one.go` forever once merged, and it is exactly the kind of noise that makes history harder to read. Amend the commit message to drop those two lines before merge; the rest of the message (what is replaced, why a stale copy is safe, and that this is part of the staging move) is good and should stay. Evidence is in [01_scheduler_note_limit.md](01_scheduler_note_limit.md).

## Questions

### Q1. Should note truncation live in the scheduler at all?

The `events.k8s.io/v1` recorder in `staging/src/k8s.io/client-go/tools/events/event_recorder.go` formats the note with `fmt.Sprintf(note, args...)` and sends it without any length handling, so every component that can produce a long note has to know the server-side limit and truncate by hand, which is why the scheduler needed the validation constant in the first place. Is there a reason the limit and the truncation cannot be owned by the recorder (or by a constant exported next to it), so that the scheduler's `truncateMessage` and `noteLengthLimit` could be deleted outright instead of being re-homed? This is outside the scope of the PR and I am not asking for it here; I am asking whether the staging-move series intends to leave a hand-copied API limit in the scheduler permanently.

## Proposed remediation sequence

First, amend the commit message to remove the `# Conflicts:` residue (finding 2); this is a thirty-second fix and should not be left for a squash that may not happen. Second, in the same commit, move `noteLengthLimit` down so it sits directly with `truncateMessage`, and tighten the comment so it names the `events.k8s.io/v1` note limit and both places that define or document it (finding 1, minimal form). Third, as a separate follow-up and only if API reviewers agree, hoist the limit into a staging package and make validation and the scheduler share it, or move truncation into the events recorder (finding 1 judo form, question Q1).

## Things checked and deliberately not raised

The `.import-restrictions` edit is complete and correct: no remaining importer, and `import-boss` is green. The 1k-line rule is not triggered. No new branching, flags, wrappers, casts or optionality are introduced. Two pre-existing oddities sit on lines the PR does not touch and are recorded in the detail file for context only, not as findings against this change: `truncateMessage` slices by byte offset and can cut a multi-byte rune, and its result is passed as the format string argument of `Eventf` rather than as an argument to a `"%s"` format.

## Detail files

- [01_scheduler_note_limit.md](01_scheduler_note_limit.md): measurements, commands, verification status, and the worked proposals for both findings and the question.
