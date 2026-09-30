# 01 — GraphQLError constructor contract (`src/error/GraphQLError.js`)

Scope: the single production-code change in PR #1582, which widens the `nodes` parameter of the `declare class GraphQLError` constructor from `$ReadOnlyArray<ASTNode> | ASTNode | void` to `$ReadOnlyArray<ASTNode> | ASTNode | void | null`.

## How the file is structured

`GraphQLError.js` (220 lines at head, unchanged in size by the PR) describes the constructor twice. Lines 22–91 hold a `declare class GraphQLError extends Error` whose constructor signature is what Flow sees at every `new GraphQLError(...)` call site. Lines 93–209 hold the real `export function GraphQLError(...)` implementation, whose own parameter list is written out a second time by hand. Flow does not check one against the other: the declaration is a type-only overlay and the function is a separate binding that the declaration shadows for type purposes (`// eslint-disable-line no-redeclare`).

The two lists were already out of sync at the merge-base. The implementation's `originalError` is `?Error & { +extensions: mixed }` (line 98), while the declaration says `?Error` (line 29).

## Finding 1.1 — The widening only reaches one of the two hand-maintained signatures, so the contracts drift further apart

**Evidence.** After the PR, line 25 (declaration) reads:

```js
    nodes?: $ReadOnlyArray<ASTNode> | ASTNode | void | null,
```

and line 94 (implementation) still reads:

```js
  nodes?: $ReadOnlyArray<ASTNode> | ASTNode | void,
```

The author's own review comment on line 25 gives the rationale: "All other arguments support `null` so there is no reason why `nodes` accepts `undefined` but not the `null`." That reasoning applies to the implementation just as much as to the declaration. At runtime the implementation already treats `null` correctly, because the blame-node computation on lines 101–107 is a truthiness check (`nodes ? [nodes] : undefined`). So the implementation's annotation is now narrower than both its behavior and its public declaration.

**Why it matters.** The public type is correct now, but the file has two sources of truth for one constructor contract, and this PR adds a second divergence to the one that already existed. The next person to edit the implementation signature has no way to tell which list is authoritative. The fix for "nodes should accept null" also went into only one of the two places that describe `nodes`. That is exactly the drift that parallel hand-written signatures invite.

**Secondary point: idiom.** Every other nullable parameter on the neighboring lines uses Flow's maybe-type shorthand: `source?: ?Source`, `positions?: ?$ReadOnlyArray<number>`, `path?: ?$ReadOnlyArray<...>`, `originalError?: ?Error`, `extensions?: ?{...}`. The new line spells the same thing out as `| void | null`, which reads as if `nodes` were special when the stated intent is to make it uniform with its siblings.

**Remedy (code judo).** Give the parameter one name and one definition, write it in the same maybe-type idiom as its siblings, and use it in both places:

```js
type BlameNodes = $ReadOnlyArray<ASTNode> | ASTNode;

declare class GraphQLError extends Error {
  constructor(
    message: string,
    nodes?: ?BlameNodes,
    source?: ?Source,
    ...
  ): void;
  ...
}

export function GraphQLError( // eslint-disable-line no-redeclare
  message: string,
  nodes?: ?BlameNodes,
  source?: ?Source,
  ...
```

At minimum, change line 94 to match line 25 and use `?(...)` on both. While touching this, the pre-existing `originalError` mismatch (line 29 vs. line 98) should be settled the same way, so the declaration and implementation agree parameter for parameter.

**Verification status.** Confirmed by reading the file at `review-head`: `sed -n 1,140p src/error/GraphQLError.js`, plus `grep -n "nodes\|originalError"` over the declaration and function headers. I confirmed runtime null-tolerance of `nodes` from the implementation on lines 101–107 and from the existing tests that pass `null` (e.g. `new GraphQLError('msg', null, source, [6])`). All 27 tests across the four changed test files pass at head under mocha. I did not run Flow itself, because it is outside this review's execution allowance. The claim that Flow does not reconcile the two signatures rests on how `declare class` + same-named `function` shadowing works, and on the fact that the pre-existing `originalError` mismatch was already tolerated at the merge-base.

## Non-findings in this file

The file stays at 220 lines with no new branching. The one-line change is directionally right: `null` is accepted everywhere else, and internal callers already pass it.
