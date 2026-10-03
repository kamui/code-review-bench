# Overwrite utility structure and regression coverage

The PR belongs in a canonical type utility rather than in a string-specific middleware branch, but the new fallback tree retains conditional layers that the new object checks make unnecessary. Removing these layers makes the implemented policy much easier to audit.

## Measurements and existing abstractions

`git diff main...review-head` shows two files, 57 additions and three deletions. `git show main:packages/server/src/core/internals/utils.ts | wc -l` reports 81 lines; `wc -l` reports 95 lines at head. The new regression has 40 lines. There is no file-size or decomposition blocker.

The `Overwrite` declaration grows from 13 lines to 20 lines. Outside the unchanged property mapper, it now contains five conditional tests: two `extends object` tests, two `TWith extends any` tests, and one `TType extends any` test. Only the object tests are needed to express the resulting policy.

`OverwriteKnown` preserves only existing keys and cannot replace this utility. `FlatOverwrite` in `packages/server/src/types.ts:19–30` additionally infers optional keys and is not distributive in the same way. Reusing either without accounting for those differences would change contracts. The unchanged property mapper can be shared between explicit policies as shown in the middleware detail; it does not need to be replaced wholesale.

## Why the fallback is misleading

The documentation at `utils.ts:6–9` promises whole-type replacement except when the replacement is `never`. The fallback at line 29 visually suggests that the original type survives in that case. A naked conditional parameter distributes over a union; `never` has zero members, so `TWith extends any ? TWith : TType` evaluates to `never` and does not select `TType`.

The scratch check asserts both `Overwrite<string, never>` and `Overwrite<{ a: string }, never>` equal `never`, and both assertions compile. The apparent exception has no effect for these cases. Never preservation would require a deliberate non-distributive test such as `[TWith] extends [never]`, which would be a separate behavior change. For this PR, remove the dead-looking fallback and document the actual absorbing result instead of silently changing never semantics.

## Worked behavior-preserving simplification

Keep both outer object conditions and the original key mapper, but replace both non-object fallback trees directly with the replacement type:

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

This reduces the declaration from 20 lines to 13 and the outer conditional tests from five to two. The object conditions still distribute over union members. A concrete `never` operand still collapses to `never`; the replacement branches no longer suggest a fallback that cannot occur. Update the documentation to state that object members merge by key, other values are replaced, and `never` remains absorbing. The context-policy repair is required separately; simplifying this type alone retains the verified context regression.

`../review-verification/simplification.ts` compares this candidate with the actual head helper using a function-based exact type-equality check. Its 25-type set covers `never`, `any`, `unknown`, `object`, an empty object, primitives, null, undefined, void, required/optional/readonly properties, object unions, optional object unions, primitive unions, mixed object/primitive unions, mutable and readonly arrays, a tuple, Date, a function, and the unset marker. All 625 Cartesian pairs pass, as do the two explicit never checks. Deferred generics are not universally proved by this matrix, and the combined context/value refactor was not compiled as an applied integration.

## Regression quality and intended behavior

The new regression asserts input and output strings for a direct parser and for the same parser followed by a pass-through middleware. `voidWithMiddleware` is constructed but has no explicit type assertion. That fixture is a minor coverage limit, not a separate actionable finding. No findings are made about the issue filename, cursor comments, or unnecessary async test callback because these are lower-value cosmetic matters.

A focused compiler configuration explicitly includes the new test and the existing context-union regression; it passes at head. A second focused configuration resolves only the new test's server import to archived merge-base source. It fails at lines 36 and 37 with TS2554, `Expected 1 arguments, but got 0`, where the string assertions reject the malformed inferred input and output. This confirms the intended inference improvement while the context reproduction confirms the separate regression.

The ordinary tests project also passes, and `tsc --showConfig -p tsconfig.json` confirms that its 661 root files include the newly added regression and that its effective exclusions are empty. The inherited build exclusions are overridden by the root project's empty exclusion list. The complete resolved configuration is saved as `../review-verification/resolved-tests-config.json`.

## Verification commands and scope

From `<clone>/packages/tests`, using its installed TypeScript binary:

```sh
./node_modules/.bin/tsc --noEmit -p tsconfig.json
./node_modules/.bin/tsc --noEmit -p <work>/review-verification/regressions.json
./node_modules/.bin/tsc --noEmit -p <work>/review-verification/regression-base.json
./node_modules/.bin/tsc --noEmit -p <work>/review-verification/simplification.json
./node_modules/.bin/tsc --showConfig -p tsconfig.json
```

The normal project, focused head regressions, and simplification matrix exit zero. The focused base regression exits two with the two expected failed string assertions. Configurations and assertions are retained in the work directory. Each compiler configuration/flag combination was run once. No runtime vitest suite or registry access was used.

`git diff --check main...review-head` passes. Final `git status --short`, `git diff --exit-code`, and `git diff --cached --exit-code` show no checkout changes, and HEAD remains the pinned head SHA. Only work-directory reports and verification scratch were created.
