# Serverless ISR path rewrite

## Scope and measurements

The reviewed change is limited to `.changeset/common-cats-travel.md` and `packages/integrations/vercel/src/serverless/entrypoint.ts` (+12/−1 overall). The entrypoint remains well below the 1,000-line threshold; this patch adds a short conditional and no new helper or module. The concern is the changed trust boundary, not file growth.

Relevant changed code in `packages/integrations/vercel/src/serverless/entrypoint.ts:19-28`:

```ts
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
    // request is reconstructed with this pathname before app.match()
}
```

The pre-existing security tests in `packages/integrations/vercel/test/path-override-security.test.js` establish the local contract: `_render` ignores an untrusted `x_astro_path` query parameter and an untrusted `x-astro-path` header. The patch adds a second path source that bypasses the middleware secret whenever `x-vercel-isr` equals `1`. Since this value is read directly from the incoming `Request`, any caller able to supply that header can pair it with `x_astro_path` to make `_render` match another route. For example, on the existing dynamic-route fixture, a request to `/api/public?x_astro_path=/api/private` with `x-vercel-isr: 1` will replace the pathname with `/api/private` before route matching. The old security tests do not include this marker, so they do not cover the new path.

The actionable finding is located at `packages/integrations/vercel/src/serverless/entrypoint.ts:24-25`. It is a security-boundary regression and should be resolved before approval. This may permit route/access-control bypass where applications rely on the selected route to enforce access. Whether Vercel overwrites or strips this header in all deployment paths cannot be verified here; the handler itself does not authenticate its value, and the current tests deliberately treat untrusted path overrides as a security concern.

## Code-judo proposal

Do not make the generic serverless entrypoint infer ISR provenance from an ordinary incoming header. Give the ISR function an explicit invocation boundary: for example, emit a small ISR-specific entrypoint/configuration that owns extraction of `ASTRO_PATH_PARAM`, while `_render` retains only the middleware-secret path override. That turns the trust distinction into function identity and removes the special-case exception from the shared request path. If the existing build architecture cannot support a separate entrypoint, pass a build-time ISR mode only to the ISR function and validate the original invocation path against that mode; do not let the request header alone activate it.

The remediation should preserve the deployment rewrite from a public route to `/_isr?x_astro_path=...`, while ensuring ordinary `_render` requests cannot opt into the ISR override. Add a regression test that sends both `x-vercel-isr: 1` and `x_astro_path=/api/private` to `_render` and asserts the request still resolves to `/api/public`. Keep the existing test for the valid ISR route path and add an in-process test for whatever explicit trusted boundary is chosen. This moves complexity to the boundary that owns ISR and keeps route selection simple inside the handler.

## Verification status

Executed from `packages/integrations/vercel`:

```text
node --test test/path-override-security.test.js test/isr.test.js
```

Result: all four existing tests passed (two ISR configuration/routing checks and two path-override security checks). The path security tests do not include the new marker, so the passing result does not invalidate the finding. `git diff --check main...review-head` passed. The task packet makes Vercel's platform unavailable; the report therefore does not claim that a live Vercel deployment was reproduced.
