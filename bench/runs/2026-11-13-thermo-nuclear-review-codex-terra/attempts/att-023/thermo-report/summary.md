# Review findings

[P1] Authenticate the ISR path override — `packages/integrations/vercel/src/serverless/entrypoint.ts:24-25`

The new branch accepts `x-vercel-isr: 1` as proof that the query parameter came from Vercel, but this handler also backs `_render` and does not authenticate or otherwise restrict that header to the ISR function. Calling the built `_render` fixture with `/api/public?x_astro_path=/api/private` returns the public endpoint normally and the private endpoint when that header is supplied. If the header reaches the function from a client, a request routed as a public URL can therefore execute a different route, bypassing path-based platform routing, access controls, or ISR exclusions. This restores the untrusted path override that the previous secret check was designed to prevent. Vercel's platform was unavailable for checking whether it strips this header from incoming requests; the in-process handler itself provides no such protection.

[P2] Preserve streamed request bodies when restoring the ISR path — `packages/integrations/vercel/src/serverless/entrypoint.ts:24-25`

Selecting the ISR query path sends body-bearing requests through the existing `new Request` reconstruction at lines 27–33. In Node, `request.body` is a stream and this constructor requires `duplex: 'half'`; without it, it throws before `app.match` or the endpoint runs. Calling the built handler with a POST body, `x-vercel-isr: 1`, and `x_astro_path=/api/public` reproduces `TypeError: RequestInit: duplex option is required when sending a body`. Consequently POSTs and other body-bearing requests reaching an ISR route fail instead of reaching their page or endpoint.

The focused adapter test command did not complete and was interrupted; the behavior above was checked directly against the built fixture handler. Vercel platform routing and header filtering were unavailable.
