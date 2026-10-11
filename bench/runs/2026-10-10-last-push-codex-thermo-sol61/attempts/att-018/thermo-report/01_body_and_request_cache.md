# Body conversion and request cache

## Scope and judgment

The actionable finding is F1 in the summary. Its changed-code anchor is `src/utils/body.ts:125–130`. The important surrounding implementation is the unchanged `src/request.ts:220–238`, which defines the cache conversion policy, plus `src/validator/validator.ts:113–127`, which already has a separate buffer-to-form decoding/cache path.

The old body utility asked the request for FormData. The new utility asks for bytes, creates a Response through `bufferToFormData()`, and directly assigns the resulting promise to `request.bodyCache.formData`. This duplicates an orchestration responsibility already owned by HonoRequest, adds a request-class special case in a conversion utility, and requires `as unknown as FormData` to conceal the promise's actual type. The structural problem has a concrete behavioral consequence; it is not merely a preference for a different file layout.

## Source evidence and mechanism

At `src/utils/body.ts:126`, `arrayBuffer()` may mean HonoRequest's cache conversion, not the original wire bytes. `HonoRequest.#cachedBody()` returns a same-format cached body when present; otherwise, if some body is cached, it creates a new Response from that first cached representation. A FormData body is serialized as multipart with a newly generated boundary. No original Content-Type is attached to that Response.

At `src/utils/body.ts:127`, `bufferToFormData()` nevertheless receives the original request header. For an original multipart request, its old boundary cannot describe the newly serialized bytes. For an original urlencoded request, the multipart bytes are interpreted as a urlencoded string. At lines 128–130 the new result unconditionally replaces the cached FormData. A failed decode therefore poisons a previously working cache, and a successful wrong-format decode replaces it with incorrect fields.

If a byte cache also exists, as in the validator's standard population sequence, decoding can succeed. But the utility still ignores the cached form, decodes another object, and discards any changes made to the previous cached form. The base utility reused that cached form. This additional observation belongs to F1 because it follows from the same ownership mistake and has the same remedy.

## Dynamic verification

The probe source is `../review-probes/cache-probe.ts`; its exact result is `../review-probes/cache-probe-results.json`, relative to this report directory. The bundle is also preserved there. It runs on Node v24.21.0 and constructs Request objects locally; it makes no network calls.

The comparison extracts `git show main:src/utils/body.ts` to scratch as `base-body.ts`. Only its HonoRequest import is redirected to the checkout's request class. That class is unchanged in the reviewed diff, so the comparison isolates the utility change while preserving the actual cache implementation. It calls the exported base and head parseBody functions with HonoRequest instances; the public request method delegates directly to that utility.

Commands used, from the clone root:

```sh
timeout 300 ./node_modules/.bin/esbuild ../clone-work/review-probes/cache-probe.ts --bundle --platform=node --format=esm --outfile=../clone-work/review-probes/cache-probe.mjs
timeout 300 node ../clone-work/review-probes/cache-probe.mjs
```

The first draft of the scratch assertions compared a null-prototype body record with an ordinary object using Node's strict deep equality. That assertion failed on the expected prototype difference. The corrected probe compares own properties, and catches subsequent formData failures so that cache poisoning is recorded. The final probe passes all assertions. This was a probe correction, not a target-test failure.

| Scenario | Base | Head |
| --- | --- | --- |
| Multipart: formData then parseBody | `{ foo: 'bar' }`; same cached form | `TypeError: Failed to parse body as FormData.`; later formData also rejects |
| Urlencoded: formData then parseBody | `{ foo: 'bar' }`; same cached form | A multipart-framing field replaces `foo`; cached form replaced |
| Validator-shaped byte and form cache, then edit cached `foo` | `{ foo: 'edited' }`; same cached form | `{ foo: 'bar' }`; cached form replaced |
| Fresh mixed-case urlencoded, with charset | Not asserted in comparison | `{ foo: 'bar' }`; subsequent formData works; header unchanged |
| Fresh mixed-case multipart, boundary `sampleBoundary` | Not asserted in comparison | `{ foo: 'bar' }`; subsequent formData works; header unchanged |

