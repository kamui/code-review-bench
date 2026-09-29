# Thermo-nuclear code quality review — withastro/astro#16079 (fix(vercel): Fix ISR path rewrite to prevent 404)

Range: `b089b904f..71ae51338`, 2 files, +12/−1. One subsystem, so one detail file: `01_vercel-entrypoint.md`. Single primary reviewer (claude-sonnet-5-5, high); no delegation and no other model was used.

## Verdict

Do not approve as written. The PR fixes a real regression, which is that ISR routes 404 because #15959 removed the unconditional query-param rewrite. But it does so with a small, ad-hoc branch inside the shared `fetch` handler, and that branch reopens the exact hole #15959 closed. The file-size rule is not in play (the entrypoint stays around 68 lines). The problems are the trust boundary, the branch structure, and missing tests.

## Findings

**1. The ISR path override is gated on a header any client can send, which re-opens the bypass #15959 closed.** In `packages/integrations/vercel/src/serverless/entrypoint.ts`, lines 21–26, the new `else if (request.headers.get('x-vercel-isr') === '1')` branch rewrites `url.pathname` from the `x_astro_path` query param. `_render` and `_isr` are built from the same entry file, so the branch is live in `_render` too. I built the security-test fixture and called the built `_render` handler in-process: `GET /api/public?x_astro_path=/api/private` returns `{"id":"public"}` without the header and `{"id":"private"}` with `x-vercel-isr: 1`. A caller can therefore choose the matched route on a function that is supposed to ignore that param, including routes excluded from ISR and pathname-based middleware decisions. Whether Vercel's edge strips this inbound header is platform behaviour I could not test, and the header is undocumented here, so production exploitability is unverified. Code judo: the handler should not sniff its own role from request bytes. Give the ISR function its own entry or a build-time constant so the query rewrite exists only in `_isr`, and `_render` keeps only the secret-gated header path. That deletes the branch, and the trust question with it. Full evidence and the probe output are in `01_vercel-entrypoint.md`.

**2. The path resolution became a mutable two-mode ladder inlined in an already busy `fetch`.** The previous one-expression `realPath` is now a `let realPath = undefined` assigned across `if / else if`, with two sources under two different trust signals, followed by a `typeof … === 'string'` guard that exists only because of the loose typing. This is the "weird conditional in a shared path" shape the skill objects to. Extract a pure `resolveRealPath(request, url, hasValidMiddlewareSecret): string | null` that states the trust policy in one place and is testable without a fixture build. If the per-function entry from finding 1 is adopted, this collapses further and mostly disappears.

**3. The consumed `x_astro_path` param is left in the rewritten URL, and there is no test of the ISR path.** Only `pathname` is replaced, so user code sees an internal routing param in `Astro.url` on ISR pages. Delete it when consuming it. The PR states no tests were added, and `test/isr.test.js` only checks generated config and routes, never invoking `_isr`. The 404-on-every-ISR-route regression this PR fixes would have been caught by an in-process call to the built `_isr` handler, the same technique `path-override-security.test.js` already uses for `_render`. Add that test, plus a forged-header regression test next to the two existing security tests.

**4. Low priority: the new lines don't match the surrounding code.** There is `if(` without a space and a missing semicolon, and an inline `'x-vercel-isr'` literal where every sibling contract string is a named constant exported from `src/index.ts`. Add an `ASTRO_ISR_HEADER` constant beside `ASTRO_PATH_HEADER` and run the formatter.

## Suggested remediation sequence

1. Move the query-param rewrite out of the shared handler: either a dedicated ISR entry or a build-time flag, so `_render` cannot honour `x_astro_path` at all (finding 1).
2. If a header check must stay, bind it to something unforgeable, and extract `resolveRealPath` with an explicit `string | null` return (finding 2).
3. Strip `x_astro_path` from the rewritten URL (finding 3).
4. Add two in-process tests: `_isr` serves the intended route, and `_render` ignores `x_astro_path` even with `x-vercel-isr: 1` (finding 3).
5. Introduce the header-name constant and format the file (finding 4).

## Notes on what was and was not verified

Verified by execution: the in-process bypass on `_render` with a forged header, and that the existing 2 security tests still pass (they do not exercise the header). Not verifiable here: Vercel edge behaviour toward inbound `x-vercel-isr`, and real ISR cache behaviour. Detail and commands are in `01_vercel-entrypoint.md`.
