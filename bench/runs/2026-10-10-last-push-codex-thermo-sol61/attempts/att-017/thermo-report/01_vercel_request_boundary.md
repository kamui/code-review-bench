# Vercel request normalization and ISR artifact ownership

## Scope and judgment

The change adds an import of `ASTRO_PATH_PARAM` and replaces one authenticated path selection expression with a mutable variable and two branches. The new branch is small, but it changes the ownership and trust contract of request normalization. There is one actionable finding: query-based ISR restoration is enabled in the common serverless handler rather than confined to the function that requires it.

The desired ISR behavior is confirmed in-process. The boundary regression is also confirmed in-process. External exploitability is unverified, and no Vercel deployment, upstream discussion, external documentation or reference answer was consulted. This review does not claim that the sample private API route performs authentication or that private data was disclosed: the route returns its dynamic parameter, making the selected route observable.

## Source evidence and measurements

The committed diff is `git diff main...review-head`, with the merge-base pinned to `b089b904f1ed578e9edaefd129bf9843120a808f` and head pinned to `71ae513388df11d7dad6b1e0077c402ad03d0d62`. It changes only the five-line patch changeset and the serverless entrypoint, for 12 additions and one deletion overall.

At head, `packages/integrations/vercel/src/serverless/entrypoint.ts:19–26` compares the incoming middleware secret with the build-generated secret, then takes the path header when authenticated or the query parameter when `x-vercel-isr` equals `1`. There is no check of the function artifact's role. At lines 27–36, that selected value becomes the URL pathname before `app.match(request)`. The new marker therefore changes actual routing, rather than merely exposing extra request metadata.

At `packages/integrations/vercel/src/index.ts:451–474`, an ISR build emits both `_render` and `_isr` from the same `entryFile`. At lines 498–505, a non-ISR build also emits `_render` from this entry. At lines 665–703, the builder copies the server entry into a function directory, and `buildISRFolder` uses the same `buildServerlessFolder` implementation. A common application module is sensible; assuming all copies should accept ISR transport metadata is the missing boundary.

At `index.ts:60–63`, the existing comment explains that ISR functions do not receive the original path, so the destination carries the regex match as `/_isr?x_astro_path=$0`. At lines 708–713, the prerender configuration allows that query and passes it through. These existing build-time concepts provide a canonical home for the new restoration behavior.

At `index.ts:260–261`, the middleware secret is described as verification that the caller is Astro's generated edge middleware rather than a third party. `src/serverless/middleware.ts:129–135` supplies that secret and the original path to `_render`. The common entrypoint still authenticates locals separately at lines 40–45. The ISR marker must not become a substitute for this secret or authorize locals.

The security tests at `test/path-override-security.test.js:30–51` explicitly establish that an ordinary `_render` query or path header must not replace the route. Neither case sends `x-vercel-isr`. The ISR tests at `test/isr.test.js:16–74` check only generated configuration and routes. The existing green suite therefore does not exercise the newly introduced trust branch.

The commands used to measure scope and source were `git diff --stat main...review-head`, `nl -ba` on the entrypoint, builder, middleware and tests, and bounded `rg` searches for the path constants and ISR marker in the adapter and Astro runtime sources. The only runtime handling of `x-vercel-isr` found in that source scope is the added branch. This is evidence of an absent repository contract, not proof about Vercel's proprietary routing.

`wc -l` reports 72 lines for the entrypoint, 800 for `index.ts` and 151 for middleware. `git show main:packages/integrations/vercel/src/serverless/entrypoint.ts | wc -l` reports 66 lines. No file crosses 1,000 lines. The new local `undefined`/`null`/string states are unnecessary for the final string check, but that is not a material standalone finding. The module is small enough that a general policy framework would be excessive. The actionable change is to narrow ownership of the ISR capability.

## Executed verification

From `packages/integrations/vercel`, the permitted selection was run once:

```sh
node --test test/path-override-security.test.js test/isr.test.js
```

All four tests passed: two configuration-generation tests and two path-override security tests. Node reported approximately 3.5 seconds total. The fixtures built their explicitly permitted ignored outputs. No dependency installation, rebuild of adapter sources, network access or wider monorepo execution was performed.

The scratch file `../isr-path-probe.mjs` reads `.vc-config.json`, imports the handler specified there by absolute filesystem URL and calls `default.fetch` with standard `Request` objects. It uses the built functions generated by the focused tests. It was executed once:

```sh
node /home/jack/.t3/bench-runs-lanes/codex-thermo-sol61/2026-10-10-last-push-codex-thermo-sol61/att-017/clone-work/isr-path-probe.mjs
```

The observations were:

| Generated function | Request and relevant headers | Status and observed body |
| --- | --- | --- |
| Non-ISR `_render` | `/api/public?x_astro_path=/api/private`, no marker | 200, `{"id":"public"}` |
| Non-ISR `_render` | Same URL, `x-vercel-isr: 0` | 200, `{"id":"public"}` |
| Non-ISR `_render` | Same URL, `x-vercel-isr: 1` | 200, `{"id":"private"}` |
| Non-ISR `_render` | Same URL, marker `1`, invalid middleware secret | 200, `{"id":"private"}` |
| Non-ISR `_render` | Same URL, marker `1`, unsigned `x-astro-locals: {}` | 403, `Forbidden` |
| ISR `_isr` | `/_isr?x_astro_path=/one`, no marker | 404 |
| ISR `_isr` | Same URL, `x-vercel-isr: 1` | 200, HTML containing `<h1>One</h1>` |
| ISR site's excluded `_render` API | `/api/public?x_astro_path=/api/private`, no marker | 200, `OK public` |
| ISR site's excluded `_render` API | Same URL, `x-vercel-isr: 1` | 200, `OK private` |

