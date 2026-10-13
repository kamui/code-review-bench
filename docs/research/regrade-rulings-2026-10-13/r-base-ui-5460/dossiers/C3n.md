# C3n: a number respelled in a controlled number field (`5` to `05`) updates nothing in the field state

Pull request: mui/base-ui #5460, "[field] Sync controlled value changes with field state".
Candidate covered: none in the packet. This group was added under point 7 of the round's brief: it is what the comment of credit question C3 describes in its own words.

Words used here. `Field.Control` is Base UI's input for a form field. It is "controlled" when the app passes it a `value` prop and stores each edit itself in `onValueChange`. A "respelling" is an edit that changes the text of a number input and not the number: `5` to `05`, `5.0` or `005`. The "field state" is what Base UI keeps about a field: whether it is dirty, filled or invalid, and its errors. A "server error" is an error the app hands the `Form` through its `errors` prop. "Send" is the form's submit button. "Base" is the commit before the change, `30b8ea2`. "Head" is the reviewed commit, `14d39e5`. The "earlier revision" is `6a40cf1`, the first commit pushed to the pull request, which the last push replaced.

**The times that matter** (read, `upstream/C-pr-5460-usercontentedits.json`, `upstream/C-pr-5460-timeline.json`): the description was written at 2026-08-10T13:16:20Z; the last push, the cut-off, is 2026-08-10T13:33:57Z; the author rewrote the description at 13:34:21Z, 24 seconds later; the pull request was marked ready and merged by its author on 2026-08-13 with no review. Every fact below is marked as knowable at the cut-off or later.

**Is this already decided?** No. Three saved rulings are near it and none covers it.

- Second-pass ruling 30 and last-push ruling 5 ruled a keystroke the app *rejects* a suggestion. There the input shows the old text again. Here the input keeps the new text and the form would send it.
- First-round ruling 24 and last-push ruling 3 ruled a suggestion that the validator gets the text of the `value` prop while the input holds other text. There the value did change and the field state did react.
- GT-r5 is a checkbox or radio whose `value` prop never changes. Its opening clause is general enough to cover this case. Its title, trigger and recorded cause are about a toggle.

**Grouping.** One thing: one edit, one app, one result in three parts of the field state (server error, dirty mark, validation). The parts share the lines, the trigger and the fix, so they are not split.

**The control.** This pull request is recorded as `known-problems`, not as an audited clean control. A new known problem would end no clean status.

## Problem

A controlled `Field.Control` has `type="number"`. The app holds a number and stores `Number(next)` on each edit. The person changes the text to another spelling of the same number, for example by typing a `0` in front of `5`.

- The input shows `05`. The app still holds `5`. The form, if sent, sends `"05"`.
- Before the change, that edit cleared a server error on the field, marked the field dirty and, in a mode that validates on change, validated it.
- After the change it does none of the three. A server error stays and Send stays blocked until the person makes an edit that changes the number.

## What changed

```diff
         onChange(event) {
           const inputValue = event.currentTarget.value;
-          onValueChange?.(inputValue, createChangeEventDetails(REASONS.none, event.nativeEvent));
+          const details = createChangeEventDetails(REASONS.none, event.nativeEvent);
+          onValueChange?.(inputValue, details);
+
+          // Controlled values sync from the `value` prop instead, so that a value the consumer
+          // rejects or rewrites never reaches the field state.
+          if (isControlled) {
+            return;
+          }
+
           // `validation.change` reads `markedDirtyRef`, so update dirty before validating.
           setDirty(inputValue !== (validityData.initialValue ?? ''));
```

