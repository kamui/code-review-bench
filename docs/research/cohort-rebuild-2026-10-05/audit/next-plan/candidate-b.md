Propose this as evaluator audit v2, subject to the owner's saved approval before first grading starts. It replaces the 2026-10-03 declaration. Close that round as superseded and unfinished, preserving its declaration, assessments, disagreements and rulings. Do not call it passed.

The audit covers the fixed selection of 604 admitted reviews in 189 batches, across 16 pull requests and 17 setups. Its answer key has 67 known problems, 31 serious and 36 other-material. Use Claude Opus 5.5 at high effort for the first grades and Codex GPT-6.1 Sol at high effort for the second grades. Freeze the exact rubric, rules, verdict contract v2, references, applicable rulings, client versions and grading policy before grading. This proposal authorizes no dispatch by itself.

The recommendation is 11 strata, at most 500 selected units, and one second-grader session per distinct selected batch. Budget for up to 189 sessions. This is an expensive audit because units share batch-sized sessions. It checks detection and claim labels separately. It does not establish a blended score or a winner.

The evidence behind the design is specific:

| Evidence | Consequence for this plan |
| --- | --- |
| First round: caught agreement was 19/20, missed 19/20 and serious 57/60. The detection differences were second-grader unresolved answers, not caught/missed flips. | Preserve caught, missed and serious checks. A disagreement is not automatically a first-grader error. |
| The detection reconciliation contains 12 distinct disagreements, spanning 13 stratum memberships: six first errors, four first-correct decisions and two undetermined. The six errors shared one derivation bug. Decision 2 identified 39 of 54 unresolved recoveries as consequences of that bug. | Keep a separate unresolved-credit stratum. Check the credit derivation for every review/problem pair before drawing. Expand fixes by cause, not just by sampled unit. |
| The first label analysis attributed 20 of 80 disputed units to test answers about different things, 15 to different splitting and 10 to different labels or thresholds on the same facts. | Use the owner's fixed claim list. Gate on outcomes, not equality of every answer or explanation. |
| Final trial: labels agreed on 401/422 claims, credit on 394/397 review/problem pairs and the first fact on 444/450 claim/problem pairs. | Audit credit directly. Also audit each claim outcome, since correct review-level credit can hide a wrongly labelled individual claim. |
| Two previously untouched batches: labels agreed on 59/71 claims and credit on 33/33 pairs. Seven of the twelve label differences concerned what was promised. | Do not treat the rehearsed ten batches as a population estimate. Sample suggestions heavily and retain rare label strata. |
| On identical-text repeat sessions, labels agreed on 176/181 for each grader; credit on 177/177 and 176/177. The second fact was much less repeatable. | Allow a small number of confirmed label errors. Do not promise perfect repeatability. Follow P24 and exclude the second fact from this audit. |

These observations come from `docs/research/cohort-rebuild-2026-10-05/audit/decision-1.md`, `decision-2.md`, `reconciliation-detection.v1.json`, `label-analysis/README.md`, and the trial's `README.md`, especially its final-text and untouched-batch sections. The trial was partly chosen around disputed comments and repeatedly used during rubric revision. Its agreement figures are design evidence, not unbiased error estimates or audit results.

Define the population after all 189 batches have valid current v2 grades. Use only selected, admitted reviews. Exclude unadmitted attempts and superseded grades. A known problem is an approved family of that review's task in the frozen references. Candidates awaiting approval are not known problems. Approved impact bands come from the pinned impact decisions, never from a grader or reviewer priority.

For each admitted review R and each known problem F of its task, derive D(R,F) from R's claims. It is `caught` if any `known_problems` entry for F has `says_what == "yes"`. Otherwise it is `unresolved` if any such entry has `says_what == "cannot-tell"`. Otherwise it is `missed`. An absent entry contributes no positive or uncertain credit. An unresolved claim about a different problem, or no known problem, cannot block F. A positive claim takes precedence over another uncertain claim. `identifies_cause` and `cause_only` never enter D.

