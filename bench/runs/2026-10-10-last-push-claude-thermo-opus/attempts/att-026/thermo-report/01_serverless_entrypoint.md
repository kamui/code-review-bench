# 01 — Serverless entrypoint (`packages/integrations/vercel/src/serverless/entrypoint.ts`)

Detail file for the review of withastro/astro#16079, range `b089b904f..71ae51338`.

## What changed

```diff
-		const realPath = hasValidMiddlewareSecret ? request.headers.get(ASTRO_PATH_HEADER) : null;
+		let realPath = undefined;
+		if(hasValidMiddlewareSecret) {
+			realPath = request.headers.get(ASTRO_PATH_HEADER)
+		} else if(request.headers.get('x-vercel-isr') === '1') {
+			realPath = url.searchParams.get(ASTRO_PATH_PARAM);
+		}
```

plus the re-added `ASTRO_PATH_PARAM` import and a changeset reading "Fix vercel ISR path rewrite".

## Background established from the clone

- `src/index.ts:63` — `const ISR_PATH = \`/_isr?${ASTRO_PATH_PARAM}=$0\`;` with the comment that ISR functions are not passed the original path. Every non-excluded on-demand route gets this `dest` (`src/index.ts:485-494`).
- `src/index.ts:702-715` — `buildISRFolder` calls `buildServerlessFolder(entry, functionName, root)` with the same entry file used for `_render`, then writes `allowQuery: [ASTRO_PATH_PARAM], passQuery: true`. `_render.func` and `_isr.func` are therefore the same code.
- Commit `335a20416` (#15959, "Require trusted secret for path overrides") removed `?? url.searchParams.get(ASTRO_PATH_PARAM)` and gated the header on the middleware secret. Its changeset: "Fix Vercel serverless path override handling so override values are only applied when the trusted middleware secret is present." Its squashed commit list includes "fix(vercel): remove ISR secret plumbing". That removal is what broke ISR; this PR is the follow-up.
- `src/index.ts:261` — `middlewareSecret = crypto.randomUUID()` per build, passed to `createConfigPlugin` and available to the entrypoint as `virtual:astro-vercel:config`. It is generated unconditionally, not only when edge middleware is enabled.
- `test/isr.test.js` shows `{ handle: 'filesystem' }` precedes the generated routes, so `_isr` is reachable by its own path as well as through the rewrite.

## Measurements

File sizes at head: `src/serverless/entrypoint.ts` 72 lines, `src/index.ts` 800 lines (unchanged by the PR). No size concern.

Tests at head, from `packages/integrations/vercel`:

```
node --test test/path-override-security.test.js test/isr.test.js
▶ ISR                                         2 pass
▶ Vercel serverless path override security    2 pass
tests 4, pass 4, fail 0
```

Neither file sends `x-vercel-isr`; `isr.test.js` asserts only `_isr.prerender-config.json` and `config.json`.

In-process probe of the built fixture handlers (script preserved next to this file as `probe.mjs.txt`; it imports each fixture's `.vercel/output/functions/<fn>.func/.../dist/server/entry.mjs` and calls `default.fetch`):

| Fixture / function | Request | Extra header | Result |
| --- | --- | --- | --- |
| serverless-with-dynamic-routes `_render` | `/api/public?x_astro_path=/api/private` | none | 200 `{"id":"public"}` |
| serverless-with-dynamic-routes `_render` | `/api/public?x_astro_path=/api/private` | `x-vercel-isr: 1` | 200 `{"id":"private"}` |
| serverless-with-dynamic-routes `_render` | `/_render?x_astro_path=/api/private` | `x-vercel-isr: 1` | 200 `{"id":"private"}` |
| serverless-with-dynamic-routes `_render` | `/api/public` + `x-astro-path: /api/private` | `x-vercel-isr: 1` | 200 `{"id":"public"}` |
| isr `_isr` | `/_isr?x_astro_path=/one` | none | 404 (the bug this PR fixes) |
| isr `_isr` | `/_isr?x_astro_path=/one` | `x-vercel-isr: 1` | 200 page One (the fix) |
| isr `_isr` | `/_isr?x_astro_path=/two` (ISR-excluded) | `x-vercel-isr: 1` | 200 page Two |
| isr `_isr` | `/_isr?x_astro_path=/api` (ISR-excluded) | `x-vercel-isr: 1` | 200 `OK` |
| isr `_render` | `/one?x_astro_path=/two` | `x-vercel-isr: 1` | 200 page Two |

## Verification status

Confirmed by execution: the fix restores ISR rendering when the header is present; the `_render` handler honours `x_astro_path` whenever the request carries `x-vercel-isr: 1`; the header-carried override (`x-astro-path`) still requires the secret; existing tests pass and do not exercise the new branch.

Confirmed by reading: both functions share one entry; the secret is available to the entrypoint at build time; the route destination is authored by the adapter.

Not verifiable here (no Vercel platform): whether the edge strips a visitor-supplied `x-vercel-isr`; what Vercel stamps on a direct request to `/_isr?...`; whether an extra query parameter in an ISR route `dest` survives `allowQuery` when listed there. The findings are stated so they hold regardless: even if the header cannot be forged, the entrypoint's correctness now depends on an undocumented platform guarantee that the code neither names nor tests, and the rule runs in a function (`_render`) that has no use for it.

## Why this is a structural problem, not a nit

Before #15959 the rule was "any request may override the path". After it, the rule was one sentence: "the path override is believed only with the secret". After this PR it is: "believed with the secret if it is in the header; or, without the secret, believed from the query if a certain request header equals `1`". The second clause is ISR knowledge living in the shared serverless path, and its predicate is a property of the incoming request rather than of the build. The adapter knows at build time which routes go to ISR and what secret it minted; none of that knowledge is used.

There is also an ordering subtlety the `if / else if` hides: when the secret is valid but `x-astro-path` is absent, the ISR arm is skipped. That is harmless today, but it is the kind of incidental coupling a helper with early returns makes explicit.

## Worked code-judo proposal A — one trust rule, two carriers

`src/index.ts`:

```ts
export const ASTRO_PATH_HEADER = 'x-astro-path';
export const ASTRO_PATH_PARAM = 'x_astro_path';
export const ASTRO_SECRET_PARAM = 'x_astro_secret';

// was: const ISR_PATH = `/_isr?${ASTRO_PATH_PARAM}=$0`;
const isrDest = (secret: string) =>
	`/_isr?${ASTRO_PATH_PARAM}=$0&${ASTRO_SECRET_PARAM}=${secret}`;

// buildISRFolder
allowQuery: [ASTRO_PATH_PARAM, ASTRO_SECRET_PARAM],
```

`src/serverless/entrypoint.ts`:

```ts
/**
 * The function is not always invoked at the path the visitor asked for: edge middleware
 * calls `/_render` with the path in a header, and ISR routes rewrite to `/_isr` with it
 * in the query. Either carrier is only believed alongside the build-time secret.
 */
function trustedPathOverride(request: Request, url: URL): string | null {
	if (request.headers.get(ASTRO_MIDDLEWARE_SECRET_HEADER) === middlewareSecret) {
		return request.headers.get(ASTRO_PATH_HEADER);
	}
	if (url.searchParams.get(ASTRO_SECRET_PARAM) === middlewareSecret) {
		return url.searchParams.get(ASTRO_PATH_PARAM);
	}
	return null;
}

// in fetch
const realPath = trustedPathOverride(request, url);
if (typeof realPath === 'string') {
	url.pathname = realPath;
	url.searchParams.delete(ASTRO_PATH_PARAM);
	url.searchParams.delete(ASTRO_SECRET_PARAM);
	request = new Request(url.toString(), { method: request.method, headers: request.headers, body: request.body });
}
```

What this deletes: the `x-vercel-isr` literal, the second trust root, the `let`, and the dependency on undocumented platform header behaviour. What it keeps: `hasValidMiddlewareSecret` for the locals check and for hiding the header from user code, unchanged. The secret is constant per deployment, so adding it to `allowQuery` does not fragment the ISR cache. `.vercel/output/config.json` is build output, not a served asset, and the same secret is already embedded in the generated middleware bundle. Deleting the two transport parameters before building the request also stops them appearing in `Astro.url` on ISR pages, which the current code (and the pre-#15959 code) leaves in place.

Cost and caveat: `isr.test.js` expects the literal `dest: '/_isr?x_astro_path=$0'` and would need to match the secret by pattern; and the approach must be confirmed on a real deployment, because #15959's history suggests ISR secret plumbing was attempted and withdrawn for a reason this clone does not record.

## Proposal B — fallback if the secret cannot ride the rewrite

Keep the header check but make the query channel exist only where it is needed, so `_render` cannot be affected by it. `buildISRFolder` already owns the `_isr` function's configuration; give that function a build-time marker the request cannot influence (for example an entry in the function's `.vc-config.json` environment, if the Build Output API accepts it for this launcher — to be verified) and read it once at module scope:

```ts
const isIsrFunction = process.env.ASTRO_VERCEL_ISR === '1';
…
if (isIsrFunction && request.headers.get(VERCEL_ISR_HEADER) === '1') {
	return url.searchParams.get(ASTRO_PATH_PARAM);
}
```

This is weaker than A (the `_isr` function still believes any query it is handed) but restores "`_render` only obeys the secret", and it needs a comment linking the Vercel documentation that guarantees the header's provenance.

## Tests to add (both proposals)

In `test/path-override-security.test.js`, reusing `loadFunctionModule`, with a second `describe` built on `./fixtures/isr/`:

```js
it('renders the rewritten path on _isr', async () => {
	const isr = await loadFunctionModule(fixture, '_isr');
	const response = await isr.default.fetch(new Request(isrUrlFor('/one')));
	assert.equal(response.status, 200);
	assert.match(await response.text(), /<h1>One<\/h1>/);
});

it('ignores x_astro_path on _render regardless of headers', async () => {
	const render = await loadFunctionModule(fixture, '_render');
	const response = await render.default.fetch(
		new Request('https://example.com/one?x_astro_path=/two', { headers: { 'x-vercel-isr': '1' } }),
	);
	assert.match(await response.text(), /<h1>One<\/h1>/);
});
```

`isrUrlFor` builds the URL from the `dest` actually emitted in `config.json`, so the test follows whichever proposal is adopted. At this head the first passes and the second fails (page Two is returned).

## Smaller items folded into finding 3

- `let realPath = undefined;` has no annotation; the type is an evolving implicit `any`. The repository's Biome config turns `noImplicitAnyLet` off, so nothing flags it, but the previous line was a typed `const`.
- `'x-vercel-isr'` is the only protocol name in the file not imported as a constant.
- `if(` / missing semicolon: not formatted by the repository's formatter (inspection only; the formatter was not run).
- Changeset: "Fix vercel ISR path rewrite" names the mechanism, not the user-visible symptom.
