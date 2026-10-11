# 01 — Form body pipeline: `parseBody`, `HonoRequest.bodyCache`, `validator('form')`

Carries the evidence for summary findings F1, F2, F3 and F5.

## How the evidence was produced

The clone was never modified. The base tree was exported with `git archive main src | tar -x` into `clone-work/main-src`. Probes are TypeScript files under `clone-work/probe`, bundled with the clone's own `./node_modules/.bin/esbuild … --bundle --platform=node --format=esm` and run with `node` (v24.21.0, undici 7.29.1). Each probe was built once per side by substituting the source root, so the same script runs against the base (`main-src/src`), the head (`clone/src`) and two scratch variants described below.

The head's own tests were run once with
`./node_modules/.bin/vitest --run --project main --coverage.enabled=false src/utils/body.test.ts src/utils/buffer.test.ts src/validator/validator.test.ts src/request.test.ts src/middleware/method-override/index.test.ts`
and reported 5 files passed, 155 tests passed, 1 skipped.

## What the PR changed in this subsystem

On the base, `parseFormData` was one line of real work: `const formData = await (request as Request).formData()`. For a `HonoRequest` that call lands in `#cachedBody('formData')` (`src/request.ts:220-239`), which returns the cached promise if there is one and otherwise stores `raw.formData()`.

On the head, `src/utils/body.ts:125-132` is:

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

The cache is now written from outside `HonoRequest` and is no longer read on this path.

## F1 — verified regression: `formData()` then `parseBody()`

Probe `probe/probe-body.ts`, cases P1 to P5. Requests are built from a `FormData` with `message=hello` (multipart) or a `URLSearchParams` (urlencoded) and wrapped in `new HonoRequest(...)`.

| Case | Base (`main`) | Head |
| --- | --- | --- |
| P1 multipart: `await req.formData(); await req.parseBody()` | `{"message":"hello"}` | throws `TypeError: Failed to parse body as FormData.` |
| P1 urlencoded: same sequence | `{"message":"hello"}` | `{"------formdata-undici-085545663634\r\nContent-Disposition: form-data; name":"\"message\"\r\n\r\nhello\r\n------formdata-undici-085545663634--\r\n"}` |
| P2 multipart / urlencoded: `text()` then `parseBody()` | throws `TypeError: Content-Type was not one of …` | `{"message":"hello"}` |
| P3 `parseBody()` three times on one request, number of platform `formData()` parses | 1 | 3 |
| P3 `validator('form')` then `c.req.parseBody()`, number of platform parses | 1 | 2 |
| P4 `parseBody()` then `cloneRawRequest(req).formData()` | throws `Failed to parse body as FormData.` | `{"message":"hello"}` |
| P5 urlencoded `parseBody()` then `text()` | a multipart document (`------formdata-undici-…`) | `message=hello` |

The mechanism for P1 is fully traceable in the source. After `req.formData()` the cache has exactly one key, `formData`. `parseFormData` asks for `request.arrayBuffer()`; `#cachedBody('arrayBuffer')` finds no `arrayBuffer`, takes `Object.keys(bodyCache)[0]`, which is `formData`, and returns `new Response(formData).arrayBuffer()` (`src/request.ts:228-235`). Serialising a `FormData` always yields `multipart/form-data` with a boundary chosen at that moment. `parseFormData` then passes those bytes to `bufferToFormData` together with `headers.get('Content-Type')` from the original request: for multipart that is the original boundary, which no longer occurs in the bytes, hence the `TypeError`; for urlencoded the multipart document is parsed as `key=value` pairs, hence the garbage key.

The validator does not hit this because it reads `c.req.bodyCache.formData` first (`src/validator/validator.ts:115-116`) and, when it has to parse, reads `arrayBuffer` before it writes `formData`, so the first cache key is the original bytes. `parseFormData` copied the second half of that logic without the first.

Who is exposed: any application where something reads the form with `c.req.formData()` before a later step calls `c.req.parseBody()`; a middleware that inspects a form field and a handler that parses the body is enough. Status: verified by execution on Node 24.

P2, P4 and P5 are improvements on the head. They are real, they are not mentioned in the PR, and no test covers them; see summary question Q2.

## F2 — the pipeline now exists twice, and its owner was not fixed

The two copies side by side:

`src/validator/validator.ts:115-127` reads the cache, else `arrayBuffer()` → `bufferToFormData(arrayBuffer, contentType)` → stores the **resolved** `FormData`, and maps failures to `HTTPException(400)`.