Use the mapped grade fields in `bench/schema/current-grade.schema.json`: `batches[].reviews[].claims[]`, each claim's `outcome` and `known_problems[].family_id`, and each review's `families[]`. Check that the saved family outcomes equal D for the entire population. Reject a missing family row, stale grade or derivation mismatch before drawing. This complete mechanical check does not replace second grading of the underlying judgments.

A claim unit is one recorded claim occurrence, identified by its review and claim ID. Include duplicates and claims with saved rulings. Do not merge them before sampling. That makes the sampling population reproducible and avoids assuming that duplicate groups were assigned correctly. Distinct-claim scorecard counts have an additional grouping limitation, stated below.

| Stratum ID | Unit | Exact first-grade population | Requested n | Maximum confirmed first errors k |
| --- | --- | --- | ---: | ---: |
| `credit-caught` | review/problem pair | All R,F with D(R,F) = `caught` | 60 | 1 |
| `credit-missed` | review/problem pair | All R,F with D(R,F) = `missed` | 60 | 1 |
| `credit-unresolved` | review/problem pair | All R,F with D(R,F) = `unresolved` | 30 | 0 |
| `serious-credit` | review/problem pair | All R,F whose F has approved impact `serious`, whatever D | 60 | 0 |
| `label-problem` | claim | `outcome == "problem"` | 40 | 1 |
| `label-refuted` | claim | `outcome == "refuted"` | 60 | 1 |
| `label-unproven` | claim | `outcome == "unproven"` | 30 | 1 |
| `label-suggestion` | claim | `outcome == "suggestion"`, every `kind` | 60 | 2 |
| `label-minor-defect` | claim | `outcome == "minor-defect"` | 30 | 1 |
| `label-outside-change` | claim | `outcome == "outside-this-change"` | 30 | 1 |
| `label-unresolved` | claim | `outcome == "unresolved"`, every `open.kind` | 40 | 1 |

These are overlapping strata only where stated. A serious pair also belongs to exactly one credit-outcome stratum. A claim and a review/problem pair are different units even when the same words decide them. Other-material problems are covered by the three credit-outcome strata. The stricter serious stratum reflects the calibrated meaning of serious: the implementer must hear about it before release. It does not reassess the impact label.

The 60-unit samples give the strongest checks to serious detection, credit in both directions, refuted claims and suggestions. Refuted claims drive a displayed reliability comparison. Suggestions are common and repeatedly crossed the promised/unresolved boundary in the trial. Two allowed suggestion errors, rather than one, accept a coarser error ceiling there. The 30- and 40-unit strata preserve coverage of less common outcomes without pretending they offer the same assurance. The sizes and tolerances are policy choices informed by the evidence, not estimates fitted to its disagreement counts.

If a population has N units, select m = min(N,n). If N is smaller than n, assess every unit and use tolerance floor(k × m / n). This prevents a tiny stratum from passing with most of its grades wrong. Report its exact census error fraction instead of a sampling interval. An empty population is recorded as empty, with no agreement percentage or accuracy claim. Do not transfer unused sample places to another stratum.

Fix the seed as `evaluator-audit-v2-2026-10-11`. The first round uses this seed without a suffix. Give each unit a canonical ID, serialized as compact UTF-8 JSON with no spaces and no ASCII escaping:

- Pair: `["pair", run_path, target_id, attempt_id, family_id]`.
- Claim: `["claim", run_path, target_id, attempt_id, claim_id]`.

Use repository-relative POSIX run paths exactly as pinned in the selection. Within each stratum, sort ascending by the hexadecimal SHA-256 of `seed + ":" + stratum_id + ":" + unit_id`. Break a hash tie by unit ID. Take the first m. Save all population IDs, their sizes, the grade and input hashes, the selected IDs and batch union. Draw once, after grading and before opening any scorecard or export based on those grades. Do not balance or replace units by hand, including units from the trial tasks. Report coverage by PR, setup, batch, problem, impact band and credit state.

Audit an overlapping selected unit once. Count it in each stratum that actually selected it, not every stratum it could belong to. The whole batch is assessed once even if several strata select it. Extra assessments from that batch are diagnostic evidence, not extra sample units or substitutes for missing ones.

