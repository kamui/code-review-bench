# Thermo Nuclear Review — honojs/hono#5067

Range reviewed: `9728702911073aec5a63a3ba2840b7240e5d3205..5226d4165d48643586152614cbd07422a0ab7a22` (`git diff main...review-head`), 6 files.

The PR makes Content-Type media-type matching case-insensitive in `parseBody()`, `bufferToFormData()` and `validator()`. The case-insensitivity changes themselves are correct and well-tested. However, the PR also rewrites how `parseFormData()` obtains the form data: instead of calling `request.formData()` (which on a `HonoRequest` went through the body cache), it now calls `request.arrayBuffer()` and re-parses the bytes with `bufferToFormData()`. That rewrite introduces one confirmed functional regression in the `HonoRequest` body-cache contract.

The touched test files (`src/utils/body.test.ts`, `src/utils/buffer.test.ts`, `src/validator/validator.test.ts`) and `src/request.test.ts` pass on the head (144 passed, 1 skipped). The regression below is not covered by any test.

## Findings

### [Medium] `c.req.parseBody()` returns corrupted data or throws when `c.req.formData()` was called first on the same request

**File / lines:** `src/utils/body.ts:125-132` (new `parseFormData` body), interacting with `src/request.ts:220-239` (`#cachedBody`).

**Evidence traced.**
Before this PR, `parseFormData` did `await (request as Request).formData()`. For a `HonoRequest` that resolves to `HonoRequest.formData()` → `#cachedBody('formData')`, which returns the already-cached `FormData` promise directly (`src/request.ts:224-226`), so calling `c.req.formData()` and then `c.req.parseBody()` composed correctly.

After this PR, `parseFormData` does `await (request as Request).arrayBuffer()` and then `bufferToFormData(arrayBuffer, headers.get('Content-Type') || '')` using the *original* request Content-Type. When `bodyCache.formData` is the only cached entry (the state after a prior `c.req.formData()` call), `#cachedBody('arrayBuffer')` takes the "any cached key" branch at `src/request.ts:228-236` and re-serialises the cached `FormData` via `new Response(formData).arrayBuffer()`. That re-serialisation always produces a *multipart* body with a freshly generated boundary. `bufferToFormData` then parses those bytes against the original header:

- Original `application/x-www-form-urlencoded`: the multipart bytes are parsed as a urlencoded string, so `parseBody()` resolves to a garbage object whose single key is the multipart preamble (`------formdata-undici-…\r\nContent-Disposition: form-data; name`) instead of the submitted fields. No error is raised, so handlers silently see wrong data.
- Original `multipart/form-data; boundary=<old>`: the regenerated boundary does not match `<old>`, so `Response.formData()` rejects with `TypeError: Failed to parse body as FormData.`, which escapes `parseBody()` and turns into a 500 from the default error handler.

I confirmed this by executing both versions with esbuild-bundled scratch scripts (work dir `scratch/probe-base.mjs` and `scratch/probe-head.mjs`) through `new Hono().request(...)`, with a handler that calls `c.req.formData()` then `c.req.parseBody()`:

```
===== base (merge-base 97287029)
urlencoded / first=formData => {"ok":true,"body":{"message":"hello"}}
multipart  / first=formData => {"ok":true,"body":{"message":"hello"}}
===== head (5226d416)
urlencoded / first=formData => {"ok":true,"body":{"------formdata-undici-077621719993\r\nContent-Disposition: form-data; name":"\"message\"\r\n\r\nhello\r\n------formdata-undici-077621719993--\r\n"}}
multipart  / first=formData => {"ok":false,"err":"TypeError: Failed to parse body as FormData."}
```

The other orderings still work on the head (verified): `parseBody()` alone, `parseBody()` → `formData()` → `parseBody()`, `validator('form')` → `parseBody()` → `formData()` → `text()`, and `blob()` → `parseBody()`. `validator('form')` is unaffected because it caches `arrayBuffer` before `formData` (`src/validator/validator.ts:119-121`), so `Object.keys(bodyCache)[0]` is `arrayBuffer`. (`text()`/`json()` → `parseBody()` actually improves: it threw on the base and now works.)

**Why this matters.** `HonoRequest`'s body cache exists precisely so the body accessors can be called in any order, and `src/request.test.ts` ("req.formData()", "req.parseBody()") tests that contract for the other orderings. A middleware that inspects the form with `c.req.formData()` (e.g. for auditing, CSRF-token extraction, or routing) followed by a handler that uses the primary documented API `c.req.parseBody()` worked on the base and breaks on this head, silently for urlencoded bodies. This is a breaking change outside the stated intent of the PR (case-insensitive matching) and the author is very likely unaware of it, since the commit message and tests only cover media-type casing.

**Suggested remedy (not applied).** In `parseFormData`, when `request` is a `HonoRequest` and `request.bodyCache.formData` is already set, `await` that cached value and use it (mirroring the check at `src/validator/validator.ts:115-116`), falling back to the `arrayBuffer()` + `bufferToFormData()` path only when it is not. Alternatively prefer `bodyCache.arrayBuffer`/`blob`/`text` sources explicitly and never derive bytes from a cached `FormData`. Add a regression test covering `formData()` → `parseBody()` for both urlencoded and multipart.

## Reviewed and found acceptable

- `src/utils/body.ts:104-106`: replacing `startsWith` with an exact, lower-cased media-type comparison. Parameters (`; charset=…`, `; boundary=…`) are stripped before comparison, so no previously accepted real-world header is rejected. The new tests cover `Application/X-WWW-Form-Urlencoded` and `Multipart/Form-Data; boundary=…`.
- `src/utils/buffer.ts:113`: lower-casing only the part before the first `;` keeps the boundary parameter intact (boundaries are case-sensitive). Empty string input yields empty string, which is unreachable from `parseBody` because the guard runs first.
- `src/validator/validator.ts:24-26`: adding the `i` flag. The boundary character class is unchanged, and `c.req.json()` does not depend on header casing. Mixed-case `multipart` passes through to `bufferToFormData`, which now normalises before handing to `Response.formData()`.
- `src/utils/body.test.ts`: the removed `vi.spyOn(req, 'formData')` mock was the only `vi` usage and the file never imported `vi` explicitly (globals), so there is no unused-import lint fallout. The replacement test appends real entries and still exercises the `dot` path.
- New import `./buffer` into `body.ts` pulls `./crypto` transitively; no new import cycle (`buffer.ts` does not import `request.ts`) and no module-load side effects.
- No devex, environment, port, secret, or feature-gate changes.

## Other observations (not findings)

- After `c.req.parseBody()` the full request body is now retained in `bodyCache.arrayBuffer` for the lifetime of the request in addition to the parsed `FormData` (previously only the `FormData` was retained). Peak memory during parsing is unchanged because platform `formData()` implementations buffer the body anyway, but the retained footprint for large multipart uploads roughly doubles. The validator already behaved this way, so this is noted for awareness only.
