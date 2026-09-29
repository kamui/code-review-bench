# Detail 01 — `src/utils/body.ts`, `src/utils/buffer.ts` (parseBody path)

Review range: `9728702911073aec5a63a3ba2840b7240e5d3205..5226d4165d48643586152614cbd07422a0ab7a22` (`git diff main...review-head`).
Model: claude-sonnet-5-5 at high, single reviewer, no delegation.

## Finding 1 (verified regression): `parseFormData` no longer reuses the HonoRequest body cache, so `c.req.formData()` followed by `c.req.parseBody()` breaks

Location: `src/utils/body.ts:121-138` (the rewritten `parseFormData`), interacting with `HonoRequest.#cachedBody` in `src/request.ts:220-239`.

Before the change, `parseFormData` called `request.formData()`. On a `HonoRequest` that goes through `#cachedBody('formData')`, which returns the cached FormData when it already exists. After the change, `parseFormData` calls `request.arrayBuffer()` and then `bufferToFormData(arrayBuffer, contentType)`. When only `bodyCache.formData` is populated (for example a handler that already called `c.req.formData()`, and the raw body is consumed), `#cachedBody('arrayBuffer')` takes the "any cached key" branch and rebuilds the buffer with `new Response(formDataObject).arrayBuffer()`. That serialization is always multipart with a freshly generated boundary. The code then parses those bytes with the original request `Content-Type`, whose boundary (or `application/x-www-form-urlencoded` type) no longer matches.

Measured with a scratch script (bundled with esbuild, run with node 24), calling `await c.req.formData()` and then `await c.req.parseBody()` in the same handler:

| Request | `main` result of `parseBody()` | Head result of `parseBody()` |
| --- | --- | --- |
| urlencoded `x=1&y=2` | `{"x":"1","y":"2"}` | garbage: one key that is the multipart-encoded text, e.g. `{"------formdata-undici-...\r\nContent-Disposition: form-data; name":"\"x\"\r\n\r\n1..."}` |
| multipart, one field `x` | `{"x":"1"}` | throws `Failed to parse body as FormData.` |

Scripts: `clone-work/t1.ts` (head) and `clone-work/t0.ts` (an unmodified `git archive main` copy). The existing unit suites (`body.test.ts`, `buffer.test.ts`, `validator.test.ts`, `request.test.ts`) all pass on head (144 passed, 1 skipped), so the regression is not covered by any test. No new test covers "cache populated by `formData()` then `parseBody()`".

A second, smaller consequence: `parseBody()` called twice on the same `HonoRequest` used to return values derived from a single cached FormData. Now the second call finds `bodyCache.arrayBuffer` and re-parses, so it builds a fresh FormData (and fresh `File` objects) each time. It also overwrites `bodyCache.formData` on every call.

Remedy: do not change the acquisition path at all (see Finding 2). If the buffer-based path is retained, it has to go through the same cache-aware accessor for both `formData` and `arrayBuffer`, and it has to check `bodyCache.formData` first, the way `validator.ts:115` does.

## Finding 2 (missed simplification): the body-acquisition rewrite is not needed to fix issue #5060

The issue is about the *guard* (`startsWith` on lower-case literals). The platform already parses mixed-case media types: on Node 24 (undici), `new Request(url, {body, headers: {'Content-Type': 'Multipart/Form-Data; boundary=...'}}).formData()` and the urlencoded equivalent both parse correctly (`clone-work/probe.mjs`). The repository's own tests exercise Node. So the smallest correct fix for `parseBody` is the two-line guard change at `src/utils/body.ts:104-106`, leaving `parseFormData` as `await request.formData()`.

