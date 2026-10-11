# Thermo-nuclear review — withastro/astro#16079 "fix(vercel): Fix ISR path rewrite to prevent 404"

Range `b089b904f..71ae51338`; 2 files (+12/−1). Detail: `01_vercel-entrypoint.md`. Single reviewer, no delegation.

## Verdict

Not approvable as written. The diff is small and fixes a real 404 for ISR routes, but it does so by re-introducing, behind a client-controllable request header, the exact query-param path override that the previous change (#15959) removed as a security fix. The fix is structurally a special-case branch dropped into a shared, security-sensitive entrypoint, and it ships with no test.

## Findings

**1. The path-override trust boundary is reopened (blocker).** In `packages/integrations/vercel/src/serverless/entrypoint.ts` (lines 21–26), the new `else if (request.headers.get('x-vercel-isr') === '1')` branch makes `url.searchParams.get('x_astro_path')` authoritative for any request carrying that header. The header is an ordinary request header and the same `fetch` handler backs `_render` and the other functions. I verified in-process against the built `_render` handler of the `serverless-with-dynamic-routes` fixture that `GET /api/public?x_astro_path=/api/private` with `x-vercel-isr: 1` is served the `/api/private` route (`{"id":"private"}`), while the same request without the header returns `/api/public`. The existing security test still passes because it never sends the header. Whether Vercel strips an inbound `x-vercel-isr` could not be checked here, and the code does not document relying on it. The remedy is a code-judo move: stop inferring "I am the ISR function" from a request header. `buildISRFolder` already knows it is building the ISR function, so give that function a build-time signal (a virtual-config flag next to `middlewareSecret`, or a thin ISR wrapper) and let the shared `fetch` keep a single secret-gated override with no request-derived mode switch. See detail file, Finding 1 and the worked proposal.

**2. Special-case branching and a magic string in a busy shared path (spaghetti growth).** `realPath` changed from one expression to a mutable `let` assigned in two branches, with a bare `'x-vercel-isr'` literal that does not live beside `ASTRO_PATH_HEADER` and `ASTRO_PATH_PARAM` in `src/index.ts`, where the ISR rewrite contract is defined. The `else if` also runs when a middleware secret is present but invalid, and `hasValidMiddlewareSecret` is now consulted for three different purposes. Extract a typed pure helper such as `resolveRealPath(request, url, hasValidMiddlewareSecret): string | null`, with the header name exported as a constant, so the decision is named and testable and `fetch` stays linear. See detail file, Finding 2.

**3. No regression test, and the security test cannot see the change.** The PR explicitly adds no tests and relies on a manual deployment. The in-process handler harness already used in `test/path-override-security.test.js` makes a positive ISR test and a negative spoofing test cheap; without them the 404 can regress and the hole in finding 1 is unguarded. See detail file, Finding 3.

**4. Loose typing and formatting on the edited lines (minor).** `let realPath = undefined;` hides the real `string | null` invariant behind a later `typeof` check, and the block has `if(` spacing and a missing semicolon that diverge from surrounding style. Using the helper from finding 2 fixes both. See detail file, Finding 4.

## Proposed remediation sequence

1. Remove the `x-vercel-isr`-gated query-param branch; replace it with a build-time ISR signal emitted by `buildISRFolder` (or secret-based plumbing), so no client-set header can select the override.
2. Move path resolution into a single typed helper and export any header/param names from `src/index.ts`.
3. Add two tests in the in-process harness: ISR-style request resolves the real route; a spoofed `x-vercel-isr: 1` + `x_astro_path` on a non-ISR function must not rewrite.
4. Tidy types/formatting as part of the helper extraction.

## Approval bar check

Structural regression: yes (finding 1). Missed dramatic simplification: yes (build-time signal instead of runtime header sniffing). Ad-hoc branching in a shared path: yes (finding 2). File-size: no concern (72 lines). Canonical-layer: partially — ISR knowledge belongs with `buildISRFolder`/`index.ts`, not sniffed in the entrypoint.
