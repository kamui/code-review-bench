# 01 — Body parsing path (`src/utils/body.ts`, `src/utils/buffer.ts`, interaction with `src/request.ts` and `src/validator/validator.ts`)

Review range: `9728702911073aec5a63a3ba2840b7240e5d3205..5226d4165d48643586152614cbd07422a0ab7a22` (`git diff main...review-head`).

## What changed

Before this PR, `parseFormData` in `src/utils/body.ts` was one line: `await (request as Request).formData()`. For a `HonoRequest` that goes through `HonoRequest.formData()`, which uses `#cachedBody('formData')` (`src/request.ts:220-239`). That method reuses a cached `formData` entry directly, derives it from another cached body if one exists, or otherwise calls `raw.formData()` and caches the promise.

The PR replaces that line with a hand-written sequence (`src/utils/body.ts:125-132`):

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

It also changes `bufferToFormData` (`src/utils/buffer.ts:113`) to lowercase everything before the first `;` in the Content-Type before it hands the body to `new Response(...).formData()`.

---

## Finding B1: `parseBody()` now breaks when the body was already read with `c.req.formData()` (multipart throws, urlencoded silently returns garbage)

**Severity:** structural regression with a user-visible correctness impact. **Status:** verified by running a probe against both head and base.

The new `parseFormData` always reads the body through `request.arrayBuffer()`. It never checks for an existing `bodyCache.formData` first. Say a handler or earlier middleware has already called `c.req.formData()`, so `bodyCache` holds only `formData`. Then `#cachedBody('arrayBuffer')` takes the "derive from another cached key" branch (`src/request.ts:228-235`) and re-serializes the cached `FormData` with `new Response(formData).arrayBuffer()`. That re-serialization is always **multipart**, and it uses a **new, randomly generated boundary**. `parseFormData` then passes those bytes to `bufferToFormData` together with the *original* `Content-Type` header. That header carries the original boundary for multipart requests, or says `application/x-www-form-urlencoded` for urlencoded ones. The header and the bytes no longer match.

