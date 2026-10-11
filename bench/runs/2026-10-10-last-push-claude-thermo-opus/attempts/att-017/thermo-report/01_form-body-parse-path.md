# 01 — Form body parse path (`parseFormData`, `bufferToFormData`, `bodyCache`)

Covers summary findings F1, F2 and F3.

## What changed

At the merge-base, `parseFormData()` in `src/utils/body.ts` was:

```ts
const formData = await (request as Request).formData()
```

For a `HonoRequest`, `.formData()` is `#cachedBody('formData')` (`src/request.ts:220-239`): return the cached entry if there is one, otherwise convert from whatever else is cached, otherwise call `raw.formData()` and cache the promise.

At head (`src/utils/body.ts:125-132`):

```ts
const headers = request instanceof HonoRequest ? request.raw.headers : request.headers
const arrayBuffer = await (request as Request).arrayBuffer()
const formDataPromise = bufferToFormData(arrayBuffer, headers.get('Content-Type') || '')
if (request instanceof HonoRequest) {
  // Cache so that a later `c.req.formData()` reuses the already-consumed body
  request.bodyCache.formData = formDataPromise as unknown as FormData
}
const formData = await formDataPromise
```

and `bufferToFormData()` (`src/utils/buffer.ts:112-113`) now lowercases everything before the first `;` in the header it is given.

One line became eight, one concept (ask the request) became five (resolve headers again, read bytes, re-parse with a normalized header, branch on request kind, write a foreign cache with a cast).

## Method

All probes were run on Node v24.21.0. Nothing was written into the clone. Sources were bundled with the clone's `./node_modules/.bin/esbuild … --bundle --platform=node --format=esm` from four trees:

- **head** — the clone's working tree at `5226d416`.
- **main** — `git archive main src` extracted into the work directory.
- **variant A** — a copy of head with `src/utils/buffer.ts` replaced by the merge-base file and `parseFormData()` restored to the merge-base one-liner (import of `bufferToFormData` removed). The new guard in `parseBody()` and the `i` flags in the validator are kept.
- **variant B** — variant A plus the consolidation described at the end of this file.

The probe builds a real multipart body (one text field, one `File`) and a real urlencoded body via `new Request(…, { body })`, optionally rewrites the media type to `Multipart/Form-Data` / `Application/X-WWW-Form-Urlencoded`, and then exercises call orders on a fresh `HonoRequest` each time.

Focused tests at head: `./node_modules/.bin/vitest --run --project main --coverage.enabled=false src/utils/body.test.ts src/utils/buffer.test.ts src/validator/validator.test.ts src/request.test.ts` → 4 files passed, 144 tests passed, 1 skipped.

## Probe matrix

`ok` means the correct `{ message: 'hello', file: File(a.txt) }` (multipart) or `{ message: 'hello' }` (urlencoded) came back. Results were identical for both media types unless noted.

| Scenario | main | head | variant A | variant B |
| --- | --- | --- | --- | --- |
| native `Request.formData()`, mixed-case media type | ok | ok | ok | ok |
| `parseBody(rawRequest)`, mixed-case | `{}` (the bug) | ok | ok | ok |
| `new HonoRequest(r).parseBody()`, mixed-case | `{}` (the bug) | ok | ok | ok |
| `formData()` then `parseBody()` | ok | **multipart: throws `Failed to parse body as FormData.`; urlencoded: one garbage key** | ok | ok |
| `blob()` then `parseBody()` | ok | ok | ok | ok |
| `text()` then `parseBody()` | throws `Content-Type was not one of …` | ok | throws (as main) | ok |
| `arrayBuffer()` then `parseBody()` | throws (as above) | ok | throws (as main) | ok |
| `parseBody()` twice: cache keys | `["formData"]` | `["arrayBuffer","formData"]` | `["formData"]` | `["arrayBuffer","formData"]` |
| `parseBody()` twice: same cache entry / same `File` | yes / yes | **no / no** | yes / yes | yes / yes |
| `parseBody()` then `formData()` | ok | ok | ok | ok |
| `validator('json')` with `Application/JSON` | not probed (the reported bug) | 200 | 200 | 200 |
| `validator('form')` mixed-case urlencoded, then `parseBody()` | not probed (the reported bug) | 200 | 200 | 200 |
| `validator('form')` mixed-case multipart, then `parseBody()` | not probed (the reported bug) | 200 | 200 | 200 |
| `Response.formData()` with `Multipart/Form-Data; boundary=sampleBoundary`, no normalization | ok | ok | ok | ok |

