# Code review: trpc/trpc#5017 — "fix(server): inference fix for inputs with middleware"

Range reviewed: `2abb2d5cd19740be37272dac6ad7fdd36244ae54..7dc04a7e94654dfad6ef1289dfe01a0a206fff3b` (`main...review-head`).

CLAUDE.md compliance: there are no CLAUDE.md files at the repository root or in any ancestor directory of the changed files, so there were no rules to check against.

## Issues found: 1

### 1. New `Overwrite` makes an `any` context resolve to a union, which breaks existing middleware-extended contexts

- **Reason flagged:** bug (type-level regression)
- **File / lines:** `packages/server/src/core/internals/utils.ts` lines 11-30 (the new `Overwrite` definition). It surfaces where the resolved ctx is computed: `ResolveOptions` at `packages/server/src/core/internals/utils.ts` lines 70-72, and `packages/server/src/core/middleware.ts` line 136.
- **Why it fails:** The new definition begins with the distributive conditional `TType extends object ? ... : ...`. When `TType` is `any`, TypeScript evaluates both branches and returns their union: the key-by-key mapped merge **or** plain `TWith`. The procedure's ctx is `Overwrite<config ctx, _ctx_out>`. So with an `any` base context (`initTRPC.context<any>()`, or `context<inferAsyncReturnType<typeof createContext>>()` where `createContext` returns `any`) and a middleware that calls `opts.next({ ctx: { user } })`, the resolver's `ctx` becomes `{ user: number } | { [x: string]: any; ... }`. Reading any property other than the ones the middleware added is then a compile error. The old `Overwrite` (`TType extends any ? TWith extends any ? {mapped} : never : never`) produced only the mapped type, so the same code compiled. I reproduced this with tsc against both versions:

  ```ts
  const t = initTRPC.context<any>().create();
  t.procedure.use((opts) => opts.next({ ctx: { user: 1 } })).query(({ ctx }) => {
    const u: number = ctx.user;
    return ctx.foo;
  });
  ```

  - `main`: no errors in the repro file.
  - `review-head`: `error TS2339: Property 'foo' does not exist on type '{ user: number; } | { [x: string]: any; [x: number]: any; [x: symbol]: any; } | { [x: string]: any; [x: number]: any; [x: symbol]: any; }'.`

  The existing `packages/tests` type-check still passes on `review-head`, so no current test catches this. One possible fix is to guard against `any` first (for example `0 extends 1 & TType ? ...`), or to make the `object` check non-distributive.
