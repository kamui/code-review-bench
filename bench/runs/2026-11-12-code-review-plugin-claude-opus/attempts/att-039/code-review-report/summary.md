# Code review — withastro/astro#16079 "fix(vercel): Fix ISR path rewrite to prevent 404"

Range reviewed: `main...review-head` (b089b904f..71ae51338). No CLAUDE.md files apply to the changed paths. Step 6 kept 1 issue.

## Issues found: 1

### 1. Client-supplied `x-vercel-isr` header reopens the path-override bypass that #15959 closed

- **Reason flagged:** bug (security regression)
- **File / lines:** `packages/integrations/vercel/src/serverless/entrypoint.ts` lines 21-26 (the new `else if(request.headers.get('x-vercel-isr') === '1') { realPath = url.searchParams.get(ASTRO_PATH_PARAM); }` branch)
- **Why it fails:** Commit 335a20416 (#15959, "Require trusted secret for path overrides") made the entrypoint override the routed pathname only when the request carries the trusted middleware secret. The new branch adds a second override path with no secret at all. It needs only an `x-vercel-isr: 1` request header plus the `x_astro_path` query parameter. Nothing in the adapter sets, strips or checks that header, so it is client-controlled as far as this code knows. The same entrypoint is built into both the `_render` and `_isr` functions (`index.ts`: `buildServerlessFolder(entryFile, ...)` and `buildISRFolder(entryFile, '_isr', ...)`), so the override also works on `_render`. That function serves every route when ISR is off, ISR-excluded routes, `/_image` and server islands.
  - **Reproduced in-process:** calling the built `_render` handler with `GET https://example.com/api/public?x_astro_path=/api/private` returns `{"id":"public"}`. Adding the header `x-vercel-isr: 1` to the same request returns `{"id":"private"}`.
  - **Impact:** a request to an allowed path renders a different route. That gets around Vercel path-based firewall, routing and edge-middleware rules, which is exactly what #15959 fixed. The existing test `test/path-override-security.test.js` still passes only because it never sends the header.
  - **Not verifiable offline:** whether Vercel's platform overwrites a client-sent `x-vercel-isr` header. The adapter does not rely on or document any such guarantee, and it would not protect `_render` in any case.
  - **Safer approach:** enable the query-param override only in the `_isr` function's build (for example, a build-time flag), not based on a request header.
