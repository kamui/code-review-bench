# Detail 02 — src/validator/validator.ts and tests

## Observations

validator.ts changes are three `i` flags on module-level regexes (src/validator/validator.ts:24-26). This is the smallest correct change for the validator guard and introduces no new branches. Residual concern is covered in detail 01 Finding B: the validator's regexes remain a separate matching rule from parseBody's media-type equality, so the notion of "is this a form request" is still defined in two places with different strictness.

One mild boundary smell stays unchanged and is now more visible: validator.ts:115-121 stores a resolved `FormData` into `c.req.bodyCache.formData`, while the new parseBody code stores a Promise there with a cast (detail 01 Finding A). The cache slot has no single owner.

Tests. The new tests (validator.test.ts, body.test.ts, buffer.test.ts) cover mixed-case JSON, urlencoded and multipart. They build the mixed-case multipart header by string-replacing the generated header, which keeps the boundary intact and is a reasonable boundary-preservation check. The two near-identical "build FormData, replace `multipart/form-data` in the header, re-send the buffer" blocks in body.test.ts and validator.test.ts are duplicated setup that could be a small shared test helper; low priority. The `buffer.test.ts` case exercises a normalization that is probably unnecessary (see detail 01 Finding B), so it protects code that may be deletable.

Verification: `./node_modules/.bin/vitest --run --project main --coverage.enabled=false src/utils/body.test.ts src/utils/buffer.test.ts src/validator/validator.test.ts src/request.test.ts` passed: 4 files, 144 passed, 1 skipped.
