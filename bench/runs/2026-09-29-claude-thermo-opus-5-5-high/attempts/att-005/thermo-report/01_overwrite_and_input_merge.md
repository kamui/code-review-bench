# 01 — `Overwrite` and the procedure-builder input merge

Scope: `packages/server/src/core/internals/utils.ts` (the rewritten `Overwrite`) and the call site that the fix is actually aimed at, `CreateProcedureReturnInput` in `packages/server/src/core/internals/procedureBuilder.ts:32-47`, plus the middleware `next()` typing in `packages/server/src/core/middleware.ts:129-158`.

All type checks below used the clone's own compiler (`packages/tests/node_modules/.bin/tsc`), offline, with scratch files under `clone-work/probe/` that import the clone's source by relative path or by the repo's `@trpc/server -> packages/server/src` path mapping. The clone itself was never modified; `git status --porcelain` was empty after the review.

Baseline: `cd packages/tests && ./node_modules/.bin/tsc --noEmit -p tsconfig.json` at head exits clean (about 8 s), so the PR's own regression test type-checks.

---

## Finding 1.1 — The fix lives in the wrong layer: a shared ctx-merging helper was widened to paper over one input rule in `.use()`

**Severity:** structural regression / missed code-judo move. **Verification:** CONFIRMED (tsc, see below).

### What the diff does

`Overwrite<TType, TWith>` used to have one job: merge two object types key by key. It has seven call sites. Five of them merge **context** (`procedureBuilder.ts:38`, `middleware.ts:65`, `middleware.ts:103`, `middleware.ts:136`, `utils.ts:71`), one merges whole params objects (`middleware.ts:81`), and it is re-exported to downstream packages through `packages/server/src/internals.ts:11`. Only two call sites apply it to **inputs**, both in `CreateProcedureReturnInput`:

```ts
// procedureBuilder.ts:39-44 (unchanged by the PR)
_input_in: UnsetMarker extends TNext['_input_in']
  ? TPrev['_input_in']
  : Overwrite<TPrev['_input_in'], TNext['_input_in']>;
_input_out: UnsetMarker extends TNext['_input_out']
  ? TPrev['_input_out']
  : Overwrite<TPrev['_input_out'], TNext['_input_out']>;
```

The reported bug was `.input(z.string()).use(mw)` producing a garbled input type. The PR fixes it by changing the shared helper into a three-way policy: key-merge when both sides are `object`, wholesale replacement otherwise, with extra arms meant to handle `never`. Every ctx call site now runs through that policy too, even though ctx types are always objects and never needed it.

### Why this is the wrong place

The real defect is that `.use()` feeds inputs through a merge at all. Look at how a middleware produces its output params (`middleware.ts:143-155`):

```ts
next: {
  (): Promise<MiddlewareResult<TParams>>;
  <$Context>(opts: { ctx: $Context }): Promise<
    MiddlewareResult<{
      ...
      _input_in: TParams['_input_in'];
      _input_out: TParams['_input_out'];
      ...
```

A middleware cannot change the input type. Every `next()` overload hands the previous `_input_in`/`_input_out` back unchanged. So for an inline `.use()`, `TNext['_input_*']` is by construction the same type as `TPrev['_input_*']`, and `CreateProcedureReturnInput` ends up computing `Overwrite<X, X>`. Merging a type with itself should give back that type. The key-remapping mapped type only does that for plain object types. The PR taught `Overwrite` to survive one more shape of `X` (primitives), when the call site should never have been merging identical types in the first place.

A standalone middleware (`experimental_standaloneMiddleware<{ input: I }>()`) only declares a *requirement*, and `MiddlewareBuilder<TParams, $Params>` assignability already enforces it. The procedure's input should still be the previous one. `issue-4947-merged-middleware-inputs.test.ts` asserts exactly that. The only builder path that genuinely combines two input types is `unstable_concat` (`procedureBuilder.ts:150-154`). For that combination the builder already has a canonical rule: `.input()` chaining uses `OverwriteIfDefined` (`procedureBuilder.ts:69-71`, `Simplify<TType & TWith>`), which matches the runtime merge of object parser outputs.

### The code-judo move

Leave `Overwrite` exactly as it was on `main` (an object merge for ctx). Replace the two ad-hoc ternaries at the call site with one named rule that reuses the existing input-merge helper:

```ts
type CreateProcedureReturnInput<
  TPrev extends ProcedureParams,
  TNext extends ProcedureParams,
> = ProcedureBuilder<{
  _config: TPrev['_config'];
  _meta: TPrev['_meta'];
  _ctx_out: Overwrite<TPrev['_ctx_out'], TNext['_ctx_out']>;
  _input_in: MergeInput<TPrev['_input_in'], TNext['_input_in']>;
  _input_out: MergeInput<TPrev['_input_out'], TNext['_input_out']>;
  _output_in: FallbackValue<TNext['_output_in'], TPrev['_output_in']>;
  _output_out: FallbackValue<TNext['_output_out'], TPrev['_output_out']>;
}>;

/**
 * Middlewares hand the previous input straight through `next()`, so `.use()` only
 * contributes an input when it is a parser chain (`unstable_concat`); merge those
 * exactly like chained `.input()` calls do.
 */
type MergeInput<TPrev, TNext> = UnsetMarker extends TNext
  ? TPrev
  : [TPrev] extends [TNext]
  ? TPrev
  : OverwriteIfDefined<TPrev, TNext>;
```