For a credit stratum, a disagreement means D differs between graders, including either direction involving `unresolved`. For a label stratum, it means the exact `outcome` differs. A change only in suggestion kind, open reason, question answer, evidence wording, duplicate group or the second fact is not a gate disagreement. Missing required answers or changed claim IDs are invalid audit output; they are not agreements and cannot be dropped from the sample.

The tolerances count confirmed first-grader errors after reconciliation, not raw disagreements. Count a unit once within a stratum even if several mistakes affected it. If both graders are wrong, that is a first-grader error too. Also count a sampled first error found while examining an apparent agreement. Publish both the original disagreement rate and the adjudicated error count. Never relabel the original comparison after reconciliation.

At the full requested sizes, a passing result has the following conservative one-sided 95% binomial upper limits. The values use the worst passing count k. A result with fewer errors has a tighter limit.

| Strata | Passing count | Upper error-rate limit |
| --- | --- | ---: |
| `serious-credit` | 0/60 | 4.87% |
| `credit-caught`, `credit-missed`, `label-refuted` | At most 1/60 | 7.67% |
| `credit-unresolved` | 0/30 | 9.51% |
| `label-suggestion` | At most 2/60 | 10.13% |
| `label-problem`, `label-unresolved` | At most 1/40 | 11.32% |
| `label-unproven`, `label-minor-defect`, `label-outside-change` | At most 1/30 | 14.86% |

For example, zero serious-credit errors rules out a 5% or higher error rate at at least 95% confidence under the sampling model. It does not rule out 1%. One error in 20, the old general allowance, would only rule out rates above 21.61% at this confidence. That is why this proposal increases the sizes.

Compute each final finite-population upper limit by inverting the hypergeometric lower tail for the observed error count, actual N and m. For a census there is no sampling uncertainty. Also save the binomial comparison, which solves P[Binomial(n,p) ≤ k] = 0.05. SHA-256 ordering is treated as a uniform draw fixed independently of the grades. Correlated errors on one PR do not invalidate sampling without replacement from this fixed population. They do limit generalization beyond it.

These are per-stratum confidence statements, not a simultaneous 95% guarantee for all eleven. Report no overall error percentage. More fundamentally, the second grader and reconciler can miss shared errors. The bounds describe errors this assessment procedure can establish. Reading them as bounds on all true errors requires the unverified assumption that the procedure recognizes every sampled error. The audit does not measure the second grader's sensitivity. It cannot certify all grades as 95% accurate.

The second grader starts a fresh, isolated, neutral session for each selected batch. Use the same source revisions, task packet, execution allowance, answer key, rubric, rules and applicable approved rulings as the first grader. Include the same neutral status of pending cases available in the first grader's input. First-pass candidate proposals and other first-pass conclusions are not additional audit inputs. If a ruling changes those inputs, version the dataset rather than silently giving one grader a newer answer key.

Supply blinded full review text and the first grader's exact item kinds, claim IDs and quoted claim list. Supply no first outcomes, answers, known-problem links, reasons, duplicate groups or derived credit. The second grader cannot see setup identities, reviewer priorities, cost, impact bands, sample stratum membership, thresholds, scorecard results or earlier audit/trial answers. An applicable saved ruling is an explicit exception to independence of judgment, not a hidden grading hint. Report ruled and unruled claims separately.

The second grader labels every listed claim in every review of the batch. It checks each claim against every known problem, not merely the first grader's links. It uses the ordinary v2 verdict contract, including recommendations and the second fact where that contract requires them. It must not add, drop, merge, split or re-quote claims. It may flag bad splitting, omitted material or a wrong item kind in notes. Match results by the pinned review/item/claim identity and exact quote. Do not use the old overlapping-quotation matcher.

Derive D from the second answers by the same declared rule and compare selected units. For diagnostic first-fact comparisons, form every fixed claim × known-problem pair in the batch. Normalize an absent `known_problems` entry to `says_what = "no"`. Report the full grid and the subset where either grader records yes or cannot-tell, so the many joint negatives cannot hide a missed link. A first-fact disagreement that leaves D unchanged is diagnostic, not an additional detection failure.

Report these items without a numerical tolerance:

