# C1nb: on a flip, a controlled field first renders the new text with the old dirty and filled marks

Pull request: mui/base-ui #5460, "[field] Sync controlled value changes with field state".
Candidates covered: none in the packet. The group comes from the comment of credit question `C1`, whose words describe it: "Dirty and filled state for controlled inputs now update one render late ... This causes ... a one-render window of stale state", and "The first keystroke can therefore render an inconsistent intermediate state."

Words used here are those of `dossiers/C1.md`. "Base" is `30b8ea2`, "head" is `14d39e5`, the cut-off is the last push, 2026-08-10T13:33:57Z. Each fact is marked *run* (2026-10-23 (UTC); `probes/C1nb/`, `probes/C1/`; jsdom and Chromium identical), *read* or *reported*. Every fact is knowable at the cut-off unless marked later.

**Already decided?** No. The nearest is `CL-r-flip-render`, ruled a suggestion in regrade-v10 18: "One more render of the field on each such keystroke; no value, mark or validation result differs." It covers the extra render, and its consequence speaks of the marks after the keystroke. It does not hold that app code can read the old marks beside the new text during the first of the two renders. No other ruled claim or known problem holds it (*read*).

**Grouping.** One thing: the state carried by the first render on a flip. Checked for the flips the probe types: the first character into an empty field, clearing it, and typing into it again (*run*). A flip of dirty alone (typing away from or back to a starting text) was not run in this probe. The same render in the modes that validate on change also carries the previous keystroke's validity. That part comes from GT-r10's place (fix A cures it, *run*) and is listed under the grouping tests, not counted here.

## Problem

Base UI lets an app draw a control's element itself through the `render` prop; the function gets the props and the component's state (dirty, filled, valid and the rest). Before the change, every render of a controlled `Field.Control` passed state that matched the text it held. After it, on a keystroke that turns dirty or filled on or off, the first render passes the new text with the old marks; a second render, before paint, passes the new marks. App code that runs after the first render and keys on the text sees the old marks and is not run again for that keystroke.

## What changed

The same lines as `C1na` (*read*, head `FieldControl.tsx`): the early return for a controlled value in the change handler (lines 144 to 146) and `setDirty` and `setFilled` in the block that runs in a layout effect after the app's render (lines 105 to 115, statements at 111 and 112). At the base the handler set the marks in the same event in which the app stored its text (base lines 117 to 122), so the one render had both.

**Runs** (`probes/C1nb/probe.test.tsx`). The app draws the input through the control's `render` prop with its own component, which reads the `state` argument. Entries are "text|D<dirty>F<filled>". Default mode, no validator:

| Keystroke | Base: renders | Head: renders | Base: effect on the text | Head: effect on the text | Both: effect on the marks | Both: attributes right after the event |
| --- | --- | --- | --- | --- | --- | --- |
| `a` into an empty field | `a`\|D1F1 | `a`\|D0F0, `a`\|D1F1 | `a`\|D1F1 | `a`\|D0F0 | `a`\|D1F1 | `a`, dirty and filled on |
| `ab` | `ab`\|D1F1 | `ab`\|D1F1 | `ab`\|D1F1 | `ab`\|D1F1 | not run | dirty and filled on |
| clear to empty | `''`\|D0F0 | `''`\|D1F1, `''`\|D0F0 | `''`\|D0F0 | `''`\|D1F1 | `''`\|D0F0 | dirty and filled off |
| `x` | `x`\|D1F1 | `x`\|D0F0, `x`\|D1F1 | `x`\|D1F1 | `x`\|D0F0 | `x`\|D1F1 | dirty and filled on |

- "Effect on the text" is a `useEffect` with `[value]` as its dependency list; a `useLayoutEffect` with the same list recorded the same at both commits (*run*).
- "Effect on the marks" is a `useEffect` keyed on `[dirty, filled]`; it recorded the same at both commits (*run*).
- An effect with no dependency list ran after both renders at the head and recorded both entries (*run*).
- The field's `data-dirty` and `data-filled`, read synchronously right after the change event returns and before the test yields to the browser, are the same at both commits (*run*). React states that a layout effect's updates are "processed **before the browser repaints the screen.**" (*read*).

### Grouping (rule Before 4)

**Against GT-r10.** GT-r10's place is line 114, `validation.change(serializedValue)`, in the same block; this group's is lines 111 and 112. Fixes as in `C1na` (`probes/C1nb/fix-*.patch`, run on the head). What the effect on the text saw (*run*, `probes/C1nb/table.txt`):

| Case | Base | Head | Fix A | Fix B | Both |
| --- | --- | --- | --- | --- | --- |
| This group: `a` into an empty field, default mode | `a`\|D1F1 | `a`\|D0F0 | `a`\|D0F0 | `a`\|D1F1 | `a`\|D1F1 |
| This group: clear to empty | `''`\|D0F0 | `''`\|D1F1 | `''`\|D1F1 | `''`\|D0F0 | `''`\|D0F0 |
| GT-r10's mode: `onChange`, validator rejects under 3 characters, type `abc` after `ab` (valid becomes true) | valid | not valid | valid | not valid | valid |

