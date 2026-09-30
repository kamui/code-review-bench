# 01 — Vercel serverless entrypoint: path-override trust model

Scope: `packages/integrations/vercel/src/serverless/entrypoint.ts` (+7 / −1) and `.changeset/common-cats-travel.md` in the range `b089b904f..71ae51338`.

## Context the diff sits in

The Vercel adapter has two ways for a function to learn the "real" request path:

1. The edge middleware (`src/serverless/middleware.ts:132-133`) calls `_render` with `x-astro-path` and the `x-astro-middleware-secret` header.
2. ISR routes in `.vercel/output/config.json` rewrite to `/_isr?x_astro_path=$0` (`src/index.ts:59-63`, the comment there notes this is undocumented Vercel behaviour), because ISR functions are not passed the original path.

Commit `335a20416` ("Require trusted secret for path overrides (#15959)") closed the unauthenticated path override: before it, `entrypoint.ts` honoured `x-astro-path ?? ?x_astro_path` from anyone. Its commit message says "fix(vercel): remove ISR secret plumbing": it dropped the query-parameter source entirely, which broke ISR (every ISR function invocation arrives at pathname `/_isr`, which matches no route, so it 404s). This PR restores the query-parameter source, gated on `request.headers.get('x-vercel-isr') === '1'`.

Two facts about the build matter for judging that gate:

- `_render` and `_isr` are the same bundle. `buildISRFolder` (`src/index.ts:702-715`) calls `buildServerlessFolder(entry, ...)` with the same `entryFile` as `_render` (`src/index.ts:453` and `:474`). In the built ISR fixture both `.vc-config.json` files point at the identical handler `packages/integrations/vercel/test/fixtures/isr/dist/server/entry.mjs`.
- Whether a function is the ISR function is therefore known at build time and nowhere else. At runtime the entrypoint has no way to tell which function it is running as, so the PR reached for a request header.

## Finding 1.1 — The ISR branch re-opens the #15959 path override on every function, gated by a request header the client can send

**Severity:** presumptive blocker (boundary / trust-model regression in a shared path).
**Verification:** CONFIRMED in-process against the built handler. Whether Vercel's edge strips or overwrites a client-supplied `x-vercel-isr` header before it reaches the function is NOT verifiable here (no platform access) and is not documented in the repo.

`entrypoint.ts:21-26` now reads:

```ts
let realPath = undefined;
if(hasValidMiddlewareSecret) {
	realPath = request.headers.get(ASTRO_PATH_HEADER)
} else if(request.headers.get('x-vercel-isr') === '1') {
	realPath = url.searchParams.get(ASTRO_PATH_PARAM);
}
```

This branch runs in every function built from this entrypoint, including `_render` in projects that never enabled ISR. The only thing standing between an untrusted request and a path override is a header whose value is under the caller's control unless the platform intervenes. Because it is an `else if` under the secret check, the untrusted request needs no secret at all.

I built the `serverless-with-dynamic-routes` fixture (no ISR configured) via the existing test and called its `_render` handler in-process with a scratch script (`clone-work/scratch/probe.mjs`):

```
no header, ?x_astro_path=/api/private         -> 200 {"id":"public"}
x-vercel-isr:1, ?x_astro_path=/api/private    -> 200 {"id":"private"}
x-vercel-isr:1, no param                      -> 200 {"id":"public"}
bad secret + x-vercel-isr:1 + x-astro-path hdr -> 200 {"id":"private"}
```

The second line is exactly the scenario `test/path-override-security.test.js` ("ignores untrusted x_astro_path query param on _render") exists to forbid. The only difference is one extra request header. The existing test still passes because it doesn't send that header (`node --test test/path-override-security.test.js test/isr.test.js`: 4/4 pass).

Structurally, the PR decides trust using a runtime property of the *request*. Trust here is really a property of the *function*, and that is a build-time fact. If Vercel does strip client `x-vercel-isr`, the code is correct, but that correctness depends on an undocumented platform behaviour that nothing in the repo records, tests, or names. The literal `'x-vercel-isr'` is the only protocol string in this file that is not a named export from `src/index.ts`. If Vercel does not strip it, #15959 has been reverted in all but name for every Vercel deployment, ISR or not. In both cases the security property of `_render` now rests on something outside the adapter's control, when the adapter already had the information needed to make it structural.

### Worked code-judo proposal

Make "am I the ISR function?" a build-time property of the ISR function instead of a runtime property of the request. The Build Output API `.vc-config.json` for Node functions accepts an `environment` map (per-function env vars). Please confirm this against Vercel's current Build Output API v3 docs before adopting it. With that map, the adapter can stamp the ISR function at build time and the entrypoint reads it from `process.env`, which a client cannot influence:

```ts
// src/index.ts — next to the other protocol constants
export const ASTRO_PATH_PARAM = 'x_astro_path';
/** Set only in the _isr function's .vc-config.json; the only function allowed to read ASTRO_PATH_PARAM. */
export const ASTRO_ISR_FUNCTION_ENV = 'ASTRO_VERCEL_ISR_FUNCTION';

// buildServerlessFolder gains an optional `environment` passthrough into .vc-config.json;
// buildISRFolder calls it with { [ASTRO_ISR_FUNCTION_ENV]: '1' }.
```

