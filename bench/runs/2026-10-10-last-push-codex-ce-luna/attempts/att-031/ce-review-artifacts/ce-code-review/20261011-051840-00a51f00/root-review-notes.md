# Root review notes

Intent: Restore Vercel ISR page rendering by using the route path carried in `x_astro_path` when the ISR handler receives an `x-vercel-isr: 1` request.

Scope: `b089b904f1ed578e9edaefd129bf9843120a808f..71ae513388df11d7dad6b1e0077c402ad03d0d62`; current branch `review-head`; two changed files. No untracked files.

Depth: Focused. Runtime route selection can fail silently as a 404, but the change is not on an auth, money, or public API contract boundary.

Independent adversarial return was preserved in `adversarial-reviewer.json` and `raw-returns.json`. It noted that the secret branch takes precedence over the ISR query path if both headers are present and the path header is absent. The available source does not establish that Vercel sends that combination, so this is retained only as a residual risk, not a finding.

Root correctness finding: ISR extracts `ASTRO_PATH_PARAM` from the function URL and updates only `url.pathname`; `url.searchParams` remains unchanged when the rewritten Request is passed to `app.match` and `app.render`. Therefore Astro routes receive the internal `x_astro_path` query parameter in addition to the original query. Recommend deleting `ASTRO_PATH_PARAM` after reading its path, preserving other query parameters.

Testing: No test changes were made. Existing `isr.test.js` verifies generated configuration and routes but does not invoke the ISR handler. No tests were run in this review.

Criteria: Root `AGENTS.md` is the only applicable instruction-file source for the changed files; it defines no direct style rule relevant to the changed behavior. No `CODING_STANDARDS.md` governs these paths. No plan or declared Compound Packs were found.
