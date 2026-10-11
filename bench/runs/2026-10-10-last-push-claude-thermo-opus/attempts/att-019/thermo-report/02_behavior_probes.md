# 02 — Tests, measurements, and verification status

Scope: `packages/react/src/field/control/FieldControl.test.tsx` (+149 / −6) and the behavioral evidence behind the findings in `01_field_control_structure.md`.

## 1. How the evidence was produced

Nothing was written to the clone; `git status --short` was empty and `HEAD` was `14d39e5d1ad6b7aca2fb067415dba09c6bea219b` after the last run.

**The PR's own suite, on head.**

```
TZ=UTC VITEST_ENV=jsdom ./node_modules/.bin/vitest run --project @base-ui/react \
  packages/react/src/field/control/FieldControl.test.tsx
```

Result: 1 file, 26 passed, 1 skipped (the `skipIf(isJSDOM)` SSR autofocus test). The chromium mode was not available, so nothing here is confirmed in a real browser.

**Scratch probes.** A scratch vitest root in the work directory (jsdom, `@testing-library/react`, aliases for `@base-ui/react/<name>` and `@base-ui/utils/<name>`) was pointed at one of three source trees through a `VARIANT` environment variable:

- `head` — the clone as checked out.
- `base` — `git archive main packages/react/src packages/utils/src` extracted into the work directory, with `node_modules` symlinked to the clone's.
- `judo` — `git archive review-head …` extracted the same way, with the diff from `01_field_control_structure.md` section 7 applied to `FieldControl.tsx`.

```
VARIANT=<head|base|judo> TZ=UTC <clone>/node_modules/.bin/vitest run --root <work>/scratch --reporter=verbose
```

Each probe renders a small tree, performs one interaction, and logs observed DOM attributes and mock call counts. Probes record what happens; they assert nothing, so "passed" for a probe file only means it ran.

**Port of the PR tests.** Eleven tests from `FieldControl.test.tsx` (the seven new ones, the modified render-count test, and three existing change-path tests) were rewritten against `@testing-library/react` so they could run against all three trees. On `head` 11 pass. On `judo` 11 pass. On `base` 8 pass and 3 fail — the numeric dirty test, the programmatic sync test, and the cancel test — which is the expected set and shows the port discriminates.

## 2. Probe results

| Probe | Scenario | base | head |
| --- | --- | --- | --- |
| P1 | Controlled, `validationMode="onChange"`, renders of the control for two steady-state keystrokes | 2 | 4 |
| P2 | Controlled, `onValueChange` calls `cancel()` then `setValue` | `validate` ×1, dirty | `validate` ×1, dirty |
| P3 | Uncontrolled, `onValueChange` calls `cancel()`, inside a `Form` with a server error | DOM `"a"`, dirty, filled, `validate` ×1, error replaced | DOM `"a"`, dirty, filled, `validate` ×0, server error still shown |
| P4 | Controlled, `Field.Root disabled`, value set programmatically, then enabled | while disabled: not dirty, `validate` ×0; after enable: not dirty, no error | while disabled: dirty, `validate` ×1; after enable: dirty, invalid, error shown; edit and return to loaded value: not dirty |
| P5 | Controlled, value set programmatically after mount, `validationMode="onChange"` | not dirty, no error | dirty, invalid, error shown |
| P6 | `value={null}`, user types `"a"` | dirty, filled, `validate` ×1 | not dirty, not filled, `validate` ×0 |
| P7 | Controlled, native `input` event prevented in capture, owner still calls `setValue` | `validate` ×0 | `validate` ×1 |
| P8 | Controlled with `value="abc"`, owner ignores the change | dirty, `validate` ×1, server error cleared | not dirty, `validate` ×0, server error kept |
| P9 | `value={5}`: type of `initialValue` seen by `Field.Validity`, type passed to `validate` from `actionsRef.validate()` | `number`, `number` | `string`, `string` |
| P10 | Controlled `value="foo"` rendered into a `<div>` | filled on mount | not filled on mount |
| P11 | Controlled control holding `"abc"` swapped (by `key`) for one holding `""` in the same `Field.Root` | filled clears | filled stays set |
| P12 | `Switch` in a disabled `Field.Root`, toggled programmatically, then enabled | while disabled: dirty, `validate` ×1; after enable: dirty, invalid | identical |
| P13 | Focus the control, then disable `Field.Root` | `data-focused` stays | `data-focused` stays |
| P14 | Focus the control, then unmount it | `data-focused` stays | `data-focused` stays |

