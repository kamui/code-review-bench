# Rule version 6: drafted pieces

The owner approves rule text one piece at a time. Each piece below is drafted, then given an unslop pass and a plain-language pass, each as its own step. Each piece is then checked against the audit units it answers. Nothing here is approved yet. Once every piece is approved, the rubric files are written from this file. Drafts of those files are beside it: `scoring.safety-first.v6.draft.md` and `scoring.safety-second.v6.draft.md`, with `v5-to-v6.diff`. Their links are written for `bench/rubric/`, where they would go. `bench/rubric/scoring.safety-*.v5.md` stay as they are.

## Why

Rule version 5, section 3, added "A claim can't borrow another claim's failure", with an example written from Django 17914's claims c4 and c5 (`2026-10-02-claude-builtin-fable-selected`, `att-007`, item-1). On 2026-11-04, in the round 2 reconciliation, the owner read that pair the other way, after being asked three times (question 38, unit #2): "Are you reading this review the same way as me? For me the c4 c5 split is just one claim, c5 relates to c4 no? Give that, it seems the answer should be raised." Then the owner chose "Raised; flip Q2; revise rule (Recommended)": "Consistent one-claim reading everywhere: #2 raised, #1 becomes Opus wrong, and I prepare a ruling to revise the rule's example and how split claims are judged."

The comment says, in sentence 3: "If a pool already exists, the 'no-db' cursor runs CREATE/DROP DATABASE on a pooled connection to the very database being dropped or cloned." Sentence 4 says: "In both cases conn.close() only returns the connection to the pool, so the session stays open and DROP DATABASE or CREATE ... TEMPLATE fails with 'database is being accessed by other users'." The claim list made c4 from sentence 3 and c5 from sentence 4. The result is in the next sentence, which points back with "In both cases". So the rule cannot say "one sentence" only.

Section 1 already says "Keep an explanation with the result it explains. Do not make it a second claim." The first grader split this pair anyway. When graders are given a claim list, section 1 also says "Do not add, drop, merge or re-quote one." So the fix has two parts. Section 1 tells the grader who writes the list not to split across the sentence. It also tells a grader given a list how to judge a split pair, without merging it. Section 3 keeps "can't borrow" for separate thoughts and gets a new example.

## Piece 1: a cause and its result are judged together (section 1)

**First draft.**

- In "Keep an explanation with the result it explains": "Do not make it a second claim, even when the result comes in the next sentence."
- New rule after "Sometimes the instructions list the claims of each comment. ... If a split looks wrong, say so in that claim's notes.": **A cause split from its result is judged as one claim.** Sometimes the list puts a cause in one claim and the result the comment gives for it in another. The comment ties them together: in one sentence, or in the next sentence pointing back with words like "so", "this means" or "in both cases". Judge the two claims together for every question, as if they were one claim. Each gets the answers the two earn together. Say in both claims' notes that you judged them together.
  - *Example:* one claim says a database cursor runs on a pooled connection to the very database being dropped. The next sentence starts "In both cases" and says the drop fails, blaming the pool for keeping the session open, and the list splits these into two claims, which judged together say the drop fails because the cursor is connected to the database being dropped, which is true and says what goes wrong, while blaming the pool is a wrong detail that doesn't change the point.
  - *Not together:* two claims that describe different failures, each with its own cause. Judge each on its own (section 3, "A claim can't borrow another claim's failure").

**Unslop pass.** Three changes. The headline was passive ("is judged as one claim"); it now names the action. "The comment ties them together:" used a colon as a connector; the sentence now states the two cases directly. The example ran four clauses into one sentence; it is now six short sentences. The parentheses in "*Not together*" became a sentence.

**Plain-language pass.** Two changes. "pointing back" stays, with the words that do it listed. "as if they were one claim" and "Each gets the answers the two earn together" merged into "Judge the two claims as one claim for every question. Give each the answers the two earn together", so the instruction comes first.

**Draft for approval.**

- In section 1, "Keep an explanation with the result it explains" becomes: **Keep an explanation with the result it explains.** Do not make it a second claim, **even when the result comes in the next sentence.** Section 3 uses the explanation when it records whether the claim identifies the cause as a fault.
- New rule after "Sometimes the instructions list the claims of each comment. Then grade those claims and no others. Do not add, drop, merge or re-quote one. If a split looks wrong, say so in that claim's notes.":
  - **Judge a cause and its result together when the list splits them.** Sometimes one claim on the list names a cause, and another gives the result the comment says that cause has. The comment says both in one sentence, or the second sentence points back to the first with words like "so", "this means" or "in both cases". Judge the two claims as one claim for every question. Give each the answers the two earn together, and say in each claim's notes that you judged them together.
    - *Example:* one claim says a database cursor runs on a pooled connection to the very database being dropped. The next sentence begins "In both cases" and says the drop fails, and it blames the pool for keeping the session open. The list makes these two claims. Judged together, they say the drop fails because the cursor is connected to the database being dropped. That is true, and it says what goes wrong. Blaming the pool is a wrong detail. It does not make the point false.
    - *Not together:* two claims that describe different failures, each with its own cause. Judge each one alone, as the rule "A claim can't borrow another claim's failure" in section 3 says.

