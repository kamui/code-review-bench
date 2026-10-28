# Thermo Nuclear Review: kubernetes/kubernetes#141463

Range reviewed: `6bb42350227f0a714f4730165e7ba622c69bd99e..d9cf69d74a25a209e79c7061ff86a14ab4b4b634` (one commit, `d9cf69d74a2 scheduler: replace pkg/apis/core/validation dependency with local const`).

## What the change does

The commit touches two files:

- `pkg/scheduler/schedule_one.go`: drops the import of `k8s.io/kubernetes/pkg/apis/core/validation`, adds an unexported constant `noteLengthLimit = 1024` to the existing `const` block, and rewrites `truncateMessage` to use that constant instead of `validation.NoteLengthLimit`.
- `pkg/scheduler/.import-restrictions`: removes the allow rule for `^k8s[.]io/kubernetes/pkg/apis/core/validation$`, so the catch-all `^k8s[.]io/kubernetes` forbid rule now blocks that package for the scheduler tree.

## Evidence traced

- **Constant value.** `pkg/apis/core/validation/events.go:37` defines `NoteLengthLimit = 1024`. The new local `noteLengthLimit` in `pkg/scheduler/schedule_one.go:61` is also `1024`, so `truncateMessage` produces byte-identical output before and after the change. The slicing logic (`message[:noteLengthLimit-len(suffix)] + suffix`) is unchanged apart from the identifier rename.
- **Remaining importers.** A search of `pkg/scheduler` for the string `k8s.io/kubernetes/pkg/apis/core/validation` finds only the new comment; no Go file under the scheduler tree, including tests, still imports the package. `pkg/scheduler/.import-restrictions` is the only restrictions file in that tree, and its final rule forbids all other `k8s.io/kubernetes` imports, so a stale importer would have been caught by import-boss.
- **Verification commands run** (vendored toolchain, `GOFLAGS=-mod=vendor`):
  - `go build ./pkg/scheduler/...` and `go vet ./pkg/scheduler` succeed.
  - `go run ./cmd/import-boss ./pkg/scheduler/...` succeeds with the tightened restrictions.
  - `go test ./pkg/scheduler -run 'TestSchedulerScheduleOne|TestUpdatePod|TestSchedulerNoPhantomPodAfterExpire|TestSchedulerFailedSchedulingReasons|TestSchedulerBinding' -count=1` passes; `schedule_one_test.go:2098` and `schedule_one_test.go:5062` exercise `truncateMessage` directly against the recorded failure message.
- **Downstream effects.** `truncateMessage` is only called from `handleSchedulingFailure` (`schedule_one.go:838`) to bound the event/condition message. No exported API, feature gate, environment variable, port, script, or build step changes. The removed package is in-repo, so `vendor/modules.txt` and the Go workspace are unaffected.
- **Risk of drift.** The local constant can silently diverge if `NoteLengthLimit` is raised in the future; in that case the scheduler would truncate earlier than the API server requires, which is harmless (messages remain valid). The comment in the diff states this reasoning explicitly. A lowered limit would be an API-compatibility break on the validation side, not something this change introduces.

## Findings

No actionable findings.
