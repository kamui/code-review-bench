# Argument comparison subsystem

## Scope and identity

Reviewed source: `src/validation/rules/OverlappingFieldsCanBeMergedRule.ts`.

Base: `730d5af8e933235fd5aa312a00be465db0b8acf5`.
Head: `efdbbcfacd92adb78e002f2b8a61f2a6a1504c19`.

The checkout's `main` and `review-head` resolve to those revisions, respectively, and HEAD resolves to the pinned head. The worktree was clean before review. This review reads the committed range, leaves the source unchanged, and uses a single review context.

The task-selected frozen skill was read from `clone-work/frozen-skill/SKILL.md`. It calls for ambitious structural simplification, evidence-backed maintainability findings, a layered report, and worked proposals. It does not require child reviewers or referenced resource files.

## Measurements and commands

The committed change has 15 insertions and 27 deletions. The file has 838 lines at base and 826 at head. The comparison implementation removes `sameArguments` and `stringifyValue`, adds `stringifyArguments`, and replaces an explicit argument-array equality check with equality of two canonical strings.

The following read-only commands established the range, size, and source anchors:

```sh
git rev-parse HEAD main review-head
git diff main...review-head
git diff --stat main...review-head
git diff --numstat main...review-head
git show main:src/validation/rules/OverlappingFieldsCanBeMergedRule.ts | wc -l
wc -l src/validation/rules/OverlappingFieldsCanBeMergedRule.ts
nl -ba src/validation/rules/OverlappingFieldsCanBeMergedRule.ts
git diff --check main...review-head
git status --porcelain=v1
```

Source searches examined the callers of `sortValueNode`, its implementation and tests, `naturalCompare` and its tests, the language printer and AST types, the overlapping-field tests, and the unique argument/input-field validation rules. Repository test configuration was inspected to run the installed Mocha loader. No ambient review guidance was loaded.

## Intended simplification and invariants

At base, `sameArguments` first compares lengths, then finds each argument by its exact name, then compares each value through `print(sortValueNode(value))`. Matching names takes quadratic work in the number of arguments in the worst case; value normalization already ignores nested input-object field order.

At head, the argument nodes become object-field nodes in a synthetic `ObjectValueNode`. Normalizing that value sorts the argument names and recursively sorts object values, after which printing yields a comparable string. This removes the name-matching loop and consolidates normalization.

For unique names, that model is direct and strongly typed. The construction needs no cast because both node types carry a `NameNode` and a `ValueNode`. It also keeps normalization local to the rule and reuses the existing utility. The `arguments ?? []` fallback is appropriate because `FieldNode.arguments` remains optional in `src/language/ast.ts:360`.

The new outer canonicalization requires sorting to impose a reliable order on every distinct argument name. It also moves wrapper allocation and printer traversal into every pair comparison, including comparisons where there are no arguments to normalize. Those are the two failed properties.

## Finding 1: Canonicalization requires a total name order

The changed anchor is `src/validation/rules/OverlappingFieldsCanBeMergedRule.ts:649`. That line delegates to `sortValueNode`, whose `sortFields` uses `naturalCompare` at `src/utilities/sortValueNode.ts:44–46`.

The dependency implements digit runs by accumulating a JavaScript number at `src/jsutils/naturalCompare.ts:16–29`. Distinct sufficiently large digit runs can round to the same number. If the remaining text and lengths also coincide, the comparator returns zero for unequal names. Both names below satisfy GraphQL's name syntax:

```text
arg9007199254740992
arg9007199254740993
```

The scratch reproduction builds this schema and parses this query:

```graphql
type Query {
  f(arg9007199254740992: Int, arg9007199254740993: Int): Int
}
```

```graphql
{
  f(arg9007199254740992: 1, arg9007199254740993: 2)
  f(arg9007199254740993: 2, arg9007199254740992: 1)
}
```

The comparator result is exactly zero. Stable sorting consequently retains the input order of these distinct names. The two strings remain different despite representing the same argument set.

Base validation with the overlap rule returns no errors. Head validation returns:

> Fields "f" conflict because they have differing arguments. Use different aliases on the fields to fetch both if this was intentional.

