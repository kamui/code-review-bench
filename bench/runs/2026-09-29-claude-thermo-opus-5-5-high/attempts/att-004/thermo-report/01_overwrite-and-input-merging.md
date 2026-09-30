# 01 — `Overwrite` and procedure input merging

Scope: `packages/server/src/core/internals/utils.ts` (the only production change in the PR), read together with the `Overwrite` call sites it now serves: `packages/server/src/core/internals/procedureBuilder.ts:38-44` (`CreateProcedureReturnInput`), `packages/server/src/core/middleware.ts:65,81,103,136`, and `utils.ts:71` (`ResolveOptions`).

Range reviewed: `main...review-head` (`2abb2d5c..7dc04a7e`). File sizes are not a concern: `utils.ts` goes from 81 to 95 lines and `procedureBuilder.ts` is 416 lines and unchanged.

## Root cause, as established from the code

`ProcedureBuilder.use()` returns `CreateProcedureReturnInput<TParams, $Params>`. For an ordinary pass-through middleware (`(opts) => opts.next()`), `next()` is typed as returning `MiddlewareResult<TParams>` (`middleware.ts:144-145`), so `$Params` is `TParams` itself. `TNext['_input_in']` is therefore identical to `TPrev['_input_in']`. It is not `UnsetMarker`, so the guard at `procedureBuilder.ts:39` falls through to `Overwrite<TPrev['_input_in'], TNext['_input_in']>`. That line was introduced by #4948/#4973 (`git log -L39,44:packages/server/src/core/internals/procedureBuilder.ts main`) to merge the declared inputs of standalone middlewares. Base `Overwrite` maps over `keyof TType | keyof TWith` unconditionally, so `Overwrite<string, string>` becomes an object carrying `charCodeAt`, `length` and the rest. That is the bug the PR's regression test reproduces.

The PR fixes it by making the shared `Overwrite` helper branch on `TType extends object` / `TWith extends object`.

## Finding 1.1 — The fix generalises a shared ctx helper to repair the input path; the existing input-merge helper does the job with a two-token change

**Where:** `packages/server/src/core/internals/utils.ts:11-30`, which exists to serve `packages/server/src/core/internals/procedureBuilder.ts:39-44`.

**Problem.** `Overwrite` had one meaning: key-wise object merge where the right-hand side wins. Every other call site uses it that way. `_ctx_out` is always an object (`deriveParamsFromConfig` seeds it with `{}`, and `next({ ctx })` is the only other producer), and `middleware.ts:81` overwrites whole `ProcedureParams` records. The PR turns `Overwrite` into a dual-mode type: key-wise merge when both sides extend `object`, whole-value replacement otherwise. It does this because one caller, the input path, feeds it non-object types. The ctx callers never need replacement mode, but they now go through it, and the new semantics leak into them. For example, `Overwrite<{ a: 1 }, unknown>` was `{ a: 1 }` on `main` and is `unknown` at head (verified, probe P3 below). `Overwrite` is also re-exported from the package's internals entry (`packages/server/src/internals.ts:11`), so the semantic change reaches beyond this file. Fixing the problem this way adds concepts to a shared primitive when the actual defect is that the input path uses the wrong primitive.

The input path composes a procedure's input from the `.input()` parsers plus any input a standalone middleware declares, and the `.use()` parameter type already requires the procedure input to be assignable to that declaration. That is an intersection of requirements, not a right-biased overwrite. `procedureBuilder.ts` already has that operation and already uses it for `.input()` chaining: `OverwriteIfDefined<TType, TWith> = UnsetMarker extends TType ? TWith : Simplify<TType & TWith>` (`procedureBuilder.ts:69-71`). The canonical `Simplify` (`types.ts:60-62`) already passes arrays and `Date` through untouched, and `string & string` is just `string`.

**Code-judo proposal (verified).** Revert `utils.ts` to `main` and change the two `use()` merge lines to call the helper the file already owns:

