# Regression tests and inference boundary

## Source evidence

The added `packages/tests/server/regression/issue-5020-inference-middleware.test.ts` is 40 lines. It constructs a router with a direct string procedure, a string procedure followed by pass-through middleware, and a no-input void procedure followed by middleware. Lines 27–37 assert that the direct and middleware string procedures both infer string inputs and outputs.

The regression directly covers the PR title's inference failure. The pre-change `Overwrite` mapped string members into an object of string keys; the scratch compiler check confirms that result is not `string`. At the head, the same primitive overwrite resolves to `string`. The new procedure assertions and the head tests-package compiler check both pass.

The void procedure at lines 13–17 is constructed but its inferred input and output are not asserted. The test's `async` marker and editor query comments do not affect the verified behavior. These are observations rather than separate actionable findings: adding cosmetic review comments would obscure the shared-helper contract issue.

Commands used to inspect this subsystem:

```sh
nl -ba packages/tests/server/regression/issue-5020-inference-middleware.test.ts
cat packages/tests/tsconfig.json
cat tsconfig.json
cat tsconfig.build.json
cat packages/tests/package.json
sed -n '1,160p' packages/tests/server/regression/issue-4321-context-union-inference.test.ts
rg -n 'Overwrite|middleware|optional|union' packages/tests/server/input.test.ts packages/tests/server/middlewares.test.ts
```

The tests configuration includes server and showcase tests. It inherits the root configuration's empty exclusion list, overriding the build configuration's test exclusions. Therefore the regression assertions are included in the completed compiler check. The installed compiler is TypeScript 5.1.3.

## Connection to the actionable finding

The summary contains one finding, detailed in [01_shared-type-contract.md](01_shared-type-contract.md): the helper has redundant fallback conditionals and a misleading `never` explanation. The new regression verifies the string repair but does not establish the helper's full replacement contract. The scratch matrix supplies focused evidence for the finding and its behavior-preserving remedy. There is no additional independent finding in this subsystem.

The existing context-union regression is material evidence against making the object checks non-distributive. Its middleware chain preserves a `SetContext | UnsetContext` union and then narrows it through middleware. It passed in both the original configuration and the coherent candidate mirror.

## Worked contract-test proposal

When implementing the summary's remedy, retain the procedure regression and add explicit type assertions near the helper's regression coverage. These assertions pin behavior that the misleading fallback could otherwise encourage someone to change. They are boundary tests, rather than a copy of the helper's conditionals.

```ts
import type { Overwrite } from '@trpc/server/src/core/internals/utils';

expectTypeOf<Overwrite<string, number>>().toEqualTypeOf<number>();

expectTypeOf<
  Overwrite<{ a: string; retained: boolean }, { a: number }>
>().toEqualTypeOf<{ a: number; retained: boolean }>();

expectTypeOf<
  Overwrite<{ kind: 'a'; a: number } | { kind: 'b'; b: string }, { added: true }>
>().toEqualTypeOf<
  | { kind: 'a'; a: number; added: true }
  | { kind: 'b'; b: string; added: true }
>();

expectTypeOf<Overwrite<string, never>>().toEqualTypeOf<never>();
expectTypeOf<Overwrite<{ a: string }, never>>().toEqualTypeOf<never>();
expectTypeOf<Overwrite<never, string>>().toEqualTypeOf<never>();
```

This proposal is review text, not an applied checkout edit. Equivalent primitive, object, and bottom-type assertions were compiled in the scratch matrix. The full coherent candidate check also retains the existing union regression. Add these focused assertions as part of the actual helper cleanup, then rerun the native configuration.

If preserving `TType` for a `never` replacement is desired instead, that is a different behavior change. It requires an explicit non-distributive `never` guard and a separate assessment of its inference effects; it should not be inferred from the present dead fallback. The demonstrated remedy preserves the pinned head's semantics.

## Verification ledger

| Check | Result | Meaning |
| --- | --- | --- |
| Native tests-package `tsc --noEmit -p tsconfig.json` | Passed, exit 0 | Pinned-head test and inference assertions compile |
| Scratch `matrix.tsconfig.json` | Passed, exit 0 | 1,024 direct-definition comparisons and explicit contract probes compile |
| First scratch `candidate.tsconfig.json` | Failed, exit 2; invalid harness | Mixed source identities; excluded from product judgment |
| Coherent scratch `coherent-candidate.tsconfig.json` | Passed, exit 0 | Existing tests package compiles with simplified helper |
| Runtime Vitest | Not run | Execution policy identifies focused type-checking as the discriminating check |

The corrected harness and all other scratch material are in [thermo-verification](../thermo-verification). Each compiler configuration/flag set ran once; there was no network access. Neither report proposes modifying runtime input parsing, parser chaining, context spreading, or object-modifier semantics outside this PR's scope.

