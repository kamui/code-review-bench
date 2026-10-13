# The proposed rule text, for the owner's read

Prepared 2026-10-13. The three proposed files are not in force and this page adopts none of them. It shows every sentence that differs from the file in force, in reading order, with one line on what each difference does. The proposed text is quoted as it stands. Suggested wording is kept apart, at the end. Inside a quotation a link is shown as its text, or as (…) where the target is long.

All of it is read: the two files of each pair were compared with `git diff --no-index`, and the proposed files with the text as it stood before version 9 (commit `b25a00a9`).

| Proposed file | File in force | Sentences added | Removed | Changed |
| --- | --- | ---: | ---: | ---: |
| [`rules.older-problems.v1.md`](../../../bench/rubric/rules.older-problems.v1.md) | [`rules.next.md`](../../../bench/rubric/rules.next.md) | 7 | 11 | 1 |
| [`scoring.older-problems.v1.md`](../../../bench/rubric/scoring.older-problems.v1.md) | [`scoring.next.md`](../../../bench/rubric/scoring.next.md) | 19 | 11 | 4 |
| [`grader.older-problems.v1.md`](../../../bench/rubric/grader.older-problems.v1.md) | [`grader.next.md`](../../../bench/rubric/grader.next.md) | 21 and two fields | 0 | 0 |

## The first thing to know

**The proposed rules and rubric were written from the text before version 9.** They do not hold what the owner accepted on 2026-10-13 (decisions P30, P31 and P32). Put in force as they stand, they would take back:

- the two examples of rule Before 2 from regrade rulings 2 and 3, and the reworded tRPC example;
- the rubric's two examples of question 2 that follow "changed lines only";
- the rubric's bullet "Showing the failure counts as saying it", with its three examples. That bullet is the credit line the owner set.

Compared with the text before version 9, the proposed rules differ in 7 added and 3 removed lines, and the proposed rubric in 13 added and 3 removed. Those are the older-problem additions alone. The grader file in force did not change in version 9, so the proposed grader file has no such gap.

The recommendation is not to read the proposed rules and rubric as they stand. Read a text that is version 9 with the older-problem sentences added. "Suggested wording" below lists exactly which sentences that is.

## 1. The rules

In reading order.

**1.1 The opening paragraph.** Status only.

- Before: "This is the graders' copy of [the two questions, version 9](…), the text the user read rule by rule. The rules are unchanged. Only the introduction is left out. It is in force. The validation policy pins it."
- After: "Proposed complete next rule text for issue #135. This copy is not in force. The owner must read and adopt it; the validation policy continues to pin rules.next.md."
- What it does: marks the copy as a proposal. A grader would read "This copy is not in force" in the rule it is told to apply, so this paragraph has to change again at activation, and the file's hash with it.

**1.2 Rule Before 2, the sentence on renames.** Moved and reworded.

- Before: "A line the change rewrites only to rename something, with no change in what it does, is not one of those lines." It stands between the two sentences on the lines the change touches.
- After: "A line the change rewrites only to rename something, with no change in what it does, is not a line the change touches." It stands in a paragraph of its own, after them.
- What it does: nothing to a grade. The owner accepted the first wording in version 9. The second is the sentence as regrade ruling 2 recorded it.

**1.3 Rule Before 2, two sentences added around "If the change does none of these".**

- Before: "If the change does none of these to an older fault, the fault is outside the review."
- After: "The introduced, worsened, reachable and exposed routes above still apply. If the change does none of these to an older fault, the fault is outside the review. An admitted older problem may earn separate credit under the older-problem policy; it creates no ordinary recall opportunity."
- What the first added sentence does: says the narrowing to changed lines leaves the other routes of the rule as they were.
- What the second does: tells a grader that an older fault outside the review can still earn credit, and that staying quiet about it is not a miss.

**1.4 Rule Before 2, the examples.** Three removed, one note added.

- Removed: "*Example:* tRPC's optional-field fault failed before the change too. The change rewrote the lines of the type helper that hold it, so it belongs to the review (second-pass 7)."
- Removed: "*Example:* SeaweedFS's listing skips a dead name without fetching another. The change adds one line above that skip and leaves the skip as it was, so the short page is outside the review (regrade 3)."
- Removed: "*Example:* Kubernetes' message cut can split a character. The change renamed the limit on that line and changed nothing it does, so the cut is outside the review (regrade 2)."
- Added: "*Reconciliation required before activation:* tRPC's optional-field ruling used editing the type helper as its scope reason (second-pass 7). Check the fault's lines and every other responsibility route before changing that saved decision."
- Kept: the Base UI example (first-round 27).
- What it does: a grader loses the two examples that show "outside the review" under version 9, and reads a note addressed to whoever activates the text. The check the note asks for is done: see [the reference check](reference-check.md) and [question 6](rulings/06-trpc-optional-field.md).

