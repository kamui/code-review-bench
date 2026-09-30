# 02 — Content-Type media-type matching (`src/utils/body.ts`, `src/utils/buffer.ts`, `src/validator/validator.ts`, sibling consumers)

Review range: `9728702911073aec5a63a3ba2840b7240e5d3205..5226d4165d48643586152614cbd07422a0ab7a22`.

## Finding C1: One concept ("what is this request's media type?") gets three different ad-hoc implementations in three files, and a sibling consumer is left case-sensitive

**Severity:** spaghetti growth / missing helper. **Status:** verified by reading the code. The `method-override` gap was confirmed by reading; no probe was run for it.

Issue #5060 is about a single rule: media types are compared case-insensitively on the `type/subtype` part, and the parameters are left alone. The PR encodes that rule three separate times, each with a different mechanism:

- `src/utils/body.ts:104`: `contentType?.split(';')[0].trim().toLowerCase()`, then string equality against two literals.
- `src/utils/buffer.ts:113`: `contentType.replace(/^[^;]+/, (mediaType) => mediaType.toLowerCase())`. This lowercases in place and does **not** trim, so the result is a different normalization from the one in `body.ts`.
- `src/validator/validator.ts:24-26`: adds the `/i` flag to three full-header regexes. This makes the parameter-name and value portions case-insensitive too, and it leaves the existing explicit `A-Z` character classes redundant.

A reader now has to check three idioms to convince themselves they agree, and they do not fully agree. `parseBody` accepts `multipart/form-data; charset=utf-8; boundary=x` because it only looks at the media type. The validator's `multipartRegex` rejects it because it allows only a single `boundary` parameter. That gap predates the PR, but the PR had both classifiers open and made them *more* different in how they are written instead of converging them. In the same codebase, `src/middleware/method-override/index.ts:74` and `:92` still use `contentType?.startsWith('multipart/form-data')` and `startsWith('application/x-www-form-urlencoded')`. A form-based method override posted with `Application/X-WWW-Form-Urlencoded` is still silently skipped, so issue #5060's class of bug survives one directory over. `src/client/fetch-result-please.ts:78` shows a fourth, independently written case-insensitive JSON regex.

**Worked code-judo proposal.** Introduce one pure helper next to the existing MIME utilities (`src/utils/mime.ts` already owns `getMimeType`/`getExtension`):

```ts
// src/utils/mime.ts
/** Returns the lower-cased `type/subtype` of a Content-Type header, or undefined. */
export const getMediaType = (contentType: string | null | undefined): string | undefined =>
  contentType?.split(';', 1)[0].trim().toLowerCase() || undefined
```

Then:

- `parseBody` becomes `const mediaType = getMediaType(headers.get('Content-Type'))` with the same equality check, or better a `const FORM_MEDIA_TYPES = new Set([...])` that is shared with the validator.
- The validator's `form` guard becomes `FORM_MEDIA_TYPES.has(getMediaType(contentType))` plus, if the stricter parameter shape really is intended, a separately named parameter check. The case rule and the parameter-grammar rule should stop being fused into one regex that now needs `/i` to express the first of them. The `json` guard can match `getMediaType(contentType)` against `/^application\/([a-z.-]+\+)?json$/` with no `/i` and no parameter grammar.
- `method-override` switches its two `startsWith` calls to `getMediaType(...) === ...`.
- The `buffer.ts` rewrite either disappears (see `01_body-parsing.md` B2, Option A) or calls the same helper to rebuild the header, so no second normalization idiom exists.

The result is one tested function that owns the case rule. The three bespoke idioms go away, and the consumer the PR missed is covered by construction instead of by someone remembering to grep for `startsWith('multipart`.

**Remedy if the larger restructure is deferred.** At minimum, fix `method-override` in the same PR so the bug class is actually closed. Also make `buffer.ts` and `body.ts` share one expression rather than two subtly different ones (one uses `trim`, the other does not).

---

## Commands used

```
git diff main...review-head -- src/validator/validator.ts src/utils/body.ts src/utils/buffer.ts
grep -rnE "startsWith\('(application|multipart|text)|=== '(application|multipart|text)/|/\^(application|multipart|text)" src --include='*.ts' | grep -v '\.test\.'
  -> src/middleware/method-override/index.ts:74, :92 (case-sensitive startsWith)
  -> src/client/fetch-result-please.ts:78 (independent /i JSON regex)
grep -n export src/utils/mime.ts   # existing MIME helper module
```
