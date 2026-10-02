# Body parsing and request cache ownership

## Finding and evidence

The actionable finding is anchored at `src/utils/body.ts:125–132`, especially the unconditional byte read at line 126 and the direct cache assignment at line 130. The head replaces the base's single `await request.formData()` call with header selection, a byte read, a buffer decoder call, a HonoRequest-specific cache branch, a double assertion, and a second await. This makes the body-to-object utility another owner of form decoding and shared mutable cache state.

The existing canonical implementation is `HonoRequest.#cachedBody` at `src/request.ts:220–238`. `HonoRequest.formData()` at lines 329–330 uses it. If the requested format is already cached, it returns that cached result. If another format is cached, it converts the first cached body through `new Response(body)[key]()`. A cached FormData becomes newly serialized multipart bytes when arrayBuffer is requested. Those bytes carry a newly generated boundary, which does not match the original request header. With an original URL-encoded header, the same serialization is decoded as URL-encoded text instead of multipart. The new utility path invokes precisely this reconstruction and then pairs it with the original Content-Type.

The branch at lines 128–131 attempts to preserve the opposite call order by publishing FormData after consuming the bytes. It does not reuse an existing FormData value and cannot protect concurrent callers while the byte read is pending. The `as unknown as FormData` expression additionally hides a Promise in a cache slot declared as a concrete value. The cache's pre-existing type mismatch is not independently charged to this PR; the new writer makes it part of the new ownership problem.

## Reproduction and verification status

The retained scratch source is `../thermo-probes/cache-probe.ts`, with a baseline utility at `../thermo-probes/base-body.ts` and bundled executable at `../thermo-probes/cache-probe.mjs`. The baseline file was obtained with `git show main:src/utils/body.ts`. Its HonoRequest import was redirected to the checkout's unchanged request implementation so both utility versions use the same constructor and private cache behavior. No source behavior was altered in that baseline utility. The baseline and head request cache code were inspected directly and are identical in the relevant section.

For each utility version, the probe creates a fresh HonoRequest from a native Request, using either FormData or URLSearchParams with `foo=bar`. It exercises these orders separately: formData before parseBody; a mutation of the cached FormData before parseBody; concurrent parseBody and formData; parseBody before formData; and repeated parseBody with a different conversion option. Concurrent results are collected with Promise.allSettled so both operations are observed even when one rejects.

| Scenario | Base | Head |
| --- | --- | --- |
| Multipart: formData then parseBody | `{ foo: 'bar' }` | `TypeError: Failed to parse body as FormData.` |
| URL-encoded: formData then parseBody | `{ foo: 'bar' }` | Object whose key contains multipart delimiter and Content-Disposition framing; expected `foo` is absent |
| Multipart: mutate cached `foo` to `changed`, then parseBody | `{ foo: 'changed' }` | FormData parsing error |
| URL-encoded: mutate cached `foo` to `changed`, then parseBody | `{ foo: 'changed' }` | Multipart framing decoded into fields |
| Both encodings: concurrent parseBody and formData | Both fulfill with `foo=bar` | parseBody fulfills; formData rejects with unsupported Content-Type |
| Both encodings: parseBody then formData | Both succeed | Both succeed |
| Both encodings: repeated parseBody | Both succeed | Both succeed, but each invocation decodes again |

The multipart failure's underlying Node cause is `no boundary found in multipart body`. The concurrency failure is `Content-Type was not one of "multipart/form-data" or "application/x-www-form-urlencoded".` The latter follows because the arrayBuffer cache is installed immediately but the formData cache has not yet been assigned; `formData()` therefore tries to decode a Response made from the bytes without a Content-Type.

The probe also installs actual Hono middleware that awaits `c.req.formData()` and then calls next, followed by a handler that serializes `await c.req.parseBody()`. On the head, multipart returns HTTP 500 with `Internal Server Error`. URL-encoded returns HTTP 200, but its JSON key contains `Content-Disposition: form-data; name` and a generated multipart delimiter. The middleware example establishes a user-visible trigger beyond direct utility calls. The scratch script exits successfully because it records expected failing outcomes; its exit status is not evidence that these outcomes are correct.