The urlencoded garbage on head, verbatim:

```
{"------formdata-undici-028405759848\r\nContent-Disposition: form-data; name":"\"message\"\r\n\r\nhello\r\n------formdata-undici-028405759848--\r\n"}
```

## F1 — why `formData()` then `parseBody()` breaks

Trace on head with `bodyCache = { formData: Promise<FormData> }`:

1. `parseFormData()` calls `request.arrayBuffer()` → `#cachedBody('arrayBuffer')`.
2. No `arrayBuffer` entry, but `Object.keys(bodyCache)[0]` is `'formData'`, so the conversion branch runs: `new Response(formData).arrayBuffer()`.
3. Serializing a `FormData` always produces `multipart/form-data` with a new random boundary. The original bytes and the original boundary are gone.
4. `bufferToFormData(thoseBytes, originalContentType)` parses new-boundary multipart bytes against the old boundary (throws) or against `application/x-www-form-urlencoded` (treats the whole multipart blob as one `key=value`).
5. The rejected or garbage promise is then written into `bodyCache.formData` (line 130), replacing the good one, so subsequent `c.req.formData()` calls are poisoned too. This last step is by reading the code; steps 1–4 are what the probe observed.

On main step 1 was `request.formData()`, which returns the cached entry immediately.

This order is not exotic. Any middleware that inspects the form with `c.req.formData()` (a CSRF token check, a honeypot field check, a logging hook) followed by a handler that uses `c.req.parseBody()` hits it.

The test suite cannot see it because every `parseBody` test starts from a fresh request. The PR in fact removed the one test seam that asserted `parseBody` goes through `req.formData()` (the `vi.spyOn(req, 'formData')` mock in `src/utils/body.test.ts`, replaced by real form data). Replacing the mock with real data is an improvement on its own; the point is only that the removal was forced by the rerouting and no ordering test took its place.

Remedy: restore the one-liner. Add two tests in `src/request.test.ts` under `req.parseBody()`: `formData()` then `parseBody()` for multipart and urlencoded, and `parseBody()` twice returning the same `File` instance.

## F2 — the rewrite is not load-bearing for the stated fix

Evidence is the variant A column above: with the rewrite and the `buffer.ts` change removed, every mixed-case row is still `ok`/200. The whole observable effect of the issue is produced by two things: the guard in `parseBody()` (`src/utils/body.ts:104-106`) and the regex flags in `src/validator/validator.ts:24-26`.

The last row of the matrix is the body of the new test at `src/utils/buffer.test.ts:120-132` run against the platform directly. It passes with no normalization, so that test does not distinguish head from main on Node; it would stay green if the `buffer.ts` change were deleted.

Variant A as a diff against head (source only):

```diff
--- src/utils/body.ts (head)
-import { bufferToFormData } from './buffer'
…
-  const headers = request instanceof HonoRequest ? request.raw.headers : request.headers
-  const arrayBuffer = await (request as Request).arrayBuffer()
-  const formDataPromise = bufferToFormData(arrayBuffer, headers.get('Content-Type') || '')
-  if (request instanceof HonoRequest) {
-    // Cache so that a later `c.req.formData()` reuses the already-consumed body
-    request.bodyCache.formData = formDataPromise as unknown as FormData
-  }
-  const formData = await formDataPromise
+  const formData = await (request as Request).formData()
--- src/utils/buffer.ts (head)
-      // Normalize the media type (case-insensitive) while keeping parameters like the boundary
-      'Content-Type': contentType.replace(/^[^;]+/, (mediaType) => mediaType.toLowerCase()),
+      'Content-Type': contentType,
```

Net source change for the PR would then be `src/utils/body.ts` +3 / −4 and `src/validator/validator.ts` +3 / −3.

What variant A gives up relative to head is the `text()`/`arrayBuffer()`-then-`parseBody()` rows, which throw on main and succeed on head. That improvement is real but unrelated to case-insensitivity, untested, and unmentioned; see Q2 in the summary. Variant B keeps it without the regression.

Unverified: behavior of `formData()` with mixed-case media types on Bun, Deno, workerd, and the Lambda adapters. The runtime-test suites were unavailable. This is Q1 in the summary.