`src/utils/body.ts:125-132` never reads the cache, does `arrayBuffer()` → `bufferToFormData(arrayBuffer, header || '')` → stores the **promise**, with no error mapping.

`src/request.ts:329-331`, `HonoRequest.formData()`, is the public method whose job this is, and it still does `this.#cachedBody('formData')`, i.e. `raw.formData()` with the unnormalized header.

To show that the owner was left behind, probe `probe/scen-body.ts` has a mode (`SIM=1`) that replaces `Request.prototype.formData` and `Response.prototype.formData` with versions that reject unless the `Content-Type` starts with a lower-case form media type. That models the kind of platform for which `buffer.ts:113` exists. On the head under that simulation, `c.req.parseBody()` (S1, S2), bare `parseBody(Request)` (S3), `bufferToFormData` (S4) and `validator('form')` (S6, S7) all succeed, and a handler that calls `c.req.formData()` on a `Multipart/Form-Data` request (S8) returns HTTP 500. On native Node 24 S8 succeeds on every side, because the platform does not need the help. Status: verified by execution, under a simulated platform; I could not run Bun, Deno or Workers.

Incidental complexity the copy brought into `body.ts`: the header collection is resolved at line 101 and again at line 125; `instanceof HonoRequest` is evaluated at 101, 125 and 128; `|| ''` at 127 cannot fire because `parseFormData` is only reached after the media type matched; and the pre-existing `if (formData) { … } return {} as T` at 134-138 is unreachable (an awaited `FormData` is always truthy) but now sits under more code.

### Worked code-judo proposal

Make `HonoRequest.formData()` the one place that knows how to obtain a `FormData`:

```ts
// src/request.ts
type BodyCache = { [K in keyof Body]?: Promise<Body[K]> }

formData(): Promise<FormData> {
  // Always derive from the original bytes so the request's own boundary stays valid
  return (this.bodyCache.formData ??= this.arrayBuffer().then((buffer) =>
    bufferToFormData(buffer, this.raw.headers.get('Content-Type') ?? '')
  ))
}
```

Then the callers shrink to what they are actually about:

```ts
// src/utils/body.ts — parseFormData is deleted
export const getMediaType = (contentType: string | null | undefined): string | undefined =>
  contentType?.split(';', 1)[0].trim().toLowerCase()

export const parseBody: ParseBody = async (
  request: HonoRequest | Request,
  options = Object.create(null)
) => {
  const { all = false, dot = false } = options

  const isHonoRequest = request instanceof HonoRequest
  const contentType = (isHonoRequest ? request.raw : request).headers.get('Content-Type') ?? ''
  const mediaType = getMediaType(contentType)

  if (mediaType !== 'multipart/form-data' && mediaType !== 'application/x-www-form-urlencoded') {
    return {}
  }

  const formData = await (isHonoRequest
    ? request.formData()
    : bufferToFormData(await request.arrayBuffer(), contentType))

  return convertFormDataToBodyData(formData, { all, dot })
}
```

```ts
// src/validator/validator.ts — no cache access, no bufferToFormData import
let formData: FormData
try {
  formData = await c.req.formData()
} catch (e) {
  let message = 'Malformed FormData request.'
  message += e instanceof Error ? ` ${e.message}` : ` ${String(e)}`
  throw new HTTPException(400, { message })
}
```

Why this is a deletion and not a rearrangement: there is one reader-and-writer of `bodyCache.formData` instead of three writers and two readers; the cache key order is always `arrayBuffer` then `formData`, so `#cachedBody`'s "first cached key" rule always finds the original bytes and the re-serialisation hazard behind F1, P4 and P5 cannot arise from any call order; `utils/body.ts` no longer touches `bodyCache`; the cast, the second header lookup, two of three `instanceof` checks, the `|| ''` fallback and the unreachable tail all disappear.

Measured on the scratch copy `clone-work/judo-src` (head plus exactly the edits above and the `method-override` change from detail file 02): `src/utils/body.ts` 240 → 221 lines, `src/validator/validator.ts` 185 → 178, `src/request.ts` 505 → 509, `src/utils/buffer.ts` unchanged from the head. Net 22 production lines fewer than the head.

Probe results for that variant: P1 multipart and urlencoded both `{"message":"hello"}`; P2 both `{"message":"hello"}`; P3 one platform parse for three `parseBody()` calls and one for `validator('form')` + `parseBody()`; P4 `{"message":"hello"}`; P5 `message=hello`; S1 to S8 all succeed both natively and under the simulated case-sensitive platform.

