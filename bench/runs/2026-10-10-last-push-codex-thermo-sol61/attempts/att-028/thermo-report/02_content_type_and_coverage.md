# Content-Type normalization, validator guards, and coverage

## Assessment

No additional actionable finding is supported in this subsystem. The PR's normalization is direct and preserves parameter values. The architectural problem is the body cache rewrite documented in 01_request_body_cache.md, not normalization itself.

`src/utils/body.ts:101–107` extracts the substring before the first semicolon, trims it, lowercases it, and compares exact supported media types. This replaces prefix matching with an explicit media-type comparison and handles case variants without changing headers. The ordinary missing/unsupported-header result remains an empty object. The implementation adds no parser modes or generic wrapper layer.

`src/utils/buffer.ts:109–114` lowercases only the initial substring before the first semicolon when building the decoding Response. It neither mutates the original request nor lowercases the multipart boundary. Reusing this existing canonical utility is appropriate for mixed-case form decoding, particularly where platform form decoders need a normalized header. A new generalized MIME parser would add concepts without a demonstrated need in this small change.

`src/validator/validator.ts:24–26` adds case-insensitive regex flags to its three existing guards. The JSON, multipart, and URL-encoded branches retain their previous supported-shape checks. The flags make media types and literal parameter names case-insensitive without modifying parameter strings or the bytes consumed by the decoder. The complex parameter regex restrictions already existed at base; this PR does not introduce them. No newly introduced guard mismatch beyond the cache regression was established.

The existing guards intentionally differ from parseBody's media-type-only check. Replacing all guards with a common allowlist would also change previously accepted/rejected parameter shapes and JSON suffix behavior. That is not a justified behavior-preserving simplification for this review.

## Changed test evidence

The body tests add mixed-case URL-encoded and multipart cases. The multipart fixture serializes an actual FormData request, changes only the media type in its header, and retains the boundary. The buffer test additionally uses the literal mixed-case boundary sampleBoundary, which provides meaningful coverage against accidentally lowercasing parameters. The validator tests add mixed-case JSON and both form encodings; the URL-encoded case includes charset=UTF-8.

The unrelated change in the file/dotted-key test removes a cast-heavy formData mock and supplies an actual File and dotted field through FormData. The expected object conversion remains unchanged. This reduces test scaffolding and still passes on the available runtime. No evidence warrants a finding about that replacement.

The new tests are single-read examples. The existing validator tests cover validator-before-parseBody and parseBody-before-validator, including caching, and pass. They do not cover native formData-before-parseBody. Validator decoding stores the original arrayBuffer first, so its passing sequence does not establish that converting a formData-first cache back to bytes is safe.

The existing request tests exercise some methods after parseBody but do not check the reverse formData-first order or identity/mutation preservation. The focused selection's success therefore coexists with the verified failure rather than contradicting it.

## File-size measurements

Counts were taken with `git show main:<path> | wc -l` and `wc -l <path>`.

| File | Base lines | Head lines |
| --- | ---: | ---: |
| src/utils/body.test.ts | 429 | 448 |
| src/utils/body.ts | 233 | 240 |
| src/utils/buffer.test.ts | 133 | 147 |
| src/utils/buffer.ts | 116 | 117 |
| src/validator/validator.test.ts | 1,440 | 1,488 |
| src/validator/validator.ts | 185 | 185 |

There is no new crossing of the 1,000-line threshold. The validator test file is already large at base. These additions sit in its existing JSON and FormData suites and introduce no new organizational burden substantial enough to require splitting the test file in this narrow fix. Treating the pre-existing size as a new blocker would misattribute the debt.

## Verification status and execution boundary

All six changed files were reviewed in the committed diff and their surrounding consumers inspected. The focused main-project test command and complete results are recorded in 01_request_body_cache.md. `git diff --check main...review-head` produced no output. Node was v24.21.0. No network was used, dependencies were left alone, and runtime suites for Bun, Deno, Workers, Lambda and other platforms were unavailable under the task policy.

No full type check, build, or platform matrix was executed. The passing focused tests establish behavior in the available main test environment, while the separate Node probe establishes the cache regression and the feasibility of a canonical decoder. They do not establish universal platform compatibility.

Only the explicitly selected frozen skill was loaded. No ambient repository guidance, personal settings, upstream discussions, previous reviews, or external reference answers were consulted. There were no child reviewers. The report and prototypes were written outside the clone.