Nothing else in the rules differs: Before 1, Before 3, Before 4, both questions and the three outcomes are word for word the text in force.

## 2. The rubric

In reading order.

**2.1 The title.** "# Scoring rubric, next version" becomes "# Scoring rubric with older-problem credit, proposed". Status only.

**2.2 The opening paragraph.** The first sentence, which lists decisions P16 to P22, is unchanged.

- Removed: "On 2026-10-13 the user accepted the three examples of question 2, which follow version 9 of the rules ([decision P30](…)), and the bullet "Showing the failure counts as saying it" under the first fact of section 3 ([decision P31](…), [decision P32](…))."
- Removed: "It is in force."
- Added: "Those approvals cover the inherited rubric." and "This proposed copy adds section 7 and is not in force."
- Changed: "The validation policy pins this text, and `scoring.md` is the earlier rubric." becomes "The active validation policy still pins scoring.next.md, and `scoring.md` is the earlier rubric."
- What it does: status. It also drops the record of the owner's acceptance of 2026-10-13.

**2.3 The pointer to the rules.** "In the repository they are `rules.next.md`." becomes "In the repository they are `rules.older-problems.v1.md`." It follows the file name.

**2.4 Question 2, the examples.** One changed, one removed. **This changes what a grader does.**

- Before: "*Responsible:* a fault that failed before the change too and sits on a line the change rewrote."
- After: "*Responsible:* a fault that failed before the change too and sits in the helper the change edits."
- Removed: "*Outside:* an older fault on an unchanged line beside the edit, which behaves the same before and after."
- What it does: returns question 2 to the reading before version 9, under which an older fault anywhere in an edited helper belongs to the review. A grader who followed these examples, and not the rule they stand beside, would answer "this change: yes" for the Astro query string and the SeaweedFS short page.

**2.5 Section 3, "Says what goes wrong?".** One bullet with three examples removed. **This changes what a grader does.**

- Removed: "**Showing the failure counts as saying it.** A claim that quotes the code which now fails, beside the input that makes it fail, says what goes wrong, though it does not spell out the result. So does a claim that states the effect in plain words, such as a setting that "is ignored" or a check that is missing for untrusted callers."
- Removed: "*Credit:* "`validate` now receives '5' (was 5 on main)", beside the validator `typeof v === 'number' && v > 3 ? null : 'err'`. The reader sees the validator refuse a value it accepted."
- Removed: "*Credit:* "ssl_version/ssl_minimum_version are ignored by urllib3 once any ssl_context is supplied.""
- Removed: "*No credit:* the rewrite "buffers plain `Request` bodies fully in memory where they used to be handled by the platform." It says what the code now does, and not that memory use rises or that anything fails."
- What it does: a grader would again refuse credit to a comment that shows the failing code beside its input without spelling out the result. The proposed text says credit for an older problem follows the same fact, so the loss would reach older-problem credit as well.

**2.6 Section 7, added whole.** **This changes what a grader does and what the scorecard shows.**

> ## 7. Proposed separate older-problem credit
>
> This addition is dormant until the owner adopts this complete policy and rules.older-problems.v1.md. The v3 grader and verdict contract activate together with those pins; no current judgment changes by installing support.
>
> An admitted older problem satisfies the existing Promised? and Delivered? tests without change responsibility. Intent is evidence, neither a mandatory nor a sufficient test. A claim stays outside-this-change. Whether its own words say what goes wrong determines credit; record whether it identifies the cause as a fault alongside it. Admission and grouping remain owner decisions, with no inherited delegation authority.
>
> Assess each catalog entry per admitted review as raised, not-raised or unresolved. One review/problem earns one credit, regardless of duplicate comments. A not-raised assessment creates no miss or denominator. A catalog containing only older problems does not change an applicable clean-control audit. Independent false claims, unresolved claims, unsafe remedies and pending candidates retain their ordinary effects.
>
> Report counts, observed counts and assessment coverage separately. Final catches are unavailable with no admitted reviews, no catalog, missing selection or assessments, pending trials, unresolved credit or pending older candidates. Catalog counts remain available. Complete zero-catch assessment is zero without penalty. Never report a share of all older faults or combine credit with recall, impact, ranking or recommendations.

