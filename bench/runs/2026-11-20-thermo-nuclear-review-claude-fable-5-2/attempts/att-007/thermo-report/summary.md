# Thermo-nuclear review — withastro/astro#16079 "fix(vercel): Fix ISR path rewrite to prevent 404"

Range reviewed: `b089b904f..71ae51338` (`git diff main...review-head`), 2 files, +12 / −1.

## What the change does

`packages/integrations/vercel/src/serverless/entrypoint.ts` is the single entry file that the adapter copies into both the `_render` function and the `_isr` prerender function (`src/index.ts:453`, `:474`, `:499`). At the merge-base, #15959 had made that handler apply a path override only from the `x-astro-path` header and only when the request carried the build-time middleware secret. That closed an unauthenticated path override, but it also dropped the `x_astro_path` query parameter that ISR routes depend on (`src/index.ts:63`, `dest: '/_isr?x_astro_path=$0'`), so every ISR page rendered the path `/_isr` and returned 404.

This PR re-adds the query-parameter override, gated on the request header `x-vercel-isr: 1`.

The functional fix works as intended when called in-process: the built `_isr` handler from the `isr` fixture returns 404 for `/_isr?x_astro_path=/one` without the header and renders "One" with it. The existing focused tests pass on the head (`node --test test/path-override-security.test.js test/isr.test.js`: 4 pass, 0 fail).

The problem is the gate. Two findings follow from it, then two low-priority ones.

## Findings

### 1. [High] Path override is re-enabled on every function behind a non-secret request header, undoing the #15959 fix

**File:** `packages/integrations/vercel/src/serverless/entrypoint.ts`, lines 21–26.

```ts
let realPath = undefined;
if(hasValidMiddlewareSecret) {
	realPath = request.headers.get(ASTRO_PATH_HEADER)
} else if(request.headers.get('x-vercel-isr') === '1') {
	realPath = url.searchParams.get(ASTRO_PATH_PARAM);
}
```

**Evidence traced.**

