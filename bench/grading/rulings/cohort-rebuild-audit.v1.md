# Rulings on the evaluator audit during the issue 30 cohort rebuild

Saved human rulings of 2026-10-04 on how the [evaluator audit](../../../docs/evaluator-audit.md) runs. Each entry holds the question as shown, the options and the user's answer.

## Second assessor

> Issue 30 also requires the evaluator audit you chose on Oct 3: 220 graded units are judged a second time (20 of each of eight kinds, plus 60 on serious bugs) and compared with the first grade; at most 0 errors are tolerated on serious bugs and 1 per other kind. The second judge must be a person or a separate model session that did not produce the grade. Since you are settling on one grader, the options are a second set of fresh Claude Opus 5.5 High sessions that never see the first verdict (known limit: the same model can repeat its own mistake, so agreement is not accuracy), or leaving the audit for later. Model audit sessions are extra plan usage, roughly $70 to $220 list price; I can only give a firm number once the sample is drawn after grading. Who performs the audit?

Options shown: "Fresh Opus 5.5 High (Recommended)", "Defer the audit", "I will audit by hand".

The user typed: "Let's use Codex Sol 6.1 High to audit, any issues with that?" Told that the route has no dollar cap, that Sol 6.1 is itself a benchmarked reviewer and that the audit steps still had to be built, the user kept the choice and approved a no-charge preflight of it.

Ruling: Codex GPT-6.1 Sol at high effort is the audit's second assessor. Its queue and its uncapped ChatGPT usage still need the user's authorization once the sample is drawn.

## Grades stay off the main branch until the draw

> The independent review found a conflict with the audit plan you approved on Oct 3. That plan says the audit sample is drawn 'after every selected batch has a current grade and before anyone opens a scorecard or an export built from those grades'. This PR would put 39 graded reviews (5 of 199 batches) on main, and merging to main rebuilds and deploys the public site from them. Most numbers stay 'unavailable', but not all: if a visitor selects only the three Codex built-in setups on the three PRs already graded for them, the site would show unaudited serious-bug detection rates (the reviewer saw Astra 81.0%, Sol 6.1 76.2%, Luna 0.0%). How should this PR handle the grades?

Options shown:

- "Keep grades off main (Recommended)": The PR records the rulings, the new reference bug, the evidence archives and the tools, but grades.json stays empty on main. The 5 graded batches stay saved on the working branch and return when grading resumes. Nothing unaudited appears on the public site and the audit plan is followed as written.
- "Allow a partly graded preview": Merge the 39 graded reviews. I save your answer as a recorded deviation from the audit plan's timing clause and correct the README to say that fully graded selections show unaudited rates. The site will show those rates under its 'v1 preview' label.
- "Do not merge yet": No PR now. Grading resumes on this branch and one PR follows after grading and the audit sample draw.

The user chose "Keep grades off main (Recommended)".

Ruling: `grades.json` on the main branch holds no batch until the audit sample is drawn. The timing clause of the audit plan is unchanged.

## An assessed audit survives its corrections

> A rule the audit documentation leaves open, which the audit tool must enforce one way or the other. After the audit compares the two judgments, any confirmed grading error is corrected by grading that batch again, which changes the saved grades. Should an audit that passed stay marked 'assessed' after those corrections? 'Assessed' is what lets the site show an overall recommendation.

Options shown:

- "Stays assessed (Recommended)": The audit judges the grades as they were when the sample was drawn. Corrections afterwards are part of its outcome and do not reopen it. The tool refuses only if grades change between the draw and the conclusion.
- "Corrections reopen it": Any regrade after the draw sets the audit back to unassessed until a new sample is drawn with a new seed. Stricter, and an audit that finds any error can only pass on a second full round.

The user chose "Stays assessed (Recommended)".

Ruling: an assessed audit describes the grades as drawn. Correcting its confirmed errors afterwards leaves it assessed.