What each paragraph does:

- The first: says the section is not in force. Like 1.1, it has to change at activation.
- The second: gives the test for admitting an older problem, says what part intent plays, keeps the claim's ordinary label, ties credit to the first fact and records the second, and keeps admission and grouping with the owner.
- The third: sets the unit of credit, says silence is no miss, keeps a clean control clean, and keeps every other effect of a review's claims.
- The fourth: says what is reported and when a final count is withheld.

## 3. The grader instructions

Nothing is removed or changed. Two fields are added to the example of a claim, and one section is added at the end.

**3.1 The claim's fields.** After `"known_problems": [],` the example gains `"older_problems": [],` and `"older_candidate": null,`.

**3.2 The section added.**

> ## Older problems under current-verdicts/v3
>
> The separate older_problems catalog in references.json contains only owner-admitted problems. It is not an ordinary answer key. Truth still comes first. For a true outside-this-change claim, older_problems lists entries with exactly problem (the catalog ID), says_what, identifies_cause (each yes, no or cannot-tell), and reason. Record both facts from the comment's own words. Cause-only wording earns no catch. Do not change the ordinary outcome or answer previously unreached ordinary questions.
>
> A claim may name several older problems but cannot also name an ordinary known problem. Split independently false allegations that the PR introduced a fault, and assess remedies independently. A harmful remedy never erases a correct catch.
>
> For an unlisted true older fault, set older_candidate to a new candidate ID. Explain its supported obligation, trigger, mechanism and undelivered outcome in the candidate evidence. A true observation about old code is insufficient. Leave older_problems empty while admission is pending. All new_candidates have kind, either ordinary or older-problem, in addition to the fields above. Ordinary candidate remains null for a resolved outside-this-change assertion. Never convert a scope ruling to admission or award pending candidates credit.
>
> Honor single-comment older credit rulings in claims.md. Evidence access failures leave the claim unresolved and earn no credit. When the inaccessible evidence concerns a listed older problem, retain its older_problems entry with says_what cannot-tell so its credit assessment stays unresolved. Independent grouping or admission questions remain for the owner under the shared adjudication workflow.

## 4. What a grader would record differently under the v3 format

Read from the proposed grader text and from the validator, `bench/tools/older_grading.py`.

1. Every claim carries two more fields, `older_problems` and `older_candidate`, also when they are empty and null. The validator refuses a claim without them.
2. A true claim whose outcome is outside-this-change gets one `older_problems` entry for each admitted older problem it bears on: `problem`, `says_what`, `identifies_cause` and `reason`. Both facts come from the comment's own words.
3. That claim's ordinary answers do not move: `this_change` stays "no", the outcome stays "outside-this-change", and `promised` and `delivered` stay null.
4. A claim that names an older problem names no ordinary known problem and no ordinary candidate. A comment that says both is split into two claims.
5. A true older fault that is not in the catalog gets `older_candidate`, a new candidate id, and an empty `older_problems` list. The claim stays resolved: its outcome is outside-this-change and its ordinary `candidate` stays null. Under v2 a candidate could hang only on an unresolved claim.
6. Every entry of `new_candidates` gains `kind`: "ordinary" or "older-problem". That is a new field on ordinary candidates as well.
7. The evidence for an older candidate has to state its "supported obligation, trigger, mechanism and undelivered outcome".
8. When the evidence for a claim about a listed older problem cannot be reached, the claim is unresolved with `true` "cannot-check" and keeps its `older_problems` entry with `says_what` "cannot-tell".
9. Where `claims.md` holds a ruling on one comment and one older problem, the claim records the ruled facts.
10. A comment that truly points at an older fault and falsely says this pull request introduced it becomes two claims: one outside-this-change, one refuted.

A grader does not record the result for a review and a problem. Mapping derives it: raised when a claim says what goes wrong, unresolved when one cannot tell, otherwise not raised.

Unchanged: `known_problems` and its two facts, the table of outcomes, `duplicate_group`, `canonical_claim_id`, `link_disputes`, recommendations, safety and the remedy inventory.

## 5. Conflicts

**With decisions the owner has made.**