Verification status and limits of the proposal, stated plainly. It was exercised only through the esbuild-bundled probes. It was not type-checked (running `tsc` is outside this review's execution allowance), and the repository's vitest suites were not run against the scratch copy. In the scratch file I kept the existing `BodyCache` type and used `as any` on the assignment; the `BodyCache` type shown above is the intended replacement and is untested against the compiler, in particular the return type of `#cachedBody` may need a matching adjustment. Two deliberate behaviour differences from the head: a cached `FormData` promise that rejects is now reported by the validator as a 400 instead of escaping as a raw error, and `c.req.formData()` buffers through `arrayBuffer()` (see F5). `request.test.ts:424-429`, which expects `formData()` after `parseBody()` on an `application/json` body to reject, is still satisfied by reading: `Response.formData()` rejects for that media type.

If the maintainers prefer the smallest possible repair instead, replace `body.ts:126-132` with a cache-first read (`request instanceof HonoRequest ? await request.formData() : …`). That fixes F1 but keeps two pipelines and leaves `c.req.formData()` unnormalized, which is why I do not recommend stopping there.

## F3 — the cast and the cache type

`src/request.ts:20-27`:

```ts
type Body = { json: any; text: string; arrayBuffer: ArrayBuffer; blob: Blob; formData: FormData }
type BodyCache = Partial<Body>
```

`#cachedBody` stores `raw[key]()`, a promise, at line 238 and treats cached entries as promises at line 230 (`as Promise<BodyInit>`). The declaration says resolved values. It compiles in `request.ts` because the write target `bodyCache[key]` for `key: keyof Body` has the intersection of all member types as its write type, and `any` (from `json`) absorbs that intersection. Outside that accident the type bites, which is why `body.ts:130` needed `as unknown as FormData`.

Writers of `bodyCache.formData` on the head: `#cachedBody` (promise), `validator.ts:121` (resolved value, which happens to type-check against the wrong declaration), `body.ts:130` (promise, double cast). Readers: `#cachedBody` returns it as-is, `validator.ts:116` awaits it, `cloneRawRequest` calls the method again. Everything works only because every consumer awaits. This is inferred from reading the code; I did not run the compiler.

Remedy: declare what is stored, `type BodyCache = { [K in keyof Body]?: Promise<Body[K]> }`, and let `HonoRequest` be the only writer (F2). If an external writer must remain, it should store a promise so there is one shape.

## F5 — memory retained by `parseBody()`

Probe `probe/mem-body.ts`, run with `node --expose-gc`, twice per side with identical results. It builds a request whose body is a stream carrying a multipart document with one 64 MB file part, snapshots `process.memoryUsage()` after several forced collections, calls `await req.parseBody()`, collects again, and reports the difference while both the `HonoRequest` and the parsed body are still referenced.

| Side | `arrayBuffers` delta | `external` delta | cache keys after `parseBody()` |
| --- | --- | --- | --- |
| Base | −64 MB | −64 MB | `formData` |
| Head | ≈ 0 MB | ≈ 0 MB | `arrayBuffer,formData` |

The absolute numbers include the release of the 64 MB source stream, which is the same on both sides, so the meaningful figure is the difference: the head keeps one extra full copy of the upload (64 MB here) alive for as long as the request object lives. Peak usage during the parse was not measured. Status: measured on Node 24 only.

This is the cost side of the P2/P4/P5 improvements. It is a legitimate design choice and the validator already makes it, but `parseBody()` is the upload API and the choice should be visible. Under the F2 proposal the choice lives in one line. The alternative that avoids the copy is to hand the platform the stream with a corrected header, `new Response(this.raw.body, { headers: { 'Content-Type': normalized } }).formData()`; that keeps base-level memory but makes `formData` the first cache key again, which brings back the base's P4 and P5 behaviour unless `#cachedBody` is taught not to re-serialise a `FormData`. I did not build that variant.

## Bundle size (context, not a finding)

`esbuild --bundle --minify --format=esm` of `src/index.ts`: base 18679 bytes, head 18933 (+254), F2 variant 18896. Of `src/preset/tiny.ts`: base 10991, head 11243 (+252), F2 variant 11205. The new `body.ts → buffer.ts → crypto.ts` import chain is tree-shaken by a bundler (no `subtle` reference appears in any bundle); in the unbundled published files it means `buffer` and `crypto` modules are loaded with the core, which they were not before. Small, but worth knowing for a library that tracks its size.
