# C1na: a controlled field renders once more on each keystroke that turns its dirty or filled mark on or off

Pull request: mui/base-ui #5460, "[field] Sync controlled value changes with field state".
Candidates covered: none in the packet. The group comes from the comment of credit question C1, whose words describe it: "They are set in a layout effect after the parent re-renders, which costs an extra render per transition."

Words used here. `Field.Control` is Base UI's text input for a form field, inside a `Field.Root` that holds the field's state. It is "controlled" when the app passes the text in through the `value` prop and stores each edit itself. The field marks itself dirty (`data-dirty`) when its value differs from the one it started with, and filled (`data-filled`) when it is not empty. A "render" is one run of a component's code by React. A "flip" is a keystroke after which dirty or filled differs from before it: the first character typed into an empty field, clearing the field, or typing away from or back to the starting text. "Base" is the commit before the change, `30b8ea2`. "Head" is the reviewed commit, `14d39e5`. The cut-off is the last push, 2026-08-10T13:33:57Z.

Each fact is marked *run* (executed for this dossier on 2026-10-17 (UTC), probes and output saved under `probes/C1na/` and `probes/C1/`), *read* or *reported*. Every fact is knowable at the cut-off unless marked later.

**Already decided?** No. No saved ruling, ruled claim or known problem holds an extra render on a flip in the default mode. The nearest is GT-r10 (regrade ruling 6, claim `CL-r-second-render-per-keystroke`): two renders per keystroke whenever validation runs on change. Its trigger counts "Each keystroke after the first" and says "In the default mode before any submit ... it renders once at both commits", so it does not hold this case. Whether this is part of GT-r10 is a grouping question for the owner (below).

**Grouping.** One thing: one extra render on each flip. Checked in every kind of flip run: dirty and filled together, filled alone with dirty, dirty alone (run).

## Problem

Before the change a controlled `Field.Control` rendered once for each keystroke in the default mode. After it, a keystroke that flips dirty or filled renders the control twice; other keystrokes still render once. Nothing differs on screen. The second render happens before the browser paints.

## What changed

The change moves a controlled field's bookkeeping out of the change handler into a block that runs after React has rendered the new value (head, `FieldControl.tsx`):

```diff
+  useValueChanged(serializedValue, () => {          // lines 105 to 115: runs in a layout effect
+    if (serializedValue === undefined) {
+      return;
+    }
+
+    clearErrors(name);
+    setDirty(serializedValue !== (validityData.initialValue ?? ''));   // line 111
+    setFilled(serializedValue !== '');                                // line 112
+
+    validation.change(serializedValue);
+  });
 ...
+          // Controlled values sync from the `value` prop instead, so that a value the consumer
+          // rejects or rewrites never reaches the field state.
+          if (isControlled) {                       // lines 144 to 146
+            return;
+          }
```

Before, the handler called `setDirty` and `setFilled` inside the change event in which the app stores the new text, and React rendered once for both. Now the app's update renders first; the block then runs as a layout effect, and when dirty or filled changes, React renders the field again before painting (read: `internals/useValueChanged.ts` calls its callback from `useIsoLayoutEffect`; React's page: "React guarantees that the code inside `useLayoutEffect` and any state updates scheduled inside it will be processed **before the browser repaints the screen.**"). When neither changes, React skips the update and there is no second render.

