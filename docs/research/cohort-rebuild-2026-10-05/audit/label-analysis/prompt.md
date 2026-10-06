You are analysing why two graders disagreed, for a code-review benchmark. Do not modify any file in the repository, do not run any model and do not use the network. Write your report to `<cache>/label-analysis-2026-10-06/report.md` and a machine-readable table to `<cache>/label-analysis-2026-10-06/units.json`, then reply with one line.

## Background

The benchmark grades saved AI code reviews. A grader splits each review comment into claims and gives each claim one outcome: `eligible`, `advisory`, `inconsequential`, `scope-excluded`, `refuted`, `unsupported` or `unresolved`, plus four test answers (support, attribution, reachability, materiality). The rubric is `bench/rubric/scoring.md` and the grader's instructions are `bench/rubric/grader.md`.

An audit had a second, independent grader grade 100 batches again (`docs/evaluator-audit.md`). The comparison is `bench/grading/current/audit/evaluator-audit-v1-2026-10-03/comparison.json`. Its `units` list has one entry per audited unit with the first and second grader's result; `strata` has the agreement per stratum. The first grader's verdicts are in `bench/grading/current/grades.json` (and the assessment archives under `bench/grading/current/assessments/<run>/<target>/assessment-N/verdicts.json`); the second grader's are under `bench/grading/current/audit/evaluator-audit-v1-2026-10-03/second/<run>/<target>/assessment-1/`. The sample is `sample.json` beside the comparison. The saved review comments are `bench/runs/<run>/attempts/<attempt>/normalized.json`.

The two graders agreed closely on whether a known problem was caught. They disagreed widely on the five claim strata: `refuted` (3 of 20 agreed), `unsupported` (4), `advisory` (7), `other-below-threshold` (5) and `unresolved-claim` (1). The owner wants to change the rubric so that two careful graders give the same label.

The owner has already decided some changes (read these):

- `docs/research/cohort-rebuild-2026-10-05/second-pass/rulings/P8-buckets.md` and `P9-names.md`: `advisory` and `inconsequential` are replaced by "minor defect" (promised and delivered, with something wrong) and "suggestion or observation" (not promised; kinds: improvement, outside supported use, relied on and not promised). A problem is "promised: yes, delivered: no".
- `docs/research/cohort-rebuild-2026-10-05/second-pass/terms/two-questions.v6.md`: the two questions and their rules.
- `docs/research/cohort-rebuild-2026-10-05/second-pass/partial/two-facts.v1.md`: for a comment and a known problem, two recorded facts, "says what goes wrong?" and "says why?".

## What to find out

For every unit in the five claim strata where the graders disagree (and the ones recorded as unmatched):

1. Find both graders' claim records for that unit: the quoted text, outcome, four test answers and notes.
2. Classify the disagreement. Use your own categories where these do not fit:
   - the comment was split into claims differently, so there is no like-for-like claim;
   - same facts found, different label (say which pair of labels, for example advisory against inconsequential);
   - different facts found: one grader checked something the other did not, or they read the code differently (say what);
   - one grader left it unresolved for a reason the other did not see (say the reason, for example a pending candidate);
   - a mistake by one grader under the rubric as written (say which and why).
3. Say whether the decided changes above would make the two graders agree on this unit, and what they would both answer under them. Say when you cannot tell.

Then answer:

- Which pairs of labels account for most of the disagreement? Give the counts as a table of first label against second label.
- How much of the disagreement comes from splitting comments differently, and what rule would make splitting repeatable?
- How much comes from the line between `refuted` and `unsupported`? Between those two and the advice labels? Propose wording that would settle each line, with two or three real units as examples (quote the claim text and both graders' reasons briefly).
- Does anything in the grader's instructions or the output format cause disagreement on its own (for example a field that is free text where a fixed choice would do)?
- After the decided changes, what disagreement would be left, and what is the smallest further change to the rubric that would remove most of it?
- Which fields that graders record now are not used by anything a reader sees, and could be dropped to make the task smaller? (Look at `src/lib/scoring.ts`, `src/lib/data.ts` and `bench/tools/current_grading.py` for what is used.)

## Output

- `report.md`: plain words, short sentences, tables where they help. Start with a half-page summary. Do not name review configurations, models or runs in the prose; refer to a unit by its position in `units.json`.
- `units.json`: a list with one object per disagreeing or unmatched claim unit: `unit` (the id from the comparison), `stratum`, `first` and `second` (outcome and the four tests), `category`, `explanation` (one or two sentences), `under_decided_changes` (what both would answer, or "cannot tell"), `still_disagree` (true, false or null).
