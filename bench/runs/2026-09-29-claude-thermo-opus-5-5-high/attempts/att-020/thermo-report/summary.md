# Thermo-nuclear review — withastro/astro#16079 "fix(vercel): Fix ISR path rewrite to prevent 404"

Range: `b089b904f..71ae51338` (1 commit, 2 files, +12/−1). Changed code: `packages/integrations/vercel/src/serverless/entrypoint.ts` plus a changeset.

## Verdict

**Request changes.** The PR does fix the ISR 404 that #15959 introduced, and the diff is small. But it fixes it by deciding a trust question from a client-controllable request header, and because the `_render` and `_isr` functions are built from the same entry, the new branch reopens the path-override bypass that #15959 was written to close. I confirmed this in-process against the built handlers. The deeper problem is structural: "am I the ISR function?" is a build-time fact about the deployment artifact, and the entrypoint is guessing it at runtime from request headers. Moving that fact to the build boundary deletes the new branch and restores the invariant that `_render` never honours `?x_astro_path=`.

## Findings

**1. The ISR branch trusts a client-controllable header and reopens the #15959 bypass on `_render` (blocker, CONFIRMED).** In `entrypoint.ts` lines 21–26 the new `else if (request.headers.get('x-vercel-isr') === '1')` branch accepts `?x_astro_path=` from any request that carries that header, with no tie to the ISR function and no secret check. Because `buildISRFolder` reuses the same entry as `_render`, the ordinary SSR function now honours the override too. Calling the built `_render` handler from the `serverless-with-dynamic-routes` fixture with `/api/public?x_astro_path=/api/private` returns `{"id":"public"}` without the header and `{"id":"private"}` with `x-vercel-isr: 1`, even when a wrong middleware secret is also sent. That is exactly the request `test/path-override-security.test.js` exists to reject. Whether Vercel strips an inbound `x-vercel-isr` before it reaches a non-ISR function is undocumented and unverifiable here, and the adapter should not rest a security check on it. The remedy is Finding 2. Full probe output and analysis: `01_serverless-entrypoint.md` §1.1.

**2. Code-judo: make ISR-ness a build-time property of the `_isr` artifact so the runtime branch disappears (missed simplification).** The handler answers a static question (which function am I?) with a dynamic probe (which headers did this request carry?). That inversion caused both #15959's 404 and this PR's trust leak, and it is why the fix had to be an `else if` bolted onto the security check. The adapter already owns the artifact boundary in `buildISRFolder`. It should mark the `_isr` function at build time: a per-function flag in its config, or a thin `_isr` entry that calls the shared handler with `{ isr: true }`. Failing that, it could gate on the adapter-defined `/_isr` pathname through an exported constant. Resolution then collapses to a pure `resolveOverridePath(request, url, hasValidMiddlewareSecret): string | null` with two named trust sources: the middleware secret, and "this is the ISR function". The `'x-vercel-isr'` magic string, the mutable `let`, and the platform-header dependency all go away, and `_render` can never honour the query param by construction. Worked code and trade-offs are in `01_serverless-entrypoint.md` §1.2.

**3. The inserted branch is below the file's own boundary and legibility conventions (minor; subsumed by 2).** Every other protocol header in this entrypoint is a named, documented constant exported from `src/index.ts`, but `'x-vercel-isr'` is an inline literal with no comment on where it comes from or why it can be trusted, and that is the one property a reviewer most needs to judge. `let realPath = undefined` widens the variable to `string | null | undefined` and turns the post-#15959 single `const` expression into a mutable value assigned from two branches in the middle of `fetch`. The `if(` spacing and missing semicolon also bypass the repo formatter. If the runtime approach is kept anyway, extract the helper from Finding 2 with a `string | null` return and name the header in `index.ts` with its provenance. Details: `01_serverless-entrypoint.md` §1.3.

**4. The ISR function's runtime contract has no test, which is how both the 404 and this PR's trust leak got through (test gap, CONFIRMED).** `test/isr.test.js` only checks `_isr.prerender-config.json` and the route table, and never invokes the built `_isr` handler. `path-override-security.test.js` does not cover the new branch. The suite passes 4/4 at the head while `_render` is bypassable. The harness already has `loadFunctionModule` for calling built functions in-process. Lift it into `test-utils.js` and add two cases: `_isr` renders `/one` from `?x_astro_path=/one`, and `_render` ignores `?x_astro_path=` even when `x-vercel-isr: 1` is sent. Worked tests and their expected status at the head vs. under Finding 2: `02_test-coverage.md` §2.1.

## Open question

Does Vercel's router strip or overwrite a client-supplied `x-vercel-isr` header on requests routed to non-ISR functions (`_render`, `_middleware`)? The PR relies on that header without documenting its provenance. If Vercel does not sanitise it, Finding 1 is directly exploitable in production to get around path-based firewall rules and edge-middleware matchers. If it does, Finding 1 still stands as a defence-in-depth regression. Either way the remedy in Finding 2 is the same.

## Secondary observation (pre-existing, not counted as a finding)

The `_isr` function will render any route when addressed directly as `/_isr?x_astro_path=…`, including routes listed in `isr.exclude` (the probe rendered `/excluded/x` this way). This was already true before #15959. A function-identity check from Finding 2 is a natural place to also refuse targets that are not ISR routes. See `01_serverless-entrypoint.md` §1.1.

## Remediation sequence

1. Replace the header check with a build-time ISR marker for the `_isr` function (Finding 2), and route the override decision through a single `resolveOverridePath` helper returning `string | null` (Findings 2 and 3).
2. Add the two in-process tests from `02_test-coverage.md` (Finding 4). Confirm the `_render` + `x-vercel-isr` case fails on the current head and passes after step 1.
3. Optionally, have the ISR function refuse `x_astro_path` targets that are excluded from ISR.

## File size

`entrypoint.ts` ends at 72 lines and `index.ts` (800 lines) is untouched. No decomposition concern.

## Detail files

- `01_serverless-entrypoint.md`: trust-boundary analysis, probe output, worked code-judo proposal, legibility notes.
- `02_test-coverage.md`: test-gap analysis and worked test cases.

Commands run: `node --test test/path-override-security.test.js test/isr.test.js` (4/4 pass) from `packages/integrations/vercel`, then `node clone-work/scratch/probe.mjs`, which calls the built `_render` and `_isr` handlers in-process.
