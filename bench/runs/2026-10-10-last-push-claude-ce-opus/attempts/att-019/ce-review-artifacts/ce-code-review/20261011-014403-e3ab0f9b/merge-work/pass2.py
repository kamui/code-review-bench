import json, re, sys
run = sys.argv[1]
F = 'packages/react/src/field/control/FieldControl.tsx'
T = 'packages/react/src/field/control/FieldControl.test.tsx'
def norm(t): return re.sub(r'\s+', ' ', t).strip().lower()
def k(reviewer, file, line, title): return f"{reviewer}|{file}|{line}|{norm(title)}"
detail = json.load(open(run + '/merge-work/source-detail-map.json'))
mech = json.load(open(run + '/mechanical-findings.json'))
raw = json.load(open(run + '/raw-returns.json'))
rawidx = {}
for r in raw:
    for f in r['findings']:
        rawidx[k(r['reviewer'], f['file'], f['line'], f['title'])] = f
mechidx = {}
for part in ('findings', 'pre_existing_findings', 'suppressed_findings'):
    for f in mech[part]:
        for rv in f['reviewers']:
            mechidx[k(rv, f['file'], f['line'], f['title'])] = f

K = {
 'cor_reset': k('correctness', F, 114, 'Clearing a controlled field after submit raises a required error'),
 'jul_reset': k('julik-frontend-races', F, 105, 'Reset after submit marks required controlled fields invalid'),
 'adv_reset': k('adversarial', F, 114, 'Clearing controlled required inputs after submit flags them invalid'),
 'api_reset': k('api-contract', F, 114, "Hyrum's Law: code-cleared required input now shows an error"),
 'cor_clear': k('correctness', F, 110, 'Programmatic change clears Form errors, contradicting the PR body'),
 'tst_clear': k('testing', F, 110, 'Programmatic change clears Form errors: untested and contradicts PR body'),
 'api_clear': k('api-contract', F, 110, 'Code-driven value change clears Form errors; PR body says otherwise'),
 'adv_clear': k('adversarial', F, 110, 'Programmatic change clears Form errors, contradicting the PR description'),
 'jul_clear': k('julik-frontend-races', F, 110, 'Programmatic change clears server errors, contradicting the PR body'),
 'fp_body': k('fast-pass', F, 110, 'PR body describes behavior the diff does not contain'),
 'cor_focus': k('correctness', F, 163, 'Promised data-focused unlatch on disable/unmount is not implemented'),
 'adv_focus': k('adversarial', F, 163, 'Promised focus-latch fix for disabled or unmounted control is absent'),
 'api_focus': k('api-contract', F, 163, 'PR claims data-focused latch fix that the diff omits'),
 'jul_focus': k('julik-frontend-races', F, 158, 'Promised data-focused unmount/disable fix is not in the diff'),
 'tst_focus': k('testing', F, 158, 'Promised data-focused latch fix has no code and no test'),
 'jul_render': k('julik-frontend-races', F, 144, 'Controlled keystrokes render twice whenever validation publishes'),
 'adv_render': k('adversarial', F, 144, 'Controlled keystrokes render twice when validating on change'),
 'tst_render': k('testing', T, 79, 'Render-count test skips the mode where controlled renders doubled'),
 'cor_bail': k('correctness', F, 144, 'Controlled change handler bails out; described two-path guard absent'),
 'tst_guard': k('testing', F, 144, 'Controlled path skips prevented/canceled guards; only uncontrolled is tested'),
 'cor_dirty': k('correctness', F, 111, 'Dirty latches when value arrives while control is disabled'),
 'jul_blur': k('julik-frontend-races', F, 114, 'Blur validation result lost when value is rewritten on blur'),
 'cor_filled': k('correctness', F, 100, 'data-filled stays set when a control remounts with empty value'),
 'tst_reject': k('testing', F, 142, 'Rejected or rewritten controlled value contract is unpinned'),
 'fp_serial': k('fast-pass', F, 93, 'Field now registers the serialized string instead of the raw value'),
}
for name, key in K.items():
    assert key in rawidx, (name, key)
    assert key in mechidx, (name, key)
    if not name.startswith('fp_'):
        assert key in detail, (name, key)

