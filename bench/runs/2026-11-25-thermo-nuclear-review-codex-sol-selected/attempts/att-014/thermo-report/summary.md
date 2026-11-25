No actionable findings.

Reviewed `kubernetes/kubernetes#141463`, covering the committed diff from `6bb42350227f0a714f4730165e7ba622c69bd99e` to `d9cf69d74a25a209e79c7061ff86a14ab4b4b634` in `pkg/scheduler/schedule_one.go` and `pkg/scheduler/.import-restrictions`.

The local `noteLengthLimit` equals the existing API validation constant, 1024. The length comparison, truncation offset, and suffix preserve the previous behavior for every input. Traced the scheduling failure handler through event recording, events/v1 conversion, and core event validation; the validator checks the same byte limit. Pod condition updates and feature-gated scheduling paths are unaffected. The removed validation import has no package initialization function or blank import requiring preservation.

The import restriction change removes only the exception for the dependency eliminated by this PR. Checked scheduler production and test sources for remaining imports and traced direct versus transitive rule handling in `cmd/import-boss`. The focused checker passed: `go run ./cmd/import-boss ./pkg/scheduler/...`.

Focused scheduler tests passed: `go test ./pkg/scheduler -run '^(TestSchedulerScheduleOne|TestHandleSchedulingFailure.*|TestScheduleOneMarksPodAsProcessedBeforePreBind|TestSchedulerFailedSchedulingReasons)$' -count=1 -timeout=3m`. Both Go commands used the supplied toolchain and caches, vendored dependencies, the checked-in workspace, disabled dependency fetching, and a five-minute command limit. `git diff --check main...review-head` passed. The full Kubernetes suite was not run. No checkout files were changed.
