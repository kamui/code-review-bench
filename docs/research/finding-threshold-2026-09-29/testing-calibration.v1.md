# Testing calibration: GraphQL and Bokeh

We are on recommendation 2 of the original five, making the finding threshold operational. Recommendation 1 established recurring-claim consistency, with release work deferred. Recommendations 3 through 5 remain pending. After this calibration, return to the original list rather than treating audit, delegation or release work as replacement recommendations.

The new [Bokeh canonical case](../../../bench/claims/CL-l-initial-display.v1.json) remains pending. It has six equivalent and twenty related saved items. The related items include different timezone premises and broader claims about rendering, so a ruling does not automatically propagate to them. The inventory inspected forty-eight candidate items across saved reviews; unrelated observations and existing product findings were excluded.

## Source and execution evidence

Both truncated mirrors passed the benchmark's pinned identity checks. The [offline probe](../../../bench/claims/evidence/CL-l-initial-display.v1_probe.py), [execution receipt](../../../bench/claims/evidence/CL-l-initial-display.v1.json) and [source/context record](../../../bench/claims/evidence/CL-l-initial-display.context.v1.json) preserve source identities, observations and limits. No browser or complete project test suite ran.

| Question | GraphQL | Bokeh |
| --- | --- | --- |
| What changed? | The existing named no-stack test replaced a plain object with `new Error`, which has a stack. | A whole general DatePicker integration-test file was added. No prior assertion was removed. |
| What does the test promise? | Its name expressly says it creates a stack when the original error has none. | Test names cover basic widget behavior, JavaScript callbacks and server callbacks. The PR fixes a post-selection timezone bug; the new tests do not expressly name initial-display regression coverage. |
| What protection is missing? | The named original-error-without-stack case no longer occurs. | No input-value assertion before the click checks the initial display of a Python-supplied date. No timezone is selected or varied by the new tests. |
| What still works? | Other tests still invoke stack generation without an original error. Do not describe the entire fallback branch as untested. | The JavaScript callback test asserts the displayed date after selecting a day. Model changes connect to rendering. Do not say that no display assertion or rendering path is exercised. |
| What did execution establish? | A mutation that skips stack generation only for a present original error without a stack fails the base named-case predicates, but passes the head predicates. The no-original-error control remains protected. | Exact conversion bodies behave identically in UTC. In Paris, the local selected-day input fails at base and works at head. In Los Angeles, the Python initial-value input works at base and is one day early at head. |

The GraphQL run used the pinned constructor with Flow-only declarations and imports removed, and reproduced the four assertions of the named case. It did not run the full dependency-backed suite. The Bokeh run used the verbatim conversion body on each input representation. It did not run Pikaday, the compiled widget or Selenium, and does not establish historical CI timezone or complete-suite pass/fail.

## Bokeh claim awaiting a ruling

The new tests create a DatePicker with a Python-supplied initial date but never assert the date initially shown in the input. Consequently, the supported west-of-UTC initial-display regression is not checked by those assertions. The coverage gap is supported; whether it violates a material new testing obligation remains disputed.

The frozen PR discussion identifies the original problem after selecting a day. The reviewer who supplied the eventual test file suggested integration tests and possibly varying timezone, and mentioned a separate issue as an alternative. This is related intent evidence, not an explicit adjudication of the missing first-display assertion. Merge and authorship do not establish acceptance or rejection of the canonical claim.

The historical register and scorecards classify coverage gaps as non-material. "Internally correct tests" alone is an insufficient reason: correct assertions can still fail a concrete testing obligation. A stronger possible reason is that these new general tests supply valid protection without replacing a specific initial-display regression check.

Current recommendation is useful advisory feedback below the detection threshold. This recognizes the specific coverage benefit without inferring that every new bug-fix test file must guard every supported rendering scenario. This is a proposed ruling, not an approved decision. New tests can still qualify as defects when an established regression obligation is demonstrably unmet; an explicit test name is evidence, not the only way to establish that obligation.

Four options for the canonical case:

1. Eligible test problem. Adopt the obligation that these newly added tests must protect the relevant initial model-backed display in this change. Queue the accepted problem for the next release and resolve its relationship to GT-l1 before changing the reference denominator. Reconcile all comparable reviews, including past rejections.
2. Useful advice below threshold. Accept the coverage observation and benefit, but find no sufficient independently violated test obligation on this evidence. No detection credit or false-finding penalty. Preserve this reason for equivalent items; future advisory reporting awaits the new rubric.
3. Refuted finding. Reject the factual allegation only with counterevidence. The current source and probes do not justify this option for the narrowed initial-display claim.
4. Keep unresolved. Leave eligibility pending, naming the further contract or execution evidence needed. No detection credit or false-finding penalty while unsettled.

After the user's saved ruling, examine tRPC's unasserted case. Keep broader or incorrect side claims in the related Bokeh items for individual assessment. Scores, historical mappings and reference selections remain unchanged.
