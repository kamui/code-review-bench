# Review blind-d0574e

### Item 1
Location: packages/integrations/vercel/src/serverless/entrypoint.ts:24
Claim: The path override is gated on a client-suppliable `x-vercel-isr: 1` header, so the untrusted `x_astro_path` override that #15959 removed is reachable again on `_render`.
Consequence: Verified in-process against the built `_render` handler: `GET https://example.com/api/public?x_astro_path=/api/private` with header `x-vercel-isr: 1` returns 200 `{"id":"private"}` instead of `public`. Any client that can get the header through to the function renders an arbitrary route under a different public URL, defeating path-based firewall or middleware rules. The existing path-override-security test still passes only because it never sends the header.
Fix: —

### Item 2
Location: packages/integrations/vercel/src/serverless/entrypoint.ts:25
Claim: The ISR function trusts whatever `x_astro_path` it is given, so a direct request to `/_isr?x_astro_path=<any route>` renders and caches routes that were never meant to go through ISR.
Consequence: Project with `isr: { exclude: ['/dashboard'] }` and edge middleware doing auth: an attacker requests `/_isr?x_astro_path=/dashboard`. If Vercel invokes the ISR function for that URL with `x-vercel-isr: 1`, the entrypoint rewrites the pathname to `/dashboard` and renders it without the edge middleware, and the response is stored in the ISR cache keyed by `x_astro_path`. No header spoofing is needed. Not reproducible here because the platform is unavailable.
Fix: —

### Item 3
Location: packages/integrations/vercel/src/serverless/entrypoint.ts:29
Claim: The rewritten Request passes `body: request.body` without `duplex: 'half'`, so any ISR-routed request with a body throws now that the rewrite branch runs for ISR again.
Consequence: Verified in-process: `POST /_isr?x_astro_path=/api/public` with a body and `x-vercel-isr: 1` throws `TypeError: RequestInit: duplex option is required when sending a body.` A form POST or API POST to a non-excluded on-demand route gets a 500 instead of reaching the route handler.
Fix: —

### Item 4
Location: packages/integrations/vercel/src/serverless/entrypoint.ts:25
Claim: The real path is recovered with `searchParams.get`, which form-decodes the raw `$0` substitution, so paths containing `+`, `&`, `#` or percent-escapes are altered or truncated.
Consequence: Verified in-process: `x_astro_path=/api/a+b` renders with `params.id === 'a b'`, and `x_astro_path=/api/a&b=c` renders with `params.id === 'a'`. If Vercel substitutes `$0` unencoded, a request to `/blog/c++-tips` or `/tags/r&d` on an ISR route renders the wrong page or 404s, and that wrong response is cached.
Fix: —

### Item 5
Location: packages/integrations/vercel/src/serverless/entrypoint.ts:24
Claim: ISR detection depends on a Vercel header that is not referenced anywhere else in the adapter, its docs or its tests, with no fallback when it is absent or has a different value.
Consequence: If Vercel omits `x-vercel-isr` or sends another value on some invocation types (background revalidation, on-demand revalidation via `x-prerender-revalidate` bypass token, preview deployments), `realPath` stays undefined, the request is matched as `/_isr`, and the 404 page is rendered and cached for that route. This is the same regression the PR set out to fix.
Fix: —

### Item 6
Location: packages/integrations/vercel/src/serverless/entrypoint.ts:28
Claim: Only `url.pathname` is rewritten, so the internal `x_astro_path` parameter stays in the request URL that user code sees.
Consequence: Verified: the rewritten URL for `/one` is `https://host/one?x_astro_path=/one`. On every ISR page `Astro.url.search` and `Astro.url.href` contain the internal parameter, so canonical links, `og:url`, pagination links or redirects built from `Astro.url` leak `?x_astro_path=...` into cached HTML.
Fix: —

### Item 7
Location: packages/integrations/vercel/src/serverless/entrypoint.ts:24
Claim: No regression or security test accompanies the change: nothing exercises the ISR rewrite, and nothing asserts that `_render` ignores `x_astro_path` when `x-vercel-isr` is present.
Consequence: `test/isr.test.js` only checks generated config and routes and never calls the handler, and `test/path-override-security.test.js` omits the new header. Both the original 404 regression and the reopened override can recur with a green test suite.
Fix: —

### Item 8
Location: packages/integrations/vercel/src/serverless/entrypoint.ts:22
Claim: Altitude: `_render` and `_isr` are built from the same entry file, so the fix distinguishes them at runtime by a request header instead of by build-time identity.
Consequence: Because `buildServerlessFolder` and `buildISRFolder` both use `entryFile`, the ISR-only branch also ships in `_render`, which is what makes the header spoof possible. Baking an ISR flag into the `_isr` function at build time (a separate entry or a virtual-config value), and validating `x_astro_path` against the ISR route list, would fix the 404 without adding a trust decision on request data. The `'x-vercel-isr'` literal is also inlined rather than exported as a named constant like the other `ASTRO_*` headers.
Fix: —

### Item 9
Location: packages/integrations/vercel/src/serverless/entrypoint.ts:23
Claim: In the middleware branch of the touched function, `x-astro-path` carries path plus query string, and assigning it to `url.pathname` percent-encodes the `?` and loses the query.
Consequence: The edge middleware sends `x-astro-path: /api/a?q=1` (`request.url.replace(origin, '')`). Verified that `url.pathname = '/api/a?q=1'` yields `https://host/api/a%3Fq=1`, and the handler then renders with `params.id === 'a%3Fq=1'` and an empty query. Requests with a query string through edge middleware match the wrong route or parameter. This predates the PR and is left in place by the rewrite of this block.
Fix: —

### Item 10
Location: packages/integrations/vercel/src/serverless/entrypoint.ts:21
Claim: `let realPath = undefined;` drops the previous `string | null` const typing, and the new block is unformatted (`if(`, missing semicolon on line 23); the changeset text is also vague.
Consequence: Maintenance cost only: the variable is an untyped evolving `let`, so a later non-string assignment would not be caught by the compiler, and the formatting will be rewritten by the repo's format CI job. The changeset `Fix vercel ISR path rewrite` does not tell users that it fixes ISR pages returning 404 since the previous release.
Fix: —
