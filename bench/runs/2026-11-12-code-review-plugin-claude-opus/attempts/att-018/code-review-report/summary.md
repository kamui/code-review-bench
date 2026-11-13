# Code review — trpc/trpc#5017 "fix(server): inference fix for inputs with middleware"

Range reviewed: `main...review-head` (2abb2d5cd19740be37272dac6ad7fdd36244ae54..7dc04a7e94654dfad6ef1289dfe01a0a206fff3b)

## Result

No issues found. Checked for bugs and CLAUDE.md compliance.

Step 1 did not stop the review: the PR is not a draft and needed review. Step 6 kept no issue.

No actionable findings.

## Process notes

- **CLAUDE.md:** no CLAUDE.md exists at the repo root or in any ancestor directory of the changed files, so neither compliance agent had anything to report.
- **Bug review:** the two bug reviewers raised two candidate issues. Both failed validation in step 5 and were dropped in step 6:
  1. **`never` fallback is dead code** (`packages/server/src/core/internals/utils.ts`, doc comment lines 6–9 and the `: TType` fallback in lines 25–30). Because `TWith extends any` distributes over `never`, the "unless TWith is never" `: TType` branch is unreachable. Rejected because `main` behaves the same way (not a regression), no real caller passes `never`, and the remaining problem is only an inaccurate comment.
  2. **An `unknown` `TWith` replaces an object `TType`** (`packages/server/src/core/internals/utils.ts`, the object-`TType`/non-object-`TWith` branch, about lines 22–24). Calling `opts.next({ ctx: u })` with `u: unknown` now drops the root ctx keys. `any` already loosened ctx on `main`, so it is not a regression. Rejected because this only happens with an unusual input. The common paths behave as on `main`: `next()` with no argument, object ctx, chained middleware, and input merges.
