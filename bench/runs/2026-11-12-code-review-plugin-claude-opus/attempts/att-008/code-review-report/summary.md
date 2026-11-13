# Code review — honojs/hono#5067

fix(utils/body,validator): normalize Content-Type media type for case-insensitive matching

Range reviewed: `main...review-head` (9728702911073aec5a63a3ba2840b7240e5d3205..5226d4165d48643586152614cbd07422a0ab7a22)

Step 1: the PR is not a draft and needs review. Step 2: no CLAUDE.md files apply. Step 6 kept 1 issue.

## Issues found: 1

### 1. `parseBody()` breaks when `c.req.formData()` was already called on the request

- **Reason flagged:** bug (behavioral regression)
- **File / lines:** `src/utils/body.ts` lines 126-127, in `parseFormData`
- **Description:** `parseFormData` used to call `request.formData()`. It now always calls `request.arrayBuffer()` and then re-parses the bytes with `bufferToFormData(arrayBuffer, headers.get('Content-Type') || '')`. Suppose a `HonoRequest`'s `bodyCache` already holds only `formData`, for example because a middleware called `await c.req.formData()` before the handler calls `await c.req.parseBody()`. `HonoRequest#cachedBody('arrayBuffer')` (`src/request.ts` lines 228-235) then rebuilds the bytes with `new Response(cachedFormData).arrayBuffer()`, which creates a new multipart body with a fresh, runtime-generated boundary. Those bytes are parsed using the request's original `Content-Type`.
- **Why it fails:**
  - **Multipart requests:** the boundary in the header doesn't match the rebuilt body, so `parseBody()` throws `TypeError: Failed to parse body as FormData.`
  - **`application/x-www-form-urlencoded` requests:** the multipart text is parsed as a query string, so `parseBody()` returns garbage keys such as `{"------formdata-undici-…\r\nContent-Disposition: form-data; name": "\"foo\"\r\n\r\nbar…"}` instead of `{ foo: 'bar' }`.
  - **On the base (`main`):** `parseFormData` called `HonoRequest.formData()`, which returned the cached `FormData`, so both cases returned `{"foo":"bar","baz":"qux"}`.
  - **Confirmed by running both versions:**
    - Head, multipart: `500 TypeError: Failed to parse body as FormData.`
    - Head, urlencoded: `200` with a garbage object.
    - Base, both cases: `200 {"foo":"bar","baz":"qux"}`.
  - **Fix direction:** in `parseFormData`, reuse `request.bodyCache.formData` when it already exists, as `validator('form')` does, before falling back to `arrayBuffer()` and `bufferToFormData`.
