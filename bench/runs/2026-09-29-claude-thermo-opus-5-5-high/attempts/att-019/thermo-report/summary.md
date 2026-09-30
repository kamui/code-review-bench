# Thermo-nuclear code quality review — withastro/astro#16079

**Range:** `b089b904f..71ae51338` (1 commit, 2 files, +12 / −1)
**Change:** restores ISR in `@astrojs/vercel` by letting `src/serverless/entrypoint.ts` read `?x_astro_path` again when the request carries `x-vercel-isr: 1`.
**Detail file:** [`01_vercel-serverless-entrypoint.md`](01_vercel-serverless-entrypoint.md). It has the evidence, in-process probe output, and the worked restructure.

## Verdict

**Do not approve as-is.** The fix does make the built `_isr` function render ISR routes again: in-process, `/_isr?x_astro_path=/one` returns 404 without the header and 200 with it. But it does so by bolting a second, secret-less trust source onto the shared entrypoint, keyed on a request header. This is the same file where #15959 just removed unauthenticated path overrides. Whether a function is the ISR function is a build-time fact the adapter already owns. Deciding it from the request is the wrong boundary, and it produces the one measurable regression below. The diff is small, but the structural problem is squarely in scope of the approval bar: "the PR adds ad-hoc branching that makes an existing flow more tangled" and "logic in the wrong layer when there is a clear canonical home".

## Findings

### 1. The ISR branch re-opens the #15959 path override on every function, including `_render` in non-ISR projects (presumptive blocker)

`entrypoint.ts:21-26` adds `else if (request.headers.get('x-vercel-isr') === '1') realPath = url.searchParams.get(ASTRO_PATH_PARAM)`. The `_render` and `_isr` functions are built from the same bundle (`buildISRFolder` reuses `buildServerlessFolder` with the same `entryFile`, `src/index.ts:453,474`), so this branch is live in `_render` too, even in projects that never enable ISR. I built the non-ISR `serverless-with-dynamic-routes` fixture and called its `_render` handler in-process. `GET /api/public?x_astro_path=/api/private` with `x-vercel-isr: 1` returns `{"id":"private"}`. Without the header it returns `public`. That is exactly the override `test/path-override-security.test.js` exists to forbid, reachable with one extra header and no secret. The existing suite still passes 4/4 because it never sends that header. I could not verify whether Vercel's edge strips a client-supplied `x-vercel-isr` (no platform access). If it does, the code is correct only by virtue of an undocumented platform behaviour that nothing in the repo names or tests. If it doesn't, #15959 is effectively reverted. The code-judo move: make ISR-ness a property of the *function*, not the *request*. Stamp the `_isr` function at build time, via the Build Output API's per-function `environment` in `.vc-config.json` (confirm availability against Vercel's docs) or a thin dedicated `_isr` handler. The entrypoint then honours `ASTRO_PATH_PARAM` only when `process.env[ASTRO_ISR_FUNCTION_ENV] === '1'`. `_render` goes back to the #15959 guarantee by construction, and the undocumented `x-vercel-isr` literal leaves the codebase entirely. Detail: 01, Finding 1.1.

### 2. Three-state `let` plus `if / else if` mutation in an already trust-sensitive handler (spaghetti growth)

The PR turns a single `const realPath = …` into `let realPath = undefined` followed by branch assignments. The variable now ranges over `undefined | string | null`, and `typeof realPath === 'string'` silently absorbs two different "no override" sentinels. The second trust source is also inlined as an `else` of the secret check, in the middle of a `fetch` handler that already sequences secret validation, the locals gate, secret stripping and the skew header. Nothing explains why this branch is deliberately not secret-gated, and the header string is the only protocol literal in the file that isn't a named constant from `src/index.ts`. Extract a pure `getPathOverride(request, url, trusted): string | null` whose two branches each carry their trust reason, and call it as a `const`. Combined with Finding 1, that shrinks the handler back to orchestration and gives the path-override policy a single, testable home. Detail: 01, Finding 1.2.

### 3. No regression test for a seam that has now broken twice (test coverage)

#15959 broke ISR without a failing test, and this PR introduces the override in Finding 1 without a failing test either. The PR body relies on a manual deployment. The adapter's harness already loads built function modules and calls `default.fetch` in-process (`test/path-override-security.test.js`), so pinning the contract is cheap. Add three cases: `_isr` with `?x_astro_path=/one` renders `/one`; `_render` with `?x_astro_path=/api/private` plus `x-vercel-isr: 1` still returns `public` (fails today); and `_render` with a wrong secret plus `x-astro-path` is not overridden. With the build-time flag from Finding 1, the second test passes by construction and the first documents why ISR needs the query parameter at all. Detail: 01, Finding 1.3.

## Not findings

File size is not a concern (`entrypoint.ts` 72 lines, `src/index.ts` 800 lines, untouched). The `if(` / missing-semicolon formatting will be normalised by the repo's CI formatter. `x_astro_path` lingering in `Astro.url.searchParams` on ISR pages predates #15959 and isn't a regression here, though `getPathOverride` would be the natural place to strip it.

## Proposed remediation sequence

1. Add the three in-process tests from Finding 3 first, so the `_render` + `x-vercel-isr` case goes red and the `_isr` render case goes green.
2. Add `ASTRO_ISR_FUNCTION_ENV` next to `ASTRO_PATH_PARAM` in `src/index.ts`. Give `buildServerlessFolder` an `environment` passthrough and have `buildISRFolder` set the flag, or ship a thin `_isr` handler if per-function env is not supported.
3. Replace the `let` / `else if` in `entrypoint.ts` with a `const realPath = getPathOverride(...)` that trusts the header only with the secret and the query parameter only in the ISR function. Delete the `'x-vercel-isr'` literal.
4. Re-run `node --test test/path-override-security.test.js test/isr.test.js`. All cases should pass, with `_render` rejecting every unauthenticated override regardless of headers.

## Verification status

- Finding 1: CONFIRMED in-process against built handlers. Platform header stripping is unverified.
- Finding 2: CONFIRMED by reading.
- Finding 3: CONFIRMED. The existing tests pass (4/4) while the Finding 1 behaviour is present.
- The clone tree was unchanged after the runs (`git status --porcelain` empty). Scratch probes are in `clone-work/scratch/`.