Probe (`probe-formdata-then-parsebody.ts` in this directory, bundled with the clone's esbuild and run on Node v24.21.0). The route is `app.post('/a', async c => { const fd = await c.req.formData(); const pb = await c.req.parseBody(); ... })`:

```
HEAD
/a multipart {"fd":"hello","pb":"ERR Failed to parse body as FormData."}
/a urlenc    {"fd":"hello","pb":{"------formdata-undici-048221591976\r\nContent-Disposition: form-data; name":"\"message\"\r\n\r\nhello\r\n------formdata-undici-048221591976--\r\n"}}

BASE (main)
/a multipart {"fd":"hello","pb":{"message":"hello"}}
/a urlenc    {"fd":"hello","pb":{"message":"hello"}}
```

The urlencoded case is the dangerous one. There is no error: `parseBody()` returns an object whose single key is a chunk of multipart framing. Code that trusts `parseBody()` gets wrong data without any signal. The existing suites (`body`, `buffer`, `validator`, `request`: 144 passed, 1 skipped) do not cover the ordering "`formData()` then `parseBody()`", so CI passed.

In fairness, the same probe shows one ordering that improves. `c.req.text()` followed by `c.req.parseBody()` used to throw on base, because `new Response(text).formData()` has no Content-Type. On head it succeeds, because the header is now supplied explicitly. That improvement comes from routing through `arrayBuffer()` + the header, and a fix for B1 should keep it (see B2).

**Remedy.** The minimal fix is to reuse a cached `formData` before touching `arrayBuffer()`: `if (request instanceof HonoRequest && request.bodyCache.formData) return convert(await request.bodyCache.formData)`. Add a regression test in `src/request.test.ts` that calls `req.formData()` then `req.parseBody()` for both multipart and urlencoded. The better fix is the restructuring in B2, which puts the rule in one owner so this ordering bug has nowhere to live.

---

## Finding B2: The PR copies the validator's "arrayBuffer → bufferToFormData → cache" sequence into `parseFormData` instead of fixing the canonical owner, `HonoRequest.formData()`

**Severity:** missed code-judo / wrong layer. **Status:** duplication verified by reading the code. The claim that the platform already accepts mixed case is verified on Node v24.21.0 only; other runtimes were not available.

After this PR, the repository has **two** hand-rolled copies of "read the body as bytes, parse it as FormData using the request's Content-Type, stash the result in `bodyCache.formData`":

- `src/validator/validator.ts:115-127`: checks `bodyCache.formData` first, then `await c.req.arrayBuffer()`, `bufferToFormData(arrayBuffer, contentType)`, and stores the **resolved** `FormData` in the cache, with errors wrapped in `HTTPException(400)`.
- `src/utils/body.ts:125-132` (new): does **not** check the cache first (this is the cause of B1), reads `arrayBuffer()`, calls `bufferToFormData(...)`, stores the **promise** in the cache behind an `as unknown as FormData` cast, and wraps no errors.

The two copies already disagree on three points: whether they read the cache first, whether they cache a value or a promise, and how they handle errors. That drift is exactly what leads to bugs like B1. Meanwhile the canonical API, `HonoRequest.formData()` (`src/request.ts:329`), still goes through `#cachedBody('formData')` → `raw.formData()`, and neither the PR's normalization nor its "supply the Content-Type" behaviour reaches it. If some runtime really does reject `Multipart/Form-Data` in `Request.formData()` (the only reason the `bufferToFormData` change and the `parseFormData` rewrite would be needed), then a user calling `c.req.formData()` directly on that runtime is still broken after this PR. The fix sits one layer too low in `parseBody` and one layer too high in `buffer.ts`, and the object that owns the concept is untouched.

The premise also deserves questioning. On Node v24.21.0, which is also the runtime the vitest suite uses, the platform already handles mixed-case media types:

```
$ node probe.mjs
multipart/form-data; boundary=b -> Hello
Multipart/Form-Data; boundary=b -> Hello
Application/X-WWW-Form-Urlencoded -> 1
$ node -e '... new Response(b, {headers: {"Content-Type": "Multipart/Form-Data; boundary=b"}}).formData() ...'
Response mixed-case multipart -> Hi
Response mixed-case urlenc -> 1
```

This matches the Fetch/MIME Sniffing specs, where "parse a MIME type" lowercases the type and subtype. So in the test environment, the new `bufferToFormData` lowercasing and the whole `parseFormData` rewrite fix no observable failure. The new tests (`body.test.ts` "mixed-case multipart", `buffer.test.ts` "mixed-case … keeping the boundary") would pass without them. Only the guard change in `parseBody` (the `mediaType === ...` check) is needed to fix issue #5060 on a spec-compliant runtime. The PR neither names a runtime that needs the extra machinery nor adds a test that fails without it.

**Worked code-judo proposal.** Choose one of these; both delete a copy instead of adding one.

*Option A (smallest, if no runtime needs byte-level normalization).* Revert `parseFormData` to `await (request as Request).formData()` and revert the `buffer.ts` change. Keep only the guard fix in `parseBody` and the validator regex change. This deletes the new cache write, the double cast, the extra `ArrayBuffer` that is now held in `bodyCache`, and the B1 regression, and still fixes issue #5060 as reported.

*Option B (if a runtime does need it, or to keep the `text()`-then-`parseBody()` improvement).* Move the rule into the owner. Give `HonoRequest` a `formData()` that is Content-Type-aware:

```ts
// src/request.ts
formData(): Promise<FormData> {
  return (this.bodyCache.formData ??= this.arrayBuffer().then((buf) =>
    bufferToFormData(buf, this.raw.headers.get('Content-Type') ?? '')
  ))
}
```

Then both consumers become direct calls:

```ts
// src/utils/body.ts
async function parseFormData<T extends BodyData>(request: HonoRequest | Request, options: ParseBodyOptions) {
  return convertFormDataToBodyData<T>(await request.formData(), options)
}

// src/validator/validator.ts  (the whole if/else + manual cache write collapses)
let formData: FormData
try {
  formData = await c.req.formData()
} catch (e) { /* existing 400 wrapping */ }
```

After Option B there is one place that knows how FormData is derived from a request and how it is cached. `parseBody`, the validator and `c.req.formData()` get the same case handling, the same cache semantics and the same "derive from any cached body using the real Content-Type" behaviour. B1 cannot recur, because nothing re-derives bytes from a cached `FormData` and then parses them against the original header. The `request instanceof HonoRequest` branch and the cast in `parseFormData` disappear. The validator also loses its peek into `c.req.bodyCache`, which is an internal detail leaking across a boundary. One detail needs care: `#cachedBody`'s "derive from another cached key" branch must not serialize a cached `FormData` back to bytes and re-parse it. With `formData` always derived *from* `arrayBuffer`, the `arrayBuffer` entry is always present first, so that branch is never taken for this pair.

---

## Finding B3: `as unknown as FormData` papers over a `BodyCache` type that lies about what it stores

**Severity:** type/boundary cleanliness. **Status:** verified by reading the code.

`BodyCache` is declared as `Partial<Body>`, which maps `formData` to `FormData` (`src/request.ts:20-27`). `#cachedBody`, however, stores *promises* (`bodyCache[key] = raw[key]()`) and consumes them with `.then` (`src/request.ts:230`). The validator stores a *resolved* `FormData` (`validator.ts:121`), and it only works because `await` accepts non-thenables and because `arrayBuffer` happens to be inserted into the cache earlier. The PR adds a third writer, and it needs `formDataPromise as unknown as FormData` (`src/utils/body.ts:130`) to get past the type. The cache now holds a mix of values and promises under one key, and the double cast hides that the type is wrong.

**Remedy.** Retype the cache to what it really holds, `type BodyCache = { [K in keyof Body]?: Promise<Body[K]> }`, and make every writer store a promise. The cast then disappears. If Option B from B2 is adopted, the only writer is `HonoRequest` itself, and the validator's `c.req.bodyCache.formData = formData` line goes away.

---

## Commands used

```
git diff main...review-head
./node_modules/.bin/vitest --run --project main --coverage.enabled=false \
  src/utils/body.test.ts src/utils/buffer.test.ts src/validator/validator.test.ts src/request.test.ts
  -> Test Files 4 passed (4); Tests 144 passed | 1 skipped (145)
./node_modules/.bin/esbuild <work>/probe2.ts --bundle --platform=node --format=esm --outfile=<work>/probe2.mjs && node <work>/probe2.mjs
git archive main src | tar -x -C <work>/base   # base copy for the same probe
node <work>/probe.mjs                           # platform mixed-case behaviour
```