## F3 — duplicated, divergent form parsing and the cache boundary

The two copies side by side:

`src/validator/validator.ts:113-127`

```ts
let formData: FormData

if (c.req.bodyCache.formData) {
  formData = await c.req.bodyCache.formData
} else {
  try {
    const arrayBuffer = await c.req.arrayBuffer()
    formData = await bufferToFormData(arrayBuffer, contentType)
    c.req.bodyCache.formData = formData
  } catch (e) { … 400 … }
}
```

`src/utils/body.ts:125-132` (head) is shown at the top of this file.

Differences that a maintainer now has to keep in their head:

- The validator consults `bodyCache.formData` first; `parseFormData()` does not. That is the direct cause of F1 and of the repeated re-parse.
- The validator stores a resolved `FormData`; `parseFormData()` stores a `Promise<FormData>` via `as unknown as FormData`. `#cachedBody` stores promises. So `bodyCache.formData` is, depending on who wrote it last, a value or a promise, while its declared type (`src/request.ts:20-27`, `BodyCache = Partial<Body>`) says value. The double cast is the type system reporting that the boundary is wrong.
- The validator uses the header as returned by `c.req.header('Content-Type')`; `parseFormData()` re-derives `headers` with a second copy of the `instanceof HonoRequest` ternary from `parseBody()` (line 101 and line 125), then branches on `instanceof HonoRequest` a third time at line 128. `parseBody()` already has `contentType` in hand and could have passed it down.
- Memory: a `HonoRequest` that goes through `parseBody()` now keeps both the raw `ArrayBuffer` and the parsed `FormData` (with its `File` blobs) alive in `bodyCache`. On main only the `FormData` was retained. For upload handlers this doubles retained body size per request. The validator already pays this cost deliberately (#1393, "cache `arrayBuffer` to use after validation"); `parseBody()` did not, and the PR does not say it intends to start.

The canonical owner of "this request's FormData" is `HonoRequest.formData()`. Both copies exist because that method cannot currently derive a `FormData` from cached bytes: its conversion branch does `new Response(body)[key]()` and drops the request's Content-Type, so `text()` → `formData()` fails. Fixing that in the owner removes the need for either copy.

### Worked consolidation (variant B)

`src/request.ts`:

```ts
import { bufferToFormData } from './utils/buffer'
…
formData(): Promise<FormData> {
  return (this.bodyCache.formData ??= this.arrayBuffer().then((buffer) =>
    bufferToFormData(buffer, this.raw.headers.get('Content-Type') ?? '')
  ))
}
```

with `BodyCache` retyped so its values are promises (for example `{ [K in keyof Body]?: Promise<Body[K]> }`), which is what `#cachedBody` already assumes when it calls `.then` on entries.

`src/utils/body.ts`: `parseFormData()` is the merge-base one-liner again; no import from `./buffer`, no knowledge of `bodyCache`.

`src/validator/validator.ts`, form case:

```ts
let formData: FormData
try {
  formData = await c.req.formData()
} catch (e) {
  let message = 'Malformed FormData request.'
  message += e instanceof Error ? ` ${e.message}` : ` ${String(e)}`
  throw new HTTPException(400, { message })
}
```

and the `bufferToFormData` import and both `c.req.bodyCache` accesses leave the validator.

Effect: one implementation instead of three partial ones; nobody outside `request.ts` touches `bodyCache`; the cast is gone; `formData()` is idempotent, so repeated `parseBody()` and validator-then-`parseBody()` return the same `FormData` and the same `File` instances; bytes are cached first, so `c.req.text()` / `c.req.json()` / `cloneRawRequest()` after form parsing still see the original bytes (the property #1393 wanted); and `text()`/`arrayBuffer()` followed by `parseBody()` works. If Q1 turns up a runtime that needs media-type normalization, `bufferToFormData()` is then the single place it applies.

Verification status of variant B: behavior probed in a scratch bundle, every row of the matrix `ok`. Not type-checked (the scratch copy used an inline cast instead of retyping `BodyCache`) and not run against the repository's vitest suites, since the clone may not be modified. The memory trade-off is the same as head's: `arrayBuffer` and `formData` are both retained. If that is not acceptable for `parseBody()` on large uploads, variant A is the right stopping point for this PR and the consolidation needs a separate decision about retention.
