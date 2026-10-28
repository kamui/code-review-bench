# Thermo Nuclear Review — trpc/trpc#5017 "fix(server): inference fix for inputs with middleware"

Range reviewed: `2abb2d5cd19740be37272dac6ad7fdd36244ae54..7dc04a7e94654dfad6ef1289dfe01a0a206fff3b` (`git diff main...review-head`), 2 files, +57/−3.

## What the change does

`packages/server/src/core/internals/utils.ts` rewrites the internal `Overwrite<TType, TWith>` helper. Before, it always produced a mapped object keyed by `keyof TType | keyof TWith`, regardless of what `TType`/`TWith` were. After, it only does the key-wise merge when both sides satisfy `extends object`; otherwise the result is `TWith` (and `never` when either side is `never`).

`Overwrite` is consumed in five places, all type-level:

- `core/internals/procedureBuilder.ts:38-44` (`CreateProcedureReturnInput`): merges `_ctx_out`, `_input_in`, `_input_out` when `.use()` / `unstable_concat()` is applied.
- `core/middleware.ts:65,81,103` (`MiddlewareBuilder.unstable_pipe`, `CreateMiddlewareReturnInput`): merges `_ctx_out` and whole `ProcedureParams`.
- `core/middleware.ts:136` and `core/internals/utils.ts:71` (`MiddlewareFunction.ctx`, `ResolveOptions.ctx`): merges root context with `_ctx_out`.
- It is also re-exported from `packages/server/src/internals.ts` so that user declaration emit can reference it.

The motivating bug: `t.procedure.input(z.string()).use((opts) => opts.next())` infers `$Params` identical to `TParams`, so `CreateProcedureReturnInput` evaluates `Overwrite<string, string>`, which mapped over `keyof string` (`charAt`, `charCodeAt`, …) and produced an unusable object type for the procedure's input. The new regression test `packages/tests/server/regression/issue-5020-inference-middleware.test.ts` asserts `inferRouterInputs`/`inferRouterOutputs` are `string` for that shape.

Note that the `t.middleware(...)` builder path does not hit this: `deriveParamsFromConfig` sets `_input_in: unknown`, so the `UnsetMarker extends TNext['_input_in']` guard in `CreateProcedureReturnInput` short-circuits to `TPrev['_input_in']`. Only inline middleware functions (and `unstable_concat`, and `experimental_standaloneMiddleware` with a declared `input`) reach `Overwrite` for inputs.

## Verification performed

I ran the clone's own `packages/tests` typecheck (`tsc --noEmit -p tsconfig.json`): exit 0, no errors, so the new regression test typechecks alongside the existing suite (including `issue-4321-context-union-inference` and `issue-4947-merged-middleware-inputs`, which exercise the union-distribution and standalone-middleware input-merge paths).

I then compiled scratch probe files under the work directory twice, once with `@trpc/server` path-mapped to the review head and once path-mapped to a `git archive main` extract of the base, using the clone's `tsc` 5.1.3 and `zod` 3.20.2. Results, for `t.procedure.input(<schema>).use((o) => o.next())`:

| Input schema | Base `inferRouterInputs` / resolver `input` | Head |
| --- | --- | --- |
| `z.string()`, `z.number()`, `z.literal('x')` | mangled object (`charAt`, `toFixed`, …) | correct primitive |
| `z.string().optional()` | mangled | `string \| undefined` |
| `z.object({a}).optional()` | `{} \| { a: string }` (lost `undefined`, so the client had to pass an argument) | `{ a: string } \| undefined` |
| `z.object({a}).nullable()` | `{} \| { a: string }` (lost `null`) | `{ a: string } \| null` |
| `z.unknown()` / `z.any()` | `{}` / index-signature object | `unknown` / `any` |
| `z.array(z.string())`, `z.tuple([...])` | mangled array-member object | **unchanged: mangled** |
| `z.string().brand('Id')` (resolver `_input_out`) | mangled | **unchanged: mangled** |
| `z.date()`, `z.set()`, `z.map()` | mangled | **unchanged: mangled** |
| `z.instanceof(ClassWithPrivateField)` | `{ amount: () => number }` (private member dropped) | **unchanged** |
| `next({ ctx: cond ? {user} : null })` downstream ctx | `{} \| { user: string }` | `{ user: string } \| null` |

Rows marked "unchanged" are pre-existing behaviour that the PR leaves intact; rows above them are improvements the PR delivers beyond the string case in its test. I found no case where head produces a worse type than base.

## Findings

### Finding 1 — Medium — `Overwrite` still key-wise merges non-plain-object inputs (arrays, tuples, Date/Set/Map, branded primitives, class instances), so the "inputs with middleware" fix is incomplete

File: `packages/server/src/core/internals/utils.ts`, lines 11–20 (the `TType extends object ? TWith extends object ? { [K in keyof TType | keyof TWith]: ... }` branch).

