# Thermo-nuclear code quality review — honojs/hono#5067

Range: `9728702911073aec5a63a3ba2840b7240e5d3205..5226d4165d48643586152614cbd07422a0ab7a22` (1 commit, 6 files, +107/−18).
Subject: case-insensitive `Content-Type` media-type matching in `parseBody()`, `validator()` and `bufferToFormData()` (issue #5060).

## Verdict

**Changes requested.** The guard and regex changes are small and correct. The `parseFormData` rewrite in `src/utils/body.ts` is not. It stops going through `HonoRequest`'s body cache and hand-writes `bodyCache.formData` from a utility module. That introduces a confirmed runtime regression: calling `c.req.formData()` and then `c.req.parseBody()` now throws for multipart bodies and silently returns garbage for urlencoded bodies. It also duplicates form-parsing and caching logic that the validator already had, while leaving the canonical `c.req.formData()` unnormalized. A code-judo move is available: make `HonoRequest.formData()` the single owner of "body → FormData". It deletes the cast, the cache write and the validator's cache branch, and fixes all three entry points at once. I checked this in a scratch copy.

No file-size concerns: body.ts is 240 lines, buffer.ts 117, validator.ts 185. Tests in the three changed test files pass (106 passed, 1 skipped).

## Findings

### 1. `parseFormData` bypasses the body cache and breaks `formData()` → `parseBody()` (regression, confirmed)

In `src/utils/body.ts:125–132`, `parseFormData` now calls `request.arrayBuffer()` and parses the bytes with the request's original `Content-Type`. When the body was already read with `c.req.formData()`, `#cachedBody('arrayBuffer')` in `src/request.ts:220–239` gets its bytes by re-serializing the cached `FormData` with `new Response(formData).arrayBuffer()`, which creates a new multipart boundary. Parsing those bytes against the old header fails for multipart (`TypeError: Failed to parse body as FormData.`, HTTP 500 in a probe app). For urlencoded it quietly produces a single garbage key containing the whole multipart envelope. It then overwrites `bodyCache.formData` with the rejected promise. On base, both cases returned `{ a: '1' }`, because `parseFormData` called `request.formData()` and got a cache hit. I confirmed this by running the same probe against `git archive main` and against the head. No test covers this order. The PR's removal of the `vi.spyOn(req, 'formData')` mock is a symptom that parsing moved off the cache-owning path. The fix is the restructuring in finding 2 plus a regression test for "formData first, then parseBody" in both encodings. Evidence and probe output: `01_form-body-parsing.md`, Finding 1.1.

### 2. Form parsing and cache management are duplicated in utilities instead of living in `HonoRequest.formData()` (structural, confirmed)

The validator already had a hand-rolled "arrayBuffer → `bufferToFormData` → write `c.req.bodyCache.formData`" sequence (`src/validator/validator.ts:115–127`). This PR adds a second copy in `src/utils/body.ts:125–131`, including an `as unknown as FormData` cast so a shared utility can store a promise into `HonoRequest`'s private-in-spirit cache. The two copies even disagree on what they store: the validator stores a resolved `FormData`, while body.ts stores a promise. Meanwhile the user-facing `c.req.formData()` still goes straight to `raw.formData()` with the unnormalized header. On a runtime whose `formData()` is case-sensitive (Node 24 is not, as I checked), the issue therefore stays unfixed for the most direct API. The code-judo move is to make `HonoRequest.formData()` compute `bufferToFormData(await this.arrayBuffer(), this.header('Content-Type') ?? '')` once and cache it with `??=`. Then `parseFormData` becomes "`request.formData()` for a HonoRequest, `bufferToFormData` for a raw Request", and the validator's form branch collapses to `await c.req.formData()` inside its existing try/catch. That removes the cache write, the cast, the validator's `if (bodyCache.formData) … else …` branch and its `bufferToFormData` import. It keeps the incidental head improvement where `text()` followed by `parseBody()` now works. I checked the proposal in a scratch copy: all ten order/encoding probes return 200 with `{ a: '1' }`, malformed multipart still yields the validator's 400, and a mixed-case urlencoded body validates. Worked diff and results: `01_form-body-parsing.md`, Finding 1.2 and "Worked code-judo proposal".

### 3. The media-type rule is written three different ways with no shared helper (maintainability, confirmed by reading)

The PR spells "compare the media type before `;` case-insensitively" three ways. `src/utils/body.ts:104` uses `split(';')[0].trim().toLowerCase()`, `src/utils/buffer.ts:113` lowercases via a `^[^;]+` regex replace without trimming, and `src/validator/validator.ts:24–26` adds `/i` to full-header regexes. Meanwhile `src/utils/mime.ts:18–26` already has the parameter-stripping idiom (`split(';', 1)[0].trim()`) in `getExtension`. Each site is correct on its own, but the concept has no name or home, so the next content-type guard will add a fourth variant. The acceptance rules of `parseBody` and `validator('form')` also still diverge visibly (any multipart parameters vs. only `; boundary=`). Add a `getMediaType()` helper to `src/utils/mime.ts` and use it in `parseBody`. Once finding 2 lands, keep `bufferToFormData` as the single normalization point on the parse side. Optionally drive the validator's form gate from the same helper. Proposal: `02_media-type-normalization.md`, Finding 2.1.

## Remediation sequence

1. Add failing regression tests first. On a `HonoRequest`, `await req.formData()` and then `parseBody()` must return the fields, for both multipart and urlencoded. Also add `await req.text()` followed by `parseBody()` to keep head's incidental fix.
2. Move "body → FormData" into `HonoRequest.formData()` (the `bufferToFormData` + `??=` cache version). Point `parseFormData` and `validator('form')` at it and delete the manual `bodyCache.formData` writes and the cast. Optionally retype `BodyCache` as a map of promises so the remaining casts go away too.
3. Add a mixed-case test that goes through `c.req.formData()` directly, since that API now shares the fix.
4. Introduce `getMediaType()` in `src/utils/mime.ts` and use it in the `parseBody` guard. Consider aligning the validator's form gate with it.

## Detail files

- `01_form-body-parsing.md`: findings 1 and 2, with probe output for base vs. head vs. the proposal, and the worked diff.
- `02_media-type-normalization.md`: finding 3, with the helper proposal and notes on the regex `/i` changes.

## Method

I read the full diff (`git diff main...review-head`) plus `src/request.ts`, `src/utils/mime.ts` and the surrounding code. I ran `vitest --run --project main --coverage.enabled=false` on the three changed test files, which passed. I wrote scratch probes under `clone-work/scratch/`, bundled them with the clone's esbuild and ran them with Node v24.21.0 against head, against base (`git archive main src`) and against a patched copy of head that implements the proposal. The clone was not modified.
