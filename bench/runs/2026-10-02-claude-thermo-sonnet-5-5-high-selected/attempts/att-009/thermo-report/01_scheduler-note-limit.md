# Detail 01: pkg/scheduler note-length limit decoupling

Scope: `git diff main...review-head` (3 hunks in `pkg/scheduler/schedule_one.go`, 1 removed rule in `pkg/scheduler/.import-restrictions`).

## Measurements and commands

- `go vet ./pkg/scheduler/` (vendored toolchain, GOFLAGS=-mod=vendor) exits cleanly on the head revision, so the package still builds without the `validation` import.
- `grep NoteLengthLimit` across pkg, cmd, test, and the client-go/api staging trees finds the canonical constant only in `pkg/apis/core/validation/events.go:37` (value 1024, used at lines 183-184). The scheduler copy is the only other definition. No test references both values.
- `grep` for `apis/core/validation"` under `pkg/scheduler`, `test/integration/scheduler`, and `test/integration/scheduler_perf` finds no remaining importers, so removing the `.import-restrictions` allow rule is consistent. Remaining mentions in `pkg/scheduler` are comments only.
- `truncateMessage` has callers at `schedule_one.go:838` and in tests at `schedule_one_test.go:2098` and `:5062`. Those tests call `truncateMessage` itself and never reference the canonical constant.

## Finding 1: Duplicated constant has no machine-checked link to its source (Verified, low severity)

`pkg/scheduler/schedule_one.go:58-61` introduces `noteLengthLimit = 1024`. The comment claims "API validation limits will never become lower than an already released version". That is a reasonable compatibility argument, but the argument only covers a limit going down. If the API limit is ever raised, or if the constant is split per field, the scheduler silently keeps truncating at 1024. That is safe for correctness because shorter messages still pass validation, but it is silent drift. Nothing in the diff, such as a test in a package that may import both, a `hack/verify` check, or a lint rule, pins the copy to the original. The comment is the only link, and comments are the weakest form of coupling.

Evidence that the guard is missing: the two scheduler tests that touch truncation call `truncateMessage` and compare against its own output (`schedule_one_test.go:2098`, `:5062`), so a wrong constant would still pass them.

## Code-judo proposals

1. **Cheapest and most direct.** Keep the local constant and add one test file that is allowed to import the validation package. `pkg/scheduler/.import-restrictions` applies to non-test code in the same way, so this would need either an integration-test location such as `test/integration/scheduler` or a restriction exception for `_test.go`. The test would assert `noteLengthLimit <= validation.NoteLengthLimit`. This turns the comment's "never lower" claim into an enforced invariant at near-zero cost.
2. **Structural.** Move the limit to a leaf location that both the API validation and the scheduler can import without dragging in `pkg/apis/core`. Examples are a constant in `k8s.io/api/core/v1` or in `k8s.io/kube-scheduler/framework`. The scheduler already depends on both. That removes the duplicate rather than documenting it, but it is a cross-component API change and probably out of proportion for a dependency-trimming PR. It is worth recording as the end-state if more such constants get copied.

## Non-findings (checked and acceptable)

- The `max` local variable was removed, which also stops shadowing the Go builtin `max`. Good.
- `truncateMessage` still slices on bytes and can split a multi-byte rune. This is pre-existing and matches the API's byte-length (`len`) validation. It is not introduced here.
- The new constant lives in the existing `const` block next to the other unexported scheduling constants. Placement and naming follow local style.
- The `.import-restrictions` edit matches the code change and tightens the allowlist as intended.
