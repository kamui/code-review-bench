# Detail: `packages/integrations/vercel/src/serverless/entrypoint.ts` (+ changeset)

Range reviewed: `b089b904f1ed578e9edaefd129bf9843120a808f..71ae513388df11d7dad6b1e0077c402ad03d0d62` (`git diff main...review-head`), 2 files, +12/-1. Entrypoint is 73 lines after the change, so there is no file-size concern.

## Change as written

```ts
let realPath = undefined;
if(hasValidMiddlewareSecret) {
	realPath = request.headers.get(ASTRO_PATH_HEADER)
} else if(request.headers.get('x-vercel-isr') === '1') {
	realPath = url.searchParams.get(ASTRO_PATH_PARAM);
}
if (typeof realPath === 'string') { url.pathname = realPath; request = new Request(...) }
```

Context: `src/index.ts:63` defines `ISR_PATH = /_isr?x_astro_path=$0`; `src/index.ts:712` sets `allowQuery: [ASTRO_PATH_PARAM]`. `buildISRFolder` reuses `buildServerlessFolder` with the same entry, so the `_render` and `_isr` functions run this identical `fetch` handler. Commit 335a20416 (#15959) had made path overrides require the middleware secret; test `test/path-override-security.test.js` asserts that an untrusted `x_astro_path` query param and an untrusted `x-astro-path` header on `_render` are ignored.

## Finding 1 — The ISR branch re-opens the untrusted path override that #15959 closed (verification: CONFIRMED in-process; platform stripping of the header is UNVERIFIED)

Measurement: I built `test/fixtures/serverless-with-dynamic-routes` and imported the built `_render.func` handler in a scratch test (`clone-work/probe.test.js`, run with `node --test`; the clone is unchanged). Results:

- `GET https://example.com/api/public?x_astro_path=/api/private` with header `x-vercel-isr: 1` and no middleware secret returned `200 {"id":"private"}`. The same request without the header returns `public` (this is what the existing security test asserts).
- `GET https://example.com/_isr?x_astro_path=/api/private` with the same header returned `200 {"id":"private"}`.

So the trust gate is now a plain request header, `x-vercel-isr`, which is not authenticated by anything in the adapter. Anyone who can reach a function with a client-controlled `x-vercel-isr: 1` header can choose the path Astro routes on. The non-ISR `_render` function, which never should honour this, also honours it, because both functions share the entrypoint. Whether Vercel's edge strips or overwrites a client-supplied `x-vercel-isr` header is not documented in the repo and I could not check the platform; the PR body says nothing about it and the added change has no test. If the platform does not strip it, this reverts the hardening of #15959 (routing-level path confusion that can bypass path-based middleware/auth matching evaluated on the outer URL). If it does strip it, the code still relies on an undocumented platform invariant with no comment.

Code-judo direction: do not derive trust from a header at all. The entrypoint can only learn "I am the ISR function" from something the build controls. Options, in order of preference:

1. Emit a distinct, build-time constant for the ISR function (for example, generate the ISR function's handler with a wrapper that passes `{ isr: true }`, or bake a per-build flag into `virtual:astro-vercel:config` for the `_isr` bundle) so `_render` can never take the query-param branch. This deletes the runtime header sniffing entirely and makes the security test extend naturally ("`_render` ignores `x_astro_path` even with `x-vercel-isr: 1`").
2. If runtime detection is unavoidable, key it on the pathname (`url.pathname === '/_isr'`) in addition to the header so `_render` cannot be steered, and add a comment recording the platform guarantee relied upon.

## Finding 2 — Trust-source branching is bolted inline onto the entrypoint `fetch`, with a magic string and loose typing (verification: CONFIRMED by reading)

The two trust modes (secret-authenticated header vs ISR query param) are now interleaved in a mutable `let realPath = undefined` inside `fetch`, next to `hasValidMiddlewareSecret`, which is reused later for locals validation and secret stripping. A reader must now track two independent notions of "trusted source" in one function. Problems:

- `'x-vercel-isr'` is an inline literal, while every sibling name (`ASTRO_PATH_HEADER`, `ASTRO_PATH_PARAM`, `ASTRO_MIDDLEWARE_SECRET_HEADER`) is an exported constant from `../index.js`. It belongs next to them.
- `let realPath = undefined;` infers a loose type and then needs `typeof realPath === 'string'` to narrow; the value is `string | null | undefined`. An explicit `string | null` (or a pure helper returning it) states the invariant.
- The ISR branch is a special case in a flow that until now was "secret ⇒ header override". Pull it into a small pure function, `resolveRealPath(request, url, hasValidMiddlewareSecret): string | null`, which lets the path-override rules be unit-tested without building a fixture and keeps `fetch` a straight line.
- Formatting deviates from the file and repo style (`if(` without a space, missing semicolon after `request.headers.get(ASTRO_PATH_HEADER)`); the repo formatter would rewrite these.

## Finding 3 — Internal `x_astro_path` param stays on the rewritten request URL (verification: CONFIRMED by reading; runtime impact not exercised)

After `url.pathname = realPath`, `url.search` still contains `?x_astro_path=...`, and the new `Request` is built from that URL. User code then sees an adapter-internal parameter in `Astro.url.searchParams` / `request.url`. Because `allowQuery` is limited to this param, that is the only query the ISR function ever sees, so deleting it (`url.searchParams.delete(ASTRO_PATH_PARAM)`) in the same branch that consumes it would make the rewrite fully hide the transport detail. Low severity; the same helper from Finding 2 is the natural home.

## Finding 4 — No test for the regression this PR fixes or for the new trust rule (verification: CONFIRMED)

The PR states "No additional test cases added"; the fix was checked manually on a deployment. `test/isr.test.js` only inspects generated config, and `test/path-override-security.test.js` exercises only `_render`. Both files already load built handlers in-process (`loadFunctionModule`), so a two-case addition is cheap: (a) `_isr`-style request with `x-vercel-isr: 1` and `x_astro_path` resolves the real route, and (b) the same headers on `_render` (or without ISR gating) do not. Case (b) would have exposed Finding 1. The 404 that #15959 introduced also went unnoticed until after merge, which is the cost of that gap.

## Changeset

`.changeset/common-cats-travel.md` is well-formed (`'@astrojs/vercel': patch`) but "Fix vercel ISR path rewrite" says nothing user-visible about the symptom (all ISR routes 404 after #15959); a one-line description of the symptom would be more useful in the changelog. Nit.

## Proposed remediation sequence

1. Make ISR detection build-time, not header-based (Finding 1), or at minimum restrict it to the `_isr` pathname and document the platform guarantee.
2. Extract `resolveRealPath` with typed return, exported header constant, and param deletion (Findings 2, 3).
3. Add in-process tests for both directions (Finding 4).
