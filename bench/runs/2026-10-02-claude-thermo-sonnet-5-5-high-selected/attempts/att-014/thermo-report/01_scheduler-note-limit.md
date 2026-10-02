# Detail 01: pkg/scheduler note length limit decoupling

Range: `6bb42350227f0a714f4730165e7ba622c69bd99e..d9cf69d74a25a209e79c7061ff86a14ab4b4b634` (`git diff main...review-head`).
Files touched: `pkg/scheduler/schedule_one.go` (+7/-5), `pkg/scheduler/.import-restrictions` (-2).

## What the diff does

`schedule_one.go` stops importing `k8s.io/kubernetes/pkg/apis/core/validation`, which it used only for `validation.NoteLengthLimit` inside `truncateMessage`. It adds a package-level constant `noteLengthLimit = 1024` at `pkg/scheduler/schedule_one.go:58-61`, with a comment saying it copies the API validation constant. `truncateMessage` (`schedule_one.go:851-858`) now reads that constant directly, and its local `max := ...` alias is gone. The matching allow-rule for `pkg/apis/core/validation` is removed from `pkg/scheduler/.import-restrictions`.

## Measurements and verification

- `grep -rn "apis/core/validation" pkg/scheduler test/integration/scheduler*` shows only comment references (`dynamicresources/nodeallocatabledynamicresources.go:403,531`, `apis/config/validation/validation_pluginargs.go:100`). No Go import of the package is left under `pkg/scheduler`, so removing the allow-rule is consistent. The catch-all `^k8s[.]io/kubernetes` forbid rule now covers the package.
- The upstream value is `NoteLengthLimit = 1024` at `pkg/apis/core/validation/events.go:37`. It is enforced at `events.go:183-184` with `len(event.Message) > NoteLengthLimit`. That is a byte-length check, and `truncateMessage` uses `len`, so the units match. The copy is value-correct today.
- The only callers of `truncateMessage` are `schedule_one.go:838` and the two tests at `schedule_one_test.go:2098` and `:5062`. The tests call the function and do not name the constant, so they need no change.
- `schedule_one.go` is 892 lines, so the 1k-line threshold is not at issue.
- I did not run tests. The change is a mechanical constant substitution with identical behavior.

## Findings

### F1. The copied constant has no drift guard, and the comment states an unenforced invariant (low severity)

`pkg/scheduler/schedule_one.go:58-61`. The new comment says that "a local copy is safe because API validation limits will never become lower than an already released version." That is a reasonable compatibility argument, but it is a promise made in prose and nothing checks it. The only test-time link between the scheduler and the API limit used to be the import itself. Now the two numbers can diverge without any signal.

Divergence has two directions. If the API limit is ever raised, the scheduler silently keeps truncating at 1024, which is harmless but leaves the two values out of step. If someone lowers it, the scheduler would start sending events the server rejects. The comment says this will not happen, and the API compatibility rules support it, but a duplicated magic number is the kind of thing that needs a mechanical check or a single canonical home.

Remedy options, from least to most ambitious:

1. Add a tiny test outside `pkg/scheduler`, where importing both packages is allowed. It would assert that the scheduler's limit equals `validation.NoteLengthLimit`. The constant is unexported, so this needs either an exported name or an `export_test.go` shim. That pushes toward option 2.
2. Code-judo: move the limit to a leaf location that both sides can import, such as an exported constant in `k8s.io/api/events/v1` or `k8s.io/kubernetes/pkg/apis/core/validation`'s own dependency-free sibling package. Then `validation.NoteLengthLimit` and the scheduler share one definition, and the import-restriction change needs no duplicated number. This removes the "copy" concept entirely. It does widen the PR across modules, so it is a follow-up rather than a blocker.
3. Accept the duplicate, and mention in the `NoteLengthLimit` doc comment in `pkg/apis/core/validation/events.go` that the scheduler keeps a copy. That at least gives a grep-able link from the original to the copy.

Given the stated goal (drop a heavy `pkg/apis` dependency from the scheduler), option 3 plus the existing comment is acceptable. Option 1 or 2 is the stronger fix.

### F2. Constant sits in a shared tuning-constants block (nit, not actionable)

`noteLengthLimit` joins `pluginMetricsSamplePercent`, `minFeasibleNodesToFind` and `minFeasibleNodesPercentageToFind` in the const block at the top of `schedule_one.go`. It is unrelated to that group, which is about sampling and feasibility. Its sole user is `truncateMessage` about 800 lines later. Placing it next to `truncateMessage` would make the dependency obvious and keep the block cohesive. I do not count this as a finding because it is a placement preference and the block is already a loose grab-bag.

## Things checked and found clean

- No conditionals, flags, or wrappers were added. The `max` alias was removed, which is a small simplification.
- No behavior change: `1024-len(" ...")` = 1020 bytes of the original message plus the suffix, same as before.
- Pre-existing, outside the diff: `truncateMessage` slices bytes, so it can cut a multi-byte UTF-8 rune in half. This is unchanged by the PR and not reported as a finding.
