# Vercel serverless entrypoint review

## Scope and measurements

Reviewed the committed range `main...review-head` for the two-file change in PR #16079. The implementation change is in `packages/integrations/vercel/src/serverless/entrypoint.ts`, around lines 18–34. The entrypoint is a 72-line module; this change adds one import and a short source-selection branch. It does not approach the skill's 1,000-line decomposition threshold.

The surrounding adapter defines `ASTRO_PATH_HEADER` and `ASTRO_PATH_PARAM` in `src/index.ts`; the latter is used in the generated ISR rewrite destination and in the ISR prerender configuration allowlist. Middleware-authenticated requests already use the header to recover their original path. The new `x-vercel-isr`-gated parameter read is adjacent to that existing path selection, and both sources feed the same `url.pathname` rewrite before route matching.

## Findings

No actionable maintainability finding. The branch adds a Vercel-specific condition at the point where the serverless entrypoint interprets Vercel request metadata. That is the local owner of this adaptation, and the selected path is normalized once for the existing route-match/render flow. There is no evidence in the diff of duplicated rewrite behavior, unrelated feature checks, a new abstraction boundary, or growth in surrounding complexity that warrants a blocking recommendation.

## Code-judo assessment

The plausible local alternative is to express selection as a nested conditional assigned to a `const`, or to extract a path-selection helper. Neither deletes a meaningful concept: the entrypoint still has two provenance rules with different trust conditions, and the selected value still needs the common URL rewrite. A helper would wrap a single small decision without creating reuse, while a nested conditional would make the security/precedence rules less scannable. Keeping the explicit branch followed by one shared rewrite is the simpler structure here.

No stronger behavior-preserving restructure is evident from this diff. A broader abstraction for Vercel request metadata would be speculative because this change introduces only one additional metadata source at its existing boundary.

## Verification status

Executed from `packages/integrations/vercel`:

- `node --test test/path-override-security.test.js test/isr.test.js` — passed, 4 tests, 0 failures.
- `git diff --check main...review-head` — passed.
- `git status --short` — empty after review; the checkout was not modified.

The ISR test verifies generated routes and prerender configuration, while the path-override tests verify the existing protections against untrusted query/header overrides. The focused suite does not directly invoke the built ISR handler with `x-vercel-isr: 1` and `x_astro_path`; therefore that specific handler behavior is not covered by this verification run. This is recorded as a test-coverage limit, not a maintainability finding, because inspection found the new read connected to the existing rewrite path without a separate structural defect.
