# Review of honojs/hono#5067

Request changes. The media-type matching fix works in the focused suite, but the new form-decoding path introduces a verified read-order regression and duplicates responsibility for the request's body cache. One actionable finding remains.

This review covers `9728702911073aec5a63a3ba2840b7240e5d3205..5226d4165d48643586152614cbd07422a0ab7a22`, inspected through `git diff main...review-head`. It used the frozen thermo-nuclear skill in one primary context, without delegated reviewers, network access, or checkout edits.

## Actionable finding

### [P2] Reuse the cached FormData before decoding the body again

In `src/utils/body.ts:125–132`, `parseFormData()` replaces the existing `request.formData()` call with an unconditional `request.arrayBuffer()` read, then writes a new parsing promise into `HonoRequest.bodyCache.formData` using `as unknown as FormData`. This bypasses the request's existing FormData cache and makes parsing depend on reader order. For an ordinary lowercase multipart request, `await req.formData(); await req.parseBody()` now throws `TypeError: Failed to parse body as FormData.` For a lowercase URL-encoded request, the same sequence returns multipart serialization fragments as fields instead of `{ foo: 'bar' }`. Both sequences return the correct body at the base commit. `HonoRequest.#cachedBody()` reconstructs bytes from the previously cached FormData with a new multipart boundary; this new path then decodes those bytes using the original request's Content-Type. Repeated `parseBody()` calls also decode again and replace the cached FormData, losing its identity and mutations. Keep decoding and cache ownership in `HonoRequest.formData()`, reuse its cached result, and let `parseBody()` only convert FormData into the requested field shape. Normalize the media type at the decoding boundary, preserve parameters, and add awaited regression assertions for both reader orders and repeated calls. Full evidence and a worked restructuring proposal are in [01_body_cache.md](01_body_cache.md).

## Remediation sequence

First restore cache reuse for both form formats; reading an already cached FormData must never serialize it back into bytes and decode it with an old header. An immediate narrow repair can check and reuse `bodyCache.formData` before the new byte-decoding path.

Then consolidate normalization and memoized decoding into `HonoRequest.formData()`, make the cache's promise contract explicit, and remove the cache assignments from `parseBody()` and the form validator. This deletes duplicate decoding orchestration and the new promise-to-FormData double cast. Preserve the validator's existing 400 response for malformed forms and the separate `all`/`dot` conversion behavior.

Add tests that actually await the reads: `formData()` followed by `parseBody()`, `parseBody()` followed by `formData()`, and repeated `parseBody()` calls with different conversion options. Cover multipart and URL-encoded bodies, plus the mixed-case multipart boundary case already exercised here. Rerun the four focused suites after the repair.

## Verification and remaining scope

The permitted focused command passed four files: 144 tests passed and one was skipped. Independent scratch probes against the head reproduced the failures, and a scratch copy of the base body utility confirmed that the same sequences worked before this PR. The request implementation itself is unchanged between base and head. Probes also verified mixed-case parsing, preservation of an uppercase-bearing multipart boundary, subsequent `formData()` after `parseBody()`, and preservation of the original header. These results establish the new cache regression despite the passing suite.

The normalization and validator changes have no additional actionable findings. Their evidence, file-size measurements, and scope limits are recorded in [02_media_types_and_tests.md](02_media_types_and_tests.md). No changed file crosses the 1,000-line threshold; the validator test file was already above it at the base. The unavailable runtime suites were not run. No open questions remain.
