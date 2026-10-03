# Regression tests and contract coverage

The added 40-line regression is useful: it compares a plain `z.string()` input route with the same route after a pass-through `.use()`, and checks both inferred router input and output as strings. The full repository type check passes, including this file. A separate scratch fixture proves that the string input assertion fails against the merge-base and passes against the head. This is a discriminating check for the stated inference fix.

The router also declares `voidWithMiddleware` at lines 13–17, but the assertions only select `str` and `strWithMiddleware` at lines 27–37. Constructing the void route checks that the API call is accepted; it does not assert the exported input/output inference for that route. This is supporting coverage evidence, not a third independent finding. When adding the regressions required by the context-boundary finding, assert this route's contract as well.

Existing tests were inspected to establish what must remain intact. `issue-4321-context-union-inference.test.ts` explicitly tests distributed context unions and narrowing through middleware. `issue-4947-merged-middleware-inputs.test.ts` checks that standalone middleware input constraints retain the extra properties from the original schema. `middlewares.test.ts` exercises context decoration, piping, and standalone context requirements. None of those inspected cases forwards an undefined top-level context patch, so their passing result does not cover the new replacement regression.

## Worked compiler regression proposal

Add compile-time coverage for the public patch boundary, in the middleware tests or a focused regression file:

```ts
const t = initTRPC.context<{ user: string; enabled: boolean }>().create();

t.procedure
  .use(({ next }) => next({ ctx: undefined }))
  .query(({ ctx }) => {
    expectTypeOf(ctx.user).toBeString();
    expectTypeOf(ctx.enabled).toBeBoolean();
    return ctx.user;
  });

t.procedure
  .use(({ ctx, next }) =>
    next({ ctx: ctx.enabled ? { extra: true as const } : undefined }),
  )
  .query(({ ctx }) => {
    expectTypeOf(ctx.user).toBeString();
    return ctx.user;
  });

const middleware = t.middleware(({ next }) => next({ ctx: undefined }));
middleware.unstable_pipe(({ ctx, next }) => {
  expectTypeOf(ctx.user).toBeString();
  return next();
});
```

All three examples access an existing root property. They assert the externally visible invariant rather than reproducing the implementation's nested conditionals. They exercise both resolver and piping paths, preventing a local repair to only one consumer from looking complete.

Keep the string comparison already in the PR. For no-input coverage, use `inferProcedureOutput` to assert the resolver's void output, or assert the existing exported router helper contract deliberately. `inferRouterInputs` derives its type from an optional procedure-argument tuple, and `inferRouterOutputs` applies serialization. It is therefore incorrect to assume that both are exactly the raw `void` type. The corrected scratch fixture compares both exported head types with the same merge-base no-input route and passes.

A small utility-level table can additionally cover object/object key overwrite, string/string replacement, mixed object/primitive input, both union operands, and never propagation. Include representative `any` and `unknown` cases because they interact with distributive conditional types. The 576-pair review matrix is verification evidence for the proposed simplification; committing an implementation-mirroring matrix of that size is unnecessary.

## Verification status

From the clone's `packages/tests` directory, `./node_modules/.bin/tsc --noEmit -p tsconfig.json` completed with exit code zero and no diagnostics. The new regression is included by that configuration, and the existing union and standalone-input tests compile. Vitest was not run because the execution policy identifies offline tsc as the discriminating check.

`../thermo-checks/policy.corrected.ts` confirms exact string input and output on the head, the preexisting failure of exact string input at the base, and head/base parity for the no-input route. It also checks the proposed split between context key merging and primitive input replacement. The associated compiler configuration passed.

`../thermo-checks/integration.ts` and its raw counterpart confirm the missing context cases: all base examples compile, all three head property accesses fail with TS18048, and removing the guards reports exactly those failures. See [01_type_contract.md](01_type_contract.md) for complete call-chain evidence and the worked type proposal.

No added test file was changed, and no remedy was applied. The proposed regression code is an actionable future change linked to the existing findings, not a claim that a patched repository has been verified.
