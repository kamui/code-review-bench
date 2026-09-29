# Detail 01 — `src/utils/body.ts`, `src/utils/buffer.ts` (parseBody path)

Range reviewed: `main...review-head` (one commit, 5226d416). Files stay far below 1k lines (`body.ts` is 240 lines), so the file-size rule does not apply.

## Finding A — `parseFormData` was rewritten to bypass `request.formData()`, and the new body-cache handoff breaks `formData()` followed by `parseBody()`

Location: `src/utils/body.ts:121-131`.

Before the change, `parseFormData` called `request.formData()`. On a `HonoRequest` that goes through `#cachedBody('formData')` (`src/request.ts:220-239`), so it shares the body cache with every other accessor. After the change, it calls `request.arrayBuffer()`, builds a `Response` by hand through `bufferToFormData`, and then writes the resulting promise into `request.bodyCache.formData` through `as unknown as FormData`.

That cache write is not what breaks. The break comes from asking for `arrayBuffer` when a different key is already cached. `#cachedBody('arrayBuffer')` sees `bodyCache.formData` and reconstructs bytes with `new Response(cachedFormData).arrayBuffer()`. Those bytes are a freshly generated multipart body with a new boundary. They are then re-parsed against the original request `Content-Type` header, which carries the old boundary or is urlencoded.

Probe, run on the head with `app.request` and a scratch bundle (`clone-work/probe.ts`):

- `await c.req.formData()` then `await c.req.parseBody()` with a multipart body: `parseBody` throws `TypeError: Failed to parse body as FormData.`
- The same sequence with an `application/x-www-form-urlencoded` body: no error, but `parseBody` returns one garbage key, `"------formdata-undici-...\r\nContent-Disposition: form-data; name"`, whose value is the raw multipart text. The handler silently gets corrupted data.
- `parseBody` twice, `parseBody` then `formData()`, and `validator('form')` then `parseBody()` all work, because `bodyCache.arrayBuffer` is already populated in those cases.

The old code returned the cached FormData in the `formData()`-then-`parseBody()` case. This is therefore a behavior regression introduced by a PR whose goal was case-insensitive matching, and no test covers it. It is exactly the "silently returns wrong data" failure class that issue #5060 complains about.

The design problem is the larger point. The PR's stated fix is a matching change, and it should have stayed a matching change. Instead it swaps the parse mechanism for every form request, including the already-working lowercase ones. It adds an arrayBuffer round trip and a second copy of the payload. It also introduces a hand-maintained cache write with a type lie: `bodyCache.formData` is typed `FormData`, but the code stores a `Promise<FormData>` behind a double cast. `request.ts` already stores raw promises there, so the type on `BodyCache` is the real inconsistency, but the cast papers over it and adds no clarity.

Code-judo proposal: keep `parseFormData` exactly as it was (`await request.formData()`), and make `HonoRequest.formData()` / the shared body path tolerate mixed-case media types in one place. For a plain `Request` the platform already parses `Application/X-WWW-Form-Urlencoded`, per the issue's own reproduction, so no rewrite is needed. If a runtime turns out not to accept mixed case for multipart, the fix belongs in one shared "get FormData from request" helper (canonical home: next to `bufferToFormData` in `utils/buffer.ts`, which the validator already uses), and only in the fallback path. Then the cast, the `instanceof HonoRequest` branch, the duplicated header lookup, and the cache handoff all disappear.

Verification status: reproduced by scratch probe on the head (outputs above). The base-side behavior is inferred from the code (`request.formData()` returns the cached value at `request.ts:224`), not run, because the clone is read-only.

## Finding B — three independent case-normalization mechanisms for one concept

Locations: `src/utils/body.ts:104-106` (split, trim, lowercase, and strict equality), `src/utils/buffer.ts:113` (regex replace of the leading segment), `src/validator/validator.ts:24-26` (three regexes given an `i` flag).

The same idea, "compare only the media-type portion case-insensitively", is now implemented three different ways in three files. `parseFormData` also re-derives the header value that `parseBody` already read. There is no shared `getMediaType(contentType)` helper. `src/utils/headers.ts` or `utils/body.ts` would be the natural home for one.

Behavioral divergence is already visible. `parseBody` trims whitespace and uses exact equality on the media type. The validator regexes forbid whitespace before `;` and allow only a restricted parameter grammar, so `multipart/form-data ; boundary=x` passes `parseBody` and is skipped by `validator('form')`. The regexes now also match parameter names case-insensitively, which the issue did not ask for and is harmless but unexamined.

Code-judo proposal: one exported `getMediaType(header?: string | null): string` returning the lowercased, trimmed portion before `;`. Then `parseBody` compares `getMediaType(...)` to the two literals, and the validator drops its multipart/urlencoded regexes in favor of the same comparison. That is roughly five lines and deletes the three regexes' redundant boundary/parameter grammars. Keep the JSON matcher as a suffix check (`/^application\/([a-z-.]+\+)?json$/` applied to the media type only). The existing regex for JSON `+json` suffixes is the only one that needs pattern matching.

Caveat: dropping the parameter grammar from the validator's regexes changes which malformed parameter strings are rejected. That is a behavior decision, so the change needs an explicit call rather than a silent one.

Verification status: reasoning from the code; not run. The `multipart/form-data ; boundary=x` divergence follows directly from the regex text at `validator.ts:25`, which has no `\s*` before `;`.

## Finding C — `bufferToFormData` silently reformats a header for every caller

Location: `src/utils/buffer.ts:113`.

The regex `/^[^;]+/` lowercases the media type before constructing the `Response`. It is behaviorally harmless, and it does preserve the boundary parameter. It is also a hidden transformation inside a helper whose name suggests pure conversion. With Finding A resolved by not calling this from `parseFormData`, the only remaining consumer is the validator, which is the one place that legitimately needs it. The comment "Normalize the media type" would be better expressed by having the caller pass a normalized header. Low priority once B lands.

Verification status: reasoned from the code; buffer unit test added by the PR passes (`src/utils/buffer.test.ts`).

## Test observations

- Focused run on the head: `body.test.ts`, `buffer.test.ts`, `validator.test.ts`, `request.test.ts` — 144 passed, 1 skipped.
- The PR deleted a `vi.spyOn(req, 'formData')` mock in the `dot: true` test and replaced it with real FormData. That is an improvement, but it exists only because the rewritten `parseFormData` no longer calls `formData()`. It is another sign that the rewrite changed the seam that tests relied on.
- Codecov reports 4 uncovered patch lines (3 in `body.ts`, 1 in `buffer.ts`). The `instanceof HonoRequest` cache branch and the `|| ''` fallback are the likely gaps. There is no test for `formData()` then `parseBody()`, the exact sequence that regressed.
