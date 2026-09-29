# Thermo-nuclear code quality review — honojs/hono#5067

Range: `9728702..5226d41` (one commit, 6 files, +107/−18). Reviewer model: claude-sonnet-5-5 at high, single primary context, no child reviewers.

## Verdict

Do not approve as structured. The user-visible fix for issue #5060 is small, correct, and tested: give the validator regexes an `i` flag and lowercase the media type before matching. But the PR also rewrote the form-parsing path in `parseBody` for every request and introduced a real regression in the process. It also spreads one concept across three separately implemented normalizations. None of the files approach 1k lines, and there is no cast-heavy generic machinery beyond a single double cast. The problems are the rewrite and the duplication, not size.

## Findings

**1. `parseFormData` no longer calls `request.formData()`, and the new cache handoff breaks `formData()` followed by `parseBody()` (`src/utils/body.ts:121-131`, structural regression).**
The old code called `request.formData()`, which shares `HonoRequest`'s body cache. The new code calls `request.arrayBuffer()`, builds FormData through `bufferToFormData`, and writes the promise into `request.bodyCache.formData` through `as unknown as FormData`. If a handler has already called `c.req.formData()`, `#cachedBody('arrayBuffer')` rebuilds bytes from the cached FormData with a fresh multipart boundary, and those bytes are re-parsed against the original `Content-Type`. I reproduced this on the head with a scratch probe: a multipart body makes `parseBody` throw `TypeError: Failed to parse body as FormData.`, and a urlencoded body makes it return a single garbage key containing raw multipart text. That is the same silent-wrong-data failure class that #5060 reports, reintroduced through a different door. No test covers the sequence, and Codecov flags uncovered patch lines in this area. The remedy is to restore `await request.formData()` in `parseFormData` and keep the PR to the matching fix; if a runtime really cannot parse mixed-case multipart natively, do the normalization in one shared "FormData from request" helper next to `bufferToFormData`, in the fallback path only. That deletes the `instanceof HonoRequest` branch, the duplicate header lookup, the cache write, and the double cast. Full evidence and the worked proposal are in `01_body-parsing.md`, Finding A.

**2. One concept, three implementations of case-insensitive media-type matching (`body.ts:104-106`, `buffer.ts:113`, `validator.ts:24-26`; missed code-judo).**
`parseBody` splits, trims and lowercases. `bufferToFormData` rewrites the header with a regex. The validator adds `i` to three regexes. They already disagree: `multipart/form-data ; boundary=x` (whitespace before the semicolon) passes `parseBody`, but the validator's multipart regex has no `\s*` before `;` and skips it. The clean structure is a single `getMediaType(header)` helper returning the lowercased, trimmed portion before `;`. `parseBody` and the validator's form branch then compare that against two string literals. The validator's multipart and urlencoded regexes can be deleted, and only the `+json` suffix matcher remains a regex. This is about five lines and removes concepts rather than adding them. It does change which malformed parameter strings the validator rejects, so that needs a deliberate decision. See `01_body-parsing.md`, Finding B, and `02_validator.md`.

**3. `bufferToFormData` now silently rewrites its input header (`src/utils/buffer.ts:113`; hidden behavior in a conversion helper).**
It is harmless and preserves the boundary. Once finding 2 is done, callers can pass a normalized value and this regex can go, so it is low priority. See `01_body-parsing.md`, Finding C.

**4. Test gap tied to finding 1.**
The added tests cover mixed-case inputs but not the interaction with the body cache. The `vi.spyOn(req, 'formData')` mock was deleted from the `dot` test only because the rewritten `parseFormData` stopped calling `formData()`, which is a signal that the seam changed. If finding 1 is fixed by reverting the rewrite, that mock can be restored or left as real FormData, and a `formData()` then `parseBody()` regression test should be added either way. Details in `01_body-parsing.md`, Test observations.

## Verified vs inferred

Run on the head: focused vitest for `body`, `buffer`, `validator` and `request` tests (144 passed, 1 skipped), plus two scratch probes (`clone-work/probe.ts`, `probe2.ts`) for body-cache interaction. Reproduced: `formData()` then `parseBody()` fails. Confirmed working: `parseBody` twice, `parseBody` then `formData()`, and `validator('form')` then `parseBody()`. Inferred from the code and not run: the base-side behavior of `formData()` then `parseBody()`, and the whitespace-before-semicolon divergence between `parseBody` and the validator.

## Proposed remediation sequence

1. Revert `parseFormData` to `await request.formData()` and remove the `bodyCache` write and cast, keeping the `parseBody` media-type comparison.
2. Add a regression test for `c.req.formData()` followed by `c.req.parseBody()` with both multipart and urlencoded bodies.
3. Introduce `getMediaType` and use it in `parseBody` and the validator's form branch, and decide deliberately whether the validator's parameter grammar stays.
4. Verify on the target runtimes that mixed-case `Multipart/Form-Data` parses natively; only if one fails, add the fallback inside `bufferToFormData` and route both callers through it.

## Detail files

- `01_body-parsing.md` — findings 1, 2, 3 and 4 with evidence and worked proposals.
- `02_validator.md` — validator-side view of finding 2.
