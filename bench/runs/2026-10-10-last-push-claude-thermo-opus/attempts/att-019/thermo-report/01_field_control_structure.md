# 01 — `FieldControl.tsx`: structure, gates, and the code-judo proposal

Scope: `packages/react/src/field/control/FieldControl.tsx` at head `14d39e5d1` against merge-base `30b8ea200` (+48 / −20). Line numbers below are head line numbers unless marked "base".

Behavioral evidence for every claim here is in `02_behavior_probes.md`; probe ids (P1…P14) refer to that file.

## 1. What the change does, in one paragraph

Before the PR, `Field.Control` updated field state (dirty, filled, form errors, validity) only from the DOM `change` event. The PR adds a second path, `useValueChanged(serializedValue, …)` at lines 105–115, that runs the same four updates when the controlled `value` prop changes, and makes the DOM path return early when the control is controlled (lines 142–146). It also serializes the controlled value to a string before registering it (line 86, lines 90–97) so the dirty baseline and the comparison have the same type, replaces the `[valueProp]`-dependent filled effect with a mount-only one (lines 99–103), and adds `details.isCanceled` to the existing `defaultPrevented` guard (line 153).

The direction is right. Syncing from the prop is how the nine sibling controls work, and "a value the consumer rejects or rewrites never reaches the field state" is a real improvement (P8: on base, typing into a controlled input whose owner rejects the change marked the field dirty, ran `validate` with the rejected text, and cleared the server error; on head none of that happens).

The implementation, however, leaves the component with more concepts than it needs and with several gates that disagree with each other.

## 2. Size and decomposition

`FieldControl.tsx` is 207 lines and `FieldControl.test.tsx` is 273 lines at head (`wc -l`). Nothing is near the 1000-line boundary and no decomposition is needed. The file-size rule does not apply to this PR.

## 3. The sync sequence now exists twice, with three different gate sets

The "field value changed" sequence is four calls: `clearErrors(name)`, `setDirty(v !== (validityData.initialValue ?? ''))`, `setFilled(v !== '')`, `validation.change(v)`.

Head carries it twice:

```tsx
// lines 105-115 — controlled path
useValueChanged(serializedValue, () => {
  if (serializedValue === undefined) {
    return;
  }

  clearErrors(name);
  setDirty(serializedValue !== (validityData.initialValue ?? ''));
  setFilled(serializedValue !== '');

  validation.change(serializedValue);
});
```

```tsx
// lines 137-157 — uncontrolled path
onChange(event) {
  const inputValue = event.currentTarget.value;
  const details = createChangeEventDetails(REASONS.none, event.nativeEvent);
  onValueChange?.(inputValue, details);

  // Controlled values sync from the `value` prop instead, so that a value the consumer
  // rejects or rewrites never reaches the field state.
  if (isControlled) {
    return;
  }

  // `validation.change` reads `markedDirtyRef`, so update dirty before validating.
  setDirty(inputValue !== (validityData.initialValue ?? ''));
  setFilled(inputValue !== '');

  // Workaround for https://github.com/react/react/issues/9023
  if (!event.nativeEvent.defaultPrevented && !details.isCanceled) {
    clearErrors(name);
    validation.change(inputValue);
  }
},
```

The two copies differ in ways a reader has to diff by eye:

| | controlled copy | uncontrolled copy |
| --- | --- | --- |
| call order | clearErrors → setDirty → setFilled → change | setDirty → setFilled → clearErrors → change |
| ordering invariant comment ("update dirty before validating") | absent | present |
| gate on `setDirty` / `setFilled` | `serializedValue !== undefined` | none |
| gate on `clearErrors` / `validation.change` | `serializedValue !== undefined` | `!defaultPrevented && !isCanceled` |

So four state updates are governed by three signals (`isControlled`, `defaultPrevented`, `isCanceled`) plus the `undefined` check, and each signal covers a different subset. The ordering invariant that the base code documented (`validation.change` reads `markedDirtyRef`, so dirty must be set first) is still load-bearing in the new copy but the comment did not travel with it.

Consequences that are verified, not hypothetical:

- **Cancel is half-applied when uncontrolled (P3).** `details.cancel()` skips `clearErrors` and `validation.change` but `setDirty` and `setFilled` still run. After one canceled keystroke in an uncontrolled control: DOM value `"a"`, `data-dirty` set, `data-filled` set, `validate` not called, the stale form error still displayed. The field is dirty and filled according to a value that its validity state has never seen.
- **Cancel is ignored when controlled (P2).** The early return at line 144 precedes the `isCanceled` read, and the prop path has no notion of cancel. `onValueChange={(v, d) => { d.cancel(); setValue(v); }}` validates and marks dirty exactly as if `cancel()` had not been called. Same on base, but the PR body presents cancel as now working ("`details.cancel()` in `onValueChange` now stops the internal handling").
- **The React #9023 workaround no longer covers controlled controls (P7).** With a prevented native `input` event on a controlled control whose owner still updates state, base skipped validation; head validates once. This may be acceptable — the owner chose to accept the value — but it is an unstated change to a guard the PR otherwise preserved.

