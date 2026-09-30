# 01 — Serverless entrypoint: path-override trust boundary

Scope: `packages/integrations/vercel/src/serverless/entrypoint.ts` (72 lines after the PR, +7/−1), read against its routing contract in `packages/integrations/vercel/src/index.ts` (`ISR_PATH`, `buildISRFolder`, ISR route generation at lines 450–496) and the prior hardening commit `335a20416` ("Require trusted secret for path overrides (#15959)").

## Background: what the entrypoint is deciding

The single serverless bundle (`entryFile`) is emitted twice: once as `_render.func` and, when `isr` is enabled, again as `_isr.func` (`buildISRFolder` calls `buildServerlessFolder` with the same `entry`). The `fetch` handler therefore runs in two different deployment roles:

- `_render`: normal SSR. Before #15959 it honoured `x-astro-path` or `?x_astro_path=` from anyone. #15959 restricted overrides to requests carrying the edge-middleware secret, and added `test/path-override-security.test.js` asserting that `_render` ignores an untrusted `?x_astro_path=`.
- `_isr`: Vercel's router rewrites ISR routes to `/_isr?x_astro_path=$0` (see the comment above `ISR_PATH`: ISR functions are not passed the original path). This function *must* honour the query parameter or every ISR route renders `/_isr` and 404s — that was the bug #15959 introduced and this PR fixes.

So the question the handler must answer is "am I the ISR function?", which is a build-time fact about the deployment artifact. The PR instead answers it at runtime from a request header.

## Finding 1.1 — the ISR branch trusts a client-controllable header and reopens the #15959 bypass on `_render` (blocker)

The new code is:

```ts
let realPath = undefined;
if(hasValidMiddlewareSecret) {
	realPath = request.headers.get(ASTRO_PATH_HEADER)
} else if(request.headers.get('x-vercel-isr') === '1') {
	realPath = url.searchParams.get(ASTRO_PATH_PARAM);
}
```

Nothing ties the `x-vercel-isr` check to the ISR function. Because `_render` is built from the same entry, `_render` now accepts `?x_astro_path=` whenever the request carries `x-vercel-isr: 1`, and that header is an ordinary request header an external client can send. The secret check that #15959 introduced is simply skipped on this branch: an invalid `x-astro-middleware-secret` combined with `x-vercel-isr: 1` also overrides the path.

**Verification: CONFIRMED in-process.** I ran the existing tests (`node --test test/path-override-security.test.js test/isr.test.js`, 4/4 pass) to build both fixtures, then called the built handlers directly from a scratch script (`clone-work/scratch/probe.mjs`):

```
_render no isr hdr      200 {"id":"public"}
_render x-vercel-isr    200 {"id":"private"}     <- /api/public?x_astro_path=/api/private + x-vercel-isr: 1
_render isr+secret-bad  200 {"id":"private"}     <- same, plus a wrong middleware secret
_isr no hdr             404 (404 page)           <- the bug being fixed, still present without the header
_isr x-vercel-isr       200 <h1>One</h1>         <- the fix works when the header is present
_isr isr + excluded     200 <h1>Dynamic</h1>     <- /_isr?x_astro_path=/excluded/x renders an ISR-excluded route
```

The first three lines are the regression: the exact request that `path-override-security.test.js` asserts must yield `public` yields `private` once one header is added. Whether this is exploitable on the Vercel platform depends on whether Vercel strips or overwrites an inbound `x-vercel-isr` before it reaches a non-ISR function. That header is not among the adapter's documented contracts, it cannot be verified offline, and the adapter should not depend on an undocumented platform sanitisation for a check that #15959 deliberately made explicit. See the open question in the summary.

