# Media-type policy

## Finding

The PR encodes case-insensitive media-type matching separately in three places. `src/utils/body.ts:104-106` splits off parameters and lowercases the media type before exact comparisons. `src/validator/validator.ts:24-26` changes all three validator regular expressions with `/i`. `src/utils/buffer.ts:110-114` independently lowercases the header prefix before passing it to `Response`. Each local form is short, but together they leave the shared rule and its parameter-preservation requirement spread across parser, validator, and adapter code. A future supported type or parameter rule can be updated in one path and missed in another.

## Evidence and measurements

- `parseBody()` compares normalized exact strings at `src/utils/body.ts:104-107`.
- JSON, multipart, and urlencoded validation each rely on regex-wide case insensitivity at `src/validator/validator.ts:24-26`.
- `bufferToFormData()` lowercases text up to the first semicolon, retaining the remaining boundary parameter's original spelling, at `src/utils/buffer.ts:110-114`.
- The changes add no meaningful condition depth, and all three implementation files stay well below the skill's 1,000-line threshold. The concern is duplicated policy, not volume.

## Worked code-judo proposal

Add a small shared utility that normalizes only the media type portion of a Content-Type value and leaves the parameter suffix unchanged. Use the normalized representation consistently for `parseBody()`'s supported-type check and for validator matching, removing the `/i` flags as a separate normalization mechanism. Have `bufferToFormData()` use the same utility before setting the `Response` header. Keep the raw parameter suffix intact so multipart boundary matching remains case-sensitive where it matters.

Keep this utility narrowly scoped to Content-Type handling; a generic header parser would add abstraction without helping this change. The result is one explicit rule with three consumers rather than three subtly different implementations.

## Verification status

Static inspection only. The added tests cover mixed-case urlencoded and multipart parsing, validator paths, and preservation of a mixed-case multipart boundary. No tests were run during this review.