FIX = {}
FIX['reset'] = ("Distinguish a user-originated change from a programmatic one: record the value handled in `onChange` in a ref (the `processedValueRef` the PR body describes) and compare and clear it in the `useValueChanged` callback. For a programmatic change keep `setDirty`/`setFilled` but do not run full validation; use the revalidate-only path `validation.commit(serializedValue, true)`, which clears a resolved error and returns early when the field is not currently invalid (useFieldValidation.ts:169-172), so a reset cannot raise a new error. This needs the `commit` type in UseFieldValidationReturnValue widened to accept `revalidate`. Assumption: a value set from code should resolve stale errors but not surface new ones. That assumption re-scopes the new test 'syncs state and validates when the controlled value changes programmatically' and departs from the sibling controls, so confirm the semantics with the maintainer; one reviewer proposed the narrower rule of skipping `validation.change` only when the programmatic value returns to the baseline (`serializedValue === (validityData.initialValue ?? '')`). If full validation on code-driven changes is the intended contract, keep the code, pin the submit-then-clear flow with a test, and state the change in the PR description. Either way add a test: Form + required controlled control, type, submit, `setValue('')` in `onFormSubmit`, expect no `data-invalid`.")
FIX['clear'] = ("Resolve the PR's open question against what the code does. Default proposal: keep `clearErrors(name)` in the prop sync (it matches the sibling controls, for example SwitchRoot.tsx:106, NumberFieldInput.tsx:102 and OTPFieldRoot.tsx:200, and the forms handbook), correct the PR description (programmatic changes do call `clearErrors(name)`; there is no `processedValueRef`, the controlled change handler returns early), and add a FieldControl test with a `<Form errors={ERRORS}>` object defined outside the re-rendering component, a named controlled `Field.Control`, and a button calling `setValue('external')`, asserting the 'Server error' text is gone afterwards. If only user edits should clear errors, do not just delete the call (controlled keystrokes reach field state only through this callback): set a ref in `onChange` before the `isControlled` early return and call `clearErrors(name)` in the `useValueChanged` callback only when that ref equals `serializedValue`, and assert the error stays after a programmatic change.")
FIX['focus'] = ("Either remove the claim from the PR description, or implement it in FieldControl: track ownership in a ref (`const focusedRef = React.useRef(false)`, set true in `onFocus` and in the autoFocus effect, false in `onBlur`) and clear on the two exit paths: `useIsoLayoutEffect(() => { if (disabled && focusedRef.current) { focusedRef.current = false; setFocused(false); } }, [disabled, setFocused]);` and an unmount cleanup `useIsoLayoutEffect(() => () => { if (focusedRef.current) { focusedRef.current = false; setFocused(false); } }, [setFocused]);`, declared before the existing autoFocus effect so a strict-mode remount re-applies autofocus focus state. The ref keeps one control from clearing focus that belongs to another control in the same Field.Root. Add tests for both paths (focus, then disable; focus, then unmount the control inside a persistent Field.Root) asserting the root no longer has `data-focused`.")
FIX['render'] = ("Handle the user's keystroke in `onChange` for controlled inputs too, so `setDirty`/`setFilled`/`clearErrors`/`validation.change` (gated by `!event.nativeEvent.defaultPrevented && !details.isCanceled`) batch with the consumer's `setValue` in the same event, and store the handled string in `processedValueRef`; in the `useValueChanged` callback return early and clear the ref when `serializedValue === processedValueRef.current`, so only programmatic changes and consumer rewrites sync from the prop. To keep the 'rejected or rewritten value never sticks' property, resync from the prop when it does not echo the processed value. Assumption: one extra render is acceptable only on reject or rewrite. Parametrize 'renders once per keystroke for controlled input changes' over `{}` and `{ validationMode: 'onChange', validate: () => null }` on Field.Root and assert the first-keystroke count instead of discarding it (the onChange row fails at the reviewed head: 4 renders for 2 keystrokes). If the second render is accepted instead (sibling controls behave the same), pin the real number in that test, rename it, and drop the comment at FieldControl.test.tsx:78.")
FIX['guard'] = rawidx[K['tst_guard']]['suggested_fix'] + " Handling the user's change inside `onChange` for controlled inputs (the fix proposed for the double-render finding) restores this guard as well."

