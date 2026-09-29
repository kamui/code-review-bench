# Field control synchronization

## Scope and evidence

Reviewed the committed `main...review-head` changes in `packages/react/src/field/control/FieldControl.tsx` and its added tests. Compared the synchronization flow with `useValueChanged`, `useRegisterFieldControl`, `useFieldValidation`, and the neighboring Number Field and Select implementations. `useValueChanged` invokes its callback after a value transition; the callback has no event or cancellation parameter. In `FieldControl`, that callback clears errors, updates dirty and filled state, and invokes `validation.change`.

## Finding 1 — Controlled cancellation is ignored

**Evidence:** `FieldControl.tsx`, lines 137–155. The `onChange` handler creates `details`, calls `onValueChange`, then returns immediately when `isControlled`. The only `details.isCanceled` check is below that return, in the uncontrolled path. Controlled prop transitions are instead consumed by `useValueChanged` at lines 105–115, which unconditionally performs the internal synchronization for every defined next value.

A controlled callback can call its setter and then `details.cancel()`. The setter schedules the new controlled prop, but cancellation is not represented in that prop. On the next render, `useValueChanged` therefore treats the prop echo as an ordinary accepted transition and clears form errors, updates dirty and filled state, and validates it. This makes cancellation work for uncontrolled fields but not for controlled fields, despite the change’s stated fix that `details.cancel()` stops internal handling.

**Verification status:** Confirmed by control-flow inspection. No test was run and no scratch regression test was created.

**Worked code-judo proposal:** Keep the two ownership paths, but make cancellation part of the controlled transition handoff. Record the canceled proposed value in a ref in the event handler when `details.isCanceled` is true. In the controlled synchronization callback, consume that marker for the matching prop echo and return before clearing errors or touching field state. Clear/replace the marker when a later uncanceled transition arrives so a canceled value cannot accidentally suppress a future independent programmatic update. Add a regression test that changes the controlled state in `onValueChange`, calls `details.cancel()`, and verifies the existing form error remains and validation is not called. The handoff should be scoped to the proposed value and consumed once; do not globally suppress the next arbitrary controlled update.

## Structural review

The new code remains in the control that owns registration, input events, and field-state synchronization. `useValueChanged` is already the canonical sibling mechanism, and serializing the controlled value establishes a coherent DOM-string comparison boundary. The extra controlled/uncontrolled branch corresponds to distinct ownership semantics and is not itself needless branching. The remaining mount-time DOM read is narrowly required for uncontrolled prefilled inputs. No file crosses a size boundary, and no separate high-confidence simplification would remove meaningful complexity without changing the behavior contract.

## Suggested remediation order

1. Add a one-transition cancellation handoff between the controlled event and prop synchronization.
2. Cover cancellation when the controlled callback both updates its value and cancels.
3. Run `TZ=UTC VITEST_ENV=jsdom ./node_modules/.bin/vitest run --project @base-ui/react packages/react/src/field/control/FieldControl.test.tsx` from the clone root.
