# 02 — Regression test `issue-5020-inference-middleware.test.ts`

Scope: `packages/tests/server/regression/issue-5020-inference-middleware.test.ts` (new file, 40 lines).

## Measurements

- The file type-checks at head (`./node_modules/.bin/tsc --noEmit -p tsconfig.json` in `packages/tests`
  exits 0).
- It asserts only the `str` and `strWithMiddleware` procedures (lines 26–39), and only against `z.string()`.
- The `voidWithMiddleware` procedure (lines 13–17) is defined and never referenced by any assertion.
- Twoslash probes (`// ^?`) are left in at lines 8 and 34. The one on line 8 sits under
  `strWithMiddleware: t.procedure` and points at nothing useful.
- `test('string', async () => { … })` contains no `await` and no runtime assertions. It is a type-only test
  wrapped in an async runtime test.
- The file name says issue 5020. The PR is #5017, its body has no closing reference, and the rename commit
  (`26814253 rename`) moved the file from `issue-5017-…` to `issue-5020-…` without explanation.

## Finding 2.1 — The regression test pins only the one shape the fix happens to cover

The test proves `Overwrite<string, string>` no longer mangles the type. It covers nothing next to that
case, including the cases that are still broken (see 01, Finding 1.1). `.input(z.array(...)).use(mw)`
and `.input(z.discriminatedUnion(...)).use(mw)` both still infer wrong types at head, and a test with
either one would have shown that the fix was in the wrong place. The PR also changes the semantics of
`Overwrite` for the context paths (`unknown` / `any` / `never` on the `TWith` side) without a single type
test for `Overwrite` itself.

**Remedy.** Make this a table-style inference test over the input shapes a middleware must pass through
unchanged: primitive, array, tuple, `Date`, optional object, discriminated union, and object with a
standalone middleware that needs a subset (the #4947 shape). For each, assert
`AppRouterInputs[k]` equals the same procedure's input without `.use()`. That one invariant ("adding a
pass-through middleware never changes inferred input or output") covers the whole bug class. Either
assert `voidWithMiddleware` (its input should be `void | undefined` and its output `void`) or delete it,
and remove the `// ^?` probes. If `Overwrite` does keep a new definition, add a small `expectTypeOf`
block for it next to its other internals tests.

## Question 2.Q — Which issue is 5020?

The file is named after issue 5020, but the PR is #5017 and the body links no issue. Regression tests
here are named after the issue they pin. If #5020 is the user report, the PR body should link it
(`Closes #5020`). If it isn't, the file should use the PR number, as it did before the `rename` commit.
