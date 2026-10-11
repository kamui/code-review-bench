# Validator media-type matching

## Finding and evidence

No actionable finding. `src/validator/validator.ts:24-26` adds the `i` flag to the existing JSON, multipart, and urlencoded expressions. This is a direct way to make their literal media-type matching case-insensitive while leaving the existing accepted parameter grammar and validator control flow in place. The regression cases in `src/validator/validator.test.ts` exercise mixed-case JSON, urlencoded, and multipart values.

The validator and `parseBody()` use different matching forms because they have different existing contracts: the validator regexes also check their accepted parameter syntax, while `parseBody()` chooses whether to delegate to form parsing based on the media type alone. Making both use a new generic parser would not clearly remove complexity and risks changing parameter acceptance. The diff adds no new branches to the validator's dispatcher and increases its 185-line implementation file by no lines.

## Verification status

Static inspection only. Read `git diff main...review-head`, `src/validator/validator.ts`, the nearby validator tests, and measured file sizes with `wc -l`. `git diff --check main...review-head` passed. Tests were not run under the review task's no-test-unless-requested rule.

## Worked remediation proposal

None recommended. The case-insensitive regex flags preserve the current recognizers and express the required matching rule with no added abstraction or conditional. A shared content-type normalization layer would need to retain parameter-aware behavior and would not simplify the current recognizers enough to justify itself.
