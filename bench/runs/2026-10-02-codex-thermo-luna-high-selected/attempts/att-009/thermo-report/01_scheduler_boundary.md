# Scheduler import boundary and note truncation

## Scope and evidence

The committed diff changes only `pkg/scheduler/.import-restrictions` and `pkg/scheduler/schedule_one.go`. The restriction for `k8s.io/kubernetes/pkg/apis/core/validation` is removed. In `schedule_one.go`, the validation import is replaced by the package-local `noteLengthLimit = 1024`; `truncateMessage` uses that constant in the length check and truncation expression. The comment identifies the source constant and explains why a stale local copy remains safe. The changed constant adds four lines in the existing constants block; `schedule_one.go` grows from 890 to 892 lines, remaining below the skill's 1,000-line threshold.

The source definition in `pkg/apis/core/validation/events.go:37` is `NoteLengthLimit = 1024`. Its use at lines 183–184 compares `len(event.Message)` to that value. The scheduler's old and new `truncateMessage` implementations both return messages at or below 1024 bytes unchanged, and otherwise return the prefix of length `1024-len(" ...")` followed by the same suffix. Thus the local copy preserves the previous byte-based behavior, including its handling of non-ASCII input.

## Maintainability assessment and code-judo proposal

This is a small and direct dependency-boundary cleanup. The implementation does not introduce a wrapper, branch, or new abstraction. The copied scalar is used at one call site and retains the validation package's existing byte-count semantics. Its source and compatibility rationale are documented beside the constant.

No stronger code-judo move is apparent. Keeping the literal local preserves the scheduler's independence from core validation. Moving the value into a new shared package or exported API solely to eliminate one local scalar would broaden ownership and dependency surface for no reduction in concepts or code. The local constant is the simplest structure that meets the migration goal while preserving behavior.

No actionable finding or remediation is warranted.

## Verification status

Static review only; tests were not run. The diff was inspected with `git diff main...review-head`, the source definition and scheduler references were searched with `rg`, and `git diff --check main...review-head` completed successfully. The worktree status was clean before report generation. No files in the repository clone were edited.
