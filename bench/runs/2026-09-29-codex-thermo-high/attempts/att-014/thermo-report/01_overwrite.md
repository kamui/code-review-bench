# Shared `Overwrite` type utility

## Scope and evidence

The production change is confined to `packages/server/src/core/internals/utils.ts:11-30`. The existing object-key mapping remains the central operation at lines 14-20. The added fallback code at lines 21-30 nests `TWith extends any` under the object/non-object branches and nests `TType extends any` and another `TWith extends any` in the non-object branch.

In the object `TType` / non-object `TWith` branch, `TWith extends any ? TWith : never` has no useful positive case beyond returning `TWith`; because the check is distributive, `never` already produces `never`. In the non-object `TType` branch, `TType extends any ? TWith extends any ? TWith : TType : never` likewise returns `TWith` for ordinary inputs, while distributivity already preserves `never` behavior. These checks make the implementation longer and obscure the actual rule described in the doc comment: merge keys for two objects, otherwise replace with `TWith`.

The same semantics can be expressed with two meaningful branches:

```ts
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

Both conditional checks are over naked type parameters, so unions remain distributive. If `TType` is `never`, the outer conditional remains `never`; if `TWith` is `never` while `TType` is an object, the inner conditional remains `never`. For non-object `TType`, returning `TWith` directly also preserves `never`. This removes three redundant conditional layers without introducing a helper or changing the merge mapping.

## Impact and remedy

This is a maintainability issue in a shared type-level utility, not a demonstrated runtime or type-correctness regression. The added conditionals make future edits to a widely used type harder to reason about and review. Simplify the branches as shown, then retain or add assertions for object/object key replacement, primitive replacement (including the string-input middleware regression), union distribution, and `never` propagation.

## Verification status

Ran from `packages/tests`:

```sh
./node_modules/.bin/tsc --noEmit -p tsconfig.json
```

The command passed with exit code 0. This validates the checked-in regression test and the repository's test TypeScript project. Runtime tests were not run under the supplied execution policy. The review checkout remained unchanged.
