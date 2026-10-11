# Vercel request boundary and ISR transport

## Judgment

The patch repairs the positive ISR transport case in an in-process invocation. The maintainability blocker is its ownership model: a deployment-specific transport rule is inferred from an incoming header inside a shared request handler. That handler is also packaged for sites without ISR and for routes explicitly excluded from ISR. This is a concrete architecture-boundary regression, rather than a complaint about adding any conditional to a short function.

The actionable finding is the paragraph titled “Keep ISR query restoration out of ordinary serverless handlers” in `summary.md`, anchored to `packages/integrations/vercel/src/serverless/entrypoint.ts:24–25`. This report supplies its evidence and remedy. It does not claim a confirmed exploit against Vercel's production platform.

## Source evidence

The range is `main...review-head`: merge-base `b089b904f1ed578e9edaefd129bf9843120a808f`, head `71ae513388df11d7dad6b1e0077c402ad03d0d62`. The recorded pull-request base SHA was not used. The diff contains two files, 12 additions and one deletion: a five-line patch changeset and the serverless entrypoint's import and path selection.

At the merge-base, the entrypoint accepts an original path only from `ASTRO_PATH_HEADER` after checking equality with the generated middleware secret. Head retains that path source and adds an alternative source, `url.searchParams.get(ASTRO_PATH_PARAM)`, controlled only by equality of the request header `x-vercel-isr` with `'1'`. Both sources reach the existing pathname assignment and request reconstruction before `app.match`.

The new branch does not inspect deployment settings, packaged function identity, or the original pathname. The absence of a valid middleware secret is precisely the condition under which it can run. A supplied invalid secret does not fail this branch. The locals check still returns 403 for unauthenticated serialized locals; the finding concerns path selection, not an assertion that locals can now be forged.

The producer side already has an ISR distinction. `src/index.ts:63` defines `ISR_PATH` as `/_isr?x_astro_path=$0`. Lines 451–474 build `_render` and `_isr` from the same `entryFile` in an ISR deployment and send excluded routes to `_render` or middleware. Lines 498–499 build the same entry file into `_render` when ISR is disabled. `buildISRFolder` at lines 702–714 delegates function creation to `buildServerlessFolder` and then adds the prerender configuration. There is no runtime function-specific configuration introduced by this patch.

The existing virtual configuration in `src/vite-plugin-config.ts` supplies `middlewareSecret` and `skewProtection`. The edge middleware producer in `src/serverless/middleware.ts:132–135` sends the generated secret with the original-path header and serialized locals. That is an explicit authenticated transport. The new ISR source has no comparable locally verifiable provenance contract.

The names `public` and `private` in the reproduction are route parameter values from an existing fixture, not protected resources. Their role is to demonstrate that the application matches a path different from the requested path. An application or deployment that authorizes the original path before this rewrite would need to account for this second path source. Such an authorization bypass is conditional on actual upstream policy and marker handling, neither of which was executed here.

## Measurements and commands

The source inspection used these commands from the clone, with ordinary reads of the named files:

```sh
git diff main...review-head
git rev-parse HEAD main
git diff --stat main...review-head
git show main:packages/integrations/vercel/src/serverless/entrypoint.ts
rg -n 'ASTRO_PATH_PARAM|ASTRO_PATH_HEADER|x-vercel-isr|middlewareSecret' packages/integrations/vercel/src packages/integrations/vercel/test
wc -l packages/integrations/vercel/src/serverless/entrypoint.ts packages/integrations/vercel/src/index.ts packages/integrations/vercel/src/serverless/middleware.ts
git show main:packages/integrations/vercel/src/serverless/entrypoint.ts | wc -l
git diff --check main...review-head
git status --porcelain=v1 --untracked-files=all
```

The entrypoint is 66 lines at the merge-base and 72 at head. The unchanged index is 800 lines; the unchanged middleware module is 151. `git diff --check` passed. Initial tracked/untracked non-ignored status was clean. No file-size blocker exists.

The meaningful complexity increase is a second authority for path selection: authenticated middleware headers versus an ISR-marker/query pair. The shared handler has no deployment evidence to distinguish the second authority from ordinary request data. This is not a giant-file concern and does not justify extracting the entire 72-line entrypoint into a collection of generic utilities.

## Executed verification

The following focused selection ran once from `packages/integrations/vercel`:

```sh
node --test test/path-override-security.test.js test/isr.test.js
```

It passed four tests in two suites, with zero failures, cancellations, or skips, in approximately 3.8 seconds. The fixture builds used the installed adapter and dependencies, with no registry access. The built `dist/serverless/entrypoint.js` contains the reviewed marker branch.

`test/path-override-security.test.js:30–51` checks query-only and header-only overrides on `_render`, with neither case setting the new marker. `test/isr.test.js:16–25` checks the prerender configuration; the following test checks the generated route list. These successful tests establish that prior covered behavior and output configuration remain intact. They do not test ISR handler rendering or the new boundary condition.

A scratch probe at `../isr-boundary-probe.mjs` imports each packaged function using the handler path in its `.vc-config.json`, sends real `Request` objects, and checks responses. It was run once:

```sh
node /home/jack/.t3/bench-runs-lanes/codex-thermo-sol61/2026-10-10-last-push-codex-thermo-sol61/att-027/clone-work/isr-boundary-probe.mjs
```

The probe does not rebuild or patch source or built code. Its asserted observations are:

| Packaged function | Input | Observed response |
| --- | --- | --- |
| Non-ISR `_render` | `/api/public?x_astro_path=/api/private`, no marker | 200, `{"id":"public"}` |
| Non-ISR `_render` | Same URL, marker `'0'` | 200, `{"id":"public"}` |
| Non-ISR `_render` | Same URL, marker `'1'` | 200, `{"id":"private"}` |
| Non-ISR `_render` | Same URL, marker `'1'`, invalid middleware secret | 200, `{"id":"private"}` |
| ISR `_isr` | `/_isr?x_astro_path=/one`, no marker | 404 |
| ISR `_isr` | Same URL, marker `'1'` | 200, body contains `<h1>One</h1>` |
| ISR deployment's `_render` | `/api/public?x_astro_path=/api/private`, marker `'1'` | 200, `OK private` |

The last row uses the ISR fixture's API routes, which are excluded from ISR. The non-ISR fixture config contains no ISR option. These cases establish that the branch's reach extends beyond ISR transport even when build configuration distinguishes those requests.

The merge-base behavior was assessed directly from its committed source, not through an executed base build. Its path selection has no query-param branch, so the marker has no role there. No broader monorepo, hosted, or platform tests were run.

## Worked code-judo proposal

Use the packaging boundary that already distinguishes `_isr` from `_render`. Keep the ordinary entrypoint's previous secret-gated path selection. Install an ISR transport entry only when `buildISRFolder` packages `_isr`, so the ordinary renderer never needs to know about the marker or the query parameter. This removes the added alternative path source from every ordinary request and makes the exceptional transport visible in the emitted function configuration.

One concrete implementation path is to have the function builder return its relative handler path after dependency copying. `buildISRFolder` can generate a small ISR entry next to that handler, importing it with a computed relative module specifier, and set the ISR function's `.vc-config.json` handler to that entry. Ordinary `buildServerlessFolder` output stays on the original handler. Generate the parameter name from the existing `ASTRO_PATH_PARAM` constant; do not duplicate a new independent string contract. The wrapper is justified by a different deployed transport boundary, rather than being a pass-through convenience abstraction.

The entry's responsibility is limited to accepting the expected ISR transport, restoring the original pathname, and delegating to the ordinary renderer. In schematic form, after establishing the marker's trust contract:

```js
// Emitted only as the handler for _isr.func.
// render and PATH_PARAM are bound by the builder to the existing handler
// and the canonical ASTRO_PATH_PARAM value.
export default {
  async fetch(request) {
    const url = new URL(request.url);
    const originalPath = url.searchParams.get(PATH_PARAM);
    if (url.pathname !== '/_isr' ||
        request.headers.get('x-vercel-isr') !== '1' ||
        originalPath === null) {
      return render.fetch(request);
    }
    url.pathname = originalPath;
    const restored = new Request(url.toString(), {
      method: request.method,
      headers: request.headers,
      body: request.body,
    });
    return render.fetch(restored);
  },
};
```

This is a design sketch, not an implemented or tested remedy. It retains the existing reconstruction semantics and query retention for the positive ISR GET path. The marker guard is not authentication by itself. The wrapper must rely on an established platform guarantee or an adapter-controlled verifiable transport before accepting request-controlled original paths. Confirm the deployed ISR pathname contract before retaining the illustrated pathname guard. Authenticated edge-middleware invocations remain owned by the ordinary renderer; explicitly define and test precedence if an ISR invocation can also carry authenticated middleware metadata.

A deployment-wide `isrEnabled` boolean is a weaker proposal: it suppresses the branch in non-ISR sites but leaves it active in `_render` for exclusions within ISR sites. Adding that boolean and more feature checks would preserve the central ambiguity. Likewise, extracting this branch into a helper called unconditionally by both function kinds would move the code without changing its authority boundary.

The packaging proposal has real costs: one generated entry file, a relative import, and a function-configuration update. Review those costs against a smaller explicitly scoped transport design if the platform provides a trustworthy invocation identity. There is no reason to introduce a general policy-object framework or rewrite the surrounding rendering pipeline for this patch. What must be removed is the inference that a request marker alone establishes ISR context in the shared renderer.

## Remediation verification

Add behavioral assertions to the existing packaged-handler fixtures, rather than tests that only mirror the branch's expression. The positive test should call the packaged ISR handler at the rewritten transport URL and check both successful status and the expected page body. Negative tests should submit the marker-plus-query combination to non-ISR `_render` and to `_render` in an ISR deployment with exclusions, asserting that the requested route remains selected. Include an invalid middleware secret to prevent accidental acceptance of unverified metadata.

If authenticated middleware and ISR metadata may legitimately coexist, test their required precedence. Missing path parameters and unsupported marker values should have defined behavior. Any documented platform-header guarantee needs independent platform evidence; in-process requests cannot verify edge filtering, cache keys, cache regeneration, or firewall behavior.

These recommendations verify the same finding and its remedy. They are not separate test-coverage findings. No claim is made about query-key precedence in Vercel's rewrite engine or about POST/cache behavior, which would require evidence unavailable in this run.

## Other reviewed changes and limits

The changeset correctly requests a patch release of `@astrojs/vercel` and describes the fix. Reusing the existing path-parameter constant is appropriate. Request reconstruction stays in one place in the submitted patch, and no new casts, wrappers, concurrency, or partial state updates are introduced. Undefined/null handling and omitted formatting are low-value concerns here and are not escalated.

No ambient guidance, extra skills, prior reviews, network sources, or other reviewer contexts were used. The frozen skill did not require a child reviewer. Only authorized focused tests wrote fixture-generated ignored output. Scratch files and reports were written in the work directory, and no remedies were applied to the checkout. Final checkout identity checks are recorded in the summary.