**Runs** (`probes/C1na/probe.test.tsx`; the counter is the change's own: a `render` prop on the control; jsdom and Chromium identical). Renders of the control per keystroke, default mode:

| Keystroke | What flips | Base | Head |
| --- | --- | --- | --- |
| `a` into an empty field | dirty and filled on | 1 | **2** |
| `ab`, `abc` | nothing | 1 | 1 |
| clear to empty | dirty and filled off | 1 | **2** |
| `x` | dirty and filled on | 1 | **2** |
| starting text `abc`, type `abcd` | dirty on | 1 | **2** |
| `abcde` | nothing | 1 | 1 |
| back to `abc` | dirty off | 1 | **2** |
| `ab` | dirty on | 1 | **2** |

The dirty and filled marks after each keystroke are the same at both commits. No validator runs in this mode. An uncontrolled control renders once on a flip and not at all on other keystrokes, at both commits (run, `probes/C1/` case C1-4).

### Grouping with GT-r10 (rule Before 4)

**The two places.**

- GT-r10's place: line 114, `validation.change(serializedValue)` inside the block, together with the early return at lines 144 to 146. When the field validates on change, validation stores a new result object from the layout effect and the control renders again on every keystroke (read, the answer key; `useFieldValidation.ts` line 313).
- This group's place: lines 111 and 112, `setDirty` and `setFilled` inside the same block, together with the same early return. On a flip they store a changed mark from the layout effect and the control renders again.

**The smallest fix for each, and both** (patches saved in `probes/C1na/`; all run on the head):

- `fix-A-gt-r10-place.patch`: for a controlled keystroke, run `clearErrors` and `validation.change` in the change handler and skip them in the block for that value.
- `fix-B-flip-place.patch`: for a controlled keystroke, set dirty and filled in the change handler and skip them in the block for that value.
- `fix-A-and-B.patch`: both.

Renders per keystroke (`probes/C1na/table.txt`; "first" is the first keystroke into an empty field, a flip):

| Case | Base | Head | Fix A | Fix B | Both |
| --- | --- | --- | --- | --- | --- |
| This group: default mode, each flip | 1 | 2 | 2 | **1** | **1** |
| This group: default mode, keystrokes without a flip | 1 | 1 | 1 | 1 | 1 |
| GT-r10: `onChange` mode, validator accepts, first / later | 1 / 1 | 2 / 2 | 2 / **1** | 2 / 2 | **1** / **1** |
| GT-r10: `onChange` mode, validator rejects, first / later | 2 / 1 | 3 / 2 | **2** / **1** | 3 / 2 | **2** / **1** |
| GT-r10: default mode after a submit attempt, later keystrokes | 1 | 2 | **1** | 2 | **1** |
| GT-r10: `Field.Root invalid`, first / later | 1 / 1 | 2 / 2 | 2 / **1** | 2 / 2 | **1** / **1** |
| GT-r10: `onBlur` mode, first keystroke on a field showing an error | 2 | 3 | **2** | 3 | **2** |

So fix A cures GT-r10's keystrokes after the first and none of this group's; fix B cures this group's and none of GT-r10's; both together give the base's count in every case. A fix that moves all of the controlled bookkeeping back into the handler is one change that cures both.

**Fix B has a side effect** (run, `probes/C1/result-fix-B-head.txt`): with a setter that refuses anything but digits, typing `x` into an empty field leaves the input empty and turns dirty and filled on, as at the base. The change's code comment says it moved the work out of the handler "so that a value the consumer rejects or rewrites never reaches the field state". Fix A has the same kind of effect for validation: the handler validates the typed text before the app has accepted it.

**The three tests**, as facts:

- *Same lines:* both come from the same block, lines 105 to 115, run as a layout effect, and the same early return at 144 to 146. Inside the block they come from different statements: lines 111 and 112 for this group, line 114 for GT-r10.
- *One fix:* no smallest fix cures both. Fix A cures only GT-r10's cases after the first keystroke; fix B cures only this group's. Both patches together, or one rewrite of the block, cure both.
- *One sentence:* "For a controlled value the field's bookkeeping runs in a layout effect after the app's render, so each piece of field state it changes there renders the control a second time for the same keystroke." True of both (run).

## Promised?

**The exact operation.** How many times Base UI's `Field.Control` renders for a keystroke of a controlled value that turns the field's dirty or filled mark on or off. The component and its `value` and `onValueChange` props are Base UI's, public and documented. That a state update made in a layout effect is rendered at once, before paint, is React's.

**How the owners classify it.** A controlled control is a documented use. No document states a number of renders.

**The six places searched.** Each is knowable at the cut-off.

1. **Project documentation at the head** (`upstream/C1na-project-docs-search.txt`: six searches of the Field, Form and Input pages, the forms handbook, the rest of the handbook and the overview; 37 matching lines, all read).
   - The use is documented: "Callback fired when the `value` changes. Use when controlled." (`components/field/types.md:181`, the `Field.Control` table), and the forms handbook shows a controlled `Field.Control` with `value={field.state.value}` and `onValueChange={field.handleChange}` in its TanStack Form demo (`handbook/forms/demos/tanstack-form/tailwind/index.tsx`, lines 102 to 104).
   - The marks are documented: "Present when the field's value has changed." and "Present when the field is filled." (`components/field/types.md`).
   - Nothing says how often a control renders. The one line on performance is about something else: "If you are working in an extremely performance-sensitive application, you might want to pass a function to the `render` prop instead of a React element." (`handbook/composition/page.mdx:82`). The release notes list "Prevent re-renders when `Field.Control` is uncontrolled ([#3820](https://github.com/mui/base-ui/pull/3820))" (`overview/releases/v1-2-0/page.mdx:71`).
2. **React's documentation** (`upstream/C1na-owner-docs-react-useLayoutEffect.md`, the page at react.dev commit `b4610fb` of 2026-06-22, the last change before the cut-off; `upstream/C1na-owner-docs-search.txt`, 11 matching lines, all read). "`useLayoutEffect` can hurt performance. Prefer [`useEffect`](/reference/react/useEffect) when possible." and "The code inside `useLayoutEffect` and all state updates scheduled from it **block the browser from repainting the screen.** When used excessively, this makes your app slow." React states no limit on renders.
3. **The change itself** (`upstream/pr-5460-usercontentedits.json`, the description's edit history; `upstream/pr-5460-base-to-head.diff`; `upstream/C1na-change-search.txt`, 27 matching lines of the diff, all read).
   - The description at the cut-off, as the task packet shows it, says nothing about renders. Its one sentence on the keystroke path describes another design: "`processedValueRef` records the value the change handler just handled, so the prop echo does not validate the same keystroke twice."
   - The test the change adds is titled "renders once per keystroke for controlled input changes". Inside it: "// The first keystroke also flips dirty and filled, so measure the steady state after it." and, after two more keystrokes, "// The controlled echo must not schedule a second render per keystroke." The test counts from after the first keystroke, a flip, and its later keystrokes flip nothing.
   - The code comment on the early return gives the design's reason: "Controlled values sync from the `value` prop instead, so that a value the consumer rejects or rewrites never reaches the field state."
4. **Maintainers up to the cut-off** (`upstream/C1na-maintainers-searches.txt`: five searches of mui/base-ui limited to `created:<2026-08-10T13:33:57Z`, 6 distinct hits, all fetched with their comments and read).
   - Issue #3819 (2026-01-21), a user: "Base UI Input (which renders Field.Control) triggers React state updates on each keystroke (setDirty, setFilled), causing the input component/subtree to re-render continuously while typing." atomiks, who wrote and merged the fix and wrote this pull request, answered: "Is your main concern that `<Field.Control>` was  re-rendering on keystroke, not necessarily when `filled`/`dirty` change?  As those two states are relatively cheap as they only happen "once"/infrequently enough to likely not cause an issue vs. every keystroke". Later: "So for an initially empty input, `filled`/`dirty` change together (batched re-render) and no extra re-renders occur while typing afterwards until the input is cleared again". The user answered: "This re-renders of focus and first typing are acceptable or negligible."
   - Pull request #3820 fixed the per-keystroke renders of an uncontrolled control; the release notes list it.
   - #4481, #5148, #3342 and #3711 were read and say nothing about a field control's renders on a flip.
5. **Public code** (`upstream/C1na-public-code-controlled-field-control.json`: one code search, `"Field.Control" "onValueChange" "value=" language:TSX`, 984 hits, 100 returned; `upstream/C1na-code-read.txt` and `upstream/C1na-code-files/`, 3 read).
   - Base UI's own forms handbook demo, in the repository since 2025-12-12: a controlled `Field.Control` driven by TanStack Form.
   - `gitbutlerapp/gitbutler`, `apps/lite/ui/src/AskpassPromptDialog.tsx` (path first committed 2026-06-10; the file read is the current one): `<Field.Control ... value={currentResponse} onValueChange={(value) => setResponse({ promptId: currentPrompt.id, value })} />`.
   - `pingdotgg/t3code`, `ProviderSettingsForm.tsx`: the matching `onValueChange` is a `Select`'s; no controlled `Field.Control` in the lines read.
   - So a controlled control is something people build. None is shown measuring or depending on the number of renders.
6. **The documented way to do the same thing, run at the head** (`probes/C1/result-head.txt`, case C1-4). An uncontrolled control renders once on each flip and not at all on other keystrokes, at both commits.

**What the code and the project's tests deliberately support.** The uncontrolled test, "avoids rerendering for uncontrolled input changes", allows one render on the first change (`expect(afterFirstChange).toBeLessThanOrEqual(initialRenderCount + 1)`). The new controlled test sets the flip keystroke aside and counts one render per later keystroke.

**Whether it worked before, and whose behaviour changed.** One render on a flip at the base; two at the head. Base UI's code changed. React did not.

**Which of the three ways.**

- Written: the use (a controlled control and its marks) is documented. The number of renders is not.
- Announced: the change's test title says "renders once per keystroke for controlled input changes"; the comment inside the same test sets the flip keystroke aside: "The first keystroke also flips dirty and filled, so measure the steady state after it." No rule says how a test's title and its own comment are read together.
- Built: one render on a flip was how this documented use behaved before the change (rule Promised 7a). Whether that cost is part of the promise is rule Promised 7b: it counts when it "was not an intended tradeoff and can be potentially mitigated or prevented". The smallest prevention found (fix B) lets a refused keystroke reach the field state again, which the change's code comment says the design exists to stop (run).

The answer proposed is **no**: nothing written or announced promises one render on a flip, the test the change adds sets the flip keystroke aside in words, the one maintainer statement on renders at a flip calls them "relatively cheap" and infrequent, and the cost is one render on an infrequent keystroke (rule Promised 8a, improvement, beside "A cleanup that now waits a few microseconds for a lock"). Rule 7b's line is open, and the test's title points the other way; see "Where the rules do not decide".

## Intended or announced

**Knowable at the cut-off.** The test's comment shows the author knew the first keystroke also flips dirty and filled and measured around it. Nothing says the extra render is accepted or avoided.

**Later.**

- 2026-08-10T13:34:21Z, 24 seconds after the cut-off: the author rewrote the description (`upstream/pr-5460-usercontentedits.json`). Its "Performance" table gives, for `onSubmit` mode: "1 per keystroke" at the base, "2 on the first, then 1" for the pull request and "2 on the first, then 1" for Number Field. That confirms the author knew of this cost and compared it with a sibling control. Under rules Before 3 and Promised 2c it is not a reason for an answer.
- Shipped in v1.8.0 on 2026-09-04; the release note is the pull request's title (`upstream/releases.json`).
- Upstream `master` as fetched on 2026-10-17 still sets dirty in the block (`upstream/master-FieldControl.tsx`, lines 96 to 105) and moved filled into a separate layout effect (lines 89 to 94).

## What the affected person sees

Nothing. A person typing into a controlled field gets the same text and the same marks as before, and the second render lands before the screen updates (run: marks identical at both commits; read: React's guarantee).

An app developer who counts renders, or whose field has many parts, sees the field render twice on the first keystroke into an empty field, on clearing it, and on typing away from or back to the starting text. The time of that render was not measured here; an earlier round measured an extra render of a similar field at about 0.03 ms in development mode (**reported**, `docs/research/regrade-rulings-2026-10-13/r-base-ui-5460/dossiers/M1.md`).

## What the maintainers did

- Up to the cut-off: the pull request was a draft with no review (the task packet). The author's statement in #3819 is above.
- Later: the author merged the pull request on 2026-08-13 with no review (`upstream/pr-5460.json`). Searches of later issues and pull requests (`upstream/later-*.json`, 8 hits, titles read) found none about renders on a flip.
- No maintainer called it a fault.

## How each fact is known

- Render counts per keystroke at the base and the head, in jsdom and Chromium: **run** (`probes/C1na/`, `probes/C1/`).
- The effect of each smallest fix: **run** (`probes/C1na/result-fix-*-head.txt`, `table.txt`).
- The side effect of fix B on a refused keystroke: **run** (`probes/C1/result-fix-B-head.txt`).
- The block, the early return, the layout effect: **read** at the head.
- The documentation, React's page, the maintainers' threads, the public code read, the description's edit history, the release, upstream master: **read** (`upstream/`).
- The time of one extra render: **reported** (earlier round), not run here.

Not run: a production build; Firefox and WebKit; a real application; a time measurement.

## Relation to existing reference families and ruled claims

- **GT-r10** is the neighbour. Same block, same early return, the same kind of harm (one more render), a different trigger (a flip in any mode, against every keystroke when validation runs on change) and a different statement inside the block (lines 111 to 112, against 114). The three tests are above. If the owner joins this to GT-r10, its wording widens to "and on a keystroke that turns dirty or filled on or off", and the comment of credit question C1 earns credit for GT-r10 under rule Before 4.
- GT-r1 to GT-r9 and GT-r11 concern what the field state holds or shows. Here it holds and shows the same at both commits (run).
- Second-pass ruling 30 and last-push ruling 5 ruled a suggestion that a refused keystroke no longer reaches the field state, on the code comment quoted above. The smallest fix for this group undoes that design (run).
- The control is `known-problems`, so no clean-control status is touched.

## Both sides

For a problem:

- The change's own test is titled "renders once per keystroke for controlled input changes", and a flip is a keystroke. The owner made GT-r10 a problem "because the test for it in the PR says it's meant to render once per keystroke and it doesn't do that" (regrade ruling 6).
- A documented use costs one render more than before on these keystrokes, and both fixes together show it can be avoided (run).
- Nothing readable at the cut-off says the extra render is accepted.

For a minor defect:

- Every outcome arrives; work is done twice on those keystrokes.

For a suggestion:

- The same test says in its own comment that the first keystroke "also flips dirty and filled" and measures after it.
- The author, in the project's tracker before the cut-off, called renders on a dirty or filled change "relatively cheap" and infrequent "vs. every keystroke".
- The cost is one render on a few keystrokes of an editing session, before paint, invisible.
- The smallest fix for it brings back what the change's code comment says the design removes: a refused keystroke reaching the field state.

## Recommendation

> **What happens:** a controlled `Field.Control` renders twice, where it rendered once before, on each keystroke that turns its dirty or filled mark on or off; other keystrokes render once and the screen is the same. **Promised: no**, the documentation states no render count, the change's test sets the flip keystroke aside ("The first keystroke also flips dirty and filled, so measure the steady state after it."), and the author called renders on a dirty or filled change "relatively cheap" before the cut-off (issue #3819); six places searched. **Delivered: not asked.** **So: suggestion or observation (improvement).** **Band, decided separately:** none.

Recommendation code: `suggestion`. Confidence: low. It falls short of high on two conditions: the rule's words do not settle the size of this cost (`gap`), and a second rule points the other way (`conflict`).

**If the owner rules it a problem, or joins it to GT-r10,** the band I would propose is other-material, as GT-r10's: nobody is stuck, nothing is lost or misreported, and the extra work is one render on an infrequent keystroke.

**Where the rules do not decide.**

- **The size of the cost.** Rule Promised 7b: "An agent that meets a cost in between says so, and the user decides." One extra render per flip sits near the improvement example, a few microseconds for a lock, but it is a render of the field and not a wait.
- **The test's title against its own comment.** Rule Promised 5 speaks of "the change's description or a code comment". The owner treated the test's title as the change's statement in regrade ruling 6. Read alone, the title covers every keystroke, the first included; read with the comment beside it, it covers keystrokes after the flip. No ruling reads the two together.
- **Grouping.** The three tests split: same block, no single smallest fix, one sentence true of both. That is the owner's under rule Before 4.

**Strongest argument against the recommendation.** The test the change adds is titled "renders once per keystroke for controlled input changes", the owner held the change to that title in regrade ruling 6, and the first keystroke into an empty field is the most common keystroke a form sees. On that reading this is GT-r10's broken promise on another keystroke, a problem, and most naturally part of GT-r10.
