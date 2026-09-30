# 01 — Body parsing and the HonoRequest body cache

Scope: `src/utils/body.ts` (`parseBody`, `parseFormData`), `src/utils/body.test.ts`, and how they interact with
`src/request.ts` (`bodyCache`, `#cachedBody`, `formData()`) and `src/validator/validator.ts` (the `form` branch).

Review range: `main...review-head` (`9728702..5226d41`).

## Finding 1.1 — `parseFormData` bypasses the canonical body cache and breaks `c.req.formData()` followed by `c.req.parseBody()` (CONFIRMED)

**Where.** `src/utils/body.ts:121-139` (head).

Before the PR, `parseFormData` was one line:

```ts
const formData = await (request as Request).formData()
```

For a `HonoRequest`, that call goes through `HonoRequest.formData()` → `#cachedBody('formData')`
(`src/request.ts:220-239`, `:329-331`), which is the one place that owns body caching. If `formData` was already
cached, that cached value was returned directly.

After the PR, `parseFormData` does its own caching:

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

This always asks for `arrayBuffer` first. When only `formData` is cached (the user called `c.req.formData()`
earlier), `#cachedBody('arrayBuffer')` falls into its "derive from any cached key" path:
`new Response(cachedFormData).arrayBuffer()`. That re-serializes the `FormData` as a **new multipart body with a
freshly generated boundary**. `parseFormData` then parses those bytes against the **original** request
`Content-Type`:

- For a multipart request, the boundary in the header no longer matches the bytes, and undici throws
  `TypeError: Failed to parse body as FormData` (`no boundary found in multipart body`). The handler returns 500.
- For an urlencoded request, the multipart bytes get parsed as `application/x-www-form-urlencoded`, which returns one
  junk key made of the multipart envelope text. The handler gets **silently wrong data**.

**Verification.** I bundled a probe with esbuild against the head sources and against a `git archive main` copy
of the base sources, then ran both under Node v24.21.0. The probe files are
`clone-work/scratch/probe.ts` and `probe-base.ts`:

```
./node_modules/.bin/esbuild <work>/scratch/probe.ts --bundle --platform=node --format=esm --outfile=<work>/scratch/probe.mjs
node <work>/scratch/probe.mjs
```

The handler is `app.post('/fd-then-parse', async (c) => { await c.req.formData(); return c.json(await c.req.parseBody()) })`.

| Scenario | base (`main`) | head (`review-head`) |
| --- | --- | --- |
| multipart, `formData()` then `parseBody()` | `200 {"a":"1"}` | **`500 Internal Server Error`** (undici: no boundary found) |
| urlencoded, `formData()` then `parseBody()` | `200 {"a":"1"}` | **`200 {"------formdata-undici-…\r\nContent-Disposition: form-data; name": "\"a\"\r\n\r\n1\r\n------formdata-undici-…--\r\n"}`** |
| multipart / urlencoded, `parseBody()` then `formData()` | 200 ok | 200 ok |
| `validator('form')` then `parseBody()` | 200 ok | 200 ok (arrayBuffer already cached by the validator) |
| `parseBody()` twice | 200 ok | 200 ok |
| `text()` then `parseBody()` | 500 (pre-existing bug: derived Response has no Content-Type) | 200 ok |

The last row is a side effect of the new code. It fixes one ordering and breaks another. That shows the real
problem: the ordering-dependent derivation logic lives in `#cachedBody`, and `parseFormData` now reaches past it
instead of fixing it. The PR's own tests all pass
(`vitest --run --project main --coverage.enabled=false src/utils/body.test.ts src/utils/buffer.test.ts src/validator/validator.test.ts src/request.test.ts`
reports 144 passed, 1 skipped). No test covers `formData()` → `parseBody()`.

**Why this is a structural problem, not only a bug.** `request.ts` already has one owner for "read the body once,
derive the other representations from the cache": `#cachedBody`. The validator already has a second, bespoke
implementation (`src/validator/validator.ts:115-127`: `arrayBuffer()` → `bufferToFormData` → write
`bodyCache.formData`). This PR adds a third copy inside a general-purpose utility (`utils/body.ts`). It reaches into
`request.bodyCache` from outside `HonoRequest`, adds an `instanceof HonoRequest` branch, and re-reads the headers
through a second `instanceof` ternary that duplicates the one in `parseBody` (line 101). Every copy has its own
ordering assumptions, and they already disagree: the validator stores a resolved `FormData`, while `parseBody` stores
a `Promise<FormData>` cast to `FormData`.

