# Apply the finding threshold

The user accepted the obligation-based eligibility rule and calibration plan in [ADR-0005](adr/0005-operational-finding-threshold.md). This workflow prepares evidence and rulings. [Rubric v2](../bench/rubric/scoring.v2.md) and its grading and reporting integration are available for new batches. Historical scores retain their pinned rubric until the saved outputs are regraded and the release is approved.

Progress: recommendations 1 through 4 are implemented in code and reporting. The testing boundary is calibrated. Issue #28's [reference calibration](research/reference-calibration-2026-10-03/README.md) rechecked every saved ruling against its claim, recorded an eligibility decision, an impact card and a proposed band for each of the 30 current families, audited the four empty-reference tasks and declared the [evaluator audit](evaluator-audit.md). Sixteen families were approved as eligible then and no band was. Issue #48's [second session](research/reference-calibration-2026-10-04/README.md) recorded the user's rulings: all 31 families are eligible, each has an impact band, and three controls are audited clean. Regraded publication remains open. Recommendation 5's accepted [PR selection process](pr-selection.md) is saved for later. The [five-item tracker](benchmark-methodology-progress.md) preserves the original order and the [integration workflow](methodology-integration.md) records remaining work.

## Assess a canonical claim

Use the existing [shared claim workflow](claim-adjudication.md), pinned source references and evidence reasons. No new schema is required to record this reasoning.

| Question | Record |
| --- | --- |
| Is the problem supported? | Trigger, mechanism, concrete violated obligation, consequence, supporting evidence and relevant counterevidence. |
| Does it belong to this change? | Base/head comparison or new promise. Distinguish introduced, worsened, new obligation and pre-existing. |
| Is the trigger reachable? | Supported usage, deployment conditions or concrete maintenance activity. State prerequisites and evidence limits. |
| Is the consequence material? | Affected behavior, protection, invariant or task; the failure or cost that justifies correction; the closest calibrated example. |

Source reasoning, contracts and static counterexamples can suffice. Execute focused checks when necessary to settle a premise; do not require reproduction for every claim. Rare reachable failures can be material. Review recovery does not require repeating the entire canonical proof or proposing a fix.

