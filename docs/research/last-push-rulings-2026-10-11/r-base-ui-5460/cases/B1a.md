# Case B1a: a prevented input event on a controlled Field.Control

Pull request: mui/base-ui #5460, "[field] Sync controlled value changes with field state". The facts below are as they could be known at the last push to the pull request, 2026-08-10T13:33:57Z, when the reviewed commit `14d39e5` became its head. Facts from after that moment are only under the last heading.

Words used here. `Field.Control` is Base UI's text input for a form field. It is "controlled" when the app passes the text in through the `value` prop and stores each edit itself in `onValueChange`. A "prevented" event is one on which a script called `event.preventDefault()`. A "server error" is an error the app hands the `Form` through its `errors` prop. "Base" is the commit before the change. "Head" is the reviewed commit. The "earlier revision" is `6a40cf1`, the first commit pushed to the pull request (committed 2026-08-10T13:12:08Z), which the last push replaced. It was public by its SHA, and bot comments on the pull request name it; the task packet shows only the head's diff.

At the last push the pull request was a draft. It had no human review and three bot comments.

## What the change does

Before, one change handler served controlled and uncontrolled fields, behind one guard:

```diff
-          // Workaround for https://github.com/react/react/issues/9023
-          if (!event.nativeEvent.defaultPrevented) {
+          if (!event.nativeEvent.defaultPrevented && !details.isCanceled) {
             clearErrors(name);
             validation.change(inputValue);
           }
```

The change adds an early exit for controlled fields above that guard:

```diff
+          // Controlled values sync from the `value` prop instead, so that a value the consumer
+          // rejects or rewrites never reaches the field state.
+          if (isControlled) {
+            return;
+          }
```

and a new block that runs whenever the `value` prop changes. It cannot see the event:

```diff
+  useValueChanged(serializedValue, () => {
+    if (serializedValue === undefined) {
+      return;
+    }
+
+    clearErrors(name);
+    setDirty(serializedValue !== (validityData.initialValue ?? ''));
+    setFilled(serializedValue !== '');
+
+    validation.change(serializedValue);
+  });
```

So in controlled mode the guard is never reached. The field's state follows the value the app stores, whatever happened to the event.

## The runs

A controlled field in `<Form errors={{ message: 'Server error' }}>` with `validationMode="onChange"`. A script dispatches a cancelable `input` event, a listener prevents it, and the app stores the value. Run in the project's own test setup, in a simulated browser (jsdom) and in real Chromium, with identical results.

| Case | Base | Earlier revision | Head |
| --- | --- | --- | --- |
| Controlled, event prevented, app stores the value | validator not called, "Server error" stays | the same as base | validator called once with `"a"`, "Server error" gone |
| The app's own `onChange` calls `event.preventDefault()` on such an event | not called, error stays | the same as base | called once, error gone |
| Uncontrolled, event prevented | not called, error stays | the same | not called, error stays |
| Controlled, an ordinary event that cannot be prevented | called once, error gone | the same | called once, error gone |
| Controlled, the app calls `details.cancel()` and does not store the value | called once, error gone | not called, error stays | not called, error stays |

Real typing: keys typed through Chromium while two handlers call `preventDefault()`. Every `input` event arrived as `cancelable: false, defaultPrevented: false`, the validator ran and the error cleared, at all three commits. Firefox and WebKit were not run.

## What bears on "Promised?"

**The operation.** A controlled `Field.Control` skipping error clearing and validation when the native `input` event was prevented and the app still stored the value. The guard is Base UI's. The event belongs to the browser and to React.

**The project's documentation at the head** (the change touches none of it). No page mentions prevented events on a field. Neighbouring sentences:
- Forms handbook: "`onChange` validates the field when the value changes, for example, after each keypress in a text field or when a checkbox is checked or unchecked."
- Customization handbook: "`cancel` stops the component from changing its internal state." and "This lets you leave the component uncontrolled as its internal state is prevented from updating. This is an alternative to controlling the component with external state and guarding the state updates conditionally."

**The owners of the event.**
- The UI Events specification (last changed 2025-12-17), in its table for the `input` event: "Cancelable | No".
- React issue 9023, which the guard's comment cites (opened 2017-02-17, open): "event.preventDefault in click handler does not prevent onChange from being called". It is about a checkbox: "When rendering an `input` element of type `checkbox` with an `onClick` and `onChange` handler, `onChange` is still called even though `event.preventDefault()` is called in the `onClick` handler."

