# C1nb: a deferred, rewritten or refused update is said to leave the input and the field's dirty and filled marks disagreeing

Pull request: mui/base-ui #5460, "[field] Sync controlled value changes with field state".
Candidates covered: none in the packet. The group comes from the comment of credit question C1: "If the consumer defers or transforms the update, field state lags or never updates." and "A consumer that debounces or wraps `setValue` in `startTransition`, or that rejects a keystroke, sees the input and `data-dirty`/`data-filled` diverge. Examples are `maxLength` filters that return the previous value, or a deferred state update."

Words used here. `Field.Control` is Base UI's text input; "controlled" means the app passes the text through `value` and stores each edit itself. `data-dirty` and `data-filled` are the field's marks for a changed and a non-empty value. "Input" below is the text the input shows. "Base" is `30b8ea2`, the commit before the change; "head" is `14d39e5`, the reviewed commit. The cut-off is the last push, 2026-08-10T13:33:57Z.

Each fact is marked *run* (executed for this dossier on 2026-10-17 (UTC); probe `probes/C1/probe.test.tsx`, cases C1-5 to C1-10, output in `probes/C1/`), *read* or *reported*.

**Already decided?** No saved ruling holds this claim. Second-pass ruling 30 and last-push ruling 5 ruled a suggestion that a keystroke the app refuses no longer reaches the field state (a server error stays). That is the behaviour this claim describes for a refused keystroke, from the other side: there the field state stays as the input stays; here the claim is that they disagree.

**Grouping.** One claim: in each named case, the input and the marks disagree, or the field state lags or never updates. The comment's sentence about `markedDirtyRef` is a separate claim with a different result, group C1nc.

## Problem

The claim: when an app delays its update (a debounced setter, `startTransition`), rewrites it, or refuses a keystroke (a `maxLength` filter that keeps the previous value), the field's state lags or never updates, and the input and `data-dirty`/`data-filled` disagree.

## What changed

For a controlled value the change handler now returns before touching the field state, and the field state follows the `value` prop from a block that runs after each render in which the prop changed (head `FieldControl.tsx` lines 105 to 115 and 142 to 146):

```diff
+          // Controlled values sync from the `value` prop instead, so that a value the consumer
+          // rejects or rewrites never reaches the field state.
+          if (isControlled) {
+            return;
+          }
```

At the base the handler set dirty and filled from the typed text and validated it, whatever the app then stored.

A controlled input in React shows the `value` prop: when the app does not store a keystroke in the same event, React puts the input back to the stored text (run, below: the input stays empty at both commits until the stored value changes).

**Runs**, base and head, jsdom and Chromium identical:

| Case | Moment | Base | Head |
| --- | --- | --- | --- |
| Setter delayed 30 ms, default mode, type `a` (C1-5) | right after | input empty; dirty and filled **on** | input empty; dirty and filled off |
| | after the delay | input `a`; on | input `a`; on |
| Setter delayed 30 ms, `onChange` mode, type `a` (C1-6) | right after | input empty; validator already called with `a` | input empty; not called |
| | after the delay | input `a`; called once | input `a`; called once with `a` |
| Setter in `startTransition`, `onChange` mode, raw input event (C1-7) | synchronously, and after a microtask | input empty; dirty and filled **on**; validator called with `a` | input empty; off; not called |
| | after 50 ms | input `a`; on | input `a`; on; called once with `a` |
| Digits-only setter, `x` into an empty field (C1-8) | after | input empty; dirty and filled **on** | input empty; off |
| Setter keeping the previous value past 3 characters, `onChange` mode, `abcd` after `abc` (C1-9) | after | input `abc`; validator called with `abcd` | input `abc`; not called again |
| Setter storing upper case, `onChange` mode, type `a` (C1-10) | after | input `A`; validator called with `a` | input `A`; validator called with `A` |

At the head the input and the marks agree at every moment observed. The field state updates when the input does, and does not update when the input does not. At the base they disagreed in every delayed or refused case, and the validator saw text the input never showed.

## Promised?

Not searched. The claim's own fact is false at the head (run), so rule Before 1 decides it before the two questions.

## Intended or announced

At the cut-off, the code comment above: "so that a value the consumer rejects or rewrites never reaches the field state" (read). Later, the description rewritten 24 seconds after the cut-off: "A controlled value the consumer rejects or rewrites no longer reaches the field state, matching Combobox, Number Field and OTP Field." (read, `upstream/pr-5460-usercontentedits.json`).

## What the affected person sees

At the head: in every case run, the marks match the text the input shows. At the base: a field whose keystroke was refused or not yet stored showed dirty and filled with an empty input.

## What the maintainers did

Nothing on this claim; the pull request had no review (the task packet; `upstream/pr-5460.json`).

## How each fact is known

- Every row of the table: **run** (`probes/C1/result-base.txt`, `result-head.txt`, `result-*-chromium.txt`).
- The early return and the block: **read** at the head.
- The comment's claim: **reported** (the saved comment).

Not run: Firefox and WebKit; a real keyboard; input method composition (where browsers can hold text in the input during composition); `useDeferredValue` on the app's side.

## Relation to existing reference families and ruled claims

- Not a known problem. Its subject, a refused or rewritten value and the field state, is the design the code comment announces and the ruled suggestion of second-pass ruling 30.
- GT-r10, compared under rule Before 4: it shares the block and the early return; it has no failing case here to run a fix against; no one sentence covers a double render and marks that disagree with the input.
- The control is `known-problems`, so no clean-control status is touched.

## Both sides

For the claim: the field state does lag the keystroke when the app delays its update, and never updates for a refused keystroke (run). Read as saying that the field state does not follow what the person typed, the sentence is true.

Against: the comment's stated result is that the input and the marks disagree. They agree at the head at every moment observed, and disagreed at the base. The input lags and stays exactly as the field state does.

## Recommendation

> **What happens:** with a delayed, rewritten or refused update, the input and the field's dirty and filled marks agree at the head at every moment observed; they disagreed at the base. **Promised: not asked**, the claim's fact is false. **Delivered: not asked.** **So: refuted.** **Band, decided separately:** none.

Recommendation code: `refuted`. Confidence: high. The claim's stated result was run in each case it names, and the opposite holds; rule Before 1 decides it.

**Strongest argument against the recommendation.** "field state lags or never updates" is true in the plain sense that the marks wait for the app's stored value and ignore a refused keystroke. On that reading the claim is a true description of the announced design, and its kind would be a suggestion, not refuted. The result it draws, that the input and the marks disagree, is what the runs refute.