```diff
+  // The DOM value is always a string, so dirty comparisons must serialize the controlled value.
+  const serializedValue = value == null ? undefined : String(value);
...
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

In plain words. Before, every edit event cleared the field's server error, set the dirty mark from the typed text and handed the typed text to validation. After, a controlled field leaves that handler early. The three steps moved to a new block that runs only when the text form of the `value` prop changes. `Number("05")` is `5`, whose text form is `"5"`, the same as before. So the block does not run.

One more fact makes the case what it is, and it is React's. For most inputs React writes the `value` prop back into the input when the two differ, so a rewritten text disappears from the screen. For `type="number"` React compares as numbers and leaves the text alone when the numbers are equal (read, `upstream/C3n-react-ReactDOMInput-v19.2.5.js` lines 121 to 131, with the source comment "We explicitly want to coerce to number here if possible."; run, below). So `05` stays in the input.

**Runs.** `probes/C3n/probe.test.tsx` (nine cases, the text set with a change event; jsdom and real Chromium give identical lines), `probes/C3n/probe-browser.test.tsx` (real key presses in Chromium) and `probes/C3/probe.test.tsx`. The table is under "What the affected person sees".

## Promised?

**The exact operation.** The field state of a controlled number-typed `Field.Control` reacting to an edit that changes the input's text and not the number the app stores: clearing a server error, setting the dirty mark, validating on change.

**Who owns it.** The field state, the `Form`, its `errors` prop and `Field.Control` are Base UI's. That a controlled number input keeps the person's spelling when the number is equal is React's. That `05` is a valid text for a number input is the browser's.

**How the owners classify it.** `Field.Control`, `Input`, their `value` prop and `onValueChange` are public. The reference gives the type of `value` as `string | string[] | number`. What the field state does on an edit that leaves a numeric `value` the same is unstated.

**The six places searched.** Each is knowable at the cut-off unless marked.

1. **Project documentation at the head** (`upstream/C3n-project-docs-search.txt`; nine searches in a clone, 125 hit lines, all read; the change touches no documentation file).
   - On validating, the forms handbook: "`onSubmit` (default) validates all fields when the containing `<Form>` is submitted, afterwards invalid fields revalidate when their value changes." and "`onChange` validates the field when the value changes, for example, after each keypress in a text field or when a checkbox is checked or unchecked."
   - On a server error, the same page: "Once a field's value changes, any corresponding error in `errors` will be cleared from the field state."
   - On dirty, the field reference: "Whether the field's value has been changed from its initial value." and "Present when the field's value has changed."
   - On the prop, the Input reference: `value`, type `string | string[] | number`, "The value of the input. Use when controlled."; `onValueChange`, "Callback fired when the `value` changes. Use when controlled."
   - No documentation page shows `type="number"` on a `Field.Control` or `Input`, and none says whose value "the field's value" is, the app's or the input's text. The documented control for numbers is Number Field, "A numeric input element with increment and decrement buttons, and a scrub area."
   - Every one of these sentences ties the reaction to the value changing. Here the app's value does not change, the input's text does, and what the form sends does.
2. **The owning dependency's documentation** (`upstream/C3n-react-dev-input.md`, React's page for `<input>` at its last change before the cut-off, 2026-04-08; `upstream/C3n-react-ReactDOMInput-v19.2.5.js`; both read).
   - React's page shows a controlled number input that stores the text: `const [age, setAge] = useState('20');`, `const ageAsNumber = Number(age);`, `onChange={e => setAge(e.target.value)}` with `type="number"`.
   - The same page, under "My input caret jumps to the beginning on every keystroke", says that if you control an input "you must update its state variable to the input's value from the DOM during `onChange`." and "You can't update it to something other than `e.target.value`".
   - React's page says nothing about spellings of a number. Its source keeps the spelling on purpose, as quoted above.
   - So the app in this case, which stores `Number(next)`, does what React's page says a controlled input must not do, and React's code nonetheless supports a numeric `value` on a number input.
3. **The change itself** (`upstream/C-pr-5460-usercontentedits.json`, `upstream/C-pr-5460.json`; the diff read in the clone; three records, all read).
   - The code comment on the early return: "Controlled values sync from the `value` prop instead, so that a value the consumer rejects or rewrites never reaches the field state."
   - The code comment on the new variable: "The DOM value is always a string, so dirty comparisons must serialize the controlled value."
   - The description as it stood at the cut-off (`task_packet`, identical to the first version in the edit history): "**Dirty state never cleared for non-string values.** The dirty check compares the input's string value against a baseline captured from the `value` prop. With `value={5}` the baseline was the number `5`, so `"5" !== 5` held forever and `data-dirty` stayed set even after the user returned the field to its initial value. Array values failed the same way by reference. The control now registers the serialized value, so the baseline and the comparison agree."
   - The same description on the two paths: "`Field.Control` needs one guard the others do not, because it has two paths: the DOM change event and the controlled prop. `processedValueRef` records the value the change handler just handled, so the prop echo does not validate the same keystroke twice." No `processedValueRef` exists at the head. That paragraph describes the earlier revision, where the change handler still handled every controlled edit.
   - The change adds a test with this app on a text input: "clears dirty state when a numeric controlled value returns to its initial value" uses `React.useState(5)` and `onValueChange={(nextValue) => setValue(Number(nextValue))}`, sets the text to `56` and back to `5`, and expects the dirty mark to go. No added test uses `type="number"`, a respelling or a server error.
4. **Maintainers up to the last push** (`upstream/C3n-maintainers-search.py`, `C3n-maintainers-hits.json`, `C3n-maintainers-read.txt`; nine searches of mui/base-ui limited to `created:<2026-08-10T13:33:57Z`; 25 distinct hits; 24 fetched with their comments and reviews, and every passage that mentions a number input, a leading zero, a numeric or string value or the dirty mark read; 1 unread, a dependency update opened by a bot. One of the 24 is this pull request, whose body GitHub returns as it reads now, after the rewrite).
   - On a number-typed `Field.Control`, a numeric controlled value, or an edit that leaves the stored value the same: nothing.
   - Issue #2108 (2025-06-12, a user): the `value` given to `onValueChange` was typed like the `value` prop; the report says input elements "**only** pass type `string`". It was fixed four days later. It is about the type of the callback and not about field state.
   - Issue #5424 (2026-08-06, a maintainer, on Number Field and not on `Field.Control`): "Text that resolves to a number the user did not enter should either be rejected, or normalized in the visible input, rather than leaving the display and the submitted value silently divergent." It concerns text and number that disagree. Here `05` and `5` are the same number.
5. **Public code** (`upstream/C3n-code-search.py`, `C3n-code-files-index.json`, `C3n-code-screen2.json`, `C3n-code-dates.json`; searched 2026-10-13; seven code searches, 238 distinct files fetched, 1 dropped unread because it is this benchmark's own repository; 43 files have a number-typed element on Base UI's own `Field.Control` or `Input`, 86 elements, each read).
   - Programs that do the operation, a number-typed Base UI control in a `Field.Root` with a numeric `value` and a handler that stores a parsed number:
     - `raterudder/raterudder`, `web/src/pages/Settings.tsx`: several such inputs, for example `value={settings.alwaysChargeUnderDollarsPerKWH}` with `onChange={(e) => handleChange('alwaysChargeUnderDollarsPerKWH', parseFloat(e.target.value))}`. On `@base-ui/react` `^1.2.0`. Present in the file's last commit before the cut-off, 2026-07-28.
     - `fenrick/MiroDiagramming`, `src/ui/pages/arrange-tab.v2.tsx` and `resize-tab.v2.tsx` (2025-11-23 to 2025-11-24): `value={spacing.spacing}` with `onValueChange={(value) => { setSpacing({ ...spacing, spacing: Number(value) }) }}` (line breaks removed). On `@base-ui-components/react` `1.0.0-beta.6`, the package's earlier name.
     - `danielhsy/hyperbolic-networks`, `visualization/src/main.tsx` (2025-11-29 to 2025-12-01): `value={round(zoom, 4)}` with `onValueChange={(newValue) => setZoom(newValue ? +newValue : 1)}`. On `@base-ui-components/react` `^1.0.0-beta.4`.
     - Later than the cut-off: `rejot-dev/fragno`, `backoffice-json-form.tsx` (2026-09-30): `value={typeof data === "number" ? data : ""}` with `onValueChange` storing `Number(value)`.
   - None of these files passes `errors` to a Base UI `Form`, gives the field a `validate` function or reads Base UI's dirty mark. `rejot-dev/fragno` sets the field's `invalid` prop from its own form library. They show that a numeric value in a number-typed Base UI input is something people build. They do not show anyone depending on the field state reacting to a respelling.
   - A search cannot prove that no program depends on it. Inputs wrapped in a project's own component in another file were not followed.
6. **The documented way to do the same thing, run at the head** (`probes/C3n/result-head.txt`).
   - React's documented way, storing the text: with `value={text}` and `onValueChange={setText}` the edit to `05` changes the value, the server error clears, the field is dirty and Send goes through (case C3n-6, `type-number-app-stores-the-text`), at both commits.
   - Base UI's documented control for numbers, Number Field, controlled with the number 5: when its text is set to `05` it calls `onValueChange(5)`, keeps "Server error" and Send stays blocked, at both commits; on blur it writes `5` back (case C3n-8).

**The platform alone.** A plain `<input type="number">` with no library, with a `0` typed in front of `5` in Chromium: the text is `05`, `valueAsNumber` is 5, one input event fires and the input is valid (run, C3n-B4).

**What the code and the project's tests deliberately support.** A numeric controlled value with an app that stores `Number(next)` is in the change's own test, on a text input. An uncontrolled `Field.Control type="number"` is in `FieldValidity.test.tsx` at both commits. The two together, a number-typed control with a numeric controlled value, are in no test. They are in one private experiment page of the documentation site, which uses the native `onChange` and `valueAsNumber` (`docs/src/app/(private)/experiments/_components/SettingsPanel.tsx`).

**Whether it worked before, and whose behaviour changed.** Before the change the respelling cleared the error, validated in `onChange` mode and set the dirty mark (run, at the base and at the earlier revision). The dirty mark at the base was the stuck mark the change set out to fix: it stayed set even after the text went back to `5` (run, C3n-3). Base UI's behaviour changed, between the earlier revision and the head. React's did not.

**Which of the three ways.** None was found for the exact operation.

- Written: the documentation promises each reaction when "the field's value" changes, and does not say whose value that is.
- Announced: the change says controlled values sync from the `value` prop, that a value the app "rejects or rewrites never reaches the field state", and that the dirty comparison uses the serialized value. Storing `05` as the number 5 is a rewrite in the plain sense of the word, though the person's text stays on screen.
- Built: the app shape is in the change's own test, and that test expects the dirty mark to follow the stored number.

How a supported use behaved before can be part of its promise (Promised 7a). Rule Promised 2a says a change that says a behaviour is changing ends the old one, and its second line says that a change which names a case and says it no longer reaches the field state announces every part of the field state. On that rule the answer proposed is **no**. Two things point the other way, and they are set out under "Where the rules do not decide".

## Delivered?

Not asked, because the answer proposed above is no.

If the owner reads the documentation's "the field's value" as the text the form would send, the promise is that a changed value clears a server error, marks the field dirty and is validated. That outcome does not arrive for a respelling at the head: the form would send `"05"` in place of `"5"` and the field reports no change (run, C3n-3).

## Intended or announced

**Knowable at the cut-off.**

- The two code comments and the description's paragraph on the dirty state, quoted above. Together they say the field state follows the `value` prop in its text form.
- The description does not mention a number input, a spelling, or a value the app rejects or rewrites. Its paragraph on the two paths describes the earlier revision. At the earlier revision a respelling still cleared the server error, set the dirty mark and was validated (run, `probes/C3n/result-earlier-revision.txt`: the same as the base in every row about the error, the dirty mark after the edit and Send). So the behaviour this group is about first appears at the head, and the only prose at the cut-off was written before it existed.

**Later.**

- The rewrite of 2026-08-10T13:34:21Z replaced that paragraph with "`useValueChanged` owns the controlled path, `onChange` owns the uncontrolled path, and `onChange` returns early when controlled." and added "A controlled value the consumer rejects or rewrites no longer reaches the field state, matching Combobox, Number Field and OTP Field."
- Shipped in v1.8.0 on 2026-09-04; the release note is the pull request's title (`upstream/C3n-releases.json`, `C3n-compare-5460-v1.8.0.json`).

## What the affected person sees

Setup: a controlled number field holding 5, the app storing `Number(next)`, in a `Form` with a Send button. Rows marked "keys" were typed with real key presses in Chromium; the rest set the text with a change event, in jsdom and Chromium with identical results.

| What the person does | Base | Head |
| --- | --- | --- |
| "Server error" showing. Types `0` in front of the `5` (keys) | the input shows `05`; the error clears; the field is dirty | the input shows `05`; **the error stays**; not dirty |
| Then presses Send (keys) | sends `amount: "05"` | **blocked**, error stays |
| Then types `6` at the end, `056`, and presses Send (keys) | sends `"056"` | the error clears at the `6`; sends `"056"` |
| "Server error" showing. Types `.0` after the `5` (keys) | at the `.` the app is not called and nothing changes; at the `0` the input shows `5.0` and the error clears | at the `0` the input shows `5.0`; **the error stays**; Send blocked |
| "Server error" showing. Deletes the `5` and types it again (keys) | the error clears at the deletion, when the app stores 0 | the same: the error clears at the deletion |
| The text set to `5.00`, `5e0` or `005` with a server error | the error clears, Send goes through | **the error stays**, Send blocked |
| No error. The text becomes `05`; Send | dirty; sends `"05"` | **not dirty**; sends `"05"` |
| The text then goes back to `5` | still dirty (the stuck mark the change fixes) | not dirty |
| `validationMode="onChange"`, the text becomes `05` | the validator is called with `"05"` | **the validator is not called** |
| The field's own validator said "Too low" for 5 at a first Send; the text becomes `05` | validated again with `"05"`, "Too low" stays | not validated, "Too low" stays |
| A validator that reads the text, "No leading zero", `onChange` mode, the text becomes `05` | "No leading zero" is shown; Send then validates the number 5 and sends `"05"` | nothing is shown; Send validates `"5"` and sends `"05"` |
| `validationMode="onBlur"`: the text becomes `05`, then the field loses focus | validated with `"05"` | validated with `"05"` |
| The app clears its own `errors` state in `onValueChange` | the error clears, sends | the error clears, sends |
| The same input with `type="text"` | React writes `5` back; the error clears | React writes `5` back; the error stays |
| The app stores the text instead of a number | the error clears, dirty, sends `"05"` | the same |
| The same input with no `value` prop | the error clears, dirty, sends `"05"` | the same |
| Base UI's Number Field, controlled, its text set to `05` | "Server error" stays; Send blocked | the same |

**What the person must do at the head to send the form again.** Make an edit that changes the number, such as typing a digit. They can then put the old number back.

**How stuck they are.** One edit away. Nothing on screen says why the error stayed, and unlike a rejected keystroke their edit is visibly there. Deleting and retyping the number clears the error, because the number passes through another value on the way.

**What the app's developer can do.** Store the text and parse it where the number is needed, as React's page shows. Or clear the app's own `errors` state in `onValueChange`.

**The other side of the same change.** At the base the dirty mark of a numeric field never cleared once it was set, and a validator could be shown text the app does not hold.

## What the maintainers did

- Up to the last push: nothing. The pull request was a draft with no review (read, `upstream/C-pr-5460-reviews.json`, `C-pr-5460-review-comments.json`, `C-pr-5460-issue-comments.json`: no review, no review comment, three bot comments).
- Later: the author rewrote the description 24 seconds after the push and merged the pull request on 2026-08-13 with no review. No maintainer has mentioned this case.
- Later: no issue or pull request opened after the push reports it (`upstream/C3n-later-number-field-control.json`, `C3n-later-leading-zero.json`, `C3n-later-controlled-unchanged.json`: no hit; `C3n-later-field-control.json`: six threads on `Field.Control`, none of which mentions a number input or a spelling, by a search of their titles and first posts).
- Later: upstream `master` of 2026-10-12 has the same early return under the same comment (read, `upstream/C3n-master-FieldControl.tsx`).
- Nobody said it does not come from this change.

## How each fact is known

- Every row of the table, the neighbours and Number Field, at the base and the head, in jsdom and in Chromium: **run** (`probes/C3n/probe.test.tsx`, `probes/C3/probe.test.tsx`, `result-*.txt`).
- The rows marked "keys" and the plain input: **run** in Chromium with real key presses (`probes/C3n/probe-browser.test.tsx`, `result-browser-*.txt`).
- The earlier revision: **run** in jsdom (`probes/C3n/result-earlier-revision.txt`, `result-grouping-earlier-revision.txt`).
- The four routes of GT-r5, the respelling cases and the controls under each patch, and the project's own tests under each patch: **run** (`probes/C3n/probe-grouping.test.tsx`, `patches/`, `result-grouping-*.txt`, `result-project-tests-*.txt`).
- The diff and the mechanism: **read** (the clone at both commits).
- Both description versions and their times; that there was no review: **read** (the saved GitHub records).
- The handbook and reference sentences, React's page and source, the maintainers' threads, the release and upstream master: **read**.
- Public code: **read** for the 86 elements in 43 files and for the five files quoted; the other 194 files were screened by script and have no such element.
- How common respelling a number is among people filling in forms: **not established**.

Not run: Firefox and WebKit; upstream master; a real form library; a phone keyboard; a locale that types a comma for the decimal point.

## Relation to existing reference families and ruled claims

**GT-r5** (a checkbox or radio `Field.Control` with a `value` attribute no longer clears its server error or validates when clicked). Both pass through the same two places.

The early return, which stops a controlled control's edit event before any field state is touched:

```ts
          if (isControlled) {
            return;
          }
