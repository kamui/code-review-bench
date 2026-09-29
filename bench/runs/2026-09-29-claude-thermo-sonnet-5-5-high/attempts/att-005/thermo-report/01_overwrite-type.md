# 01 — `Overwrite` type (`packages/server/src/core/internals/utils.ts`)

## Code under review (head, lines 4–30)

The type has three outcomes: key-by-key mapped type (both object), `TWith` (first arm, `TWith extends any ? TWith : never`), and `TWith` again in the non-object arm with a `: TType` else branch. Callers: `procedureBuilder.ts` (`_ctx_out`, `_input_in`, `_input_out`), `middleware.ts` (`_ctx_out`), and `ResolveOptions.ctx` in this file.

## Measurements and commands

Scratch files live under `clone-work/scratch/` (nothing added to the clone). Command shape, run from `packages/tests`:

    ./node_modules/.bin/tsc --noEmit --strict --skipLibCheck --target es2020 --moduleResolution node <scratch>.ts
    ./node_modules/.bin/tsc --noEmit -p tsconfig.json     # passes, no output

### Resolved types of the head `Overwrite`

| TType, TWith | Result |
| --- | --- |
| string, string | string |
| {a:1}, {b:2} | {a:1; b:2} |
| {a:1}, never | never |
| string, never | never |
| never, {a:1} | never |
| {a:1}\|{b:2}, {c:3} | {a:1;c:3} \| {b:2;c:3} (distributes) |
| {a:1}, string | string |
| unique symbol (UnsetMarker), {a:1} | {a:1} |
| {a:1}, {b:1}\|string | string \| {a:1;b:1} |
| any, {a:1} | {a:1} \| index-signature object |
| {a:1}, any | any |
| unknown, {a:1} | {a:1} |
| {a:1}, unknown | unknown |
| string, undefined | undefined |
| {a:1}, undefined | undefined |
| undefined, {b:1} | {b:1} |

`Overwrite<string, never>` and `Overwrite<{a:1}, never>` are `never`: the `: TType` arm on line 29 and the "unless TWith is never" sentence on line 9 do not describe the behaviour (finding 2).

### Equivalence of the proposed replacement

Proposed:

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

Checked with `Eq<Overwrite<A,B>, Proposed<A,B>>` (the standard `<T>() => T extends X ? 1 : 2` identity check) for 21 pairs: string/string, object/object, `{a:1}`/never, string/never, never/`{a:1}`, never/never, object-union/object, `{a:1}`/string, symbol/object, object/object-union, object/(object|string), any/object, object/any, unknown/object, object/unknown, array/object, string/undefined, object/undefined, (object|undefined)/object, undefined/object, and optional-key overwrite. All 21 compile as `true`; the compiler reported no error.

Why it is equivalent: `TWith extends any ? TWith : never` is the distributive identity on `TWith`; `TType extends any ? X : never` is always-true and distributes only to preserve `never`, which the leading `TType extends object ?` already does; the `: TType` arm is unreachable.

## Code-judo summary

Twenty lines and four conditionals become a two-level conditional plus the mapped type, and the readers' rule ("both objects: merge; otherwise the right side wins") is the structure of the code.

## Verification status

Confirmed by compilation against the repo's TypeScript. Pre-existing quirks noticed but not attributable to this PR: the mapped type is not homomorphic, so optional keys lose their `?` (`Overwrite<{a:1}, {a?:string}>` is `{a: string | undefined}`).
