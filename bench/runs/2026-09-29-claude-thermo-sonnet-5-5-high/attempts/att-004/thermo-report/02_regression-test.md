# 02 — Regression test (`packages/tests/server/regression/issue-5020-inference-middleware.test.ts`)

Verification status: read directly from the diff; no runtime execution (type-level assertions only matter to tsc).

Observations, each with remedy:

- Coverage is limited to `string` input and output on a plain procedure and one with `.use((opts) => opts.next())`. This is the reported failure, but the `Overwrite` doc comment promises object/object merging, primitive replacement, and `never` handling, and none of these are asserted. Remedy: add a small type-level test for `Overwrite` itself, covering object+object, primitive+primitive, object+primitive, primitive+object, arrays, never, unknown, any.
- `voidWithMiddleware` (from line 13) is declared on the router but never asserted. Either assert `AppRouterInputs['voidWithMiddleware']` is `void | undefined` or remove it.
- Debris: `// ^?` markers at lines 8 and 34 (editor quick-info hints), and an `async` test callback without `await`.
- Naming: the file is `issue-5020-…` while the PR is #5017 and the earlier review thread referenced `issue-5017-…`. Pick the real issue number.
- PR body is the unfilled template (`Closes #` empty), so intent is not recorded.
