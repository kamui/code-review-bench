# 02 — Media-type matching

Covers finding F3 and question Q2 of `summary.md`. Files: `src/utils/body.ts`, `src/utils/buffer.ts`, `src/validator/validator.ts`, `src/middleware/method-override/index.ts`.

## Three normalizers for one rule

The rule from the originating issue is a single sentence: the media-type portion of `Content-Type` is case-insensitive; parameter values are not. The PR encodes it three times.

| Site | Mechanism | What it yields |
| --- | --- | --- |
| `src/utils/body.ts:104-106` | `contentType?.split(';')[0].trim().toLowerCase()` then `===` against two literals | boolean guard on the essence only; parameters ignored |
| `src/utils/buffer.ts:113` | `contentType.replace(/^[^;]+/, (mediaType) => mediaType.toLowerCase())` | whole header with the essence lower-cased, parameters untouched, no trimming |
| `src/validator/validator.ts:24-26` | `/i` appended to `jsonRegex`, `multipartRegex`, `urlencodedRegex` | boolean guard on the full header grammar, parameters included |

None of the three is wrong. The `/i` flag also makes the `boundary=` parameter name and the `[a-z-\.]+\+` structured-syntax prefix case-insensitive, which is correct (parameter names are case-insensitive) and was confirmed harmless: `APPLICATION/VND.API+JSON` now passes `jsonRegex`. `bufferToFormData` rightly leaves the boundary value alone, and the new test in `src/utils/buffer.test.ts` pins that.

The maintainability problem is that the same idea has three spellings with three slightly different edges, and no name. A reader who wants to know "how does Hono decide a request is a form?" has to find and reconcile all of them.

## The guards already disagree

Evaluated with the exact expressions from head (scratch probe, Node v24.21.0):

| `Content-Type` value | `parseBody` guard | validator form guard | validator json guard |
| --- | --- | --- | --- |
| `multipart/form-data ; boundary=X` | accept | reject | reject |
| `application/x-www-form-urlencoded ; charset=utf-8` | accept | reject | reject |
| `application/x-www-form-urlencodedfoo` | reject | reject | reject |
| `APPLICATION/VND.API+JSON` | reject | reject | accept |

The first two rows differ because `parseBody` trims around the essence while the validator regexes do not allow whitespace before `;`. That divergence existed in spirit on `main` (prefix match versus regex), so it is not a regression; the point is that this PR was the natural moment to give the essence check one home, and instead it added a third variant.

Verification status: **computed from head's expressions by execution.** The table does not claim any of these headers occur in practice.

## Q2 — prefix match became exact match

`main`:

```ts
contentType?.startsWith('multipart/form-data') ||
contentType?.startsWith('application/x-www-form-urlencoded')
```

head:

```ts
mediaType === 'multipart/form-data' || mediaType === 'application/x-www-form-urlencoded'
```

A value such as `application/x-www-form-urlencodedfoo` was parsed as a form on `main` and yields `{}` on head. This is very likely the better behaviour, but it is a semantic tightening that the PR title and tests do not mention. If it is intended, one negative test would pin it.

## The sibling that was missed: `method-override`

`src/middleware/method-override/index.ts`:

```ts
// line 74
if (contentType?.startsWith('multipart/form-data')) {
…
// line 92
if (contentType?.startsWith('application/x-www-form-urlencoded')) {
  const params = await parseBody<Record<string, string>>(clonedRequest)   // line 93
```

These are the same lower-case prefix guards the PR removed from `parseBody`, sitting in front of a call to `parseBody`.

Probe on head (identical on the merge-base, so this is a missed site rather than a regression):

```
POST /posts, body `_method=DELETE`
  Content-Type: application/x-www-form-urlencoded     → handled as DELETE
  Content-Type: Application/X-WWW-Form-Urlencoded     → handled as POST
```

Verification status: **reproduced by execution on both trees** for the urlencoded branch. The multipart branch at line 74 uses the same guard shape and was confirmed by reading, not by execution.

A repository-wide search of non-test sources for `Content-Type` matching shows the remaining request-side sites are `src/middleware/csrf/index.ts:29-30` (already `/i`) and the three sites this PR touched; `pretty-json` and `ssg` inspect response headers that Hono itself sets. So `method-override` is the one remaining request-side holdout.

## Worked proposal

Add one pure helper beside `bufferToFormData` (or in `src/utils/body.ts` if the `buffer` import is removed per `01_body_parsing_and_cache.md`):

```ts
/** Lower-cased media type of a Content-Type header value, without parameters. */
export const getMediaType = (contentType: string | null | undefined): string | undefined =>
  contentType?.split(';', 1)[0].trim().toLowerCase()
```

`parseBody`:

```ts
const mediaType = getMediaType(headers.get('Content-Type'))
if (mediaType === 'multipart/form-data' || mediaType === 'application/x-www-form-urlencoded') {
  return parseFormData(request, { all, dot })
}
```

`method-override`:

```ts
const mediaType = getMediaType(c.req.header('content-type'))
if (mediaType === 'multipart/form-data') { … }
if (mediaType === 'application/x-www-form-urlencoded') { … }
```

`bufferToFormData` keeps its in-place rewrite, because it must hand the platform a full header with parameters intact; that is a different operation from classifying the request and does not need to share code with the helper.

The validator's three regexes stay as they are with `/i`. They validate the parameter grammar as well as the essence, which is a deliberately stricter contract; a one-line comment above them saying so would stop the next reader from "unifying" them with the essence helper and loosening validation by accident.

Tests to add with the helper: mixed-case `method-override` for the urlencoded and multipart branches, and the negative case from Q2.

Verification status of the proposal: **not prototyped**; it is a mechanical extraction of the expression already on `src/utils/body.ts:104`.

## Test duplication (note only)

`src/utils/body.test.ts` and `src/validator/validator.test.ts` each hand-build a mixed-case multipart request with the same six-line sequence (create a `FormData` request, read its generated header, replace the media type, rebuild from `arrayBuffer()`). If more mixed-case tests are added for `method-override`, lift that into a shared test helper rather than pasting it a third time.
