# Thermo-nuclear review: kubernetes/kubernetes#141463

Range: `6bb42350227f0a714f4730165e7ba622c69bd99e..d9cf69d74a25a209e79c7061ff86a14ab4b4b634` (`main...review-head`), one commit, two files:
`pkg/scheduler/schedule_one.go` and `pkg/scheduler/.import-restrictions`.

No actionable findings.

## What the change does

`truncateMessage` in `pkg/scheduler/schedule_one.go` used `validation.NoteLengthLimit` from
`k8s.io/kubernetes/pkg/apis/core/validation`. The commit replaces that with a package-local
`noteLengthLimit = 1024`, drops the import, and removes the matching allow rule from
`pkg/scheduler/.import-restrictions`.

## What was checked

**Behaviour is unchanged.** `pkg/apis/core/validation/events.go:37` defines `NoteLengthLimit = 1024`, the same value as
the new local constant, so `truncateMessage` cuts at the same byte offset as before. Its only production caller is
`handleSchedulingFailure` (`schedule_one.go:838`), which truncates the `FailedScheduling` event note; the two test
callers in `schedule_one_test.go` (lines 2098, 5062) use the same function. The limit was a compile-time constant
before and after, so scheduler/API-server version skew behaves as it did.

**The removed import-restriction rule is not needed by anything else.** No file under `pkg/scheduler` (including tests
and subpackages) imports `k8s.io/kubernetes/pkg/apis/core/validation` at head, and this is the only
`.import-restrictions` file in that tree. `import-boss`, built from `cmd/import-boss` and run on `./pkg/scheduler/...`
at head, reported no violations.

**Nothing the scheduler relies on leaves the binary.** Comparing `go list -deps` for `./pkg/scheduler` and
`./cmd/kube-scheduler` at head against the base version of `schedule_one.go` (via a build overlay), 17 packages drop
out: `pkg/apis/core/validation`, `pkg/apis/core/v1`, `pkg/apis/core/pods`, `pkg/apis/core/helper/qos`,
`pkg/apis/{apps,autoscaling,certificates}`, `pkg/api/service`, `pkg/api/v1/service`, `pkg/capabilities`,
`pkg/fieldpath`, `pkg/util/parsers`, `k8s.io/kubelet/pkg/apis`, `k8s.io/component-helpers/node/util/sysctl`,
`github.com/distribution/reference`, `github.com/opencontainers/go-digest`, `github.com/robfig/cron/v3`.
None of them registers flags, metrics or feature gates. The only `init` functions are in `pkg/apis/core/v1`, and they
only append to that package's own `localSchemeBuilder`; neither `pkg/api/legacyscheme` nor `pkg/apis/core/install` was
in the scheduler's dependency set before or after, so no scheme the scheduler uses loses registrations. The test
dependency set for `./pkg/scheduler` shrinks by the same 17 packages.

**Build and tests.** `go build ./pkg/scheduler/` and `go vet ./pkg/scheduler/` succeed at head.
`go test ./pkg/scheduler -run 'TestSchedulerScheduleOne$|TestSchedulerFailedSchedulingReasons|TestSchedulerWithVolumeBinding' -count=1`
passes. The full `pkg/scheduler` suite and the wider Kubernetes verify scripts were not run (outside the allowance).

**Other categories.** No feature gates, secrets, environment variables, ports, or build/run steps are touched, and
`git diff --check` is clean.

## Non-actionable notes

- The new comment's rationale (API validation limits are not lowered after release) holds for the direction that
  matters: if the upstream limit were ever raised, the scheduler would only truncate earlier than strictly required.
- The commit message carries a leftover `# Conflicts: pkg/scheduler/schedule_one.go` block from a rebase. Cosmetic only.
