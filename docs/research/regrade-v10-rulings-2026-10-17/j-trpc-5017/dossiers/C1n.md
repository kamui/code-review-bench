# C1n: a middleware or resolver helper typed against the default `ProcedureParams` can no longer read `ctx`

Pull request: trpc/trpc #5017. Base `2abb2d5c` (v10.43.2), head `7dc04a7e` (v10.43.3). Cut-off: the last push, 2023-11-09T22:08:09Z. This is the own result of the comment in credit question C1, prepared as a candidate as the brief asks (rule section 3: a comment whose words describe a result that the known problem's statement does not hold). Probes: [`../probes/C1n/`](../probes/C1n/).

## Problem

The comment: "With the default `ProcedureParams` (`ctx: any`, `_ctx_out: unknown`), `Overwrite<any, unknown>` now resolves to `unknown` instead of an any-index-signature object, changing ctx for generically typed middleware and resolvers." And: "Third-party generic helpers that read `opts.ctx.session` would stop compiling with 'Property does not exist on type {}' after a patch release."

What the runs show (*run*, TypeScript 5.1.3 and 5.2.2 alike):

| Probe | Base | Head |
| --- | --- | --- |
| `c1-01`: `Overwrite<any, unknown>` | `{ [x: string]: any; [x: number]: any; [x: symbol]: any; }` | `unknown` |
| `c1-01`: the `ctx` of `MiddlewareFunction<ProcedureParams, ProcedureParams>` and of `ResolveOptions<ProcedureParams>` | `{ [x: string]: any; [x: number]: any; [x: symbol]: any; }` | `{}` |
| `c1-01`: `ProcedureParams['_config']['$types']['ctx']`, `ProcedureParams['_ctx_out']` | `any`, `unknown` | the same |
| `c1-02`: `export const logSession: MiddlewareFunction<ProcedureParams, ProcedureParams> = async (opts) => { const session = opts.ctx.session; … }`, then `t.procedure.use(logSession)` on an instance whose context has `session` | compiles | TS2339 "Property 'session' does not exist on type '{}'." |
| `c1-03`: `function sessionOf(opts: ResolveOptions<ProcedureParams>) { return opts.ctx.session; }` | compiles | TS2339, same text |
| `c1-04`: `Overwrite<{ a: 1 }, any>` | an index-signature object | `any`; reads compile at both |
| `c1-05`: the documented instance-independent middleware, `experimental_standaloneMiddleware<{ ctx: { session: string } }>().create(…)`, reading `opts.ctx.session` | compiles | compiles |
| `c1-06`: the shape found in public programs, `MiddlewareFunction<{ _ctx_out: Context } & ProcedureParams, { _ctx_out: Context & { user: … } } & ProcedureParams>`, reading `ctx.session.user` | compiles | compiles |
| `c1-07`: the shape a user posted in issue #3434 (2022-12-23), `MiddlewareFunction<ProcedureParams<AnyRootConfig, C>, ProcedureParams>` | compiles | compiles |

So the comment is right: a helper typed with the bare default parameters reads `ctx` at base and cannot at head. When the helper names its context type (`c1-06`, `c1-07`), or uses the documented standalone middleware (`c1-05`), it compiles at both.

## What changed

The first check of `Overwrite` changed from `TType extends any` to `TType extends object` (head line 11, *read*). Checked against `any`, a condition gives both of its branches, so with `any` as the first type the head's result joins two branches (*read*; *run*). With `unknown` as the second type, both branches return `unknown`. The object branch does so through line 12 (`unknown` is not an object) and lines 21 to 24. The other branch does so through lines 25 to 28. The joined result is `unknown`, which `Simplify` shows as `{}` (*run*: `c1-01`). At base the single check `TType extends any` led to the key-by-key copy over `keyof any`, an object that allows every property (*run*).

`MiddlewareFunction` (`middleware.ts` line 130) types a middleware's `ctx` as `Simplify<Overwrite<TParams['_config']['$types']['ctx'], TParams['_ctx_out']>>`. `ResolveOptions` (`utils.ts` lines 69 to 72) types the resolver's `ctx` the same way. With the default `ProcedureParams` these are `Overwrite<any, unknown>` (*read*).

**Before 2.** The behaviour changed between the commits (run), on the line the change rewrote (line 11). It is not an older fault.

## Promised?

