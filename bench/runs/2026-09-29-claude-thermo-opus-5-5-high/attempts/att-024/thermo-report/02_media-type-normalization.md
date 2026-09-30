# 02 — Media-type normalization: three spellings of one rule

Scope: `src/utils/body.ts:104–106`, `src/utils/buffer.ts:112–113`, `src/validator/validator.ts:24–26`, and the existing helper `src/utils/mime.ts:18–26` (`getExtension`).

## What the PR changed here

The issue asks for one rule: compare the media type (the part before `;`) case-insensitively, and leave parameters such as `boundary` alone. The PR implements that rule three times, with three different mechanisms:

- `body.ts:104`: `contentType?.split(';')[0].trim().toLowerCase()`, then exact equality against two literals.
- `buffer.ts:113`: `contentType.replace(/^[^;]+/, (mediaType) => mediaType.toLowerCase())`, which lowercases in place without trimming.
- `validator.ts:24–26`: adds the `/i` flag to the three full-header regexes. This also makes the parameter *names* (`boundary=`) and the JSON structured-suffix prefix (`[a-z-\.]+\+`) case-insensitive. Both are fine per RFC 9110, but it is a broader change than the one in the other two places.

Each of these is locally correct. I checked the regex edits: the multipart boundary character class already contained `A-Z`, so `/i` does not widen what a boundary may contain in any meaningful way, and all three changed test files pass (`vitest --run --project main` on body/buffer/validator tests: 106 passed, 1 skipped).

## Finding 2.1 — The "media type of a Content-Type header" concept has no home, so each call site reinvents it

**Verification status: CONFIRMED by reading.** This is a maintainability finding, not a behavior bug.

`src/utils/mime.ts` already owns MIME concepts, and `getExtension` there already strips parameters with `mimeType.split(';', 1)[0].trim()`. The PR adds another inline copy of the same parsing in `body.ts`, adds a regex-based variant in `buffer.ts`, and encodes the rule a third way in the validator regexes. The next time someone touches content-type matching (for example `c.req.json()` guards, or the compress/etag middleware), there will be no obvious helper to reuse and a fourth idiom will appear. The small inconsistencies are already visible: `body.ts` trims and `buffer.ts` does not, and `body.ts` uses `split(';')` while `mime.ts` uses `split(';', 1)`.

## Worked proposal

Add one named helper next to the existing MIME code:

```ts
// src/utils/mime.ts
export const getMediaType = (contentType: string | null | undefined): string | undefined =>
  contentType?.split(';', 1)[0].trim().toLowerCase()
```

- `parseBody` gate: `const mediaType = getMediaType(headers.get('Content-Type'))`. The comparison with the two literals is unchanged.
- `getExtension` can use the same parameter-stripping step for its `baseType`. It must not lowercase there unless that is intended; keep the helper split into `stripParams` / `getMediaType` if needed.
- `bufferToFormData` keeps its in-place lowercasing, since it must preserve the parameters. Once finding 1.2's remedy lands, it is the single place that converts bytes to `FormData`, so its normalization is the only one on the parse side. Add a comment that points to the shared rule.
- The validator regexes can stay (they check parameter grammar too, which a media-type helper does not). The `/i` flag is acceptable there. Alternatively, test `getMediaType(contentType)` against a small set for the form branch and keep the regex only for JSON's `+json` suffix. That makes the parseBody and validator acceptance rules visibly the same, where today they differ: parseBody accepts any multipart parameters, while the validator only accepts `; boundary=` with at most one space.

This is lower priority than 01. It deletes no branches, but it gives the concept one name and one location, so the next change doesn't grow a fourth copy.
