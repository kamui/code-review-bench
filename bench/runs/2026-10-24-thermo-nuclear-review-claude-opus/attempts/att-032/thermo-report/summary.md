# Thermo Nuclear Review: kubernetes/kubernetes#141463

Range: `6bb42350227f0a714f4730165e7ba622c69bd99e..d9cf69d74a25a209e79c7061ff86a14ab4b4b634` (`main...review-head`)

## Scope

- `pkg/scheduler/schedule_one.go`: drops the `k8s.io/kubernetes/pkg/apis/core/validation` import and adds a local `noteLengthLimit = 1024` constant. `truncateMessage` now uses that constant.
- `pkg/scheduler/.import-restrictions`: removes the allow rule for `^k8s[.]io/kubernetes/pkg/apis/core/validation$`.

## Verification traced

1. **Value parity.** `pkg/apis/core/validation/events.go:37` defines `NoteLengthLimit = 1024`, and the API server enforces it on event messages (`events.go:183-184`). The local constant has the same value. `truncateMessage` therefore behaves exactly as before: messages of 1024 bytes or less pass through unchanged, and longer ones are cut to 1020 bytes plus `" ..."`. The truncation math (`noteLengthLimit-len(suffix)`) is unchanged.
2. **Callers.** The only callers are `schedule_one.go:838` (the failure-condition message in `handleSchedulingFailure`) and two test assertions (`schedule_one_test.go:2098`, `:5062`). These tests call the same function, so their expectations still match.
3. **Import restriction removal.** No package under `pkg/scheduler/...` (tests included) still imports `pkg/apis/core/validation`. I grepped for the quoted import path, which also catches aliased imports. In `cmd/import-boss/main.go:375` and `:506`, rules are non-transitive by default (`!rule.Transitive && !isDirect[imp]`). Allowed dependencies such as `pkg/controller` that pull in the validation package transitively therefore do not trip the catch-all forbid rule. `go run ./cmd/import-boss ./pkg/scheduler/...` exited 0 with no violations, and `go vet ./pkg/scheduler/` exited 0.
4. **Drift risk.** The constant is now copied by hand rather than shared. If the server-side limit were ever *raised*, the scheduler would keep truncating at 1024. That is safe: it is the current behaviour and still yields valid notes. API validation limits are not lowered for compatibility reasons, so the copy cannot produce messages the API server rejects. The code comment documents this reasoning.
5. **Security / feature gates / devex.** The change touches no feature gate, credential, networking, or build workflow. Removing the allow rule only tightens the dependency policy.

## Findings

No actionable findings.
