# Card: a prevented input event on a controlled field

Pull request: mui/base-ui #5460. `Field.Control` is Base UI's text input for a form field. It is controlled when the app passes the text in through the `value` prop and stores each edit itself. "Base" is the commit before the change and "head" the reviewed commit; the "earlier revision" is a first version of the pull request that the reviewed commit replaced.

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
