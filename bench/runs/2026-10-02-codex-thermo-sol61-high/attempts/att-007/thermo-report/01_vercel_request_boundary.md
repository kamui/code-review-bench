# Vercel request boundary, ISR restoration, and release metadata

## Scope and method

This is the sole subsystem detail report for the two-file committed change. Review evidence comes from the pinned diff, surrounding adapter code, existing fixture tests, and direct calls to the built function handlers. Repository guidance, external discussions, live deployments, ambient skills, and prior review judgments were not used. The selected frozen skill does not require child reviewers, and none were started.

The head is `71ae513388df11d7dad6b1e0077c402ad03d0d62`; local `main` is the merge-base `b089b904f1ed578e9edaefd129bf9843120a808f`. The moved base SHA in the packet was not used for the diff. `git diff main...review-head --numstat` reports five added changeset lines and seven added/one removed entrypoint lines.

## Measurements and source evidence

`git show main:packages/integrations/vercel/src/serverless/entrypoint.ts | wc -l` gives 66 lines, and `wc -l packages/integrations/vercel/src/serverless/entrypoint.ts` gives 72. The surrounding `index.ts` remains 800 lines at both revisions. `serverless/middleware.ts` has 151 lines and `vite-plugin-config.ts` has 33. No file crosses 1,000 lines. The new changeset has five lines and correctly specifies a patch for `@astrojs/vercel`.

`nl -ba packages/integrations/vercel/src/serverless/entrypoint.ts` establishes the relevant anchors. Lines 19–20 compare the middleware header with the generated secret. Previously, the original path came exclusively from `ASTRO_PATH_HEADER` after that comparison succeeded. New lines 21–26 introduce a mutable value initially set to `undefined`, then choose the authenticated middleware header or the query parameter based on `x-vercel-isr: 1`. Lines 27–34 reconstruct the request, and line 36 matches the resulting URL. The secret still protects locals at lines 40–45; the new query branch does not confer permission to inject locals.

`index.ts:44–45` defines `ASTRO_PATH_HEADER` and `ASTRO_PATH_PARAM`. Lines 59–63 document the ISR transport behavior and define `/_isr?x_astro_path=$0`. The parameter is therefore genuinely needed to restore a platform-rewritten ISR pathname. Reusing this exported constant is appropriate; there is no near-duplicate utility that the patch should have used instead.

`index.ts:450–453` creates the server entry and builds `_render`; line 474 passes that same entry to `buildISRFolder` for `_isr`. With ISR disabled, lines 498–500 still build `_render` from the server entry. `buildISRFolder` at lines 701–715 calls the same serverless folder builder and adds the prerender configuration. The generated function identity is already known here, but the runtime patch infers ISR identity from an incoming header instead. Both generated fixture bundles were inspected and contain the new branch, including the fixture whose adapter configuration has no ISR setting.

`serverless/middleware.ts:129–135` forwards incoming request headers and then supplies the authenticated Astro headers. That flow is source evidence of a separate trusted middleware protocol. It does not establish how Vercel sanitizes headers before that protocol runs. `vite-plugin-config.ts` exports the deployment's secret and skew-protection setting through a virtual module; it currently has no per-function identity contract.

## F1 — Bind ISR path restoration to function identity

The changed branch at `packages/integrations/vercel/src/serverless/entrypoint.ts:24–25` is an architectural trust-boundary regression, rather than merely an untidy conditional. It allows a request value to select a transport interpretation in the common renderer. As a result, code intended only to restore ISR paths also changes routing in `_render`, where an untrusted query override was previously ignored. Neither the actual function destination nor authentication is checked in that fallback.

The concrete reproduction calls the built non-ISR fixture's `_render` handler with `new Request('https://example.com/api/public?x_astro_path=/api/private', { headers: { 'x-vercel-isr': '1' } })`. Its JSON response has `id` equal to `private`. The same URL without the header has `id` equal to `public`. Supplying an invalid `x-astro-middleware-secret` and a contradictory `x-astro-path` still selects the query's `private` route. These are dynamic endpoint parameters in the existing fixture, not a claim that the fixture contains authorization-protected pages.

The practical risk is disagreement between the pathname presented to an upstream route policy and the route selected by Astro. That risk becomes externally reachable if Vercel forwards a supplied ISR marker to a normal serverless handler. Public bypass of middleware, firewall policy, or cache isolation was not tested, and this report does not claim it was. The reproduced problem is the shared handler accepting an unauthenticated rewrite outside the ISR function.

The remedy is to make the generated ISR transport responsible for its path metadata and to preserve `_render`'s existing refusal of query-based overrides. Commit a positive ISR handler test and negative `_render` tests alongside that change. A separate finding about missing tests would duplicate this root cause, so the test gap is included in F1's remediation.

## Verification commands and outcomes

From `packages/integrations/vercel`, the permitted command was:

```sh
node --test test/path-override-security.test.js test/isr.test.js
```

The command completed in approximately four seconds with four passing tests. The ISR suite's two tests assert prerender configuration and generated routes. The security suite's two tests assert rejection of an untrusted query parameter and an untrusted Astro path header without an ISR marker. Consequently these checks cannot detect the new combination.

The scratch test was written outside the clone and run once:

```sh
node --test /home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high/att-007/clone-work/isr-boundary.test.mjs
```