```ts
type CreateProcedureReturnInput<TPrev extends ProcedureParams, TNext extends ProcedureParams> =
  ProcedureBuilder<{
    _config: TPrev['_config'];
    _meta: TPrev['_meta'];
    _ctx_out: Overwrite<TPrev['_ctx_out'], TNext['_ctx_out']>;
    _input_in: UnsetMarker extends TNext['_input_in']
      ? TPrev['_input_in']
      : OverwriteIfDefined<TPrev['_input_in'], TNext['_input_in']>;
    _input_out: UnsetMarker extends TNext['_input_out']
      ? TPrev['_input_out']
      : OverwriteIfDefined<TPrev['_input_out'], TNext['_input_out']>;
    _output_in: FallbackValue<TNext['_output_in'], TPrev['_output_in']>;
    _output_out: FallbackValue<TNext['_output_out'], TPrev['_output_out']>;
  }>;
```

Then rename `OverwriteIfDefined` to what it actually does (for example `IntersectIfDefined` or `MergeInput`), since it intersects rather than overwrites. After that, `Overwrite` goes back to being an object-only ctx helper, and no new branches are needed anywhere. The existing `UnsetMarker extends TNext[...]` guard must stay in front: plain `t.middleware(...)` reports `_input_in: unknown`, and `UnsetMarker extends unknown` is what keeps `.use(mw).input(z.object(...))` legal. A first variant that folded that guard into the helper after the `TType` check broke `issue-4527-nested-middleware-root-context.test.ts` and `middlewares.test.ts`, so the guard order matters. The version above is the verified one.

What the proposal buys, measured against head:

| Probe | `main` | head (PR) | proposal |
| --- | --- | --- | --- |
| P6 `.input(string[]).use(passthrough)` input is `string[]` | fail | **fail** | pass |
| P8 `.input([string, number]).use(passthrough)` input is the tuple | fail | **fail** | pass |
| P10 `.input(number).use(passthrough)` input is `number` | fail | pass | pass |
| PR's own test (`issue-5020-…`) | fail | pass | pass |
| P12 `.input({ a: 'x' }).use(standalone<{ input: { a: string } }>)` keeps `{ a: 'x' }` | fail (widened to `string`) | fail (widened) | pass |
| P3 `Overwrite<{ a: 1 }, unknown>` is `{ a: 1 }` | pass | **fail** (`unknown`) | pass (unchanged from main) |

