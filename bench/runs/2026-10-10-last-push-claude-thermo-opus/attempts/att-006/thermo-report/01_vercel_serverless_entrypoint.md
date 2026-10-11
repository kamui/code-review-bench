# Detail 01 — Vercel serverless entrypoint (`packages/integrations/vercel`)

Evidence, commands, verification status and worked proposals for the findings in [`summary.md`](summary.md). Reviewed range `b089b904f..71ae51338`; all line numbers are on the head (`review-head`).

## 1. What changed

```
A  .changeset/common-cats-travel.md                            (+5 −0)
M  packages/integrations/vercel/src/serverless/entrypoint.ts   (+7 −1)
```

`entrypoint.ts` at the head, lines 17–34:

```ts
async fetch(request: Request): Promise<Response> {
	const url = new URL(request.url);
	const middlewareSecretHeader = request.headers.get(ASTRO_MIDDLEWARE_SECRET_HEADER);
	const hasValidMiddlewareSecret = middlewareSecretHeader === middlewareSecret;
	let realPath = undefined;
	if(hasValidMiddlewareSecret) {
		realPath = request.headers.get(ASTRO_PATH_HEADER)
	} else if(request.headers.get('x-vercel-isr') === '1') {
		realPath = url.searchParams.get(ASTRO_PATH_PARAM);
	}
	if (typeof realPath === 'string') {
		url.pathname = realPath;
		request = new Request(url.toString(), { … });
	}
```

At the merge-base the whole block was `const realPath = hasValidMiddlewareSecret ? request.headers.get(ASTRO_PATH_HEADER) : null;`.

## 2. History that frames the change

`git log -- src/serverless/entrypoint.ts` shows the previous commit on this file is `335a20416` "Require trusted secret for path overrides (#15959)". Its three squashed messages are "require middleware secret for path overrides", "remove ISR secret plumbing" and a changeset reading "Fix Vercel serverless path override handling so override values are only applied when the trusted middleware secret is present." It replaced

```ts
const realPath = request.headers.get(ASTRO_PATH_HEADER) ?? url.searchParams.get(ASTRO_PATH_PARAM);
```

with the secret-gated header and added `test/path-override-security.test.js`, whose two tests call the built `_render` handler and assert that neither `?x_astro_path=/api/private` nor an `x-astro-path` header moves a request for `/api/public`.

That commit removed the only way the ISR function learns its real path. `src/index.ts:60-63` explains why the parameter exists:

```ts
// This isn't documented by vercel anywhere, but unlike serverless
// and edge functions, isr functions are not passed the original path.
// Instead, we have to use $0 to refer to the regex match from "src".
const ISR_PATH = `/_isr?${ASTRO_PATH_PARAM}=$0`;
```

So this PR is restoring a required input. The review question is how it is trusted.

## 3. Structural facts relied on

One entry, two functions. `src/index.ts:451-474`:

```ts
if (isr) {
	const isrConfig = typeof isr === 'object' ? isr : {};
	await builder.buildServerlessFolder(entryFile, NODE_PATH, _config.root);
	…
	await builder.buildISRFolder(entryFile, '_isr', isrConfig, _config.root);
```

and the non-ISR branch calls `buildServerlessFolder(entryFile, NODE_PATH, …)` with the same `entryFile`. `buildISRFolder` (`src/index.ts:702-715`) is `buildServerlessFolder` plus a `prerender-config.json`. The handler therefore cannot tell from its own code which function it is running as, and any branch added to it is present in `_render` for every deployment.

Routing. In the `isr` fixture the emitted route table (asserted by `test/isr.test.js`) sends excluded routes, `_image` and `_server-islands` to `_render`, and only `^/one/?$` and `^/404/?$` to `/_isr?x_astro_path=$0`. With an edge middleware entry point the excluded routes go to `_middleware` instead (`src/index.ts:465`).

The only build-time data channel into the handler is `virtual:astro-vercel:config`, produced by `src/vite-plugin-config.ts` from a `Config` of `{ middlewareSecret, skewProtection }` and typed in `virtual.d.ts`. The `isr` option is in scope where `createConfigPlugin` is called (`src/index.ts:233`, `:307`).

`x-vercel-isr` appears exactly once in `packages/integrations/vercel/src` and `packages/astro/src` (the new line). There is no constant, comment or doc reference for it.

## 4. What was executed

All commands were run from `packages/integrations/vercel` on the head; the clone's `git status --short` was empty before and after.

Existing tests:

```
node --test test/path-override-security.test.js test/isr.test.js
  ✔ ignores untrusted x_astro_path query param on _render
  ✔ ignores untrusted x-astro-path header on _render
ℹ tests 4   ℹ pass 4   ℹ fail 0
```

Scratch probe (`clone-work/scratch/isr-gate.test.js`, outside the clone). It builds the existing `isr` and `serverless-with-dynamic-routes` fixtures through the repo's `test/test-utils.js`, imports the emitted function handlers the same way `path-override-security.test.js` does, and prints status and body. The built adapter was confirmed to be the head (`dist/serverless/entrypoint.js` contains the `x-vercel-isr` branch).

