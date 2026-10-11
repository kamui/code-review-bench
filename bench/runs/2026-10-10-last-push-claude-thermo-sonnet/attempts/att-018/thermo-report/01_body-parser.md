# Detail 01 — `src/utils/body.ts` (parseBody / parseFormData)

Range reviewed: `9728702911073aec5a63a3ba2840b7240e5d3205..5226d4165d48643586152614cbd07422a0ab7a22` (`git diff main...review-head`). File size is 240 lines after the change, so there is no file-size concern.

## Finding 1 — `parseFormData` was rewritten to bypass `request.formData()`, and the rewrite regresses `formData()` followed by `parseBody()` (body.ts:124-132)

Status: **verified by execution** (Node v24.21.0, real `Hono` app bundled with esbuild from both `main` and `review-head`).

### What changed
Before, `parseFormData` was one line: `const formData = await (request as Request).formData()`. For a `HonoRequest` that call goes through `#cachedBody('formData')`, so it reuses any cached form data. The head replaces it with:

- `request.arrayBuffer()`
- `bufferToFormData(arrayBuffer, headers.get('Content-Type') || '')`
- a write of the resulting promise into `request.bodyCache.formData`, cast through `as unknown as FormData`

The issue (#5060) only needed the media-type *guard* fixed. Parsing was never broken. A scratch probe on Node 24 shows the platform accepts mixed-case media types in both paths:

- `new Request(..., 'Multipart/Form-Data; boundary=...').formData()` returned `[['a','b']]`.
- `new Request(..., 'Application/X-WWW-Form-Urlencoded').formData()` returned `[['a','b']]`.

So the parse-path rewrite is not required for the fix on this runtime. Other runtimes were not tested.

### Regression
`HonoRequest.#cachedBody` (src/request.ts:220-239) serves a missing key from *another* cached key by wrapping the cached value in `new Response(body)[key]()`. If `formData` is the only cached key, `arrayBuffer()` is rebuilt by re-serializing the `FormData` through `Response`. That produces a **new multipart boundary** and always multipart encoding. `parseFormData` then parses those bytes with the **original** request `Content-Type`, whose boundary or encoding no longer matches.

Probe handler, run against both trees:

```ts
app.post('/a', async (c) => { await c.req.formData(); return c.json(await c.req.parseBody()) })
```

| Request | `main` | `review-head` |
| --- | --- | --- |
| `multipart/form-data` body | 200 `{"k":"v"}` | **500** `Failed to parse body as FormData.` |
| `application/x-www-form-urlencoded` body | 200 `{"k":"v"}` | **200 with garbage**: `{"------formdata-undici-...\r\nContent-Disposition: form-data; name":"\"k\"\r\n\r\nv\r\n------formdata-undici-...--\r\n"}` |

The urlencoded case is worse than the crash: it silently returns a corrupted object instead of failing. Calling `parseBody()` first, `parseBody()` twice, and `validator('form')` then `parseBody()` all still work, because `arrayBuffer` is already cached on those paths. So the existing and added tests (body, buffer, validator) all pass (106 passed, 1 skipped) and none exercise the failing ordering.

The existing test that mocked `req.formData` with a fake object was deleted and rewritten around a real `FormData`. That is a reasonable cleanup in itself. But it removed the only test coupling `parseBody` to the `formData()` cache, and nothing replaced it.

### Remediation (code-judo)
Revert `parseFormData` to its original single call, `await (request as Request).formData()`, and keep only the guard change. That deletes the `HonoRequest` branch, the second header lookup, the `bodyCache` write and the cast. It also removes the buffering path and the regression. Add one test: `await c.req.formData(); await c.req.parseBody()` for both multipart and urlencoded bodies.

If a future runtime really cannot parse mixed-case media types in `formData()`, handle that once in a shared helper (see detail 02) rather than by changing the parse path of `parseBody`.

## Finding 2 — cast-through-`unknown` write to `bodyCache.formData` (body.ts:128-131)

Status: verified by reading `src/request.ts` and `src/validator/validator.ts`.

`request.bodyCache.formData = formDataPromise as unknown as FormData` stores a `Promise<FormData>` in a slot typed `FormData`. `validator.ts` stores a *resolved* `FormData` in the same slot (`c.req.bodyCache.formData = formData`) and reads it back with `await`. The same field therefore holds a promise on one path and a value on the other, and each side is held together by `await`. The `unknown` cast is the visible symptom of an unclear invariant: "what does `bodyCache.<key>` hold?" `#cachedBody` itself stores promises (`bodyCache[key] = raw[key]()`), so the *slot type* is what is wrong. This PR adds a second cast-through-unknown writer instead of going through `request.formData()`, which is the canonical writer. Reverting Finding 1 removes this finding with it.

## Finding 3 (minor) — guard semantic tightening is not called out

`startsWith('multipart/form-data')` became exact equality on the lower-cased, trimmed media type, so `multipart/form-data-foo` is no longer routed to form parsing. That is arguably correct, but it is a behavior change and has no test. If it is kept, a one-line test should pin it.
