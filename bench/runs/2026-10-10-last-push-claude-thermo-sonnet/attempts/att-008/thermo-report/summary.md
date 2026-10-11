# Thermo-nuclear code quality review — honojs/hono#5067

Range: 9728702..5226d41 (`git diff main...review-head`), 6 files, +107/-18. Fixes #5060 (mixed-case Content-Type media types skipped by `parseBody()` and `validator()`).

## Verdict

The behavior fix is correct and tests pass (144 passed, 1 skipped across body, buffer, validator and request tests). On the strict maintainability bar this is not a clean approval: the bug lives in two lowercase-only guards, but the PR also rewrites the body-reading path in `parseFormData` and adds a hidden normalization inside a generic buffer utility, and it ends up with three different mechanisms for "compare the media type case-insensitively". No file size concerns (largest touched source file is 240 lines). The detail reports carry the evidence.

## Findings

**1. `parseFormData` was rewritten beyond what the fix needs (src/utils/body.ts:121-138).** The issue is the guard at the top of `parseBody`. The new `mediaType` check at src/utils/body.ts:104-106 already fixes it. Instead of leaving `request.formData()` alone, the PR now calls `arrayBuffer()`, runs `bufferToFormData`, and for `HonoRequest` assigns a Promise into `bodyCache.formData` using `as unknown as FormData`. That is a second copy of the arrayBuffer/bufferToFormData/cache sequence already in src/validator/validator.ts:118-121, but with different cache semantics (resolved value there, Promise here, one typed slot). It also duplicates the `request instanceof HonoRequest ? request.raw.headers : request.headers` lookup inside the same file, buffers plain `Request` bodies fully in memory where they used to be handled by the platform, and forced the existing "dot" test to be rewritten because it mocked `formData()`. I probed Node v24.21.0 and mixed-case `Application/X-WWW-Form-Urlencoded` and `Multipart/Form-Data; boundary=...` already parse through `Request.formData()` natively; I could not run other runtimes, and the PR does not show one that needs this. The code-judo move is to revert `parseFormData` to its single `formData()` call and keep only the guard change, which deletes the cast, the duplicate header lookup, the duplicated sequence, and the test churn. If a runtime truly needs buffering, give the FormData-with-cache operation one owner that both parseBody and the validator call. Full evidence: `01_body-and-buffer.md`, Finding A.

**2. Media-type normalization is implemented three different ways, and `bufferToFormData` now silently rewrites its input (src/utils/body.ts:104, src/utils/buffer.ts:112, src/validator/validator.ts:24-26).** parseBody uses `split(';')[0].trim().toLowerCase()` with equality, buffer.ts uses `replace(/^[^;]+/, ...)` which does not trim, and the validator uses `/i` regexes with their own parameter rules. These already differ in edge cases (whitespace handling, which parameters are accepted), and the PR widens the divergence instead of consolidating it. Putting a regex-based Content-Type rewrite into a generic bytes-to-FormData utility is hidden behavior in the wrong layer, and given the Node probe it is probably redundant. Recommended: introduce a single `getMediaType()` helper, use it in parseBody (and have the validator guard reuse it where it only needs the type), and remove the `buffer.ts` change unless a failing runtime is demonstrated. Full evidence: `01_body-and-buffer.md`, Finding B.

**3. Minor: duplicated test setup and a test for probably-dead code.** The "generate multipart, replace header casing, resend buffer" setup is repeated in body.test.ts and validator.test.ts, and the new buffer test protects the normalization described in finding 2. Low priority; resolved automatically if findings 1 and 2 are taken. See `02_validator-and-tests.md`.

## Remediation sequence

1. Revert the `parseFormData` rewrite (finding 1) and restore the original "dot" test mock, keeping only the `mediaType` guard change in `parseBody`.
2. Decide whether any supported runtime needs normalized Content-Type for `Response.formData()`. If not, drop the `buffer.ts` change and its test. If so, add one shared media-type helper and use it everywhere instead of three ad-hoc mechanisms.
3. Leave the three `/i` flags in the validator; they are the minimal correct validator change.
4. If the cache slot semantics matter, give `bodyCache.formData` a single typed owner (promise everywhere, matching `HonoRequest.#cachedBody`) in a separate change.

## Detail files

- `01_body-and-buffer.md` — findings 1 and 2 with evidence, probe description and code-judo proposals.
- `02_validator-and-tests.md` — validator and tests observations and the vitest command run.
