# Thermo Nuclear Review — kubernetes/kubernetes#141463

Range: `6bb42350227f0a714f4730165e7ba622c69bd99e..d9cf69d74a25a209e79c7061ff86a14ab4b4b634` (`git diff main...review-head`).

No actionable findings.

## What was audited

- `pkg/scheduler/schedule_one.go`: removes the import of `k8s.io/kubernetes/pkg/apis/core/validation` and adds a local constant `noteLengthLimit = 1024`, which `truncateMessage` now uses.
- `pkg/scheduler/.import-restrictions`: drops the allow rule for `^k8s[.]io/kubernetes/pkg/apis/core/validation$`. Imports of that package now hit the catch-all `^k8s[.]io/kubernetes` forbid rule.

## Evidence traced

- **Value equivalence.** `pkg/apis/core/validation/events.go:37` defines `NoteLengthLimit = 1024`. The local constant matches it. `truncateMessage` keeps the same logic: messages of 1024 bytes or fewer pass through unchanged, and longer ones become `message[:1020] + " ..."`. It still returns exactly 1024 bytes, which is the length API event validation accepts (`events.go:183`, `len(event.Message) > NoteLengthLimit`). Behaviour is identical, and the result is still a compile-time untyped constant.
- **Callers.** `truncateMessage` is called at `schedule_one.go:838`, where it builds the FailedScheduling event message, and from tests at `schedule_one_test.go:2098` and `:5062`. Tests call the function rather than the removed symbol, so they are unaffected.
- **Import-restriction tightening.** Running `grep` over `pkg/scheduler` finds no remaining Go import of `k8s.io/kubernetes/pkg/apis/core/validation`, including in test files. The only matches are comments in `framework/plugins/dynamicresources/nodeallocatabledynamicresources.go` and `apis/config/validation/validation_pluginargs.go`. Removing the allow rule therefore cannot make import-boss fail for any current scheduler package. `pkg/scheduler/apis/config/validation` is a different import path and doesn't match the anchored, removed regex.
- **Build/test.** `go vet ./pkg/scheduler/` passed. `go test ./pkg/scheduler/ -run 'TestSchedulerScheduleOne|Truncat|TestSchedulerFailedSchedulingReasons'` passed.
- **Drift risk.** If the API server ever raised `NoteLengthLimit`, the local copy would stay at 1024. Messages would then only be truncated more than necessary and would never be rejected, so this is safe, as the new comment says. Lowering an API validation limit would break already-stored objects, so that direction is not a realistic concern.
- **Security, devex and feature gates.** No secrets, environment variables, ports, scripts or feature gates are touched. The new `.import-restrictions` rule is stricter than before, so it can't widen what the scheduler may import.
