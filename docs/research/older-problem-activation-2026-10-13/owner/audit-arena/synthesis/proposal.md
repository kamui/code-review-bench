# Proposal: the audit amendment for a regrade with older problems

For the owner's approval before any batch is graded under verdict contract v3. It amends the plan in force (thirteen strata, seed `evaluator-audit-v2-2026-10-12`, receipt `cohort-rebuild-audit.v3.md`). It authorizes no dispatch.

Facts are marked read (the repository at `32d52580`), run (computed or counted for this proposal) or reported (the owner's decisions of 2026-10-13, not yet filed).

## 1. What stays

The thirteen strata, their sizes and allowed errors, the seed, the second grader (Codex GPT-6.1 Sol, high), the fixed claim list, the reconciliation rules, the failure rule and the redraw seed all stay as declared (read).

Two things about them change without any decision:

- Their populations come from the new grades and the amended answer key of 71 known problems (reported). Keeping the seed fixes the order of units, not which units exist.
- The `outside-this-change` claim stratum (30 units, 1 error allowed) now covers claims that can carry older-problem credit.

## 2. What is added: three strata

An `older-review` unit is one admitted review and one admitted older problem of its task. Its id is `<run>/<attempt>#older:<problem>`. Its result is derived from the review's claims, as mapping derives it (read, `older_grading.assessment`):

- `raised` when a claim has `says_what` "yes" for the problem;
- otherwise `unresolved` when one has "cannot-tell";
- otherwise `not-raised`.

| Stratum | Population | Size | Allowed errors | A pass rules out an error rate above |
| --- | --- | ---: | ---: | ---: |
| `older-raised` | Older-review units whose result is `raised` | all | none: checked whole | no rate claimed |
| `older-unresolved` | Older-review units whose result is `unresolved` | all | none: checked whole | no rate claimed |
| `older-not-raised` | Older-review units whose result is `not-raised` | 60 | 1 | 7.7% |

Why:

- **Every raised credit is checked.** A raised credit is the number the reader sees. The population is small: comments from 16 reviews were linked to the first five candidates (reported), and seven or eight problems may be admitted. A fixed size of 30 with no error allowed would turn into a sample the moment the stratum held 31 units, and one error would then fail the whole round. Checking all of them costs little, because the second grader works by batch and those batches are graded twice anyway.
- **Every unresolved credit is checked**, for the same reason; the stratum will hold a few units or none.
- **Not raised is sampled at 60 with 1 allowed**, the rule the plan uses for `missed`. A wrong "not raised" hides a credit. It costs no review a miss.

A stratum checked whole has no tolerance, as the plan already says: every confirmed error is corrected and counted, and no rate is claimed. The 7.7% is the one-sided 95% binomial bound at the largest passing count (run). The three strata are not a joint guarantee with the thirteen.

Populations, if every candidate now known is admitted (reported counts of admitted reviews; run):

| Task | Admitted reviews | Candidates | Units |
| --- | ---: | ---: | ---: |
| `x-kubernetes-141463` | 36 | 1 | 36 |
| `s-seaweedfs-10735` | 41 | 3 | 123 |
| `o-astro-16079` | 40 | 1 | 40 |
| `j-trpc-5017` | 40 | 1 or 2 | 40 or 80 |
| `p-hono-5067` | about 40 | 1 | about 40 |
| All | | 7 or 8 | about 279 or 319 |

How they split into raised, not raised and unresolved is not known before grading.

## 3. Checked over the whole cohort, with no sample

The three checks in force stay. A fourth is added:

4. For every admitted review and every older problem of its task there is exactly one saved result, and it equals the derivation in section 2. No claim names both an ordinary known problem and an older problem. No credit rests on a candidate that is still pending or on a claim that is not `outside-this-change` or unresolved for want of evidence.

The validator already enforces most of this when the grades are loaded (read, `current_grading.py`). Declaring it makes it a named check whose result is saved before the draw.

## 4. The draw waits for the candidates

Under contract v3 a grader records an `older_candidate` for any true older fault that is not in the catalog (read). Admitting one changes what its task's graders are given, so that task's batches are graded again.

So: **the sample is drawn only when no older-problem candidate is pending.** Each candidate the regrade raises is ruled on first, the batches of a task with a new admission are graded again, and the draw follows. The pass then describes one frozen catalog.

A fault admitted after the round concludes is outside that round. Its task's batches are graded again, every raised and unresolved credit for it is checked by a second session before its counts are shown, and the pass sentence is not extended to it.

## 5. The second grader

Unchanged in form: a fresh isolated session for each sampled batch, with the task packet, the rule text, both catalogs and the saved rulings the first grader had, and the first grader's claim list. It gets no outcome, no fact, no link, no credit, no reviewer identity, no stratum membership and no impact label.

One sentence is added to its instructions: when the review text states an older fault that the claim list does not record, note it as a concern about the list, with the evidence. Such notes are reported and are not counted as errors, as concerns about the list are today.

## 6. Disagreements and errors

A disagreement in the new strata is a different older-review result. The plan's rules apply unchanged: tolerances count confirmed first-grader errors after the owner's reconciliation, both graders wrong counts as a first-grader error, an undetermined disagreement holds the round open, and every confirmed error is corrected by grading the batch again.

Not a disagreement: a difference only in `identifies_cause` (recorded, neither compared nor shown, as decision P24 says); a different candidate id for the same fault.

## 7. Reported without a tolerance

Added to the plan's list:

- Each grader's `says_what` for every claim and older problem, and the claims where they differ, including a second claim on a credit already raised.
- For every `outside-this-change` claim in a second-graded batch: both graders' older-problem entries and candidate referrals, side by side.
- Older-problem candidates only the second grader raised. Each goes to the owner as a candidate like any other. It is not by itself a first-grader error.
- The second grader's notes of older faults missing from the claim list.
- Older credit results by problem, by task and by impact label.

## 8. What may be said after a pass

"Detection, claim outcomes, control silence and older-problem credit passed evaluator audit v2 at its declared tolerances, for the fixed claim list and the older problems admitted when the sample was drawn. Every raised and unresolved older-problem credit was checked in full."

A pass does not show that the catalog holds every older fault, that the claim list is complete, or that an impact label is right. Labels are owner decisions with a blind inspector's label and are checked through impact calibration, not here.

## 9. Cost and effort

- **Sessions.** The older units come from the batches of the affected tasks: 64 if all five tasks gain an older problem (run from the reported batch counts). The second pass was already planned as nearly a full pass of 198 batches, about $100 at list price with $200 reserved (read, the receipt in force). The older strata add at most the batches the thirteen would not have touched: at most 64, about $32 at $0.50 a batch, and probably far fewer. The reserve stays $200.
- **Regrading after an admission.** Each candidate admitted after the regrade sends its task's 9 to 14 batches through the first grader again.
- **The owner's time.** The receipt in force expected 20 to 45 disagreements. The new strata add up to about 100 compared units (60 sampled, and the raised and unresolved ones); at the trial's credit disagreement rate of about 1% that is a handful more. The larger load is the candidates graders raise, each of which needs a prepared question.

## 10. What the tools need first

- `audits.json` and `docs/evaluator-audit.md`: the three strata, the fourth check, sections 4 to 8, the new receipt. The schema already accepts a size of "all" (read) and requires an integer `max_errors`, which the conclusion ignores for a whole stratum (read); declare 0.
- `evaluator_audit.py`: the fourth check among the whole-cohort checks; the comparison's report of older `says_what`, candidate referrals and the side-by-side list; a refusal to draw while an older-problem candidate is pending.
- The second grader's instruction sentence of section 5.
- Tests for each.

No new unit kind, no new stage for the second grader and no schema change.

## 11. Declared with what in view

The plan in force was declared before any grade existed. This amendment is declared before any grade under contract v3 exists, but not blind: a first regrade under contract v2 is saved, its aggregate outcomes are known (53 claims labelled outside this change), and the proposer knows that comments from 16 reviews were linked to the first five candidates (reported). The declaration's `basis` says so in those words.

## 12. For the owner to approve

1. The three strata: `older-raised` and `older-unresolved` checked whole, `older-not-raised` at 60 with 1 error allowed.
2. The fourth whole-cohort check.
3. The draw waits until no older-problem candidate is pending, and a later admission is outside the concluded round.
4. The added sentence in the second grader's instructions and the additions to what is reported without a tolerance.
5. The seed kept, the `basis` wording of section 11, and the sentence of section 8.
