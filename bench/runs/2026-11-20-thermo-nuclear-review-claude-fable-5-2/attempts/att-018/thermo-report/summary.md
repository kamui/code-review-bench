# Thermo-nuclear review — trpc/trpc#5017 "fix(server): inference fix for inputs with middleware"

Range reviewed: `2abb2d5cd19740be37272dac6ad7fdd36244ae54..7dc04a7e94654dfad6ef1289dfe01a0a206fff3b` (`main...review-head`), 2 files, +57 / −3.

## What the change does

`Overwrite<TType, TWith>` in `packages/server/src/core/internals/utils.ts` used to build a key-by-key mapped type for every pair of union members. For primitives that produced a mangled object made of the primitive's prototype members, so `t.procedure.input(z.string()).use(...)` inferred garbage for its input. The new version merges key-by-key only when both sides extend `object`, and otherwise returns `TWith` outright.

The stated goal is met: with a pass-through middleware, `string`, `number`, `boolean`, enum, `null`, optional and nullable primitive inputs, and optional/nullish object inputs now infer correctly, where at the merge-base they were mangled or silently lost `undefined`/`null`.

## How it was checked

- `packages/tests` type-checks cleanly at the head with the clone's own `tsc` 5.1.3 (`tsc --noEmit -p tsconfig.json`, no diagnostics).
- A scratch harness under the work directory compiled the same probe file twice, once against the head's `packages/server/src` and once against a `git archive main` copy of it, and printed the resolved types and diagnostics for each. Every "base" / "head" result quoted below comes from that harness.
- The runtime vitest suite was not run; the change is type-only.

`Overwrite` is not only used for inputs. It is shared by five context call sites, and the new "non-object `TWith` replaces `TType`" rule also applies there. That is where the regression below comes from.

## Findings

### 1. [Medium] Context type is wiped to `{}` (or becomes possibly `undefined`) after middlewares whose resulting context is not an object type

**File:** `packages/server/src/core/internals/utils.ts`, lines 21–24 (the new `TType extends object` / `TWith` not an object branch). It is reached through the unchanged context call sites `packages/server/src/core/internals/procedureBuilder.ts:38`, `packages/server/src/core/middleware.ts:65`, `:103`, `:136` and `packages/server/src/core/internals/utils.ts:71`.

**Evidence.** At the merge-base, `Overwrite<{ a: 1 }, unknown>` and `Overwrite<{ a: 1 }, undefined>` both resolved to `{ a: 1 }`, because `keyof unknown` and `keyof undefined` are `never` and the mapped type kept `TType`'s keys. At the head they resolve to `unknown` and `undefined`: `TType extends object` is true, `TWith extends object` is false, and the new branch returns `TWith`. That is the right rule for inputs, but `_ctx_out: Overwrite<TPrev['_ctx_out'], TNext['_ctx_out']>` uses the same type, and two ordinary situations give it a non-object `TWith`:

1. **`$Params` cannot be inferred from the middleware's return type.** TypeScript then falls back to the constraint `ProcedureParams`, whose `_ctx_out` defaults to `unknown` (`packages/server/src/core/procedure.ts:28`). This happens for a middleware that only throws, and for one whose result is typed `any`, such as an instrumentation wrapper. All of the following compile at the merge-base and fail at the head with `Property 'foo' does not exist on type '{}'`:

   ```ts
   const t = initTRPC.context<{ foo: string }>().create();

   t.procedure
     .use(() => { throw new TRPCError({ code: 'UNAUTHORIZED' }); })
     .query(({ ctx }) => ctx.foo);

   const deny = t.middleware(() => { throw new TRPCError({ code: 'FORBIDDEN' }); });
   t.procedure.use(deny).query(({ ctx }) => ctx.foo);

   t.procedure
     .use(async (opts) => { const res: any = await opts.next(); return res; })
     .query(({ ctx }) => ctx.foo);

   declare function instrument(name: string, fn: () => unknown): Promise<any>;
   t.procedure
     .use((opts) => instrument(opts.path, () => opts.next()))
     .query(({ ctx }) => ctx.foo);
   ```

   For these procedures `_def._ctx_out` is `{ foo: string }` at the merge-base and `unknown` at the head, so `ResolveOptions['ctx']` becomes `Simplify<unknown>`, which is `{}`. A later `.use(({ ctx }) => ...)` in the same chain sees `{}` too.

2. **`next({ ctx })` is given a value that may be `undefined`.** These compile at the merge-base and fail at the head with `'ctx' is possibly 'undefined'`:

   ```ts
   // context here is { foo: string; user?: User }
   declare const extra: { bar: number } | undefined;
   t.procedure.use((opts) => opts.next({ ctx: extra })).query(({ ctx }) => ctx.foo);

   t.procedure
     .use((opts) => opts.next({ ctx: opts.ctx.user ? { user: opts.ctx.user } : undefined }))
     .query(({ ctx }) => ctx.foo);
   ```

   `_ctx_out` goes from `{ foo; ... } | { foo; ...; bar }` at the merge-base to `{ foo; ...; bar } | undefined` at the head. The old type matched the runtime, which spreads the two contexts (`{ ...callOpts.ctx, ...nextOpts.ctx }`, `procedureBuilder.ts:371-374`) and so keeps the previous context when the new one is `undefined`. The new type says the context can be `undefined`, which never happens.

The same root cause changes two internal-but-exported generic types: `Parameters<MiddlewareFunction<ProcedureParams, ProcedureParams>>[0]['ctx']` and `ResolveOptions<ProcedureParams>['ctx']` go from `{ [x: string]: any; ... }` to `{}`. Nothing in this repository depends on that, since `packages/tests` still type-checks.

