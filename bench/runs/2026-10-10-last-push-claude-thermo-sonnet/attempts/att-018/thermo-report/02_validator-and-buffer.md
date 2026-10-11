# Detail 02 — `src/validator/validator.ts` and `src/utils/buffer.ts`

## Finding 4 — three independent mechanisms for "case-insensitive media type" with no shared helper (body.ts:104, validator.ts:24-26, buffer.ts:113)

Status: verified by reading the diff. Behavior of each piece is covered by the added tests, which pass.

The same concept, "compare the media type case-insensitively but leave parameters alone", is implemented three ways in three files:

1. `body.ts` — `contentType?.split(';')[0].trim().toLowerCase()`, then equality.
2. `validator.ts` — the three regexes gain an `i` flag. This also makes the whole string, including parameter values, case-insensitive. That is harmless today only because the parameter classes already accept both cases.
3. `buffer.ts` — `contentType.replace(/^[^;]+/, (m) => m.toLowerCase())` inside `bufferToFormData`, a different regex-based extraction of the same media-type segment.

A reader has to hold three idioms in their head, and a future media type (e.g. a new `+json` rule) has to be taught three places. Because `parseBody` and `validator('form')` each decide "is this a form body?" separately, they can drift apart again. This is exactly the class of bug #5060 reported.

### Remediation
Add one small helper next to `bufferToFormData`, for example `getMediaType(contentType: string | null | undefined): string` returning the trimmed, lower-cased text before `;`. Then:

- `parseBody` becomes `const mediaType = getMediaType(contentType)` followed by the existing equality check.
- The validator can test equality against `mediaType`, and `application/(…+)?json` with one small `endsWith('json')` or regex check on the normalized media type. The three regexes lose their parameter-matching tails, which are the part that is hard to read.
- `bufferToFormData` should not normalize at all if `Response.formData()` already accepts mixed case. The probe showed `Request.formData()` does on Node 24, and `Response` goes through the same undici body mixin. Verified on Node only, so confirm on Workers, Bun and Deno before deleting.

## Finding 5 — `bufferToFormData` change is unneeded on the verified runtime and adds a hidden transform to a shared util (buffer.ts:112-113)

Status: partially verified. Node 24 accepts mixed-case `Content-Type` in `Request.formData()`; `Response.formData()` and the other runtimes were not run (runtime test suites are unavailable in this review).

`bufferToFormData` is a general-purpose helper. Silently rewriting its `Content-Type` argument adds a hidden normalization step to every caller (validator and now `parseBody`), duplicates what the platform already does, and is paired with a unit test that only proves the platform behavior. If the validator's guard is made case-insensitive (the `/i` regexes), the buffer change is dead weight for the validator path. If an old runtime really needs it, a comment naming that runtime should say so. As written, the comment says "normalize the media type", which describes the code and not the reason.

## Things checked and found fine

- `/i` on the three validator regexes is correct, and the new JSON, urlencoded and multipart validator tests pass.
- Multipart boundaries are preserved in every path: the lower-casing only touches text before the first `;`.
- Test run: `./node_modules/.bin/vitest --run --project main --coverage.enabled=false src/utils/body.test.ts src/utils/buffer.test.ts src/validator/validator.test.ts` → 106 passed, 1 skipped.
- No file crosses 1000 lines (`body.ts` 240, `validator.ts` 185).
