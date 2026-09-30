# 01 — Form body parsing: `parseFormData` rewrite and the HonoRequest body cache

Scope: `src/utils/body.ts` (lines 101–139 at head), its interaction with `src/request.ts` (`#cachedBody`, `formData()`, `bodyCache`) and the duplicate form-parsing path in `src/validator/validator.ts` (lines 105–127).

## What the PR changed here

Before the PR, `parseFormData` was a single line: `const formData = await (request as Request).formData()`. For a `HonoRequest` that goes through `HonoRequest.formData()` → `#cachedBody('formData')`, which is the one place in the codebase that owns "read the body once, cache it, and convert between cached representations."

After the PR, `parseFormData` (body.ts:125–132) does this by hand:

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

It asks for the body as an `ArrayBuffer`, parses it with `bufferToFormData` (whose media type is now normalized to lower case), and then writes directly into `HonoRequest.bodyCache.formData`. It needs an `as unknown as FormData` cast to do that.

## Finding 1.1 — Regression: a `formData` value that is already cached is re-serialized with a new boundary and then parsed against the old header

**Verification status: CONFIRMED by execution** (head vs. base, Node v24.21.0).

`#cachedBody('arrayBuffer')` (request.ts:220–239) first looks for an `arrayBuffer` cache entry. If there isn't one, it falls back to *any* cached key and converts with `new Response(body)[key]()`. When the cached key is `formData`, `new Response(formData).arrayBuffer()` re-encodes the form as **multipart with a freshly generated boundary**. The rewritten `parseFormData` then passes those bytes to `bufferToFormData` together with the **original** request `Content-Type`:

- If the original was multipart, the boundary in the header no longer matches the boundary in the bytes, so `Response.formData()` throws `TypeError: Failed to parse body as FormData.`
- If the original was urlencoded, the multipart bytes are parsed as urlencoded. That succeeds silently and produces garbage keys, where the entire multipart envelope becomes one key.

Any handler or middleware that calls `c.req.formData()` before `c.req.parseBody()` hits this. On base that sequence worked because `parseFormData` called `request.formData()` and got a cache hit.

