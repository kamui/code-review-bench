# Detail 01 — `packages/integrations/vercel/src/serverless/entrypoint.ts`

Range reviewed: `b089b904f..71ae51338` (`git diff main...review-head`), 2 files, +12/−1. The changeset file is trivial. All substance is in the 7 added lines of the entrypoint (now lines 21–26).

## Measurements and commands

- `git diff main...review-head` showed the change. The entrypoint is 73 lines, so file-size limits are not a concern.
- `git log -p -- packages/integrations/vercel/src/serverless/entrypoint.ts` showed that #15959 (335a20416) removed `?? url.searchParams.get(ASTRO_PATH_PARAM)` and made the path override require the middleware secret. This PR re-adds the query-param read behind a different gate.
- `grep ASTRO_PATH_PARAM|x-vercel-isr` over the adapter: `x-vercel-isr` appears only at `entrypoint.ts:24`, as a bare string literal. Every other adapter header is a named constant in `src/index.ts:44-48`. `ASTRO_PATH_PARAM` is defined at `index.ts:45`, and `ISR_PATH` (`index.ts:63`) and `allowQuery` (`index.ts:712`) use it.
- Scratch probe, outside the clone, at `<tmp>/s/probe.test.js`. It builds the `serverless-with-dynamic-routes` fixture and calls the built `_render` handler in-process with `GET /api/public?x_astro_path=/api/private`:
  - no extra header: 200 `{"id":"public"}`
  - header `x-vercel-isr: 1`: 200 `{"id":"private"}`

## Finding 1 — The ISR branch re-opens the path-override hole closed by #15959, behind a spoofable header (verified)

Evidence: `entrypoint.ts:24-25`. The `else if (request.headers.get('x-vercel-isr') === '1')` branch lets any request that carries that header rewrite `url.pathname` from the `x_astro_path` query param. No secret is checked on this path. #15959 existed to stop exactly that: an untrusted caller choosing which route runs. The probe above reproduces it. The security test added by #15959, `test/path-override-security.test.js` ("ignores untrusted x_astro_path query param on _render"), passes only because it omits the `x-vercel-isr` header. Adding that header makes the handler return the other route's response.

The branch is also not scoped to the ISR function. `entrypoint.ts` is the shared handler for `_render` and `_isr`, so the override is live on every serverless function. Whether Vercel's edge strips or overwrites an inbound `x-vercel-isr` is undocumented, and the adapter itself says ISR behavior is "isn't documented by vercel anywhere" (`index.ts:60-62`). Trust is therefore placed in an undocumented platform header. The code gives no reason to assume it is safe, and nothing tests the assumption.

Why this is a structural problem and not only a security one: trust decisions are now scattered. `hasValidMiddlewareSecret` is one trust root. The header sniff is a second, weaker one that sits next to it. The `locals` check further down (`entrypoint.ts:42`) still uses `hasValidMiddlewareSecret` only, so ISR requests are trusted for path but not for locals. This makes the model harder to reason about.

Code-judo proposal: the problem is that the entrypoint has to infer "who is calling me" from request details. Make the ISR function's path source a property of the build, not a runtime header check. `buildISRFolder` (`index.ts:~700`) already builds a separate function folder for ISR. Give it its own tiny handler or a build-time constant, for example via the `virtual:astro-vercel:config` module already imported on line 8. The `_isr` handler then reads `ASTRO_PATH_PARAM` unconditionally, and the shared `_render` handler never has the branch. The `x-vercel-isr` sniff and the 3-way `realPath` selection both go away. If one shared entry must stay, pass a build-time `isIsr` flag through the virtual config module. It cannot be forged per request and it needs no header-sniffing.

Remediation: do not gate on a request header. Scope the query-param read to the ISR function at build time, or authenticate it. Add a regression test that calls `_render` with `x-vercel-isr: 1` and `x_astro_path` and expects the original route. Add a positive ISR-handler test, because the PR itself added none (the body says "No additional test cases added").

## Finding 2 — Tangled `realPath` selection: `let realPath = undefined`, mixed null/undefined, magic string (verified)

Evidence: `entrypoint.ts:21-26`. What was a single-expression `const` (`hasValidMiddlewareSecret ? header : null`) is now a mutable `let` initialized to `undefined`. It is assigned from two sources (`string | null`) in an if/else-if, and then guarded by `typeof realPath === 'string'`. The `= undefined` initializer is redundant and the variable has no annotation, so the type is inferred as `string | null | undefined`. Two missing-value sentinels (`null` from `.get()`, `undefined` from the initializer) now flow into one check. `'x-vercel-isr'` and `'1'` are bare literals in a file where every sibling header is an imported named constant. The new lines also break local style (`if(` without a space, a missing semicolon on line 23), which the repo's formatter would flag.

Code-judo proposal: extract a small pure helper, for example `resolveRealPath(request, url, hasValidMiddlewareSecret): string | null`, with early returns and an `ASTRO_ISR_HEADER` constant in `index.ts` next to the other protocol constants. Better still, apply the Finding 1 build-time split and the helper disappears entirely. The result would be `const realPath = hasValidMiddlewareSecret ? request.headers.get(ASTRO_PATH_HEADER) : null`, restored, with ISR handled elsewhere.

Remediation: at minimum, restore `const`, drop `= undefined`, name the header constant, and run the formatter.

## Finding 3 — The fix is unaccompanied by a test, and the regression it repairs shipped because the ISR path had none (verified)

Evidence: `test/isr.test.js` only checks the emitted routes and prerender config (the `dest: '/_isr?x_astro_path=$0'` lines). It never invokes the `_isr` handler. #15959 broke every ISR route precisely because nothing exercised handler-level path resolution. This PR again adds none, and relies on a manual deployment. `path-override-security.test.js` already shows the in-process pattern (`loadFunctionModule` + `fetch`).

Remediation: add a test pair in `path-override-security.test.js` or `isr.test.js`. One asserts that `_isr` with the ISR marker and `x_astro_path` resolves the real route. The other asserts that `_render` ignores the same inputs, including with the ISR marker present. The second test would have caught Finding 1.

## Verification status

- Finding 1: executed, and the spoof reproduced (see Measurements).
- Finding 2: read from source and diff. Style points were not run through a formatter, which is not available offline.
- Finding 3: read from `test/isr.test.js` (75 lines) and `test/path-override-security.test.js`.
- Not verifiable here: how Vercel's platform treats an inbound `x-vercel-isr`. The review rests on the facts that the header is undocumented and that the handler cannot tell a platform-set header from a client-set one.