1. Rubric 2.5 removes "Showing the failure counts as saying it" (decision P31). The owner's credit line of 2026-10-13 is that a comment which shows the failing code beside its input, or states the effect in words close to the problem, says what goes wrong.
2. Rubric 2.4 returns the examples of question 2 to "in the helper the change edits" (against decision P30, "changed lines only").
3. Rules 1.4 removes the two examples from regrade rulings 2 and 3, which the owner accepted in version 9 (decision P32).
4. Rubric 2.2 drops the sentence that records those acceptances.
5. Section 7 says "Intent is evidence, neither a mandatory nor a sufficient test." On the Kubernetes cut the owner said: "i dont know if it's a bonus. It depends on whether this cut was intentional or not." The owner then answered that question for the one case. The proposed sentence replaces the owner's test with a different one. It may be the better rule; the owner has not been asked.

**With version 9's accepted text.**

6. Rules 1.2 words the rename sentence differently from version 9. The meaning is the same.
7. Rules 1.4 replaces the tRPC example with a note. Version 9 states as fact that "The change rewrote the lines of the type helper that hold it". The diff shows the copy that holds the fault unchanged (read; see question 6 in `rulings/`). So version 9's example needs correcting whatever happens to the proposal, and the proposed note is right to doubt it. A note to the activator is still not a rule a grader can apply.

**Between proposed sentences.**

8. Rules 1.2 and 1.3 say "changed lines only". Rubric 2.4, in the file that points to those rules, says a fault "in the helper the change edits" belongs.
9. The grader text says "Each new_candidates entry has exactly id, claim, evidence, limits, relevance, would_settle and items." Its new section says "All new_candidates have kind … in addition to the fields above."
10. The grader's new section says "For a true outside-this-change claim, older_problems lists entries". Its last paragraph keeps an entry on an unresolved claim.
11. The rules say "This copy is not in force" and the rubric says "is not in force" and "dormant". [The proposed policy](../issue-135-bonus-credit-plan-2026-10-13/validation-policy.proposed.json) pins those very bytes. A policy that pinned them would put in force a text that says it is not. The pins of the proposed policy cannot be the pins of the activation.
12. Section 7 says "Admission and grouping remain owner decisions". [The contract page](../../older-problem-credit.md) says admission, credit and grouping all stay with the owner. The rubric leaves credit out.

## 6. Terms used without a definition

| Term | Where | What is missing |
| --- | --- | --- |
| "older problem", "older fault" | rules 1.3, section 7, grader | No sentence says what one is. Rule Before 2 speaks of "an older fault" without defining it. See undecided point 1. |
| "admitted", "admitted older problem" | rules 1.3, section 7 | Who admits and by what record is only on the contract page. |
| "the older-problem policy" | rules 1.3 | Names no file or section. |
| "ordinary recall opportunity" | rules 1.3 | "Recall" is not a word of the rules or the rubric. |
| "exposed" | rules 1.3 | The bullets above it say "makes it reachable, or makes it show sooner or more clearly". The rubric in force uses "exposes it" for the same thing. |
| "satisfies the … tests" | section 7 | The rule text asks two questions with three outcomes. "Satisfies" has to be read as promised yes and delivered no. |
| "without change responsibility" | section 7 | It does not say which rules are set aside: Before 2 only, or also the parts of "Promised?" that speak of what the change says. |
| "catalog", "catalog entry" | section 7, grader | Defined on the contract page only. |
| "admitted review" | section 7 | The same word "admitted" for a review and for a problem. |
| "credit", "catch", "final catches", "observed counts", "assessment coverage" | section 7 | Five reporting words, none defined in the rubric. |
| "pending trials", "missing selection" | section 7 | Scorecard words, not defined in the rubric. |
| "applicable clean-control audit" | section 7 | Does not say when an audit stops being applicable. |
| "ordinary effects" | section 7 | Does not say which effects a pending candidate has. See undecided point 6. |
| "cause-only wording" | grader | The rubric calls this a claim that identifies the cause as a fault and does not say what goes wrong. |
| "true observation about old code" | grader | The line between it and "an unlisted true older fault" is the admission test, which the grader is not told to apply. |
| "v3" | section 7, grader | The contract name is `current-verdicts/v3`. |

## 7. What the proposed text decides that the owner has not been asked

