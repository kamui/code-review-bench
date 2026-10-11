# Detail 01 — Vercel serverless entrypoint (`packages/integrations/vercel`)

Review range `b089b904f..71ae51338` (`git diff main...review-head`), 2 files, +12 / −1.
Reviewed by one primary reviewer, model `claude-opus-5-5` at `high`. No cross-model or
alternate-model review was run, and no work was delegated.

## 1. What the change does

`packages/integrations/vercel/src/serverless/entrypoint.ts` is the single fetch handler the
adapter bundles into `dist/server/entry.mjs`. `VercelBuilder.buildServerlessFolder`
(`src/index.ts:660`) copies that one bundle into `_render.func`, and `buildISRFolder`
(`src/index.ts:702`) copies the *same* bundle into `_isr.func` and adds a prerender config with
`allowQuery: ['x_astro_path']`. ISR routes are emitted as
`{ src: '^/one/?$', dest: '/_isr?x_astro_path=$0' }` (`src/index.ts:63`, `:485-494`), so the ISR
function is invoked on its own mount path and has to recover the real path from the query string.

Commit `335a20416` (#15959, "Require trusted secret for path overrides") deleted the query-param
read entirely, so the ISR function matched nothing and every ISR route returned 404. This PR puts
the read back behind a request-header check:

```ts
// entrypoint.ts:21-26 at head
let realPath = undefined;
if(hasValidMiddlewareSecret) {
	realPath = request.headers.get(ASTRO_PATH_HEADER)
} else if(request.headers.get('x-vercel-isr') === '1') {
	realPath = url.searchParams.get(ASTRO_PATH_PARAM);
}
```

## 2. Measurements and commands

File sizes at head: `src/serverless/entrypoint.ts` 72 lines, `src/index.ts` 800 lines (`wc -l`).
No file-size threshold is in play.

The permitted adapter tests pass at head:

```
cd packages/integrations/vercel
node --test test/path-override-security.test.js test/isr.test.js
# tests 4, pass 4, fail 0
```

Those four tests never invoke the `_isr` handler and never send `x-vercel-isr`, so they exercise
neither arm of the behaviour this PR changes.

After that run the built handlers remain in the fixtures' ignored `.vercel/output/` directories.
A scratch script outside the clone (`clone-work/scratch/probe.mjs`) imports them by absolute
path, exactly as `loadFunctionModule` in `test/path-override-security.test.js:5-15` does, and
calls `default.fetch(new Request(...))`:

| # | Handler (fixture) | Request | Extra header | Result |
| --- | --- | --- | --- | --- |
| A | `_render` (serverless-with-dynamic-routes) | `/api/public?x_astro_path=/api/private` | none | 200 `{"id":"public"}` |
| B | `_render` (serverless-with-dynamic-routes) | `/api/public?x_astro_path=/api/private` | `x-vercel-isr: 1` | 200 `{"id":"private"}` |
| C | `_isr` (isr) | `/_isr?x_astro_path=/one` | none | 404 |
| D | `_isr` (isr) | `/_isr?x_astro_path=/one` | `x-vercel-isr: 1` | 200 `<h1>One</h1>` |
| E | `_isr` (isr) | `/_isr?x_astro_path=/two` | `x-vercel-isr: 1` | 200 `<h1>Two</h1>` |
| F | `_isr` (isr) | `/_isr?x_astro_path=/excluded/secret` | `x-vercel-isr: 1` | 200 `<h1>Dynamic</h1>` |
| G | `_render` (isr) | `/two?x_astro_path=/one` | `x-vercel-isr: 1` | 200 `<h1>One</h1>` |
| H | `_isr` (isr) | `/_isr?x_astro_path=/one` | `x-vercel-isr: true` | 404 |

Row C is the merge-base behaviour for an ISR invocation (no branch reads the param) and row D is
the fix: the PR does what it says for a request that carries the header. Row A is the existing
security test. Row B is that same test with one extra request header, and it returns the route
the test asserts must not be reachable. Rows E and F reach `/two` and `/excluded/*`, which the
fixture's `isr.exclude` (`test/fixtures/isr/astro.config.mjs`) routes to `_render` precisely so
they are *not* served by the ISR function.

## 3. Verification status

Confirmed in-process, against the head build:

- The fix works when the invocation carries `x-vercel-isr: 1` (row D) and does nothing otherwise
  (rows C, H).
- The new arm is not confined to the ISR function. `_render` honours the query param whenever the
  header is present (rows B, G), because both functions are the same bundle.
- Inside the ISR function the override target is unconstrained (rows E, F).
- The emitted `config.json` puts `{ "handle": "filesystem" }` first
  (`test/fixtures/isr/.vercel/output/config.json`), ahead of every route definition, so
  `/_isr` and `/_render` are function paths in their own right rather than only rewrite targets.

Not verifiable here (Vercel's platform is unavailable):

- Whether Vercel strips or overwrites a client-supplied `x-vercel-isr` before it reaches a
  function that is *not* a prerender function. If it does not, row B is reachable on a deployed
  site and the #15959 hardening is undone for `_render`. Nothing in the repository documents the
  header; `src/index.ts:60-62` already notes that the ISR path behaviour "isn't documented by
  vercel anywhere", and `x-vercel-isr` appears nowhere else in `packages/` (`grep -rn "x-vercel-"`).
- Whether a client can request `/_isr?x_astro_path=/two` directly on a deployment and receive the
  rendered, cacheable response of rows E/F. The emitted routing table says the path resolves to
  the function; what the platform does with it was not observed.
- What URL the platform hands to the ISR function (assumed `/_isr?...` from the `dest` string and
  from the 404 the PR describes).
- `tsc` was not run. `let realPath = undefined;` is an evolving-`any` declaration that control
  flow resolves to `string | null | undefined`; this is read from the language rules, not from a
  compiler run.

## 4. Finding evidence

### 4.1 Trust placed in a request header, in a bundle shared by two functions

The comment trail matters here. #15959's whole point, per its changeset
(`.changeset/short-cycles-fail.md` in that commit), was that "override values are only applied
when the trusted middleware secret is present", and one of its squashed commits is titled
"remove ISR secret plumbing". The intent was an unforgeable token. This PR's second arm replaces
the token with `request.headers.get('x-vercel-isr') === '1'`, a value with no secret in it. Its
safety rests on a platform guarantee the code neither states nor can test.

The deeper problem is that the question being asked, "am I the ISR function?", is not a
per-request fact. It is fixed when `buildISRFolder` writes `_isr.func`. Asking each request to
answer it is what makes `_render` answer "yes" in rows B and G.

### 4.2 No test for either direction

The regression this PR fixes existed because no test called the `_isr` handler; `test/isr.test.js`
only inspects JSON files. #15959 then removed the param read and all tests stayed green. This PR
restores the read and all tests stay green again, in both directions: deleting the new `else if`
would not fail a test, and neither does the bypass in row B. The harness needed is already in the
tree (`loadFunctionModule`), and the probe table above is, in effect, the missing test file.

### 4.3 Structure of the edit

The merge-base line was one `const` expression. Head is a mutable `let` with an evolving type, a
two-arm `if`/`else if`, an inline string literal for the header where every other header in the
file is a named import from `../index.js`, and no comment explaining why an undocumented platform
header decides the request path. The two arms also read from different places (a header, then a
query param) under different trust conditions, and that asymmetry is exactly what a reader needs
spelled out. The formatting (`if(`, missing semicolon on line 23) does not match the file and will
be rewritten by the repository's format job.

Related duplication on the build side: the function name `'_isr'` is written twice as a bare
literal, once inside `ISR_PATH` (`src/index.ts:63`) and once at the `buildISRFolder` call
(`src/index.ts:474`), while `_render` and `_middleware` have `NODE_PATH` and `MIDDLEWARE_PATH`.

One pre-existing wart the PR restores rather than introduces: after the rewrite the request URL
still carries `?x_astro_path=/one`, so `Astro.url.searchParams` exposes an adapter-internal
parameter to user code. That was true before #15959 as well.

## 5. Worked code-judo proposal

The move is to change who owns the fact. The build knows which function is the ISR function; the
request does not. Once the build says so, the header sniff disappears, `_render` cannot take the
ISR arm at all, and the entrypoint's decision collapses back to a single expression.

Build side (`src/index.ts`), name the function once and stamp it:

```ts
export const ISR_FUNCTION_NAME = '_isr';
/** Set only in the ISR function's environment, so the shared entrypoint knows which copy it is. */
export const ASTRO_ISR_FUNCTION_ENV = 'ASTRO_VERCEL_ISR_FUNCTION';

const ISR_PATH = `/${ISR_FUNCTION_NAME}?${ASTRO_PATH_PARAM}=$0`;
// ...
await builder.buildISRFolder(entryFile, ISR_FUNCTION_NAME, isrConfig, _config.root);
```

```ts
async buildServerlessFolder(
	entry: URL,
	functionName: string,
	root: URL,
	environment?: Record<string, string>,
) {
	// ...
	await writeJson(vcConfig, {
		runtime,
		handler: handler.replaceAll('\\', '/'),
		launcherType: 'Nodejs',
		maxDuration,
		supportsResponseStreaming: true,
		environment,
	});
}

async buildISRFolder(entry: URL, functionName: string, isr: VercelISRConfig, root: URL) {
	await this.buildServerlessFolder(entry, functionName, root, { [ASTRO_ISR_FUNCTION_ENV]: '1' });
	// prerender config unchanged
}
```

`writeJson` goes through `JSON.stringify`, which drops `undefined` values; the built
`_isr.func/.vc-config.json` in the fixture already omits `maxDuration` for that reason. So
`_render.func/.vc-config.json` stays byte-identical and no existing snapshot moves.

Runtime side (`src/serverless/entrypoint.ts`), one pure function, one `const`:

```ts
// Both functions run this bundle; only the ISR copy is built with this variable set.
const isIsrFunction = process.env[ASTRO_ISR_FUNCTION_ENV] === '1';

/**
 * The edge middleware forwards the original path in a header, authenticated by the secret.
 * Vercel invokes the ISR function at /_isr?x_astro_path=<original path> (see ISR_PATH).
 * Nothing else may override the path.
 */
function getPathOverride(request: Request, url: URL, fromMiddleware: boolean): string | null {
	if (fromMiddleware) return request.headers.get(ASTRO_PATH_HEADER);
	if (isIsrFunction) return url.searchParams.get(ASTRO_PATH_PARAM);
	return null;
}

// in fetch():
const realPath = getPathOverride(request, url, hasValidMiddlewareSecret);
if (realPath !== null) {
```

That deletes the `let`, the magic literal, and the dependency on an undocumented header, and it
gives the trust rule a name and a home that a test can import.

Caveat, stated plainly: the `environment` key of `.vc-config.json` is recalled from Vercel's Build
Output API v3 function configuration and was not checked against the platform or its docs here.
If maintainers would rather not depend on it, the same shape works with a build-time fact that
needs no platform feature: compare the handler's own mount path,
`url.pathname === '/' + ISR_FUNCTION_NAME`, which only the `_isr` function can observe because the
filesystem route sends that path to `_isr.func` and nowhere else. That variant needs one
confirmation of the URL the platform passes to a prerender function.

Either variant makes rows B and G return the un-rewritten route. Neither constrains rows E and F,
which need a second, separate decision: whether the ISR function should refuse an override whose
matched route the build excluded from ISR. The channel for that already exists
(`virtual:astro-vercel:config`, fed by `createConfigPlugin` at `src/index.ts:307`, where `isr` is
in scope), but it is a larger change than this PR and is raised as a question rather than a
required edit.

If the helper is adopted, deleting `x_astro_path` from `url.searchParams` after reading it is a
one-line addition that stops the internal parameter reaching `Astro.url`. That changes what user
code observes, so it is a maintainer's call and not part of the behaviour-preserving proposal.

## 6. Tests to add

In `test/isr.test.js` (hoist `loadFunctionModule` into `test/test-utils.js` so both files share it):

- `_isr` with `/_isr?x_astro_path=/one` renders `/one` (row D, under whichever marker is chosen).
- `_render` from the same fixture ignores `x_astro_path` (row G must render `/two`).

In `test/path-override-security.test.js`:

- `_render` ignores `x_astro_path` when the request also carries `x-vercel-isr: 1` (row B must
  return `public`). At head this test fails, which is the point.
