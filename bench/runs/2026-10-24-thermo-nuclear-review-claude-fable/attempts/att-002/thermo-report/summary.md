# Thermo Nuclear Review — trpc/trpc#5017 "fix(server): inference fix for inputs with middleware"

Range reviewed: `2abb2d5cd19740be37272dac6ad7fdd36244ae54..7dc04a7e94654dfad6ef1289dfe01a0a206fff3b` (`git diff main...review-head`), 2 files, +57/−3.

## What the change does

`Overwrite<TType, TWith>` in `packages/server/src/core/internals/utils.ts` previously always built a mapped type over `keyof TType | keyof TWith`, so `Overwrite<string, string>` (what `.input(z.string()).use(mw)` produces for `_input_in`/`_input_out`) became an object of `String.prototype` method keys. The new implementation only maps key-by-key when both sides `extends object`; otherwise it returns `TWith` wholesale (or `never` when `TWith` is `never`). A regression test asserts that `inferRouterInputs`/`inferRouterOutputs` for a `z.string()` procedure with a pass-through middleware are `string`.

## Verification performed

- `packages/tests`: `./node_modules/.bin/tsc --noEmit -p tsconfig.json` on the head passes cleanly (includes the new regression test and the existing `issue-4947` standalone-middleware input-merging test, `concat.test.ts`, `middlewares.test.ts`).
- `packages/server`: `tsc --noEmit -p tsconfig.json` reports only pre-existing declaration errors inside `node_modules/next` (environment, unrelated); no errors in `src`.
- Scratch probes (written under the work directory, compiled with the clone's TypeScript 5.1.3 and `paths` pointing at `packages/server/src`) comparing the old and new `Overwrite` across primitives, `undefined`, `null`, `void`, `unknown`, `any`, `never`, unions, arrays, tuples, functions, `Date`, and real builder chains (`.input().use()`, `t.middleware`, `experimental_standaloneMiddleware`, `unstable_concat`, `.use().input().use()`, union contexts).

Behaviour confirmed unchanged or improved: object/object merging (including context merges like `{ user: User | null }` → `{ user: User }`), `never` handling, `unknown`/`any`/`UnsetMarker` inputs (guarded by `UnsetMarker extends TNext[...]`), standalone middlewares with `string` and object inputs, object-object `unstable_concat`, and union contexts (`issue-4321`). Optional/nullable object inputs with middleware are now `{ a: string } | undefined` / `| null` instead of the old `{ a: string } | {}`, which is a correctness improvement.

## Findings

### 1. Medium — `next({ ctx: X | undefined })` / `next({ ctx: null })` now types the downstream `ctx` as possibly `undefined`/`null`, breaking previously compiling resolvers

**Priority:** Medium

**Where:** `packages/server/src/core/internals/utils.ts` lines 21–30 (the two new non-object branches of `Overwrite` that return `TWith` wholesale). Consumed for context merging at `packages/server/src/core/internals/procedureBuilder.ts:38` (`_ctx_out: Overwrite<TPrev['_ctx_out'], TNext['_ctx_out']>`), `packages/server/src/core/middleware.ts:65`, `:103`, `:136`, and `packages/server/src/core/internals/utils.ts:71` (`ResolveOptions.ctx`).

**Evidence traced:** `MiddlewareFunction.next` has the overload `<$Context>(opts: { ctx: $Context })` (`middleware.ts:145`) with `$Context` unconstrained, so a middleware may legally pass a nullable context, e.g. `opts.next({ ctx: opts.ctx.user ? { authed: true as const } : undefined })` or `opts.next({ ctx: null })`. `Overwrite` is distributive, so the new type resolves `Overwrite<RootCtx, { authed: true } | undefined>` to `{ RootCtx & authed } | undefined` and `Overwrite<RootCtx, null>` to `null`. The previous implementation iterated `keyof undefined` / `keyof null` (= `never`) and therefore produced `RootCtx | (RootCtx & { authed: true })` and `RootCtx` respectively. I confirmed with the clone's `tsc` 5.1.3 against the head: a procedure `t.procedure.use((opts) => opts.next({ ctx: opts.ctx.user ? { authed: true as const } : undefined })).query(({ ctx }) => ctx.session)` now fails with `TS18048: 'ctx' is possibly 'undefined'`; a `t.middleware` doing the same breaks the next `.use()` in the chain the same way; and `inferProcedureParams<…>['_ctx_out']` for `next({ ctx: null })` is literally `null`. The runtime never produces that value: `procedureBuilder.ts:371–374` merges with `{ ...callOpts.ctx, ...nextOpts.ctx }`, and spreading `undefined`/`null` is a no-op, so the real context is the previous object, exactly what the old type said.

**Failure:** Any user middleware that conditionally passes `undefined`/`null` as `ctx` (a natural way to write "only extend the context when a session exists") stops type-checking after upgrading to this patch release, with every downstream `ctx.<field>` access flagged as possibly undefined, even though the value can never be undefined at runtime. The reviewed change set is sold as an input-inference fix, but it also changes context-merge semantics for non-object `TWith`; the non-object branch should fall back to `TType` when `TWith` is `undefined`/`null` (or the `_ctx_out` call sites should guard with `NonNullable`), and a regression test for nullable `next({ ctx })` should accompany the change.

### 2. Low — the fix only covers primitive inputs; array / tuple / Map / Set inputs with a middleware remain mangled, and the new regression test does not exercise them

**Priority:** Low

**Where:** `packages/server/src/core/internals/utils.ts` lines 11–20 (the `TType extends object ? TWith extends object ? { [K in keyof TType | keyof TWith]: … }` branch); test coverage in `packages/tests/server/regression/issue-5020-inference-middleware.test.ts` lines 5–18 and 25–39.

**Evidence traced:** Arrays, tuples, `Map`, `Set`, `Date` etc. all satisfy `extends object`, so for them the key-by-key mapped type is still taken. Because the mapped type's key set is `keyof TType | keyof TWith` rather than a bare `keyof T`, it is not homomorphic and TypeScript does not preserve array-ness. With the clone's `tsc` on the head: for `t.procedure.input(z.array(z.string())).use((o) => o.next())`, `inferRouterInputs<…>['arrMw']` is not identical to `string[]` (an `Eq<…, string[]>` check yields `false`; it is an object literal type with `[x: number]: string`, `length`, `push`, … and `[Symbol.unscopables]` etc.), and `z.map(z.string(), z.number())` input is likewise not identical to `Map<string, number>`. Tuples (`z.tuple([z.string(), z.number()])`) remain assignable to `[string, number]` but are no longer identical to it. I verified the same mangling existed before this PR (`Overwrite<string[], string[]>` old vs new are identical), so this is not a new break, but it is the same defect class the PR title claims to fix, and `expectTypeOf<Input>().toEqualTypeOf<string[]>()` on such a procedure fails today. The new test file only asserts on `z.string()`; the `voidWithMiddleware` procedure it declares (lines 13–17) is never asserted on, and no array/optional/nullable case is covered.

**Failure:** Users with `z.array(...)`/`z.map(...)` inputs plus any `.use()` still see an unreadable, structurally-equal-but-not-identical input type in `inferRouterInputs`, hover tooltips, and `toEqualTypeOf` tests; the merged test suite would not catch a future regression on those shapes or on the void/no-input + middleware path it half-declares.

### 3. Low — deprecated `unstable_concat` of a required object parser with an optional object parser now advertises `undefined` as a valid client input that the server rejects

**Priority:** Low

**Where:** `packages/server/src/core/internals/utils.ts` lines 21–24 (`TWith` non-object branch returning `TWith`), reached via `packages/server/src/core/internals/procedureBuilder.ts:39–44` from `unstable_concat` (`procedureBuilder.ts:150–154`).

**Evidence traced:** `.input()` chaining has an explicit guard against chaining an optional parser onto a required one (`procedureBuilder.ts:97–100`), but `unstable_concat` has no such guard and goes straight through `Overwrite<TPrev['_input_in'], TNext['_input_in']>`. With the clone's `tsc` on the head, `t.procedure.input(z.object({ a: z.string() })).unstable_concat(t.procedure.input(z.object({ b: z.number() }).optional()))` yields an `inferRouterInputs` entry of `{ a: string; b: number } | undefined` (plus the usual `void`), whereas the old `Overwrite` produced `{ a: string } | { a: string; b: number }`. At runtime (`middleware.ts:226–256`) the first input middleware runs the required `z.object({ a })` parser against `rawInput`, so `undefined` is rejected with `BAD_REQUEST`, while `{ a }` alone is accepted (the optional second parser accepts it). The old type matched runtime; the new one admits an input the server refuses and drops the `{ a }`-only shape.

**Failure:** Limited blast radius because `unstable_concat` is marked deprecated and the required-then-optional combination is unusual, but for anyone using it the client type now permits calling the procedure with no input, which fails at runtime, and rejects the valid `{ a }`-only call.

## Not reported

- The runtime code is untouched; there is no security surface in this change.
- Devex/feature-gate/env changes: none.
- `Overwrite<any, X>` now resolves to `X | { [x: string]: any; … }` instead of the old index-signature object; both are effectively `any`-permissive, so no practical change for `ProcedureBuilder<any>` consumers.
- The new test is named `issue-5020-…` while the PR is #5017 with no linked issue; cosmetic only.