| Fixture / function | Request | `x-vercel-isr: 1` | Result |
| --- | --- | --- | --- |
| `isr` / `_isr` | `/_isr?x_astro_path=/one` | yes | `200` `<h1>One</h1>` |
| `isr` / `_isr` | `/_isr?x_astro_path=/one` | no | `404` |
| `isr` / `_isr` | `/_isr?x_astro_path=/api/secret` (`/^\/api/` is in `isr.exclude`) | yes | `200` `OK secret` |
| `isr` / `_render` | `/one?x_astro_path=/api/secret` | yes | `200` `OK secret` |
| `isr` / `_render` | `/one?x_astro_path=/api/secret` | no | `200` `<h1>One</h1>` |
| `serverless-with-dynamic-routes` / `_render` | `/api/public?x_astro_path=/api/private` | yes | `200` `{"id":"private"}` |
| `serverless-with-dynamic-routes` / `_render` | `/api/public?x_astro_path=/api/private` | no | `200` `{"id":"public"}` |

Rows 1–2 confirm the PR's stated fix. Rows 4 and 6 are the evidence for finding 1. Row 3 is the evidence for finding 2.

### Verification status per finding

| Finding | Verified here | Not verified here |
| --- | --- | --- |
| 1 | The handler logic: `_render` honours the override when the header is present (rows 4, 6), in a fixture with ISR disabled as well as one with it enabled. | Whether Vercel's edge lets a client-sent `x-vercel-isr` reach a function. No platform access. |
| 2 | The `_isr` handler renders an ISR-excluded route when given its path (row 3). | Whether `/_isr?…` is directly reachable by a client on a deployment and arrives carrying `x-vercel-isr: 1`. |
| 3 | By reading the diff; no tool was run for formatting or lint. | — |
| 4 | By reading `test/isr.test.js` and the PR body; the three proposed cases were run as the scratch probe. | — |
| 5 | By reading both changeset files. | — |

## 5. Finding 1 in depth — the gate

The handler now has two trust rules of very different strength. The first compares a header to a per-build random secret that only the generated edge middleware knows. The second compares a header to the constant `'1'`. The second is safe only if the platform owns that header name end to end. That may well be true, and the author's manual deployment shows Vercel sets it for ISR invocations, but the repo carries no statement of it, and the previous commit on this file exists because an assumption about who can supply an override turned out to be wrong.

The part that is checkable and fixable inside the repo is scope. A deployment with `isr` unset has no `_isr` function and no route that produces `x_astro_path`, yet its `_render` now contains a query-param override path. That exposure buys nothing. Emitting the flag at build time removes it:

```ts
// vite-plugin-config.ts
export interface Config {
	middlewareSecret: string;
	skewProtection: boolean;
	isr: boolean;
}

// index.ts, where the plugin is created
createConfigPlugin({ middlewareSecret, skewProtection, isr: Boolean(isr) }),
```

plus the matching `export const isr: boolean;` in `virtual.d.ts`. This is a narrowing, not a complete answer: in an ISR build `_render` and `_isr` still share the code, so the header check remains the discriminator there, and the question in the summary still needs an answer.

Two further discriminators would not depend on the header, and both need a check on a real deployment before anyone relies on them, so I list them as options and not as the recommendation. One is the request path: the route table sends ISR traffic to `/_isr?…`, so if the function sees `url.pathname === '/_isr'` that is a fact produced by the adapter's own routing. The other is a per-function marker written by `buildISRFolder` into the function's own config, if the Build Output API's function `environment` field is available for this launcher type.

## 6. Finding 2 in depth — the override value is unconstrained

`isr.exclude` is how a user says "this route must not be cached by ISR", and with edge middleware it is also what puts `_middleware` in front of the route. The route table enforces it for requests that arrive at the public path. The handler does not: it rewrites to whatever `x_astro_path` says. Row 3 above shows the consequence inside the handler.

The data needed to enforce it already exists at build time, in the loop at `src/index.ts:475-497` that decides, per route, between `NODE_PATH` and `ISR_PATH`. A sketch of the shape, which keeps one source of truth for "is this route ISR-served":

```ts
// index.ts — computed once, used for both routeDefinitions and the virtual config
const isIsrRoute = (route: IntegrationResolvedRoute) =>
	!route.isPrerendered &&
	!isExcludedFromIsr(route, isrConfig) &&
	!/^\^\\\/_(image|server-islands)/.test(route.patternRegex.source);
```

with the list of ISR-served route patterns emitted through the virtual config, and the handler accepting a query-param override only when `app.match` on the rewritten request yields one of them. This also removes the duplicated exclusion test that currently appears twice in that block (once to expand `RegExp` exclusions into `routeDefinitions`, once as `excludeRouteFromIsr`). I have not built this; the virtual config plugin is created in the config hook (`src/index.ts:307`) while `routes` is filled in later, so the ordering needs care, and that is the reason it is sequenced last in the summary.

## 7. Finding 3 in depth — worked code-judo for the handler

