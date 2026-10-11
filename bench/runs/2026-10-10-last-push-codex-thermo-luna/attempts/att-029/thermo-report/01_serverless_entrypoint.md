# Serverless entrypoint

## Finding: Make the ISR path selection explicit and consistently formatted

The added code at `packages/integrations/vercel/src/serverless/entrypoint.ts:21-26` assigns the path through a mutable variable initialized as `undefined`, without declaring the intended `string | null` boundary. Its `if` and `else if` formatting and omitted semicolons also diverge from the surrounding module. This is a small clarity and consistency regression in a request boundary where the selected path comes from two distinct sources: the authenticated middleware header or the Vercel ISR query parameter.

The simpler repair is to keep the current straight-line branches and give the result an explicit nullable type, for example:

```ts
let realPath: string | null = null;
if (hasValidMiddlewareSecret) {
	realPath = request.headers.get(ASTRO_PATH_HEADER);
} else if (request.headers.get('x-vercel-isr') === '1') {
	realPath = url.searchParams.get(ASTRO_PATH_PARAM);
}
```

This keeps both source-selection rules visible where the request is rewritten, clarifies the value consumed by the existing string guard, and follows the file's syntax conventions. Extracting a helper or adding a new header abstraction would introduce more indirection than this six-line decision needs.

## Evidence and assessment

The entrypoint already uses `ASTRO_PATH_HEADER` and `ASTRO_PATH_PARAM` for the two path values, then applies one shared rewrite when `realPath` is a string. That shared rewrite is direct and avoids duplicating request reconstruction. The review found no reason to restructure that flow or split the code into another module. This finding concerns only the unclear local type and inconsistent formatting of the new decision.

The change does not push a file across the 1,000-line threshold; the modified entrypoint is 72 lines in the reviewed head. No new type casts, optional APIs, or additional helper layers were introduced.

## Verification status

Ran from `packages/integrations/vercel`:

```text
node --test test/path-override-security.test.js test/isr.test.js
```

Result: 4 tests passed, 0 failed. The ISR tests verify prerender configuration and generated routes; the path-override tests verify that untrusted query/header overrides are ignored on `_render`. Neither test directly invokes the new `x-vercel-isr` branch, so that branch remains without direct automated coverage in this run.

Also ran `git diff --check main...review-head`; it reported no whitespace errors. The checkout was clean after verification. No platform-level verification was possible under the packet's execution policy.