Record uncertainty and exact-claim maintainer evidence when relevant. Unknown upstream disposition is not rejection. A deliberate behavior and a known defect deferred for cost are different. Assess useful advice separately from inconsequential observations, scope exclusions, refuted allegations, unsupported assertions and unresolved claims, using the [accepted recommendation's distinctions](research/finding-threshold-2026-09-29/recommendation.md#evidence-and-dispositions). These distinctions are defined by rubric v2 and its claim-level mapping format.

An adequate check examines the relevant path, prerequisites, callers or contract, attribution and obvious counterevidence. Name what was inspected and the necessary premise still missing. Missing execution access that prevents a fair decision leaves the claim unresolved. Missing support after adequate inspection is different from direct counterevidence. Rubric v1's historical treatment of unsupported assertions remains intact.

## Rules the user set on 2026-10-05

The user ruled on 42 candidate problems in the [cohort rebuild](research/cohort-rebuild-2026-10-05/README.md) and then set four rules for the questions above. Each links its saved ruling. They guide the preparation of later rulings; they approve nothing by themselves.

**Unusual input.** A regression reachable only through unusual input is a reference problem when the project gave users reason to rely on that input: it is documented, deliberately supported, or a practice users demonstrably follow. It is advice when the input is invalid or nothing shows users producing it. [Ruling](research/cohort-rebuild-2026-10-05/rulings/P1-unusual-input-rule.md). GT-w1 and GT-u5 keep their rulings. GT-u5 is the doubtful one under this rule: its input is a bad value from a misbehaving server, and the same class of value already crashed the client before the change. It was not ruled again.

**Older faults.** A fault the change did not introduce is not excluded for that reason. When the change makes it reachable or more visible, or a reviewer could detect it from the lines the change touches, it is judged by its impact: a reference problem when someone is harmed in supported use, advice when the comment is correct and useful but no one is, and not counted when the impact is trivial. It is out of scope only when the change does none of those. [Ruling](research/cohort-rebuild-2026-10-05/rulings/P4-older-faults-rule.md).

**Partly kept promises.** When a change only partly keeps a promise, the shortfall is a reference problem when a person using the software in a supported way can realistically take a real loss, whether that loss is new or is the old loss the change set out to remove. A run, the code or a report can establish the loss; a report is not required. The absence of reports counts toward advice only in proportion to the project's user base, the time since release and how often the path is used. When nobody can realistically lose anything, it is advice. [Ruling](research/cohort-rebuild-2026-10-05/rulings/P5-harm-rule.md).

**Documentation gaps.** A reference problem is something the change should have been corrected for, whether or not the correction is in code. A missing statement in the documentation a change adds can be one, and its impact card carries the `documentation` domain. Questions and records say "problem", not "bug". [Ruling 41](research/cohort-rebuild-2026-10-05/rulings/41-D5.md).

The [pinned rubric](../bench/rubric/scoring.md) still describes `scope-excluded` as including a pre-existing issue, which is narrower than the older-faults rule. The rubric was left unchanged, because changing it sends every graded batch back. The rulings of 2026-10-05 reach the graders as canonical claims with saved decisions. A comment on a surfaced older fault that no canonical claim covers is therefore still graded `scope-excluded` where the rule would call it advice. Neither outcome earns detection credit or counts as a false claim.

## Testing calibration intake

This is an intake queue with historical context, not a blinded adjudicator dossier or completed evidence audit. Preserve the original wording and qualifiers of saved items. Hide configuration identities and historical grades for an independent initial judgment, then compare the reasons against saved judgments and rulings. Claim records and later evidence must remain outside reviewer inputs.

| Order | Case and sources | Settlement question | Status |
| --- | --- | --- | --- |
| 1 | GraphQL [target](../bench/targets/k-graphql-js-1582/target.json), [packet](../bench/targets/k-graphql-js-1582/packet.md) and [register v1](../bench/targets/k-graphql-js-1582/register.v1.json). | Establish the positive anchor: what protection did the no-stack test supply, and how does the changed fixture stop distinguishing removal of the fallback? | Existing historical accepted reference. Preserve model-assisted provenance; no new human eligibility ruling inferred. |
| 2 | Bokeh [target](../bench/targets/l-bokeh-9232/target.json), [packet](../bench/targets/l-bokeh-9232/packet.md), [saved review, item 1](../bench/runs/2026-09-29-codex-thermo-high/attempts/att-004/normalized.json), [scorecard](../bench/runs/2026-09-29-codex-thermo-high/scoring/l-bokeh-9232/scorecard.v1.md) and [register v1](../bench/targets/l-bokeh-9232/register.v1.json). | Do the added integration tests fail a concrete regression obligation, or supply useful but incomplete coverage? Compare their purpose, inputs, assertions and exercised paths with GraphQL. Separately consider whether any test problem is independent of the registered product defect. | User accepted useful advisory feedback below threshold. [Ruling](../bench/claims/rulings/CL-l-initial-display.v2.md). Old mappings preserved. |
| 3 | tRPC [target](../bench/targets/j-trpc-5017/target.json), [packet](../bench/targets/j-trpc-5017/packet.md), [saved grades](../bench/runs/2026-09-24-builtin-baseline/scoring/j-trpc-5017/scorecard.v2.md) and [register v3](../bench/targets/j-trpc-5017/register.v3.json). | Does leaving `voidWithMiddleware` unasserted violate a specific meaningful test obligation? Neither an unused fixture nor the absence of a production type failure settles that question. | User accepted useful advisory feedback below threshold. [Ruling](../bench/claims/rulings/CL-j-void-assertion.v2.md). Old mappings preserved. |

Work on the pinned target head and base. Before adding a new canonical case, inventory saved reviews and link equivalent and related claims across runs. The intake's single Bokeh example identifies a starting item, not the complete affected cohort. Capture evidence in versioned records; a novel or disputed decision stays pending until the user's saved ruling under ADR-0002.

The [first source-based comparison](research/finding-threshold-2026-09-29/testing-calibration.v1.md) records GraphQL's lost specific-case protection and Bokeh's missing initial-display assertion. Its initial pending state is preserved. The [current Bokeh case](../bench/claims/CL-l-initial-display.v3.json) links six equivalent and twenty related items and records the user's advisory ruling. The [initial tRPC comparison](research/finding-threshold-2026-09-29/trpc-testing-calibration.v1.md) is preserved; the [current case](../bench/claims/CL-j-void-assertion.v3.json) records its advisory ruling across six equivalent and fourteen related items. The [calibrated boundary](research/finding-threshold-2026-09-29/calibrated-testing-boundary.v1.md) consolidates these decisions and the remaining limits. The historical GraphQL reference retains its provenance.

For each testing case, identify the claimed protection from the test and change purpose, then the specific relevant failure it should detect and why it cannot. Check contrary evidence and attribution. Passing tests alone do not settle whether their protection is effective. A failure that some imaginable test could catch does not alone establish a material obligation.

After testing, calibrate architecture/maintenance, scope exclusions and unsupported-risk boundaries with saved positives, negatives and unresolved claims. Preserve the approved ripgrep and SeaweedFS rulings as settled anchors. Record residual disagreement and what would settle it; do not modify the rule merely to reproduce every old label.

## Apply the implemented contract

The [integration workflow](methodology-integration.md) pins rubric v2, normalized review hashes and shared claim decisions. Grading preserves item provenance and assesses multiple claims separately. Equivalent claims retain the saved decision; related assertions require individual assessment. Detection does not require a remedy.

Assign impact bands, audit empty-reference controls and record advice-benefit examples through [impact calibration](impact-calibration.md). Those judgments come after eligibility and never change it. Regrade all comparable saved outputs, including earlier rejections, without rerunning the PR reviews. New or disputed eligibility decisions still require saved human rulings. Automated authority remains deferred. Preserve historical scores and publish a reconciled release only after its audit and approval.