Head validation with the default rule set returns precisely the same single error. This is a valid-query rejection, not an interaction with unknown arguments, duplicate argument names, or variable validation.

The comparator weakness exists at base, and already affects nested object-field ordering there. The new finding is the extension of that weakness to top-level argument equality: the base's exact-name lookup handles this reproduction correctly. The review is anchored to the changed delegation, not to a pre-existing utility line.

### Worked code-judo proposal

Keep one canonicalization path and make its name ordering distinguish unequal names. A minimal change to the existing sort callback preserves natural order for all non-ties:

```ts
.sort((fieldA, fieldB) => {
  const nameA = fieldA.name.value;
  const nameB = fieldB.name.value;
  return (
    naturalCompare(nameA, nameB) ||
    (nameA < nameB ? -1 : nameA > nameB ? 1 : 0)
  );
});
```

This fixes the boundary where the ordering guarantee belongs, and avoids adding a separate argument-order algorithm to the validation rule. The scratch variant replaced only this callback and injected that utility into an in-memory copy of the head rule. It accepts the reproduction and agrees with head on all 324 ordinary argument-shape comparisons.

A direct lexical comparator is another possible simplification because canonical equality only requires a deterministic order, not natural numeric presentation. The minimally verified proposal is the tie-breaker above. Replacing natural ordering wholesale was not tested.

The shared utility is also called by `src/utilities/findBreakingChanges.ts:538` when stringifying default values. A production fix should exercise that consumer's tests as well as sorter and overlap tests. The scratch check establishes the reported repair, not compatibility of every consumer. No production source was modified.

## Finding 2: Normalizing empty arguments multiplies work

The changed helper at `src/validation/rules/OverlappingFieldsCanBeMergedRule.ts:637–649` always maps argument nodes, creates an object node, sorts a copy, and calls the printer. For empty arguments this entire pipeline merely computes `{}`.

The pair traversal at lines 486–501 compares each occurrence of a response name with every other occurrence. The call at line 591 invokes `stringifyArguments` for both fields. For n repetitions of one argument-free field, there are n(n−1)/2 comparisons and n(n−1) synthetic-object serializations. The pair traversal predates this change; the costly empty normalization does not.

The base's empty-array path checks equal lengths and calls `every` on an empty array, returning immediately without printing an AST. That cheap path is lost in the refactor.

### Timing evidence

The scratch measurement uses `type Query { f: Int }` and the valid query `{ f f ... }`, with 100, 300, and 600 occurrences of `f`. Documents and schema are built before measurement. Every measured validation call returns zero errors.

For each document size, there are three warm-up runs per variant and nine measured runs per variant. Base and head measurement order alternates. The table reports medians from the captured final run:

| Repeated fields | Field pairs | Base ms | Head ms | Empty-case proposal ms | Head/base |
| --- | ---: | ---: | ---: | ---: | ---: |
| 100 | 4,950 | 1.354 | 27.726 | 1.342 | 20.48× |
| 300 | 44,850 | 8.719 | 253.504 | 8.438 | 29.07× |
| 600 | 179,700 | 30.290 | 1,091.240 | 30.107 | 36.03× |

An earlier independent run measured approximately 21.28×, 33.43×, and 37.32×, respectively. The exact ratio varies, but both measurements show the same large regression.

These numbers isolate the rule on one repeated-field workload. They are not production latency estimates, a benchmark of unrelated rules, or a claim that every GraphQL document becomes this much slower. The structural explanation and the restored baseline in the scratch variant make this more than a timing-only suspicion.

### Worked code-judo proposal

Use the identity of the empty argument set before constructing an AST:

```ts
function stringifyArguments(fieldNode: FieldNode): string {
  const args = fieldNode.arguments ?? [];
  if (args.length === 0) {
    return '{}';
  }

  const inputObjectWithArgs: ObjectValueNode = {
    kind: Kind.OBJECT,
    fields: args.map((argNode) => ({
      kind: Kind.OBJECT_FIELD,
      name: argNode.name,
      value: argNode.value,
    })),
  };
  return print(sortValueNode(inputObjectWithArgs));
}
```