The non-ISR fixture declares `GET` at `src/pages/api/[id].js` and returns its matched `params.id`. The excluded ISR fixture declares `GET` at `src/pages/api/[dynamic].ts`, returning `OK ${params.dynamic}`. These observations demonstrate which dynamic route parameter Astro selected. No authentication assumption about the fixtures is needed to demonstrate path replacement.

The intended positive ISR assertion verifies the response status; the logged HTML additionally shows the page content. The missing-marker control verifies 404 status. The other seven cases verify both status and exact body. All assertions passed as observations of the head's actual behavior. A regression test for the proposed invariant would instead expect `public` in the marker-plus-query `_render` cases and would currently fail.

There was no executable merge-base comparison. The baseline behavior is established by the committed source diff: without a valid middleware secret, the old `realPath` expression evaluates to `null`, leaving the URL untouched. The marker did not appear in that expression.

## Worked code-judo proposal

The builder already knows whether it is constructing `_render` or `_isr`. Use that knowledge instead of asking each request to nominate its transport mode. Keep the shared application handler for rendering and authenticated middleware requests; give the ISR artifact a narrow entry that restores its transport path before calling that handler.

A possible generated wrapper has the following shape. This is a design sketch, not an applied patch. The import path and embedded secret must be generated from the existing build entry and the same build-owned secret, rather than accepted from request data.

```ts
import server from './application-entry.mjs';

// Embed the same private build value used by the application's virtual config.
const middlewareSecret = 'BUILD_GENERATED_VALUE';

export default {
  async fetch(request: Request): Promise<Response> {
    const authenticatedMiddleware =
      request.headers.get('x-astro-middleware-secret') === middlewareSecret;

    if (!authenticatedMiddleware && request.headers.get('x-vercel-isr') === '1') {
      const url = new URL(request.url);
      const path = url.searchParams.get('x_astro_path');
      if (path !== null) {
        url.pathname = path;
        request = new Request(url, request);
      }
    }

    return server.fetch(request);
  },
};
```

Restore the common entrypoint's former authenticated-only path selection. Generate and select this wrapper only in `buildISRFolder`; keep `_render`'s `.vc-config.json` pointed at the original application entry. The builder would need the existing middleware secret passed through from its caller, and it would write the wrapper beside the copied application entry before pointing the ISR function configuration's handler to it. The original application entry and its dependencies are already copied into that artifact. Production source should reuse the existing header/query constants when generating this text, as the middleware generator already does.

The wrapper's authentication check preserves the current precedence even for a valid secret without an original-path header: the new query branch remains disabled in that case. It never adds a middleware secret to an untrusted request. Synthesizing the secret merely to reuse the existing path-header branch would accidentally authenticate unsigned locals and is not an acceptable shortcut.

This restructuring removes ISR metadata handling from the shared rendering flow. It introduces an entry layer where the builder already owns deployment-specific entry selection, rather than introducing a general dispatcher or new mutable runtime state. Keeping the marker check inside the ISR wrapper preserves the observed missing-marker behavior; deleting that check would require confirming the platform invocation contract first. Preserve query handling, middleware precedence and request reconstruction semantics during implementation, including testing bodies if non-GET invocations are supported.

For a smaller patch, an equivalent explicit per-artifact capability can guard the added branch. Its value must distinguish the emitted function copies, not merely indicate that the site has ISR enabled. Both functions on an ISR site use the same source entry today, so adding `isr: true` to their shared virtual config would leave the demonstrated excluded-route problem intact. Merely checking for `/_isr` in a caller-controlled URL is also not an artifact identity or authentication guarantee. A named helper that reads the same marker everywhere would hide the defect without changing the boundary.

## Acceptance checks for the remedy

Exercise the generated entry files directly, following the existing security test's loader. An ordinary `_render` request to `/api/public` must still yield `public` when both the marker and the private-path query are supplied, including with an invalid middleware secret. Repeat against an ISR-excluded API, because a global ISR flag is an easy mistaken remedy. An `_isr` request with its generated path query and marker must return `/one` successfully. Authenticated middleware must retain path-header precedence, and unsigned locals must remain forbidden.

These checks are targeted verification for the one finding. They can run offline and do not require a mock Vercel cache. A deployment test or authoritative contract is separately needed to determine which client headers the platform permits to reach each artifact. Until then, the handler mismatch is proven and the possible upstream access-control consequence remains an inference.

## Changeset and remaining quality checks

`.changeset/common-cats-travel.md` contains a valid patch entry for `@astrojs/vercel` and describes the ISR path fix. It adds no configuration or runtime abstraction and has no actionable finding. `git diff --check main...review-head` reported no whitespace errors; the unformatted conditional and missing semicolon are cosmetic issues, not substitutes for the boundary review.

The change introduces no independent asynchronous tasks, multi-step state persistence, type casts, generic object shape or duplicate canonical path utility. The existing URL and request reconstruction remain in the natural request-adapter layer. The problem is specifically that the new transport capability is available in copies of that layer which do not serve ISR.

Before and after verification, `git status --porcelain=v1` was empty, and final worktree and index diffs were empty. Head remained the pinned review commit. All scratch and report files were written under `clone-work`. No source remedy was applied, no reviewer was delegated and no ambient guidance was loaded.
