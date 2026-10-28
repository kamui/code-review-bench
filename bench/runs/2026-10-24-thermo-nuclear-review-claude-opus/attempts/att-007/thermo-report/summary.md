# Thermo Nuclear Review — withastro/astro#16079 "fix(vercel): Fix ISR path rewrite to prevent 404"

Range: `b089b904f1ed578e9edaefd129bf9843120a808f..71ae513388df11d7dad6b1e0077c402ad03d0d62` (`git diff main...review-head`). There are two changed files: a new changeset and `packages/integrations/vercel/src/serverless/entrypoint.ts` (+7/−1).

## Findings

### [High] A spoofable `x-vercel-isr` header reopens the path-override bypass that #15959 closed. In `middlewareMode: 'edge'` this skips Astro middleware

**File / lines:** `packages/integrations/vercel/src/serverless/entrypoint.ts` lines 21–34. The new `else if (request.headers.get('x-vercel-isr') === '1') { realPath = url.searchParams.get(ASTRO_PATH_PARAM); }` branch is at lines 24–26.

**What changed and why it matters.** Commit `335a20416` ("Require trusted secret for path overrides (#15959)") was a security fix. Before it, the function rewrote `url.pathname` from the `x-astro-path` header or the `x_astro_path` query parameter on any request. After it, the rewrite happens only when the request carries the per-build random `middlewareSecret` (`src/index.ts:261`). That commit added `test/path-override-security.test.js`, which asserts that `/api/public?x_astro_path=/api/private` still renders `public`. This PR adds a second way to get the rewrite. It only requires the request header `x-vercel-isr: 1`, and a client can send that header. Nothing in the adapter sets, signs, or checks it. The only thing that could make it trustworthy is undocumented Vercel edge behaviour. The adapter does not rely on that behaviour anywhere else, and this PR does not document it.

**This is not limited to the ISR function.** The same `entrypoint.ts` bundle is the handler for every serverless function the adapter emits. `buildServerlessFolder(entryFile, NODE_PATH, …)` builds `_render`, and `buildISRFolder(entryFile, '_isr', …)` builds `_isr` from the same `entryFile` (`src/index.ts:450–474`, `499`). The check therefore also runs in `_render`, and it runs in deployments that never turned ISR on (`isr: false`, `src/index.ts:498–507`). `_render` is publicly reachable at `/_render`, because Vercel routes function folder names as paths (`src/index.ts:55–57`). The generated edge middleware itself calls `fetch(new URL('/_render', request.url))` (`src/serverless/middleware.ts:127–135`).

**Impact traced end to end:**
- With `middlewareMode: 'edge'` (or the deprecated `edgeMiddleware: true`), Astro middleware runs only in the `_middleware` edge function. It is not bundled into the serverless function. See `packages/astro/src/types/public/integrations.ts:97` ("Middleware code will not be bundled and imported by all pages") and `core/build/plugins/plugin-manifest.ts:88–89`. This is exactly why #15959 required the secret. An attacker who sends `GET /_render?x_astro_path=/admin` with `x-vercel-isr: 1` reaches `_render` directly, skips the edge middleware, and gets `/admin` rendered with no middleware at all. Any authentication or authorization done in `src/middleware.ts` is bypassed.
- In every mode, a request to an allowed path can render a different route, e.g. `/api/public?x_astro_path=/api/private` with the header. This defeats Vercel firewall/WAF rules, `vercel.json` rules, and path-based logging that key on the request path. It is the same class of bypass the #15959 test was written to block.

**Verification.** I wrote a scratch test (`clone-work/scratch/isr-spoof.test.js`). It builds the existing `test/fixtures/serverless-with-dynamic-routes` fixture, which uses the default config with ISR disabled, and calls the built `_render` handler in-process:
- `GET https://example.com/api/public?x_astro_path=/api/private` with no extra header returns `200 {"id":"public"}`. This is the baseline protection from #15959.
- The same request with `x-vercel-isr: 1` returns `200 {"id":"private"}`.
- `GET https://example.com/_render?x_astro_path=/api/private` with `x-vercel-isr: 1` returns `200 {"id":"private"}`.

The existing `test/path-override-security.test.js` and `test/isr.test.js` both still pass (4/4). The security test never sends the new header, so it does not catch this regression. The PR adds no test of its own.

**Remedy direction (not applied).** Gate the query-parameter override on something the client cannot forge and that applies only to the ISR function. One option is a build-time flag or separate entry for `_isr`. Another is to embed the per-build secret in the ISR route's `dest` (for example `/_isr?x_astro_path=$0&<secret param>`) and compare it in the handler. Keep `_render` strictly secret-gated, and extend `path-override-security.test.js` to send `x-vercel-isr: 1`.

## Other notes (not findings)
- Whether Vercel actually sends `x-vercel-isr: 1` on ISR invocations, and whether its edge strips a client-supplied copy, cannot be checked here because the platform is unavailable. The author's manual deployment suggests the header is present on ISR calls. Even if Vercel stripped or overwrote client copies on `_isr`, the adapter would still depend on undocumented platform behaviour for a security boundary. I found nothing indicating that `_render`, a plain serverless function, gets this protection.
- Lines 22–24 are missing a space after `if`/`else if` and a semicolon, and line 21 types `realPath` implicitly as `any` through `let realPath = undefined`. These are style issues that CI formatting will handle; they are not actionable defects.