The last line is secondary and largely pre-existing (it was true before #15959 as well): the ISR function will render any route, including ones the user listed in `isr.exclude`, if addressed directly as `/_isr?x_astro_path=…`. It is worth noting because a function-identity check (Finding 1.2) is the natural place to also refuse excluded or non-ISR route targets.

Remedy: the trust decision must not be derivable from request data an attacker controls. See 1.2 for the structural fix. At minimum, the ISR branch must be unreachable in the `_render` artifact.

## Finding 1.2 — code-judo: make "this is the ISR function" a build-time fact, and the runtime branch disappears

The handler is currently answering a static question (which artifact am I?) with a dynamic probe (which headers did this request carry?). That inversion is the root cause of both the original 404 and this PR's trust leak, and it is why the fix had to be a new `else if` bolted onto the security check.

The adapter already owns the artifact boundary: `buildISRFolder(entry, '_isr', isrConfig, root)` writes `_isr.func/.vc-config.json` and `_isr.prerender-config.json`, and the ISR route `dest` is the constant `ISR_PATH`. Two ways to push the fact to that boundary, in order of preference:

1. **Per-function build flag.** Have `buildServerlessFolder` accept the role and write it into that function's config, e.g. an `environment` entry in `.vc-config.json` for `_isr.func` only (Build Output API function configs accept an `environment` map; confirm against the current spec before relying on it), or emit a two-line `_isr` entry wrapper that imports the shared handler factory with `{ isr: true }`. The entrypoint then reads a constant, not a header.
2. **Route-identity check.** If a per-function flag is not feasible, gate on the one thing Vercel's router guarantees for ISR invocations and that the adapter itself defines: the rewritten pathname. Export the ISR function name from `index.ts` (it is currently a bare `'_isr'` literal passed to `buildISRFolder` and baked into `ISR_PATH`) and compare `url.pathname === `/${ISR_FUNCTION_NAME}``. This is weaker than (1) but is still anchored in the adapter's own routing contract rather than an undocumented platform header.

With (1), the resolution collapses to one pure function whose two modes are explicit and whose trust sources are named:

```ts
// serverless/entrypoint.ts
import { isISRFunction, middlewareSecret, skewProtection } from 'virtual:astro-vercel:config'; // or a per-function flag

function resolveOverridePath(request: Request, url: URL, hasValidMiddlewareSecret: boolean): string | null {
	// Edge middleware forwards the original path in a header, authenticated by the shared secret.
	if (hasValidMiddlewareSecret) return request.headers.get(ASTRO_PATH_HEADER);
	// Vercel invokes the ISR function at ISR_PATH; the original path is the only thing in the query.
	if (isISRFunction) return url.searchParams.get(ASTRO_PATH_PARAM);
	return null;
}
```

and the handler body goes back to a single `const realPath = resolveOverridePath(...)`, which is the shape it had after #15959. What disappears: the `'x-vercel-isr'` magic string, the `let` + mutable branch in the middle of `fetch`, and the implicit dependency on how Vercel decorates ISR requests. What becomes true by construction: `_render` can never honour `?x_astro_path=` regardless of headers, so `path-override-security.test.js` stays meaningful.

Verification: PROPOSAL, not executed (editing the clone is out of scope). The `_isr` 404 without the header in the probe above shows the only behaviour the build flag must reproduce is "query param honoured in `_isr`, ignored in `_render`".

## Finding 1.3 — boundary and legibility of the inserted branch (minor, subsumed by 1.2 if adopted)

Even keeping the runtime approach, the branch is written below the file's own conventions:

- Every other protocol header the entrypoint touches is a named, documented constant exported from `src/index.ts` next to `ASTRO_PATH_HEADER`/`ASTRO_PATH_PARAM` (lines 40–52). `'x-vercel-isr'` is an inline string literal with no comment explaining where it comes from or why it is trustworthy — which is exactly the property a reviewer needs to evaluate.
- `let realPath = undefined;` widens the variable to `string | null | undefined` (three states for "no override") and turns the previous single `const` expression into a mutable variable assigned from two branches in the middle of the busiest function in the file. The downstream `typeof realPath === 'string'` check only works because it happens to treat `null` and `undefined` alike.
- `if(` / `else if(` and the missing semicolon after `request.headers.get(ASTRO_PATH_HEADER)` do not match the repo formatter (the history contains `[ci] format` commits cleaning this up after merge). Cosmetic, but it signals the change did not go through the local format step.

Remedy: fold into the `resolveOverridePath` helper from 1.2 with a `string | null` return type and name the header constant in `index.ts` with a comment citing its provenance — or, better, delete the header dependency entirely per 1.2.

## File size / decomposition

`entrypoint.ts` is 72 lines; `index.ts` (800 lines) is untouched. No size concern.
