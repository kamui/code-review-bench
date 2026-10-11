# ISR transport and shared request routing

## Scope and measurements

The review compares local `main` at `b089b904f1ed578e9edaefd129bf9843120a808f` with `review-head` at `71ae513388df11d7dad6b1e0077c402ad03d0d62`. The packet's later base SHA was not used. `git diff main...review-head` reports two files, twelve insertions and one deletion. The runtime change imports the existing `ASTRO_PATH_PARAM` and replaces one authenticated path-source declaration with a mutable variable and a two-source branch.

`wc -l packages/integrations/vercel/src/serverless/entrypoint.ts packages/integrations/vercel/src/index.ts` reports 72 and 800 lines at the head. Reading the merge-base blobs gives 66 and 800 lines respectively. Neither file crosses 1,000 lines. The new changeset has five lines, declares an `@astrojs/vercel` patch and accurately names the intended fix. There are no actionable release-metadata findings.

Only the frozen skill and task packet governed this review. Repository guidance files, ambient configuration, upstream discussions and other reviews were not loaded. The frozen skill references no additional resource required for this task and does not require a child reviewer. This review used one primary context without delegation.

## Source evidence for F1

At the merge-base, `entrypoint.ts` selects `ASTRO_PATH_HEADER` only when `ASTRO_MIDDLEWARE_SECRET_HEADER` equals the generated middleware secret. Otherwise it assigns `null`; query parameters cannot select another route. The changed lines 24–25 add a separate authority: a request header equal to the literal string `1` enables `url.searchParams.get(ASTRO_PATH_PARAM)` even when the middleware secret is absent or incorrect.

Lines 27–36 then assign that value to `url.pathname`, construct another Request and call `app.match` with the resulting URL. This is route selection, not harmless metadata. The locals authorization check remains at lines 40–45, but an ordinary request with no locals header never enters that check. That later check does not protect the new query-path branch.

The builder trace explains why this is a structural regression. `src/index.ts:44–45` defines the existing transport keys. Lines 60–63 explain that ISR functions receive an internal path rather than the public original path and establish `/_isr?x_astro_path=$0`. Lines 452–453 show that ISR-enabled builds create `_render` from `entryFile`. Line 474 passes the same `entryFile` into `buildISRFolder` for `_isr`. Non-ISR builds also use `buildServerlessFolder(entryFile, NODE_PATH, ...)`, at line 499. The changed request code therefore runs in both transport contexts, even when ISR is disabled.

`VercelBuilder.buildISRFolder`, at lines 702–715, is the canonical owner of ISR-specific function output. It already supplies the expiration, bypass token and query policy alongside the copied handler. That is where the function's transport identity is known independently of incoming request data. The PR instead introduces an ISR check into the common handler and depends on a platform marker to distinguish those contexts at runtime.

`src/serverless/middleware.ts:128–136` shows a different, explicit contract for edge middleware: it fetches `/_render` and supplies the generated secret, original path header and serialized locals. That authenticated path source should keep precedence. There is no equivalent validation or function-identity check for the newly accepted marker in the reviewed entrypoint.

The branch is short and does not justify a giant framework, policy object or separate state machine. Its problem is that a mode-specific transport rule now changes ordinary route selection. Formatting, the missing semicolon and replacing `undefined` with `null` are not separate findings.

## Existing coverage and focused execution

The command was run once from `packages/integrations/vercel`:

```sh
node --test test/path-override-security.test.js test/isr.test.js
```

It exited successfully in approximately 4.1 seconds with four passing tests, two suites, no failures and no skips. The two security tests call the built `_render` handler and prove that an untrusted query alone and an untrusted `x-astro-path` header alone do not change the route. Neither test combines the query with the new marker.

The two ISR tests inspect `.prerender-config.json` and deployment route definitions. They verify `allowQuery: ['x_astro_path']`, `passQuery: true` and destinations such as `/_isr?x_astro_path=$0`. They never invoke the ISR handler, so they would pass even when ISR rendering returned 404. The PR adds no tests. The missing combinations should be addressed as verification of F1's remedy rather than as a second generic test-coverage finding.

The tests built their existing fixtures using the preinstalled dependencies, producing only the policy-authorized ignored output. No package installation, network access, adapter rebuild or other monorepo test was attempted.

## Actual-handler reproduction

The scratch script [isr-handler-probe.mjs](../isr-handler-probe.mjs) reads each fixture's `.vc-config.json` and imports the actual copied function handler by absolute file URL. It does not mock Astro, rewrite the built modules or contact a server. Run command:

```sh
node /home/jack/.t3/bench-runs-lanes/codex-thermo-sol61/2026-10-10-last-push-codex-thermo-sol61/att-006/clone-work/isr-handler-probe.mjs
```

It exited successfully. Its assertions encode the observed head behavior, including the undesired override; this is a reproducer, not a passing security regression test.

| Function and request | Observed result |
| --- | --- |
| Non-ISR `_render`, `/api/public?x_astro_path=/api/private`, no marker | 200, `id: public` |
| Same handler and URL, `x-vercel-isr: 1` | 200, `id: private` |
| Same handler and URL, marker plus incorrect middleware secret | 200, `id: private` |
| Non-ISR `_render`, `/api/public`, marker plus untrusted path header only | 200, `id: public` |
| Non-ISR `_render`, query override plus `x-vercel-isr: 0` | 200, `id: public` |
| ISR `_isr`, `/_isr?x_astro_path=/one`, no marker | 404 |
| Same ISR handler and URL, `x-vercel-isr: 1` | 200, One page |
| ISR `_isr`, `/_isr`, marker but no query override | 404 |
| ISR deployment's `_render`, `/does-not-exist?x_astro_path=/one`, marker | 200, One page |

