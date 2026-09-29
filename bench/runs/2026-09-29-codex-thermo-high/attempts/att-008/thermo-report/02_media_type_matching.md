# Media-type matching and form-data conversion

## Review

`src/utils/body.ts:104-107` extracts and lowercases only the portion before the first semicolon, then uses exact equality for the two supported form types. That is more precise than the old `startsWith()` checks and leaves parameters available on the original header. `src/utils/buffer.ts:106-116` lowercases only the media type before constructing a `Response`, preserving the multipart boundary parameter's spelling. `src/validator/validator.ts:16-18` adds the regular-expression `i` flag to the supported JSON, multipart, and URL-encoded patterns; it changes matching behavior without rewriting header parameters. The changed implementation remains direct and no extra condition chain or abstraction layer was introduced.

The regression tests cover mixed-case URL-encoded and multipart parsing, preservation of a case-sensitive multipart boundary, and mixed-case JSON/form validation. They exercise the requested behavior across the parser, shared conversion helper, and validator.

## Code-judo assessment

There are three matching mechanisms: an extracted normalized media type for `parseBody`, a case-insensitive regex for validator routing, and a media-type-only rewrite before `Response.formData()`. They serve distinct constraints: exact form-type dispatch, JSON subtype/parameter regex recognition, and platform parser input normalization while preserving parameters. Consolidating them into a general content-type abstraction would add indirection without eliminating those distinct needs, so no abstraction extraction is recommended from this diff.

## Verification status

Static review only. `git diff --check main...review-head` was clean. Tests were not run. No actionable issue was identified in this subsystem.
