# Scheduler dependency boundary

## Scope and evidence

The reviewed diff changes only `pkg/scheduler/schedule_one.go` and `pkg/scheduler/.import-restrictions`. In `schedule_one.go`, the import of `k8s.io/kubernetes/pkg/apis/core/validation` is removed, `noteLengthLimit = 1024` is added beside the scheduler's other local constants, and `truncateMessage` uses that constant. The new comment identifies `validation.NoteLengthLimit` as the source and explains why this local copy is safe: API validation limits cannot become lower than an already released version. The restriction file removes the corresponding allow rule, so the broad rule forbidding other `k8s.io/kubernetes` imports now applies to the validation package.

The source contract is in `pkg/apis/core/validation/events.go`: `NoteLengthLimit` is 1024, and event message validation rejects messages whose `len` exceeds that limit. `truncateMessage` also uses Go's byte-oriented `len`, so the copied threshold and event validation measure the same way. The scheduler truncates the event message before recording the failure event; the pod condition separately retains `errMsg`.

## Structural assessment

The local constant is a deliberate boundary choice, not an accidental duplicate helper. Importing `pkg/apis/core/validation` just to obtain one integer couples scheduler code to the broad API validation package. The local copy adds no new branch or abstraction, and its explanatory comment states both the owner of the contract and the compatibility invariant that permits the copy. Removing the import exception tightens the import policy to match the code.

The code-judo alternative considered was moving the shared constant into a lower-level package imported by both the scheduler and validation. That would centralize the number, but would require changing ownership and dependency structure outside this small change and introduce an additional shared package/API surface for a single compatibility-stable limit. No evidence in the reviewed change establishes an existing lower-level canonical home. Given the import boundary and the stated invariant, the direct local constant is the simpler structure; no restructuring is warranted from this diff alone.

## Measurements and verification

`pkg/scheduler/schedule_one.go` is 890 lines at the base and 892 lines at the head, remaining below the skill's 1,000-line threshold. The full diff is two files, with seven insertions and seven deletions. `git diff --check main...review-head` completed without output, and the working tree was clean after inspection.

No tests were run. This review assessed the committed diff, source contract, and structure; it did not execute package tests.

## Finding status

No actionable maintainability finding. The possible duplication of the event note limit was assessed above and is justified by the documented compatibility invariant and scheduler import boundary.