**Failure.** This is a compile-time break for existing user code in a release labelled as a fix. Any procedure chain containing a throw-only middleware, an `any`-returning middleware, or a `next({ ctx: maybeUndefined })` loses its context typing in every later middleware and in the resolver, so `ctx.<anything>` stops type-checking. Runtime behaviour is unchanged. The PR's regression test and the existing suite do not cover these shapes, which is why `tsc` on `packages/tests` stays green.

**Direction for a fix.** Inputs and contexts need different non-object behaviour: an input is replaced by the next parser's output, while a context is always spread-merged. Either stop routing inputs through the shared `Overwrite` and give them their own helper, or keep `TType` when `TType` is an object and `TWith` is not (`unknown`, `undefined`, `null`), which restores the merge-base result for the context call sites.

### 2. [Low] The fix covers primitives only: array, tuple, `Date`, `Map` and `Set` inputs are still mangled after a middleware, and the regression test asserts only `string`

**File:** `packages/server/src/core/internals/utils.ts`, lines 11–20 (the `TType extends object ? TWith extends object ?` discriminator), and `packages/tests/server/regression/issue-5020-inference-middleware.test.ts`, lines 25–38.

**Evidence.** The new discriminator is `extends object`, which arrays, tuples, `Date`, `Map`, `Set` and functions all satisfy. They still go through the non-homomorphic mapped type `{ [K in keyof TType | keyof TWith]: ... }`, which flattens them into a plain object listing every prototype member. For `t.procedure.input(z.array(z.string())).use((o) => o.next())`, `inferRouterInputs` resolves to `{ [x: number]: string; [iterator]: () => IterableIterator<string>; [unscopables]: {...}; length: number; ... }` (about 5,000 characters) at both the merge-base and the head. Without the middleware it is `string[]`. `z.tuple([z.string(), z.number()])`, `z.date()`, `z.map(...)` and `z.set(...)` behave the same way. A type-identity check of the inferred input against `string[]` or `[string, number]` is false at the head, while the same check against `string` is now true.

This is not a regression: the result is identical at the merge-base. It is the same symptom the PR sets out to fix, left in place for every non-primitive, non-plain-object input. The practical impact is limited, because the flattened types remain mutually assignable with `string[]` and `[string, number]` (checked both ways), so client calls and resolver bodies still compile. What stays broken is hover and error output, and any identity-style assertion such as `expectTypeOf<Input>().toEqualTypeOf<string[]>()`.

The new test would not catch this. It asserts only `str` and `strWithMiddleware`; `voidWithMiddleware` is declared at lines 13–17 but never asserted, and there is no array, tuple, object or optional case.

**Failure.** Users with array, tuple or `Date` inputs behind a middleware still get the mangled inferred input type that this PR is titled as fixing, and the regression test gives no signal for it.

**Direction for a fix.** Pass non-plain objects through untouched, for example by short-circuiting when `TWith` extends `readonly unknown[] | Date | Map<any, any> | Set<any>`, or by skipping `Overwrite` when the two sides are identical. Add array, tuple and object assertions to the test.

### 3. [Low] The doc comment's "unless TWith is never" does not match the type, and two of the new branches are unreachable

**File:** `packages/server/src/core/internals/utils.ts`, lines 6–9 (comment) and lines 24 and 29 (branches).

**Evidence.** The comment says a non-object `TWith` overwrites `TType` "unless TWith is never", and line 29 (`: TType`) reads as though `TType` is kept in that case. Both conditionals are distributive over `TWith`, so a `never` `TWith` yields `never` before either false branch is consulted: `Overwrite<{ a: 1 }, never>` and `Overwrite<string, never>` both resolve to `never` at the head, as they did at the merge-base. `TWith extends any` is true for every non-`never` type, so the `: never` on line 24 and the `: TType` on line 29 can never be selected.

**Failure.** There is no behaviour change from the merge-base. The risk is to maintainers: the comment and the dead `: TType` branch document a "keep `TType`" fallback that does not exist, on a type that finding 1 shows is sensitive to exactly this kind of edge case. A non-distributive check such as `[TWith] extends [never] ? TType : ...` would make the comment true; otherwise the comment and the dead branches should go.

## Checked and found sound

- **Inputs with a pass-through middleware.** `string`, `number`, `boolean`, `bigint`, enum, `null`, `undefined`, `void`, `unknown`, `any`, optional and nullable strings, plain objects, optional and nullish objects, `string | object` unions, records, and transforms in both directions all resolve to the parser's own type at the head. Optional and nullish objects are improved as a side effect: the merge-base produced `{} | { a: string }` and dropped `undefined`/`null`.
- **Chains.** Two middlewares in a row, a middleware before `.input()`, `t.middleware(...)` builders, `unstable_pipe`, `experimental_standaloneMiddleware` with a declared input, and `next({ ctx: {...} })` with an object context are unchanged or improved.
- **`unstable_concat`.** A string input concatenated onto a builder with no input was mangled at the merge-base and is `string` at the head. For inputs, the new replace rule matches the runtime `createInputMiddleware` merge, which combines only when both values are plain objects.
- **`any`-typed middlewares.** With `MiddlewareFunction<any, any>`, `_ctx_out` goes from an `any`-valued index signature to `any`. Both are fully permissive, so nothing breaks.
- **Discriminated-union inputs.** With a middleware these still produce cross-product members such as `{ type: 'b'; a: string; b: number }`. This is identical at the merge-base and comes from the untouched pairwise distribution, so it is not reported as a finding.
- **Security, devex and feature gates.** The change is type-level only, with no runtime, environment, script or flag changes.