**The operation.** Reading a property of `ctx` inside a middleware or resolver helper typed with tRPC's generic default parameters, `MiddlewareFunction<ProcedureParams, ProcedureParams>` or `ResolveOptions<ProcedureParams>`. Owner: tRPC. Classification: `MiddlewareFunction` and `ProcedureParams` are exported from the package's main entry (`packages/server/src/core/index.ts`). Both declarations carry `@internal` in their JSDoc (`middleware.ts` line 128; `procedure.ts` line 24, beside "FIXME: this should only take 1 generic argument instead of a list"). `ResolveOptions` is not exported from the entry point (*read*). The project builds without `stripInternal`, so the marked types ship in the published declarations (*read*: no `stripInternal` in the repository).

Searches, saved under `../upstream/`:

- **project-docs** (checked; `C1n-project-docs.txt`, 16 hits read). `MiddlewareFunction`, `ProcedureParams` and `ResolveOptions` appear nowhere in the documentation. For a middleware not tied to one instance the documentation offers `experimental_standaloneMiddleware`: "tRPC has a new experimental API called `experimental_standaloneMiddleware` which allows you to independently define a middleware that can be used with any tRPC instance." (`www/docs/server/middlewares.md:259`), and "Using `experimental_standaloneMiddleware` you can create a middleware that explicitly defines its requirements, i.e. the Context, Input and Meta types:" (line 262). The project's tests never use `MiddlewareFunction<…>` or `ProcedureParams` this way (`C1n-project-tests.txt`, 22 lines read). *Read, at the cut-off.*
- **owner-docs** (not applicable). No dependency owns the operation; `MiddlewareFunction`, `ProcedureParams` and `Overwrite` are tRPC's own types.
- **change** (checked; `C1n-change.txt`, 11 hits read, silent on helpers). The hits are the reviewer's proposal ("If A extends any, return A", "If both extend any, return B", …) and diff lines. Nothing about generic helpers or the default parameters. *Read, at the cut-off.*
- **maintainers** (checked; `C1n-maintainers-q1.json` to `q3.json`, 12 hits, opening posts read; #4713 and #3434 read with their comments). Queries `repo:trpc/trpc MiddlewareFunction`, `ProcedureParams middleware`, `reusable middleware type ctx`, before the cut-off. #4713 (opened 2023-08-12, merged 2023-08-24), by the contributor who later wrote this change's rewrite of the helper, added the standalone middleware: "Enables creation of middlewares that are not bound to any particular tRPC instance; instead they can define a type of `Context`/`Input` that they expect" (line breaks removed). In #3434 (2022-12-23) a user posted a helper typed `MiddlewareFunction<ProcedureParams<AnyRootConfig, C>, ProcedureParams>`; no maintainer answered it. *Read.*
- **public-code** (checked; `C1n-code-q1.json`, `C1n-code-q2.json`, `C1n-code-hits.json`). `"MiddlewareFunction<ProcedureParams" language:TypeScript`: 0 hits. `"ProcedureParams" "MiddlewareFunction" "@trpc/server" language:TypeScript`: 13 hits, all read. Three type a middleware as `MiddlewareFunction<{ _ctx_out: X } & ProcedureParams, …>`: `jden6/pray-with-us` (file last changed 2023-11-29), `olivermolina/Fantasy-Contest` (2023-05-04) and `olivermolina/nextjs-aws-fargate` (2024-04-24). Their shape compiles at head (`c1-06`, run). Two are generic helpers over `<T extends ProcedureParams>` that return `MiddlewareFunction<T, T>` (`arjovanramshorst/posthog-trpc`, first committed 2024-10-16, which passes `ctx` to a callback; `PrantaDas/tRPC-starter`, last changed 2025-02-24, which does not read `ctx`). They do not use the bare default and were not compiled here. The other eight are generated declaration files. No program in the failing bare form was found. *Read; code as it is now.*
- **documented-way** (checked; `c1-05`). The standalone middleware reading `ctx`: compiles at head. *Run.*

**How the promise is made.** None found. No documentation, test or announcement covers typing a helper with the default parameters, and both types are marked `@internal`. Against that, they are exported from the package's main entry, and the use worked before the change (*read*; *run*). Rule Promised 7a counts how a use behaved before only for a use that something else promises. Programs are shown typing middleware with these types, but only in a shape that still compiles.

## Intended or announced

Nothing in the change mentions it (read). Released in 10.43.3. The same failure at 10.43.4, 10.43.6 and 10.45.2 (run, later). On 2023-11-15, in issue #5037, the contributor who wrote this change's rewrite of the helper wrote about another report: "I think in your (Sentry's) case the `T` in your middleware gets inferred as `Promise<MiddlewareResult<U>>` where `U` is `ProcedureParams<AnyRootConfig, unknown, unknown, unknown, unknown, unknown, unknown>`" and "my suspicion is that since `any` behaves very bizarrely with conditional types (it becomes a union of both arms of the ternary type), that has something to do with it." (https://github.com/trpc/trpc/issues/5037#issuecomment-1812945802; *read*, later).

## What the affected person sees

A developer who wrote a reusable middleware typed `MiddlewareFunction<ProcedureParams, ProcedureParams>`, and reads `opts.ctx.something`, gets after upgrading `TS2339: Property 'session' does not exist on type '{}'.` on each read. The same code compiled at 10.43.2. The message names neither the helper nor the default parameters.

## What the maintainers did

No statement about this exact use was found, before or after the cut-off (read). The later fixes for the neighbouring problems, #5039 and #5057, did not change it: it still fails at 10.45.2 (run, later). The later comment above suspects the `any` behaviour in general, in a different report.

## How each fact is known

Types and compile results: *run* at base, head, three fixes and three later releases. Mechanism and export status: *read*. The comment's own statements: *reported*, each run here. Searches: *read*, saved.

## Relation to existing reference families and ruled claims

**Already decided?** No. The nearest is regrade ruling 12 (`regrade-12`), which widened GT-j2 to reads of an `any` context inside a `t.middleware()` function and after one. There the developer typed the instance's context as `any`. Here nobody writes `any`: it comes from the library's default parameters, in a helper not made by any instance. The packet asks whether this is part of GT-j2, part of GT-j1, or neither.

**Grouping tests (rule Before 4), run** (`results/table.md`, `results/known/table.md`). This candidate's failing cases are `c1-02` and `c1-03`.

| Fix | `c1-02`, `c1-03` | GT-j2's three saved cases | GT-j1's two saved generic cases |
| --- | --- | --- | --- |
| `gt-j2-fix-any` (GT-j2's fix at line 11: an `any` first type is merged as before the change) | both compile | all compile | both fail |
| `gt-j2-revert-first-gate` (line 11 back to `extends any`, its second branch removed) | both fail | all compile | both fail |
| `gt-j1-revert-second-gate` (GT-j1's fix) | both fail | all fail | both compile |

- Against **GT-j2**. *Same lines:* yes, head line 11 with `any` as the first type. *One fix:* yes, `gt-j2-fix-any` cures both. GT-j2's other saved fix, which reverts line 11, does not cure this one, because the object branch then returns `unknown` through lines 21 to 24. *One sentence:* yes: "when the first type is `any`, the new first check takes both branches, and the result no longer allows reads of other properties". Here the second type is `unknown`, so both branches give `unknown`.
- Against **GT-j1**. *Same lines:* cannot tell. Lines 12 and 21 to 24 produce one of the two `unknown` branches, but fixing them alone cures nothing here. *One fix:* no. *One sentence:* no; GT-j1's first type is an object, not `any`.

**Clean control.** The control state is `known-problems`.

## Both sides

*For a suggestion (outside supported use).* The use rests on two types the owners mark `@internal`. Neither is documented, and the project's tests never type a helper with them. The documented way to write an instance-independent middleware compiles. The public programs that type middleware with these types name their context type, and that shape still compiles. No program in the failing form was found. Rule Promised 7a: "It used to work", for a use nothing supports, is not a promise.

*For widening GT-j2 (or a problem).* The two types are exported from the package's main entry and ship in its declarations. Code that compiled at 10.43.2 stops compiling in a patch release. By all three tests it is GT-j2's fault, and the owner ruled GT-j2 eligible as "an unintended side effect that makes previously compiling code fail". The exported type allows exactly this use (rule Promised 6, "The accepted type allows it.").

## Recommendation

> **What happens:** a middleware or resolver helper typed with tRPC's default parameters, `MiddlewareFunction<ProcedureParams, ProcedureParams>`, can no longer read a property of `ctx` (TS2339 on `{}`), where it compiled before the change. **Promised: no**: both types are marked `@internal`, undocumented, and unused this way in the project's tests; the documented standalone middleware works; no program was found in the failing form. **Delivered: not asked.** **So: suggestion or observation (outside supported use).** **Band, decided separately:** none.

Confidence: low. *Conflict:* the `@internal` marks (no promise) against the export from the package's main entry together with rule Promised 6's accepted type and GT-j2's eligibility ground (a promise). *No precedent:* no ruling covers a type that is both exported and marked internal. The fact that would reverse it is a documented or tested use of these types for a helper, or a program shown in the failing form. None was found.

The rules do not decide whether an exported type marked `@internal` is promised, so this is the owner's. If the owner holds it promised, all three tests point to widening GT-j2. In that case the comment in C1 says what goes wrong for GT-j2.
