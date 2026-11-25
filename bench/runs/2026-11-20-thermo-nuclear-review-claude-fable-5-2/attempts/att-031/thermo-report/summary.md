# Thermo-nuclear review: kubernetes/kubernetes#141463

Range: `6bb42350227f0a714f4730165e7ba622c69bd99e..d9cf69d74a25a209e79c7061ff86a14ab4b4b634` (`main...review-head`)

No actionable findings.

## What the change does

- `pkg/scheduler/schedule_one.go`: drops the import of `k8s.io/kubernetes/pkg/apis/core/validation` and replaces `validation.NoteLengthLimit` in `truncateMessage` with a new package-local constant `noteLengthLimit = 1024`.
- `pkg/scheduler/.import-restrictions`: removes the allow rule for `k8s.io/kubernetes/pkg/apis/core/validation`, so the catch-all "forbid any other k8s.io/kubernetes imports" rule now covers it.

## What was checked

- **Value parity.** `pkg/apis/core/validation/events.go:37` defines `NoteLengthLimit = 1024`; the local copy at `pkg/scheduler/schedule_one.go:61` is the same value, so `truncateMessage` (`schedule_one.go:852-858`) behaves identically. The slice arithmetic is unchanged.
- **Staleness direction.** The API server rejects events whose message exceeds the limit (`events.go:183`). If the upstream limit is ever raised, the scheduler only truncates earlier than necessary; it cannot produce a rejected event unless the limit is lowered, which would break already-released clients.
- **No remaining users of the removed import.** No file under `pkg/scheduler` (tests and subpackages included) imports `k8s.io/kubernetes/pkg/apis/core/validation`, and `schedule_one.go` has no other `validation.` reference. The only callers of `truncateMessage` are `schedule_one.go:838` and two test call sites.
- **Import-restriction semantics.** `pkg/scheduler/.import-restrictions` is the only restrictions file under `pkg/scheduler`, and its rules are not marked `transitive`, so the removed rule affects direct imports only. Packages that still reach core validation indirectly are unaffected.
- **No feature-gate, devex, or security surface.** No gates, environment variables, build scripts, or runtime behaviour are touched.

## Commands run

- `go vet ./pkg/scheduler/` — exit 0.
- `go run ./cmd/import-boss ./pkg/scheduler/...` — exit 0, no violations.
- `go test ./pkg/scheduler/ -run 'TestSchedulerScheduleOne|TestSchedulerFailedSchedulingReasons' -count=1` — `ok`.

The full scheduler and repository test suites were not run (outside the focused-command allowance).