`isCanceled` was appended to a condition whose comment says it is a workaround for a React bug. Those are unrelated concerns sharing one `if`, and the comment now mis-describes half of the condition.

## 4. Two definitions of "controlled"

```tsx
const isControlled = valueProp !== undefined;                                // line 83
const value = isControlled ? valueUnwrapped : undefined;                     // line 84
const serializedValue = value == null ? undefined : String(value);           // line 86
```

Line 83 says controlled means "not `undefined`". Line 86 says there is a controlled value only when it is "not nullish". The DOM path trusts the first (line 144: `if (isControlled) return;`), the prop path trusts the second (line 106: `if (serializedValue === undefined) return;`). For `value={null}` both paths decline, and the field stops tracking anything.

P6 confirms it: with `value={null}`, React itself treats the input as uncontrolled, so typing changes the DOM to `"a"`; on head the field stays not-dirty, not-filled, and `validate` is never called. On base the same interaction marked it dirty and filled and validated once. `null` is outside the declared prop type (`React.ComponentProps<'input'>['value']`), but it reaches components routinely from untyped callers and from form-library adapters that hold `null` for empty database columns, and the previous code tolerated it.

After this PR the component carries five names for one idea — `valueProp`, `valueUnwrapped`, `isControlled`, `value`, `serializedValue` — and the PR added the fifth without retiring any. `useControlled` is called only for its dev-mode mode-switch warnings; its setter is discarded and, for a control mounted controlled, `value` is simply `valueProp`. The PR did not introduce that, but it built another layer on top of it.

## 5. `filled` has two sources of truth

```tsx
// lines 99-103
useIsoLayoutEffect(() => {
  if (validation.inputRef.current?.value) {
    setFilled(true);
  }
}, [validation.inputRef, setFilled]);
```

On mount `filled` is read from the DOM and can only move to `true`. On every later change it is read from the prop (line 112) or from the change event (line 150) and can move both ways. The base effect also handled the explicit empty case (`else if (hasExternalValue && valueProp === '') setFilled(false)`), which the PR deleted.

P11: swap a controlled control holding `"abc"` for a different controlled control holding `""` inside the same `Field.Root`. On base `data-filled` clears; on head it stays set, because the new instance mounts with an empty DOM value and the effect has no branch that lowers the flag. P10 is the mirror image for a `render` target with no DOM `.value` (base: filled, head: not filled); that case is exotic and I would not block on it, but it comes from the same asymmetry.

## 6. The disabled / unregistered path

`useRegisterFieldControl(…, !disabled, …)` unregisters the control while disabled, and `useFieldControlRegistration.captureInitialValue` only captures the baseline on the first real registration. The new prop sync does not consult `disabled`, so a programmatic change while the control is disabled runs:

```tsx
setDirty(serializedValue !== (validityData.initialValue ?? ''));
```

with `validityData.initialValue === null`, i.e. it compares against `''` because "no baseline captured yet" and "baseline is the empty string" are the same expression. It also runs `validation.change`, which calls the consumer's `validate` for a disabled control.

P4 (disabled while loading, value set to `"loaded"`, then enabled): on head `data-dirty` is set while disabled and `validate` is called once; after enabling, the baseline is captured as `"loaded"` — proven by dirty clearing when the user edits and returns to `"loaded"` — yet `data-dirty` is still set and the validation error is displayed before any interaction. The dirty flag and the baseline it is defined against disagree. On base none of this happens.

P12 shows `Switch` has the identical behavior on base, so this is inherited from the sibling pattern rather than invented here. It is newly reachable for text inputs because programmatic changes used to be ignored, and "disable the form while loading, then populate and enable" is a far more common shape for text inputs than for switches.

I did not find a clean local fix. Gating the sync on `disabled` trades this staleness for the opposite one (a control that was enabled, then disabled, then changed, then re-enabled would keep `dirty === false` against an already-captured baseline). The real owner is `Field.Root`: the place that captures the baseline is the place that can keep `dirty` consistent with it.

## 7. Worked code-judo proposal

The reframing: there is one event, "the field's value is now X", with two sources. Give it one function and let each source decide only whether to call it.

