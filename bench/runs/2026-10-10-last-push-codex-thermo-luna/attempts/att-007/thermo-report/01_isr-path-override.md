# ISR request path override

## Finding

**The new ISR override has no runtime regression coverage.** In `packages/integrations/vercel/src/serverless/entrypoint.ts:24-25`, the handler now reads `x_astro_path` only when `x-vercel-isr` equals `1`. This is the request-time behavior that fixes ISR route dispatch, and the marker is also the boundary that prevents ordinary requests from using the query parameter as a path override. Neither behavior is asserted in the committed tests. `test/isr.test.js` verifies the generated prerender config and route table; `test/path-override-security.test.js` verifies that untrusted path overrides are ignored on `_render`, but does not exercise this new ISR branch.

The test gap matters because the PR's manual Vercel checks cannot run in this review environment, while an in-process handler test is supported by the existing fixture setup. The existing `isr` fixture contains the routes needed to observe dispatch. A focused addition can load the built `_isr` handler, send a request carrying `x-vercel-isr: 1` and `x_astro_path=/one`, and assert the response matches `/one`; a second request with the query but without the marker should demonstrate that the query does not select the route. Keep this as test coverage around the existing branch rather than extracting a helper solely to make these two decisions look abstract.

## Evidence

The diff adds the conditional at `entrypoint.ts:21-26`; the specific gate and query read are at lines 24-25. The existing ISR tests in `packages/integrations/vercel/test/isr.test.js` assert only the prerender config and route generation. `packages/integrations/vercel/test/path-override-security.test.js` calls the `_render` function and checks query/header override handling there.

## Verification status

Ran the permitted focused command from `packages/integrations/vercel`:

`node --test test/path-override-security.test.js test/isr.test.js`

Result: 4 tests passed, 0 failed. This confirms the current build/config and `_render` security assertions; it does not validate the added ISR request-time branch. Vercel's platform routing and ISR cache were unavailable, so no platform-level behavior was verified.

## Worked code-judo proposal

Extend the ISR fixture test to load and call its built `_isr` handler, with a small table of request cases: marker plus `x_astro_path` must dispatch to the requested route; no marker plus the same query must retain the original request path. This uses the fixture and handler-loading pattern already present in `path-override-security.test.js`, puts the policy at its actual execution boundary, and avoids production helper indirection. If the tests reveal that the marker is not an adequate trust signal, clarify the boundary there before adding more production logic.