1. **What makes an older fault eligible.** Section 7 gives one test: the two questions, without responsibility for the change. It sets no limit on where the fault is. The five candidates all sit beside changed lines, in a function or a branch the change edits. Under the proposed words a true fault in a file the change never touches could be admitted too, and so could a fault the change makes rarer (the SeaweedFS short page) or a regression from an earlier pull request (the Astro query string). The text also does not say that a minor defect or a suggestion cannot be an older problem; that follows only if "satisfies" is read as promised and not delivered.
2. **Whether "was it intended" is the test.** The proposed answer is no: intent is evidence. The owner's words on the Kubernetes cut made it the test for that case. In the five admissions the blind sessions recorded intent as "not shown" in nine of ten answers, and eight of those nine admitted all the same. Under an intent test they would have had no ground.
3. **Whether the cut-off applies.** The text does not say that an admission is judged on what a reviewer could know at the cut-off. Rule Before 3 says it of the two questions, and the five dossiers follow it. Three of the five were fixed by the maintainers after their cut-offs.
4. **Whether an older problem has a label.** The contract says an entry "has no impact band". The owner judged the Kubernetes cut "other-material" and asks for the label's reasoning with every recommended problem. The boundary's definition of other-material, "A real bug that earns credit when a review raises it, but it does not have to be raised", is close to the definition of the credit itself.
5. **How an older catch is displayed.** A separate panel and CLI section, "Older problems caught": credits by review and problem, distinct problems raised, reviews with a catch, catches per problem, the size of the catalog, how many admitted reviews are assessed, unresolved credit and pending candidates. No share of older faults, no blend with recall, impact, ranking or a recommendation. One credit for a review and a problem; each scheduled repetition can earn its own.
6. **What the pending gates are.** Read in `src/lib/scoring.ts`: a pending older candidate withholds the final counts of this panel for every selection that holds its task ("Older-problem candidates await admission."), and it is counted with the novel candidates that limit a recommendation ("novel candidate awaits an eligibility ruling"). A grader creates such a candidate whenever it meets "an unlisted true older fault". Nothing caps how many, and each needs a dossier, two blind answers and the owner. The first regrade produced the five that are prepared here; the next regrade is the first whose graders are told to look for them.
7. **Whether a task with only older problems stays a clean control.** The proposed answer is yes. The owner ruled so for Kubernetes 141463. The general sentence and the change of the display phrase from "correctly silent" to "without a refuted or unsupported claim" have not been put to the owner.
8. **Who settles what.** Admission and grouping: the owner, each with two blind answers, with no delegation. Credit: a grader decides the two facts on each comment as for an ordinary known problem; a disputed one is an owner decision of a type that, by the contract page, agents never settle.
9. **Whether an older problem can be the same fault as a known problem.** A claim cannot name both catalogs. Nothing says how rule Before 4 applies across them.
10. **That a quiet review is assessed.** Every admitted review of a task is given a result for every older problem of that task, also when it said nothing. That is what makes "not raised" a count and the audit strata possible; it is also 199 new units for the five candidates.

## 8. Suggested wording

None of this is adopted. It is offered so that the owner reads one text and not two.

**A. Start from version 9.** Keep `rules.next.md` and `scoring.next.md` as they are and add only:

- to rule Before 2, after "If the change does none of these to an older fault, the fault is outside the review.": the two sentences of 1.3, with "the routes above" in place of the four names, for example: "The other routes above still apply. A fault outside the review can still earn the separate credit for older problems (rubric, section 7); a review that stays quiet about it loses nothing."
- to the rubric: section 7, without its first paragraph, and the pointer of 2.3.
- to the grader text: the two fields and the section, with the word "exactly" of the inherited sentence on `new_candidates` extended to name `kind`.

That keeps rubric 2.4 and 2.5 as the owner accepted them and keeps the two regrade examples.

**B. Correct the tRPC example of rule Before 2** in the same new version, in whichever direction [question 6](rulings/06-trpc-optional-field.md) is ruled. If GT-j5 moves outside, the example becomes one of "outside the review". If it stays, the example gives the true reason.

**C. Define the term once, in section 7.** For example: "An older problem is a fault that was in the project before the change, that the change did not introduce, worsen, expose or say it fixes, and that the owner has admitted. It is admitted when, judged on what a reviewer could know at the cut-off, the answer to 'Promised?' is yes and to 'Delivered?' is no." Whether to add "and that a reviewer of this change would meet in the code the change edits" is the owner's choice under point 1.

**D. Say who settles credit** in section 7: "Admission, grouping and a disputed credit remain owner decisions."

**E. Say when a grader adds an entry**, as the inherited text does for known problems: "add an entry for each older problem where either fact is 'yes' or 'cannot-tell'".

**F. Replace the status paragraphs** (1.1, 2.1, 2.2 and the first paragraph of section 7) at activation with the form the files in force use: which decisions the owner made, on what date, and "It is in force."