**The change's own words.**
- The code comment in the diff: "Controlled values sync from the `value` prop instead, so that a value the consumer rejects or rewrites never reaches the field state."
- The description as it read at the last push (written 2026-08-10T13:16:20Z). It does not mention the prevented-event guard. Its passages on the paths:
  - "`Field.Control` was the only Field control that ignored controlled value changes. Switch, Checkbox, Checkbox Group, Radio Group, Select, Slider, Combobox, Number Field, and OTP Field all sync through `useValueChanged` already. `Field.Control` drove everything from the DOM change event, so a value set from code had no path at all."
  - "`Field.Control` needs one guard the others do not, because it has two paths: the DOM change event and the controlled prop. `processedValueRef` records the value the change handler just handled, so the prop echo does not validate the same keystroke twice."
  - "`details.cancel()` in `onValueChange` now stops the internal handling. It was ignored."
- The description and the head's code differ. No `processedValueRef` exists at the head. For a controlled field the head's change handler returns before it handles anything, so only the `value` prop path acts. The description was written for the earlier revision, where the change handler still handled controlled keystrokes, the guard still applied to them (run, the table above), and the handler carried the comment "Even a canceled change counts as processed: the consumer opted out of Base UI handling for it, so the controlled echo must not validate it either."
- Tests. The existing test "does not clear errors or validate when change is prevented" renders a control without a `value` prop. The change left it unchanged and added no controlled twin. The first row of the table is that test written for a controlled control: it passes at the base and fails at the head.

**Maintainers before the last push** (four searches of the project's issues and pull requests; 10 hits, 7 read, 3 bot dependency updates unread). Nothing says whether a prevented `input` event must stop validation in a controlled `Field.Control`. The guard is discussed for another control: in #4810 a maintainer wrote on 2026-05-15 that NumberField's hidden input "still only guards `defaultPrevented`" and proposed a predicate to "ignore this hidden input change when the native event was already prevented or the root is locked". The guard reached `Field.Control` in #4923 (merged 2026-05-28) and its test in #5281 (merged 2026-07-20); neither thread discusses it.

**Public code** (searched 2026-10-11; 368 files returned for Base UI field or input imports together with `preventDefault` and a change handler; every change handler and every `input` or `change` listener in them checked for preventing the event). In files dated up to the last push, the only code that prevents an `input` event at a field is Base UI's own test, and two copies of it in ports of the library. The search count varied between result pages, so it cannot prove that no such program exists.

**The documented way to veto a change, at the head.** A controlled app that does not store the value gets no validation and keeps the server error.

## What the affected person sees

- With a script-dispatched cancelable `input` event that is prevented, and an app that stores the value: at the base the validator is not called and "Server error" stays; at the head the validator is called once with the stored value and "Server error" disappears.
- A person typing sees no difference at any commit.
- Nobody is blocked at either commit.

## Impact facts

- **Who is affected:** an app, a test or a library that dispatches its own cancelable `input` events at a controlled `Field.Control`, prevents them, and still stores the value. None was found.
- **How:** a server error on the field is cleared and the validator runs for the stored value, where both were skipped before.
- **How stuck:** not stuck. Nothing fails, no data is lost, and no submit is blocked or wrongly allowed in any run.
- **How common:** not measured. A real browser does not send a preventable `input` event for typing (run in Chromium; the specification says the event is not cancelable).
- **What an app can do:** decline to store the value; then the field does nothing at the head.

## Known only later

- 2026-08-10T13:34:21Z, 24 seconds after the last push: the author rewrote the description. The passage on the two paths became "The fix gives each mode one owner instead of running two sync paths at once. `useValueChanged` owns the controlled path, `onChange` owns the uncontrolled path, and `onChange` returns early when controlled." A list item became "A controlled value the consumer rejects or rewrites no longer reaches the field state, matching Combobox, Number Field and OTP Field."
- 2026-08-13: the author marked the pull request ready and merged it six seconds later, with no review.
- 2026-08-25: a follow-up, #5563, fixed two other regressions from this change and does not touch this path.
- 2026-09-04: shipped in v1.8.0. The release note is the pull request's title.
- 2026-08-31 to 2026-10-10: two public programs treat a prevented change event as their consumer's veto on a Base UI input: `nocpiun/gitlab-ui-react` (`form-input.tsx`, on `@base-ui/react` `^1.7.0`) and `qingye-lab/qingye-ui` (`tag-input.tsx`, on `1.7.0`). Their change handlers call the consumer's `onChange` and return without storing the value when `event.defaultPrevented` is set. Both read React's own flag, and when they veto they do not store the value.
- 2026-10-09: upstream `master` still has the early return above the guard. No issue or pull request opened after the push reports this.
