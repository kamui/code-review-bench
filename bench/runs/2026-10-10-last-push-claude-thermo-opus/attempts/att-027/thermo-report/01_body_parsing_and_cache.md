# 01 — Body parsing and the request body cache

Covers findings F1 and F2 of `summary.md`. Files: `src/utils/body.ts`, `src/request.ts`, `src/validator/validator.ts`, `src/utils/buffer.ts`.

## What the PR changed

On `main`, `parseFormData` was one call:

```ts
const formData = await (request as Request).formData()
```

For a `HonoRequest` that call resolves to `HonoRequest.formData()` → `#cachedBody('formData')` (`src/request.ts:220-239`), which returns the cached value if present, otherwise derives it from whichever body representation is already cached, otherwise calls `raw.formData()` and caches the promise.

On head (`src/utils/body.ts:121-139`):

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

The intent is evidently to route the header through `bufferToFormData`, which now lower-cases the media type before the platform parser sees it. The side effects are what this file documents.

## How the evidence was produced

All commands were run from the clone root with the clone's own binaries; nothing was written into the clone (`git status --short` was empty after every step). Scratch sources live under the work directory in `scratch/`, the merge-base tree was materialized with `git archive main src | tar -x -C <work>/base`, and the prototype tree with `git archive review-head src | tar -x -C <work>/judo` followed by the edits shown below.

Each probe was bundled once per tree and run with node:

```
./node_modules/.bin/esbuild <work>/scratch/probe.ts --bundle --platform=node --format=esm --outfile=<work>/scratch/probe.mjs
node <work>/scratch/probe.mjs
```

Runtime: Node v24.21.0. Focused test run on head:

```
./node_modules/.bin/vitest --run --project main --coverage.enabled=false \
  src/utils/body.test.ts src/utils/buffer.test.ts src/validator/validator.test.ts \
  src/request.test.ts src/middleware/method-override
→ Test Files 5 passed (5); Tests 155 passed | 1 skipped (156)
```

## F1 — `formData()` then `parseBody()` regresses

### Mechanism

1. `await c.req.formData()` on a fresh request sets `bodyCache.formData = raw.formData()`. The cache has exactly one key, `formData`.
2. `await c.req.parseBody()` reaches `parseFormData`, which calls `request.arrayBuffer()` → `#cachedBody('arrayBuffer')`.
3. `arrayBuffer` is not cached, but `formData` is, so `#cachedBody` takes the "derive from any cached key" branch at `src/request.ts:228-235` and evaluates `new Response(formData).arrayBuffer()`.
4. `new Response(formData)` serializes as `multipart/form-data` with a **new** boundary chosen by the runtime, regardless of what the original request's encoding was.
5. `parseFormData` passes those bytes to `bufferToFormData` together with the **original** `Content-Type` header — the old boundary for multipart requests, or `application/x-www-form-urlencoded` for urlencoded ones.

For multipart the boundary no longer matches and the platform parser throws. For urlencoded the multipart envelope is parsed as a query string, producing one nonsense key/value pair.

### Probe output

| Scenario | `main` (merge-base) | head |
| --- | --- | --- |
| multipart: `await req.formData(); await req.parseBody()` | `{"message":"hello"}` | **THROW** `TypeError: Failed to parse body as FormData.` (cause: `no boundary found in multipart body`) |
| urlencoded: `await req.formData(); await req.parseBody()` | `{"message":"hello"}` | `{"------formdata-undici-055429008490\r\nContent-Disposition: form-data; name":"\"message\"\r\n\r\nhello\r\n------formdata-undici-055429008490--\r\n"}` |
| app handler: `await c.req.formData(); await c.req.parseBody()` (multipart) | status 200, both values correct | **status 500**, `Internal Server Error` |

Verification status: **reproduced by execution on both trees.** The requests in these scenarios use ordinary lower-case `Content-Type` headers; header casing is not involved.

### Why the shipped tests do not catch it

`src/request.test.ts` exercises `parseBody()` followed by other accessors for non-form content types, and `parseBody()` alone for forms. Nothing calls `formData()` (or populates `bodyCache.formData` without `arrayBuffer`) before `parseBody()`. The new comment in `parseFormData` asserts a cache contract in the opposite direction (`parseBody` then `formData`), and that direction is not tested either.

### Minimal repair

Keep the PR's guard change and restore the accessor call for `HonoRequest`:

```ts
const formData = await (request as Request).formData()
```

On Node v24.21.0 this is sufficient for the PR's own goal: the probe shows the platform parser accepting mixed-case headers natively.

```
native Multipart/Form-Data formData()               => [["message","hello"]]
native Application/X-WWW-Form-Urlencoded formData() => [["message","hello"]]
```

Whether other runtimes need the normalization is question Q1 in the summary. If they do, use the restructuring below instead of the minimal repair.

## F2 — duplicated sequence, external cache writes, inconsistent cache contents

### The two copies side by side

`src/validator/validator.ts:113-127` (unchanged by the PR):

```ts
if (c.req.bodyCache.formData) {
  formData = await c.req.bodyCache.formData
} else {
  try {
    const arrayBuffer = await c.req.arrayBuffer()
    formData = await bufferToFormData(arrayBuffer, contentType)
    c.req.bodyCache.formData = formData
  } catch (e) { /* 400 */ }
}
```

`src/utils/body.ts:125-132` (new): the same three steps, without the cache check, storing a promise instead of a value, and needing `as unknown as FormData` to do so.

`BodyCache` is declared as `Partial<Body>` (`src/request.ts:20-27`), i.e. resolved values, while `#cachedBody` stores promises and treats entries as promises (`(bodyCache[anyCachedKey] as Promise<BodyInit>).then(…)`). That mismatch predates the PR, and the validator's resolved-value write is the outlier. The PR does not fix the type; it adds a third writer and a second cast to get around it.

