# Detail 02: src/validator/validator.ts, src/utils/buffer.ts, tests

## Finding B: three independent mechanisms for the same normalization

Evidence:

- `src/validator/validator.ts:24-26` adds the `/i` flag to three regexes.
- `src/utils/body.ts:104` uses `contentType?.split(';')[0].trim().toLowerCase()`.
- `src/utils/buffer.ts:113` uses `contentType.replace(/^[^;]+/, (mediaType) => mediaType.toLowerCase())`.

Problems:

- The `/i` flag also makes the parameter portion of the regexes case-insensitive. This is harmless for `boundary=` and `charset=` names but is broader than the issue's "media type only" request.
- `bufferToFormData` is a generic utility. Mutating the header inside it only matters if the media type reaches it un-normalized. After the validator fix, its only real in-repo callers are the validator (which passes the raw header) and, in this PR, `parseFormData`. The lowercasing hides a per-call-site concern in a low-level helper, and the new comment restates the code.
- There is no shared `getMediaType(contentType)` helper to own this rule.

Verification: read-only. Behavior of the `/i` regexes and the buffer replace is covered by the added tests; not executed separately.

### Code-judo proposal

Add one tiny helper (for example in `src/utils/headers.ts` or `src/utils/mime.ts`): `getMediaType(header) => header.split(';')[0].trim().toLowerCase()`. Use it in `parseBody`. In the validator, compute the media type once and compare it against a small set or a light regex, keeping the existing parameter regexes only where parameter syntax is being validated. Drop the `replace` from `buffer.ts` if the platform already accepts the header. The rule then has one owner and one test.

## Test duplication

`body.test.ts` and `validator.test.ts` both hand-build a multipart body by creating a `Request`, reading its Content-Type, string-replacing `multipart/form-data`, and re-sending the arrayBuffer. That is a copy-pasted setup and would be one small test helper.
