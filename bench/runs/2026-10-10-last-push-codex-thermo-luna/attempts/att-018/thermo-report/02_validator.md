# Validator media-type matching

## Scope and evidence

`src/validator/validator.ts:24-26` changes only the three existing regular expressions: JSON, multipart form data, and URL-encoded form data now use the regular-expression `i` flag. The `validator()` switch and its early-exit behavior at `src/validator/validator.ts:93-128` remain unchanged. JSON still reaches `c.req.json()` only when its pattern matches; form validation still parses through `bufferToFormData()` after its existing content-type gate.

The tests add mixed-case `Application/JSON`, URL-encoded media type with a charset, and multipart media type with its generated boundary. The multipart test changes only the media type spelling and retains the generated boundary text, checking that normalization does not corrupt it.

## Structural assessment

This is a compact modification to the existing matching policy, not a new parser or dispatch layer. The regexes were already the canonical validator checks, and applying case-insensitivity to those checks avoids duplicating conditions in the switch. The multipart pattern's character set already accepts uppercase and lowercase boundary characters; the `i` flag does not rewrite the boundary passed to the form parser. The shared conversion helper normalizes only the media type prefix while retaining parameters.

There is a possible narrower implementation using normalized media-type extraction instead of regex flags, but the validator patterns also validate the supported parameterized forms and JSON suffix forms. Replacing those patterns would create a new parsing abstraction without making this small change easier to follow. No abstraction or simplification opportunity is sufficiently clear to warrant a finding.

## Verification status

The focused validator suite passed as part of the 3-file Vitest run: 106 tests passed total and 1 was skipped. `git diff --check main...review-head` passed. Platform suites unavailable under the execution policy were not run.

## Finding status and remediation

No actionable finding. No code change is recommended from this subsystem review.
