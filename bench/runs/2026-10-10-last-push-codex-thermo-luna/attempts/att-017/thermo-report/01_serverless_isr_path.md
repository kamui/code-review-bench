# Serverless ISR path override

## Finding: Caller-controlled ISR marker enables arbitrary route override

**Evidence.** The changed code in `packages/integrations/vercel/src/serverless/entrypoint.ts` lines 21–25 first uses `x-astro-path` only when `x-astro-middleware-secret` matches the generated middleware secret. The added `else if` uses the public request's `x-vercel-isr` header as the condition for assigning `url.searchParams.get(ASTRO_PATH_PARAM)` to `realPath`. Lines 27–34 then replace the request pathname with that value before `app.match()` and `app.render()`.

The generated ISR rewrite in `packages/integrations/vercel/src/index.ts` sends matched paths to `/_isr?x_astro_path=$0`, and sets `allowQuery` to that parameter. Those facts explain why the path parameter is needed for actual ISR handling, but they do not authenticate the marker at the request handler boundary. The existing path-override security tests call the handler with user-supplied path overrides and expect them to be ignored; they cover the ordinary `_render` request but not a request that also supplies `x-vercel-isr`.

**Reproduction and verification.** The focused test command, run from `packages/integrations/vercel`, was:

```text
node --test test/path-override-security.test.js test/isr.test.js
```

It passed all 4 tests. I then called the built `_render` handler in-process against the generated dynamic-route fixture twice, with the same URL `https://example.com/api/public?x_astro_path=/api/private`. With no added headers it returned `{"id":"public"}`; with `x-vercel-isr: 1` it returned `{"id":"private"}`. This demonstrates that callers reaching this handler can opt into the newly added route override and select another app route. The clone diff also passes `git diff --check`.

**Verification limit.** The execution policy makes Vercel's edge middleware, routing, firewall, and ISR cache unavailable. The in-process result verifies the handler behavior when the supplied header reaches it; it does not establish whether Vercel strips or overwrites a caller-supplied `x-vercel-isr` header before invocation. The implementation should not treat an unauthenticated request header as proof of platform provenance unless that platform guarantee is established and tested.

**Code-judo proposal.** Keep ISR path recovery, but make the trust decision explicit at the boundary instead of accepting a request-controlled marker. Carry or verify an unforgeable ISR provenance signal, or validate the proposed original path against adapter-generated ISR route data before changing `url.pathname`. Add paired tests: an ordinary direct request with a forged marker must stay on its requested route, while a genuine ISR invocation must still render the original route. This leaves the common path rewrite in one place and removes the implicit assumption that a `x-vercel-*` header is trustworthy merely because of its name.

**Action.** Resolve the provenance contract before relying on this override in production. Add regression coverage that reproduces the public/private route selection above and confirms that an ordinary caller cannot activate the ISR override. Verify the genuine ISR path on Vercel when a platform run is available.

## Measurements and scope

The target change is 12 insertions and 1 deletion across two files; the changed entrypoint is 72 lines after the change. It does not approach the skill's 1,000-line threshold, and the branch itself is small. The maintainability concern is not file growth: it is that a second path-override authority is introduced beside the authenticated middleware override, with the new authority relying on an externally supplied marker. No other actionable structural finding was identified in this narrow change.
