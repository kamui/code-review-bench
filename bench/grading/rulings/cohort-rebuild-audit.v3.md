# Rulings on the evaluator audit during the issue 30 cohort rebuild, third receipt

Saved human rulings of 2026-10-12 on the [evaluator audit](../../../docs/evaluator-audit.md), made after the last-push selection was published and before any of it was graded. Each entry holds the question as shown, the options and the user's answer. The first closes the round declared on 2026-10-03. The second selects the plan that replaces it.

## Audit decision 3: round one is closed as superseded

Asked 2026-10-12 (03:33 UTC), with the regrade plan. Shown as a formatted message: that round one was declared on 2026-10-03 with nine strata and 216 units; that the second grader (Codex GPT-6.1 Sol, high) agreed with the first on caught 19 of 20, missed 19 of 20 and serious 57 of 60, but on comment labels only 1 to 7 of 20 per stratum, which led to the new rubric; that the round was never concluded and the grades it sampled were removed at the switch to the next rubric; and that the tool cannot conclude it, so closing it takes the owner's words as a receipt and a closure record saying that it did not pass and why.

Question as shown: "Audit round 1 was never concluded and its sampled grades are gone. How is it closed?"

Options shown:

- "Close as superseded (Recommended)": Recorded with your answer as the receipt: round 1 is closed as superseded by the next rubric. It did not pass and is not described as passed. Its sample, comparison and reconciliation stay as evidence.
- "Close as failed": Recorded as a round that failed its label tolerances (1 to 7 of 20 agreed per label stratum), which is what led to the rubric change.
- "Need more context": Tell me what is missing about round 1 and I will lay it out.

The user chose "Close as superseded (Recommended)".

Decision: the round drawn with seed `evaluator-audit-v1-2026-10-03` is closed as superseded by the next rubric. It did not pass and is not described as passed. Its declaration, sample, second assessments, comparison, reconciliation and rulings stay as evidence.

## Audit decision 4: the plan for the next audit

The same message proposed a plan of about 265 units. The user did not choose among its options and answered: "$arena 2, this agents suggested audit 1 proposal (strata, sample size, tolerances), astra 6 high. Come up with an audit 1 proposal with strata sample size, tolerance, etc." Two proposals were then written independently, the session's and one by Codex GPT-6 Astra at high effort, and scored blind by a third model against a rubric neither had seen. Astra's was taken as the base and parts of the other were added. The [record](../../../docs/research/cohort-rebuild-2026-10-05/audit/next-plan/note.md) holds both proposals, the judge's report and what was kept or rejected.

Asked 2026-10-12 (04:12 UTC). Shown as a formatted message: why Astra's proposal was the base; the table of twelve strata with each one's unit, size, allowed errors and the error rate a pass rules out at 95% confidence; that tolerances count confirmed first-grader errors after reconciliation and not raw disagreements; that a stratum no larger than its size is audited whole with no tolerance; that claims linked to a saved ruling leave the claim strata and get a mechanical check over the whole cohort; that the second fact is recorded and not compared; that a stratum over tolerance fails the round, with one redraw under a new seed and a separately approved spend; that the owner is the final reconciler; what differed from the earlier proposal (larger sizes, `serious` at 100 units with one error allowed in place of 60 with none, and the new `other-material` and `control-silence` strata); the cost (a full second pass of up to 189 sessions, about $100 at list price on the ChatGPT plan with $200 reserved, and roughly 20 to 45 disagreements for the owner to reconcile); the risk that the `suggestion` stratum fails if the label disagreement of the trial's two fresh batches holds; and the tool changes needed first. The full text is the [proposal](../../../docs/research/cohort-rebuild-2026-10-05/audit/next-plan/proposal.md) (SHA-256 `b1f52b36e72ed517feefbed507a412724e1b81ebc6aded6512abeeb53c704214`).

Question as shown: "Which audit plan should I write up as the declaration for your approval receipt?"

Options shown:

- "As proposed (Recommended)": Twelve strata as in the table, serious at 100 units with one error allowed. About $100 of Codex usage (reserve $200) and roughly 20 to 45 disagreements for you to reconcile.
- "Serious stays 0 of 60": Same plan, but serious credit keeps round one's rule: 60 units, no error allowed. Same 5% limit, but about a 1 in 5 chance of failing a sound grader on one error, against 1 in 16.
- "Smaller sample": Same strata and rules with my earlier sizes (20 in most strata, 60 serious). Roughly 100 to 130 sessions and half the reconciling, but a pass only rules out error rates above 22% in most strata.

The user chose "As proposed (Recommended)".

Decision: the next audit follows the proposal, with seed `evaluator-audit-v2-2026-10-12`.

Selected sample: 100 units in `serious`; 60 units in each of `caught`, `missed`, `other-material`, `refuted` and `suggestion`; 40 units in each of `unresolved-claim` and `control-silence`; and 30 units in each of `unresolved-credit`, `unproven`, `minor-defect` and `outside-this-change`.

Selected tolerance: at most 0 confirmed first-grader errors in `unresolved-credit`, at most 2 in `suggestion`, and at most 1 in each other stratum. A stratum audited whole has no tolerance.

The selection authorizes no dispatch. The second pass needs its own queue, usage estimate and authorization after the draw.