| Report | Reason it is not a pass criterion |
| --- | --- |
| Raw outcome confusion tables; suggestion kinds; unresolved reasons; four-question differences, including whether each question was reached | These explain where the rubric is read differently. Requiring identical intermediate answers caused noise in round one. Suggestion-kind changes do not change the displayed outcome rate. |
| First-fact agreement, added/missing problem links, caught-to-missed directions and serious results split by first credit state | Review-level credit is the declared detection unit. These diagnostics reveal cancellations and dilution within the serious stratum. |
| Ruled versus independently judged units; trial-task versus other-task results; PR/setup counts | Saved rulings and repeated trial tasks can inflate apparent agreement. Small subgroups cannot support their own promised error bounds. |
| Claim-list, not-a-finding and duplicate-group concerns | The owner fixed the claim list. This is not an independent audit of splitting or omitted claims. Scorecard reliability counts distinct groups per admitted review, whereas this sample checks recorded claim occurrences. |
| Causes of confirmed errors, candidate/link disputes and corrections outside the selected sample | Whole-batch work can find consequential errors outside the sample. They require action, but must not change its denominator. |
| Remedy inventory, sufficiency/safety discrepancies and advice-benefit concerns found during the normal verdict | Those dimensions have separate checks. This audit supplies no certification of remedy safety, benefit or control eligibility. |
| Exact selected batch count, sessions, failures, replacements, usage and coverage | These establish cost and scope, not grading correctness. |

Do not compare, score or publish a new agreement measure for `identifies_cause` or `cause_only` in this audit. Preserve their raw verdict fields only. P24 explicitly chose to keep recording the second fact without displaying or auditing it. The historical 315/450 agreement and weak self-repeatability explain that exclusion. Work on issue 78 needs a separate repeatability study and declaration.

The owner is the final reconciler. Someone may prepare evidence, but neither grader has the deciding vote on its own disagreement. Each selected gate disagreement gets an adjudication record with both original answers, pinned review wording, source or execution evidence, a reason and one decision: first correct, first wrong, both wrong, or undetermined. Use the repository's adjudication-record and shared claim-adjudication workflows. Group repeated questions for one ruling, then apply it to every equivalent unit. Do not reduce the error count by counting a repeated error as one incident.

A genuine unresolved question can be the correct grade. An unseen or newly adopted ruling is not retroactive proof of a first-grader mistake. Decide against the frozen contract and evidence, and record later changes separately. An undetermined gate disagreement prevents conclusion. Only the owner settles disputed credit, new eligibility, relied-on use, grouping or impact. New approved problems require regrading all reviews of that task, including earlier rejections and quiet reviews.

Freeze the first grades through comparison and the round conclusion. Save the original result before correcting anything. Fix every confirmed first error, even within tolerance or outside the sample. Version and remap affected batches rather than editing the frozen grades. A proven derivation-only bug may be repaired from saved facts with a versioned deterministic recomputation and the owner's recorded deviation, as in decision 2. A changed judgment requires new grading or a mapped saved ruling.

Two sampled errors sharing a cause trigger a check of every batch that cause could affect. A single proven systematic tool error does too. A gate stratum over tolerance fails the round. Reassess every batch containing its population, and widen the affected set when the cause crosses strata. A source-of-truth or rubric change also requires the affected regrade. Keep dependent measures provisional while this happens.

After repairs, draw all populated strata again from the corrected frozen grades with seed `evaluator-audit-v2-2026-10-11-round-2`, using the same sizes and tolerances. Use fresh second sessions. This proposal permits one such validation round, with a separately approved spend. It does not permit repeated draws on unchanged grades until one passes. A second failure returns to the owner for a newly declared plan. Preserve both failures and replacement costs; report confidence statements per frozen round, not as a claim about repeated attempts to pass.

A passing round still needs its confirmed errors corrected and the corrected snapshot linked before publication as audited. Corrections confined to the adjudicated sampled errors can retain that round's result with a correction ledger. Changes to the rubric, answer key, claim inventory or wider affected populations require the new round. A known unresolved systematic defect prevents publication even if the sampled counts fit the tolerances.

