# 01 — `Overwrite` rewrite and the procedure input-merge path

Scope: `packages/server/src/core/internals/utils.ts` (lines 4–30 at head) and its consumers, mainly
`packages/server/src/core/internals/procedureBuilder.ts:32-47` (`CreateProcedureReturnInput`).

## Context: what the PR is fixing

`t.procedure.input(z.string()).use((opts) => opts.next())` inferred a mangled input type. The reason
is that `.use()` gives the middleware the builder's current params (`MiddlewareFunction<TParams, $Params>`),
and `opts.next()` returns `MiddlewareResult<TParams>`. So `$Params['_input_in']` is just the procedure's
existing input sent back unchanged. `CreateProcedureReturnInput` then evaluates
`Overwrite<TPrev['_input_in'], TNext['_input_in']>` = `Overwrite<string, string>`. The old `Overwrite`
mapped over `keyof string` and produced `{ charAt: …; charCodeAt: … }`.

The PR fixes this by rewriting the shared `Overwrite` so it only merges key by key when both sides are
`object`. In every other case `TWith` replaces `TType`.

`Overwrite` has six call sites. Two are the input merges in `procedureBuilder.ts:41,44`. The other four
merge context: `procedureBuilder.ts:38`, `middleware.ts:65,81,103,136` and `utils.ts:71`
(found with `grep -rn "Overwrite<" packages/*/src`).

## Measurements and commands

All checks ran with the clone's own compiler (`packages/tests/node_modules/.bin/tsc`). Scratch files are in
`clone-work/scratch/`. The clone was not modified (`git status --short` is empty afterwards).

1. **Head baseline.** `cd packages/tests && ./node_modules/.bin/tsc --noEmit -p tsconfig.json` exits 0 with
   no output. The PR's own regression test type-checks.
2. **`Overwrite` matrix** (`scratch/overwrite-matrix.ts`, imports the head `Overwrite` directly):
   - `Overwrite<string, string>` = `string`. Passes; this is the PR's target.
   - `Overwrite<string[], string[]>` ≠ `string[]`. **Fails**: arrays are `object`, so they still go through the
     key-by-key mapped type.
   - `Overwrite<{t:'a';x:1}|{t:'b';y:2}, same>` ≠ the input union. **Fails**: the type distributes over both
     sides and cross-merges into `{t:'b';x;y}` and `{t:'a';x;y}`.
   - `Overwrite<{a:1}, never>` = `never`, and `Overwrite<string, never>` = `never`.
   - `Overwrite<{a:1}, unknown>` = `unknown` at head. At base (`OldOverwrite`, copied verbatim) it was `{a:1}`.
     **This is a behaviour change.**
   - `Overwrite<{a:1}, any>` = `any` at head.
   - `Overwrite<{a:1}|{b:2}, {c:3}>` still distributes to `{a:1;c:3}|{b:2;c:3}`, so context unions keep
     working.
   - An 18-case matrix (primitives, `never` on either side, `unknown`, `any`, unions, `undefined`, `{}`) shows
     that the head `Overwrite` is identical to the collapsed form proposed below (check `c9` passes).
3. **End-to-end procedure inference** (`scratch/procedure-cases.ts`, run against the head source,
   the base source (`git archive main packages/server/src`) and a "judo" variant):

   | procedure (`.input(X).use(o => o.next())`) | base | head (this PR) | judo variant |
   | --- | --- | --- | --- |
   | `z.string()` | mangled | `string` ✅ | `string` ✅ |
   | `z.array(z.string())` | mangled | **mangled** (`{ [x: number]: string; [iterator]…; concat…; …31 more… }`) | `string[]` ✅ |
   | discriminated union `a`/`b` | 4-way cross-merged union | **4-way cross-merged union** | `{t:'a';x}\|{t:'b';y}` ✅ |
   | `z.date()` | passes the `Equal` check | passes | passes |
   | `z.object({a}).optional()` | `{} \| {a} \| …` (bogus `{}`) | `void \| {a} \| {a} \| undefined` | `void \| {a} \| undefined` |

4. **Regression safety of the judo variant.** `scratch/tsconfig.judo-full.json` type-checks the whole
   `packages/tests` tree against the judo copy of `packages/server/src`. `scratch/tsconfig.head-full.json`
   does the same against an untouched `git archive review-head` copy. Pointing `@trpc/server` at a copy
   outside the repo adds harness noise: 562 errors, mostly duplicate `unique symbol` identity in the
   react/client tests. **The two sorted error sets are byte-identical** (`diff h.err j.err` prints nothing).
   None of the input and context regression suites (`issue-4947-merged-middleware-inputs`,
   `issue-4321-context-union-inference`, `issue-4527-nested-middleware-root-context`,
   `issue-2856-middleware-infer`, `issue-5020-inference-middleware`, `server/middleware.test.ts`,
   `inferenceHelpers.test.ts`) has an error under either variant.
   Verification status: **partial**. The harness noise could hide a regression inside the noisy files,
   but none of the files that exercise input or context merging are noisy.

## Finding 1.1 — The fix patches a shared primitive instead of the input-merge rule, and misses the same bug for arrays and unions

The actual defect is in `CreateProcedureReturnInput` (`procedureBuilder.ts:39-44`). That code assumes
anything a middleware reports as `_input_in`/`_input_out` is a new input that must be merged. For an inline
`.use()` middleware, though, it is always the procedure's own input sent back through
`MiddlewareResult<TParams>`. Merging a type with itself should be a no-op. The code had no rule for that, so
it depended on `Overwrite<T, T>` being an identity, which it isn't. The PR doesn't add that rule. Instead it
changes `Overwrite` itself, which is also the merge for all six context call sites, so that `Overwrite<T, T>`
happens to be an identity when `T` is a primitive. That makes the fix incomplete in exactly the way measured
above:

