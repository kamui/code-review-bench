# Review of withastro/astro#16079

Request changes for one shared-handler boundary regression. The intended ISR repair works in-process: `/_isr?x_astro_path=/one` with `x-vercel-isr: 1` renders the One page instead of returning 404. However, the exception also enables query-driven route substitution in the ordinary serverless handler, including deployments with ISR disabled. Production exploitability depends on Vercel ingress behavior, which was unavailable for this review.

Reviewed `b089b904f1ed578e9edaefd129bf9843120a808f..71ae513388df11d7dad6b1e0077c402ad03d0d62`, using `git diff main...review-head`. The review used the frozen thermo-nuclear-code-quality-review skill and one primary reviewer context. No other reviewers, ambient repository instructions, upstream discussions, or network resources were used.

## Finding: Confine ISR path restoration to the ISR boundary

In `packages/integrations/vercel/src/serverless/entrypoint.ts:24–25`, the new `x-vercel-isr === '1'` branch lets a request header authorize `x_astro_path` rewriting in every instance of the shared handler, without a valid middleware secret or an ISR-specific execution context. Calling the built `_render` handler for the fixture with ISR disabled at `/api/public?x_astro_path=/api/private` returns the fixture response `{"id":"private"}` when that header is present, whereas the same request without it returns `{"id":"public"}`; an invalid middleware secret does not prevent the rewrite. This leaks ISR transport policy into ordinary route matching and replaces the previous authenticated rewrite boundary with an implicit platform-header trust assumption. Bind query-based restoration to the generated ISR entry, preserve authenticated middleware forwarding in the ordinary handler, and explicitly establish the provenance of any remaining request-controlled rewrite signal. Add handler tests covering the successful ISR request and the combined header/query attack on `_render`, including an ISR-disabled build. This is a reproduced adapter-boundary regression; it is not a verified Vercel deployment exploit. [Full evidence and worked restructuring](01_isr_request_boundary.md).

## Open question

Does Vercel strip or overwrite a client-supplied `x-vercel-isr` header before invoking both `_render` and `_isr`, including excluded routes and direct function paths? The checkout provides no such guarantee, and the offline execution policy prevents checking it. Establish this contract before treating the header as trusted routing metadata; the in-process route substitution alone does not establish an edge-middleware or firewall bypass in production.

## Remediation sequence

First, establish the platform's routing-metadata trust contract and move ISR restoration to the function boundary that already owns ISR deployment configuration. The detail report works through an ISR-specific generated entry that delegates to the ordinary renderer, so ordinary requests no longer discover their execution mode from a header. Merely extracting the current conditional into a helper would preserve the defect.

Then add request-level regression cases to the existing fixture tests: a genuine ISR regeneration request must render `/one`, while `_render` must continue rendering `/api/public` when presented with both `x_astro_path=/api/private` and `x-vercel-isr: 1`. Cover an invalid middleware secret and preserve valid middleware-header precedence. Re-run the focused adapter tests after implementing that boundary.

## Verification and quality assessment

The focused command `node --test test/path-override-security.test.js test/isr.test.js` passed all four tests. None exercises the new header/query combination; the ISR suite checks generated configuration rather than rendering. A separate scratch script imported the built fixture handlers and checked nine request cases, reproducing both the intended repair and the ordinary-handler substitution. The private/public labels are fixture route parameters, not evidence of access to protected data.

The runtime file grows from 66 to 72 lines. There is no 1,000-line threshold crossing, new cast, duplicated utility, unnecessary asynchronous serialization, or unrelated decomposition requirement. The five-line changeset correctly requests a patch release for `@astrojs/vercel`. Formatting and the temporary variable's nullable states do not warrant separate findings. The material concern is ownership and trust of path restoration. No remedies were applied; the tracked checkout remained unchanged. See [the subsystem report](01_isr_request_boundary.md) for measurements, reproduction, and verification limits.
