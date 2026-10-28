# Thermo nuclear review — trpc/trpc#5017 (`main...review-head`)

Scope: `Overwrite` in `packages/server/src/core/internals/utils.ts` and the new regression test. I type-checked the clone with its own `tsc`. The existing `packages/tests` project is clean at head. I also checked scratch files from the work directory against a copy of the old `Overwrite`.

The change fixes the intended case. `Overwrite<UnsetMarker, X>` no longer produces a mangled object built from the symbol's keys. `Overwrite<string, string>` now resolves to `string`. Object-with-object inputs and contexts still merge key by key, so I found no regression there.

## Finding 1 — Medium: `any` context loses its index signature because `Overwrite` stops distributing over `any`

File: `packages/server/src/core/internals/utils.ts`, lines 11–30. The first conditional is now `TType extends object ? … : …`.

Evidence:
- The old type started with `TType extends any ? TWith extends any ? {mapped} : never : never`. When `TType` is `any`, that check resolves to the true branch only. The result was a single `{ [x: string]: any; [x: number]: any; [x: symbol]: any; user: number }`.
- With the new type, `any extends object ? A : B` evaluates to `A | B`. For `Overwrite<any, { user: number }>` this gives `{ user: number } | { [x: string]: any; [x: number]: any; [x: symbol]: any }`. The `B` branch is `TWith` itself, and the `A` branch is the index-signature object.
- Scratch check, with the old type copied locally and the new one imported from the clone:
  - `Old<any, {user:number}>`: `o.foo` type-checks.
  - `Overwrite<any, {user:number}>`: `n.foo` fails with TS2339, `Property 'foo' does not exist on type '{ user: number; } | { [x: string]: any; … }'`.
- Reproduced through the public API. With `initTRPC.context<any>().create()` and `t.procedure.use(opts => opts.next({ ctx: { user: 1 } })).query(({ ctx }) => ctx.foo)`, the same TS2339 error appears. The inferred ctx is `{user:number} | {[x:string]:any…} | {[x:string]:any…}`.
- `Overwrite` is used for `_ctx_out` in `procedureBuilder.ts:38`, `middleware.ts:65, 103, 136` and `utils.ts:71` (`ResolveOptions.ctx`). `context<any>()` is a common escape hatch in loosely typed apps.

Failure: users with an `any` context who add a middleware that returns `ctx` stop being able to read untyped properties off `ctx`. Code that compiled on the previous release now fails to type-check, and nothing in the PR covers the case. A fix would wrap the checks so that `any` collapses, for example with an `0 extends 1 & TType` guard, or make `TType` non-distributive on `any`. A regression test with `any` ctx should be added.

## Finding 2 — Low: the doc comment and the final `: TType` branch are wrong, and nothing in the tests exercises the new semantics

File: `packages/server/src/core/internals/utils.ts`, lines 6–10 and 24–28.

Evidence:
- The comment says it overwrites "unless TWith is never". The final branch `TWith extends any ? TWith : TType` is the only place that mentions this. `TWith` is a naked type parameter, so the conditional distributes. `Overwrite<string, never>` evaluates to `never`, not `string`. The `: TType` branch is unreachable.
- The object branch behaves the same way: `Overwrite<{a:1}, never>` is `never`. That matches the old behaviour, but the comment promises something different.
- Test file `packages/tests/server/regression/issue-5020-inference-middleware.test.ts` has several gaps:
  - It is named after issue 5020, while the PR is #5017.
  - `voidWithMiddleware` is declared but never asserted.
  - It covers only a primitive `string` input plus a no-op middleware.
  - It does not cover object inputs with middleware, optional or union inputs, or the `any` ctx case from Finding 1.
  - It contains leftover `// ^?` twoslash markers.

Failure: no runtime or type failure today. The misleading comment invites a future "fix" that behaves differently from what it describes. The thin tests mean the `Overwrite` rewrite, which touches every context and input inference path, is guarded only by the single `string` case. That is how Finding 1 got through.
