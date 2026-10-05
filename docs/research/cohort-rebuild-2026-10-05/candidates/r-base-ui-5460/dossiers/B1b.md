# B1b: `details.cancel()` is not consulted in a controlled Field.Control

Pull request: mui/base-ui #5460, "[field] Sync controlled value changes with field state".
Candidates covered: NC-37b7c968b098, NC-827646914110, NC-8aa4810db277 (each also asserts B1a).

Split out of group B1. B1a is about a prevented browser event. This one is about the app calling `details.cancel()`.

## Problem

`Field.Control` calls the app's `onValueChange(value, details)` on every edit. `details.cancel()` is documented as "Cancels Base UI from handling the event."

The pull request description says "`details.cancel()` in `onValueChange` now stops the internal handling. It was ignored." That is true only for an uncontrolled field. In a controlled field, if the app calls `cancel()` and still stores the new value, the field clears the server error, updates dirty and filled, and validates, exactly as if `cancel()` had not been called.

## What changed

The new cancel check sits below an early exit that controlled fields always take:

```diff
+          const details = createChangeEventDetails(REASONS.none, event.nativeEvent);
+          onValueChange?.(inputValue, details);
+
+          // Controlled values sync from the `value` prop instead, so that a value the consumer
+          // rejects or rewrites never reaches the field state.
+          if (isControlled) {
+            return;
+          }
 ...
-          if (!event.nativeEvent.defaultPrevented) {
+          if (!event.nativeEvent.defaultPrevented && !details.isCanceled) {
             clearErrors(name);
             validation.change(inputValue);
           }
```

Controlled field state is updated by a new block that runs when the `value` prop changes. That block does not know about `details`. So whether the field reacts depends only on whether the app stored the value.

Before the change, `details.isCanceled` was never read in either mode.

## Intended or announced

Announced as a fix, in the description: "`details.cancel()` in `onValueChange` now stops the internal handling. It was ignored." The same description says controlled fields are driven only by the `value` prop: "A controlled value the consumer rejects or rewrites no longer reaches the field state". The two statements are not reconciled. The new test "does not validate when the change is canceled" uses an uncontrolled field only.

Shipped in v1.8.0 on 2026-09-04. The release note does not mention `cancel()`.

## What the affected person sees

The probe uses a controlled field in a `Form` with a server error and `validationMode="onChange"`.

App calls `cancel()` and stores the value:

- Before the change: validator called with `"a"`, server error cleared, field dirty and filled.
- After the change: exactly the same.

App calls `cancel()` and does not store the value:

- Before the change: validator still called with `"a"`, server error cleared, field marked dirty and filled, although the input stayed empty. That was wrong.
- After the change: nothing happens. Validator not called, server error stays, field not dirty, not filled.

So nothing got worse. The second case got better. What remains is that `cancel()` by itself has no effect in controlled mode. An app that wants to stop the field from reacting must not store the value. No end user is stuck.

## What the maintainers did

- No maintainer has mentioned this. The pull request had no human review.
- Upstream master on 2026-10-05 has the same code. The probe gives the same result there.
- No later issue or pull request about `cancel()` in `Field.Control` was found.

## How each fact is known

Runs used the project's own test setup. Unless a line says Chromium, they ran in jsdom, the simulated browser that setup uses.

- Both controlled cancel cases at base, head and master: **run** (`probes/B1/probe.test.tsx`, cases `B1b-controlled-cancel-and-store` and `B1b-ref-controlled-cancel-and-reject`; `result-base.txt`, `result-head.txt`, `result-master.txt`).
- The diff lines, the uncontrolled-only test, the `cancel` documentation text: **read** (pinned diff; `FieldControl.test.tsx` and `createBaseUIEventDetails.ts` at head).
- The description's two statements: **read** (`bench/targets/r-base-ui-5460/packet.md`, `upstream/pr-5460.json`).
- Release and review facts: **read** (`upstream/releases.json`, `pr-5460-reviews.json`, `pr-5460-timeline.json`).
- That any app both cancels and stores a value: not established by anyone.

## Relation to existing reference bugs and ruled claims

Not GT-r1 and not GT-r2. No ruled claims exist for this pull request.

B1b and B6 are two halves of one shortfall: the new promise that `cancel()` "stops the internal handling" is kept only in part. B6 is the uncontrolled half (dirty and filled still update). They sit on different code paths and would be fixed separately, but they share one promise and one reading of it. If the owner rules on one, the same reasoning applies to the other.

## Both sides

For calling it a bug:

- The description states a behaviour that does not hold in controlled mode, which is the mode the documentation points `onValueChange` at ("Use when controlled").
- A wrapper component that forwards `onValueChange`, stores the value, and lets a parent call `cancel()` gets no effect from the cancel.

Against:

- Nothing regressed. Cancel-and-store behaves identically before and after (run).
- The realistic way to reject an edit in controlled mode is to not store it, and that now works where it did not before (run).
- Calling `cancel()` and storing the value is self-contradictory. The field then really holds the new value. Keeping its validity for the old value would leave it stale, which is what the pull request set out to remove.

## Recommendation

`advisory`. The observation is correct: the description overstates the fix. But there is no failing behaviour for a user, nothing got worse, and the only affected usage contradicts itself.

Strongest argument against this recommendation: the pull request makes a new, explicit promise about a public callback and does not keep it in the mode the callback is documented for. Under a "new promise" reading that is a defect of this change even without a regression.