### Measured consequences on head

| Scenario | `main` | head |
| --- | --- | --- |
| `parseBody()` twice — cache keys | `["formData"]` | `["arrayBuffer","formData"]` |
| `parseBody()` twice — same cached `FormData` object reused? | yes | **no** (re-parsed, cache overwritten) |
| `validator('form')` then `parseBody()` — same cached object reused? | yes | **no** (re-parsed) |
| `validator('form')` then `parseBody()` — cache entry shape before / after | value / value | value / **promise** |

Verification status: **reproduced by execution on both trees.**

The extra `arrayBuffer` cache entry is retained until the request object is collected. This was read from the cache keys, not measured as heap usage; on runtimes whose native `formData()` streams file parts, the peak-memory difference for large uploads could be larger than "one extra copy", but that was not measured (those runtimes were unavailable).

One genuine improvement should be acknowledged: because `arrayBuffer` is now the first cache key, a later `c.req.text()` / `c.req.arrayBuffer()` after `parseBody()` is derived from the original bytes rather than from a re-serialized `FormData`. The probe confirms that on head `parseBody()` followed by `text()` returns the original bytes for a mixed-case multipart request. The restructuring below keeps that property.

### Bundle effect (measurement, not a finding)

`src/utils/body.ts` is imported by `src/request.ts`, so its new import of `src/utils/buffer.ts` enters the core module graph.

```
esbuild src/index.ts --bundle --format=esm --minify
base: 18679 bytes minified, 7682 gzip, 24 input modules
head: 18933 bytes minified, 7771 gzip, 26 input modules  (+254 / +89; buffer.ts contributes 107 bytes, body.ts grows 856 → 1003)
```

Tree-shaking removes `crypto.ts`'s contribution in a bundle; unbundled ESM consumers now load `utils/buffer.ts` and `utils/crypto.ts` with the core. Small, but it is a cost paid for the duplicate rather than for the feature.

### Worked code-judo proposal

State one invariant in the class that owns the cache: *a `HonoRequest`'s `FormData` is always derived from its cached bytes, parsed with a normalized header.* Then every caller just awaits `formData()`.

`src/request.ts`:

```ts
import { bufferToFormData } from './utils/buffer'

formData(): Promise<FormData> {
  // Always derive FormData from the cached bytes: a FormData cannot be re-serialized losslessly
  return (this.bodyCache.formData ??= this.arrayBuffer().then((buffer) =>
    bufferToFormData(buffer, this.raw.headers.get('Content-Type') ?? '')
  ) as unknown as FormData)
}
```

(The remaining cast is the pre-existing `BodyCache` typing lie, now confined to the class that owns it. Typing `BodyCache` as a map of promises would remove it entirely and is worth doing in the same change.)

`src/utils/body.ts` — `parseFormData` shrinks, and the `bodyCache` write, the cast, the duplicate header lookup and the `|| ''` all go away:

```ts
const formData = await (request instanceof HonoRequest
  ? request.formData()
  : bufferToFormData(await request.arrayBuffer(), request.headers.get('Content-Type') ?? ''))
```

(If Q1 is answered "no runtime needs it", the second arm becomes `request.formData()` and the whole expression collapses back to `main`'s single call, with no `instanceof` and no `buffer` import.)

`src/validator/validator.ts` — the cache-check branch and the `bufferToFormData` import disappear:

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

Size of the prototype relative to head: 3 files, +14 / −23.

Probe results for the prototype:

| Scenario | prototype |
| --- | --- |
| multipart: `formData()` then `parseBody()` | `{"message":"hello"}` |
| urlencoded: `formData()` then `parseBody()` | `{"message":"hello"}` |
| `parseBody()` twice — same cached object | yes |
| `validator('form')` then `parseBody()` — same cached object, shape | yes, promise / promise |
| app handler `formData()` then `parseBody()` | status 200 |
| mixed-case multipart via `parseBody()` | `{"message":"hello"}` |
| mixed-case multipart via `validator('form')` | 200 `{"message":"hello"}` |
| malformed multipart via `validator('form')` | 400 `Malformed FormData request. Failed to parse body as FormData.` |
| `parseBody()` then `text()` returns original bytes | true |

Verification status: **probe-verified only.** The prototype lives outside the clone and was not run against the vitest suite; `src/request.test.ts` has assertions about cache contents (for example around line 580) that would need to be re-checked.

Trade-offs to decide consciously:

- `c.req.formData()` would always buffer the body as an `ArrayBuffer` first. Head already does this for `parseBody()` and `main` already does it for `validator('form')`; the prototype extends it to direct `c.req.formData()` calls. If that is unacceptable, keep the native `raw.formData()` call when the cache is empty and fall back to bytes only when another representation is already cached — still inside `HonoRequest`, still one owner.
- A cached-then-failed parse stays cached as a rejected promise, as it does on `main` for `raw.formData()`.
- The validator would wrap a cached parse failure in a 400 as well, where today it awaits a pre-populated cache entry outside the `try`. That is arguably a fix, but it is a behaviour difference.

### Dead weight inside `parseFormData`

- `src/utils/body.ts:125` recomputes `headers` with the same `instanceof` ternary `parseBody` evaluated at line 101, and re-reads the header `parseBody` already has in `contentType`. Passing the string down would do.
- `src/utils/body.ts:127`: `headers.get('Content-Type') || ''` — `parseFormData` is private and only reachable after `mediaType` matched one of two non-empty literals, so the fallback cannot fire. A silent fallback on an invariant that always holds hides the invariant.
- `if (formData) … return {} as T` at lines 134-138 is pre-existing and equally unreachable; not attributable to this PR, but it goes away for free if the function is touched.