- `.input(z.array(z.string())).use(mw)` still infers a mangled array-prototype object. Arrays and tuples
  are `object`, so they take the key-mapping branch. The commit message says "only overwrite keys of
  objects", but arrays count as objects to the compiler, so this is the same bug the PR set out to fix.
- `.input(z.discriminatedUnion(...)).use(mw)` still infers impossible hybrid members such as
  `{ t: 'b'; x: number; y: string }`, because `Overwrite` distributes over both unions and merges every
  pair.

The change also affects callers that were never broken. The context call sites now get
`Overwrite<X, unknown> = unknown` (it used to be `X`) and `Overwrite<X, any> = any`. I haven't found a public
path that produces `unknown` context today. Still, the PR changes the semantics of a helper shared by six
sites to fix one of them, and adds no type tests for the other five.

**Code-judo proposal.** Add the missing rule where it belongs and leave `Overwrite` as a pure object merge.
A middleware whose input already covers the procedure's input (the pass-through case, or a standalone
middleware that needs a subset, as in #4947) does not change the input:

```ts
// procedureBuilder.ts
/** A middleware that passes its input through (or only needs a subset) doesn't change the input */
type MergeInput<TPrev, TNext> = UnsetMarker extends TNext
  ? TPrev
  : [TPrev] extends [TNext]
  ? TPrev
  : Overwrite<TPrev, TNext>;

type CreateProcedureReturnInput<TPrev extends ProcedureParams, TNext extends ProcedureParams> =
  ProcedureBuilder<{
    _config: TPrev['_config'];
    _meta: TPrev['_meta'];
    _ctx_out: Overwrite<TPrev['_ctx_out'], TNext['_ctx_out']>;
    _input_in: MergeInput<TPrev['_input_in'], TNext['_input_in']>;
    _input_out: MergeInput<TPrev['_input_out'], TNext['_input_out']>;
    _output_in: FallbackValue<TNext['_output_in'], TPrev['_output_in']>;
    _output_out: FallbackValue<TNext['_output_out'], TPrev['_output_out']>;
  }>;
```

Together with **reverting `Overwrite` to its base definition**, this variant (`scratch/judo/…`) fixes
the string case, and also arrays, discriminated unions and the bogus `{}` member in the optional-object
case. It also collapses the two duplicated three-line ternaries into one named rule. In the controlled
comparison it produces an identical error set across the test package. The net diff is about 7 added and
6 removed lines in one file, compared with a 17-line rewrite of a shared utility plus the context-semantics
changes described above.

If the author still wants `Overwrite` hardened against non-objects as defence in depth, do it in a separate,
tested change (see 1.2). It isn't needed for this bug.

## Finding 1.2 — The new `Overwrite` is a four-arm nest with a dead branch and a doc comment that misstates the `never` case

Head `utils.ts:11-30` has four conditional levels and three "just return `TWith`" leaves:

```ts
export type Overwrite<TType, TWith> = TType extends object
  ? TWith extends object
    ? { …merge… }
    : TWith extends any ? TWith : never
  : TType extends any
  ? TWith extends any
    ? TWith
    : TType        // line 29 — unreachable
  : never;
```

`TWith extends any ? TWith : X` distributes over `TWith`. When `TWith` is `never` it produces `never` and
never evaluates `X`, and every other type extends `any`. So the `: TType` on line 29 and the `: never` on
line 24 can't be reached. The doc comment says TType is overwritten "unless TWith is never", which suggests
that `TType` survives when `TWith` is `never`. It doesn't: `Overwrite<{a:1}, never>` = `never` (verified,
check `c2`). A reader has to work through distributive-conditional rules to see that three of the leaves say
the same thing.

**Remedy.** If `Overwrite` keeps the object guard, write it in the equivalent form that makes the rule
visible. The 18-case matrix (check `c9`) verified that it matches the head version:

```ts
/**
 * @internal
 * Merge `TWith` over `TType` key by key when both are objects; otherwise `TWith` wins.
 * Distributes over unions on both sides.
 */
export type Overwrite<TType, TWith> = TWith extends any
  ? TType extends object
    ? TWith extends object
      ? { [K in keyof TType | keyof TWith]: K extends keyof TWith ? TWith[K] : K extends keyof TType ? TType[K] : never }
      : TWith
    : TWith
  : never;
```

This form also keeps the distribution behaviour that the context-union tests depend on. Better still,
follow 1.1 and don't change `Overwrite` in this PR at all.

## Finding 1.3 (minor, noted for completeness) — Overwrite/merge helper sprawl

`internals/utils.ts` already exports `Overwrite` and `OverwriteKnown`. `procedureBuilder.ts:69` has a private
`OverwriteIfDefined` (`Simplify<TType & TWith>`, used by `.input()`). `types.ts:19` exports `FlatOverwrite`.
`DefaultValue` is re-exported as `FallbackValue`. That makes four "overwrite" spellings with different
semantics for primitives, unions and optional keys. `.input()` and `.use()` merge inputs with different
helpers, which is why they disagree on edge cases. This PR didn't create the sprawl, so it isn't a
blocker. It is, however, the reason the bug fix got aimed at the wrong helper. A follow-up should give input
merging one named rule, `MergeInput` from 1.1, and use it from both `.input()` and `CreateProcedureReturnInput`.