Instead the PR also:
- rewrites `parseFormData` to fetch an ArrayBuffer and re-wrap it in a `Response` inside `bufferToFormData` (an extra buffer copy plus an extra `Response` for every form parse, replacing the native streaming/cached path);
- reaches into `HonoRequest.bodyCache` from a utility that is meant to accept a plain `Request` too (`if (request instanceof HonoRequest) { request.bodyCache.formData = ... }`), which leaks the request class's private caching contract into `utils/body.ts`;
- stores a *promise* in a slot typed as `FormData` via `as unknown as FormData` (`body.ts:130`). `validator.ts:121` stores the resolved value in the same slot, while `#cachedBody` stores promises. The slot now holds three different conventions;
- forces the test at `body.test.ts:86-95` to stop mocking `req.formData` (the mock no longer applies) and instead build a real FormData round trip. Needing to rewrite an unrelated test to keep it passing is a signal that the change altered the seam.

If cross-runtime concerns (older Bun/Deno/Workers not lower-casing the essence) motivated the rewrite, that should be stated in the PR and covered by a runtime test. The PR description gives no such rationale, and the runtime-test suites were not available in this review, so that is unverified. Even then, the right place is one shared helper (see Finding 3), not a special path inside `parseFormData`.

Worked code-judo proposal for `body.ts`:

```ts
// body.ts — guard only
const mediaType = getMediaType(contentType)          // shared helper, see Finding 3
if (mediaType === 'multipart/form-data' || mediaType === 'application/x-www-form-urlencoded') {
  return parseFormData(request, { all, dot })
}

async function parseFormData(request, options) {
  const formData = await (request as Request).formData()   // unchanged
  ...
}
```

This deletes the `headers` re-read, the `arrayBuffer` hop, the `bodyCache` write, the cast, the `bufferToFormData` import, and the mock removal in the test. Net diff for `body.ts` drops from +12/−5 to about +2/−4.

## Finding 3 (duplication): three different media-type normalization mechanisms across the diff

- `src/utils/body.ts:104`: `contentType?.split(';')[0].trim().toLowerCase()`, then string equality.
- `src/validator/validator.ts:24-26`: `/i` appended to three hand-written regexes.
- `src/utils/buffer.ts:113`: `contentType.replace(/^[^;]+/, (m) => m.toLowerCase())`.

There is no shared "get lower-cased media type of a Content-Type header" helper even though `src/utils/` is the natural home. Each site re-derives "the part before `;`" differently (`split`, regex anchor, `replace`). Behavior also differs between them: `parseBody` is now an exact-equality match (so `multipart/form-data-x` stops matching, where it used to), while the validator remains a whole-header anchored regex (a `multipart/form-data; charset=utf-8` header is still rejected by `multipartRegex`, and `/i` also silently makes the `boundary=` parameter *name* and every parameter name case-insensitive). The two guards now answer the same question with different rules.

Remedy: add one `getMediaType(contentType: string | null | undefined): string` (lower-cased, trimmed, parameters stripped) next to the other header helpers and use it in `body.ts` and in the `validator.ts` switch, replacing the regexes with set membership (`json` needs `application/json` or `application/*+json`, which is a one-line check on the returned string). That leaves `buffer.ts` untouched.

## Finding 4 (buffer.ts): the normalization in `bufferToFormData` is redundant on the tested runtime

`src/utils/buffer.ts:113` lower-cases the media type before handing it to `new Response(...).formData()`, which already does that per the Fetch spec's MIME type parsing. The new `buffer.test.ts` case ("Should parse mixed-case multipart/form-data media type while keeping the boundary") would pass on `main` under Node as well. It therefore does not demonstrate the need for the change. Either delete the normalization and the test, or add a comment naming the runtime where the native path fails. This is a lower-priority follow-on to Finding 2.

## Coverage note

Codecov reports 4 uncovered patch lines (3 in `body.ts`, 1 in `buffer.ts`). Those are most likely the `formData()`-cache branch and the `|| ''` fallback (`headers.get('Content-Type') || ''` at `body.ts:127` is unreachable because the guard already required a non-null value) — the same code that carries Finding 1.

## Verification status

- Finding 1: verified by execution, head vs. an extracted copy of `main` (outputs above).
- Finding 2: Node behavior verified; behavior on other runtimes not verified (runtime-test suites unavailable).
- Findings 3 and 4: verified by reading the diff; Finding 4's "passes on main" claim rests on the Node probe, not on running the new test against `main`.