**Checked against the units it answers.**

| Unit | The owner's decision | Under piece 1 |
| --- | --- | --- |
| Round 2 #2 (question 38): does c4 raise PE-v1? | raised (first grader right) | c4 and c5 are judged together. Together they say the drop fails on a connection to the database being dropped, PE-v1's failure. Raised. Matches. |
| Round 2 #1 (question 2): is c5 refuted? | not refuted (first grader wrong) | Judged with c4, the point is true and the pool is a wrong detail ("An overstated word does not make a true point false" and "A list with one wrong item is still true" in section 2). Not refuted. Matches. |
| Round 1 #6 (question 3): the DROP part of the same review, c6 then | refuted, judged on the part the first grader defined | Conflicts. That claim list split the sentence differently, but it is the same review and the same DROP failure. The round 2 decision on #1 already reversed it. The round 1 record stays as written. |
| Round 1 #28 and #8 (question 3): two examples, two outputs | judged on the defined part | Unchanged. Each pair is two different results, not a cause and its result. |
| Round 1 #7 (question 19): the same review's PE-v1 raise | not raised | Conflicts. The round 2 decision on #2 already reversed it. The record stays. |
| Round 1 #11 (question 19): `2026-10-02-claude-builtin-selected` `att-014`, "the cursor comes from the 'default' pool (the DB being created or dropped), not the intended one," split from its result | not raised | Would now be judged with its result and likely raised. The current grade lists that sentence as one claim, `c9`, with PE-v1 not raised. See `affected.md`. |

## Piece 2: a claim can't borrow a separate claim's failure (section 3)

**First draft.**

- **A claim can't borrow another claim's failure.** When two claims in a comment are separate thoughts, each with its own cause, don't count one claim's failure for the other. Judge each claim by its own words and its own explanation. A cause and the result the comment gives for it are not separate thoughts; section 1 judges them together.
  - *Example:* a comment says the new cache key leaves out the user's id. Later it says the cleanup job deletes live entries because it compares times in two time zones. These are two thoughts with two causes. The deleted entries don't count for the cache-key claim.

**Unslop pass.** One change. A semicolon joined two sentences; they are now two sentences.

**Plain-language pass.** One change. "are not separate thoughts" became "are one thought", which says what they are rather than what they are not.

**Draft for approval.** In section 3, the rule and its example become:

- **A claim can't borrow another claim's failure.** **When two claims in a comment are separate thoughts, each with its own cause, don't count one claim's failure for the other.** Judge each claim by its own words and its own explanation. **A cause and the result the comment gives for it are one thought. Section 1 says to judge them together.**
  - *Example:* **a comment says the new cache key leaves out the user's id. Later it says the cleanup job deletes live entries because it compares times in two time zones. These are two thoughts with two causes. The deleted entries don't count for the cache-key claim.**

The example is made up, as piece 2 of rule version 5 ("three error branches have no test") was. No saved ruling decides a clean case of two separate thoughts. The nearest real one, SeaweedFS 10735's "The reverse interleaving also exists" beside "the sequence is still not atomic", is itself unclear, so it is listed in `affected.md` and not used as the example.

**Checked against the units it answers.** Rule version 5's piece 5 named units 7 and 11 (not raised), 33 and 38 (stay as decided) and 26 (unchanged). Units 7 and 11 now go to piece 1 (see above). Units 33 and 38 are SeaweedFS interleavings with their own timing and stay separate under piece 2. Unit 26 is unchanged.

## Piece 3: section 3 reads a joined pair as one claim

**First draft.** In section 3, "Judge the claim by its own words." becomes "Judge the claim by its own words. Claims judged together under section 1 count as one claim here."

**Unslop pass.** No change.

**Plain-language pass.** "Claims" became "Two claims", so it is clear the rule means the pair.

**Draft for approval.** In section 3: "Judge the claim by its own words. **Two claims judged together under section 1 count as one claim here.** Do not add a condition that the comment does not state, ..."

Section 6 ("Raised") already judges a claim "by the test of section 3", so it needs no change.

## Not a piece

The revision note at the top of each file is replaced with one that names rule version 6 and points to this file, as rule version 5's note did. `v5-to-v6.diff` shows it. It needs no approval of its own.

## What filing would take

Once every piece is approved: copy the two drafts to `bench/rubric/scoring.safety-first.v6.md` and `bench/rubric/scoring.safety-second.v6.md` with the approval recorded in the note, add `bench/rubric/remedy-policy.v6.json` pinning them, and select it in `bench/grading/current/remedy-policy.json`. That makes every saved grade stale, as rule version 5 did (`docs/current-grading.md`). The audit's second-grader composition is built from `scoring.safety-second.*`, so it changes with it.
