# Control lifetime, decomposition, and test coverage

## Finding: empty replacement controls inherit filled state

FieldControl.tsx:99–103 replaces the old value-dependent filled effect with a mount effect that only handles a truthy input value. The controlled observer at :105–115 supplies later updates, but the shared `useValueChanged` hook intentionally does not fire for an initial value. Neither path therefore assigns false when an empty controlled control mounts inside a root whose previous control left filled true.

The scratch reproduction retains one Field.Root and changes the Field.Control key. The first instance is controlled with value `'foo'`; the replacement instance is controlled with value `''`. After replacement, its DOM value is empty, yet the persistent root still has `data-filled`. No mode switch or unmount of the field root is involved. The baseline effect specifically includes `hasExternalValue && valueProp === ''` and sets filled false, so the baseline passes this probe.

The new test for an empty controlled value at FieldControl.test.tsx:179–187 mounts a fresh root. That root begins with filled false, so the missing false assignment is invisible. The existing registration architecture deliberately captures the initial dirty baseline once per root, rather than once per control. `useFieldControlRegistration.ts:85–101` documents that ownership. Filled state has a different meaning: it describes the current control's value, so preserving the original baseline does not justify preserving the previous control's filled state.

Verification status: failed on the head and passed in the isolated baseline comparison. The head assertion confirms the replacement DOM input is empty before checking filled. The changed-line anchor is FieldControl.tsx:99–103. Source, logs, and exact commands are described in [01_value_synchronization.md](01_value_synchronization.md) and preserved in the probes directory.

## Worked minimal correction

Make mount initialization total rather than truthy-only:

```tsx
useIsoLayoutEffect(() => {
  const input = validation.inputRef.current;
  if (input) {
    setFilled(input.value !== '');
  }
}, [validation.inputRef, setFilled]);
```

This assigns both true and false for a present control and avoids interpreting an absent input as an empty control. It leaves subsequent controlled observation to `useValueChanged`, leaves the field-level dirty baseline alone, and does not add control-instance flags or a second lifecycle state machine. Add a test that retains the root and replaces the keyed input, because mount-only tests on fresh roots cannot exercise this ownership boundary. This remedy is a proposal, not an applied or tested patch.

## Test architecture and verification coverage

The diff strengthens the uncontrolled render test by counting calls to the control's render callback instead of renders of a wrapper component. That now measures the relevant element render path. The controlled render test uses a non-strict renderer and starts counting after the first dirty/filled transition; this is a reasonable steady-state performance assertion. The new controlled validation, numeric baseline, and programmatic update tests cover the main advertised functionality.

The coverage gaps map to concrete failures rather than an abstract demand for more tests. The existing prevented-change test is uncontrolled, so it cannot observe the new controlled early return. The cancellation test checks validation but omits dirty and filled. The empty-controlled mount test uses a new root, masking stale retained state. The numeric test demonstrates string/number comparison consistency but does not test native normalization. Extend those focused scenarios; do not add a broad snapshot or tests that merely restate each setter call.

The external positive tests are equally relevant to remediation. Rejected controlled attempts do not validate or mutate field state. Uppercase rewrites validate `'A'` once. An asynchronous validation for `'old'` cannot publish after a programmatic prop change to `'new'`. All three passed on the head. These are behavior requirements for any proposed simplification, and explain why moving all handling back into the DOM event would be a poor remedy.

All runnable existing tests in the four selected files passed: 165 passes, four skips. Scratch probes exposed five failures, four of which also passed with the baseline control. The probe suite exits with failure by design because its assertions express the required behavior; this is evidence of the review findings, not a harness failure. Browser-only autofocus hydration and other skipped cases remain unexecuted. No rendered-output or browser-specific conclusion is claimed.

## Size and complexity measurements

`git diff --stat main...review-head` reports two changed files, 197 insertions, and 26 deletions. `wc -l` on the head and `git show main:<file> | wc -l` on the baseline give these sizes:

| File | Baseline | Head | Net growth |
| --- | ---: | ---: | ---: |
| packages/react/src/field/control/FieldControl.tsx | 179 | 207 | 28 |
| packages/react/src/field/control/FieldControl.test.tsx | 130 | 273 | 143 |

Neither file approaches or crosses the skill's 1,000-line threshold. The production component remains compact enough to keep the local transition helper in the same file. Extracting a generic cross-control abstraction merely to move these 28 added lines elsewhere would not buy clarity. The structural problem is the duplicated accepted-value checklist and the different meanings of the value supplied to it, not physical module size.

The PR adds three local decision points: missing serialized values in the controlled observer, controlled-mode early return in the event handler, and canceled-details checking alongside native prevention. Controlled/uncontrolled adapters are justified by ownership. Their divergent transition policy is not. The opportunity is to keep the adapters small and delete duplicated policy, while relying on the registration getter and canonical validation mechanism already present. There is no new cast-heavy boundary, excessive generic wrapper, sequential independent work, or large module explosion to report.

## Scope exclusions and checkout integrity

The PR body promises focus cleanup on disabled/unmounted controls, but this pinned head adds no cleanup. The old and new source both set focused false only in blur handling. That absence is recorded as a packet/source mismatch, not promoted into a new regression finding. Likewise, the body mentions `processedValueRef`, which does not exist in the reviewed head; review remedies do not assume it is present. The head also does call `clearErrors(name)` on controlled changes despite the body's open question.

Only the selected frozen skill, supplied packet and policy, committed diff, implementation dependencies, and tests were read as review evidence. Repository guidance was not loaded or treated as instructions. The review was single-context; no child call was required by the frozen skill.

The initial `git status --porcelain=v1` was empty and HEAD matched the pinned SHA. Subsequent `git status --porcelain=v1` and `git diff --exit-code` remained clean. HEAD was `14d39e5d1ad6b7aca2fb067415dba09c6bea219b`; its committed tree was `886c9868c129584e419e29a6aefa273d70bcff39`. Every new source, configuration, log, and report file is in clone-work. The clone was neither edited nor checked out to another branch, and its installed ignored dependencies were left untouched.