The validator-shaped case follows the existing production sequence exactly: read request bytes, decode using the buffer helper, cache the resolved form, and then edit that form. It does not claim to be a separate full middleware integration test. The actual validator tests run in the focused selection recorded below.

The minimal regression trigger is:

```ts
const data = new FormData()
data.append('foo', 'bar')
const req = new HonoRequest(
  new Request('http://localhost/', { method: 'POST', body: data })
)
await req.formData()
await req.parseBody() // Base succeeds; reviewed head rejects.
```

Replacing `data` with `new URLSearchParams({ foo: 'bar' })` produces incorrect fields instead of throwing.

## Worked code-judo proposal

Give form decoding one owner: HonoRequest. Buffer normalization remains the existing utility's job, while body conversion remains the body utility's job. Once the cache already contains FormData, there is no reason to serialize and parse it again.

An honest cache contract can retain compatibility with both current producer shapes:

```ts
type BodyCache = Partial<{
  [K in keyof Body]: Body[K] | Promise<Body[K]>
}>
```

Readers must normalize cache hits with `Promise.resolve()`; updating the type alone is insufficient because the existing generic conversion branch assumes `.then()` is present. Keep that contract correction in the request layer, including the generic cache reader, rather than introducing a new promise-as-value assertion in a caller.

With that contract in place, the request's form method can be direct:

```ts
formData(): Promise<FormData> {
  const cached = this.bodyCache.formData
  if (cached) {
    return Promise.resolve(cached)
  }

  const pending = this.arrayBuffer().then((buffer) =>
    bufferToFormData(buffer, this.raw.headers.get('Content-Type') ?? '')
  )
  this.bodyCache.formData = pending
  return pending
}
```

Publishing the promise synchronously gives concurrent consumers one decode. Consulting the cache first preserves prior form reads and mutations. Obtaining bytes through the existing request method on cache misses retains body caching for later reads. This proposal requires importing the existing buffer helper into the request layer; the body/request import cycle already exists, so references must stay inside callable functions as they are today.

The conversion utility can then remove its cache write and promise cast:

```ts
const formData = request instanceof HonoRequest
  ? await request.formData()
  : await bufferToFormData(
      await request.arrayBuffer(),
      request.headers.get('Content-Type') ?? ''
    )
return convertFormDataToBodyData<T>(formData, options)
```

Both operations return FormData or reject. There is no need for the old truthiness guard and impossible empty-record fallback after a successful decode. The distinct branch for a plain Request is an actual interface boundary: it lacks Hono's reusable cache and still needs header normalization before platform decoding.

The validator can keep its existing admission regexes and HTTP 400 translation while replacing its cache-hit/cache-miss branches with:

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

This deletes duplicated decoding orchestration and direct cache manipulation from two callers. It does not merge the body utility's optional all/dot conversion with the validator's different field aggregation semantics. Those are distinct output policies and should remain distinct.

A smaller immediate repair is to have parseFormData return the existing `bodyCache.formData` when available, before asking for bytes, and ensure it does not overwrite that cache. That fixes the demonstrated failures with less scope, but leaves two places responsible for decoding/cache orchestration. The canonical request method is the stronger maintainability remedy.

These are design sketches. They have not been applied, compiled as a patch, or run against runtime suites. The actual reviewed behavior and regression probes have been executed. Validation of an implementation should include both body read orders, repeat parseBody with different all/dot options, cached form mutations, concurrent consumers, mixed-case headers, and unchanged malformed-body error handling. These checks all verify the cache ownership change rather than mirroring the proposed code.

## Focused target checks

```sh
timeout 300 ./node_modules/.bin/vitest --run --project main --coverage.enabled=false src/utils/body.test.ts src/utils/buffer.test.ts src/validator/validator.test.ts src/request.test.ts
```

Executed once, exit 0: four files passed, 144 tests passed, one test skipped, reported duration 317 ms. Existing coverage includes validator-to-parseBody and parseBody-to-validator flows, but not formData-first for form requests. The former already has original bytes cached; the latter populates bytes before FormData in the new implementation. Neither exposes the failing case where only a decoded form is available.

No runtime-specific behavior beyond the available Node runtime was verified. A full suite and TypeScript check were outside this focused execution. No remedy was applied to the checkout.
