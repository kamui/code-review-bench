# Task: name and define the two questions that sort a review comment

You are designing the core rule of a code-review benchmark's grading. Produce a proposal the repository owner can accept or reject. Do not change any file in the repository.

Repository (read-only): `<repo>`

## Background

The benchmark grades saved AI reviews of real pull requests. Each pull request has an answer key of reference problems (bands: serious, other-material). A correct review comment is sorted by two questions asked in order. Today they are called **Owed?** and **Lost?**:

| Owed | Lost | Outcome |
| --- | --- | --- |
| yes | yes | Problem, on the answer key (then banded serious or other-material) |
| yes | no | Minor defect |
| no | not asked | Suggestion or observation (an improvement, or something outside supported use) |

The owner accepted this structure today (decision P8). The owner then reviewed seven earlier "advice" rulings that two blind assessors had called problems, and changed six of them to problems. During that review the owner asked what "Lost" means, liked the definition offered ("a promised outcome did not happen for someone in supported use ... It does not ask whether anyone is shown hurt, how many, or how badly. That is the band's job"), and then wrote:

> I like that Lost definition, however, I'm not sure Owed and Lost are the best terms. Debate the names of these terms and also review the meanings based on all the evidence you have form me and previous rulings to inform the proper definition/rules.

## Evidence to read

All paths are under `docs/research/cohort-rebuild-2026-10-05/` unless they start with `docs/` or `bench/`.

- `second-pass/DISCUSSION.md`: the owner's statements verbatim, what decided each ruling, and what the reviews taught. Start here.
- `second-pass/buckets/SYNTHESIS.md` and `second-pass/buckets/two-questions.v1.md`: the accepted structure and the current wording of the two questions.
- `second-pass/buckets/blind-test/`: two assessors' labels on 46 cases, the key, and `notes-sol.md` and `notes-astra.md`, which list where the rule was hard to apply. Every `rule_gap` in the label files is a place the rule must now decide.
- `second-pass/rulings/`: rulings `01` to `10`, decision `P8`, and the six reviews `R1` to `R6` (each shows the owner's earlier ruling, what each agent picked and why, and the owner's final choice).
- `rulings/`: the 44 first-round rulings, nine band checks, and the rules the owner set (`P1` unusual input, `P4` older faults, `P5` partly kept promises, `P7` review-time information). Rulings 5, 10, 11, 16, 21 and 27 were changed by the reviews; read both.
- `docs/finding-threshold.md`, section "Rules the user set on 2026-10-05".
- `bench/rubric/scoring.md`: the four eligibility tests graders use now (support, change attribution, supported reachability, material consequence).
- `docs/research/impact-boundary-2026-10-04/impact-boundary.v4.md`: what separates serious from other-material. The band is a third, separate step and is not being redesigned here.
- Dossiers, if you need a case's facts: `candidates/<pull request>/dossiers/` (first round) and `second-pass/candidates/<pull request>/dossiers/`.

## What to produce

Write `proposal.md` in your working directory:

1. **Names.** A name for each of the two questions, and for the yes and no answer of each where that helps. Debate at least four alternatives for each question, including the current names, and say why each loses or wins. Say whether the three outcome names (problem, minor defect, suggestion or observation) should change. Names must read correctly to someone who has not seen the definition, must not overlap each other, must not collide with words the project already uses for something else ("eligible", "unsupported claim", "material", "serious"), and must work both as a question in a ruling and as a field in a record.
2. **Definitions and rules.** For each question: one sentence; then the rules that decide hard cases, in order of precedence. At minimum decide: a practice users demonstrably follow that is undocumented, against an interface its owner calls private or discourages; a statement in a pull request's description or code comment against narrower documentation; a general documented contract (a handbook) against behaviour specific to one component; an older fault the change leaves alone, exposes or makes fire sooner; evidence from after the merge; a resource cost of a sound fix; two promises that conflict; an outcome that fails once and works on retry; and a harmless extra message.
3. **Fit to the evidence.** Place every second-pass ruling (`01` to `10`), every review (`R1` to `R6`) and every first-round ruling whose outcome was advice or was later changed. For each: the two answers and the outcome under your rules, and whether it matches the owner's final ruling. List every case that does not fit and say whether the rule or the ruling should give way. Do not bend a rule to fit one case without saying so.
4. **What changes in the written rules.** Which of the owner's existing rules (unusual input, older faults, partly kept promises, documentation gaps, review-time information) your definitions restate, narrow or replace, and how the four tests in the grader rubric map onto your two questions.
5. **The line for a recommendation.** The exact wording an agent should use in the first line of a recommendation to the owner, so that the answer to each question is stated separately from the band.
6. **Rejected alternatives and the strongest objection** to your own proposal.

Also write `rationale.md`: a short note on what you considered and rejected.

Write for the owner: plain language, short sentences, a direct recommendation. Do not run any model (`claude`, `codex`), `grade.py` or `regrade.py`. Do not write anywhere but your working directory. Do not read `` outside your working directory, and do not read `docs/research/cohort-rebuild-2026-10-05/second-pass/terms/`.
