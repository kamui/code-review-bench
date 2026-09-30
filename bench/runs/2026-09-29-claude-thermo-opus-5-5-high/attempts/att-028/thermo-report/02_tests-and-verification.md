# 02 — Tests and verification

Scope: `packages/react/src/field/control/FieldControl.test.tsx` (+149 / −6), plus the commands
used to verify the findings in `01_field-control-sync.md`.

## Finding 2.1 — The new render-count test's name and comment overstate what it guarantees

**Where:** `FieldControl.test.tsx`, test `renders once per keystroke for controlled input changes`.

**Evidence.** The test mounts a controlled control under a bare `<Field.Root>`, so it uses the
default `onSubmit` validation mode. It then asserts `renderCountRef.current === settled + 2` after
two keystrokes, with the comment "The controlled echo must not schedule a second render per
keystroke." The PR body's own table says that in `onChange` mode the head renders **twice** per
keystroke. A scratch probe confirmed that (`onChange` mode, controlled, three keystrokes: head
deltas `2 2 2`, base `1 1 1`).

**Why it matters.** Someone reading the suite would conclude that controlled `Field.Control`
renders once per keystroke. That is true only for one validation mode, and the mode that matters
most for per-keystroke cost (`onChange`) is the one that renders twice. A test named as an
invariant that holds only by configuration is a legibility trap, and it will not catch a
regression that adds a third render in `onChange` mode.

**Remedy.** Rename it to `renders once per keystroke for controlled input changes in onSubmit
mode` and narrow the comment to match. Better, parameterize it over `validationMode` with the
expected per-keystroke counts (`onSubmit: 1`, `onChange: 2`). That way the cost the PR body
accepts is pinned instead of just described.

**Verification status:** confirmed by execution.

## Finding 2.2 — Two behaviors the PR claims (or changes) have no test

**Evidence.**

- The PR body says a programmatic change used to let "a pending async validator for the old value
  … still publish." None of the new tests uses an async `validate`. The programmatic-change test
  only checks `filled`, `dirty` and a synchronous `validate` call.
- The controlled-mode behavior for a prevented input changed (Finding 1.2) and so did the
  submit-time `validate` argument type (Finding 1.3). The suite pins neither. The existing
  prevented-input test is uncontrolled only.

**Remedy.** Add three focused tests. The first starts an async validator on value A, then changes
the controlled value to B programmatically and asserts that A's result never publishes. The
second is a controlled variant of `does not clear errors or validate when change is prevented`
that asserts whichever semantics are chosen for Finding 1.2. The third submits a form with a
numeric controlled value and asserts the `validate` argument.

**Verification status:** confirmed by reading the diff.

## Commands run

The existing suite at head passes:

```
TZ=UTC VITEST_ENV=jsdom ./node_modules/.bin/vitest run --project @base-ui/react \
  packages/react/src/field/control/FieldControl.test.tsx
# Test Files 1 passed; Tests 26 passed | 1 skipped (27)
```

Scratch probes ran outside the clone. `clone-work/scratch` aliases `@base-ui/react/*` and
`@base-ui/utils/*` to the clone's sources at head. `clone-work/scratch-base` aliases them to a
`git archive main packages/react/src packages/utils/src` snapshot. Both link the clone's
`packages/react/node_modules`. Each was run with:

```
TZ=UTC <clone>/node_modules/.bin/vitest run --root <dir> --silent=false --reporter=verbose
```

Probe output (`probe.test.tsx`, identical in both directories):

| Probe | base | head |
| --- | --- | --- |
| controlled + capture-phase `preventDefault` on `input`: `validate` calls | 0 | 1 |
| controlled, `onChange` mode, render deltas for 3 keystrokes | 1 1 1 | 2 2 2 |
| controlled `'x' → null`: `data-filled` / DOM value | set / `x` | set / `x` |
| then `null → ''`: `data-filled` | absent | absent |
| form submit with controlled `value={5}`: `validate` first arg | `number 5` | `string "5"` |
| controlled `5 → '5'` programmatically: `validate` calls | 0 | 0 |

Nothing was written inside the clone.
