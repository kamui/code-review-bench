## Promised?

The operation is setting a controlled `Field.Control` value after mount. Base UI owns it and documents it as public. The disputed expectation is narrower. A late load would become a fresh initial value, so the field would stay clean and would not validate before someone touched it. No source found promises that expectation.

**Read, at the pinned head:** the field documentation defines dirty as "Whether the field's value has been changed from its initial value." It defines touched separately. The forms handbook says `onChange` "validates the field when the value changes". The customization handbook supports changing controlled values from code. It does not say that such changes replace a field's baseline. The forms handbook shows a form library supplying its own `dirty`, `touched`, `invalid` and error state.

**Read, before the cutoff:** the frozen PR description names the behavior it changes. Code-set values previously left "filled, dirty, or validity" stale. The new test, `syncs state and validates when the controlled value changes programmatically`, deliberately expects dirty state and one validator call. The accepted value type supports strings such as `loaded`. Neither the old tests nor the new ones establish a special late-loading baseline.

The baseline code is unchanged between the two commits. It captures the initial value once. It says "Consumers that want a fresh baseline remount or key `<Field.Root>` itself." The project's existing tests preserve the baseline across control remounts and swaps. In a [July 22 maintainer statement](https://github.com/mui/base-ui/pull/5290#issuecomment-5048021297), before the August 13 cutoff, atomiks says "A new baseline requires remounting or keying the Field root." This concerns swaps and remounts. It supports ownership of the baseline; it does not separately discuss async loads.

The six searches are saved. Counts below refer to matching files or result occurrences, not people using the feature. Full commands and raw records are in the linked files.

| Place | Hits read / hits | What was found |
| --- | --- | --- |
| [Project docs](../upstream/refresh-N1-project-docs.json) | 6 / 6 | Field and Input definitions, both handbooks, and form-library examples. No late-load exception. A further search of all `docs/src` for prefill and async initial values returned zero matches, saved in `refresh-N1-project-docs-broad.txt`. |
| Owner docs | Not applicable | Base UI owns the disputed operation. No dependency supplies its baseline policy. |
| [Change](../upstream/refresh-N1-change.json) | 3 / 3 | The PR description and both changed files. Programmatic dirty and validation updates are announced and tested. |
| [Maintainers](../upstream/refresh-N1-maintainers.json) | 29 / 29 | Queries for dirty/programmatic, prefill, and Field/initial value, restricted to records created before merge. Counts are 3, 3 and 23, including repeated results. The baseline statement above is explicit. |
| [Public code](../upstream/refresh-N1-public-code.json) | 7 / 7 | Direct code search for the Field import, `Field.Control`, `setValue` and `useEffect`. Each file was read at a dated revision before cutoff. Code-set values occur. Reliance on a fresh Base UI baseline was not established. |
| [Documented way](../upstream/refresh-N1-documented-way.json) | 1 / 1 | The handbook's React Hook Form integration works at head. Its external state owns dirty and error display. |

The public-code records include `webscit/app-framework` from May 4, `cellajs/cella` from July 29, `cellajs/raak` from July 28, `goorm-dev/vapor-ui` from March 20, and three `milejs/mile` files from December 12 or July 5. The dates identify inspected revisions. They do not establish when a practice began. The parameter controller loads values from code in `onChange` mode. It has no check that depends on Base UI keeping dirty false. The slug programs use React Hook Form. The other files show controlled or uncontrolled inputs, without the disputed dependency.

Additional discovery searches are preserved with their limits. The broader fetch code search returned 41 matches; three overlapping files were read and dated, and 38 were not. The initial dirty and effect searches returned 151 and 218 matches. Those broad results were not read as programs. They establish no reliance. Additional maintainer searches and the inspected overlaps are recorded in the maintainer index. This is not an exhaustive claim that nobody relies on the old behavior. Two code-search requests hit a rate limit. Both succeeded on retry. The failures remain saved.

No written, announced or built promise for quiet late initialization was found. Written definitions, the announcement and the deliberate tests support the behavior now observed. Under Promised 2a and 2c, the named change ends the earlier quiet behavior. Promised 8a makes an initialization guide or a different loading policy an improvement. It is not a refusal to support controlled fields.

## Recommendation

> **What happens:** a late-loaded controlled value becomes dirty and validates in `onChange` mode. **Promised: no**, a fresh quiet baseline is not promised, and the PR expressly announces syncing code-set values with dirty and validity. **Delivered: not asked**, the disputed expectation is not promised. **So: suggestion or observation (improvement).** **Band, decided separately:** none.

**Run in this refresh:** [the probe](../probes/N1/refresh/probe.test.jsx) executes four cases at each commit in jsdom and Chromium. All four runs pass. [Base](../probes/N1/refresh/result-base.txt) loads text without dirty state or validation. [Head](../probes/N1/refresh/result-head.txt) marks it dirty and shows `Loaded value rejected` in `onChange` mode. Default `onSubmit` stays quiet before submission. Touched remains false. The platform is not configured differently; no platform-setting comparison or prevalence estimate applies.

The same probe runs the documented Controller integration with real React Hook Form. A Load button adds `reset({ username: 'loaded' })` to that integration. Loading stays clean and quiet; submitting displays the error. The handbook does not itself show that Load button. Keying a fresh Field root also stays clean at both commits. No actual network fetch, unsaved-change prompt, Firefox or WebKit run was made. [Tool versions and cleanup](../probes/N1/refresh/environment.txt) and the Chromium outputs are saved beside the probe.

The earlier dossier's mechanism and probe results still hold. The code's dirty state is truthful relative to the original baseline. The validator rejects the value the application asks it to validate. A different initialization policy would help some forms, but no broken promised outcome or separate minor defect was established. This does not reach GT-r3's required-error suppression rule: N1 loads nonempty text and remains dirty.

The strongest argument against this recommendation is Promised 7a. Late loading is an ordinary use of a documented controlled field, and it used to stay quiet. If the owner reads the PR announcement as general state repair that never names late initialization, that earlier behavior could remain promised. Promised 2b would then matter. Against that reading, the PR names dirty and validity updates for code-set values, and the older baseline rule already requires a new Field root.

The nearest rulings are `first-24`, the announced switch to validating the application's stored value, and `first-22`, where a general announcement did not withdraw a specific required-error rule. Both were shown again under the current reading. N1 resembles the first. No opposing late-load rule was found. Promised 2a rests on only that one substantive precedent and has no recorded blind application to a new case. No ruling decides this exact late-loading case. That limit keeps the decision with the owner despite high confidence in the evidence.
