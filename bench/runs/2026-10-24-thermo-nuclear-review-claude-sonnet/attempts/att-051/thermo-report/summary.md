# Thermo nuclear review: kubernetes/kubernetes#141463

No actionable findings.

What I checked:
- `noteLengthLimit = 1024` in `pkg/scheduler/schedule_one.go` equals `NoteLengthLimit = 1024` in `pkg/apis/core/validation/events.go:37`. `truncateMessage` behaves exactly as before.
- No other file under `pkg/scheduler` imports `k8s.io/kubernetes/pkg/apis/core/validation`. Remaining mentions are comments only. Removing the allow rule from `.import-restrictions` therefore breaks no import check.
- `truncateMessage` callers, in `schedule_one.go` and the tests at `schedule_one_test.go:2098` and `:5062`, are unaffected.