GT-r10's render counts under each fix are in `dossiers/C1na.md` (*run*): fix A cures them after the first keystroke, fix B does not.

- *Same lines:* yes. The same block and early return; different statements inside the block (111 and 112 here, 114 for GT-r10).
- *One fix:* no. Fix B cures this group's stale marks and none of GT-r10's; fix A cures GT-r10's renders and stale validity and none of this group's. Both together cure both (*run*).
- *One sentence:* yes. "For a controlled value the field's bookkeeping runs in a layout effect after the app's render, so the render before it carries the new text with the old field state."

**Against `C1na` (the extra render on a flip, a ruled suggestion).** Same lines, the same smallest fix (fix B cures both, *run*), and one sentence true of both: "on a flip the field renders once with the new text and the old marks, then again with the new marks." The two are the first and the second render of the same keystroke.

Fix B's side effect (*run*, case N3): an app that refuses anything but digits gets dirty and filled turned on by the refused `x`, as at the base; at the head they stay off.

## Promised?

**The exact operation.** The `state` argument Base UI passes to a control's `render` function during the render in which a controlled control first holds the new text, read by the app's code after that render. Base UI owns the `render` prop and its `state`; React owns when a layout effect's update is rendered and when effects run.

**How the owners classify it.** The `render` prop and its state argument are public and documented. Nothing states the state's timing relative to the text between two renders of one keystroke (unstated).

**The six places searched**, each knowable at the cut-off:

1. **Project documentation at the head** (`upstream/C1nb-project-docs-search.txt`: four searches of the Field, Form and Input pages and the handbook for the marks, controlled use, words for staleness, and the render function's state; 47 lines, all read).
   - The marks: "Present when the field's value has changed." and "Present when the field is filled." (`components/field/types.md`, for each part).
   - The render prop: "`ReactElement \| ((props: HTMLProps, state: Field.Control.State) => ReactElement)`" (`components/field/types.md:184`).
   - The handbook (`handbook/composition/page.mdx`, "Render function"): "Using a function gives you complete control over spreading props and also allows you to render different content based on the component's state." The example renders content from `state.checked`.
   - No hit for "in sync", "out of sync", "stale", "intermediate" or "consistent". Nothing about reading the state in an effect.
2. **React's documentation** (`upstream/react-useLayoutEffect.md`, `upstream/react-useEffect.md`, each at its last commit before the cut-off; `upstream/owner-docs-search.txt`, 14 lines, all read).
   - "React guarantees that the code inside `useLayoutEffect` and any state updates scheduled inside it will be processed **before the browser repaints the screen.** This lets you render the tooltip, measure it, and re-render the tooltip again without the user noticing the first extra render."
   - "If your Effect is caused by an interaction (like a click), **React may run your Effect before the browser paints the updated screen**."
   - React presents a render followed by a corrective render from a layout effect as invisible to the user. It says nothing about effects observing the first.
3. **The change** (`upstream/change-search.txt`, 24 lines, all read). The description at the cut-off says of code-set values: "updated the input text but not filled, dirty, or validity, so a resolved error stayed visible". It says nothing about intermediate renders. The code comment: "Controlled values sync from the `value` prop instead, so that a value the consumer rejects or rewrites never reaches the field state." No test reads the state in an effect (*read*, the diff).
4. **Maintainers up to the cut-off** (`upstream/maintainers-searches.txt`, the six `[C1nb]` queries; 62 results, 47 distinct; titles of all read, and #4685 and #5345 opened, the two whose titles concern when field state updates). #4685 (atomiks, 2026-04-26) is about removing `flushSync` from validation; #5345 about a stuck focus mark. Neither speaks of state between two renders of one keystroke.
5. **Public code** (`upstream/C1nb-public-code-*.json`, three queries, 25 hits; `upstream/C1nb-code-read.txt`, all 25 read). Every program found reads the field state during render, for class names and attributes. One, `vijayksingh/metalui` (path first committed 2026-09-30, after the cut-off), reads `state.disabled` in a layout effect keyed on that value. None reads dirty, filled or validity in an effect keyed on the text.
6. **The documented way, run at the head.** Rendering content from the state, as the handbook shows: the content and attributes on screen are the same at both commits after each keystroke (*run*, the attributes right after the event; *read*, React's guarantee). An effect keyed on the marks sees the same values at both commits (*run*).

**What the code and the tests deliberately support.** No test under `packages/react/src/field` or `packages/react/src/input` at the head uses `useEffect` or `useLayoutEffect`, or a `render={(props, state)` function (*read*: `rg -l 'useEffect|useLayoutEffect'` and `rg -n 'render=\{\(props, state\)'` over their `*.test.tsx` files, no hits).

**Whether it worked before, and whose behaviour changed.** At the base each render's state matched its text (*run*). Base UI's code changed; React did not.

**Which of the three ways.** None found. Written: the render function is documented for rendering "different content based on the component's state", which still works. Announced: nothing. Built: no test of the field or the input reads the state in an effect. Rule Promised 7a: "It used to work", for a use that nothing supports, is not a promise. Rule Promised 8a: something goes wrong for an app whose effect keys on the text and reads the marks; the owners said nothing; no program was found doing it. That is "outside supported use".

## Intended or announced

At the cut-off: the code comment states the design (marks follow the stored value). The test's comment shows the author knew the first keystroke flips the marks. Nothing speaks of the first render's state (*read*).

Later: the author's rewritten description (2026-08-10T13:34:21Z) speaks of render counts only. Shipped in v1.8.0 (2026-09-04). On `master` at `a212e6b` (2026-10-23) dirty is still set in the block, and filled in a separate layout effect added by #5563 (`upstream/master-FieldControl.tsx`, `upstream/later-pr-5563.json`). Both still run after the app's render. That was not run.

## What the affected person sees

- A person typing sees nothing different (*run*: the attributes after each keystroke; *read*: React's guarantee).
- An app developer whose component, drawn through the control's `render` prop, runs an effect when the text changes and reads dirty or filled in it gets the old marks for a flip keystroke, for example dirty off for the first character typed. That effect does not run again for the keystroke, so whatever it stored or sent stays stale until the next keystroke that changes the text (*run*). Effects keyed on the marks, or with no dependency list, see the right values (*run*).

## What the maintainers did

- Up to the cut-off: a draft with no review (`upstream/pr-5460-reviews.json`, `upstream/pr-5460-review-comments.json`: empty).
- Later: merged 2026-08-13 with no review. No later issue or pull request found about state in the first render; #5563 changed how filled is set and keeps it in a layout effect (*read*).
- No maintainer acknowledged it.

## How each fact is known

- What renders and effects saw at both commits and under each fix; the attributes after the event; fix B's side effect: **run** (`probes/C1nb/`, `probes/C1/`).
- The block, the early return, the base handler: **read**.
- Base UI's documentation, React's documentation, the change, the maintainers' threads, the public code, later history: **read** (`upstream/`).

Not run: a production build of React; Firefox and WebKit; a real application with such an effect; `master`.

## Relation to existing reference families and ruled claims

- `C1na` and its ruled claim `CL-r-flip-render` (regrade-v10 18, suggestion): the second render of the same keystroke, with the same lines and fix. If the owner sees this as the same thing, the ruling decides it.
- **GT-r10**: same block, a different statement; the three tests are above.
- GT-r2 (filled at mount) and GT-r11 (a control enabled in the render that sets its value): marks that end wrong. Here the marks end right (*run*).
- Second-pass ruling 30 and last-push ruling 5 rest on the code comment that a refused value "never reaches the field state". Fix B undoes that (*run*).
- The control is `known-problems`, so no clean-control status is touched.

## Both sides

For a problem:

- At the base every render's state matched its text; the documented `render` function receives "the component's state", and at the head that state is wrong for one render on the commonest keystroke a form sees.
- App code can act on it: an effect keyed on the text stores the wrong marks and is not run again (*run*).

For a suggestion:

- The documented use of the state, rendering content from it, gets the right result on screen (*run*, *read*). React documents the render-then-correct pattern of a layout effect as unseen.
- No program found reads dirty, filled or validity in an effect keyed on the text; the one that reads the state in an effect keys it on the state, which works (*read*, *run*).
- It is the content of the extra render the owner already ruled a suggestion; the smallest fix is the same, and it undoes what the code comment says the design is for.

## Recommendation

> **What happens:** on a keystroke that turns dirty or filled on or off, a controlled `Field.Control` first renders the new text with the old marks, and an app effect keyed on the text sees that pair and is not run again; the screen and the marks after the keystroke are the same as before. **Promised: no**, the `render` function is documented for rendering content from the state, which still works; nothing written, announced or built covers reading the state in an effect keyed on the text; six places searched, and none of 25 public programs does it. **Delivered: not asked.** **So: suggestion or observation (outside supported use).** **Band, decided separately:** none.

Recommendation code: `suggestion`. Confidence: medium. It falls short of high on one condition: no earlier ruling of the same shape (`no-precedent`). No ruling covers state that is wrong only between two renders of one keystroke.

The fact that would reverse it: a program, before the cut-off, that reads dirty, filled or validity in an effect keyed on the text. That would make it "relied on, not promised", which goes to the owner. The search found none among 25 hits.

**Strongest argument against.** The `render` function's state is documented as the component's state, and at the base it was always right. A developer has no way to know that one render in a flip carries old marks. If the owner reads "the component's state" as a promise for each render, the stale pair is not delivered.