Target shape, behaviour-identical to the PR for every input (the three-state variable collapses to `string | null`, and the `typeof` test becomes a null check):

```ts
/**
 * Set by Vercel on requests it dispatches to a prerender (ISR) function.
 * <link to the Vercel reference that guarantees this, and a note on whether
 * the edge strips it from client requests>
 */
const VERCEL_ISR_HEADER = 'x-vercel-isr';

/**
 * The path the request was originally made to, when a trusted hop rewrote it:
 * - the edge middleware calls `/_render` and passes the path in a header, proven by the secret;
 * - Vercel's ISR routing calls `/_isr` and passes the path in a query param.
 */
function resolvePathOverride(
	request: Request,
	url: URL,
	hasValidMiddlewareSecret: boolean,
): string | null {
	if (hasValidMiddlewareSecret) {
		return request.headers.get(ASTRO_PATH_HEADER);
	}
	if (request.headers.get(VERCEL_ISR_HEADER) === '1') {
		return url.searchParams.get(ASTRO_PATH_PARAM);
	}
	return null;
}
```

and in `fetch`:

```ts
const realPath = resolvePathOverride(request, url, hasValidMiddlewareSecret);
if (realPath !== null) {
	url.pathname = realPath;
	…
}
```

What this deletes: the `let`, the unannotated `undefined` initialiser, the third state, the `typeof` discrimination, and the bare literal. What it adds: one function that is the whole answer to "who may tell this handler its path", which is where the `isr` flag from finding 1 and the eligibility check from finding 2 then go as one extra condition each, instead of as further `else if` arms in `fetch`. `VERCEL_ISR_HEADER` belongs beside `ASTRO_PATH_HEADER` and `ASTRO_PATH_PARAM` in `src/index.ts` if anything else will reference it, and `ASTRO_PATH_PARAM` there deserves the doc comment its neighbours have.

Formatting, for completeness: the added lines use `if(` and `else if(` and omit a semicolon on line 23, against `if (` on line 27 and semicolons everywhere else in the file. The repo's formatter bot will presumably rewrite it, so this is noted only because it is in the diff.

Adjacent, not introduced by this PR and not a finding: `url.pathname = realPath` leaves `x_astro_path` in the query string that user code sees on `Astro.url`, and percent-encodes a `?` inside the value (probe: `x_astro_path=/one%3Fa%3D1` with the header gives `404`). The ISR `$0` match never contains a query, so this does not affect the fix. If `resolvePathOverride` is extracted, stripping the parameter from `url.searchParams` in the ISR arm is a one-line follow-up worth considering separately.

## 8. Finding 4 in depth — the tests to add

`test/isr.test.js` has two tests and both read JSON. `test/path-override-security.test.js` already contains the helper that makes handler-level tests cheap:

```js
async function loadFunctionModule(fixture, functionName) { … return import(functionEntry); }
```

Proposed additions. The requests and the responses asserted in the first two were observed in the scratch probe (section 4); the tests themselves have not been run in this form, and the third asserts the outcome finding 1 asks for, not today's behaviour:

```js
// test/isr.test.js
it('serves an ISR route when Vercel passes the path as a query param', async () => {
	const isr = await loadFunctionModule(fixture, '_isr');
	const res = await isr.default.fetch(
		new Request('https://example.com/_isr?x_astro_path=/one', { headers: { 'x-vercel-isr': '1' } }),
	);
	assert.equal(res.status, 200);
	assert.match(await res.text(), /<h1>One<\/h1>/);
});

it('does not apply the query param without the ISR marker', async () => {
	const isr = await loadFunctionModule(fixture, '_isr');
	const res = await isr.default.fetch(new Request('https://example.com/_isr?x_astro_path=/one'));
	assert.equal(res.status, 404);
});
```

```js
// test/path-override-security.test.js — fixture has ISR disabled
it('ignores x_astro_path on _render even when x-vercel-isr is supplied', async () => {
	const renderFunction = await loadFunctionModule(fixture, '_render');
	const response = await renderFunction.default.fetch(
		new Request('https://example.com/api/public?x_astro_path=/api/private', {
			headers: { 'x-vercel-isr': '1' },
		}),
	);
	assert.equal((await response.json()).id, 'public');
});
```

The last test would fail on the head today (the probe got `private` for that request) and is expected to pass once the `isr` flag from section 5 is in place. `loadFunctionModule` should move to `test/test-utils.js` so the two files share it.

## 9. Finding 5 — changeset

`.changeset/common-cats-travel.md` body: `Fix vercel ISR path rewrite`. The neighbouring changeset from #15959 is a full sentence describing behaviour. Suggested: `Fixes a regression where routes served through ISR returned a 404.`

## 10. Checks that came back clean

- File size: `entrypoint.ts` 72 lines, `index.ts` 800, `middleware.ts` 151. No threshold is in play.
- Reuse: the PR re-imports the existing `ASTRO_PATH_PARAM` instead of repeating `'x_astro_path'`.
- Ordering: the secret-gated header is consulted first, so a request carrying a valid middleware secret is never downgraded to the weaker rule.
- The existing adapter tests named in the execution policy pass on the head.
