# Detail 02: `packages/tests/server/regression/issue-5020-inference-middleware.test.ts`

## Observations (read from the diff, 40 new lines)

- The file name says issue 5020 while the PR is 5017; the earlier review thread anchors on `issue-5017-inference-middleware.test.ts`.
- Two `// ^?` twoslash markers remain (lines 7 and 30 region), which are editor debugging aids.
- `voidWithMiddleware` is defined on the router but never asserted on.
- `test('string', async () => ...)` has no awaits.
- Coverage is one end-to-end case for `z.string()` with and without `.use()`. There is no test for `Overwrite` directly, and no case for arrays, `any`, `unknown`, or `never`.

## Proposed replacement

Keep this file as the end-to-end regression under the correct number, drop the markers and the unused procedure (or assert `expectTypeOf<Input>().toBeVoid()` / `toBeUndefined()` on it), and add `packages/tests/server/overwrite.test-d.ts`-style checks such as `expectTypeOf<Overwrite<string, string>>().toEqualTypeOf<string>()`, `Overwrite<string, number>` = `number`, `Overwrite<{a:1}, {b:2}>` = `{a:1; b:2}`, `Overwrite<{a:1}, string>` = `string`, `Overwrite<string[], number[]>` = `number[]` (this one fails today), and an explicit expectation for `any`.

## Verification status

Read from source; runtime vitest not executed per policy. The array expectation is supported by the tsc probe recorded in detail 01.
