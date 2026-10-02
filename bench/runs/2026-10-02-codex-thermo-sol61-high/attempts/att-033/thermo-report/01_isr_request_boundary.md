# ISR request restoration and the serverless boundary

## Scope and measurements

This subsystem contains the only changed implementation file, `packages/integrations/vercel/src/serverless/entrypoint.ts`, and its build-time producers and request-level tests. The release metadata in `.changeset/common-cats-travel.md` was also reviewed. Source evidence comes from the pinned committed range, not the later state of the merged repository.

The following read-only measurements establish the scope:

```sh
git rev-parse HEAD main
git diff main...review-head
git diff --numstat main...review-head
git show main:packages/integrations/vercel/src/serverless/entrypoint.ts | wc -l
wc -l packages/integrations/vercel/src/serverless/entrypoint.ts
git diff --check main...review-head
git status --porcelain=v1
```

HEAD is `71ae513388df11d7dad6b1e0077c402ad03d0d62`; main is `b089b904f1ed578e9edaefd129bf9843120a808f`. The changeset adds five lines. The entrypoint adds seven and removes one, growing from 66 to 72 lines. The diff check is clean, and status was clean before the review and after the focused tests. No changed file crosses 1,000 lines.

The diff imports `ASTRO_PATH_PARAM` and replaces an authenticated-only path selector with this selector at lines 21–26:

```ts
let realPath = undefined;
if(hasValidMiddlewareSecret) {
  realPath = request.headers.get(ASTRO_PATH_HEADER)
} else if(request.headers.get('x-vercel-isr') === '1') {
  realPath = url.searchParams.get(ASTRO_PATH_PARAM);
}
```

Lines 27–36 then assign that value to `url.pathname`, reconstruct the request, and call `app.match`. The rewrite therefore determines which Astro route executes, rather than merely correcting a display URL.

## Finding: Confine ISR path restoration to the ISR boundary

The changed anchor is `packages/integrations/vercel/src/serverless/entrypoint.ts:24–25`. This branch grants a second path-override mechanism whenever middleware authentication fails or is absent. Its only qualification is a literal request-header value. It does not know which Vercel function is running or whether ISR was configured for the deployment.

This distinction matters because `packages/integrations/vercel/src/index.ts:450–474` sends the same built entry to `buildServerlessFolder` for `_render` and to `buildISRFolder` for `_isr`. At lines 702–714, `buildISRFolder` delegates to the ordinary function builder and adds prerender configuration. There is no corresponding runtime separation. In the ISR-disabled fixture, the same serverless entry still contains the new branch.

At the merge-base, a request without the generated middleware secret always yields `realPath = null`, so its query cannot select another route. At the head, that same unauthenticated request can select another route by also setting `x-vercel-isr: 1`. The comparison follows directly from the committed source, and the head behavior was reproduced with the real built handlers.

The architectural regression is that an ISR deployment concern now changes general-purpose routing authority. In a deployment where clients can supply the header, routing checks performed against the external path can disagree with the route actually rendered. Whether Vercel allows that header through, and whether a particular application has vulnerable path-based policies, remain unverified. The fixture endpoint named `private` only echoes a parameter and is not an authenticated resource.

Do not fix this merely by extracting `getRealPath(request)` with the same inputs: that would obscure the missing execution-context boundary. Bind query restoration to the ISR function's generated entry, and establish a trusted provenance contract for internal routing metadata. Preserve the ordinary renderer's authenticated middleware-header behavior. Tests must cover the combined header/query input rather than only testing each input independently.

## Verification

The permitted focused tests ran once from `packages/integrations/vercel`:

```sh
timeout 300 node --test test/path-override-security.test.js test/isr.test.js
```

All four tests passed in approximately 4.4 seconds. `test/path-override-security.test.js:30–51` checks a query override without the ISR header and a path header without authentication. `test/isr.test.js` checks prerender configuration and generated routes. None calls `_isr` with the newly required signal, or combines the query override with that signal on `_render`. These results verify configuration and the existing negative cases; they do not protect the changed branch.

The scratch probe is saved at `../isr-review-probe.mjs`, outside the checkout. It reads each fixture's `.vc-config.json`, imports the configured handler, and invokes `default.fetch` with new WHATWG requests. It does not replace the application or stub the route matcher. The command ran once:

```sh
timeout 300 node /home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high/att-033/clone-work/isr-review-probe.mjs
```

The results were:

| Fixture/function | URL path and query | Additional headers | Result |
| --- | --- | --- | --- |
| ISR-disabled `_render` | `/api/public` | none | 200, `{"id":"public"}` |
| ISR-disabled `_render` | `/api/public?x_astro_path=/api/private` | none | 200, `{"id":"public"}` |
| ISR-disabled `_render` | `/api/public?x_astro_path=/api/private` | `x-vercel-isr: 1` | 200, `{"id":"private"}` |
| ISR-disabled `_render` | `/api/public?x_astro_path=/api/private` | `x-vercel-isr: 0` | 200, `{"id":"public"}` |
| ISR-disabled `_render` | `/api/public?x_astro_path=/api/private` | `x-vercel-isr: 1`, invalid middleware secret | 200, `{"id":"private"}` |
| ISR `_isr` | `/_isr?x_astro_path=/one` | none | 404 |
| ISR `_isr` | `/_isr?x_astro_path=/one` | `x-vercel-isr: 1` | 200, One page |
| ISR `_render` | `/api/public?x_astro_path=/api/private` | `x-vercel-isr: 1` | 200, `OK private` |
| ISR `_isr` | `/_isr?x_astro_path=/api/private` | `x-vercel-isr: 1` | 200, `OK private` |

