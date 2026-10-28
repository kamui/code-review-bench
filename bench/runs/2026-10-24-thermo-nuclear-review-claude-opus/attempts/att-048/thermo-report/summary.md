# Thermo Nuclear Review — kubernetes/kubernetes#141463

Range: `6bb42350227f0a714f4730165e7ba622c69bd99e..d9cf69d74a25a209e79c7061ff86a14ab4b4b634` (`git diff main...review-head`)

## Change summary

- `pkg/scheduler/schedule_one.go`: drops the import of `k8s.io/kubernetes/pkg/apis/core/validation`. Adds a package-private constant `noteLengthLimit = 1024`. `truncateMessage` now uses that constant instead of `validation.NoteLengthLimit`.
- `pkg/scheduler/.import-restrictions`: removes the allow rule for `^k8s[.]io/kubernetes/pkg/apis/core/validation$`. Any direct import of that package from `pkg/scheduler/...` now falls through to the catch-all `^k8s[.]io/kubernetes` forbidden rule.

## What I traced

1. **The constant's value is unchanged.** `pkg/apis/core/validation/events.go:37` defines `NoteLengthLimit = 1024`, and `events.go:183-184` enforces it on event messages. The new local constant is also 1024. `truncateMessage` behaves the same as before: messages of 1024 bytes or fewer are returned as-is, and longer ones are cut to 1020 bytes plus `" ..."`, for a total of 1024. That output still passes API-server validation.
2. **The call sites are unchanged.** `truncateMessage` is called in `handleSchedulingFailure` (`schedule_one.go:838`) for the `FailedScheduling` event message. It is also called in tests (`schedule_one_test.go:2098`, `:5062`). The pod condition message still uses the untruncated `errMsg`, as it did before the change. No other code referenced `validation.NoteLengthLimit` through this package.
3. **The import restriction is safe to remove.** I searched all of `pkg/scheduler` for direct imports of `k8s.io/kubernetes/pkg/apis/core/validation`, including `_test.go` files, and found none. The only other mentions are comments in `framework/plugins/dynamicresources/nodeallocatabledynamicresources.go:403,531` and `apis/config/validation/validation_pluginargs.go:100`. Comments are not imports, so the tighter rule will not fail `verify-import-boss`. `pkg/scheduler/.import-restrictions` is the only restrictions file in the tree.
4. **Build and tests pass.** `go vet ./pkg/scheduler/` passed, so the package and its tests compile without the removed import. `go test ./pkg/scheduler/ -run 'TestSchedulerFailedSchedulingReasons|TestSchedulerScheduleOne$'` passed (`ok k8s.io/kubernetes/pkg/scheduler 18.136s`). These tests exercise the truncation path.
5. **No drift risk in practice.** If the API server's limit is later raised, the scheduler would keep truncating at 1024 bytes, which is still valid. Only a lower limit would cause events to be rejected. The new comment says this cannot happen, which is consistent with Kubernetes API compatibility rules.
6. **Other areas.** The change touches no feature gates, security-sensitive paths, environment variables, ports, scripts, or developer workflows.

## Findings

No actionable findings.
