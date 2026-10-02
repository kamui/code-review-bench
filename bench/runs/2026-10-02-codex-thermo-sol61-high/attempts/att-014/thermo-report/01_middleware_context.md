# Middleware context policy

The context regression is a shared abstraction boundary failure. Changing `Overwrite` to handle primitive input values is appropriate for input inference, but context augmentation continues to implement object spreading at runtime. Its type policy must continue to describe that operation.

## Source evidence

`packages/server/src/core/internals/procedureBuilder.ts:33–47` uses `Overwrite` for `_ctx_out`, `_input_in`, and `_input_out` in the same builder transition. The primitive replacement branches added at `utils.ts:21–30` therefore affect all three fields.

`ResolveOptions` in `utils.ts:69–75` also applies `Overwrite` when combining root context with middleware context. `middleware.ts:65`, `103`, and `136` use the same helper for middleware composition and handler context. These are the complete context call sites found by `rg -n 'Overwrite|OverwriteKnown' packages --glob '*.ts' --glob '!**/node_modules/**'`. `middleware.ts:81` additionally merges procedure parameter objects; it is a different use that must retain property merging.

The runtime implementation in `procedureBuilder.ts:360–389` accepts a next-options context patch and passes `{ ...callOpts.ctx, ...nextOpts.ctx }` downstream when the `ctx` property exists. Spreading `undefined` preserves the existing context properties. An optional object patch likewise preserves root context on both branches. The PR changes no runtime source.

## Reproduction

The complete scratch source is `../review-verification/context-regression.ts`:

```ts
import { initTRPC } from '@trpc/server';
const t = initTRPC.context<{ requestId: string }>().create();
export const unchanged = t.procedure
  .use(({ next }) => next({ ctx: undefined }))
  .query(({ ctx }) => ctx.requestId);
export const optionalPatch = t.procedure
  .use(({ ctx, next }) => next({
    ctx: ctx.requestId ? { extra: true } : undefined,
  }))
  .query(({ ctx }) => ctx.requestId);
```

Using the checkout's TypeScript 5.1.3 binary, `context.json` fails at both property reads with TS18048: `'ctx' is possibly 'undefined'`. `context-base.json`, which resolves `@trpc/server` to an archive of `main`'s server source, compiles the identical source successfully. The base source exists only in the work directory. Dependency resolution uses the installed checkout dependencies through a work-directory symlink; the checkout remains unchanged.

For the direct undefined patch, the old key mapper keeps `requestId` because the patch has no keys. The new helper instead replaces the context with `undefined`. For the optional patch, distributivity introduces an `undefined` alternative into the new downstream context. Both changes are concrete compile-time regressions for previously accepted calls.

## Worked code-judo proposal

Separate the two policies at their canonical owner instead of adding undefined checks at each call site. Reuse the unchanged property mapper underneath both policies:

```ts
type OverwriteKeys<A, B> = {
  [K in keyof A | keyof B]: K extends keyof B
    ? B[K]
    : K extends keyof A
    ? A[K]
    : never;
};

// Preserve the existing context merge behavior and distribution.
type MergeContext<A, B> = A extends any
  ? B extends any
    ? OverwriteKeys<A, B>
    : never
  : never;

// Whole-value replacement is a separate policy for inputs.
type Overwrite<A, B> = A extends object
  ? B extends object
    ? OverwriteKeys<A, B>
    : B
  : B;
```

Route `_ctx_out` transitions and root-context resolution through `MergeContext`, including the middleware builder's pipe transition. Keep input inference and parameter-object overwrites on the value policy. This extracts a meaningful shared operation and makes the semantic difference visible in the names. The context policy is exactly the prior helper's operation; the compact value policy is checked separately in the utility detail. This proposal avoids duplicating the key mapper and avoids scattering nullish-patch checks throughout the builder.

Do not replace these operations with a non-distributive `Omit<A, keyof B> & B` shortcut. The existing `issue-4321-context-union-inference.test.ts` specifically records a regression caused by mapping away union members. Preserve the current property's modifier behavior as well; optionality changes would expand this PR's scope.

## Verification status and commands

All compiler invocations use `<clone>/packages/tests/node_modules/.bin/tsc`, run from `<clone>/packages/tests`, without emitting files or contacting the network. Here `<work>` denotes the sibling `clone-work` directory.

```sh
./node_modules/.bin/tsc --noEmit -p tsconfig.json
./node_modules/.bin/tsc --noEmit -p <work>/review-verification/context.json
./node_modules/.bin/tsc --noEmit -p <work>/review-verification/context-base.json
./node_modules/.bin/tsc --noEmit -p <work>/review-verification/regressions.json
```

The normal project and focused existing/new regressions pass. The context head check fails as described, and the identical base check passes. Each configuration was compiled once for this flag set, and every command finished far below five minutes. Runtime behavior is source-verified; the runtime suite was outside the discriminating execution allowance. The combined remediation is a worked proposal, not an applied or integration-tested change.
