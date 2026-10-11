# Body parsing and media-type normalization

## Scope and evidence

The changed path is `src/utils/body.ts:95-138`, with the shared `bufferToFormData()` helper in `src/utils/buffer.ts:106-116`. `parseBody()` obtains the request headers, extracts the text before the first semicolon, trims it, and lowercases that media type before comparing against its two supported form types. Parameter values are therefore excluded from normalization.

`parseFormData()` now reads the request bytes, calls the existing `bufferToFormData()` helper with the original `Content-Type`, and for a `HonoRequest` places the resulting promise in `bodyCache.formData`. The helper lowercases only the prefix before `;` when constructing the `Response`; it does not rewrite the multipart boundary. The `HonoRequest` cache is also used by `validator('form')` and by the request's body methods, so the caching behavior is in the established request-body mechanism rather than a separate cache abstraction.

The patch adds mixed-case URL-encoded and multipart tests in `src/utils/body.test.ts`, plus a helper-level multipart test using a mixed-case boundary in `src/utils/buffer.test.ts`. The pre-existing test that mocked `formData()` was changed to use an actual `FormData` request; the replacement continues to assert the same dot-notation behavior while avoiding a mock that no longer fits the byte-oriented parser path.

## Structural assessment

This change adds several lines to `parseFormData()` and an import, but it does not push the module toward a size boundary: `src/utils/body.ts` is 240 lines after the patch. The new code reuses `bufferToFormData()` rather than introducing a second MIME parser. The split at `;` is direct and preserves the parameter string, which matters for boundary matching. No extra mode flag, conditional chain, or wrapper was introduced.

The cache assignment at `src/utils/body.ts:130` casts the parse promise through `unknown` because `BodyCache` in `src/request.ts` is currently typed as `Partial<Body>`, whose entries are resolved body values. This is a type-shape mismatch and worth recognizing, but the cache consumers await entries and the same cache also already accepts resolved form data from the validator. In this patch, retaining the promise lets later consumers reuse the in-flight parse and avoids parsing a consumed stream again. That tradeoff is local and comprehensible; a repo-wide cache type redesign would be broader than this PR and is not necessary to resolve the media-type behavior.

## Code-judo check

A smaller-looking alternative would be to normalize only the `parseBody()` guard and continue calling `request.formData()`. That would remove the byte conversion and cache assignment. It would also leave this entry point using the platform's direct form parser while the validator uses the shared `bufferToFormData()` path, and would not record the parsed result on `HonoRequest` for subsequent body consumers. Given that the existing helper already owns buffered form parsing and is used by the validator, the PR's reuse of that helper is the more consistent boundary. I do not see a simpler restructuring that preserves the shared parse behavior and the newly populated cache.

## Verification status

The focused command `./node_modules/.bin/vitest --run --project main --coverage.enabled=false src/utils/body.test.ts src/utils/buffer.test.ts src/validator/validator.test.ts` passed: 3 files, 106 tests passed, 1 skipped. `git diff --check main...review-head` passed. Runtime-specific suites for Deno, Bun, Workers, Lambda, and other platforms were unavailable under the supplied policy; no claim about those suites is made.

## Finding status and remediation

No actionable finding. No code change is recommended from this subsystem review.