Before the draw, keep scorecards and grade-derived exports unopened. After the draw and before conclusion, any display is a preview that says evaluator audit pending or failed. Show unavailable values with their reasons; never turn unresolved credit into misses. Give no supported setup recommendation or audited badge. After a pass, the allowed description is: "Detection and claim outcomes passed evaluator audit v2 at its declared tolerances, conditional on the fixed claim inventory." Link the limits and correction ledger. It is not certification of claim extraction, duplicate grouping, remedies, impact calibration or the completeness of the answer key.

The scorecard port must preserve the rules in `docs/current-grading.md` and `src/lib/scoring.ts`. Detection uses scheduled trials, including terminal unadmitted zeroes, and reports equal-problem and equal-PR means separately for serious and other-material problems. The audit population contains admitted grades only; it does not audit delivery records. Reliability uses distinct claims per admitted review and the fraction of reviews containing each outcome. Its matched comparison averages per-PR rates only across sufficiently assessed shared PRs. Claim-occurrence audit bounds are not confidence intervals on those weighted scores, on "all serious caught", or on repeated serious misses. Publish grouping concerns and their fixes rather than implying that label agreement validates their denominators. Remedies, controls, advice, delivery, cost and time retain their own limits. The second fact stays hidden, and no blended score or winner rule is added. An audit pass does not override unresolved candidates, partial coverage or other existing recommendation blockers.

The tools need a substantive port before this plan can run:

- `evaluator_audit.py` must read v2, enumerate the declared populations, verify D across the complete cohort, emit the fixed claim list without first labels, validate exact identities, and compare the specified fields. It must record overlapping selections, actual census tolerances, exact finite-population bounds, full batch counts and diagnostic-only results separately. Preserve locks, immutable inputs, provenance, failures, evidence pins and verified workspace cleanup. Reconciliation must support both-wrong and discovered shared errors, corrections and superseded rounds.
- `calibration.py` and the audit schema must replace the hardcoded old stratum requirement with this versioned declaration. Validate every population definition, size, tolerance, seed, human receipt, empty/census state and evidence link. Check approved impact joins. Do not let the old plan's state or a diagnostic-only field satisfy a gate. Queue unfinished and failed dimensions explicitly, without implying an overall score.
- The export and scoring port must accept v2 outcomes directly and keep all metric arithmetic in `src/lib/scoring.ts`. Audit tooling must not invent replacement scorecard rates. Before dispatch, verify the ports with synthetic v2 cases, including an unrelated unresolved claim, a second-only problem link, shared-cause errors, exact-ID refusal, overlaps, tiny and empty strata, both-wrong adjudication and a changed input hash. No paid model is needed for these checks.

The maximum requested sample is 500 stratum memberships. Overlap and small populations reduce it. The exact session count is the union of selected `(run_path, target_id)` batches, at most 189. It cannot be known before grading. For scale only, 500 independent unit hits spread equally over 189 batches occupy about 176 batches: 189 × [1 − (188/189)^500]. Four hundred hits occupy about 166. Real strata have unequal concentrations and overlaps, so these are illustrations, not forecasts. Reserve up to one full 189-session second pass, roughly the first pass's cost under the stated per-batch assumption. Reconciliation, retries and a failed-round regrade are extra. Do not use the trial's cheaper Codex invoices to promise a cheaper audit. If the priced queue exceeds the approved reservation, retain the draw and request the additional authorization; do not trim the sample.

The owner must decide these items before grading:

1. Adopt the populations, seed, sizes and tolerances above. Recommended: yes, including the weaker stated limits for rare labels and the stricter serious check.
2. Fund up to 189 second-grader batch sessions, then price the exact drawn union before dispatch. Recommended: yes; authorize any repairs or second round separately.
3. Use Codex GPT-6.1 Sol at high effort, fresh sessions, and the first grader's fixed claim inventory. Recommended: yes. The fixed inventory is already the owner's decision; retain its limitation rather than reopening it here.
4. Accept the owner as final reconciler and the stated correction, failure and publication rules. Recommended: yes. Preserve P24's exclusion of the second fact.

Save the approval as a receipt and pin this proposal in the new declaration. The old audit's authority does not automatically approve this replacement.
