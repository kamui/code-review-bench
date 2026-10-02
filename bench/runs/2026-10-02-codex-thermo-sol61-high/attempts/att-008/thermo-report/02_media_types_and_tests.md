# Media-type matching, boundaries, and test structure

There are no additional actionable findings in this subsystem. The meaningful regression belongs to body-cache ownership and is detailed in `01_body_cache.md`. The case normalization, regex flag changes, and test additions are proportionate to the originating issue.

## Matching and parameter preservation

`src/utils/body.ts:101–107` isolates the portion before the first semicolon, trims it, and lowercases it before comparing the two supported form media types. This makes the guard case-insensitive and avoids treating a longer, unsupported prefix as a supported media type. It does not mutate the original header or lowercase a multipart boundary.

`src/utils/buffer.ts:106–117` lowercases only the prefix before a semicolon in the header supplied to a new Response. The remainder, including a mixed-case boundary value, is preserved. This is an appropriate shared decoding boundary for consumers of `bufferToFormData()`; it is not a thin wrapper or a second bespoke parser. The new direct test uses `sampleBoundary`, with an uppercase letter that would expose whole-header lowercasing.

`src/validator/validator.ts:24–26` adds the case-insensitive flag to the existing three regexes. This also permits uppercase parameter names and subtype letters, but does not rewrite parameter values. JSON decoding uses `c.req.json()`; form decoding already uses the normalized buffer helper when no cached FormData exists. The changes add no new branches to the validator. Existing limitations in these regexes, such as restrictions on multipart parameter syntax, were not introduced here and are not actionable findings for this PR.

The difference between extracting a media type for `parseBody()` and validating the fuller header in the validator predates the PR. Replacing the full header grammar with a new generic parser would broaden this patch's behavior and add machinery without resolving the actual cache defect. The useful structural simplification is centralizing form decoding and reuse, as worked in the other detail file.

## Tests and coverage evidence

The body utility adds mixed-case URL-encoded and multipart inputs. The multipart test creates a real encoded body and replaces only the media-type spelling in its header, avoiding assumptions about a generated boundary. The validator adds mixed-case JSON, URL-encoded with charset, and multipart inputs and checks the decoded output. The buffer test directly checks a case-sensitive boundary. These tests substantiate the issue fix.

The body utility also replaces a mocked FormData iterator in its file/dot-path test with actual FormData fields and platform serialization. It retains the assertion that nested field conversion produces an object rather than a File. The test passed in the permitted environment. The change removes a fake FormData shape and a cast; it does not introduce a new abstraction or a structural quality regression.

The new tests do not cover the reverse reader order or repeated HonoRequest conversion. That missing coverage matters because the implementation switched away from the canonical reader, rather than because a coverage percentage alone establishes a bug. The packet's frozen coverage comment was not used as a finding or as a substitute for source and dynamic verification.

## File-size and complexity measurements

The commands were `wc -l` on each changed head file and `git show main:<path> | wc -l` for each base file. Counts are physical newline-delimited lines, including comments and blank lines.

| File | Base lines | Head lines | Change |
| --- | ---: | ---: | ---: |
| `src/utils/body.ts` | 233 | 240 | +7 |
| `src/utils/body.test.ts` | 429 | 448 | +19 |
| `src/utils/buffer.ts` | 116 | 117 | +1 |
| `src/utils/buffer.test.ts` | 133 | 147 | +14 |
| `src/validator/validator.ts` | 185 | 185 | 0 |
| `src/validator/validator.test.ts` | 1440 | 1488 | +48 |

No file crosses from below 1,000 lines to above 1,000 lines. The validator test file was already large, and the additions remain grouped in its existing JSON and form sections. Splitting it could be independent maintenance, but this PR does not create a new decomposition threshold breach or justify a separate actionable finding.

The production diff is small. The extra request-type branch and cast in the body utility are still significant because they implement a second cache owner. Neither sheer line count nor passing tests neutralizes that structural regression. The buffer normalization and regex flags themselves add no orchestration, state, or optional contract.

## Executed verification

The focused selection ran once with this flag set from the clone root:

```sh
./node_modules/.bin/vitest --run --project main --coverage.enabled=false src/utils/body.test.ts src/utils/buffer.test.ts src/validator/validator.test.ts src/request.test.ts
```

Vitest v4.1.7 reported four test files passed, 144 tests passed, and one skipped; exit code was zero. Reported duration was 336 ms. Coverage was disabled to avoid writing reports into the read-only checkout. `git diff --check main...review-head` also completed without output or errors.

The separate head scratch probe checked a literal `Multipart/Form-Data; boundary=sampleBoundary` header against the matching serialized body, verified `{ foo: 'bar' }`, and checked the original header stayed exactly unchanged. Its uppercase URL-encoded input with a charset also parsed correctly. These assertions passed while the reader-order checks in the same probe exposed the cache defect.

Initial repository identity was head `5226d4165d48643586152614cbd07422a0ab7a22`, base `9728702911073aec5a63a3ba2840b7240e5d3205`, and head tree `3fefae9434bb89f7bbe764bc5f4b9e3840d6bb95`. The initial tracked working tree was clean. All report and scratch writes were confined to the work directory. No dependency installation, remote access, registry access, runtime-test execution, or delegated review occurred.

The final integrity check is recorded in `integrity.txt` beside these reports. It records the same head and tree identity and a clean tracked working tree. No fixes were applied. The available Node environment establishes the reported reproduction, but cross-runtime behavior and performance were not measured.
