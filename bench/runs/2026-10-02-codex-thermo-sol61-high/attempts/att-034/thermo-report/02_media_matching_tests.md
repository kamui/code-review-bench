# Media matching, tests, and scope measurements

## Inspection and judgment

The committed diff contains six files, 107 additions and 18 deletions. The behavior target is case-insensitive supported media-type matching with unchanged parameter values. The matching and buffer changes meet that target in the exercised cases. The report's one blocker comes from the form decoder's cache ownership, documented separately in [01_body_cache.md](01_body_cache.md).

`src/utils/body.ts:101–107` extracts the segment before `;`, trims it, and lowercases it before exact matching. The previous implementation used startsWith. Exact matching narrows accidental prefix matches such as a longer unsupported subtype starting with a supported name. No supported-media-type failure was established from that narrowing. The guard adds no mode flags or unrelated branches.

`src/utils/buffer.ts:110–116` keeps the existing Response-based platform decoder and changes only the header's media-type segment via `replace(/^[^;]+/, ...)`. Characters after the first semicolon are not transformed. `src/utils/buffer.test.ts:120–132` builds a multipart body with `sampleBoundary` and gives the header the identical mixed-case boundary. Passing this test directly establishes that the helper preserves that parameter's case on the available runtime.

`src/validator/validator.ts:24–26` adds `/i` to the existing JSON, multipart, and URL-encoded regular expressions. Their syntax and parameter restrictions otherwise remain the same. Case-insensitive matching does not mutate the Content-Type passed to the buffer helper, so boundary values remain intact. Older limitations in these expressions are outside this case-only change; the review does not inflate them into new findings.

The new tests cover mixed-case URL-encoded and multipart parseBody, mixed-case JSON validation, and both form validator encodings. The URL-encoded validator test includes `charset=UTF-8`. The multipart integration tests obtain a platform-generated body and header and replace only the media type. The buffer fixture supplies the stronger explicit mixed-case boundary check, since generated boundaries may consist entirely of lowercase letters and digits.

The edited File/dot test replaces a custom formData stub with a real FormData containing a File and a dotted field. Its expected conversion remains the same and it passes in the focused run. This aligns the test with the changed consumption path. It does not supply coverage for HonoRequest cache reuse, which is the material missing behavior.

## Measurements

Line counts were obtained with `wc -l` on the head and `git show main:<path> | wc -l` on each base file. Counts include comments and whitespace.

| File | Base lines | Head lines | Net change |
| --- | ---: | ---: | ---: |
| `src/utils/body.ts` | 233 | 240 | +7 |
| `src/utils/body.test.ts` | 429 | 448 | +19 |
| `src/utils/buffer.ts` | 116 | 117 | +1 |
| `src/utils/buffer.test.ts` | 133 | 147 | +14 |
| `src/validator/validator.ts` | 185 | 185 | 0 |
| `src/validator/validator.test.ts` | 1,440 | 1,488 | +48 |

The large validator test file predates this PR. None of the changed files crosses from below 1,000 lines to above it. The added cases reside in their corresponding existing JSON and FormData groups, so an unrelated test-file decomposition is not made a blocker. The serious structural issue is the utility taking responsibility for an existing request cache, despite the small overall line delta.

Canonical-helper searches used `rg -n 'bufferToFormData|normalize.*[Cc]ontent|mediaType' src/utils src/request.ts src/validator`. The existing helper is reused by the new utility path, but cache ownership still becomes distributed. A shared media-type abstraction alone would not repair the verified corruption. The stronger simplification is to make formData acquisition canonical, as worked in the body/cache detail.

The review considered replacing all validator expressions with one normalized-media-type matcher. That would also change how existing parameter syntax is admitted. The current regex flag edits are direct, so a broader parser rewrite is not established as an actionable simplification for this small fix. No new `any`, mode enum, sequential orchestration, partial external updates, or identity wrapper was introduced by those matching edits.

## Focused execution

The exact test selection was run once from the clone root with coverage disabled, using its installed binary:

```sh
./node_modules/.bin/vitest --run --project main --coverage.enabled=false src/utils/body.test.ts src/utils/buffer.test.ts src/validator/validator.test.ts src/request.test.ts
```

Vitest v4.1.7 completed with four passing files, 144 passing tests and one skipped test, in 424 ms. This establishes that the supplied case regression tests and adjacent existing tests pass. Runtime `expectTypeOf` assertions in this selection are not a substitute for a full TypeScript compiler run. No full-suite, coverage, performance, or unavailable-platform claim is made.

Existing cache tests in `src/validator/validator.test.ts:1159–1211` exercise validation followed by parseBody and parseBody followed by validation. Those orders have an arrayBuffer cache available and pass. `src/request.test.ts` checks formData and parseBody separately. Neither arrangement exercises the failing native formData-first cache order or the new concurrent publication window. Assertions for those specific invariants are necessary as part of the single finding's remedy.

The packet's Codecov result is contextual evidence, not a locally reproduced coverage measurement. The review disabled coverage to avoid producing files in the read-only checkout and did not make its reported percentage a separate finding.

## Scope and integrity

The pinned refs and diff were verified locally with `git rev-parse HEAD main review-head`, `git diff main...review-head`, and `git diff --numstat main...review-head`. The initial status was clean. At completion, `git diff --exit-code`, `git diff --cached --exit-code`, and `git status --porcelain` remained clean. Scratch sources, executable bundles, summary, detail reports, and the finding locator are under clone-work. Dependencies were left alone.

Only the frozen selected SKILL.md was loaded; it references no additional workflow resource. No ambient repository guidance, external discussions, prior reviews, or reference answers were used. No child reviewer was required or started. Network and runtime-specific test suites were unavailable and were not attempted.
