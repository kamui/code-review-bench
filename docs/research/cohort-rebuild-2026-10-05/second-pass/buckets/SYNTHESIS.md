# What the buckets should be: synthesis, 2026-10-05

The user asked, after second-pass ruling 9, whether "advice" and "problem" are named and divided correctly ([the statements](../DISCUSSION.md#the-users-statements)). Two proposals were written independently and judged. This is the synthesized proposal, with a blind test of its central rule. Nothing here is adopted until the user rules.

## Proposal

**The answer key does not change.** A pull request's reference problems keep two bands, serious and other-material. No third band is added.

**Advice is split, and one weak distinction is merged.** A correct comment that does not catch a reference problem gets one of two labels:

- **Minor defect.** Something the project owes its users is wrong, and nothing promised is lost. Ruling 9 is the anchor: an error line at every shell start, completion still loads and works.
- **Suggestion or observation.** Nothing owed is failing. It records which kind: an *improvement* (nothing is wrong; this would make the change better) or *outside supported use* (something does stop working, but only where the project owes nothing, as in ruling 2).

Today's `advisory` and `inconsequential` both go into these two. The audit's two assessors could not tell them apart: of ten claims the first called inconsequential, the second called seven advisory.

**Two questions decide it**, asked in order of a comment whose facts are established ([`two-questions.v1.md`](two-questions.v1.md)):

1. **Owed?** Did the change fail something the project owes the people who use it in a supported way?
2. **Lost?** Is a promised result wrong, missing or blocked, and not merely untidy?

| Owed | Lost | Outcome |
| --- | --- | --- |
| yes | yes | Problem, on the answer key |
| yes | no | Minor defect |
| no | not asked | Suggestion or observation |

Rarity decides neither question. A valid option few people set is supported use; a popular trick on a private interface is not.

**Names shown to a reader.** "Reference problem" for a comment that catches one (stored as `eligible`), "Minor defect", "Suggestion or observation", "Outside review scope" (`scope-excluded`), "Refuted claim", "Unproven claim" (`unsupported`, renamed in display because "unsupported claim" and "unsupported use" mean different things), "Unresolved claim".

**Scoring.** A minor defect and a suggestion or observation are counted per review, separately, and cost a silent review nothing. Neither enters a detection rate, earns detection credit or counts as a false claim. A count is not a measure of usefulness; benefit is still established by its own inspected sample. Every other measure is unchanged. [Candidate 2](candidate-2/proposal.md#scoring-and-the-cost-of-silence) has the measure-by-measure table.

**What other-material already means.** The saved boundary says an other-material problem "earns credit when a review raises it, but it does not have to be raised. Shipping without the implementer ever hearing of it is acceptable." A missed one still lowers the other-material detection rate. So the answer key is not a list of what a reviewer must catch; only the serious band is. The key is also a curated list that grows by ruling, not an exhaustive inventory, and that limit should be stated beside every detection rate.

## The blind test

Two models from one vendor, Codex GPT-6.1 Sol and GPT-6 Astra at high effort, each applied the two questions to 46 real cases without seeing an outcome: the dossier text of 23 cases the user ruled a problem and 23 the user ruled advice, shuffled ([prompt](blind-test/prompt.md), [key](blind-test/key.json), labels for [Sol](blind-test/labels-sol.json) and [Astra](blind-test/labels-astra.json)). Three cases are set aside as unfair: one dossier serves two rulings with opposite outcomes (S3), and one ruling was changed by a live check made after its dossier was written (A2a). That leaves 43.

| | Result |
| --- | ---: |
| The two models gave the same outcome | 41 of 43 |
| Sol matched the user (problem or not) | 35 of 43 |
| Astra matched the user | 34 of 43 |
| Cases the user ruled a problem, both said problem | 20 of 21 |
| Cases the user ruled advice, both said not a problem | 14 of 22 |
| Both models confident and agreeing | 23, of which 20 matched the user |

What it shows:

1. **The rule can be applied the same way twice.** Two models agreed on 41 of 43. That is far better than the audit's agreement on the present labels.
2. **It does not yet reproduce the user.** Both models called a problem seven cases the user ruled advice, and called an observation one the user ruled a problem. The rule is consistent and it draws the line in a different place.
3. **Each of the seven has a reason in its saved ruling that the rule does not state.**

   | Ruling | The user's ground for advice | Missing from question 1 |
   | --- | --- | --- |
   | [5](../../rulings/05-H3.md), Hono, uploads need more memory | A deliberate, sound fix with a cost the author may not have known; "not necessarily a bug" | A resource cost of a sound fix is a tradeoff to point out, not a failed obligation |
   | [10](../../rulings/10-N1.md), ripgrep, `source _rg` from its own directory | Fails as it did before; not a documented step | An older failure the change leaves as it was, on an undocumented path |
   | [11](../../rulings/11-N2.md), ripgrep, a renamed completion file | A regression, but no user is shown doing it | The unusual-input rule: a practice nobody is shown following |
   | [16](../../rulings/16-A2b.md), Astro, unchecked path value | The harm came six months later from the hosting platform's change | Review-time information: harm that needs a later outside change |
   | [21](../../rulings/21-B1b-B6.md), Base UI, `details.cancel()` (two cases) | The behaviour follows the stored value by design; nobody shown hurt | Behaviour that follows the documented design |
   | [27](../../rulings/27-B8.md), Base UI, combobox validates label text | An older fault; the change only shows the false error sooner | An older fault the change does not make worse |

   The one the other way is [ruling 22](../../rulings/22-B2.md) (GT-r3): both models read the new required error as following the announced behaviour.
4. **The minor-defect bucket is small.** Of the 22 advice cases, both models put one there (ruling 9) and Sol one more. The rest of the user's advice is a suggestion or observation: about eight outside supported use, five improvements, and the seven above.
5. **Confidence is not yet a guide.** Where both models were confident and agreed, 3 of 23 still differed from the user. This bears on [issue #59](https://github.com/kamui/code-review-bench/issues/59).

The models share a vendor, the cases are the user's own past rulings, and the rule was written after reading them. The test shows the rule is applicable, not that it is right.

## What the user decides

1. **The structure:** two bands unchanged, advice split into minor defect and suggestion or observation, `advisory` and `inconsequential` merged into them.
2. **The seven.** Either question 1 is sharpened with the six grounds in the table, so the rule follows the rulings, or any of the seven is changed to a problem, so the rulings follow the rule. The first keeps every saved ruling. The second makes the rule simpler and the answer key longer.
3. **Timing.** The rubric change rides with the already-open relabel of the comment outcomes and its single regrade of all 199 batches. Until then each advice ruling records its kind, as [the discussion record](../DISCUSSION.md#open-the-buckets) does.

Still open after that, from [the judge](verdict.md): the audit plan samples `inconsequential` and needs new strata for the new labels; a calibration threshold has to be declared before the regrade is paid for; and the sharpened rule should be tried on cases it was not written from. The 13 rulings left in this pass are the first such cases.

## Synthesis note

- **Candidates.** [Candidate 1](candidate-1/proposal.md) is the recording session's two earlier answers to the user (Claude Opus 5.5). [Candidate 2](candidate-2/proposal.md) was written by Codex GPT-6 Astra at high effort from [the task](task.md) alone. Both chose to split advice and rejected a third band; that convergence is the main result.
- **Judge.** Codex GPT-6.1 Sol at high effort scored both against [six criteria](rubric.md) without being told who wrote which ([verdict](verdict.md)). It chose candidate 2 as the base, 27 points to 18. The recording session read both and agrees.
- **Base: candidate 2.** Its minor defect is narrow (a supported failure with no material loss), it separates unsupported evidence from unsupported use, it handles a comment that is partly a minor defect and partly refuted, and it states every measure and the migration.
- **Grafted from candidate 1.** The one-line framing that this splits optional feedback and adds no band; the side-by-side of what ruling 9 costs a silent review under each option; recording now why each advice ruling fell short, so the migration asks the user nothing twice.
- **Rejected from candidate 1.** Counting breakage in unsupported use as a minor defect, and calling a minor defect "useful" on its label alone. The judge also corrected its claim that a problem "should have been corrected before merge"; the other-material definition says otherwise.
- **Added in synthesis.** The two questions as one rule; the "Unproven claim" display name; the blind test and its findings. Candidate 2's migration steps and its fallback (record defect status as a fact inside one optional-feedback label, if assessors cannot hold the new boundary) stand as written there.
- **Corrections to candidate 2.** Ruling 30's bundle name is documented as a value to read; reassigning it is what is undocumented.
- **Dropouts.** None.
- **Verification.** The blind test above. It confirmed the rule is applicable and found the gap in item 3, which the proposal now puts to the user instead of papering over.