P12 is a pre-existing defect (right-biased overwrite widens a narrowed parser type to the middleware's looser declaration). The proposal fixes it for free because intersection is the correct model for this path.

**Verification status:** confirmed with tsc (commands below). All six regression tests that exercise `.use()` inference pass under the proposal (2856, 4321, 4527, 4645, 4947, and the PR's 5020), as does `middlewares.test.ts`. A full `packages/tests` type-check in a scratch copy of the head tree gave byte-identical error sets for head and proposal (570 lines each, all caused by the scratch copy's module layout; the real clone type-checks clean at head). So the proposal adds no new type errors anywhere in the test suite.

## Finding 1.2 — The new `extends object` test still mangles array and tuple inputs, so the bug the PR targets is only fixed for primitives

**Where:** `packages/server/src/core/internals/utils.ts:11-20`.

**Problem.** Arrays, tuples, `Map`, `Set` and functions all satisfy `extends object`, so they still reach the non-homomorphic `{ [K in keyof TType | keyof TWith]: … }` mapping. Because the key set is a union of `keyof` expressions rather than `keyof T` of a type parameter, TypeScript does not preserve array or tuple shape. It produces a plain object with `length`, `push`, `[Symbol.iterator]` and the rest. A procedure written `t.procedure.input(z.array(z.string())).use((opts) => opts.next())` is still broken after this PR. tsc reports the inferred router input as `{ [x: number]: string; [iterator]: () => IterableIterator<string>; [unscopables]: …; … 31 more …; flat: … }` instead of `string[]`, and the tuple case likewise comes out as `{ [x: number]: string | number; …; 0: string }`. This is the same class of garbling the PR's own review thread describes for `Overwrite<string, string>`. (`Date` happens to survive, because the mapped copy is structurally identical to `Date`.)

**Remedy.** Adopt Finding 1.1, which removes `Overwrite` from the input path entirely. If the author wants to keep the `Overwrite` approach, the guard would have to become "plain record" (exclude `readonly unknown[]` and callables at minimum), which is more special-casing piled onto a shared helper. That is the reason to prefer 1.1. Either way, add array and tuple cases to the regression test.

**Verification status:** confirmed with tsc (probes P6 and P8 fail at head; the actual types were printed via assignment errors in `show.ts`).

## Finding 1.3 — Two of the new branches are unreachable, and the doc comment describes behaviour the type does not have

**Where:** `packages/server/src/core/internals/utils.ts:6-9` (doc), `:21-24` and `:25-30` (branches).

**Problem.** `TWith` is a naked type parameter, so `TWith extends object ? … : …` distributes over it. By the time the false branch runs, `TWith` is a single non-`never` member, and `TWith extends any ? TWith : never` (lines 21-24) always takes `TWith`. The same reasoning applies to lines 26-29: `TWith extends any ? TWith : TType` can never yield `TType`, because a `never` `TWith` distributes to `never` before the branch is chosen. The comment says the type overwrites "unless TWith is never", which implies `TType` survives in that case. It does not: `Overwrite<{ a: 1 }, never>` and `Overwrite<string, never>` are both `never` at head. So the new code has 20 lines, two fallbacks that never fire, and a comment that promises a third behaviour. For type-level code, where readers cannot step through execution, this is exactly the kind of misleading structure that makes the next edit wrong.

**Remedy.** If `Overwrite` stays dual-mode, collapse it to its actual behaviour and fix the comment:

```ts
/**
 * @internal
 * Key-wise overwrite when both sides are objects; otherwise `TWith` replaces `TType`.
 */
export type Overwrite<TType, TWith> = TType extends object
  ? TWith extends object
    ? {
        [K in keyof TType | keyof TWith]: K extends keyof TWith
          ? TWith[K]
          : K extends keyof TType
          ? TType[K]
          : never;
      }
    : TWith
  : TWith;
```

This three-branch form is type-identical to head on all 15 probed pairs, including `never`, `unknown`, unions on either side, arrays, `undefined` and `null`. Preferably, adopt 1.1 and revert `Overwrite` to `main`, so none of these branches exist.

**Verification status:** confirmed with tsc (`collapse.ts`: all 15 cases compare equal; `show.ts`: `Overwrite<{a:1}, never>` and `Overwrite<string, never>` are assignable to `1`, i.e. `never`).

## Commands and evidence

All scratch files are under `clone-work/scratch/`; the clone was not modified (`git status --short` is empty after the runs).

- In-repo head check: `cd clone/packages/tests && ./node_modules/.bin/tsc --noEmit -p tsconfig.json` completes with no errors (about 6.5 s).
- `scratch/probe.ts` against head `Overwrite`/router (`tsc -p tsconfig.json`): failing probe lines 11 (`Overwrite<string[],string[]>`), 15 (`Overwrite<{a:1},unknown>`), 17 and 19 (`never` cases), 30 (router input `string[]`), 32 (router tuple input). Line 13 (`Date`) passes.
- `scratch/probe_base.ts` against `main` (`git archive main packages/server/src` into `scratch/base`): additionally fails line 8 (`Overwrite<string,string>`) and line 34 (router `number` input), and passes line 15. So the `unknown` change is new in this PR.
- `scratch/show.ts`: prints the mangled array and tuple types quoted in Finding 1.2.
- `scratch/tsconfig.{head,alt}.json` with `t_head/`, `t_alt/`: regression tests 2856, 4321, 4527, 4645, 4947, 5020 and `middlewares.test.ts` report no errors under either variant. `concat.test.ts:53` errors identically in both, an artifact of path-mapping its relative helpers. Probes 30, 32, 34 and 43 (P12) pass only under the proposal.
- `scratch/full` (a `git archive review-head` copy with symlinked `node_modules`): `alt_err.txt` and `head_err.txt` are identical (`diff` reports `IDENTICAL`).
- `scratch/collapse.ts`: equivalence of the collapsed form, 15 of 15 cases `true`.