Retain the existing coverage comment around the optional-arguments fallback in the real implementation. The snippet omits that annotation only for readability.

The string is the exact representation already produced by printing an empty object. Returning it removes allocation, sorting, and traversal rather than moving those operations elsewhere. This identity case belongs in the normalization helper, so it does not introduce unrelated branches into conflict traversal.

The scratch variant inserted only the empty-case return into an in-memory head module. It agrees with head on the 324 comparison cases, handles omitted `arguments`, and restores baseline timing in the measured argument-free workload.

For repeated nonempty argument sets, normalization still repeats per field pair, as value normalization already did at base. A larger restructuring could cache signatures with field metadata in the validation invocation, but it would require changing the tuple model or threading additional state through recursive conflict helpers. That is not justified by this small diff or the evidence collected. A global cache would also introduce cross-invocation assumptions about externally supplied ASTs. The small empty identity case is the proportionate remedy.

## Other reviewed concerns

The new helper reduces the number of local functions and removes the bespoke name search. The file stays below 1,000 lines and shrinks; extracting its traversal algorithm merely to satisfy a size concern is not warranted.

The synthetic value stays internal and never becomes part of the user's document. The sorter returns copied object/list containers, and the characterization test confirms validation does not mutate the parsed AST. Names and scalar values are safely reused. There is no new public abstraction, `any`, `unknown`, or cast in this change.

The missing-argument fallback preserves the old behavior for hand-built ASTs. The scratch test deletes `arguments` from both field nodes and confirms that base, head, and both remedies accept the result.

Duplicate names can change the overlap rule's standalone diagnostics because the old algorithm searches the first name match while the new representation retains all entries. Such documents already violate `UniqueArgumentNamesRule`. This was considered and not promoted to an additional actionable finding. The reported name-order reproduction uses unique, declared names.

List order, variable spelling, literal kinds, nested object sorting, and string printing are still delegated to the same value normalizer and printer. The 324-case matrix found no additional base/head differences among its selected ordinary names and values. It includes equivalent and nonequivalent field pairs. Some cases deliberately omit declarations irrelevant to rule-isolated comparisons; only the dedicated reproduction is a default-rule valid-document check.

No asynchronous work, externally visible update, or transaction boundary is introduced. The skill's parallel-orchestration and atomic-update concerns do not apply.

## Verification status and reproducibility

Existing focused suite:

```sh
BABEL_DISABLE_CACHE=1 ./node_modules/.bin/mocha \
  src/validation/__tests__/OverlappingFieldsCanBeMergedRule-test.ts \
  src/utilities/__tests__/sortValueNode-test.ts
```

Result: 48 passing. The command uses the repository's `.mocharc.yml` and `resources/ts-register.js`, with Babel caching disabled to avoid writing cache artifacts into the checkout.

Scratch characterization and proposal checks:

```sh
BABEL_DISABLE_CACHE=1 ./node_modules/.bin/mocha --extension cjs \
  /home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high-selected/att-016/clone-work/review-probes.cjs \
  --timeout 120000
```

Result: four passing, in approximately 18 seconds. This is a characterization suite: its name-order test asserts the observed erroneous head result and the repaired result. Passing it does not mean the head behavior is correct.

The base module comes from `git show main:src/validation/rules/OverlappingFieldsCanBeMergedRule.ts` and is transpiled in memory with the installed Babel configuration. Since the PR changes only that file, its imported dependencies are identical at the two pinned revisions. The scratch variants likewise transpile modified source strings in memory; nothing is written to the checkout.

The timing test asserts correctness on every measured call. The comparator remedy and empty-case remedy are tested as separate variants. A combined patch, full TypeScript check, complete test suite, and shared-utility consumer tests were not run because there is no applied production patch. Those remain implementation validation steps, not missing evidence for the two findings.

The installed tooling emitted an outdated Browserslist-data notice and the existing suite emitted a Node module-type warning. Neither caused test failures; no dependency update was attempted. All focused commands completed within five minutes and used no network.

Raw scratch evidence is preserved in [../review-probes.cjs](../review-probes.cjs) and [../review-probes.log](../review-probes.log). The final source identity and clean-tree checks are recorded in the adjacent verification artifact.
