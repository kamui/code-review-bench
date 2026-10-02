# Serverless routing and ISR transport

## Scope and judgment

The changed executable code is confined to `packages/integrations/vercel/src/serverless/entrypoint.ts`. The review followed its dependencies into the adapter's routing configuration, function packaging, virtual configuration, and existing tests. The architecture already distinguishes `_render` and `_isr` when emitting functions, but the new code tries to rediscover that distinction from an incoming header.

The actionable concern is an ownership and trust-boundary regression, with a demonstrated effect on the ordinary handler. The ISR recovery itself works in the tested case. No separate claim is made that the platform permits an Internet client to forge the marker.

## Finding and source evidence

At `entrypoint.ts:19–25`, middleware-secret equality first determines whether the original path header is accepted. The added `else if` then accepts the query path when `x-vercel-isr` equals `1`, including when the secret is absent or invalid. At lines 27–36, this path becomes the new request pathname before `app.match` selects a route. It therefore affects route selection, rather than just metadata exposed to a page.

The merge-base has only the secret-authenticated header path: `const realPath = hasValidMiddlewareSecret ? request.headers.get(ASTRO_PATH_HEADER) : null`. It does not use the ISR header to authorize query rewriting. Reading this baseline establishes the introduced delta without relying on review history.

At `index.ts:450–474`, `_render` and `_isr` are built from the same `entryFile`. At `index.ts:702–714`, `buildISRFolder` calls `buildServerlessFolder` with that entry and then writes the prerender configuration. The latter configuration includes `allowQuery: [ASTRO_PATH_PARAM]` and `passQuery: true`; it does not produce a distinct routing decoder. At `index.ts:665–699`, ordinary function packaging traces and copies that server entry and sets it as the function's configured handler.

The constant and transport routing already have canonical homes. `index.ts:44–45` declares `ASTRO_PATH_HEADER` and `ASTRO_PATH_PARAM`. Lines 60–63 explain the ISR original-path limitation and generate `/_isr?x_astro_path=$0`. This explains why restoring a query path is necessary, but does not justify enabling that decoder in `_render`.

The non-ISR dynamic-route fixture configures the adapter without `isr`. Its `src/pages/api/[id].js` returns the selected route parameter as JSON. This is a routing witness: the names `public` and `private` are test inputs, not evidence that the fixture has private data or authentication.

## Measurements and commands

`git diff main...review-head` shows two changed files, with 12 additions and one deletion: five additions in the changeset and seven additions/one deletion in the entrypoint. The source implementation adds one alternative path source and one request-header conditional. It does not add a module or a reusable policy contract.

`git show main:packages/integrations/vercel/src/serverless/entrypoint.ts | wc -l` reports 66 baseline lines. `wc -l packages/integrations/vercel/src/serverless/entrypoint.ts packages/integrations/vercel/src/index.ts` reports 72 and 800 head lines. The adapter index remains at 800 baseline lines. There is no file-size threshold finding.

`rg -n 'ASTRO_PATH_PARAM|ASTRO_PATH_HEADER|x-vercel-isr|middlewareSecret|path.override' packages/integrations/vercel/src packages/integrations/vercel/test` locates the path producers, consumer, secret contract, and focused tests. Within the adapter source, `x-vercel-isr` appears only in the new branch; no local module establishes its provenance or reserves it to an ISR function.

`nl -ba` reads of the entrypoint, builder, configuration plugin, and tests supplied the line anchors. `git show main:packages/integrations/vercel/src/serverless/entrypoint.ts` supplied the baseline. The built `dist/serverless/entrypoint.js` contains the same new branch, so the in-process fixture observations exercise the change under review.

## Existing verification

From `packages/integrations/vercel`, the permitted focused command was:

```sh
node --test test/path-override-security.test.js test/isr.test.js
```

All four tests passed, with zero failures, skips, or cancellations, in approximately four seconds. They were run once with these flags.

`path-override-security.test.js:30–51` calls the built ordinary function and asserts that an untrusted query override and an untrusted `x-astro-path` header do not alter `public`. Neither test supplies `x-vercel-isr: 1`, so they do not cover the new path into query rewriting.

`isr.test.js:16–74` asserts the generated prerender configuration and routing rules. It confirms that `/one` is sent to the ISR function with the original path encoded in the query, but never executes that function. The unchanged tests could therefore pass with the reported ISR 404 still present, or with the new cross-function rewriting behavior.

## Direct handler verification

The scratch script `../path-override-probe.mjs` uses no network and performs no build. It reads each already-built fixture function's `.vc-config.json`, imports its configured entry by absolute file URL, and calls `default.fetch` with a real `Request`. The probe command was:

```sh
node /home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high/att-019/clone-work/path-override-probe.mjs
```

All six assertions passed. The observations are:

| Function | Request | Headers | Result |
| --- | --- | --- | --- |
| Non-ISR `_render` | `/api/public?x_astro_path=/api/private` | none | 200, `{"id":"public"}` |
| Non-ISR `_render` | same | `x-vercel-isr: 0` | 200, `{"id":"public"}` |
| Non-ISR `_render` | same | `x-vercel-isr: 1` | 200, `{"id":"private"}` |
| Non-ISR `_render` | same | `x-vercel-isr: 1`, incorrect middleware secret | 200, `{"id":"private"}` |
| `_isr` | `/_isr?x_astro_path=/one` | none | 404 |
| `_isr` | same | `x-vercel-isr: 1` | 200, body includes `<h1>One</h1>` |