```ts
// src/serverless/entrypoint.ts
const isIsrFunction = process.env[ASTRO_ISR_FUNCTION_ENV] === '1';

function getPathOverride(request: Request, url: URL, trusted: boolean): string | null {
	if (trusted) return request.headers.get(ASTRO_PATH_HEADER);
	if (isIsrFunction) return url.searchParams.get(ASTRO_PATH_PARAM);
	return null;
}
```

What this buys:

- `_render` goes back to the exact #15959 guarantee: no path override without the secret, whatever headers arrive. The security test holds by construction, not because Vercel happens to strip a header.
- The undocumented `x-vercel-isr` header disappears from the codebase entirely. You no longer have to rely on, name, or test a platform header.
- The trust source is visible in one place. `src/index.ts` owns both the ISR route rewrite (`ISR_PATH`) and the flag that allows the entrypoint to honour it, so the producer and consumer of `x_astro_path` live together.

One residual exposure is inherent to Vercel's ISR design and was present before #15959: the `_isr` function is itself addressable at `/_isr?x_astro_path=...`, so it can always be steered to any ISR-eligible route. The proposal doesn't widen that. It confines the query-parameter trust to that one function instead of spreading it to every function.

If the env-var route turns out not to be available, a second-best alternative is a separate tiny handler file for `_isr` that sets a module-level flag before delegating to the shared app. The principle is the same: trust comes from which function this is, not from what the request says.

## Finding 1.2 — Three-state `let` + `if / else if` mutation where one pure resolver belongs

**Severity:** legibility / spaghetti growth (secondary to 1.1, and fixed by the same restructure).
**Verification:** CONFIRMED by reading.

The PR replaces a single `const` expression with `let realPath = undefined;` followed by an `if / else if` that assigns `string | null`. So `realPath` is now `undefined | string | null`, and line 27's `typeof realPath === 'string'` quietly handles two different "no override" sentinels. Beyond that, the `fetch` handler is already a sequence of trust-sensitive steps: secret check, path rewrite, locals gate, secret stripping, skew header. It now carries a second, independent trust source inline in the middle of that flow, as an `else` of the secret check. A reader has to work out that the ISR branch is deliberately *not* secret-gated, and nothing in the code says why.

The `getPathOverride` helper in the proposal above collapses this to a `const` with a single `string | null` contract:

```ts
const realPath = getPathOverride(request, url, hasValidMiddlewareSecret);
if (realPath !== null) { ... }
```

Each branch's trust reason then sits next to its return, and `fetch` goes back to reading as orchestration.

Formatting note, not a finding: the inserted lines use `if(` without a space and omit a semicolon (`entrypoint.ts:22-24`), unlike the rest of the file. The repo's CI formatter commits (`[ci] format`) will normalise this, so it isn't worth review time on its own.

## Finding 1.3 — No regression test for the exact seam that broke twice

**Severity:** maintainability / test coverage.
**Verification:** CONFIRMED. The suite passes both with this bug fix and with the trust hole demonstrated in 1.1.

This file's path-override logic has now shipped two regressions in a week: #15959 broke ISR with no failing test, and this PR adds an unauthenticated override path that no test notices. The PR body says "No additional test cases added" and relies on a manual deployment. Yet the adapter's test harness already does exactly what is needed: `test/path-override-security.test.js` loads a built function module and calls `default.fetch` in-process. I did the same for `_isr` in the `isr` fixture (`clone-work/scratch/probe-isr.mjs`):

```
_isr no header  /_isr?x_astro_path=/one -> 404   (the bug this PR fixes)
_isr isr:1      /_isr?x_astro_path=/one -> 200 <title>One</title>
```

Three cases would pin the contract. Two go next to the existing ones and one goes in `isr.test.js`:

- `_isr` with `?x_astro_path=/one` renders `/one` (would have caught #15959's regression).
- `_render` with `?x_astro_path=/api/private` plus `x-vercel-isr: 1` still returns `public`. This fails today; it is Finding 1.1.
- `_render` with a wrong secret plus `x-astro-path` stays un-overridden (already covered for the no-secret case).

With the env-var proposal, the first test sets the ISR function's env var (or loads the `_isr` function's config). The second then passes by construction.

## Non-findings checked

- File size: `entrypoint.ts` is 72 lines and `src/index.ts` is 800 lines, neither touched in a size-relevant way.
- The query parameter `x_astro_path` stays on `url` after the rewrite, so ISR pages see it in `Astro.url.searchParams`. That was also true before #15959, so it isn't a regression from this PR. A `getPathOverride` helper would be the natural place to strip it if desired.
- The changeset text "Fix vercel ISR path rewrite" is terse, but it is scoped correctly as a `patch` to `@astrojs/vercel`.

## Commands run

```
git diff main...review-head
git show 335a20416 -- packages/integrations/vercel/src
node --test test/path-override-security.test.js test/isr.test.js    # from packages/integrations/vercel: 4 pass, 0 fail
node clone-work/scratch/probe.mjs       # cwd: test/fixtures/serverless-with-dynamic-routes, imports built _render
node clone-work/scratch/probe-isr.mjs   # cwd: test/fixtures/isr, imports built _isr
git status --porcelain                  # empty after runs (fixture output is ignored)
```
