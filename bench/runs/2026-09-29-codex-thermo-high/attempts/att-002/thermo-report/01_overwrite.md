# Shared `Overwrite` utility

## Finding

**Collapse the redundant non-object conditionals.** In `packages/server/src/core/internals/utils.ts:11-30`, the PR correctly introduces a distinction between merging two object types and replacing the prior type with `TWith` otherwise. But after the object/object case, it checks `TWith extends any`, and then checks `TType extends any` before checking `TWith extends any` again. These branches do not model useful alternatives to the stated rule: the inner test returns `TWith` for every non-`never` input and yields `never` for `never`; the outer test simply repeats that result after the already-distributed `TType extends object` test. The explanatory comments describe replacement, not distinct fallback behavior.

`Overwrite` is a shared internal abstraction used for context and input composition in `middleware.ts` and `procedureBuilder.ts`. Extra conditional layers here spread complexity to every reader trying to reason about inference composition. This is a particularly good code-judo opportunity: the primitive replacement rule can be represented by one direct branch, while only the object/object case needs the mapped type.

## Worked equivalent

The changed alias can be expressed with the same behavior in a smaller shape:

```ts
export type Overwrite<TType, TWith> = TType extends object
  ? TWith extends object
    ? {
        [K in keyof TType | keyof TWith]: K extends keyof TWith
          ? TWith[K]
          : K extends keyof TType
          ? TType[K]
          : never;
      }
    : TWith
  : TWith;
```

The conditional on `TType` continues to distribute over unions. When both current and incoming members are objects, the key-wise merge remains. When either side is non-object, returning `TWith` directly handles ordinary values and `never` without spelling tautological `extends any` checks. The proposed simplification should be type-checked against the existing suite before landing.

## Evidence and scope

The regression fixture at `packages/tests/server/regression/issue-5020-inference-middleware.test.ts:5-40` covers plain string input and middleware-wrapped string input, asserting that inferred input and output remain strings. It also declares a void procedure with middleware but makes no type assertions for that declaration. The core structural suggestion concerns only the conditional expression in `Overwrite`; it does not call for changing the tested inference behavior.

The helper is currently 95 lines long and remains well below the skill's 1,000-line threshold. The PR adds 17 net lines to it; this is not a file-size finding. No other actionable structural issue is supported by the small diff.

## Verification status

- `cd packages/tests && ./node_modules/.bin/tsc --noEmit -p tsconfig.json`: passed with exit code 0.
- `git diff --check main...review-head`: passed with no output.
- Runtime tests were not run under the supplied execution policy, which states they are not the discriminating check for this type issue.
- The proposed reduced alias is a review suggestion only and was not applied or separately compiled.