The script asserts the current `private` result in order to record the regression. A remediation regression test should instead require `public` for these ordinary-renderer requests. Passing the probe is evidence of the current behavior, not an approval signal.

These results verify the restored ISR route and the widened ordinary-handler path source. They do not simulate a Vercel cache miss, cache hit, firewall, header sanitizer, or edge middleware. The `/api/private` route name does not prove a data leak. An external authorization bypass would additionally depend on platform forwarding and application routing or authorization rules.

## Worked code-judo proposal

Use the function identity the builder already knows. The ordinary function should expose only authenticated middleware decoding. The ISR function should expose query-path decoding because packaging explicitly selects it. This removes `x-vercel-isr` from path-source selection altogether and avoids requiring callers or maintainers to infer deployment identity from a magic header.

A concrete design is to keep a single private rendering function, with the decoded ISR path passed only by an ISR-specific method on the adapter entry. The sketch below shows ownership and precedence; it is a proposal, not a tested patch:

```ts
async function render(request: Request, isrPath: string | null): Promise<Response> {
  const url = new URL(request.url);
  const hasValidMiddlewareSecret =
    request.headers.get(ASTRO_MIDDLEWARE_SECRET_HEADER) === middlewareSecret;
  const realPath = hasValidMiddlewareSecret
    ? request.headers.get(ASTRO_PATH_HEADER)
    : isrPath;

  // Keep the existing request rewrite and rendering pipeline here:
  // match, authorize locals, remove secret, skew protection, render, cookies.
  // Only the source of realPath changes.
}

export default {
  fetch(request: Request): Promise<Response> {
    return render(request, null);
  },
  fetchIsr(request: Request): Promise<Response> {
    const url = new URL(request.url);
    return render(request, url.searchParams.get(ASTRO_PATH_PARAM));
  },
};
```

The explicit `string | null` input models one decoded transport value, rather than an optional boolean that activates special handling throughout the renderer. The authenticated middleware branch remains canonical and retains precedence even if the transport query is also present. The ordinary method cannot acquire an ISR path through HTTP headers. The two entry methods earn their existence by binding different transport capabilities, while all application rendering remains in one function.

The current Astro auto-entrypoint re-exports the adapter's default object (`packages/astro/src/core/build/plugins/plugin-ssr.ts:42–46`), and the generated fixture `entry.mjs` does the same. A `fetchIsr` method on that object would therefore remain reachable through the generated entry without switching Astro to the legacy named-export API. This is relevant because simply adding an arbitrary named export would not be carried through this auto-entrypoint.

For packaging, have `buildServerlessFolder` return its existing handler location and the configuration it just wrote. The ISR-specific builder can then emit a small wrapper into its own function folder, point `.vc-config.json.handler` to that wrapper, and retain the runtime, launcher, duration, and streaming fields. A generated wrapper would import the copied entry using the returned relative handler path:

```js
import server from './<copied server handler>';
export default {
  fetch(request) {
    return server.fetchIsr(request);
  },
};
```

Generate that import with a normalized relative path and `JSON.stringify`, following the codebase's existing generated-module practice. The wrapper is selected by the `_isr` artifact, while `_render` continues to point to the ordinary copied entry. It is a transport boundary, not a second render implementation. Keep its generation inside the ISR builder, which already owns the prerender configuration, rather than scattering ISR flags through general rendering or adding a generic policy framework.

This proposal adds a small artifact-level adapter but deletes the runtime request-header mode and its implicit authenticity assumption. It leaves the existing path constants, render pipeline, and locals-secret checks in their canonical layers. It does not require decomposing an otherwise cohesive 72-line module or moving the full rendering pipeline to a new file.

The design preserves valid middleware path precedence, ordinary request behavior, query retention, and the legitimate ISR query rewrite. It intentionally removes the reproduced untrusted ordinary-handler rewrite. It also makes ISR decoding independent of the marker's presence; the actual generated ISR artifact becomes the source of that authority.

No code-judo implementation was applied or executed. The packaging integration, generated wrapper path, and retained function configuration need verification in an implementation. Do not interpret the design sketch as evidence that a replacement already passes tests.

## Actionable verification for the remedy

Extend the existing ordinary-handler test with marker `1`, both absent and invalid secrets, requiring the original route. Add an ISR handler test using `/_isr?x_astro_path=/one`, requiring the page response without relying on marker authenticity. These are meaningful behavior tests, not assertions that mirror a helper's implementation.

Check that a valid middleware path wins over an ISR query path and that locals remain forbidden without the valid secret. Inspect the generated handler configuration to ensure `_isr` selects the wrapper and `_render` selects the ordinary entry. Retain the existing routing and prerender configuration assertions. The focused suite plus these behavioral cases directly exercise the ownership change.

## Remaining question and limits

Vercel's sanitization of `x-vercel-isr` was not available for verification. If the platform guarantees that ordinary invocations cannot receive a client-controlled marker, the demonstrated local path override is not by itself evidence of a deployed vulnerability. That guarantee would still be an external prerequisite of a feature-specific conditional in the common handler. The proposed function boundary eliminates that prerequisite from ordinary rendering.

No additional findings are raised for query retention, missing `duplex`, middleware query encoding, cache behavior, or deployment-specific ISR header values. Some request construction and middleware behavior predate this PR, and the available evidence does not establish an introduced defect in those areas.

Tracked and visible untracked checkout state remained clean, and refs remained pinned. Only the explicitly permitted fixture tests rebuilt ignored output. All reviewer-written files are under the work directory.
