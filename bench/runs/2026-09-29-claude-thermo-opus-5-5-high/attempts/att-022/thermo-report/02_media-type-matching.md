# 02 — Media-type matching: three idioms, no shared helper

Scope: `src/utils/body.ts:104-106`, `src/utils/buffer.ts:110-114`, `src/validator/validator.ts:24-26`, and the
Content-Type checks elsewhere in `src/` that the diff did not touch.

## Finding 2.1 — One concept ("the case-normalized media type of a Content-Type") is implemented three different ways, and a sibling call site is left case-sensitive (CONFIRMED by reading)

The PR fixes the same bug, case-sensitive media-type matching, with three unrelated mechanisms:

| Site | Mechanism |
| --- | --- |
| `utils/body.ts:104` | `contentType?.split(';')[0].trim().toLowerCase()` followed by string equality |
| `utils/buffer.ts:113` | `contentType.replace(/^[^;]+/, (mediaType) => mediaType.toLowerCase())` (no trim) |
| `validator/validator.ts:24-26` | `/i` added to three regexes, which also makes parameter names and `boundary=` case-insensitive |

These rules are close but not the same. `body.ts` trims whitespace around the media type and `buffer.ts` does not.
The validator's `/i` flag also relaxes parameter-name matching, which the other two leave alone. A reader has to hold
three normalizations in their head to answer the question "which Content-Type values does Hono treat as form data?",
and the answer differs between `parseBody` and `validator('form')`. The validator still rejects
`multipart/form-data; charset=utf-8; boundary=x`, for example, and `parseBody` accepts it.

The fix is also incomplete across the codebase. `grep -rniE "multipart/form-data|x-www-form-urlencoded" src` shows
that `src/middleware/method-override/index.ts:74` and `:92` still gate on
`contentType?.startsWith('multipart/form-data')` and `startsWith('application/x-www-form-urlencoded')`. So the
`Application/X-WWW-Form-Urlencoded` request from issue #5060 is still ignored by `methodOverride({ form: ... })`.
Compare `src/middleware/csrf/index.ts:30`, which already uses an `/i` regex. The codebase now has at least four
spellings of this rule. That is the scattered-special-case growth the approval bar warns about: each call site
patches its own copy, with no shared definition.

**Worked code-judo proposal.** Introduce one tiny, named helper next to the other MIME utilities and route every
site through it:

```ts
// src/utils/mime.ts (already the home of mime lookup)
/** Lower-cased `type/subtype` of a Content-Type header, without parameters. */
export const getMediaType = (contentType: string | undefined | null): string | undefined =>
  contentType?.split(';', 1)[0].trim().toLowerCase() || undefined

/** Same header with only the media type lower-cased; parameters such as `boundary` are preserved. */
export const normalizeContentType = (contentType: string): string =>
  contentType.replace(/^[^;]*/, (t) => t.trim().toLowerCase())
```

- `parseBody`: `const mediaType = getMediaType(headers.get('Content-Type'))`. The two-way equality stays, or becomes
  a `Set` lookup shared with method-override.
- `bufferToFormData`: `'Content-Type': normalizeContentType(contentType)`. The inline regex and its comment move
  into the named helper.
- `validator`: match the regexes against `normalizeContentType(contentType)` and keep them case-sensitive. The
  regexes then document the accepted grammar, and case folding happens in exactly one place, instead of an `/i` flag
  that silently widens parameter matching as well.
- `method-override`: `const mediaType = getMediaType(contentType)` and equality checks, which closes the leftover
  instance of #5060.

This does not add a layer. It replaces three ad-hoc expressions, plus one missed site, with one definition that has a
name. A future "should `Text/Plain` count too?" question then has one answer in one place.

## Minor notes

- The behavior change from `startsWith('multipart/form-data')` to exact equality in `parseBody` is an improvement
  (`multipart/form-dataX` no longer matches). It is still worth a test, because it narrows what was accepted before.
- The two new mixed-case multipart tests (`body.test.ts`, `validator.test.ts`) build the body through a
  `source.arrayBuffer()` plus header `.replace(...)` dance. A tiny test helper such as
  `mixedCaseMultipart(formData)` would remove the duplication, though this is cosmetic.
