# Review blind-08baa6

### Item 1
Location: packages/integrations/vercel/src/serverless/entrypoint.ts:24
Claim: The path override is gated on a request header (`x-vercel-isr: 1`) that any client can send, and the same entrypoint is bundled into `_render`, so the untrusted `x_astro_path` override closed by #15959 is reopened.
Consequence: Confirmed in-process against the built `_render` handler of the `serverless-with-dynamic-routes` fixture: `GET /api/public?x_astro_path=/api/private` with header `x-vercel-isr: 1` returns `{"id":"private"}` (without the header it returns `public`). This holds even for deployments with ISR disabled. Anything keyed on the public URL (edge middleware auth, firewall rules, route rules) is bypassed unless Vercel strips this undocumented header from client requests, which could not be checked here. The existing `path-override-security.test.js` still passes because it never sends the header.
Fix: —

### Item 2
Location: packages/integrations/vercel/src/serverless/entrypoint.ts:25
Claim: The `_isr` function trusts any `x_astro_path` value, so it will render any route in the app, including routes listed in `isr.exclude` that are meant to go to `_render` or `_middleware`.
Consequence: Confirmed in-process on the `isr` fixture: `/_isr?x_astro_path=/two` and `/_isr?x_astro_path=/api/secret` (both excluded from ISR) return 200 with `Two` and `OK secret`. `/_isr` is a publicly routable function path, so a client can request it directly; excluded routes that rely on edge middleware for auth are rendered without middleware running, and per-user responses can end up stored in the shared ISR cache. No check that the path is an ISR-eligible route is made.
Fix: —

### Item 3
Location: packages/integrations/vercel/src/serverless/entrypoint.ts:29
Claim: The rewritten `new Request(...)` passes `body: request.body` without `duplex: 'half'`, so any request with a body that takes the newly restored ISR rewrite branch throws.
Consequence: Confirmed in-process: `POST /api/public?x_astro_path=/api/private` with a body and `x-vercel-isr: 1` throws `TypeError: RequestInit: duplex option is required when sending a body`. A form submission or Astro action POSTed to an ISR-served page yields a 500 instead of a response. The line is unchanged, but this PR routes ISR traffic back through it.
Fix: —

### Item 4
Location: packages/integrations/vercel/src/serverless/entrypoint.ts:24
Claim: The fix only applies when the header is exactly `x-vercel-isr: 1`; any invocation of the `_isr` function without that exact header still 404s.
Consequence: Confirmed in-process: `/_isr?x_astro_path=/one` with no header, or with `x-vercel-isr: true`, returns the 404 page. If Vercel omits the header on some invocations (on-demand revalidation via `bypassToken` / `x-prerender-revalidate`, cache-bypass or draft-mode requests, non-GET requests), those requests keep the 404 this PR is meant to fix. The header is undocumented and only one manual GET was tested.
Fix: —

### Item 5
Location: packages/integrations/vercel/src/serverless/entrypoint.ts:25
Claim: `url.searchParams.get()` form-decodes the path that Vercel substituted via `$0`, so paths containing `+`, `&`, `#`, `%2F` or `%25` are altered before route matching.
Consequence: If Vercel substitutes the raw matched path into `/_isr?x_astro_path=$0`, then `/blog/a+b` becomes `/blog/a b` (param `a%20b`), `/blog/a&b` is truncated to `/blog/a`, and `/blog/a%2Fb` becomes `/blog/a/b`, which matches a different route or 404s. An empty value (`x_astro_path=`) passes the `typeof === 'string'` check and sets the pathname to `/`. Depends on Vercel's substitution encoding, which could not be checked offline.
Fix: —

### Item 6
Location: packages/integrations/vercel/src/serverless/entrypoint.ts:28
Claim: Only `url.pathname` is replaced; the internal `x_astro_path` query parameter stays on the URL handed to `app.match` and `app.render`.
Consequence: On every ISR-served page, `Astro.url` / `request.url` is `/one?x_astro_path=%2Fone`. Canonical URLs, `Astro.url.href`, redirects built from the current URL and code iterating `searchParams` see an internal parameter that the visitor never sent. Fix: `url.searchParams.delete(ASTRO_PATH_PARAM)` before rebuilding the request.
Fix: —

### Item 7
Location: packages/integrations/vercel/src/serverless/entrypoint.ts:22
Claim: Altitude: the ISR case is decided per request from a header inside an entrypoint shared by `_render` and `_isr`, rather than being a build-time property of the `_isr` function.
Consequence: Because `buildISRFolder` reuses the same entry as `buildServerlessFolder`, `_render` inherits ISR-only behavior and the only discriminator is attacker-influenced request data. Baking an ISR flag into the `_isr` bundle (or a separate entry/virtual config value) and validating the path against the ISR route list would remove the header dependency and the `_render` exposure in one change.
Fix: —

### Item 8
Location: packages/integrations/vercel/src/serverless/entrypoint.ts:24
Claim: No regression test accompanies the fix; nothing invokes the built `_isr` handler, and the security test does not cover the new header branch.
Consequence: `test/isr.test.js` only asserts generated config and routes, so the original 404 regression from #15959 went undetected and can recur. `test/path-override-security.test.js` never sends `x-vercel-isr`, so it keeps passing while the override it guards against is reachable again.
Fix: —

### Item 9
Location: packages/integrations/vercel/src/serverless/entrypoint.ts:21
Claim: Cleanup: the new block uses an untyped `let realPath = undefined`, a hardcoded `'x-vercel-isr'` string, and is unformatted (`if(`, missing semicolon).
Consequence: Every other header name in this file is a named constant exported from `../index.js` (`ASTRO_PATH_HEADER`, `ASTRO_MIDDLEWARE_SECRET_HEADER`), so the magic string is inconsistent and unsearchable. `realPath` is implicitly `any`, losing the `string | null` typing the replaced `const` had. The formatting will be rewritten by the next `[ci] format` commit, adding churn.
Fix: —

### Item 10
Location: .changeset/common-cats-travel.md:5
Claim: The changeset text "Fix vercel ISR path rewrite" does not tell users what was broken or that it affects them.
Consequence: Neighbouring changesets use a descriptive present-tense sentence (e.g. "Updates the minimum supported version of VS Code to 1.92.0"). Users whose ISR routes 404 after upgrading cannot tell from the changelog that this release fixes it; something like "Fixes a regression where routes served by ISR returned a 404" would.
Fix: —
