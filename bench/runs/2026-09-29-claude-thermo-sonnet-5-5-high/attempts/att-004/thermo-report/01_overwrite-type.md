# 01 — `Overwrite` type (`packages/server/src/core/internals/utils.ts`)

Verification status for everything below: verified with `packages/tests/node_modules/.bin/tsc --noEmit --strict --skipLibCheck --target es2022` on scratch files under `clone-work/scratch/` that import the type from the PR head. Unrelated module-resolution errors from importing server source without the tests tsconfig were ignored; they do not affect the result of the probes.

## The type as merged (head, lines 11-30)

```ts
export type Overwrite<TType, TWith> = TType extends object
  ? TWith extends object
    ? { [K in keyof TType | keyof TWith]: ... }   // key-merge
    : TWith extends any
    ? TWith                                        // (A)
    : never
  : TType extends any
  ? TWith extends any
    ? TWith                                        // (B)
    : TType                                        // (C) unreachable
  : never;
```

## Finding 1 — redundant arms; worked code-judo proposal

Every non-merge arm returns `TWith`. `TWith extends any ? TWith : never` is `TWith` (distribution over a union re-forms the union; over `never` gives `never`). Likewise `TType extends any ? (TWith extends any ? TWith : TType) : never` is `TWith`. Proposal:

```ts
export type Overwrite<TType, TWith> = TType extends object
  ? TWith extends object
    ? { [K in keyof TType | keyof TWith]: K extends keyof TWith ? TWith[K] : K extends keyof TType ? TType[K] : never }
    : TWith
  : TWith;
```

Measurement: scratch/c.ts compared the merged `Overwrite` with this version using an `Eq` helper over the cross product of `[string, {a:1}, {a:1}|{b:2}, never, unknown, any, string[], undefined, null, symbol, () => 1, {}, string | {a:1}]` with itself (169 pairs). Result: the "differing pairs" type resolved to `never`, i.e. no differences. The proposal deletes 9 lines and two `extends any` idioms with no behaviour change.

## Finding 2 — unreachable arm (C) and false doc comment

Probe: `const r1: Overwrite<string, never> = 0 as any as { __r1: 1 }` fails with "Type '{ __r1: 1; }' is not assignable to type 'never'". So `Overwrite<string, never>` is `never`. Same for `Overwrite<{a:1}, never>` (probe r7). The doc comment on lines 6-9 ("unless TWith is never") therefore claims a `TType`-preserving behaviour that never occurs. Cause: `TWith extends any` is distributive on a naked type parameter, and distribution over `never` yields `never` without evaluating either branch.

## Finding 3 — arrays / `Date` / functions

Probe r3 `Overwrite<string[], string[]>`: tsc reports the assigned `{ __r3: 1 }` object as "missing the following properties from type '{ [x: number]: string; [iterator]: ...; concat: ...; ... 32 more ...; flat: ... }'", i.e. the result is a plain object with all Array members, not `string[]`. Probe on `Overwrite<Date, {a:1}>` likewise yields an object with `Date`'s methods merged with `a`. Cause: `extends object` is true for arrays and functions, and the `[K in keyof A | keyof B]` mapped type is not homomorphic so it does not preserve array-ness. Suggested remedy: gate the merge on a record-like check (not array, not function) or fall through to `TWith` for those.

## Finding 4 — `unknown` / `any` on the right

Old implementation (scratch/old.ts): `Overwrite<{a:1}, unknown>` keeps `a` (assigning `{__r2:1}` errors with "Property 'a' is missing"). New implementation: probe r2 assigned `{__r2:1}` without error, so the result is `unknown`. Probe r5 (`Overwrite<{a:1}, any>`) also accepts an arbitrary value; old version kept `a`. Call sites that pass `_ctx_out` through this type: `procedureBuilder.ts:38`, `middleware.ts:65`, `middleware.ts:103`, `middleware.ts:136`, and `ResolveOptions` at `utils.ts:71`. I did not find a call path in the repo that feeds `unknown` into these, so the practical impact is unconfirmed; the contract change is unpinned by tests.

## Related observation (not a separate finding)

`procedureBuilder.ts:39-44` guards `_input_in`/`_input_out` with `UnsetMarker extends TNext[...]` before calling `Overwrite`, while `middleware.ts` uses `FallbackValue` for the same job on the same fields. Two spellings of one concept are maintained side by side (the existing `FIXME` at `middleware.ts:~92` acknowledges this). Out of scope for this diff, but the cleaned-up `Overwrite` would make unifying them easier.
