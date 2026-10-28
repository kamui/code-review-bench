# Thermo Nuclear Review: kubernetes/kubernetes#141463

Range reviewed: `6bb42350227f0a714f4730165e7ba622c69bd99e..d9cf69d74a25a209e79c7061ff86a14ab4b4b634` (one commit, `scheduler: replace pkg/apis/core/validation dependency with local const`).

## What the change does

- `pkg/scheduler/schedule_one.go` drops the import of `k8s.io/kubernetes/pkg/apis/core/validation` and adds a package-local constant `noteLengthLimit = 1024`. `truncateMessage` now compares against and slices with the local constant instead of `validation.NoteLengthLimit`. The removal of the `max` local also stops shadowing the Go builtin.
- `pkg/scheduler/.import-restrictions` removes the allow rule for `^k8s[.]io/kubernetes/pkg/apis/core/validation$`, so the catch-all `forbiddenPrefixes` rule for `k8s.io/kubernetes` now applies to that package across the scheduler tree.

## Evidence traced

- The upstream constant is `NoteLengthLimit = 1024` in `pkg/apis/core/validation/events.go:37`, used by both legacy and `events.k8s.io` event validation. The local copy has the identical value, so the truncation boundary and the `message[:noteLengthLimit-len(suffix)] + suffix` slice produce byte-for-byte the same output as before. No change in the length of the event message or the pod condition message the scheduler emits.
- No other file under `pkg/scheduler/` (including tests) imports `k8s.io/kubernetes/pkg/apis/core/validation` or references `NoteLengthLimit`, so removing the allow rule cannot break any other scheduler package. The two test call sites (`schedule_one_test.go:2098`, `schedule_one_test.go:5062`) call `truncateMessage` directly and do not reference the validation constant.
- Verification commands run from the clone root with the vendored toolchain:
  - `go vet ./pkg/scheduler/` compiles the package and its tests cleanly.
  - `go run ./cmd/import-boss ./pkg/scheduler/...` (the same tool `hack/verify-import-boss.sh` runs) exits 0, confirming the tightened `.import-restrictions` is satisfied by every scheduler package.
  - `go test ./pkg/scheduler/ -run 'TestSchedulerScheduleOne|TestSchedulerWithExtenders|TestUpdatePod|TestSchedulerFailedSchedulingReasons|TestSchedulerNoPhantomPodAfterExpire|TestSchedulerNoPhantomPodAfterDelete'` passes, covering the failure-handling paths that go through `truncateMessage`.
- Drift risk: the comment correctly notes that the API server limit will not be lowered in a released version. If the server limit were ever raised, the scheduler would only truncate more aggressively than strictly required, which is not a correctness or security regression. No feature gates, environment variables, ports, scripts, or build steps are touched, so there is no devex or feature-leak exposure.

## Findings

No actionable findings.
