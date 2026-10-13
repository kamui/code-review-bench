# Card: a number respelled in a controlled number field leaves the field's state as it was

Pull request: mui/base-ui #5460. Base UI is a React component library. `Field.Control` is its input for a form field; it is "controlled" when the app passes it a `value` prop and stores each edit itself. The field keeps a state of its own: whether it is dirty (changed from its initial value) or invalid, and its errors. A "server error" is an error the app hands the form to show on a field. "Send" is the form's submit button. "Base" is the commit before the change and "head" the reviewed commit.

## The runs

The field: `<Field.Control type="number" value={value} onValueChange={(next) => setValue(Number(next))} />` with `value` starting at the number 5, in a form with a Send button. A "respelling" is an edit that changes the text and not the number: `5` to `05`, `5.0` or `005`. After it the input shows the new text, the app still holds 5, and the form, if sent, sends the new text.

Run in the project's own test setup at both commits. Rows marked "keys" were typed with real key presses in Chromium. The other rows set the text with a change event, in jsdom and in real Chromium with identical results. Probes: `probes/C3n/`, `probes/C3/`. All *run*.

| What the person does | Base | Head |
| --- | --- | --- |
| "Server error" showing. Types `0` in front of the `5` (keys) | the input shows `05`; the error clears; the field is dirty | the input shows `05`; the error stays; the field is not dirty |
| Then presses Send (keys) | sends `amount: "05"` | does not send; the error stays |
| Then types `6` at the end, giving `056`, and presses Send (keys) | sends `"056"` | the error clears at the `6`; sends `"056"` |
| "Server error" showing. Types `.0` after the `5` (keys) | the error clears when the input shows `5.0` | the error stays; Send does not send |
| "Server error" showing. Deletes the `5` and types it again (keys) | the error clears at the deletion, when the app stores 0 | the same |
| "Server error" showing. The text set to `5.00`, `5e0` or `005`, then Send | the error clears; sends | the error stays; does not send |
| No error. The text becomes `05`; Send | the field is dirty; sends `"05"` | the field is not dirty; sends `"05"` |
| The text then goes back to `5` | the field is still dirty | the field is not dirty |
| The field validates on every change; the text becomes `05` | the app's validator is called with `"05"` | the validator is not called |
| The field's validator said "Too low" for 5 at a first Send; the text becomes `05` | validated again with `"05"`; "Too low" stays | not validated; "Too low" stays |
| The field validates on blur; the text becomes `05`, then the field loses focus | validated with `"05"` | validated with `"05"` |
| The app clears its own errors when it is told of an edit | the error clears; sends | the error clears; sends |

Neighbours, each with "Server error" showing and the text set to `05`, then Send. All *run*.

| Field | Base | Head |
| --- | --- | --- |
| The same field with `type="text"` | the input goes back to `5`; the error clears; sends `"5"` | the input goes back to `5`; the error stays; does not send |
| The same number input, the app stores the text and not a number | the error clears; dirty; sends `"05"` | the same |
| The same number input with no `value` prop | the error clears; dirty; sends `"05"` | the same |
| The library's Number Field, controlled with the number 5 | "Server error" stays; does not send; on blur the text becomes `5` | the same |
| A plain `<input type="number">` with no library, a `0` typed in front of `5` (keys) | the text is `05`, its numeric value is 5, the input is valid | the same |

Not run: Firefox and WebKit; a real form library; a phone keyboard; a locale that types a comma for the decimal point.

## What the affected person sees

A server error shows on a number field. The person adds a leading zero or a trailing `.0` to the number. The edit appears in the input. At the head the error stays and Send does nothing until they make an edit that changes the number. At the base the error cleared and Send went through. Nothing on screen says why the error stayed.

With no error showing, the field is not marked as changed after the respelling, while the form would send `"05"` in place of `"5"`.

## Impact facts

- **Who is affected:** the end user of an app that controls a number-typed `Field.Control` with a number, stores a parsed number on each edit, and shows server errors through the form. For the dirty mark alone: the user or the developer of such an app that shows or reads the dirty mark.
- **How:** after a respelling, a server error stays and Send does not send; the field is not marked dirty while the form would send other text than at the start; a validator that runs on every change is not called for that edit.
- **How stuck:** one edit that changes the number away. Deleting the number and typing it again clears the error. They can then put the old number back and send it. The app's developer can store the text and parse it where the number is needed, or clear the app's own errors when told of an edit (*run*).
- **How common:** not measured. Three public programs found before the cut-off control a number-typed input of this library with a number; none shows server errors through the form in that file (*read*).
- **The other direction:** at the base the dirty mark of a field controlled with a number never cleared once set, even after the text returned to its initial value. The head clears it (*run*).
- **Neighbours:** the library's own Number Field keeps a server error on the same edit at both commits. An app that stores the text, and an input with no `value` prop, clear it at both commits. A keystroke that an app rejects outright, so that the input shows the old text again, also leaves a server error in place at the head and cleared it at the base (*run*).
