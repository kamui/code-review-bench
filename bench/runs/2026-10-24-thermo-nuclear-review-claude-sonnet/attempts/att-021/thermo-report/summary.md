# Thermo nuclear review: trpc/trpc#5017 (`2abb2d5c..7dc04a7e`)

Scope: the new `Overwrite` in `packages/server/src/core/internals/utils.ts` and the added regression test. I checked it with the clone's own `tsc`. `packages/tests` type-checks cleanly. I also ran probe files under the work directory against both the PR's `utils.ts` and `main`'s. I traced every `Overwrite` caller (`procedureBuilder.ts:38-44`, `middleware.ts:65,81,103,136`, `utils.ts:71`). The runtime is untouched; this is a types-only change, and I found no security, devex or feature-gate issues.

## Findings

### 1. [Medium] `Overwrite` still mangles arrays, tuples, Map and Set, so the "middleware after input" inference bug is only partly fixed

File: `packages/server/src/core/internals/utils.ts` lines 11-30 (the `TType extends object` / `TWith extends object` branch).

The new rule is "key-wise merge when both sides are `object`", but arrays, tuples, `Map`, `Set`, `Date` and functions are all `object`. When both sides are the same such type, the non-homomorphic mapped type `{ [K in keyof TType | keyof TWith]: ... }` is applied to them. That is the same garbling the PR set out to fix, now for array-like types instead of `string`.

Evidence: I put the PR's `utils.ts` in place and built `t.procedure.input(<schema>).use((o) => o.next()).query(...)`, then read `inferRouterInputs`:
- `z.array(z.string())` infers `{ [x: number]: string; [iterator]: () => IterableIterator<string>; [unscopables]: {...}; ... flat: ...}`, not `string[]`.
- `z.tuple([z.string(), z.number()])` infers the same kind of mangled array-method object.
- `z.map(z.string(), z.string())` is not `Map<string, string>`.
- `z.string().optional()` infers `string | void | undefined`, and `z.object({a}).optional()` infers `void | {a} | {a} | undefined`.

On `main`, the same probe is wrong for those cases and also for `string` and `number`. The PR fixes only the primitive and plain-object cases, and the cases above remain broken.

Failure: for a procedure with an array, tuple or Map input plus any `.use()` (`.use(opts => opts.next())` is enough), the client-side input type and the `input` argument in the resolver are the wrong shape. Callers writing `trpc.x.query(['a'])` get a type error, or lose their types. This is the bug class the PR targets, so the fix is incomplete. The replacement rule should treat only plain, non-array records as key-mergeable, for example by excluding `readonly unknown[]`, `Map`, `Set`, `Date` and functions, or by only merging when `TType`/`TWith` are not array-like.

### 2. [Low] `Overwrite` semantics changed for `unknown`/`any`, and the doc comment and dead branch misdescribe the behaviour

File: `packages/server/src/core/internals/utils.ts` lines 6-9 and 25-29.

The doc comment says a non-object `TWith` overwrites `TType` "unless TWith is never". Because `TWith extends ...` is a distributive conditional on a naked type parameter, a `never` `TWith` always yields `never`. The final `: TType` fallback (line 28) can never be reached, since `TWith extends any` is always true. Separately, `Overwrite<{a: 1}, unknown>` and `Overwrite<{a: 1}, any>` now collapse to `unknown`/`any` (probe: `Overwrite<{a:1}, unknown>` accepts any value). Before, they stayed `{a: 1}` and an index-signature object respectively. `Overwrite<any, {b: 1}>` becomes the union `{b: 1} | {[x: string]: any}`.

Failure: this is mostly latent today, because `procedureBuilder.ts:39-44` guards `UnsetMarker extends TNext[...]` and the ctx paths pass `{}`-like values. But `_ctx_out` is also passed through `Overwrite` at `middleware.ts:65,103,136` and `utils.ts:71`. A middleware that returns an `any`/`unknown` ctx would now widen the whole `ctx` to `any`/`unknown`, silently dropping the previous ctx keys. The comment should be corrected and the unreachable branch removed.

### 3. [Low] Regression test is misnamed, covers only part of the change, and has an unused case

File: `packages/tests/server/regression/issue-5020-inference-middleware.test.ts` lines 1-40.

The PR and its review thread refer to issue 5017 (the earlier test path was `issue-5017-inference-middleware.test.ts`), but the file is named `issue-5020-...`, so the regression file does not match the PR it belongs to. The `voidWithMiddleware` procedure is defined but never asserted on. Nothing covers an array, tuple, optional or `Map` input with a middleware, which is where finding 1 shows the fix still fails, and nothing covers `Overwrite` directly (object-on-object merge, union ctx). The `// ^?` markers are leftover debugging aids.

Failure: the test passes `tsc` today and would not catch finding 1, so the remaining breakage stays invisible to CI.