This deletes the entire utils.ts hunk (+17/−3 becomes 0) and names the input rule in the file that owns the procedure builder. It also fixes the cases the PR still gets wrong (Finding 1.2). The `[TPrev] extends [TNext]` arm says "a middleware passed my input through or only required a supertype of it". That is the actual invariant, written down once.

A narrower alternative is to give `.use()` its own return type that copies `TPrev['_input_*']` verbatim and to keep a merge only for `unstable_concat`. That is equally valid and even more explicit. The snippet above just keeps the existing shared `CreateProcedureReturnInput` shape.

### Verification of the proposal

I checked it in a scratch copy of the repo tree: `git archive review-head`, `utils.ts` restored from `main`, `procedureBuilder.ts` patched as above, dependencies symlinked from the clone. The scratch procedure-level probe asserts, using `inferRouterInputs`:

| case | head | proposal (with main's `Overwrite`) |
| --- | --- | --- |
| `.input(string).use(mw)` → `string` | pass | pass |
| `.input(string[]).use(mw).use(mw)` → `string[]` | **fail** (mangled array object) | pass |
| `.input({ a?: string }).use(mw)` → `{ a?: string }` | **fail** (`{ a: string \| undefined }`) | pass |
| `.input({a,b,c}).use(standalone<{input:{a}}>)` → `{a,b,c}` | pass | pass |
| `.input({ a?: string }).unstable_concat(input({ z }))` → `{ a?: string; z: number }` | **fail** (`{ a: string \| undefined; z: number }`) | pass |

A full `packages/tests` type-check of the proposal was run next to an unmodified-head control in the same scratch harness. Both produced the identical set of 570 diagnostics, and `diff` of the two error lists was empty. Those diagnostics come from the scratch harness itself: symlinked `node_modules` and dual package resolution. They are not code problems; the clone's own head type-check is clean. So the proposal adds no new type errors, the PR's new regression test included. The scratch trees were deleted afterwards.

---

## Finding 1.2 — The fix is incomplete: arrays, tuples and optional keys are still mangled by `.use()`

**Severity:** correctness defect in the area the PR claims to fix. The array case also mangles at base; the PR does not make it worse, but it misses it. **Verification:** CONFIRMED (tsc).

The new gate is `TType extends object`. Arrays, tuples and functions are `object` in TypeScript. They still go through the non-homomorphic mapped type `{ [K in keyof TType | keyof TWith]: ... }`, which flattens every array method into a plain property bag. And because the mapped type is not homomorphic (the key set is a union), it also drops `?` modifiers from plain objects.

Probe `clone-work/probe/probe4.ts` (type-level, against head `Overwrite`):

```ts
Overwrite<string[], string[]>                 // ≠ string[]            (fails; base also fails)
Overwrite<[string, number], [string, number]> // ≠ the tuple           (fails)
Overwrite<{ a?: string }, { a?: string }>     // ≠ { a?: string }      (fails: a becomes required)
```

Procedure-level (scratch probes, removed afterwards):

```ts
t.procedure.input(arr /* string[] */).use((o) => o.next()).query(...)
// inferRouterInputs -> { [x: number]: string; [iterator]: ...; concat: ...; ... 34 more ...; findLastIndex: ... }

t.procedure.input(opt /* { a?: string } */).use((o) => o.next()).query(...)
const bad: I['mw'] = {};
// error TS2741: Property 'a' is missing in type '{}' but required in type '{ a: string | undefined; }'.
```

`z.array(...)` inputs and objects with optional fields are at least as common as bare `z.string()` inputs, so a client calling any such procedure behind a middleware still sees a wrong input type. Patching `Overwrite` further (an `any[]` arm, a homomorphic rewrite, and so on) would add still more branches to a shared helper. The call-site rule in Finding 1.1 fixes all of these at once, because it never maps over the input type.

---

## Finding 1.3 — The new `Overwrite` has dead arms, and its doc comment contradicts its behaviour

**Severity:** type-contract / legibility. **Verification:** CONFIRMED (tsc).

The doc comment says the helper "will overwrite the entire TType with TWith, unless TWith is never", which suggests `TType` survives when `TWith` is `never`. It does not. Both conditionals are distributive over a naked type parameter, so a `never` `TWith` short-circuits to `never` before any arm is chosen:

```ts
Overwrite<{ a: 1 }, never>  // never   (probe.ts P1 — passes as `never`)
Overwrite<string, never>    // never   (probe.ts P2 — passes as `never`)
```

As a result, the `: TType` arm at `utils.ts:29` can never be taken. The `TWith extends any ? ... : never` wrapper at lines 21-24 and the `TType extends any ? ... : never` wrapper at lines 25-30 only re-trigger distribution that has already happened. The 20-line nested ternary with three commented arms is equivalent to this:

```ts
export type Overwrite<TType, TWith> = TType extends object
  ? TWith extends object
    ? { [K in keyof TType | keyof TWith]: K extends keyof TWith ? TWith[K] : K extends keyof TType ? TType[K] : never }
    : TWith
  : TWith;
```

Probe `clone-work/probe/probe6.ts` checks mutual identity (via the `<T>() => T extends X ? 1 : 2` equality trick) between head's `Overwrite` and this collapsed form over 17 pairs. The pairs cover primitives, objects, `never` on either side, `unknown`, `undefined`, `null`, `symbol`, unions on either side and arrays, plus `any`. All of them are identical.

If Finding 1.1 is adopted, this finding goes away because the hunk is reverted. If the maintainers do want a widened `Overwrite`, it should be the collapsed form, with a comment that states what actually happens: "`never` on either side yields `never`; unions distribute; arrays/functions are treated as objects".
