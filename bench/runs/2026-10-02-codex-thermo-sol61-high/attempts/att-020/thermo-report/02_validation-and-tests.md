# Validation, normalization, and test design

## Scope and disposition

This report covers src/validator/validator.ts, src/validator/validator.test.ts, src/utils/buffer.ts, src/utils/buffer.test.ts, and the changed body tests. It introduces no additional actionable finding beyond the cache-ownership finding in summary.md.

The matching changes are appropriately small. The relevant architectural improvement is to remove decoding/cache orchestration from consumers as part of fixing that finding.

## Normalization evidence

src/utils/body.ts:104 extracts the segment before the first semicolon, trims it, and lowercases it for matching. Lines 106–107 accept the two supported form media types by equality. Compared with startsWith(), this avoids classifying strings with unsupported media-type suffixes as supported forms.

src/utils/buffer.ts:113 replaces only the prefix matched by /^[^;]+/ with its lowercase form. The header suffix, including multipart boundary values, is left intact. The callback is direct; it is not a general parsing framework or a hidden dispatch mechanism.

src/utils/buffer.test.ts adds a multipart fixture with sampleBoundary in the body and header. Because that value contains uppercase B, a whole-header lowercase implementation would break this test. The test therefore exercises parameter preservation, not just a mixed-case media-type label.

The parser's exact media-type guard and the validator's full-header regexes have different pre-existing responsibilities. This review does not propose replacing all guards with equality checks: doing so would alter accepted JSON suffixes and parameter validation.

No existing canonical content-type normalization helper was found in the relevant source search. Other split(';') sites concern client result selection or SSG output decisions. Moving this change into those consumers would not improve ownership.

## Validator evidence

src/validator/validator.ts:24–26 adds /i to the existing JSON, multipart, and URL-encoded expressions. Their structure and branches are otherwise unchanged.

The JSON expression already accepts selected application/*+json forms. The flag broadens case matching without adding a second JSON parsing mode.

The multipart expression matches the header, including boundary syntax, but does not rewrite the header. Case-insensitive matching of a boundary character in the regex does not lowercase the boundary passed to the decoder. The buffer utility handles actual header normalization and preserves that suffix.

The existing form branch at lines 113–127 separately checks bodyCache.formData, reads arrayBuffer(), decodes, and stores the result. That duplication predates this PR. It becomes relevant because the new body utility now takes on similar responsibilities while omitting the reuse check.

This is supporting architecture evidence for the new regression, not an independent finding against unchanged validator code.

## Focused tests and commands

The focused selection ran once:

~~~sh
./node_modules/.bin/vitest --run --project main --coverage.enabled=false src/utils/body.test.ts src/utils/buffer.test.ts src/validator/validator.test.ts src/request.test.ts src/middleware/body-limit/index.test.ts
~~~

Result: five test files passed, 156 tests passed, one skipped, and exit code zero. Vitest reported approximately 357 ms total duration.

Coverage was disabled as required for this focused execution. The packet's frozen Codecov report is context, not a locally verified coverage measurement or an independent maintainability finding.

The new body tests at lines 45–68 create raw Request objects and confirm both mixed-case form encodings. They do not exercise HonoRequest cache ownership or middleware composition.

The new validator JSON test at lines 149–159 confirms that Application/JSON is parsed and returned. Form tests at lines 293–327 confirm mixed-case URL-encoded headers with charset and mixed-case multipart headers with generated boundaries.

The existing clone-request form coverage at validator.test.ts:1149–1209 exercises validator then parseBody and parseBody then validator. The validator's cold path populates arrayBuffer before formData, so those routes avoid the failing formData-first representation conversion.

The request tests exercise each body method and parseBody individually. Their formData test does not subsequently assert parseBody fields. Some existing asynchronous not.toThrow checks are weaker than awaiting and checking the resulting data, but they are outside this diff and are not separate findings.

The cache probe added outside the checkout fills the missing order: public formData() first, then parseBody(). Its exact base/head results are in 01_body-and-cache.md.

## Changed test quality

The dotted-file test previously mocked req.formData().forEach(). It now submits a real file and a dotted field. That is consistent with the new buffer path and removes mock-specific coupling.

The assertions retain the intended behavior: the dotted field replaces the File-valued parent with a nested object rather than mutating the file. The test passes on Node v24.21.0.

The available execution does not establish whether File constructors and platform parser identities behave identically in every supported runtime. No cross-runtime defect is inferred from that uncertainty.

## Size measurements

Measurements came from wc -l on each head file and git show main:<path> | wc -l for its base content.

| File | Base | Head | Net |
| --- | ---: | ---: | ---: |
| src/utils/body.test.ts | 429 | 448 | +19 |
| src/utils/buffer.test.ts | 133 | 147 | +14 |
| src/validator/validator.test.ts | 1440 | 1488 | +48 |
| src/validator/validator.ts | 185 | 185 | 0 |

The validator tests exceed 1000 lines at both revisions. The PR does not cross the skill's below-1000 to above-1000 threshold. The new tests are grouped with their existing JSON/form sections and do not introduce a new sprawling subsystem.

No decomposition question is required solely by these measurements. Splitting an already large test file would be an unrelated project unless evidence showed a material new organization problem.

## Worked consumer simplification

After HonoRequest owns normalized form decoding, the validator's form case can use this shape:

~~~ts
let formData: FormData
try {
  formData = await c.req.formData()
} catch (e) {
  let message = 'Malformed FormData request.'
  message += e instanceof Error ? ' ' + e.message : ' ' + String(e)
  throw new HTTPException(400, { message })
}

// Retain existing repeated-field aggregation and validation.
~~~

Retain the existing content-type guard before this code. Retain the distinction between validator aggregation and parseBody's all/dot options. Those behaviors need not be forced into a generic conversion abstraction.

This removes the explicit cache-vs-buffer branch, the bufferToFormData import in the validator, and direct cache mutation. The request remains responsible for body consumption and reuse; the validator remains responsible for mapping parse failures to HTTP 400 and validating fields.

This sketch is not an implemented patch. Catching failures from cached values should be considered deliberately during implementation: the current cached branch sits outside try/catch, whereas this shape maps all decoder failures consistently. If exact existing error behavior is required, preserve that distinction explicitly or justify the change with a test.

The point of the proposal is deletion of duplicated orchestration. It is not a recommendation to broaden media-type support, introduce a parser registry, or rewrite validation types.

## Verification limits

git diff --check main...review-head passed. No formatter, lint fixer, coverage run, package installation, or runtime-specific suite was run.

The five focused suites and differential scratch probe provide local evidence for the changed paths and the reported regression. They do not establish behavior on Deno, Bun, Workers, Lambda, or other unavailable platforms.

No independent performance benchmark was executed. Additional byte buffering and duplicate decode ownership are visible in source, but no measured resource regression is claimed.