def cand(title, sev, file, line, conf, cls, owner, rv, fix, fe_key, src_keys, why_key, extra_evidence=()):
    ev = []
    for sk in src_keys:
        for e in (detail.get(sk, {}).get('evidence') or []):
            if e not in ev: ev.append(e)
    for e in extra_evidence:
        if e not in ev: ev.append(e)
    reviewers, indep = [], []
    for sk in src_keys:
        m = mechidx[sk]
        for r in m['reviewers']:
            if r not in reviewers: reviewers.append(r)
        for r in m['independent_reviewers']:
            if r not in indep: indep.append(r)
    return {
        'finding': {'title': title, 'severity': sev, 'file': file, 'line': line, 'confidence': conf,
                    'autofix_class': cls, 'owner': owner, 'requires_verification': rv, 'pre_existing': False,
                    'suggested_fix': fix, 'first_evidence': rawidx[fe_key]['first_evidence'], 'evidence': ev},
        'carry': {'source_keys': list(src_keys), 'why_key': why_key, 'reviewers': reviewers, 'independent_reviewers': indep},
    }

cor_bail_scratch = [e for e in detail[K['cor_bail']]['evidence'] if e.startswith('Scratch differential test')]
assert len(cor_bail_scratch) == 1
C = []
C.append(cand(rawidx[K['cor_reset']]['title'], 'P1', F, 114, 75, 'manual', 'downstream-resolver', True, FIX['reset'], K['cor_reset'],
              [K['cor_reset'], K['jul_reset'], K['adv_reset'], K['api_reset']], K['cor_reset']))
C.append(cand(rawidx[K['cor_clear']]['title'], 'P2', F, 110, 100, 'manual', 'human', True, FIX['clear'], K['cor_clear'],
              [K['cor_clear'], K['jul_clear'], K['api_clear'], K['tst_clear'], K['adv_clear'], K['fp_body']], K['jul_clear']))
C.append(cand(rawidx[K['cor_focus']]['title'], 'P2', F, 163, 100, 'manual', 'downstream-resolver', True, FIX['focus'], K['cor_focus'],
              [K['cor_focus'], K['adv_focus'], K['api_focus'], K['jul_focus'], K['tst_focus']], K['cor_focus']))
C.append(cand(rawidx[K['jul_render']]['title'], 'P2', F, 144, 75, 'manual', 'downstream-resolver', True, FIX['render'], K['jul_render'],
              [K['jul_render'], K['adv_render'], K['tst_render'], K['cor_bail']], K['jul_render']))
g = cand(rawidx[K['tst_guard']]['title'], 'P2', F, 144, 75, 'manual', 'downstream-resolver', True, FIX['guard'], K['tst_guard'],
         [K['tst_guard']], K['tst_guard'], extra_evidence=cor_bail_scratch)
# correctness's composite finding names this consequence too; it is split across the render and guard candidates
g['carry']['source_keys'].append(K['cor_bail']); g['carry']['partial_sources'] = [K['cor_bail']]
for r in mechidx[K['cor_bail']]['reviewers']:
    if r not in g['carry']['reviewers']: g['carry']['reviewers'].append(r)
for r in mechidx[K['cor_bail']]['independent_reviewers']:
    if r not in g['carry']['independent_reviewers']: g['carry']['independent_reviewers'].append(r)
C.append(g)
for key in ('cor_dirty', 'jul_blur', 'cor_filled', 'tst_reject', 'fp_serial'):
    f = rawidx[K[key]]
    C.append(cand(f['title'], f['severity'], f['file'], f['line'], f['confidence'], f['autofix_class'], f['owner'],
                  f['requires_verification'], f.get('suggested_fix'), K[key], [K[key]], K[key]))
synthetic = [{'reviewer': 'synthesis', 'findings': [c['finding'] for c in C], 'residual_risks': [], 'testing_gaps': []}]
json.dump(synthetic, open(run + '/merge-work/helper-input-pass2.json', 'w'), indent=1)
carry = {f"{c['finding']['file']}|{c['finding']['line']}|{norm(c['finding']['title'])}": c['carry'] for c in C}
assert len(carry) == len(C)
json.dump(carry, open(run + '/merge-work/carry-map.json', 'w'), indent=1)
print('candidates', len(C))
