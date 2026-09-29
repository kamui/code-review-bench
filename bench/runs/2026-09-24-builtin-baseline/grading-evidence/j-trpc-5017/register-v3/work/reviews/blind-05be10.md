# Review blind-05be10

### Item 1
Location: packages/server/src/core/internals/utils.ts:21
Claim: When the base type is an object and TWith is `unknown` (or `any`), the new `TWith extends any ? TWith` branch replaces the whole type instead of keeping it, so a middleware ctx typed `unknown` wipes out the root context type.
Consequence: `initTRPC.context<{ user: string | null }>()` then `.use((o) => o.next({ ctx: x as unknown }))`: in the resolver, `ctx` was `{ user: string | null }` on base but is `{}` on head (checked with tsc), so `ctx.user` stops type-checking. `Overwrite<{a:1}, unknown>` went from `{a:1}` to `unknown`. With `any` it becomes plain `any` instead of keeping the keys.
Fix: —

### Item 2
Location: packages/server/src/core/internals/utils.ts:11
Claim: `TType extends object` is also true for arrays, tuples, Date, Map and functions, so these still go through the key-by-key mapped type (built over `keyof TType | keyof TWith`, which does not preserve arrays). This is the same bug class the PR fixes for `string`.
Consequence: `t.procedure.input(z.array(z.string())).use(o => o.next()).query(...)`: `inferRouterInputs['arr']` is `{ [x: number]: string; [iterator]: ...; flat: ...; ... }`, not `string[]`. The same happens for `z.tuple`, `z.map` and `z.date()`. An array-like value `{[k:number]:string; length:number}` is rejected (TS2740). Confirmed with tsc on head; the output is identical on base, so the fix leaves these unfixed.
Fix: —

### Item 3
Location: packages/server/src/core/internals/utils.ts:12
Claim: `TWith extends object` sits on a bare type parameter, so it distributes over TWith as well as TType. With union inputs, every member of TType is merged with every member of TWith, which creates fake combinations.
Consequence: A `z.discriminatedUnion('type', [{type:'a',a:string},{type:'b',b:number}])` input followed by `.use(o => o.next())` gives the input type `{type:'a';a} | {type:'b';a;b} | {type:'a';a;b} | {type:'b';b}` (tsc). Clients can send `{type:'b', a:'x', b:1}`, and narrowing on `type` in the resolver no longer gives the exact variant.
Fix: —

### Item 4
Location: packages/server/src/core/internals/procedureBuilder.ts:39
Claim: Altitude: the underlying problem is that CreateProcedureReturnInput merges TPrev and TNext inputs with a key-merge helper. A `.use()` middleware passes `_input_in`/`_input_out` through unchanged, so no merge is needed. Changing the generic `Overwrite` type only patches the `string` symptom.
Consequence: Every non-plain-object input (arrays, Date, Map, tuples, unions) still comes out mangled after `.use()`, as shown above. `CreateMiddlewareReturnInput` in `middleware.ts:104` already uses `FallbackValue<TNext['_input_in'], TPrev['_input_in']>`. Using that here, or `TPrev` input when TNext just forwards it, fixes all shapes without special cases.
Fix: —

### Item 5
Location: packages/server/src/core/internals/utils.ts:29
Claim: The `: TType` fallback, which the doc comment describes as 'unless TWith is never', can never be reached. A distributive conditional over `never` returns `never` before it gets there, so `Overwrite<X, never>` is `never`, not `X`.
Consequence: `Overwrite<string, never>` and `Overwrite<{a:1}, never>` both resolve to `never` (checked with tsc), which contradicts the new doc comment. Anyone relying on the documented behaviour, e.g. a middleware whose ctx/input is inferred as `never`, gets a `never` ctx/input instead of keeping the previous type.
Fix: —

### Item 6
Location: packages/tests/server/regression/issue-5020-inference-middleware.test.ts:13
Claim: The test defines `voidWithMiddleware` but never asserts on it. It only covers string input: nothing checks object, optional-object, array or union inputs after middleware, or the `ctx` merge that the `Overwrite` rewrite also changes.
Consequence: The ctx regression (unknown ctx wipes context) and the array/union mangling above all pass this test suite unnoticed. Void input through middleware is set up but never verified.
Fix: —

### Item 7
Location: packages/server/src/core/internals/utils.ts:21
Claim: Simplification: the nested `TWith extends any ? ... : never` and `TType extends any ? (TWith extends any ? TWith : TType) : never` branches are redundant. `X extends any` is always true for non-never X, and never already short-circuits.
Consequence: The type can be written as `TType extends object ? TWith extends object ? {merged} : TWith : TWith` with identical results. The extra branches (lines 21-30) add dead code, including the misleading `: TType` arm that the docs rely on.
Fix: —

### Item 8
Location: packages/tests/server/regression/issue-5020-inference-middleware.test.ts:1
Claim: The regression test is named after issue 5020, but the PR is #5017 and references no issue. The review thread refers to `issue-5017-inference-middleware.test.ts`.
Consequence: Someone tracing this regression test goes to trpc/trpc#5020, which is not linked from the PR (no closing reference), and cannot find the context for why the test exists.
Fix: —

### Item 9
Location: packages/tests/server/regression/issue-5020-inference-middleware.test.ts:8
Claim: Leftover twoslash `// ^?` probe comments (lines 8 and 34) are debugging artifacts. On line 8 the comment sits in the middle of a method chain and points at nothing meaningful.
Consequence: Committed dead comments make the test harder to read, and editors with twoslash-query extensions will show hover output at meaningless positions.
Fix: —
