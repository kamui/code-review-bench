# Thermo nuclear review — honojs/hono#5067

Scope: `main...review-head` (6 files). The case-insensitive media-type matching itself is fine: the validator regexes gain `/i`, `parseBody` compares the lower-cased `split(';')[0]`, and `bufferToFormData` lowercases only the part before the first `;`, so boundary values are preserved. The related vitest suites pass: 40 files and 981 tests across `src/utils`, `src/validator`, `request`, `hono`, `body-limit` and `helper`. One regression comes from the refactor of `parseFormData`.

## Finding 1 — High: `c.req.parseBody()` fails or returns garbage when `c.req.formData()` was already called

**File:** `src/utils/body.ts`, lines 121-132 (`parseFormData`; the arrayBuffer call is at line 126 and the cache write at line 130).

**Change:** `parseFormData` used to call `request.formData()`. On a `HonoRequest` that goes through `#cachedBody('formData')`, so an already-cached `FormData` was returned unchanged. It now calls `request.arrayBuffer()` and re-parses the bytes with `bufferToFormData(arrayBuffer, <original Content-Type header>)`.

**Trace:**
1. `c.req.formData()` caches `bodyCache.formData = raw.formData()`, and the raw body is consumed.
2. `parseBody()` then calls `request.arrayBuffer()`, which goes through `HonoRequest.#cachedBody('arrayBuffer')` (`src/request.ts` 220-238).
3. `arrayBuffer` is not cached, but `formData` is. The "any cached key" branch therefore runs `new Response(<FormData>).arrayBuffer()`.
4. That re-serializes the FormData as multipart with a new `------formdata-undici-…` boundary.
5. `parseFormData` passes those bytes to `bufferToFormData` together with the request's original `Content-Type` header, whose boundary (or media type) no longer matches the bytes.

**Failures, reproduced with a scratch handler in the clone:**
- **multipart request:** `await c.req.formData(); await c.req.parseBody()` throws `TypeError: Failed to parse body as FormData` ("no boundary found in multipart body"). The result is a 500. On `main`, `parseBody` reused the cached FormData and worked.
- **`application/x-www-form-urlencoded` request:** the same sequence returns HTTP 200 with silently corrupted data. The multipart re-serialization is parsed as urlencoded, giving `{"------formdata-undici-034105017868\r\nContent-Disposition: form-data; name":"\"k\"\r\n\r\nv\r\n------formdata-undici-034105017868--\r\n"}`.

**Who is affected:** any middleware or handler that reads `c.req.formData()` and a later one that calls `c.req.parseBody()`. This is a documented, supported combination, because `parseBody` relies on `bodyCache` for exactly this reuse. The existing test suite does not cover it; the new tests only exercise `parseBody` on a fresh request.

**Related secondary effect:** because `parseFormData` always re-parses from the buffer and then overwrites `bodyCache.formData`, calling `parseBody()` after `validator('form')` (which caches both `arrayBuffer` and `formData`) now does a second full multipart parse. It also replaces the previously cached FormData and its `File` objects instead of reusing them. This is wasted CPU and memory for large uploads, and it breaks object identity between consumers.

**Suggested direction (not applied):** in `parseFormData`, reuse `bodyCache.formData` when it is present, and otherwise build it from the buffered body. Alternatively, keep `request.formData()` and only normalize the Content-Type header. The case-insensitive fix needs neither the re-serialization nor the cache overwrite.

## Other observations (not actionable)
- `src/middleware/method-override/index.ts` (lines 74 and 92) still uses a case-sensitive `startsWith` on the Content-Type. That code is not part of this diff, so it is out of scope.
- The new `parseBody` test replaces the `formData` mock with a real FormData round trip, and it passes.