**Worked code-judo proposal.** Fix the derivation once, in the layer that owns it, and delete the bespoke copies.

1. Teach `#cachedBody` to derive `formData` with the request's own `Content-Type`, instead of the header-less
   `new Response(body)`:

   ```ts
   // src/request.ts
   #cachedBody = (key: keyof Body) => {
     const { bodyCache, raw } = this
     const cachedBody = bodyCache[key]
     if (cachedBody) {
       return cachedBody
     }
     if (key === 'formData') {
       // Always parse form data from bytes with the request's own Content-Type
       // (media type normalized in bufferToFormData); never re-encode a FormData.
       return (bodyCache.formData = this.arrayBuffer().then((buf) =>
         bufferToFormData(buf, raw.headers.get('Content-Type') ?? '')
       ))
     }
     // ...existing "derive from any cached key" path for the other representations
   }
   ```

   This keeps `formData` from ever being the only cached representation, so no later `arrayBuffer()` call has to
   re-encode it with a new boundary.
2. `parseFormData` goes back to the direct flow. It no longer imports `bufferToFormData`, reads headers, or touches
   `bodyCache`:

   ```ts
   const formData = await (request as Request).formData()
   return convertFormDataToBodyData<T>(formData, options)
   ```

   For a raw `Request` this still uses the platform parser. Mixed-case media types are accepted there (see the
   question in the summary). If a runtime turns out not to accept them, give the raw-`Request` branch the same
   `arrayBuffer` + `bufferToFormData` treatment in one small helper that does not write any cache.
3. The validator's `form` branch (`validator.ts:113-127`) becomes `formData = await c.req.formData()` inside the
   existing try/catch. Its manual `bodyCache.formData` read and write go away.

Result: one owner of body caching, zero `bodyCache` writes outside `HonoRequest`, no `instanceof` branches in
`utils/body.ts` beyond the existing header lookup, and no cast. Add a regression test for `formData()` →
`parseBody()` for both multipart and urlencoded bodies.

## Finding 1.2 — `bodyCache.formData = formDataPromise as unknown as FormData` hides the cache's real contract (CONFIRMED by reading)

**Where.** `src/utils/body.ts:130`, and `BodyCache = Partial<Body>` at `src/request.ts:20-27`.

`BodyCache` is typed as holding resolved values (`formData: FormData`, `arrayBuffer: ArrayBuffer`, ...). At runtime,
`#cachedBody` stores **promises** (`bodyCache[key] = raw[key]()`), and the validator stores a **resolved**
`FormData`. The PR's `as unknown as FormData` double cast writes a promise into a slot typed as a value. That is the
third shape convention for the same field. It only works because every reader happens to `await` the slot or hand it
back from a promise-returning method. A future reader that trusts the type (`bodyCache.formData.get(...)`) will get a
Promise and fail at runtime, and the type checker will not catch it.

**Remedy.** Make the boundary explicit rather than casting through it. Type the cache as
`{ [K in keyof Body]?: Promise<Body[K]> }`, which is what `#cachedBody` actually stores. Then the validator's resolved
write becomes a type error that pushes it onto the canonical path. Better still, with the Finding 1.1 restructure
nothing outside `HonoRequest` writes to `bodyCache`, so the cast disappears on its own.

## Minor notes (not escalated)

- Removing the `vi.spyOn(req, 'formData')` mock in `body.test.ts` and building a real `FormData` instead is a good
  change. The old test was coupled to the implementation's call shape.
- `if (formData)` in `parseFormData` (`body.ts:134`) is dead code, because `formData()` and `bufferToFormData` both
  resolve to a `FormData` or throw. This predates the PR, and part of the codecov "missing lines" comes from it. The
  simplified `parseFormData` in the proposal above drops it.
- All touched files stay well under 1000 lines (`body.ts` 240, `buffer.ts` 117, `validator.ts` 185, `request.ts` 505).