Evidence: the discriminator chosen is `extends object`. In TypeScript every array, tuple, `Date`, `Set`, `Map`, function, class instance, and any intersection containing an object member (for example zod's `string & z.BRAND<'Id'>`, whose `_output` is used for `_input_out`) satisfies `object`. For all of these, `.use((o) => o.next())` yields `$Params === TParams`, so `CreateProcedureReturnInput` (`procedureBuilder.ts:41,44`) evaluates `Overwrite<T, T>` and falls into the key-wise branch, which produces a non-homomorphic mapped type over `keyof T`. With `tsc` against the head I confirmed:

- `Overwrite<string[], string[]>` is `{ [x: number]: string; [Symbol.iterator]: ...; length: number; pop: ...; ... }`, not `string[]`; same for tuples, `Date`, `Set<string>`, `Map<string, number>`.
- `t.procedure.input(z.string().brand('Id')).use((o) => o.next()).query(({ input }) => byId(input))` where `byId(id: string & z.BRAND<'Id'>)` is a compile error on head (`Type '{ [x: number]: string; charAt: ...; ... 46 more ... }' is not assignable to type 'string'`). Without the middleware it compiles.
- `t.procedure.input(z.instanceof(Money)).use((o) => o.next()).query(({ input }) => spend(input))` where `Money` has a private field is a compile error on head (`Property 'cents' is missing in type '{ amount: () => number; }'`), because the mapped type drops private members. Without the middleware it compiles.
- For plain arrays the mangled type remains structurally assignable to `string[]`, so user code usually compiles, but `inferRouterInputs` hover text and `expectTypeOf(...).toEqualTypeOf<string[]>()` still show/fail with the garbled object, and `z.array(...).optional()` yields `string[] | <mangled> | undefined`.

Failure: for any procedure whose input parser resolves to a non-plain object and which has an inline middleware (or `unstable_concat`, or a standalone middleware with a declared `input`), the server-side `input` and the client-side `inferRouterInputs` type is still the garbled mapped object; for branded primitives and classes with private/protected members this is a hard type error in the user's resolver. This is identical to the base behaviour (not a regression), but the PR's title and the rewritten helper's doc comment present `Overwrite` as now behaving sanely for inputs with middleware, and the only regression test covers `z.string()`. The merge-by-keys behaviour is only meaningful for the context-merge call sites and for the object-input merge in `issue-4947`; for the `_input_in`/`_input_out` call sites, where `TType` and `TWith` are the same parser-derived type, a narrower discriminator (for example treating arrays/tuples, built-ins and intersections with primitives as "replace entirely", or short-circuiting `Overwrite<T, T>` to `T`) would close the remaining gap.

### Finding 2 — Low — Doc comment and the `: TType` fallback branch misdescribe the `never` case; `Overwrite<X, never>` is `never`, not `X`

File: `packages/server/src/core/internals/utils.ts`, lines 6–9 (comment "Otherwise it will overwrite the entire TType with TWith, unless TWith is never.") and lines 25–29 (`TType extends any ? TWith extends any ? TWith : TType : never`).

Evidence: `TWith extends any ? TWith : TType` is a distributive conditional on a naked type parameter. Instantiated with `TWith = never` it distributes over the empty union and yields `never`; the `: TType` arm is unreachable. I confirmed with `tsc` against the head that `Overwrite<string, never>` evaluates to `never` (an `as Overwrite<string, never>` expression is assignable to `0`, which only `never` is). The same holds in the object branch: `Overwrite<{ a: string }, never>` is `never`, since `TWith extends object` likewise distributes over `never`.

Failure: no runtime or current type-level consumer is affected, because no call site passes `never` as `TWith` today, and base also produced `never`. The cost is maintainability: the comment promises a `TType` fallback that cannot occur, and the dead `: TType` arm suggests to future readers that `never` is handled. If a future call site relies on the documented "unless TWith is never" behaviour, the whole `ProcedureParams` field collapses to `never`.

### Finding 3 — Low — Regression test covers only the `z.string()` shape and carries an unused fixture; the other behaviour changes in this PR are untested

File: `packages/tests/server/regression/issue-5020-inference-middleware.test.ts`, lines 13–17 (`voidWithMiddleware` is declared and never asserted on) and lines 25–39 (assertions only on `str` and `strWithMiddleware`).

Evidence: the rewrite of `Overwrite` changes the inferred type for several shapes that existing tests do not cover and this test does not either: optional and nullable object inputs with inline middleware (base: `{} | { a: string }`; head: `{ a: string } | undefined` / `| null`), `z.unknown()`/`z.any()` inputs with middleware, non-string primitives and literals, and `next({ ctx: X | null })` downstream context. The `voidWithMiddleware` procedure is built but no assertion references `AppRouterInputs['voidWithMiddleware']` or `AppRouterOutputs['voidWithMiddleware']`. The file also uses the `issue-5020` number while the PR carries no linked issue; harmless but worth confirming the reference is right.

Failure: the positive behaviour changes listed above (and the still-broken shapes in Finding 1) have no guard, so a later edit to `Overwrite` could silently regress them while this test stays green. No functional breakage results from the test itself.

## Non-findings checked

- Union distribution over context (`issue-4321`) still works: `TType extends object` and `TWith extends object` distribute over unions exactly as the previous `extends any` guards did; the existing test typechecks.
- Standalone middleware input merging (`issue-4947`) is unaffected: both sides are plain object types and take the key-wise branch.
- `ResolveOptions.ctx` / `MiddlewareFunction.ctx` with the default `object` root context and `{}` `_ctx_out` still produce `{}` / the root context.
- `Overwrite<T, any>` now yields `any` instead of an index-signature object, and `Overwrite<any, T>` yields `T | index-signature object`; both only arise when a middleware typed `MiddlewareFunction<any, any>` is passed to `.use()`, and neither is materially worse than base.
- No runtime code is changed; no devex, feature-gate, dependency, or security surface is touched. `Overwrite` is exported via `@trpc/server/internals` for declaration emit only, so the semantic change is visible to consumers who referenced it directly, which is acceptable for a bug fix on an `@internal` type.
- The clone tree was verified unchanged after review (`git status --porcelain` empty, HEAD `7dc04a7e`).