```diff
@@ -80,7 +80,8 @@
     state: 'value',
   });
 
-  const isControlled = valueProp !== undefined;
+  // React treats a nullish `value` as uncontrolled, so the field does too.
+  const isControlled = valueProp != null;
   const value = isControlled ? valueUnwrapped : undefined;
   // The DOM value is always a string, so dirty comparisons must serialize the controlled value.
   const serializedValue = value == null ? undefined : String(value);
@@ -97,21 +98,22 @@
   );
 
   useIsoLayoutEffect(() => {
-    if (validation.inputRef.current?.value) {
-      setFilled(true);
-    }
+    setFilled(Boolean(validation.inputRef.current?.value));
   }, [validation.inputRef, setFilled]);
 
-  useValueChanged(serializedValue, () => {
-    if (serializedValue === undefined) {
-      return;
-    }
-
+  const syncFieldState = useStableCallback((nextValue: string) => {
     clearErrors(name);
-    setDirty(serializedValue !== (validityData.initialValue ?? ''));
-    setFilled(serializedValue !== '');
+    // `validation.change` reads `markedDirtyRef`, so update dirty before validating.
+    setDirty(nextValue !== (validityData.initialValue ?? ''));
+    setFilled(nextValue !== '');
 
-    validation.change(serializedValue);
+    validation.change(nextValue);
+  });
+
+  useValueChanged(serializedValue, () => {
+    if (serializedValue !== undefined) {
+      syncFieldState(serializedValue);
+    }
   });
 
   const inputRef = React.useRef<HTMLElement>(null);
@@ -141,19 +143,12 @@
 
           // Controlled values sync from the `value` prop instead, so that a value the consumer
           // rejects or rewrites never reaches the field state.
-          if (isControlled) {
+          // `defaultPrevented` is a workaround for https://github.com/react/react/issues/9023
+          if (isControlled || event.nativeEvent.defaultPrevented || details.isCanceled) {
             return;
           }
 
-          // `validation.change` reads `markedDirtyRef`, so update dirty before validating.
-          setDirty(inputValue !== (validityData.initialValue ?? ''));
-          setFilled(inputValue !== '');
-
-          // Workaround for https://github.com/react/react/issues/9023
-          if (!event.nativeEvent.defaultPrevented && !details.isCanceled) {
-            clearErrors(name);
-            validation.change(inputValue);
-          }
+          syncFieldState(inputValue);
         },
         onFocus() {
           setFocused(true);
```

What this buys:

- One copy of the sequence, one copy of the ordering comment, one call order.
- One gate in the DOM handler. All four updates are skipped or applied together, so the half-applied state in P3 cannot be expressed.
- One predicate for "controlled". `value={null}` falls through to the DOM path, matching what React does with the element (P6 returns to base behavior).
- The mount effect states `filled` instead of only raising it (P11 returns to base behavior).

Net change against head: 39 changed lines, the handler shrinks from 21 lines to 14, and no new concept is introduced.

**One deliberate behavior delta, which needs the author's decision.** On base and on head, a prevented or canceled *uncontrolled* change still updates dirty and filled. The proposal skips them too. I think that is the defensible reading of "cancel stops the internal handling" and it is what `Switch` does (`if (eventDetails.isCanceled) return;` before any state is touched), but the DOM value of an uncontrolled input has already changed and cannot be reverted, so either choice leaves something out of step. The alternative that is also coherent is the opposite one: an uncontrolled native input cannot be canceled, so `isCanceled` is not consulted at all and only the React-bug guard remains. What is not coherent is the current middle position. Whichever is chosen, the test at `FieldControl.test.tsx:189` should assert dirty, filled and the error state, not only `validate`, and a controlled-mode cancel test should pin whatever the answer is there.

**Verification of the proposal.** I applied exactly this diff to a private copy of the head tree (the clone was not touched) and ran an 11-test port of the PR's `FieldControl` tests plus the probes against it. All 11 ported tests pass, as they do on head; three of them fail on base, which confirms the port still discriminates. The repository's own harness (`createRenderer`, `describeConformance`) was not run against the proposal because that would require writing into the clone. The proposal does not change P1 (render count), P4 (disabled baseline), or P5 (hydration); those are separate decisions.

## 8. A larger move, out of scope for this PR

This PR adds the tenth hand-written copy of the same shape. `SwitchRoot`, `CheckboxRoot`, `CheckboxGroup`, `RadioGroup`, `SelectRoot`, `SliderRoot`, `AriaCombobox` (twice), `NumberFieldInput`, `OTPFieldRoot` and now `FieldControl` each call `useValueChanged(value, () => { clearErrors(name); setDirty(…); [setFilled(…);] validation.change(value); })`, each with its own equality rule and its own omissions (Slider and Select do not set filled; NumberField does not set dirty there). Every one of them also already pushes its value into `Field.Root` on every change through `useRegisterFieldControl(ref, id, value, …)`, whose effect depends on `value`.

So `Field.Root` is told about each value change twice: once through registration, where it does nothing with the news, and once through four separate setter calls from the control. If the registration carried the control's equality rule (and an "is empty" rule), the root could do the sync itself when a registration's value changes, and it is the only place that knows whether a baseline exists (section 6). That would delete the sync block from every control rather than adding an eleventh variant. It is a cross-component refactor and should not be attached to this fix, but it is the structure this PR is leaning toward, and with it in place this fix would have been a registration-argument change rather than a second sync path.
