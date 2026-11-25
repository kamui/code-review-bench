# Thermo-nuclear review — honojs/hono#5067

Range `9728702911073aec5a63a3ba2840b7240e5d3205..5226d4165d48643586152614cbd07422a0ab7a22` (`main...review-head`), 6 files, 1 commit.

The case-insensitive matching itself is correct: mixed-case urlencoded, multipart and JSON media types are now parsed by `parseBody()` and `validator()`, and the multipart boundary keeps its case. The focused suites pass (`src/utils/body.test.ts`, `src/utils/buffer.test.ts`, `src/validator/validator.test.ts`, `src/request.test.ts`: 144 passed, 1 skipped).

The problems come from the unannounced second change in the commit: `parseFormData()` no longer calls `request.formData()` but reads `request.arrayBuffer()` and re-parses it through `bufferToFormData()`. That rewrite causes one regression in a previously working call sequence and two smaller behaviour changes. All three were reproduced by bundling a scratch script against the head sources and against a `git archive main` copy of the merge-base sources, on Node 24.

## Findings

### 1. `c.req.formData()` followed by `c.req.parseBody()` now fails with a 500 or returns corrupted data

**Priority: High**

**File:** `src/utils/body.ts`, lines 125-132 (`parseFormData`)

**Evidence.** `parseFormData()` now always calls `request.arrayBuffer()` and parses the result with the original `Content-Type` header; it never looks at `request.bodyCache.formData`. When the body was already read with `c.req.formData()`, `bodyCache` holds only the `formData` key, so `HonoRequest.#cachedBody('arrayBuffer')` (`src/request.ts:228-235`) takes the "any cached key" path and returns `new Response(formData).arrayBuffer()`. That re-serialises the `FormData` as multipart with a freshly generated boundary. `parseFormData()` then hands those bytes to `bufferToFormData()` together with the request's original `Content-Type`, which names the old boundary, or names `application/x-www-form-urlencoded`.

On `main` the same sequence worked, because `parseFormData()` called `request.formData()`, which returned the cached `FormData`.

Reproduced with a middleware that does `await c.req.formData()` and a handler that does `await c.req.parseBody()`:

| Request body | `main` | head |
| --- | --- | --- |
| multipart (`message` + a file) | 200, keys `["message","file"]` | 500, `TypeError: Failed to parse body as FormData.` |
| urlencoded (`message=hello`) | 200, `{"message":"hello"}` | 200, single key `"------formdata-undici-055511184086\r\nContent-Disposition: form-data; name"`, no `message` |

**Failure.** Any app where one layer reads `c.req.formData()` (for example a middleware checking a form token or honeypot field) and a later layer calls `c.req.parseBody()` breaks on upgrade. Multipart requests throw and become a 500. Urlencoded requests are worse: `parseBody()` succeeds and returns an object whose only key is multipart framing text, so handlers silently see every field as missing. Reading the body several times through `HonoRequest` is what `bodyCache` exists for, and the reverse order (`parseBody()` then `formData()`) still works, so this is an order-dependent regression that the new tests do not cover.

The same root cause means that `parseBody()` no longer reuses an already parsed `FormData`. Calling `c.req.parseBody()` twice, or after `validator('form', ...)`, re-parses the whole body each time, and for multipart the second call returns different `File` instances than the first (`a.file === b.file` was `true` on `main`, is `false` on head).

**Suggested direction.** Reuse `request.bodyCache.formData` when it is set, as `validator()` already does at `src/validator/validator.ts:115-116`, and fall back to the array-buffer path only otherwise. A regression test for `formData()` then `parseBody()` on both body types would pin it.

### 2. `c.req.parseBody()` now keeps the raw body in memory alongside the parsed `FormData`

**Priority: Low**

**File:** `src/utils/body.ts`, lines 126-131 (`parseFormData`)

**Evidence.** For a `HonoRequest`, `request.arrayBuffer()` stores the full raw body in `bodyCache.arrayBuffer`, and the new code then also stores the parsed `FormData` in `bodyCache.formData`. After `parseBody()` on head `Object.keys(c.req.bodyCache)` is `["arrayBuffer","formData"]`; on `main` it is `["formData"]`. Measured with a 64 MiB file upload and a forced GC inside the handler: `process.memoryUsage().arrayBuffers` is 256 MB on head versus 192 MB on `main`, a difference of exactly one extra copy of the body held for the life of the request.

**Failure.** Upload handlers that use `c.req.parseBody()` hold roughly twice the upload size per in-flight request (raw bytes plus the `File` contents). On memory-capped runtimes this lowers the largest upload that fits and the number of concurrent uploads a worker survives. I could only measure this on Node; the Workers, Bun and Deno suites are unavailable here, so the effect on runtimes whose native `request.formData()` avoids full buffering is not measured. `validator('form')` already had this cost; this change extends it to every `parseBody()` caller.

### 3. A comma-joined duplicate `Content-Type` header is no longer parsed and silently yields `{}`

**Priority: Low**

**File:** `src/utils/body.ts`, lines 104-106 (`parseBody`)

**Evidence.** The guard changed from `contentType.startsWith(...)` to an exact comparison of `contentType.split(';')[0].trim().toLowerCase()`. When a request carries two `Content-Type` header lines, `Headers.get()` returns them joined, for example `application/x-www-form-urlencoded, application/x-www-form-urlencoded`. The part before the first `;` is then the whole joined string and matches neither literal. Reproduced with a plain `Request` and body `a=1`: `main` returns `{"a":"1"}`, head returns `{}`.

**Failure.** Requests whose `Content-Type` was duplicated by a client or intermediary used to be parsed and now reach handlers as an empty body with no error. This is rare, and the stricter match also has a benefit (a value such as `application/x-www-form-urlencoded-foo` used to reach `request.formData()` and throw, and now returns `{}`), so it is a small compatibility note rather than a blocker.

## Checked and found sound

- **CSRF interplay.** `src/middleware/csrf/index.ts:30` already matches form content types with the `i` flag, so accepting mixed-case media types in `parseBody()` and `validator()` does not open a path around the CSRF middleware.
- **Boundary handling.** `bufferToFormData()` lowercases only the text before the first `;`. `Multipart/Form-Data; BOUNDARY=AbC` and a quoted mixed-case boundary both parse correctly on head.
- **Validator regexes.** Adding the `i` flag does not change the structure of the three patterns, so there is no new backtracking risk, and `validator('form')` followed by `parseBody()` still works because `bodyCache.arrayBuffer` is populated first.
- **Other call orders improved.** `c.req.text()` or `c.req.arrayBuffer()` followed by `c.req.parseBody()` threw on `main` and works on head. `cloneRawRequest()` after `parseBody()` failed for multipart and returned garbage for urlencoded on `main`, and is correct on head.
- **Unchanged failure modes.** A body consumed directly through `c.req.raw`, multipart without a boundary, an empty multipart body, and a `bodyLimit` overflow behave the same on both sides.
- **Modified test.** The "should not update file object properties" test in `src/utils/body.test.ts` drops its `req.formData` mock because that method is no longer called; the real-`FormData` version still exercises the same assertion.

No devex-breaking or feature-gate changes are in this diff.