The last case also establishes that the full application's route table is available inside `_isr`, including routes excluded by the deployment's ISR configuration. This is supporting boundary evidence, not a separate finding: ordinary requests are supposed to rely on deployment routing to select the appropriate function. The probe does not establish whether the platform permits direct calls with these inputs.

Verification status: intended GET ISR restoration confirmed; ordinary-handler route substitution confirmed; prior authenticated-only behavior established by base source inspection; valid-secret precedence preserved by source inspection, not a new execution test; deployed header sanitization, edge middleware, firewall behavior, and ISR cache behavior unavailable. No network calls, monorepo-wide test run, or adapter rebuild were performed. Fixture tests wrote only their permitted ignored build outputs.

## Worked code-judo proposal

The strongest simplification is to stop inferring the function's execution role from request contents. `VercelBuilder` already knows when it is constructing `_render` and `_isr`. Let that distinction select the path-normalization entry, keeping one shared rendering implementation.

The present flow is:

```text
_render and _isr
  -> same fetch
  -> authenticated middleware header OR request claims ISR
  -> rewrite pathname
  -> match and render
```

The proposed ownership is:

```text
_render -> authenticated middleware normalization -> shared render
_isr   -> ISR transport normalization             -> ordinary handler
```

A small generated ISR entry can import the already-built ordinary entry and normalize only the ISR transport URL before delegating. It should be generated beside the built entry so the existing `copyDependenciesToFunction` tracing follows the import. `buildISRFolder` would pass that entry to `buildServerlessFolder`; `_render` would continue using the ordinary entry. This requires neither a generic policy framework nor duplicate application rendering logic.

The following sketch illustrates the normalization operation, not a tested patch or a complete ingress-authentication design:

```ts
// This entry is emitted only for the ISR function.
import renderer from './entry.mjs'; // actual generated filename is used

export default {
  fetch(request: Request): Promise<Response> {
    const url = new URL(request.url);
    const path = url.searchParams.get('x_astro_path');
    if (path !== null) {
      url.pathname = path;
      request = new Request(url, request);
    }
    return renderer.fetch(request);
  },
};
```

The generated parameter literal should come from the existing `ASTRO_PATH_PARAM` constant. The ordinary renderer would retain its authenticated middleware path-header selector and lose the new ISR-header branch. The sketch preserves the search parameters and the legitimate GET regeneration behavior. Passing the original request as initialization illustrates retaining method, headers, and other request properties; body-bearing and abort behavior require checks before selecting the final cloning implementation.

This deletes header-based role discovery from the shared handler and confines the unavoidable transport adaptation to a boundary with a concrete purpose. The wrapper earns its existence by keeping ordinary request routing independent of ISR claims. Moving all authentication, locals parsing, cookies, and rendering into a new framework would add unnecessary indirection to a 72-line file.

Function separation is only part of the remedy. If client requests can invoke `_isr` with arbitrary routing metadata, that entry must also authenticate or validate its inputs using a supported platform contract. A different filename, an ISR-enabled boolean, or a pathname check alone is not proof that the query originated from a trusted rewrite. The proposal deliberately does not invent a Vercel request-signing or header-transform API that cannot be verified offline.

Use the existing dynamic-route and ISR fixtures for regression tests. A handler-level assertion should require the combined header/query request on `_render` to keep `id === 'public'`, including when a bogus middleware secret is supplied. A second assertion should call the actual ISR handler at its internal URL and require the One page. Preserve authenticated path-header priority and the ordinary negative cases. If the accepted design relies on Vercel sanitizing the header, add a separately verified deployment contract test when that platform is available.

The generated-entry restructuring has not been implemented, bundled, or deployed. Its tradeoff is one purposeful transport entry and a small builder adjustment. If maintainers choose the smaller runtime guard instead, it must use trustworthy execution context and satisfy the same negative tests; simply adding an `isrEnabled` flag does not distinguish `_render` from `_isr` on an ISR-enabled deployment.

## Remaining quality assessment

The extra nullable state in `realPath`, missing statement terminator, and spacing are local readability issues. They do not justify separate actionable findings under the skill's preference for a small number of consequential comments. There are no new casts, optional API parameters, generic dispatch frameworks, sequential async orchestration, or partial-state updates in this diff. ISR path restoration belongs in this adapter, not in Astro's general application matcher.

The changeset names the affected package and declares a patch release. Its wording is consistent with the intended repair. No changeset remedy or broad file decomposition is warranted.

## Open question

Does Vercel strip or overwrite a client-supplied `x-vercel-isr` header before invoking both `_render` and `_isr`, including excluded routes and direct function paths? The checkout provides no such guarantee, and the offline execution policy prevents checking it. Establish this contract before treating the header as trusted routing metadata; the in-process route substitution alone does not establish an edge-middleware or firewall bypass in production.