With the proposal applied (`judo`): P3 becomes not dirty, not filled, `validate` ×0, server error kept (all four updates skipped together); P6 returns to the base result; P11 returns to the base result; P1, P4, P5, P7, P8, P9 and P10 are the same as head.

### Notes on individual probes

**P1.** The second render is `validation.change` → `commit` → `setValidityData(…)` running inside the layout effect that `useValueChanged` schedules, after the commit that delivered the new `value`. On base the same `setValidityData` ran inside the change handler and batched with the owner's `setValue`. `commit` builds a fresh `validityData` object on every validating pass, so it never bails out, and `validityData` is part of `Field.Root`'s context value, so every context consumer re-renders with it. In the default `onSubmit` mode before a submit attempt, `commit(value, true)` returns at `state.valid !== false` without setting state, which is why the PR's test sees one render.

**P4 and P12.** P12 is the control for P4: it shows the disabled behavior is the sibling pattern's, present on base, and not specific to this PR's code. The inference that the baseline is `"loaded"` after enabling comes from the third P4 observation (returning to `"loaded"` clears dirty) and from reading `captureInitialValue` in `packages/react/src/internals/field-register-control/useFieldControlRegistration.ts`.

**P7.** jsdom lets a synthetic `input` event be cancelable; real browsers do not make `input` cancelable, so the practical reach of this guard is whatever React issue the base comment refers to. I could not consult that issue (no network) and report only that the guard's coverage changed.

**P13 and P14.** `onFocus` and `onBlur` (head lines 158–168) are byte-identical to base and nothing else in the diff touches `setFocused`. The probes confirm the observable behavior is unchanged.

## 3. Review of the test changes

**`renders once per keystroke for controlled input changes` (lines 47–80).** The comment on the assertion says "The controlled echo must not schedule a second render per keystroke." The test mounts `Field.Root` with the default validation mode and no `validate`, the one configuration where the prop sync does no state work. It passes on base as well, so it does not detect the change this PR makes. In `validationMode="onChange"` — the configuration where a per-keystroke cost actually lands on users — the count is two per keystroke (P1). The test name states a general property that the code does not have.

**`does not validate when the change is canceled` (lines 189–201).** It asserts only `expect(validate).not.toHaveBeenCalled()`. It does not observe dirty, filled, or form errors, which is how the half-applied state in P3 passes, and there is no controlled counterpart, which is how P2 passes.

**`avoids rerendering for uncontrolled input changes` (lines 19–45, modified).** Moving the counter from a wrapper component into the `render` prop is a real improvement: the old wrapper element was created once by the test and would never re-render regardless of what `Field.Control` did, so the old assertion could not fail.

**Not covered by any test in the diff.** The PR body names array values ("Array values failed the same way by reference"), a clear button / form-library reset clearing a resolved error, and a pending async validator for the old value being superseded. None has a test. `clearErrors` on a programmatic change — the behavior the body's open question is about, and which head implements at line 110 — has no test either; the existing `Form`-based test at line 203 covers only the uncontrolled prevented path.

## 4. Verification status by finding

| Finding (summary numbering) | Status | Evidence |
| --- | --- | --- |
| 1. Duplicated sync sequence with divergent gates | Verified by reading head lines 105–115 and 137–157; proposal verified against 11 ported tests | section 3 of `01`, `judo` run |
| 2. Cancel half-applied / ignored when controlled | Verified by execution | P2, P3 |
| 3. Two predicates for "controlled"; `value={null}` dead | Verified by execution, regression against base | P6 |
| 4. Second commit per keystroke in `onChange` mode; test over-claims | Verified by execution, regression against base | P1; ported render-count test passes on base |
| 5. Mount-only `filled` effect can only raise the flag | Verified by execution, regression against base | P11 (P10 secondary) |
| 6. Disabled / uncaptured baseline | Verified by execution on head; shown to be inherited from siblings | P4, P12 |
| 7. PR body describes behavior the head does not contain | Verified by diff inspection and execution | P13, P14; `git diff main...review-head` |
| Question A (hydration marks dirty and validates) | Observed; intent unknown | P5 |
| Question B (`initialValue` / `validate` argument type) | Observed; intent unknown | P9 |

## 5. Limits

No browser run, so focus behavior (P13/P14) and anything depending on real `input` event semantics (P7) are jsdom observations only. The proposal was verified against a port of the tests, not against the repository's `createRenderer` / `describeConformance` harness. The bundle-size and benchmark comment in the packet refers to commit `6a40cf1`, which is not the head under review, and its Menu/Dialog/Tooltip deltas are in components this diff does not touch; I drew no conclusion from it. This was a single-reviewer pass on `claude-opus-5-5`; no child reviewers were started and no cross-model or alternate-model review was run.
