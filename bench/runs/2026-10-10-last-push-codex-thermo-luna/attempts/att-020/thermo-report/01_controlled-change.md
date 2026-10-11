# Controlled change synchronization

## Finding: Controlled changes bypass the native cancellation guard

**Source:** `packages/react/src/field/control/FieldControl.tsx`, lines 137–155, especially lines 142–145.

The handler now invokes `onValueChange`, then immediately returns for controlled inputs. The `event.nativeEvent.defaultPrevented` guard remains below that return and therefore runs only for uncontrolled inputs. Controlled prop changes are subsequently processed by `useValueChanged` at lines 105–115. If a native listener prevents the event while `onValueChange` still updates the parent's state, the new controlled value reaches that effect, which clears form errors, updates dirty/filled state, and calls `validation.change`. In the former single-path handler, the callback ran but internal error clearing and validation were skipped when the native event had been prevented. This makes the new controlled synchronization path inconsistent with the existing event contract.

The test at `FieldControl.test.tsx` lines 203–225 verifies that prevention skips validation and preserves server errors, but it leaves the input uncontrolled. The new controlled tests check prop synchronization and callback validation without preventing the native event. The focused suite passes, so the issue is a missing scenario rather than a currently failing test.

## Worked code-judo proposal

Treat the controlled prop as authoritative for genuine external changes, while distinguishing a prop echo caused by a prevented DOM event. Capture the proposed string and cancellation outcome in a small ref when the native event is prevented. In the `useValueChanged` callback, consume and skip only a matching canceled echo; clear that marker when another value arrives. This keeps the existing shared hook for external updates and avoids moving the whole validation path back into the event handler, which would again validate values the parent rejects or rewrites.

An alternative is a single explicit change-origin record shared by `onChange` and the controlled synchronization effect, but avoid a general event queue or a second state machine: there is only one pending input echo to classify. Keep the invariant visible in code, and test both the prevented parent echo and a later unrelated prop update so the suppression cannot leak into programmatic synchronization.

## Verification status

Ran the permitted focused jsdom suite from the clone root:

```text
TZ=UTC VITEST_ENV=jsdom ./node_modules/.bin/vitest run --project @base-ui/react packages/react/src/field/control/FieldControl.test.tsx
```

Result: 26 tests passed and 1 was skipped. No scratch test was added. The controlled prevention regression is inferred from the guard's position relative to the early return and the prop-change effect; the existing suite does not directly execute that case.
