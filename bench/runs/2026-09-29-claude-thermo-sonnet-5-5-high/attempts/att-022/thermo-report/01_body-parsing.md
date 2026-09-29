# Detail 01: src/utils/body.ts and HonoRequest body cache

Range reviewed: 9728702..5226d41 (`git diff main...review-head`).

## Finding A: parseFormData was rewritten to read arrayBuffer and hand-seed bodyCache, which regresses formData() then parseBody()

Evidence: `src/utils/body.ts:121-132`. Before the PR, `parseFormData` called `request.formData()`. On a `HonoRequest` that goes through `#cachedBody('formData')` (`src/request.ts:220-239`), which reuses any cached form data. After the PR it calls `request.arrayBuffer()`, then `bufferToFormData(...)`, then writes `request.bodyCache.formData = formDataPromise as unknown as FormData`.

Why it is a problem: `#cachedBody('arrayBuffer')` looks for a cached `arrayBuffer` first. If none exists but a `formData` entry does, it rebuilds the buffer with `new Response(cachedFormData).arrayBuffer()`. That produces a new multipart boundary. `parseFormData` then parses the new bytes with the original request `Content-Type`, whose boundary no longer matches, so the parse fails.

Verification (executed): scratch app bundled with esbuild, run against `review-head` and then against `main`, with the clone restored to `review-head` afterwards (`git status` clean).

- Handler: `await c.req.formData(); return c.json(await c.req.parseBody())` with a multipart POST.
- `main`: 200 `{"a":"1"}`.
- `review-head`: 500 `TypeError: Failed to parse body as FormData.`
- `parseBody()` twice followed by `formData()` still works on both.

Root cause of the detour: the issue (#5060) says the platform `Request` already parses mixed-case media types. The only bug was the guard in `parseBody` (and the validator regexes). Nothing required replacing `request.formData()`.

Cast smell: `formDataPromise as unknown as FormData` exists only to smuggle a Promise into a field typed `FormData`. The validator stores a plain `FormData`, so the two writers of `bodyCache.formData` now disagree on shape. A cache write from a free function into `HonoRequest` internals (`instanceof` branch, public mutable `bodyCache`) is a boundary leak.

Test fallout: the existing test that mocked `req.formData` (the `dot` option test) had to be rewritten to use real `FormData`, because the mock no longer intercepts anything. That is a signal that the implementation coupling changed for no behavioral reason.

### Code-judo proposal

Keep `parseFormData` exactly as it was (`await (request as Request).formData()`). Change only the guard:

```ts
const mediaType = contentType?.split(';')[0].trim().toLowerCase()
if (mediaType === 'multipart/form-data' || mediaType === 'application/x-www-form-urlencoded') {
  return parseFormData(request, { all, dot })
}
```

That deletes the `bufferToFormData` import, the `instanceof` cache write, the cast, the second header lookup, and the test rewrite. If some runtime truly cannot parse mixed-case `formData()` natively, put the arrayBuffer path behind `HonoRequest.formData()` itself (the canonical cache owner) rather than in `utils/body.ts`, and store a consistently typed value.

### Related duplication

`parseBody` (`body.ts:101-104`) computes the header lookup twice (once in `parseBody`, again in `parseFormData`). Media-type extraction is implemented three ways across the PR: `split(';')[0].trim().toLowerCase()` in body.ts, `replace(/^[^;]+/, ...)` in buffer.ts, and `/i` regexes in validator.ts. See detail 02.

## Test coverage

Codecov flagged 3 uncovered patch lines in body.ts. Nothing tests `formData()` followed by `parseBody()`, the interaction that breaks.
