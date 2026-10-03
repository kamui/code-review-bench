# Scheduler dependency boundary and event-note truncation

## Scope and judgment

There are no actionable findings in this subsystem. The two changed files form one cohesive boundary change: replace the scheduler's only direct use of a server-validation symbol with a private output-limit constant, then remove permission to import that server package. The review examined the committed diff, the validator, the public event contract, the concrete event recorder, relevant scheduler tests, and import-boss evaluation semantics. It did not edit or apply a remedy to the checkout.

The base is `6bb42350227f0a714f4730165e7ba622c69bd99e`; the head is `d9cf69d74a25a209e79c7061ff86a14ab4b4b634`. `git rev-parse HEAD main review-head` confirmed the packet's revisions, and `git show --no-patch --format=fuller review-head` identified the dependency-removal commit. Only this pinned range was reviewed.

## Measurements and source evidence

`git diff main...review-head --stat` reports two changed files, seven insertions, and seven deletions. `git diff main...review-head --numstat` attributes seven insertions and five deletions to `schedule_one.go`, and two deletions to `.import-restrictions`. The diff removes one import, adds one private constant with its rationale, removes a redundant local alias in the truncation helper, and removes one import-rule exception.

`git show main:pkg/scheduler/schedule_one.go | wc -l` gives 890 lines; `wc -l pkg/scheduler/schedule_one.go` gives 892. The restriction file shrinks from 28 to 26 lines. Neither changed file crosses 1000 lines. No new function, conditional, switch, nullable mode, interface, cast, goroutine, or state mutation appears in the diff. The pre-existing scheduling workflow is large, but the two-line net growth does not create a decomposition blocker.

At `pkg/apis/core/validation/events.go:33–38`, `NoteLengthLimit` is the untyped constant 1024. At lines 183–184, event validation compares `len(event.Message)` against that value. At `staging/src/k8s.io/api/events/v1/types.go:81–85`, the public note field documents a maximal length of 1kB and separately tells libraries to handle values up to 64kB. The latter is a library-readiness statement, not permission to emit 64kB notes under the pinned validator.

At `pkg/scheduler/schedule_one.go:58–61`, the new private constant uses the same numeric value and explains its source and the reason for keeping a local copy. At lines 838–845, the scheduler sends `truncateMessage(errMsg)` to the warning-event recorder while retaining `errMsg` for the pod condition. Those call sites are unchanged.

The source searches used `rg -n 'NoteLengthLimit|noteLengthLimit|truncateMessage' pkg staging --glob '*.go'` and `rg -n '"k8s.io/kubernetes/pkg/apis/core/validation"' pkg/scheduler --glob '*.go'`. The first locates the validator, helper, production caller, and existing test callers. The second produces no matches at head. That no-match result is expected and confirms that no scheduler Go source still needs the removed direct-import exception.

To check whether the local copy unnecessarily bypasses a canonical public helper, the review searched `staging/src/k8s.io/api`, `apimachinery`, `client-go`, `component-helpers`, and `kube-scheduler` for `func .*Truncat|NoteLengthLimit|[Nn]oteLength|[Nn]oteLimit`. It found no exported event-note limit or equivalent truncation utility. Unrelated resource-health limits and body-truncation tests do not own this contract.

## Behavior equivalence and invariant

The old helper assigns `max := validation.NoteLengthLimit`, returns its input when `len(message) <= max`, and otherwise returns `message[:max-len(" ...")] + " ..."`. Substituting the pinned constant value yields exactly the new helper. Both limits are untyped integer constants, so the substitution changes neither the comparison nor the slice index semantics.

| Input byte length | Base result | Head result |
| --- | --- | --- |
| 0 through 1024 | Original input | Original input |
| 1025 or greater | First 1020 bytes plus `" ..."` | First 1020 bytes plus `" ..."` |

The suffix consumes four bytes, leaving a positive 1020-byte slice bound. For every input taking the slice branch, the input length exceeds 1024, so that bound is valid. The output in that branch is exactly 1024 bytes. This is a source-level equivalence proof, not a claim that the selected tests explicitly exercise each row.

The helper continues to operate on bytes, including the pre-existing possibility of cutting through a multibyte character. The refactor does not introduce or expand that behavior. Changing encoding behavior would require a separate behavior-sensitive change, rather than being a necessary remedy for this constant substitution.

The new comment's future-compatibility rationale depends on preserving the released API's accepted limit. If the server raises its limit, a scheduler retaining 1024 stays conservative and merely emits a shorter note than newly permitted. The pinned sources establish today's equality and the published note contract; these tests cannot certify every future validation-policy decision. No evidence in this range contradicts the intended stable client-side budget.

## Import-policy enforcement

The removed rule previously matched exactly `k8s.io/kubernetes/pkg/apis/core/validation` and allowed it using the empty prefix. The remaining specific allowances do not match that path, and the final selector `^k8s[.]io/kubernetes` with `forbiddenPrefixes: [ "" ]` remains at `pkg/scheduler/.import-restrictions:24–26`.

