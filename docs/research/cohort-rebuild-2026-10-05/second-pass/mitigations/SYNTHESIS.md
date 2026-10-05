# The three mitigations, reviewed and rebuilt: synthesis, 2026-10-05

After the review of the nine the user asked for action on three lessons ([P10](../rulings/P10-undocumented-use.md)), the recording session acted in one commit, and the user asked: "How to address the last 3 questions and improvements to what we've done to mitigate so far". Two proposals were written independently and judged. This is what was taken from them and built.

## How it was made

- **Candidates.** [Candidate 1](candidate-1/proposal.md) by Claude Fable 5.1 and [candidate 2](candidate-2/proposal.md) by Codex GPT-6 Astra, both at high effort, each from [the task](task.md) alone. Candidate 1 came with working prototypes and 14 tests ([`candidate-1/prototype/`](candidate-1/prototype/)).
- **Judge.** Claude Opus 5.5 at high effort, against [six criteria](rubric.md), without being told who wrote which ([verdict](verdict.md)). Candidate 1 is the base, 27 points to 21. The recording session read both and agrees.
- **They converged** on every main finding: the first check tested that a form was filled and passed all five recorded failures; nothing bound the session that asks or a review of a saved ruling; the written rule said more than several rulings and was headed "Rules the user set" before the user had read it; and the list of cases that stay with the user could not be applied by a tool, because nothing recorded what each party answered.
- **Dropouts.** None.

## What was built

| Finding | Built | From |
| --- | --- | --- |
| The check passed `searched: ["x"]` and a "problem" over "no promise found" | [`bench/tools/ruling_dossier.py`](../../../../bench/tools/ruling_dossier.py): six named searches, each a query, a saved response, hits and read counts; the recommendation must follow from promise and delivery | Candidate 1's prototype |
| "Null means did not look" came back as a free `none` | A place is `checked`, `not-applicable` with a reason, or `blocked`; a blocked search cannot support "not promised" | Candidate 2, the judge |
| `duplicate` was exempt, though a duplicate that widens a family is exactly the pyOpenSSL case | A duplicate still needs its promise | Candidate 2, the judge |
| Old dossiers would be rewritten, losing the first recommendation | `refresh.json` and a supplement are laid over the first summary, which stays as written | Candidate 2, the judge |
| Nothing recorded each party's first answer | [`bench/tools/ruling_record.py`](../../../../bench/tools/ruling_record.py): a before-record written and committed before the user is asked, and an after-record that pins it | Candidate 1's prototype; the pin and `exposure` from candidate 2 |
| The cases that stay with the user were five sentences to remember | Computed from the record: assessors disagree, the recommendation differs from theirs, a named gap or conflict, no earlier ruling of that shape, a clause from one ruling or from the ruling in question or never tested, a change to a saved ruling | Candidate 1; "recommender against the assessors" from both |
| A recovery question with agreeing assessors printed "no recorded reason" | The kind of decision is a reason: recovery, grouping, band and control are the user's under ADR-0006 | Candidate 2 |
| A precedent decided under the old reading does not look like a changed ruling | A nearest ruling whose reading is `earlier` and that was not shown again is a reason | The judge |
| One ruling had three names, and the circle check compares names | [`terms/rulings.index.json`](../terms/rulings.index.json): one name per ruling, reviews as aliases, and the reading each was decided under | The judge |
| Which rulings a clause came from was known only by reading | [`terms/two-questions.v4.clauses.json`](../terms/two-questions.v4.clauses.json): 32 clauses, `from` apart from `tested_on` | Candidate 1; the split from candidate 2 |
| Nothing fails when a record is never written | A test: every second-pass ruling from the eleventh on has its before-record, and every saved record is sound | The judge |
| The surprise record said match or miss, not which way | A surprise against the first recommendation says whether it was more lenient or stricter | The judge |
| The rule overstated several rulings | [`terms/two-questions.v4.md`](../terms/two-questions.v4.md) and the section in [`docs/finding-threshold.md`](../../../finding-threshold.md), corrected clause by clause | Candidate 1's text; the cut-off, outside data and "built" from candidate 2 |
| The asking session was told to "find the promise yourself" | [`docs/claim-adjudication.md`](../../../claim-adjudication.md), "Prepare a ruling": do not ask on a failing directory; write the record before asking; the rule sentence goes in the option | Both |
| The diagnosis on file was wrong for one failure | Corrected in the discussion record and the checklist: in the pyOpenSSL case the quotation was in the dossier and was misread | Both noticed; the judge asked for the correction |

Version 4 keeps "the first rule that applies decides". Candidate 2 proposed weighing the clauses together; the ordered rule is the one that was tested, and a conflict between two rules is now recorded and sent to the user.

## Checked

- Bench tests pass with the two tools' 17 tests.
- Review 9, reconstructed as the first record ([`S9-ruling-30-R2a.before.json`](../rulings/S9-ruling-30-R2a.before.json)), gives seven reasons it stays with the user and four surprises, among them both blind assessors agreeing "through a clause written from this ruling; not counted as agreement".
- The check still refuses the five open candidate directories, which predate it.

## Not built, and why

- **A limit on the delegation policy** (text for ADR-0006 and a `decision_route` in the adjudication schema, from candidate 2). It is a policy version, so it is the user's to adopt, and nothing among the 13 open rulings is delegated.
- **A walk of every dossier directory in the bench tests.** It would fail on the directories written before the check; the record test covers what is asked from now on.
- **A backfill of records for the 63 earlier rulings and a summary command.** Issue 59.
- **An export of facts for the blind assessors.** The dossiers carry recommendations outside their Recommendation section; the export is built when the 13 are asked.
- **Excerpt and hash checking of each quotation** (candidate 2). Not worth its cost before the 13.

## What these still cannot do

They cannot make a query the right one, or make an agent read a quotation under the right question; they make both visible. They cannot prove a record was written before the answer beyond the commit order and the pin. Every clause is untested today, so the tool gives a reason on every case: that is the true state, and it means these 13 all come to the user. Candidate 1's own objection stands: the lean to "advice" did more damage than any missing search, and what corrected it was two blind assessors with a plain rule. The record is how the next round finds out which of the two to trust.

## For the user

1. **Read the rule text** ([P11](../rulings/P11-rule-text.md)): version 4 and the section in the finding threshold. Each sentence is the session's unless marked shown.
2. **The three rules that still turn on harm** (older faults, partly kept promises, unusual input): reword them to the two questions, or keep them and say which governs.
3. **The list of cases that stay with the user**: accept it as how questions are prepared; adopting it as a limit on delegation is a later, separate decision.
