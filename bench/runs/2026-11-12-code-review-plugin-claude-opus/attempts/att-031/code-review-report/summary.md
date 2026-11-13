# Code review: kubernetes/kubernetes#141463

**PR:** scheduler: replace pkg/apis/core/validation dependency with local const
**Range:** 6bb42350227f0a714f4730165e7ba622c69bd99e..d9cf69d74a25a209e79c7061ff86a14ab4b4b634 (`main...review-head`)

## Result

Step 1 stopped the review. The PR is not a draft, but the step 1 eligibility check found that it doesn't need code review: it is a trivial, obviously correct change. Steps 2–6 kept no issues because steps 4–6 never ran.

Why it counts as trivial:
- `pkg/scheduler/schedule_one.go` drops the `k8s.io/kubernetes/pkg/apis/core/validation` import. It replaces `validation.NoteLengthLimit` with a local `noteLengthLimit = 1024`, which matches `NoteLengthLimit = 1024` in `pkg/apis/core/validation/events.go:37`. `truncateMessage` behaves exactly as before.
- `pkg/scheduler/.import-restrictions` removes the allow rule for `pkg/apis/core/validation`. No other file under `pkg/scheduler` imports that package, so the tighter rule won't break the import check.
- No CLAUDE.md files apply: none exist at the repository root, `pkg/`, or `pkg/scheduler/`.

No actionable findings.