The commands used from the clone root were:

```sh
./node_modules/.bin/esbuild ../clone-work/thermo-probes/cache-probe.ts --bundle --platform=node --format=esm --outfile=../clone-work/thermo-probes/cache-probe.mjs
node ../clone-work/thermo-probes/cache-probe.mjs
```

The actual invocation used the equivalent absolute work paths. It ran offline on Node v24.21.0. This verifies base-versus-head behavior on the allowed runtime. It makes no claim about results on Deno, Bun, Workers, or other unavailable platforms.

## Worked code-judo proposal

The useful ownership boundary is simple: HonoRequest owns consuming the request and memoizing decoded forms; the body utility owns converting an already decoded FormData into an object with `all` and `dot` semantics. Normalize headers at the decoder boundary with the existing `bufferToFormData()` helper. This retains the intended case fix while deleting the utility's new byte consumption, HonoRequest-specific cache write, and assertion.

For the canonical request path, use a promise-only cache contract. The following sketches the target design; it is a review proposal, not an applied or verified patch:

```ts
type BodyCache = Partial<{ [K in keyof Body]: Promise<Body[K]> }>

// HonoRequest.formData(): the arrayBuffer path preserves the original bytes.
formData(): Promise<FormData> {
  const cached = this.bodyCache.formData
  if (cached) {
    return cached
  }

  const pending = this.arrayBuffer().then((buffer) =>
    bufferToFormData(buffer, this.raw.headers.get('Content-Type') ?? '')
  )
  this.bodyCache.formData = pending
  return pending
}
```

`this.arrayBuffer()` installs its cache synchronously. The pending formData promise is then stored before this method returns and before its continuation decodes the buffer. Any later formData caller sees the same promise. A request that already has a cached FormData returns it directly instead of serializing it again. Repeated parseBody calls can apply different conversion options to the same decoded form without reparsing uploaded files.

The body utility can use a small adapter for its existing native-Request-or-HonoRequest input boundary:

```ts
const formData = request instanceof HonoRequest
  ? await request.formData()
  : await bufferToFormData(
      await request.arrayBuffer(),
      request.headers.get('Content-Type') ?? ''
    )

return convertFormDataToBodyData<T>(formData, options)
```

The native Request path still uses the normalized buffer decoder needed by the supplied case regressions. Native Requests have no Hono cache to repair. This explicit input distinction is at the ownership boundary rather than being a post-consumption cache patch.

The validator already has a bespoke cache-or-buffer acquisition branch at `src/validator/validator.ts:113–127`. Once formData acquisition is canonical, that branch can become `formData = await c.req.formData()` inside its malformed-body error handling. Keep the validator's own aggregation of repeated form keys; it has distinct semantics from the body utility's default conversion. A promise-only cache also requires removing the validator's concrete FormData write and reviewing the generic cache access types. These are related ownership edits, not separate findings against unchanged validator code.

This proposal removes two external cache-management paths and makes the same promise responsible for success and failure across callers. A narrower fix that merely checks the cache inside parseFormData addresses the sequential corruption but leaves consumption ownership distributed and needs separate concurrency handling. Centralizing the decoder removes the category of problem.

The restructuring should preserve valid-body semantics, case-sensitive boundaries, and the validator's documented malformed-body HTTP 400 behavior. Error behavior for a pre-existing rejected cache must be specified because the current validator's cached branch awaits outside its catch. Do not treat the sketch as proof of equivalence for every failure mode. Check `text()` and `arrayBuffer()` before form decoding, `cloneRawRequest()` after decoding, rejected bodies, and duplicate keys and uploaded File values before accepting a patch. The existing scratch comparison establishes the defect, not the proposed remedy.

## Actionable validation

Add exact-value assertions for formData-first and parseBody-first orderings with multipart and URL-encoded bodies, and for concurrent callers. Include cached FormData identity and mutation visibility so an implementation cannot pass simply by reconstructing superficially similar values. Keep the buffer fixture with an uppercase boundary letter and add equivalent integration coverage with mixed-case Content-Type. Assert actual fields and statuses in middleware composition. No checkout tests or implementation files were changed by this review.
