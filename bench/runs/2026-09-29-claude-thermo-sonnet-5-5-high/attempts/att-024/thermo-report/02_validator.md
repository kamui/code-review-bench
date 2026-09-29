# Detail 02 — `src/validator/validator.ts`

Range reviewed: `main...review-head`. The file is 185 lines, so the file-size rule does not apply.

## Observation — the validator change is the right size, but is inconsistent with the rest of the PR

Location: `src/validator/validator.ts:24-26`.

The three regexes each gain an `i` flag. As a fix for the reported bug this is minimal and correct. The tests added in `validator.test.ts` cover `Application/JSON`, mixed-case urlencoded and mixed-case multipart with a preserved boundary. The validator's `form` branch already goes through `bufferToFormData`, which is why the buffer normalization is needed there.

The weakness is architectural rather than behavioral. The validator's matching (regex with a restricted parameter grammar) and `parseBody`'s matching (split, trim, lowercase, exact compare) are two implementations of the same predicate. The validator also builds its own FormData through `bufferToFormData` and its own cache write (`c.req.bodyCache.formData = formData` at about line 112), which duplicates what `parseFormData` now also does by hand. Two bespoke copies of "read arrayBuffer, build FormData, store in bodyCache" are the direct consequence of Finding A in `01_body-parsing.md`.

Proposal: see Finding B in `01_body-parsing.md`. A single `getMediaType` helper lets the validator drop the `multipartRegex` and `urlencodedRegex` constants altogether. After that, only the `+json` suffix matcher remains as a regex.

Verification status: validator tests pass on the head (see the focused run in `01_body-parsing.md`). The parameter-grammar divergence is derived from regex text, not run.
