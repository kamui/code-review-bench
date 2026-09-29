# 02 — Regression test (`packages/tests/server/regression/issue-5020-inference-middleware.test.ts`)

## What the test does

Builds a router with `str` (input string, no middleware), `strWithMiddleware` (input string, then `.use(next)`), and `voidWithMiddleware` (no input, `.use(next)`), then asserts `toBeString()` on input and output for `str` and `strWithMiddleware` inside one `async` test.

## Gaps

- `voidWithMiddleware` (lines 13–17) is never referenced by an assertion. It is the scenario for "no input, with middleware", where `_input_in` stays `UnsetMarker`, and would guard the `UnsetMarker extends TNext[...]` branch in `procedureBuilder.ts`.
- Only a primitive input is tested. The object-input path goes through the key-by-key arm of `Overwrite` that this PR rewrote; chained `.input(objA).input(objB)` (see `issue-4947-merged-middleware-inputs`) is the neighbouring behaviour to protect.
- No direct type tests of `Overwrite` for primitive/object/never combinations, although the new doc comment specifies them.
- Residue: `// ^?` twoslash markers at lines 8 and 30; `async` callback with no `await`; a single `test` holding two blocks, so a failure does not say which case broke.
- Naming: file says 5020, PR is #5017, and the earlier thread referenced `issue-5017-inference-middleware`.

## Proposed shape

    test('void input with middleware', () => {
      type Input = AppRouterInputs['voidWithMiddleware'];
      expectTypeOf<Input>().toEqualTypeOf<void | undefined>(); // whichever the library intends
    });
    test('object input with middleware', () => { /* z.object input + .use */ });
    test('Overwrite', () => {
      expectTypeOf<Overwrite<string, string>>().toEqualTypeOf<string>();
      expectTypeOf<Overwrite<{ a: 1 }, { b: 2 }>>().toEqualTypeOf<{ a: 1; b: 2 }>();
      expectTypeOf<Overwrite<{ a: 1 }, string>>().toEqualTypeOf<string>();
    });

## Verification status

Confirmed by reading the file; `tsc --noEmit -p packages/tests/tsconfig.json` passes with the test as committed. The runtime vitest suite was not run (type-level check only).
