# Ruling 21 supplement: is `cancel()` on Field.Control documented, expected and used?

Added by the recording session on 2026-10-05 during review 5 of the seven, after the user asked: "is: After `cancel()`, the person types "a" (run) with still yes on Field marked dirty and filled a common use case, or is it a rare use case? Is it a documented supported use case that should end in the field not marked dirty and filled, is that the expectation a user could reasonably have? If so, it's a problem." Every fact below was read at the pinned commits or fetched in that session.

## What the documentation says

Read, the customization handbook at the head (`handbook-customization-head.mdx`; the page is identical at the commit before the change):

- "`cancel` stops the component from changing its internal state."
- "This lets you leave the component uncontrolled as its internal state is prevented from updating. This is an alternative to controlling the component with external state and guarding the state updates conditionally."
- Its example cancels a tooltip's `onOpenChange`. The drawer page has another `onOpenChange` example. No page shows `cancel()` on a field.

Read, the Field reference at both commits (`docs/src/app/(docs)/react/components/field/types.md`):

- `Field.Control.ChangeEventDetails` lists `cancel` with "Cancels Base UI from handling the event." at both commits.
- `onValueChange` on `Field.Control`: "Callback fired when the `value` changes. Use when controlled."
- `data-dirty`: "Present when the field's value has changed." `data-filled`: "Present when the field is filled."

## What was true before the change

Run (first-round probes B6 and B1): before the change `Field.Control` never read `isCanceled`. Calling `cancel()` did nothing. Nobody can have relied on it working for a field. The pull request is what makes it work, and its description says "`details.cancel()` in `onValueChange` now stops the internal handling."

## Whether anyone does it

Fetched 2026-10-05 (`search-field-control-cancel.json`): a GitHub code search for TSX files containing `Field.Control`, `onValueChange` and `cancel()` returns 23 files. Nineteen are Base UI's own tests or copies of them in forks and ports, and this repository's probes. Four are applications; read one by one, none calls `cancel()` on a field's `onValueChange` (one cancels a dialog's `onOpenChange`, three do not call `cancel()` at all). A second search for the field package with `isCanceled` or `cancel()` (`search-base-ui-field-cancel.json`) returns three files, none an application doing this. A code search can miss private code and differently written calls.

## Reading

- Documented: yes, as a general contract for every component's event details, and specifically promised for `Field.Control` by this pull request.
- A reasonable expectation: by the handbook's words, yes. Dirty and filled are the field's internal state, and `cancel` "stops the component from changing its internal state." Against it: an uncontrolled text input cannot take back what was typed, so the box shows the new text, and marks that did not update would contradict it.
- Common: no. No application is shown doing it, and it could not have been a practice before this change.
- The controlled case differs: the handbook calls `cancel()` "an alternative to controlling the component", and an application that cancels and still stores the new value is asking for both.
