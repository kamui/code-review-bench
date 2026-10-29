# Thermo Nuclear Review — honojs/hono#5067

Range reviewed: `9728702911073aec5a63a3ba2840b7240e5d3205..5226d4165d48643586152614cbd07422a0ab7a22` (`git diff main...review-head`), 6 files.

The PR's stated goal (case-insensitive media-type matching in `parseBody()` and `validator()`) is achieved. The `validator.ts` regex `/i` flags, the `mediaType` extraction in `parseBody()`, and the media-type lowercasing in `bufferToFormData()` are all correct, keep multipart boundaries and other parameters intact, and are covered by the new tests. The changed test files and `src/request.test.ts` pass (144 passed, 1 skipped).

However, the PR also rewrote `parseFormData()` in `src/utils/body.ts` to consume the body through `arrayBuffer()` + `bufferToFormData()` instead of `request.formData()`. That rewrite introduces a functional regression in `HonoRequest` body-cache handling that is not covered by any test. One actionable finding below, plus one low-priority note.

## Finding 1 — High: `parseBody()` after `c.req.formData()` now throws (multipart) or returns garbage (urlencoded)

**File/lines:** `src/utils/body.ts:125-132` (new `parseFormData` body), in combination with the existing `HonoRequest.#cachedBody` at `src/request.ts:220-239`.

**Evidence traced.** Before this PR, `parseFormData()` called `(request as Request).formData()`. For a `HonoRequest` that resolves to `HonoRequest.formData()` → `#cachedBody('formData')`, which returns the already-cached `FormData` promise if `c.req.formData()` had been called earlier. After this PR, `parseFormData()` unconditionally calls `(request as Request).arrayBuffer()` → `#cachedBody('arrayBuffer')`. When `bodyCache.arrayBuffer` is empty but `bodyCache.formData` is populated (because `c.req.formData()` ran first), `#cachedBody` falls into its "any cached key" branch and does `new Response(formData).arrayBuffer()`. That re-serializes the `FormData` as a brand-new multipart document with a freshly generated boundary (`------formdata-undici-…`). `parseFormData()` then passes those bytes to `bufferToFormData(arrayBuffer, headers.get('Content-Type'))`, i.e. parses them under the *original* request Content-Type:

- Original `multipart/form-data; boundary=<old>` → the new body does not contain `<old>` → undici throws `TypeError: Failed to parse body as FormData` (`no boundary found in multipart body`). `parseBody()` rejects and an app handler returns `500 Internal Server Error`.
- Original `application/x-www-form-urlencoded` → the multipart bytes are parsed as a URL-encoded string → `parseBody()` resolves successfully with a garbage object such as `{"------formdata-undici-0943…\r\nContent-Disposition: form-data; name": "\"message\"\r\n\r\nhello\r\n------formdata-undici-0943…--\r\n"}`. No error is raised; the handler silently receives wrong data.

Additionally, line 130 overwrites `request.bodyCache.formData` with the new (rejected) promise before awaiting it, so after one failed `parseBody()` a subsequent `c.req.formData()` — which used to work — also throws.

I verified this by bundling a scratch script against the head and against the merge-base source tree (extracted with `git archive main src`, clone untouched):

```
# head (5226d41)
multipart:  formData() then parseBody()              -> THREW: Failed to parse body as FormData.
urlencoded: formData() then parseBody()              -> {"------formdata-undici-0943…\r\nContent-Disposition: form-data; name":"\"message\"\r\n\r\nhello\r\n…"}
multipart:  formData(), parseBody(), formData() again -> THREW: Failed to parse body as FormData.
app: middleware c.req.formData() → handler c.req.parseBody() (multipart)  -> 500 Internal Server Error
app: middleware c.req.formData() → handler c.req.parseBody() (urlencoded) -> 200 with garbage body

# merge-base (9728702)
multipart:  formData() then parseBody()              -> {"message":"hello"}
urlencoded: formData() then parseBody()              -> {"message":"hello"}
multipart:  formData(), parseBody(), formData() again -> {"firstGet":"hello","secondGet":"hello"}
app: middleware formData() → handler parseBody() (multipart)  -> 200 {"message":"hello"}
app: middleware formData() → handler parseBody() (urlencoded) -> 200 {"message":"hello"}
```

Orderings that still work on the head (verified): `parseBody()` → `formData()`/`text()`, `parseBody()` twice, `blob()` → `parseBody()`, `arrayBuffer()` → `parseBody()`, and `validator('form')` → `parseBody()` (the validator caches `arrayBuffer` first, so the new code finds the original bytes). Some orderings actually improve (`text()` → `parseBody()` and `arrayBuffer()` → `parseBody()` used to throw and now work), but that does not offset breaking the previously supported `formData()` → `parseBody()` order.

**Failure it causes.** Any application where `c.req.formData()` is consumed before `c.req.parseBody()` on the same request — e.g. a middleware (auth/CSRF/logging) that reads a form field via `c.req.formData()` and a handler that later calls `c.req.parseBody()`, or a handler that uses both APIs — regresses from working to either a 500 (multipart) or silently corrupted parsed data (urlencoded). This is a public, documented API (`c.req.parseBody()`), and the urlencoded case fails without any error, which is why I rate it High rather than Medium. It is a correctness/compat regression, not a security vulnerability. No new test exercises this ordering, so CI does not catch it.

**Suggested direction (not applied).** Mirror the validator's logic: in `parseFormData()`, if `request instanceof HonoRequest && request.bodyCache.formData` is set, `await` that cached value instead of re-reading `arrayBuffer()`; only fall through to `arrayBuffer()` + `bufferToFormData()` when no `formData` is cached. Add a regression test for `formData()` followed by `parseBody()` for both media types.

## Note — Low: body bytes now retained for the request lifetime

**File/lines:** `src/utils/body.ts:126`.

For a `HonoRequest`, the new `parseFormData()` populates `bodyCache.arrayBuffer` with the full raw body and separately keeps the parsed `FormData` (which holds `File` contents). Before the PR only the `FormData` was retained. For large multipart uploads this roughly doubles memory retained per request after `parseBody()`. The validator already has the same characteristic, so this is a parity change rather than a new pattern, and I am not raising it as an actionable finding — just flagging it as a side effect the author may not have intended.

## Checked and found no issue

- `jsonRegex`/`multipartRegex`/`urlencodedRegex` with `/i`: boundary charset and parameter syntax unchanged; mixed-case `application/JSON`, `+json` suffixes, and `charset` parameters all match as intended.
- `parseBody()` media-type extraction (`split(';')[0].trim().toLowerCase()`): equivalent to the old `startsWith` for all well-formed values; slightly stricter for bogus values like `multipart/form-datax`, which is desirable.
- `bufferToFormData()` lowercases only the media-type portion before `;`; boundary and charset parameters are preserved (test added, verified).
- Raw `Request` path of `parseBody()` (e.g. `method-override` middleware uses it on a cloned raw request): `arrayBuffer()` + `Response.formData()` is behaviourally equivalent to `Request.formData()`; body-consumed semantics unchanged.
- Removal of the `vi.spyOn(req, 'formData')` mock in `body.test.ts`: the test now uses a real `FormData`; `vi` is not referenced elsewhere in that file, so no unused-import/lint issue.
- No feature-gate, devex, env-var, port, or secret changes in the diff.
- Clone tree verified unchanged after review (`git status --porcelain` empty).
