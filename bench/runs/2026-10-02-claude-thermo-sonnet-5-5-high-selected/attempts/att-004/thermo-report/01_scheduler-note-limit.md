# 01: scheduler note-length limit decoupling

## Scope

`pkg/scheduler/schedule_one.go` and `pkg/scheduler/.import-restrictions`.

## Measurements and commands

- `git diff main...review-head` shows 2 files changed. The edits are one import removed, one constant added, `truncateMessage` rewritten to use the constant, and one allow-rule removed.
- `wc -l pkg/scheduler/schedule_one.go` reports 892 lines, below the 1000-line threshold.
- `grep -rn "NoteLengthLimit" pkg test staging` shows the canonical definition at `pkg/apis/core/validation/events.go:37` and its only other use at line 183 in the same file. There is no other copy.
- `grep -rn "apis/core/validation" pkg/scheduler cmd/kube-scheduler test/integration/scheduler` finds only comments, with no imports left. Removing the `.import-restrictions` rule is therefore consistent.
- `go build ./pkg/scheduler/` (vendored toolchain) completes with no errors.
- The two test callers, `pkg/scheduler/schedule_one_test.go:2098` and `:5062`, call `truncateMessage` unchanged.

## Verification status

Behavior is unchanged: both the old and new values are 1024, and the suffix arithmetic is the same. Build passed. I did not run the package tests, since the change is a constant substitution.

## Finding: duplicated limit has no drift guard

Location: `pkg/scheduler/schedule_one.go` lines 58-61.

The new `noteLengthLimit = 1024` duplicates `NoteLengthLimit` in `pkg/apis/core/validation/events.go:37`. The comment justifies the copy by saying API limits never go lower, which is a statement of intent and nothing enforces it. The scheduler's truncation exists to stay under the API server's limit. If the server limit ever dropped, the scheduler would again emit notes the API server rejects, and nothing would fail loudly. The scheduler's own tests cannot import the validation package, because `.import-restrictions` applies to test files as well.

### Worked code-judo proposal

Do not undo the decoupling. Add the check where the dependency is already legal, in `pkg/apis/core/validation`, with a test that asserts `NoteLengthLimit` is still at least 1024 and points to the scheduler constant in its failure message. This adds no runtime dependency and makes the "never lowers" rule enforced. A second option is a short comment pointing to an existing API-compatibility guarantee, if one exists.

## Other observations (not findings)

- `truncateMessage` slices by bytes and can cut a multi-byte rune in half. This predates the PR and is unchanged, so it is out of scope.
- The constant sits in the existing `const` block beside `minFeasibleNodesPercentageToFind` and matches its comment style. The placement is fine.