Probe (scratch file `clone-work/scratch/probe.ts`, bundled with the clone's esbuild and run with node; the same probe was run against `git archive main src`):

```
route: await c.req.formData(); return c.json({ ok: await c.req.parseBody() })

base  /fd-then-parse multipart  200 {"ok":{"a":"1"}}
base  /fd-then-parse urlencoded 200 {"ok":{"a":"1"}}
head  /fd-then-parse multipart  500 {"err":"TypeError: Failed to parse body as FormData."}
head  /fd-then-parse urlencoded 200 {"ok":{"------formdata-undici-049980181593\r\nContent-Disposition: form-data; name":"\"a\"\r\n\r\n1\r\n------formdata-undici-049980181593--\r\n"}}
```

The failure also leaves `bodyCache.formData` overwritten with a rejected promise, so a later `c.req.formData()` in the same request rejects as well.

For fairness: the rewrite incidentally fixes the reverse order. `c.req.text()` followed by `parseBody()` failed on base (`Content-Type was not one of ...`) because `#cachedBody` converts a cached `text` value via `new Response(text).formData()` with no content type. It passes on head. The remedy below keeps that improvement.

None of the added or existing tests cover "formData already cached, then parseBody". The PR's test edit for the `dot` case removed the `vi.spyOn(req, 'formData')` mock, because `parseFormData` no longer calls `formData()` at all. That is a sign the PR moved off the cache-owning code path.

## Finding 1.2 — Structural: form parsing and cache writes now live in two utility modules, and the canonical owner is left unfixed

**Verification status: CONFIRMED by reading.** The duplication and the unnormalized `c.req.formData()` are visible in the source. On Node 24 the platform already accepts mixed-case media types (`new Request(..., {headers: {'content-type': 'Multipart/Form-Data; boundary=...'}}).formData()` returns the data), so this concern applies to runtimes whose `formData()` is case-sensitive, which is presumably why `bufferToFormData` was normalized.

The validator already contained a hand-rolled copy of "`c.req.arrayBuffer()` → `bufferToFormData(buf, contentType)` → write `c.req.bodyCache.formData`" (validator.ts:115–127). The PR adds a second copy of the same idea to `body.ts`. The two copies disagree on shape: the validator stores a resolved `FormData`, while body.ts stores a `Promise<FormData>` cast to `FormData`. A shared utility (`utils/body.ts`) now reaches into `HonoRequest`'s cache internals with a double cast. That is exactly the kind of implementation detail that `#cachedBody` exists to hide.

Meanwhile the canonical, user-facing API `c.req.formData()` still calls `raw.formData()` with the unnormalized header. On a case-sensitive runtime the PR therefore fixes `parseBody()` and `validator('form')` but not `c.req.formData()`, even though the issue's premise ("case-only variants of supported content types are silently skipped") applies to it equally. The fix sits one layer too low and in two places, not once in the layer that owns request-body parsing.

## Worked code-judo proposal (verified in a scratch copy)

Make `HonoRequest.formData()` the only place that turns a request body into `FormData`. Then have both callers use it.

`src/request.ts`:

```ts
import { bufferToFormData } from './utils/buffer'
...
formData(): Promise<FormData> {
  return (this.bodyCache.formData ??= this.arrayBuffer().then((buffer) =>
    bufferToFormData(buffer, this.header('Content-Type') ?? '')
  ) as unknown as FormData) as unknown as Promise<FormData>
}
```

(The casts are only needed because `BodyCache` is typed as `Partial<Body>` while it actually stores promises. That is pre-existing. Retyping it as `{ [K in keyof Body]?: Promise<Body[K]> }` removes the casts here and in every other cache access.)

`src/utils/body.ts` `parseFormData` body becomes:

```ts
const formData =
  request instanceof HonoRequest
    ? await request.formData()
    : await bufferToFormData(await request.arrayBuffer(), request.headers.get('Content-Type') ?? '')
```

`src/validator/validator.ts` form branch collapses to:

```ts
try {
  formData = await c.req.formData()
} catch (e) {
  let message = 'Malformed FormData request.'
  message += e instanceof Error ? ` ${e.message}` : ` ${String(e)}`
  throw new HTTPException(400, { message })
}
```

This also removes the now-unused `bufferToFormData` import from validator.ts.

Result: the `bodyCache.formData` write and the `as unknown as FormData` cast in body.ts disappear, the validator's `if (c.req.bodyCache.formData) … else …` branch disappears, and `c.req.formData()` gains the case-insensitive behavior the issue asks for. Because `formData()` goes through `this.arrayBuffer()`, it gets the raw bytes when nothing is cached and keeps the "text cached first" improvement from head. Because the `??=` returns the cached value first, it no longer re-serializes a cached `FormData`.

Scratch verification (copy of `review-head:src` under `clone-work/scratch/judo`, same probe):

```
/fd-then-parse         multipart 200 {"ok":{"a":"1"}}   urlencoded 200 {"ok":{"a":"1"}}
/text-then-parse       multipart 200 {"ok":{"a":"1"}}   urlencoded 200 {"ok":{"a":"1"}}
/parse-then-fd         multipart 200                    urlencoded 200
/validator-then-parse  multipart 200                    urlencoded 200
/parse-then-validator  multipart 200                    urlencoded 200
validator('form') with malformed multipart  -> 400 "Malformed FormData request. Failed to parse body as FormData."
validator('form') with Application/X-WWW-Form-Urlencoded -> 200 {"a":"1"}
```

One behavior nuance to note in the PR if this lands: a rejected cached `formData` promise reached via the validator is now wrapped in the 400 `HTTPException`, where before it propagated raw. That is arguably more consistent.

Minor, and subsumed by the proposal: at head, `parseBody` reads `Content-Type` (body.ts:101–102) and `parseFormData` re-derives `headers` and reads it again (body.ts:125–127). If any bespoke path remains, pass the value down instead of recomputing it.

## Recommended regression test

In `src/utils/body.test.ts` or `src/request.test.ts`, add: for both multipart and urlencoded, create a `HonoRequest`, `await req.formData()`, then `expect(await req.parseBody()).toEqual({ message: 'hello' })`. Add the mirror case of `await req.text()` followed by `parseBody()` to lock in the incidental improvement.