The non-ISR fixture's `src/pages/api/[id].js` returns the matched parameter as JSON. The names `public` and `private` distinguish selected routes; the fixture does not implement authorization, and the reproduction does not claim it does. The important result is that ordinary route selection changes on an unauthenticated marker-plus-query request. Reading the merge-base declaration establishes why that combination could not select a query path previously; the merge-base handler was not separately built or executed.

## Worked code-judo proposal

Use the builder's existing function identity to select the ISR adapter, rather than asking the common renderer to infer its identity from every request. Restore the shared entrypoint's previous authenticated path rule. In `buildISRFolder`, after the common entry has been traced into the function directory, generate a small ISR entry module and set that function's `.vc-config.json` handler to the generated entry. It imports the traced common entry and owns only ISR URL normalization. `_render` keeps its current build output and cannot enable query overrides by receiving the marker.

The following is a worked shape for the generated module, not an applied patch. The import path is derived from the traced handler. The query-key literal is generated from the existing `ASTRO_PATH_PARAM` constant, so no independently maintained key is introduced.

```js
import render from './<traced-common-handler>';

const pathParam = 'x_astro_path'; // Emit from ASTRO_PATH_PARAM.

export default {
  fetch(request) {
    const url = new URL(request.url);
    const originalPath =
      url.pathname === '/_isr' && request.headers.get('x-vercel-isr') === '1'
        ? url.searchParams.get(pathParam)
        : null;

    if (originalPath !== null) {
      url.pathname = originalPath;
      request = new Request(url, request);
    }

    return render.fetch(request);
  },
};
```

This wrapper earns its existence by making the deployment transport boundary executable. It removes the ISR branch, ISR query import and mutable path-source variable from the shared renderer. It shares matching, locals validation, response rendering and cookie handling rather than copying them. The only added adapter owns a concrete wire-format conversion and is exported solely by the ISR function. Avoid a generic dispatch framework or public request-selected mode flag.

The shared entrypoint can retain its original direct declaration:

```ts
const realPath = hasValidMiddlewareSecret
  ? request.headers.get(ASTRO_PATH_HEADER)
  : null;
```

A narrower local edit would additionally require `url.pathname === '/_isr'` in the PR's new branch and could collapse the mutable variable into one declaration. That stops the ordinary public-path reproduction, but still allows an `_render` invocation with the internal ISR URL and marker to interpret the query. It narrows input conditions without moving ownership and is not a substitute for binding the decoder to the generated function.

The proposal sketch deliberately retains the current marker requirement. Removing it would require first proving the platform's ISR invocation contract. This review does not infer that guarantee from unavailable documentation or a production deployment.

## Proposal verification and integration work remaining

[isr-boundary-proposal.mjs](../isr-boundary-proposal.mjs) extracts the compiled shared handler object, restores exactly the merge-base path-source declaration in scratch memory, supplies a stub app and wraps it with the proposed ISR decoder. It runs without modifying the checkout. Command:

```sh
node /home/jack/.t3/bench-runs-lanes/codex-thermo-sol61/2026-10-10-last-push-codex-thermo-sol61/att-006/clone-work/isr-boundary-proposal.mjs
```

Seven assertions passed. Ordinary rendering ignores the marker both at a public URL and at `/_isr`. The ISR adapter restores `/one` for an internal URL with the marker. It leaves an unmarked internal URL, an unrelated public URL and a missing parameter unchanged. A valid middleware path header retains precedence over the ISR query after delegation.

These checks validate the routing policy against a stubbed app. They do not validate dependency tracing, the generated wrapper import, `.vc-config.json` mutation, full Astro rendering or a Vercel deployment of the proposal. Implementation must cover those integration points and preserve authenticated middleware behavior, including valid-secret requests with a missing path header. That unusual combination is not covered by the sketch's seven assertions and should receive an explicit policy before claiming complete behavior equivalence.

Actionable regression tests should invoke the actual generated handlers. Keep the existing unauthenticated query/header controls, add the marker-plus-query request to `_render`, assert the authenticated-header precedence, and cover `_isr` with valid, missing and incorrect marker data. Verify that ISR-excluded routes use the ordinary renderer and cannot acquire ISR decoding through request headers. The generated ISR wrapper and its configured handler path should be tested together rather than only comparing deployment route strings.

## Limits and open question

The entrypoint accepts the marker locally without authenticating it or tying it to a generated function identity. Whether an external caller can get that header through Vercel is unknown under this task's offline policy. Vercel edge middleware, firewall behavior, reserved-header filtering, direct function routing, ISR regeneration and cache keys were not executed. No claim of a demonstrated deployed authorization bypass or cache poisoning is made.

Q1 asks whether Vercel strips or overwrites the marker on all ordinary-render invocations. A positive guarantee could reduce the production security risk. It would not remove the source-level coupling between ordinary rendering and ISR transport decoding. F1's structural remedy gives the adapter its own explicit boundary instead of relying on that unstated external invariant.

The pre-review tracked status was clean and the checked-out SHA matched the pinned head. The final tree/status audit is recorded in the summary's companion index validation process. Scratch scripts and reports reside in `clone-work`; no repository remedy was applied.
