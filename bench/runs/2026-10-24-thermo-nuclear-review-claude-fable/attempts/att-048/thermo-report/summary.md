# Thermo Nuclear Review: kubernetes/kubernetes#141463

Range reviewed: `6bb42350227f0a714f4730165e7ba622c69bd99e..d9cf69d74a25a209e79c7061ff86a14ab4b4b634` (one commit, `d9cf69d74a2 scheduler: replace pkg/apis/core/validation dependency with local const`).

## Verdict

No actionable findings.

## What the change does

The commit touches two files.

- `pkg/scheduler/schedule_one.go`: drops the import of `k8s.io/kubernetes/pkg/apis/core/validation`, adds an unexported constant `noteLengthLimit = 1024` in the existing `const` block (lines 58-61), and rewrites `truncateMessage` (lines 851-858) to use that constant instead of `validation.NoteLengthLimit`.
- `pkg/scheduler/.import-restrictions`: removes the allow rule for `^k8s[.]io/kubernetes/pkg/apis/core/validation$`, so that package is now caught by the trailing "forbid any other k8s.io/kubernetes import" rule for everything under `pkg/scheduler`.

## Evidence traced

**Constant value is identical.** `pkg/apis/core/validation/events.go:37` defines `NoteLengthLimit = 1024`. The new local constant is also `1024`, so the truncation threshold and the slice bound `message[:noteLengthLimit-len(suffix)]` are byte-for-byte the same as before. `truncateMessage` logic is otherwise unchanged (same `<=` comparison, same `" ..."` suffix). There is no behavioural change for the `FailedScheduling` event message emitted in `handleSchedulingFailure` (`schedule_one.go:838`).

**The limit the scheduler is matching is still enforced at 1024 on the path it uses.** The scheduler records events through the `events.k8s.io/v1` recorder. `ValidateEventCreate` in `pkg/apis/core/validation/events.go` applies the strict `len(event.Message) > NoteLengthLimit` check (line 183) for that API version, so the local copy remains aligned with the server-side rejection threshold. The in-code comment correctly states the only drift risk (the server limit becoming lower than a released value), which Kubernetes API compatibility rules forbid; a future increase of the server limit would merely leave the scheduler truncating earlier than strictly required, not produce rejected events.

**No remaining importers are broken by the tightened import rule.** A search of `pkg/scheduler/**` for the string `k8s.io/kubernetes/pkg/apis/core/validation` finds only the explanatory comment at `schedule_one.go:58`; no Go file (including `_test.go` files, which import-boss also checks) imports the package. `pkg/scheduler/.import-restrictions` is the only restrictions file under `pkg/scheduler`, so the rule removal applies uniformly to all subpackages, and all of them are clean. Removing an allow rule can only tighten enforcement; it cannot admit new imports.

**Callers and tests.** `truncateMessage` is unexported and has exactly three callers: `schedule_one.go:838` and the two test harnesses at `schedule_one_test.go:2098` and `schedule_one_test.go:5062`. None reference the removed import or the exported constant. Nothing outside `pkg/scheduler` consumed the scheduler's use of `validation.NoteLengthLimit`.

**Verification commands run from the clone root** (vendored, offline toolchain):

- `go build ./pkg/scheduler/...` succeeded.
- `go vet ./pkg/scheduler` succeeded.
- `go run ./cmd/import-boss -v 0 ./pkg/scheduler/...` (the same tool `hack/verify-import-boss.sh` uses) reported no violations.
- `go test ./pkg/scheduler -run 'TestSchedulerScheduleOne|TestUpdatePod|TestHandleSchedulingFailure|TestSchedulerFailedSchedulingReasons|TestSchedulerNoPhantomPodAfterExpire|TestSchedulerNoPhantomPodAfterDelete|TestSchedulerBinding' -count=1` passed (`ok k8s.io/kubernetes/pkg/scheduler 22.181s`).

## Checks against the skill's categories

- **Bugs / correctness:** none. Same constant value, same arithmetic, same control flow.
- **Breaking existing functionality:** none. The only cross-package effect is the removal of a dependency edge; no other package depended on the scheduler importing `pkg/apis/core/validation`.
- **Security:** none. No new input handling, no change to the truncation bound, no new trust boundary.
- **Developer experience:** none. No scripts, environment variables, ports, secrets, or build steps change. The import-boss rule change is enforced by the existing `hack/verify-import-boss.sh` and passes.
- **Feature-gate leaks:** not applicable; no gated code is touched.

## Non-actionable observation (not a finding)

The local constant is not tied to `validation.NoteLengthLimit` by any test, so if the API server limit were ever raised the scheduler would keep truncating at 1024. That is safe (messages are never rejected) and is exactly the trade-off the commit comment documents, so it is recorded here only for completeness and not reported as an issue.