It reads each generated `.vc-config.json`, imports its actual handler using an absolute file URL, and calls `default.fetch`. It uses the output of the permitted fixture build and performs no fixture mutations or network calls.

| Check | Expected | Observed | Result |
| --- | --- | --- | --- |
| `_render`, query override alone | `id: public` | `id: public` | Pass |
| `_render`, query override plus supplied ISR marker | `id: public` | `id: private` | Fail |
| `_render`, query override plus marker and invalid secret | `id: public` | `id: private` | Fail |
| `_isr?x_astro_path=/one`, ISR marker present | 200 and `<h1>One</h1>` | 200 and matching page | Pass |
| Same internal ISR URL, marker absent | 404 under current behavior | 404 | Pass |

The two failures are assertion failures with actual `private` and expected `public`. The legitimate ISR check confirms that the local fix addresses its stated purpose under the simulated platform request shape. The absent-marker check records present behavior and is not a recommendation to require that behavior after restructuring.

The merge-base was inspected directly for the original path selector; no baseline build was performed. Attribution to this patch follows from the committed selector change and the runtime observations, rather than a claim of two fully executed builds. No full-monorepo tests or hosted tests were available or run.

`git diff --check main...review-head` completed successfully, and `git status --short` remained empty after tests. The head tree ID is `21d2d55ccba72775e9fa3274aa988cf28ffd3a4a`. Test builds produced only the ignored output permitted by the packet; those outputs and installed dependencies were left alone.

## Worked code-judo proposal

The structural simplification is to stop discovering the function's transport mode from the request. The build already emits separate `_render` and `_isr` folders. Give their launchers a small, explicit contract with the common renderer: a deployment-derived path or `null`. This boundary earns its existence because it separates two transport contracts; it is not a general policy framework or a generic request-rewrite abstraction.

The following sketches the common selector and the two launcher responsibilities. It is an architectural proposal, not an applied patch; exposing the common function through the generated server entry and wiring `.vc-config.json` need implementation in the builder.

```ts
// Common renderer: one place owns authenticated middleware precedence.
async function renderRequest(
  request: Request,
  trustedDeploymentPath: string | null,
): Promise<Response> {
  const hasValidMiddlewareSecret =
    request.headers.get(ASTRO_MIDDLEWARE_SECRET_HEADER) === middlewareSecret;
  const realPath: string | null = hasValidMiddlewareSecret
    ? request.headers.get(ASTRO_PATH_HEADER)
    : trustedDeploymentPath;

  // Retain the existing single URL/request reconstruction and app dispatch.
  // Retain locals authentication, secret removal, skew protection, and cookies.
  // ...
}

// Emitted ordinary serverless launcher.
const renderLauncher = {
  fetch(request: Request) {
    return renderRequest(request, null);
  },
};

// Emitted ISR launcher, bound by the build output to the ISR function.
const isrLauncher = {
  fetch(request: Request) {
    const path = new URL(request.url).searchParams.get(ASTRO_PATH_PARAM);
    return renderRequest(request, path);
  },
};
```

This removes `x-vercel-isr` as a runtime authority in the shared handler, removes the mutable `undefined` state, and keeps path selection in a `string | null` contract. It preserves authenticated middleware precedence, including the existing case where a valid secret with a missing path header does not fall back to a different path source. It does not require changing query propagation, app matching, locals handling, response cookies, or skew protection as part of this fix.

A deployment-wide virtual `isr` boolean would fail to establish the necessary boundary: ISR deployments have `_render` as well, and excluded routes must continue to use ordinary serverless semantics. Bind the choice in each generated launcher's code, not in input headers or a site-wide configuration flag. Merely moving the same header check into a helper would leave the trust problem intact.

The generated ISR launcher still needs the platform's existing route-rewrite contract for the path parameter. This proposal does not claim to solve arbitrary direct access or parameter collision on Vercel. Those contracts require platform evidence. Its concrete, testable improvement is that supplying an ISR marker to `_render` no longer changes the selected route.

The complete launcher wiring and compatibility checks were not implemented under the read-only policy. The sketches establish the ownership change and its effect, not a validated drop-in replacement. A smaller patch is also acceptable if it establishes an equally explicit function-specific boundary and passes the same tests.

## Q1 — What guarantees the provenance of the ISR marker?

The open platform question is whether Vercel strips or overwrites externally supplied `x-vercel-isr` on all normal function entry paths. No source in the supplied adapter implements such sanitization, and the new literal appears only in the patched runtime selector. Network and the Vercel platform are unavailable, so the question remains unverified. It is kept separate from the finding: local dispatch behavior is reproduced, while deployed attack reachability is conditional.

## Review disposition and remediation

Require the function boundary correction and its regression tests before approval under the frozen skill's boundary and special-case-growth standards. Avoid expanding this patch into unrelated cleanup of the builder or middleware generator. The current scale does not justify splitting files for size, and no dramatic broader simplification was demonstrated beyond moving transport ownership to the existing generated-function boundary.

After implementation, verify ordinary rendering, successful ISR rendering, absence of the path parameter, authenticated middleware precedence, and preservation of the rest of the common response flow. Platform-specific header provenance should be confirmed separately when that environment becomes available. The changeset is appropriate and needs no additional actionable comment.
