# Thermo Nuclear Review — trpc/trpc#5017 "fix(server): inference fix for inputs with middleware"

Range reviewed: `2abb2d5cd19740be37272dac6ad7fdd36244ae54..7dc04a7e94654dfad6ef1289dfe01a0a206fff3b` (`git diff main...review-head`). Files changed: `packages/server/src/core/internals/utils.ts` (the `Overwrite` type was rewritten) and the new regression test `packages/tests/server/regression/issue-5020-inference-middleware.test.ts`.

`Overwrite` is used in five places: `_ctx_out` merging in `procedureBuilder.ts:38` and `middleware.ts:65,103`, the `ctx` exposed to middlewares and resolvers (`middleware.ts:136`, `utils.ts:71`), and, the point of this PR, `_input_in` and `_input_out` merging in `CreateProcedureReturnInput` (`procedureBuilder.ts:39-44`). That type is applied by both `.use()` and `.unstable_concat()`.

How I verified this: I ran `tsc` on scratch probes against both the base server source (exported from `main` into a temp dir) and the head server source. I also ran the repo's own `packages/tests` `tsc --noEmit`, which is clean at head. The intended fix works. `.input(z.string()).use(mw)` now infers `string` where it used to infer a mangled `String`-method object. The same holds for `number`, `string | null` and `unknown` inputs, and for `context<any>()`. Ctx merging of plain objects is unchanged.

## Findings

### [Medium] A middleware or concatenated procedure with an optional/nullable input now turns a required procedure input into `| undefined`, which breaks resolvers that used to compile and drops the required-input check for callers

**Location:** `packages/server/src/core/internals/utils.ts` lines 11-24, specifically the new `TWith extends any ? TWith` branch at lines 21-23. It is reached through `CreateProcedureReturnInput` at `packages/server/src/core/internals/procedureBuilder.ts` lines 39-44 (`.use()` / `.unstable_concat()`).

**Evidence:** The new `Overwrite` distributes over `TWith`. When `TType` is an object and `TWith` is a union that includes a non-object member such as `undefined` or `null`, that member is no longer key-merged into `TType` (the old code produced roughly `TType` for it). It now *replaces* `TType` outright. So `Overwrite<{ projectId: string; name: string }, { projectId?: string } | undefined>` was `{projectId; name} | {projectId; name}` at base and is now `{ projectId: string | undefined; name: string } | undefined` at head. One way this happens is a reusable standalone middleware whose input is declared optional, attached to a procedure whose input is required:

```ts
const projectMw = experimental_standaloneMiddleware<{ input: { projectId?: string } | undefined }>()
  .create((o) => o.next());
t.procedure
  .input(z.object({ projectId: z.string(), name: z.string() }))
  .use(projectMw)
  .query(({ input }) => input.name.toUpperCase());
r.createCaller({}).c();
```

I checked this with `tsc` against both trees:
- base: the resolver compiles. `c()` is rejected with `TS2554: Expected 1 arguments, but got 0`, which is correct because the zod object parser requires input.
- head: the resolver fails with `TS18048: 'input' is possibly 'undefined'`, and `c()` with no argument is **accepted**. `inferRouterInputs` for the procedure becomes `void | { projectId: string | undefined } | undefined`.

**Impact:** Existing code that compiled on base breaks after the upgrade: every resolver that reads `input.x` now needs a non-null assertion. On the client side, `procedureBuilder.ts`/`procedure.ts:66-70` treats `undefined extends _input_in` as an optional argument, so callers can omit an input the server's parser always rejects. That error only shows up at runtime as a `BAD_REQUEST` instead of at compile time. `unstable_concat` with a builder whose input is optional hits the same root cause. Keeping `TType` for the non-object members of `TWith` when `TType` is an object (or skipping `undefined`/`null` members) would keep the base behaviour while still fixing the primitive case.

## Notes (not actionable)

- `Overwrite` still key-maps arrays, tuples and `Date`, because they satisfy `extends object`. For example, `.input(z.array(z.string())).use(mw)` infers a mapped `Array`-method object for the input. This is unchanged from base, and with `tsc` I confirmed it stays assignable to and from `string[]` and `[string, number]` in both resolvers and callers, so it is cosmetic.
- The doc comment's "unless TWith is never" and the `: TType` branch at line 29 cannot be reached, because a distributive conditional over `never` yields `never`. `Overwrite<X, never>` is therefore `never`, the same as at base.
- The new test declares `voidWithMiddleware` but never asserts on it.