`cmd/import-boss/main.go` supplies the implementation evidence. `Rule.Evaluate` checks forbidden prefixes before allowed prefixes, and `hasPathPrefix` treats the empty prefix as a match. `verifyRules` walks matching rules until a decision is made. Its `!rule.Transitive && !isDirect[imp]` guard skips indirect imports for the ordinary rules used here. Removing the exception therefore prohibits restoring the direct server-validation import; it does not promise elimination of that package from the entire transitive graph.

The review read import-boss source and the relevant verification script as implementation evidence. It did not execute repository setup scripts or load repository guidance as review instructions. The focused `go run` invocation below exercised the actual policy on the scheduler subtree. The loader sets `Tests: true`, so this also checks the test variants affected by the inherited restriction file.

## Worked code-judo analysis

The useful structural move is already present: give the client its own conservative output budget, remove its direct knowledge of server-side validation machinery, and forbid that dependency from returning. The resulting implementation remains direct:

```go
const noteLengthLimit = 1024

func truncateMessage(message string) string {
    if len(message) <= noteLengthLimit {
        return message
    }
    suffix := " ..."
    return message[:noteLengthLimit-len(suffix)] + suffix
}
```

This is the head's behavior with only the surrounding comments and constant group omitted for illustration. It needs one private constant and the existing helper, with no migration modes or runtime negotiation. Deleting the temporary `max` alias also removes a local name readers previously had to resolve.

One possible alternative would export a note-length constant from a staging API or helper package and consume it from both validation and the scheduler. That would unify literal ownership, but would also introduce a public symbol and coordinated cross-package changes for a single stable number. There is no existing suitable symbol to reuse, and the public event type already states the protocol limit. For this change, the documented local budget is a cleaner boundary than expanding the shared API solely to avoid a literal copy. No shared-constant extraction is requested.

Another possible alternative would delete `truncateMessage` and rely on the event recorder. Inspection of `staging/src/k8s.io/client-go/tools/events/event_recorder.go:72–100` shows that it formats the note, constructs an event, and enqueues it. Its `makeEvent` assigns `Note: message` without applying a length cap. Removing the scheduler helper would therefore allow oversized notes to reach validation and would not preserve behavior. Adding truncation to the shared recorder would change behavior for all recorder clients, so it is not an evident simplification for this narrowly scoped dependency removal.

Extracting the constant and seven-line helper into a separate scheduler module would leave the same logic and add another file to navigate. The file stays below the size threshold, the constant is private, and the helper remains at the event-emission boundary. No meaningful abstraction or complexity reduction follows from that extraction. No decomposition or generic truncation framework is requested.

## Verification commands and results

All Go commands ran from the clone root with the checked-in workspace preserved. The executable was `/home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high-selected/att-012/clone-cache/toolchain/bin/go`. `GOROOT`, `GOMODCACHE`, and `GOCACHE` pointed respectively to `clone-cache/toolchain`, `clone-cache/gomodcache`, and `clone-cache/gocache` under the same attempt directory. The commands set `GOFLAGS=-mod=vendor`, `GOPROXY=off`, `GOSUMDB=off`, and `GOTOOLCHAIN=local`. Each was wrapped in `timeout 300s`. The import-boss command additionally prepended the cached toolchain's `bin` directory to `PATH`, so its package loader used the permitted Go executable.

The following focused test ran once with this flag set:

```sh
go test ./pkg/scheduler \
  -run '^(TestSchedulerScheduleOne|TestSchedulerFailedSchedulingReasons|TestHandleSchedulingFailure.*)$' \
  -count=1 -timeout=4m
```

It exited 0 and reported `ok k8s.io/kubernetes/pkg/scheduler 18.197s`. This checks package compilation and selected scheduling and failure-handling paths, including the existing asynchronous API-call test variants where supplied. The existing tests are not a dedicated API-validation integration test or an independent assertion of the truncation limit.

The focused policy verification ran once:

```sh
go run ./cmd/import-boss -v 2 ./pkg/scheduler/...
```

It exited 0, loaded 147 package entries including test variants, and reported `Completed successfully.` This confirms that the removed exception does not strand a scheduler source or test import and that the remaining restriction configuration parses and evaluates successfully.

`git diff --check main...review-head` exited 0 with no output. `git status --porcelain=v1` was empty before and after review. The final `git rev-parse HEAD main review-head` and `git rev-parse HEAD^{tree}` retained the pinned revisions and tree `f352e4a83a44eebca7557612450597686a4ddb4d`.

No package test was repeated, no full Kubernetes suite was run, no dependencies were fetched, no fixture server was needed, and no checkout file was changed. The skill contains no mandatory child-review call or referenced resource needed for this workflow, so the review remained in its one primary context.

## Remediation and questions

No actionable remediation is required. Keep the explicit client-side cap, its provenance comment, the direct helper flow, and the stricter import boundary. The alternatives above do not demonstrate a behavior-preserving structural improvement over this patch. There are no open review questions.
