# Detail 02 — `src/validator/validator.ts`

## Finding 5: hand-written regexes were patched with `/i` instead of a media-type parse

Location: `src/validator/validator.ts:24-26`.

Adding the `i` flag to `jsonRegex`, `multipartRegex`, and `urlencodedRegex` fixes the reported inputs (`Application/JSON`, `Application/X-WWW-Form-Urlencoded`, `Multipart/Form-Data`). It is the minimal edit and behaves correctly for the cases in the issue; the new tests in `validator.test.ts` pass. The maintainability problem is what it leaves in place: the whole-header anchored regexes with parameter grammars (`(;\s*[a-zA-Z0-9\-]+\=([^;]+))*`), which are the reason this code is brittle in the first place, remain. The flag also widens matching beyond the media type: parameter names such as `Boundary=` and `Charset=` are now accepted case-insensitively, and the `[a-z-\.]+` suffix class now admits upper-case structured-syntax names. The issue asked for case-insensitivity of the media-type portion only. That may be harmless (parameter names are case-insensitive per RFC 9110), but the PR does not say it was intended, and there are no tests for it.

This is the same missed opportunity as Detail 01 Finding 3: `body.ts` and `validator.ts` now solve the same normalization two different ways. The validator switch only needs the media type to answer "is this JSON / multipart / urlencoded"; a shared `getMediaType()` helper would let all three regexes become three string comparisons, and the parameter grammar (which the platform parser validates anyway) can go away.

Worked proposal:

```ts
const mediaType = getMediaType(c.req.header('Content-Type'))
case 'json':
  if (!mediaType || !(mediaType === 'application/json' || /^application\/[a-z0-9.+-]+\+json$/.test(mediaType))) break
case 'form':
  if (mediaType !== 'multipart/form-data' && mediaType !== 'application/x-www-form-urlencoded') break
```

Caveat: dropping the parameter grammar loosens what the validator accepts (for example a malformed parameter list would now reach the parser and fail with a 400 rather than skip). That is a behavior change that needs its own decision; if it is unwanted, keeping the `/i` regexes is the defensible minimum and this finding is a low-priority cleanup.

## Finding 6 (test-shape): multipart tests re-implement the same header rewriting inline

`validator.test.ts` (new "mixed-case multipart/form-data" case) and `body.test.ts` (new "mixed-case multipart/form-data" case) each build a real request, read its `Content-Type`, `.replace('multipart/form-data', 'Multipart/Form-Data')`, and re-send the buffered body. The same five-line dance is copied in two files (and a variation in `buffer.test.ts`). A small test helper (or simply a hard-coded `Multipart/Form-Data; boundary=...` body string, as `buffer.test.ts` already does) would remove the duplication.

## Verification status

Verified by reading the diff and running `src/validator/validator.test.ts` (all pass). The "parameter names now case-insensitive" observation follows from the regex flag semantics; it was not separately executed.
