# Detail 01 — src/utils/body.ts and src/utils/buffer.ts

Range reviewed: `git diff main...review-head` (9728702..5226d41). Files stay small (body.ts 240 lines, buffer.ts 117), so no file-size concern.

## Finding A: `parseFormData` was rewritten to bypass `request.formData()`, and the rewrite is not needed to fix the reported bug

Evidence. Before the PR, `parseFormData` called `(request as Request).formData()` (src/utils/body.ts, old line 125). After the PR it reads `arrayBuffer()`, calls `bufferToFormData` with the raw header, and for `HonoRequest` hand-assigns the promise into the body cache with a double cast (src/utils/body.ts:125-131):

    const headers = request instanceof HonoRequest ? request.raw.headers : request.headers
    const arrayBuffer = await (request as Request).arrayBuffer()
    const formDataPromise = bufferToFormData(arrayBuffer, headers.get('Content-Type') || '')
    if (request instanceof HonoRequest) {
      request.bodyCache.formData = formDataPromise as unknown as FormData
    }

The bug in issue #5060 is in the guard in `parseBody` (the lowercase `startsWith` literals). The guard fix is the `mediaType` line at src/utils/body.ts:104-106 and is sufficient on its own for the platform here: I ran a probe under Node v24.21.0 (scratch script in the work directory, outside the clone) showing that `new Request(..., {headers: {'content-type': 'Application/X-WWW-Form-Urlencoded'}}).formData()` and a mixed-case `Multipart/Form-Data; boundary=...` Request and Response all parse correctly. The issue itself states the platform parser accepts these values. I could not run the Deno, Bun or Workers suites (unavailable), so whether some runtime needs the normalization is unverified; the PR does not claim that anywhere.

Problems this rewrite introduces:

1. Same "arrayBuffer -> bufferToFormData -> assign bodyCache.formData" sequence now exists twice (validator.ts:118-121 and body.ts:125-131) with different cache semantics: the validator stores a resolved `FormData`, parseBody stores a `Promise` forced through `as unknown as FormData`. `bodyCache` is typed `Partial<Body>` where `formData: FormData`, so the type lies in one of the two places. A reader must now know that the slot sometimes holds a promise and sometimes a value. (`HonoRequest.#cachedBody` stores promises through the same untyped slot, so the promise form is the real convention and the validator's resolved form is the odd one; the PR extends the cast-based convention instead of fixing it.)
2. The `request instanceof HonoRequest ? request.raw.headers : request.headers` expression is now duplicated inside the same file (body.ts:101 and body.ts:125).
3. A plain `Request` path now buffers the entire body into memory via `arrayBuffer()` rather than letting the platform stream-parse; HonoRequest already does this for the validator, but for plain Request this is a behavior change introduced silently under a "case-insensitive matching" title.
4. The existing "dot" test had to be rewritten (removal of the `vi.spyOn(req, 'formData')` mock in src/utils/body.test.ts) purely because the implementation no longer calls `formData()`. A test needing to change for a bug fix that should only touch a guard is a symptom of the over-wide change.

Code-judo proposal. Revert `parseFormData` to its prior body (one `await (request as Request).formData()` line, no cache assignment, no extra header lookup). The `mediaType` guard alone fixes parseBody. That deletes the duplicated header lookup, the cast, the second copy of the buffer/cache sequence, and the test rewrite. If a runtime genuinely needs normalized headers, put the single "get FormData with cache" operation in one place (for example a `HonoRequest`-level helper that the validator and parseBody both call) so the cache type and the buffering rule are owned once.

Verification status: read and executed. Targeted vitest run of body, buffer, validator and request tests passes (144 passed, 1 skipped). Necessity probe done on Node 24 only.

## Finding B: media-type normalization is implemented three different ways, and `bufferToFormData` now silently rewrites its input

Evidence.
- src/utils/body.ts:104 uses `contentType?.split(';')[0].trim().toLowerCase()` and exact equality.
- src/utils/buffer.ts:112 uses `contentType.replace(/^[^;]+/, (mediaType) => mediaType.toLowerCase())`.
- src/validator/validator.ts:24-26 uses `/i` on three regexes.

Three mechanisms for one concept ("the media type of this header, compared case-insensitively"). They already differ in edge behavior: the `split/trim` path trims leading whitespace, the `replace(/^[^;]+/)` path does not trim and lowercases whitespace along with the type, and the validator regexes accept different parameter shapes than parseBody (for example validator's multipart regex only allows an optional `boundary=` parameter while parseBody accepts any parameters). The previous code already had this divergence between parseBody and validator; the PR adds a third rule instead of consolidating.

`bufferToFormData` is a generic buffer utility in `utils/buffer.ts`. Making it rewrite the Content-Type before handing it to `Response` adds hidden behavior (a regex with a magic `^[^;]+` capture) to a function whose contract is "turn bytes plus content type into FormData". Given the Node probe above, the lowercase step is likely redundant, and nothing in the PR shows it is needed; the new buffer test only proves the output parses, which it would without the normalization on this platform.

Code-judo proposal. If normalization is kept anywhere, create one small exported helper (for example `getMediaType(contentType: string | null | undefined): string` in `utils/headers.ts` or `utils/buffer.ts`) that does `split(';')[0].trim().toLowerCase()`, and use it in parseBody and in the validator guard (validator then compares against a small set or keeps the regexes for parameter validation but is no longer the only place with its own rule). Drop the `buffer.ts` change entirely unless a failing runtime is demonstrated. That leaves one definition of "media type" in the codebase.

Verification status: read; divergence reasoned from the code; no runtime outside Node available.