```

The prop block, which touches the field state only when the text form of the `value` prop changes:

```ts
  useValueChanged(serializedValue, () => {
```

The three tests of rule Before 4, by running (`probes/C3n/probe-grouping.test.tsx`; each patch applied to the head alone; GT-r5's four routes G1 to G4, the respelling cases R1 and R2, and three controls, all run against each; jsdom and Chromium agree):

| Patch | What it changes | GT-r5: G1 to G4 | Respelling: R1, R2 | Rejected keystroke (K1) | Accepted keystroke (K2) | The project's own tests |
| --- | --- | --- | --- | --- | --- | --- |
| none (the head) | | fail | fail | error stays | validated once | 74 pass |
| `fix-gt-r5-alone` | the handler does not return early for a checkbox or radio | **cured** | fail | error stays | once | 74 pass |
| `fix-respelling-alone` | after a controlled edit event, the text the input keeps is compared with the last text seen, and a new text is synced | fail | **cured** | error stays | once | 74 pass |
| `fix-both` | the two together | cured | cured | error stays | once | 74 pass |
| `fix-input-own-state` | one mechanism: the second patch, with a checkbox's or radio's ticked state counted as part of what the input holds | cured | cured | error stays | once | 74 pass |
| `fix-every-change-event` | the early return removed | cured | cured | **error clears**, as at the base | **twice** | 1 fails: "validates once when a controlled value is changed by the user", a test the change adds |

- **Same lines?** Yes. Both fail at the early return and are missed by the prop block.
- **Would one fix cure both?** The smallest fix for each place cures only its own. Two single fixes do cure both. One watches what the input itself holds, its text or its ticked state. The other removes the early return, which undoes what the code comment says about rejected values and fails a test the change adds.
- **Is one sentence about the cause true of both?** Yes: "an unchanged `value` prop is taken to mean the person changed nothing in the input, and that is false here." GT-r5's recorded cause, a `value` prop "taken to mean the prop carries the edited state, which is false for checkbox and radio inputs", is true here for the spelling and false for the number. That sentence is false of a rejected keystroke.

In GT-r5 the click changes what the field holds, whether the box is ticked, and the `value` prop was never meant to carry it. Here the edit changes what the input shows and what the form sends, and the `value` prop carries the number and not the spelling. The grouping is the owner's.

**The rejected keystroke** (second-pass ruling 30, last-push ruling 5; a suggestion). The same lines and the same code comment. There the keystroke changes nothing on screen and nothing the form sends. Here it changes both. With `type="text"` in place of `type="number"` this case becomes that one: React writes `5` back (run, C3n-6).

**`CL-r-sanitized-value`** (first-round ruling 24, last-push ruling 3; a suggestion). The validator gets the text of the `value` prop while the input holds other text. That ruling noted that no run produced a blocked submit. Here Send is blocked while a server error shows.

**`CL-r-array-text-form-dirty`** (second-pass ruling 29; a suggestion). Two values with one text form compare as unchanged. In that case "The dirty state matches what the input shows and what the form submits." Here it does not: the input shows `05`, the form sends `"05"`, and the field is not dirty.

**GT-r4** (the validator gets `"5"` for the number 5 on submit) is the same app shape and another fault: it is about the type handed to `validate`, and its statement allows the dirty comparison to use the text form.

Not GT-r1, GT-r2, GT-r3, GT-r6, GT-r7, GT-r8 or GT-r9.

## Both sides

For a problem:

- The person's edit is real. The input shows it and the form would send it, and the field says nothing changed: a server error stays, Send is blocked, and the dirty mark is off while the value to be sent differs from the initial one.
- The documentation ties each reaction to the field's value changing, and the value the `Form` itself reports for the field did change.
- The use is supported. The reference gives `number` as a type of `value`, and the change's own test has this app.
- It worked before, and at the earlier revision the description was written for.
- GT-r5 is the same lines and the same wrong inference, and its statement of what was owed opens with a clause this case meets.
- The description at the cut-off says nothing about it.

Against:

- The change says it: controlled values sync from the `value` prop, a value the app rewrites never reaches the field state, and the dirty comparison uses the serialized value. The app here rewrote `05` to `5`.
- Two saved rulings on the same code comment went this way, one for a rejected keystroke with the same blocked Send, one for an input holding other text than the `value` prop.
- The documented controls agree with the head. Number Field keeps the server error on the same edit at both commits, and an app that stores the text, as React's page shows, works.
- React's page says a controlled input must be updated to `e.target.value`. The app in this case does not do that.
- The number is the same. A field whose value is the number 5, as it was at the start, can be called unchanged, and the base called it dirty for ever.
- The person is one ordinary edit away, and respelling a number without changing it is an unusual thing to type.
- Nobody is shown depending on it.

## Recommendation

> **What happens:** in a controlled number-typed `Field.Control` whose app stores a number, an edit that respells the same number (`5` to `05`) stays in the input and would be sent, and updates nothing in the field state: a server error stays and Send stays blocked, the dirty mark stays off and validation does not run, where before the change every edit did all three. **Promised: no**, a code comment in the change says controlled values sync from the `value` prop "so that a value the consumer rejects or rewrites never reaches the field state", the description says the control now compares the serialized value, the documentation ties each reaction to the field's value changing without saying whose value, Number Field keeps the error on the same edit, and no maintainer statement up to the push, public program or documented way was found that promises a reaction to a change of spelling. **Delivered: not asked**, and the documented ways, storing the text or using Number Field, behave at the head as at the base. **So: suggestion or observation (improvement).** **Band, decided separately:** none.

Recommendation code: `suggestion`. Confidence: low.

It is not `relied-on`: three public programs before the cut-off build a number-typed Base UI input with a numeric value, and none is shown depending on the field state reacting to a respelling. They are recorded in the summary so the owner can weigh them.

If the owner rules it a problem, the band proposed is other-material: the person who respells a number after a server error is one edit away from sending, the usual way of editing a number changes it, and nothing is lost. If the owner rules it part of GT-r5, it takes GT-r5's label and adds no weight.

**Where the rules do not decide.** Two rules point different ways, so the case is the owner's whatever the confidence.

- Promised 2a against Promised 6 and 7a. The use is supported by the accepted type and by the change's own test, and it behaved better before. 2a ends that only "for what it says". Whether "a value the consumer rejects or rewrites" says it for a number the app parses, while the person's text stays on screen and would be sent, is a reading. The line under 2a was written from the rejected keystroke, where nothing stays on screen.
- Before 4. The three tests come out yes, no and yes against GT-r5, and one wider fix cures both without undoing anything. Grouping is always the owner's.
- The description at the cut-off describes a design in which this could not happen. Decision P27 makes that description what the change says. It does not say what a code comment announces when the description describes something else. Last-push ruling 5 let the code comment stand for the rejected keystroke.

**Strongest argument against the recommendation.** This is not a rejected keystroke. The person typed something, it stayed, and the form will send it. A field that then reports itself unchanged, keeps a server error and refuses to send is reporting something untrue about its own state, in a use the reference lists and the change tests. That is GT-r5's fault in another input type: an unchanged `value` prop read as "nothing was edited". The only words against it are one code comment about rejecting and rewriting, written about apps that discard input.
