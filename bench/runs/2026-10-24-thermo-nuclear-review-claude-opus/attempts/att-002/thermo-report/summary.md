# Thermo Nuclear Review — trpc/trpc#5017 "fix(server): inference fix for inputs with middleware"

Range reviewed: `2abb2d5cd19740be37272dac6ad7fdd36244ae54..7dc04a7e94654dfad6ef1289dfe01a0a206fff3b` (`git diff main...review-head`).

Changed files:
- `packages/server/src/core/internals/utils.ts` — rewrites the internal `Overwrite<TType, TWith>` helper type.
- `packages/tests/server/regression/issue-5020-inference-middleware.test.ts` — new type-level regression test for `.input(z.string()).use(mw)`.

`Overwrite` is a type-only change, but it is not limited to input inference. It is also used for **context** typing everywhere:
- `procedureBuilder.ts:38` (`_ctx_out` after `.use()`),
- `procedureBuilder.ts:41,44` (`_input_in` / `_input_out` after `.use()`, which is the target of this PR),
- `middleware.ts:65,103` (`_ctx_out` in middleware builders / `unstable_pipe`),
- `middleware.ts:81` (`Overwrite<TNewParams, $Params>`; both are param objects, so the behavior is unchanged),
- `middleware.ts:135-137` (`opts.ctx` inside every middleware function),
- `utils.ts:71` (`ResolveOptions['ctx']`, the `ctx` seen by every resolver).

It is also re-exported publicly from `packages/server/src/internals.ts:11`.

I checked the new type with the clone's own `tsc`, using scratch files that ran each case against both the head source and the merge-base source (`git archive main packages/server/src`). The repository's own `packages/tests` type-check passes on the head (`tsc --noEmit -p tsconfig.json`, exit 0).

## Findings

### [Medium] `Overwrite<any, X>` now produces a union, which breaks `ctx` typing for any app whose root context type is `any`

- **File / lines:** `packages/server/src/core/internals/utils.ts:11-30`. The trigger is the new `TType extends object ? … : TType extends any ? TWith extends any ? TWith : TType : never` structure.
- **Evidence traced:**
  - The old implementation started with `TType extends any ? …`. When the checked type is `any` and the extends target is `any`, TypeScript takes only the true branch. So `Overwrite<any, { user: string }>` resolved to a single mapped type, `{ [x: string]: any; …; user: string }`, and arbitrary property access on it still worked.
  - The new implementation starts with `TType extends object`. When the checked type is `any` and the extends target is not `any`/`unknown`, TypeScript evaluates **both** branches and unions them. The result is now `{ user: string } | { [x: string]: any; [x: number]: any; [x: symbol]: any }`. Property access on that union only works for keys present in *every* member, so root-context properties are no longer accessible.
  - This flows through the context plumbing listed above:
    - `t.middleware(...)` builds its params from `deriveParamsFromConfig`, whose `_ctx_out` is `{}` (`middleware.ts:122`).
    - Its `opts.ctx` is `Simplify<Overwrite<TConfig ctx, {}>>` (`middleware.ts:135-137`).
    - With `ctx: any` this becomes `{} | { [x: string]: any; … }`.
    - Procedure resolvers after a context-extending `.use()` get `ResolveOptions['ctx'] = Overwrite<any, Overwrite<any, {user}>>` (`utils.ts:71`, `procedureBuilder.ts:38`).
  - Reproduced with tsc on the head, using the scratch file `clone-work/scratch/t.ts`:
    ```ts
    const tAny = initTRPC.context<any>().create();
    tAny.middleware(({ ctx, next }) => { ctx.foo; return next(); });
    // head: TS2339 Property 'foo' does not exist on type '{} | { [x: string]: any; [x: number]: any; [x: symbol]: any; }'
    tAny.procedure.use((o) => o.next({ ctx: { user: 'x' } })).query(({ ctx }) => { ctx.foo; });
    // head: TS2339 Property 'foo' does not exist on type '{ user: string; } | { [x: string]: any; ... }'
    ```
    The same file compiles without these errors against the merge-base source.
- **Failure caused:** This is a compile-time regression for existing code. An app whose context type resolves to `any` stops type-checking after upgrading. That happens with `initTRPC.context<any>()`, or with a context type inferred from an untyped or JS `createContext`, which `inferAsyncReturnType` then reports as `any`. Every `t.middleware` that reads `ctx.<anything>` fails, and so does every resolver that reads a root-context property after a middleware that calls `next({ ctx: … })`. Procedures with no context-extending middleware are not affected, because `Overwrite<any, any>` still collapses to `any`. The PR is scoped to input inference, so this context breakage is very likely unintended. Neither the new test nor the existing suite covers an `any` context. A guard such as `0 extends 1 & TType ? <old behaviour / TType & TWith> : …` before the `object` check would keep the previous behavior for `any`.

## Non-actionable observations (not findings)

- Array and tuple inputs followed by `.use()` still hit the key-wise mapped branch, because `string[] extends object`. `inferRouterInputs` therefore still shows a mapped object type instead of `string[]` / `[string, number]`. This is identical to the merge-base behavior. I checked usability with tsc: assignment to `string[]`, spread, `.map`, positional tuple destructuring and `Array.isArray` narrowing all still type-check. So this is cosmetic, not a breakage.
- For a non-object `TType`, the `: TType` branch at `utils.ts:29` is unreachable. `TWith` is a naked type parameter, so `Overwrite<string, never>` distributes to `never`, not `TType`. The doc comment ("unless TWith is never") does not say what happens then. This matches the old behavior (it also returned `never`), so nothing regresses.
- Optional/nullable object inputs with middleware (`z.object(...).optional()` then `.use()`) now infer `{…} | undefined` correctly; before, the type collapsed to `{…} | {}`. `z.string()`, `z.undefined()` and `z.unknown()` inputs are also fixed. `z.any()` stays `any`, and `z.date()` stays `Date`.
- The new test file is named `issue-5020-…` while the PR is #5017. Its `voidWithMiddleware` procedure is never asserted. Both are cosmetic.
- No security, devex, or feature-gate surface is touched. The change is type-level only and has no runtime effect.
