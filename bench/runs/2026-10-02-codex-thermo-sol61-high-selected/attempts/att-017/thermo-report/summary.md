# Kubernetes scheduler dependency review

Reviewed `kubernetes/kubernetes#141463` at base `6bb42350227f0a714f4730165e7ba622c69bd99e` and head `d9cf69d74a25a209e79c7061ff86a14ab4b4b634`, using the committed `main...review-head` diff. The packet's revision cutoff is `2026-09-09T20:30:36Z`. This is one primary review using the frozen thermo-nuclear-code-quality-review skill; no independent reviewers or alternate models were used. Repository guidance, upstream discussion, later revisions, and external review material were not consulted.

## Verdict and findings

The change meets the skill's structural approval bar. There are no actionable findings.

In `pkg/scheduler/schedule_one.go:58–61`, the documented local constant replaces a dependency on the server's core-validation implementation. Its value is the same 1024-byte limit enforced by `pkg/apis/core/validation/events.go:37,183–184` and described by the public Event API. At `schedule_one.go:851–857`, substituting this value for the old constant and deleting the intermediate variable preserves the existing truncation algorithm. This removes coupling without introducing wrappers, modes, casts, or additional control flow. The conservative client limit need not increase whenever a server accepts larger notes. The full boundary analysis and worked alternatives are in [01_scheduler_event_boundary.md](01_scheduler_event_boundary.md).

In `pkg/scheduler/.import-restrictions`, removing the explicit core-validation exception makes the remaining Kubernetes catch-all reject that direct dependency. The scheduler subtree contains no remaining direct import of the removed package, and the actual import-boss check succeeds for production and test packages. This is a matching restriction change, with no new exception or enforcement mechanism. The evidence and rule evaluation are in [02_import_restrictions.md](02_import_restrictions.md).

The diff has seven insertions and seven deletions across two files. `schedule_one.go` grows from 890 to 892 lines, and `.import-restrictions` shrinks from 28 to 26 lines. No file crosses 1000 lines. The truncation helper retains one conditional and no new state, sequencing, or partial updates. No obvious decomposition or dramatic simplification has been missed: the dependency-removal move is already the simpler design for this scope.

## Verification

The focused scheduler test command passed with `ok k8s.io/kubernetes/pkg/scheduler 18.461s`. It selected `TestSchedulerScheduleOne`, `TestSchedulerFailedSchedulingReasons`, and all `TestHandleSchedulingFailure*` tests, running once with `-count=1`. This verifies compilation and the relevant existing scheduling and failure-handling paths; it is not an API-server integration or exhaustive boundary test.

The scheduler-subtree import-boss command passed, reported 147 loaded package representations, and ended with `Completed successfully.` A focused `go list` confirmed that the scheduler's direct imports exclude core validation. `git diff --check main...review-head` passed. All Go commands used the provided toolchain and caches, vendor mode, disabled dependency fetching, the checked-in workspace, and a five-minute command limit.

Current behavior equivalence at the truncation boundary also follows directly from the source: both constants equal 1024, both branches compare the byte length using `<=`, and both over-limit expressions retain the first 1020 bytes and append the same four-byte suffix. Existing byte-slicing and event-formatting behavior was not changed by this PR and is not reported as a new defect.

## Remediation sequence and questions

No remediation is required for the reviewed change. Retain the documented conservative client limit and the removal of its import exception. No broader shared-constant migration or file extraction is justified by this diff; the detail files explain the evaluated alternatives and why they do not delete additional complexity here.

There are no unresolved review questions. The full Kubernetes test suite, race testing, API-server integration, and speculative future validation changes were outside the checks performed. The reports make no claim about the entire scheduler dependency closure becoming independent of core validation.

## Checkout integrity

The checkout was clean at the start and remained unchanged during review. Head remained `d9cf69d74a25a209e79c7061ff86a14ab4b4b634`, whose tree is `f352e4a83a44eebca7557612450597686a4ddb4d`. Reports were written only in the supplied work directory. The finding index records the explicitly empty findings and questions sets without converting observations or worked alternatives into findings.
