# 02 — Regression test `issue-5020-inference-middleware.test.ts`

Scope: `packages/tests/server/regression/issue-5020-inference-middleware.test.ts` (new, 40 lines).

**Verification:** read directly. The file type-checks cleanly as part of `packages/tests` at head (`./node_modules/.bin/tsc --noEmit -p tsconfig.json`, exit 0).

## Finding 2.1 — The test is scaffolding that was never finished, and it does not pin the shapes that still break

**Severity:** test legibility / coverage (actionable, lower priority than 01).

- **Dead fixture.** `voidWithMiddleware` (lines 13-17) is added to the router but never asserted. Either assert its input (`void`/`undefined`, the `UnsetMarker` pass-through branch) or delete it. As written, a reader has to guess why it is there.
- **Leftover editor markers.** The `// ^?` twoslash query comments at lines 8 and 35 are playground debugging aids. They are not documentation. Line 8 is not even attached to an identifier the reader cares about.
- **Naming does not locate the bug.** The file is named `issue-5020-…`, but the PR is #5017 and has no linked issue; the review thread refers to it as `issue-5017-…`. The `describe('inferRouterInputs')` / `test('string')` labels do not say that the thing under test is "input survives `.use()`". The regression folder names files by issue number so they can be traced back. With a number that points nowhere, that convention is worse than a descriptive name.
- **Coverage stops at the one reported shape.** The test pins only `z.string()`. The closely related shapes that still fail at head (see `01_overwrite_and_input_merge.md`, Finding 1.2) have no test: `z.array(...)`, and `z.object({ a: z.string().optional() })` before a middleware. There is no direct type-level test of `Overwrite`'s new semantics either, even though the PR changed the contract of an exported internal helper.

**Remedy.** Rename the test after the behaviour (for example `inference-input-through-middleware.test.ts`) or after a real issue number. Drop the `^?` markers. Assert `voidWithMiddleware` or remove it. Add three cases: array input, optional-key object input, and an object input followed by a standalone middleware whose declared input is a subset. With the call-site fix from Finding 1.1, all of these pass. They are also exactly the cases that would have shown this PR's `Overwrite` change was incomplete.
