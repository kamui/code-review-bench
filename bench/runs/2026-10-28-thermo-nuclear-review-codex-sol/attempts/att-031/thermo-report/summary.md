No actionable findings.

Reviewed the complete committed diff for kubernetes/kubernetes#141463 from `6bb42350227f0a714f4730165e7ba622c69bd99e` to `d9cf69d74a25a209e79c7061ff86a14ab4b4b634`, covering `pkg/scheduler/schedule_one.go` and `pkg/scheduler/.import-restrictions`.

The new `noteLengthLimit` at `pkg/scheduler/schedule_one.go:58–61` equals the existing `validation.NoteLengthLimit` of 1024 in `pkg/apis/core/validation/events.go:37`. The changed `truncateMessage` implementation at `pkg/scheduler/schedule_one.go:851–857` preserves the previous byte-length comparison, cutoff, and four-byte suffix. Messages of up to 1024 bytes remain unchanged, and longer messages produce the same bytes as before. The scheduling failure path still truncates the event note while retaining the full error message in the pod condition.

Traced the event recorder into `staging/src/k8s.io/client-go/tools/events/event_recorder.go`, the note-to-message conversion in `pkg/apis/events/v1/conversion.go`, and event validation through `pkg/registry/core/event/strategy.go` to `pkg/apis/core/validation/events.go`. The copied limit matches the server's byte-length constraint. The patch does not change feature-gate decisions or validation enforcement.

The import restriction change removes only the exception for the dependency removed by the code change. Searched scheduler sources, including tests, for remaining core-validation imports and checked the import-boss rule evaluation and test-package loading. No remaining import requires that exception.

Validation passed: `git diff --check main...review-head`; focused scheduler tests using `go test ./pkg/scheduler -run '^(TestSchedulerScheduleOne|TestSchedulerFailedSchedulingReasons|TestHandleSchedulingFailure.*)$' -count=1 -timeout=240s`; and `go run ./cmd/import-boss ./pkg/scheduler/...`. Go commands used the provided toolchain, vendored dependencies, and checked-in workspace. The full Kubernetes test suite was not run. The checkout remained clean.