- The new branch runs in the `_render` function as well as `_isr`, because both are built from the same entry file. `_render` exists on every server-output deployment, including ones that never enabled ISR, and no legitimate ISR request ever reaches it.
- The only thing separating a trusted override from an untrusted one is now the literal header value `x-vercel-isr: 1`. It is not a secret, it is not compared against anything generated at build time (unlike `middlewareSecret`, `src/index.ts:261`), and nothing in the repository documents or enforces that it can only originate from Vercel. `grep` finds the string in exactly one place in the repo: the new line 24.
- I called the built handlers in-process (scratch script outside the clone, fixtures built by the adapter's own tests):

  | Function (fixture) | Request | Result |
  | --- | --- | --- |
  | `_render` (`serverless-with-dynamic-routes`) | `/api/public?x_astro_path=/api/private` | `{"id":"public"}` |
  | `_render` (`serverless-with-dynamic-routes`) | same, plus `x-vercel-isr: 1` | `{"id":"private"}` |
  | `_render` (`isr`) | `/two?x_astro_path=/api/secret`, plus `x-vercel-isr: 1` | `OK secret` |
  | `_render` (`middleware-with-edge-file`, edge mode) | `/_render?x_astro_path=/`, plus `x-vercel-isr: 1` | 200, index page (404 without the header) |

  The first two rows are the exact request from the #15959 regression test `ignores untrusted x_astro_path query param on _render`, with one header added. That test still passes on this head only because it never sends the header.
- `_render` is directly addressable from the internet: the generated `config.json` puts `{"handle": "filesystem"}` first, and the adapter's own edge middleware reaches it at `/_render` over the public URL (`src/serverless/middleware.ts:129`). The catch-all route also sends every unmatched path to `_render`.

**Failure caused.** A client that can deliver `x-vercel-isr: 1` to the function can make it render any on-demand route while the platform sees a different URL. That is the vulnerability #15959 fixed: path-based controls that run in front of the function (Vercel Firewall/WAF custom rules keyed on path, and any routing or protection keyed on the public URL) are evaluated against `/api/public` while the app serves `/api/private`. Before this PR that hole was closed on all deployments; after it, the hole is one unauthenticated header wide on all deployments, not only ISR ones.

**What I could not confirm.** Vercel's platform is unavailable to this review, so I could not test whether Vercel's edge strips or overwrites a client-supplied `x-vercel-isr` header before invoking a function. If it does, this route is not reachable in production; if it does not, the #15959 fix is fully bypassable. The PR does not establish it either way, and the code depends on it entirely. Astro's own in-function middleware still runs against the overridden path (the edge-mode probe above still returned the middleware's `x-message` header), so auth implemented in Astro middleware is not bypassed; the exposure is to controls outside the function.

**Direction.** Do not let a bare header authorise the override. At minimum confine the branch to the ISR function (for example, only when the incoming pathname is `/_isr`), so `_render` keeps the #15959 behaviour unconditionally. That removes this finding regardless of how Vercel treats the header; finding 2 remains.

### 2. [Medium] On ISR deployments, `/_isr?x_astro_path=<any path>` renders any on-demand route, including routes listed in `isr.exclude`

**File:** `packages/integrations/vercel/src/serverless/entrypoint.ts`, lines 24–29.

**Evidence traced.**

- No spoofing is needed here. `/_isr` is a public filesystem route (same `config.json` evidence as above), the prerender config forwards the parameter (`allowQuery: ['x_astro_path']`, `passQuery: true`, `src/index.ts:712–713`), and the request arrives as a genuine ISR invocation, which by the PR's own premise carries `x-vercel-isr: 1`.
- The handler then assigns the parameter to `url.pathname` (line 28) with no check that the value is a route the adapter actually mapped to ISR. In-process on the `isr` fixture, whose config excludes `/two`, `/excluded/*` and `/^\/api/`:

  | Request to `_isr` with `x-vercel-isr: 1` | Result |
  | --- | --- |
  | `/_isr?x_astro_path=/one` (the intended ISR route) | 200 "One" |
  | `/_isr?x_astro_path=/two` (excluded) | 200 "Two" |
  | `/_isr?x_astro_path=/api/secret` (excluded) | 200 `OK secret` |

**Failure caused.** On a deployment with `isr` enabled, anyone can request `/_isr?x_astro_path=/some/route` and have that route rendered by the ISR function and stored in the ISR cache under that key. Routes the site owner explicitly kept out of ISR with `isr.exclude` are rendered and cached anyway, and path-based platform rules written for `/some/route` see only `/_isr`. Each distinct parameter value is its own cache key and its own function invocation. This behaviour existed before #15959 and was removed by it; this PR brings it back for ISR deployments without mentioning the security trade-off, which is why I am reporting it despite it being tied to the PR's intent.

**Direction.** After applying the override in the ISR branch, reject paths that the build did not route to ISR (excluded routes, `/_image`, `/_server-islands`, prerendered routes), or carry a build-time secret in the ISR destination as #15959 first attempted.

### 3. [Low] No test covers the restored behaviour or its security boundary

**File:** `packages/integrations/vercel/src/serverless/entrypoint.ts`, lines 21–26 (no test file changed).

**Evidence traced.** `test/isr.test.js` asserts only the generated prerender config and route table; it never invokes the `_isr` handler. That is why #15959 could break every ISR page with a green suite. `test/path-override-security.test.js` invokes `_render` but never sends `x-vercel-isr`. The PR body states "No additional test cases added".

**Failure caused.** The regression this PR fixes can recur silently, and the bypass in finding 1 is invisible to the suite. The harness needed already exists: `loadFunctionModule` in `path-override-security.test.js` loads a built function and calls `fetch` on it, and the four probes in the tables above are each a few lines on top of it.

### 4. [Low] New block is unformatted and leaves `realPath` implicitly `any`

**File:** `packages/integrations/vercel/src/serverless/entrypoint.ts`, lines 21–26.

**Evidence traced.** `if(` and `else if(` have no space before the parenthesis and line 23 has no trailing semicolon, against the repo's Biome settings (`biome.jsonc`: `"semicolons": "always"`). `let realPath = undefined;` has no annotation, so the previous `string | null` type is lost; it compiles (the adapter's `dist` contains the change) and behaves correctly because of the `typeof realPath === 'string'` guard on line 27.

**Failure caused.** No runtime effect. The formatter will rewrite these lines in a follow-up commit, and the loose type removes compiler help on the one variable that carries a security decision. `let realPath: string | null = null;` restores it.

## Questions

1. Is `x-vercel-isr: 1` present on every invocation of a prerender function — background revalidation after `expiration`, on-demand revalidation with `x-prerender-revalidate: <bypassToken>`, and `__prerender_bypass` cookie requests — and is a client-supplied copy of the header removed by Vercel before the function runs? The header is undocumented, the PR's manual test covers only a first page load, and both the fix (line 24) and finding 1 turn on the answer. If any ISR invocation lacks the header, that invocation renders `/_isr` and returns 404 as before.

## Checked and found sound

- Precedence between the two branches: a request with a valid middleware secret uses only the `x-astro-path` header and ignores the query parameter, so the edge-middleware path is unchanged. `x-vercel-isr: 1` with only an `x-astro-path` header (no secret) is still ignored (`{"id":"public"}` in-process).
- `middlewareSecret` is always a fresh `crypto.randomUUID()` (`src/index.ts:261`), so the first branch cannot be satisfied by an absent header.
- An empty `x_astro_path=` with the header yields a 404, not a crash.
- The changeset targets `@astrojs/vercel` as a patch, which matches the change.
- No environment variables, ports, scripts, build steps or feature gates are touched.
