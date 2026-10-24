# C1na: a Date or Map input followed by a middleware is rebuilt as an object type (the comment's own result, part 1)

Pull request: trpc/trpc #5017. Split from the comment's own result (C1n) because the comment names two different things: dates and maps, here, and functions, in [C1nb](C1nb.md). They fail differently and an earlier ruling covers only this one. The credit question is [C1](C1.md). Probes: [`../probes/C1na/`](../probes/C1na/), the probes of the regrade-v10 round run again unchanged.

**Already decided by regrade-v10 ruling 9** (`docs/research/regrade-v10-rulings-2026-10-17/rulings/09-trpc-date-map-set-input.md`, claim `CL-j-date-map-set-input`). The owner answered "Outside the review (Recommended)": "not a fault of this change, and not admitted as a pre-existing problem, since every valid use compiles." The claim it decided has the same trigger (a `Date`, `Map` or `Set` input followed by a middleware), the same mechanism (the rewritten checks answer yes, and the unchanged copy rebuilds the type from its members) and the same consequence (a long object type; every ordinary use compiles). The comment says no more than that: it names dates and maps among the values that "take the key-by-key branch", calls it a "mangled type" bug and names no use that fails.

## Problem

With a middleware after a whole input that is a `Date` or a `Map`, the input's type in the resolver and for callers is an object type listing the members, not `Date` or `Map`. The same before and after the change.

## What changed

```diff
-export type Overwrite<TType, TWith> = TType extends any
-  ? TWith extends any
-    ? {
+export type Overwrite<TType, TWith> = TType extends object
+  ? TWith extends object
+    ? // Both TType and TWith are objects: overwrite key-by-key
+      {
         [K in keyof TType | keyof TWith]: K extends keyof TWith
```

Head lines 11 and 12 are the rewritten checks. A `Date` and a `Map` pass both, as they passed the old `extends any`. The copy at head lines 14 to 20 is unchanged and rebuilds them (*read*).

Rule Before 2: "introduces it or makes it worse" and "makes it reachable" cannot apply, since the types are the same at both commits (*run*). "Leaves it ... in the lines the change touches" could apply only if the rewritten checks do the wrong thing for these inputs; the owner ruled they do not, since nothing fails. "In the thing the change says it fixes" cannot apply: the change names a string input only.

## Runs at base and at head

The same at both commits (*run*, TypeScript 5.1.3, Zod 3.20.2):

| Probe | Base | Head |
| --- | --- | --- |
| `m1-01-types`: the types printed | object types listing the members | the same |
| `m1-03-exact-type-map`: exact-type test for `Map` and `Set` | TS2554 on four lines | the same |
| `m1-05-uses-date`: 19 ordinary uses of a `Date` input | compiles | compiles |
| `m1-06-uses-map-set`: 25 ordinary uses of `Map` and `Set` inputs | compiles | compiles |
| `m1-11-readonly-size-write`: `input.size = 0` | accepted after the middleware (TS2578) | the same |
| `m1-12-documented-way-middleware-first` | compiles | compiles |

## Promised?

Not asked again here. The regrade-v10 dossier searched the six places (`docs/research/regrade-v10-rulings-2026-10-17/j-trpc-5017/dossiers/M1.md`) and the owner ruled the case outside the review, which needs no promise.

## Intended or announced

Nothing in the description, commits, review or issue #5020 mentions dates or maps (*read*, `../upstream/change-assembled.txt`).

## What the affected person sees

A long object type on hover and in error messages. Every ordinary use compiles and runs. For `Map` and `Set` an exact-type test fails and a write to `size` compiles and then throws (*run*, regrade-v10 round).

## What the maintainers did

Nothing by the cut-off (*read*). The same results hold at 10.43.4, 10.43.6 and 10.45.2 (*run* in the regrade-v10 round; later than the cut-off).

## How each fact is known

Types and uses: *run* at both commits ([`../probes/C1na/result-base.txt`](../probes/C1na/result-base.txt), [`../probes/C1na/result-head.txt`](../probes/C1na/result-head.txt)). The ruling and the diff: *read*.

## Relation to existing reference families and ruled claims

The ruled claim `CL-j-date-map-set-input`, regrade-v10 ruling 9, outside the review and not admitted. Not part of PE-j1: PE-j1's saved fix leaves the exact-identity check for `Map` and `Set` failing (regrade-v10 round, *run*), and no use fails here. Not part of GT-j7, which the owner kept to arrays and tuples. The control of this pull request is `known-problems`, so no clean control is touched.

## Both sides

*Outside the review:* the copy that rebuilds the type is unchanged, the types are the same at both commits, and every ordinary use compiles; the owner ruled exactly this.

*Against:* the rewritten checks send these inputs to the copy, which activation ruling 7 counted as a fault of the checks for arrays.

## Recommendation

> **What happens:** a `Date` or `Map` input after a middleware is shown as a long object type, and every ordinary use compiles, the same at both commits. **Promised:** not asked. **Delivered:** not asked. **So: outside-scope**, as regrade-v10 ruling 9 decided. **Band, decided separately:** none.

Confidence: high. One rule decides (Before 2, unchanged copy), the owner's ruling of the same shape went this way, and the fact that would reverse it, a valid use that fails to compile, is settled by the runs: none fails.
