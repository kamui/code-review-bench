# Verdict on the three analyses of "Says why?"

I recommend candidate 2 as the base. It explains the 31 differences best and its rule text is the only one that tells a grader to make an entry for a known problem the claim never describes, which is where 21 of the 31 differences sit. It needs three wrong figures corrected, a softer claim about the name, and parts of the other two grafted in. The three are close: 24, 23 and 23 out of 30.

## What I checked

- **Counts.** I recomputed every count from `grounding/differences.json` and `grounding/counts.json`. The facts they rest on: grader 1 says yes in 13 rows and grader 2 in 18. Grader 1 has no entry in 14 rows (D07 to D17, D25, D29, D31) and grader 2 in 7 (D21, D22, D24, D26, D27, D28, D30). Two of the 31 rows differed under the draft (D21, D29) and ten in the round before this one. Since that round grader 2 moved 18 rows from no to yes, and grader 1 moved 4 from no to yes and 1 from yes to no. The 31 rows are 22 distinct claims from 21 comments.
- **Ids.** For each candidate I read the comment, the claim, both graders' reasons and the answer key entry for D01, D03, D04, D14, D16, D18, D19, D21, D25, D28, D29, D30 and D31, and for A20 and A22 in the agreed sample.
- **Quotations.** I searched for every quotation of evidence in the three reports in the grounding files. All are exact. One is described wrongly (candidate 2, line 45, below).
- **Rule texts.** I read each proposed text against rulings 15, 22, 25, S11, 27, 28 and 30 and decisions P19 and P20. None of the three contradicts a ruling. Each adds one thing the owner has not ruled on, and each says so.

## Scores

| Criterion | Candidate 1 | Candidate 2 | Candidate 3 |
| --- | --- | --- | --- |
| 1. Faithful to the data | **4.** Every count I recomputed is right, including 29, 21 and 0 agreements across the rounds and 22 claims from 21 comments, but D29 does not fit the kind it is placed in. | **3.** The kinds fit the rows best of the three, but line 7 says grader 1 made no entry in 16 of 18 rows when it is 14, and line 77 counts 62 grader reasons when 21 of them are blank. | **4.** Every count is right, including 14 and 7 missing entries and 330 judgments in the check, but D29 does not fit its kind and line 57 misstates what grader 1 did with it. |
| 2. Explains the disagreement | **4.** Each kind is tied to a rule line or a decision and to the earlier rounds, but the choice to make no entry is named only as a "possible ambiguity" and never counted. | **5.** It quotes the two rule lines that conflict on which words count, names the gate "each known problem the claim is about", shows the answer key drawing D15 and D17 apart from D14 and D16, and lists four places where a grader contradicts itself. | **4.** It ties each kind to the rule text and is the most careful about what the rounds can prove, but one kind holds 22 of the 31 rows and hides the difference between D03 to D06 and the rows with no entry. |
| 3. Answers the question about the phrase | **4.** It says what the fact asks, calls the name poor, notes that "why" also sounds like the first fact's "why this matters", and says plainly that no trial has changed the name alone. | **4.** It alone shows the name confusing the owner, with exact words from the second look at ruling 26 and from P19, but its flat "No" for the graders rests on the miscounted 62 reasons and tests only one way of misreading the name. | **4.** It says the records "do not prove" a naming effect, uses A04, A08 and A22 well, and proposes a test of the name alone, but it misses that the first fact's own definition uses "why". |
| 4. The proposal can be adopted | **4.** The name carries both conditions and every sentence reads without the heading, and a table gives the owner's answer on each ruling, but the text drops both examples the owner approved and gives no exact wording for the edits elsewhere. | **4.** It keeps the approved examples, gives exact wording for four edits in other sections and a fallback line for the owner's other answer, but the name drops "and says it is a fault" and lines 89 and 97 are worded badly. | **3.** The name and text are short and pass every ruling, but line 87, "A real step in the chain of causes is enough", widens the fact beyond anything the owner has ruled, and the approved examples are gone. |
| 5. It would work | **3.** Its 28 answers mostly follow from its text, but nothing in the text tells a grader to make an entry for a problem the claim does not describe, which is what 21 rows turn on. | **4.** It settles 30 and says to make the entry, but the answers for D01, D25, D29 and D30 do not clearly follow from its own lines, and its line on the answer key's wording opens a new question. | **3.** It settles 29 and tells the grader to record each problem the cause produces, but it answers yes on D14 and D16 against the answer key's own sentence and counts them as settled. |
| 6. Checkable and plain | **4.** Three versions are compared, one changing only the name, with a clear pass condition and a second trial on unseen claims, but it gives no cost and does not test whether graders make entries unprompted. | **4.** Two steps with a cost for each and four numeric pass conditions, one of them the number of entries each grader opens, but no version changes only the name and it reuses the batches the text was written from. | **5.** Three versions, a repeat, a record required for every pair, the ruled comments outside the 55 added, and two untouched batches chosen in advance, all in the plainest prose of the three. |
| **Total** | **23** | **24** | **23** |

## What each candidate got wrong

### Candidate 1

- **D29, lines 37 and 47.** It files D29 under "Reading only the quoted fragment". Grader 1 did not read the fragment alone. It used the surrounding comment to tie the same claim to GT-i7 in D30, and it simply made no entry for GT-i4. D29 and D30 are one claim, and the report puts them in two different kinds.
- **The missing entry, line 29 and lines 131 to 147.** A grader made no entry in 21 of the 31 rows. The report calls the gate a "possible ambiguity" and its rule text never says to make an entry for a known problem the claim does not describe. Its table predicts yes for those rows on a step the text does not state.
- **A20, line 170.** It says A20 stays no because of "the particular fault named". Its own text says a false example does not erase a cause and that a cause can be criticised for a different result, and A20 is a refuted claim that faults `String(value)`. The answer does not follow from the text. See the last section.
- **The examples, lines 131 to 147.** The text drops both examples under "Says why?" in the current rule, which decision P16 says the owner kept. One of them is the forked child sentence of ruling 15. The text also says "wrong or worth changing", which is looser than "says it is a fault".
- **D18, line 195.** It answers yes without saying that the comment calls the change "the right fix for the dirty bug". The other two flag this.
- **D14 and D16, lines 191 and 193.** It leaves them open, although its own line "Check that it causes this known problem" and the answer key's sentence on GT-i6 point to no. Asking the owner is fair, but the report does not say which way its text leans.
- **Line 149.** It says to update the old name in sections 1, 3 and 5 and gives no wording.

### Candidate 2

- **Line 7.** "in 16 of those 18 grader 1 made no entry at all". It is 14. Grader 1 made an entry and answered no for D03 to D06, as the report's own kind 4 says.
- **Line 77.** "All 62 grader reasons in the 31". There are 41 reasons. The other 21 are blank because the grader made no entry. Grader 1 left no reason in 14 rows, so those rows cannot show how it read the name. The same line says the first fact's "name was reworked three times". Its name never changed. Its definition did.
- **Line 77, the conclusion.** It answers "No" to whether the name misleads graders. The evidence supports "not shown". Grader 2's reason on D19, "does not itself explain", and grader 1's demand for the mechanism on D03 are both what a reader would do if the name asked for an explanation of a result. The report tests only the reading "why it matters" and proposes no trial of the name alone.
- **Line 45.** It says grader 2's notes on D21 end with "Thus the particular empty-input mismatch the claim asserts is disproved." The sentence is exact, but the notes go on for two more sentences.
- **Line 39.** It says that in "the tRPC and requests cases the comment itself says" the causes are shared. The words "the same root cause" are in one tRPC comment only, the one behind D09 and D10.
- **The name, lines 81 and 87.** "Names the cause?" leaves out the second condition. Ruling 28's own file says comment D "names the registration of the serialized value and never says that step is wrong", and its answer is no. The bare name gives the wrong answer on a ruled comment. The other two candidates rejected this name for that reason.
- **Line 89.** "The step behind a known problem is what the answer key says goes wrong." This defines the cause with the first fact's own phrase. A grader could read any claim that says what goes wrong as naming the cause.
- **Lines 90 to 99.** "the answer is no" and "this fact" need the heading to make sense. In P19 the owner wrote of "the answer is yes": "the answer to what? I dont understand this sentence". The text also says "step" throughout under a name that says "cause".
- **D01, line 113.** It answers yes because the comment says the validation error is dropped from the message. That is the result. The diagnosis is only in the proposed fix, and line 90 says "A fix on its own is not an explanation."
- **D25, line 137.** It answers yes without asking whether the comment calls the hostname setting wrong, which is the question it rightly raises for D18.
- **D29 and D30, lines 141 and 142.** It answers yes for both. The answer key for GT-i7 says that problem differs from GT-i4 and from the verification settings, and that a design that meets GT-i4 "leaves this in place". Line 97 says such a statement means no. The table and the text can be read against each other here.
- **Line 145.** "the other eleven agreed no answers stay no". It did not test this. By lines 96 and 98, A20 comes out yes.
- **The check, lines 149 to 162.** No version changes only the name, so the owner's question stays untested. Step two reruns the ten batches the text was shaped on and does not say so.
- **The rationale.** At 619 words it is the longest of the three and may run past the one page allowed.

### Candidate 3

- **D29, lines 16 and 57 to 59.** It files D29 under "Describing a cause or calling it wrong", then says at line 59 that the comment plainly criticises the shared context. The split is over which known problem to tie the claim to. Line 57 says grader 1 links the claim "to verification settings alone". Grader 1 also ties it to GT-i7 with a yes, which is D30.
- **Line 14.** One kind holds 22 rows. In D03 to D06 grader 1 made an entry and asked for the mechanism. In D07 to D17 grader 1 made no entry at all. These are different failures and need different sentences in the rule.
- **Line 87.** "A real step in the chain of causes is enough." The owner has not ruled this, and the current rule says "the real cause" and "The cause of a neighbouring problem does not count." The report says it is its own reading and would ask the owner, but it pastes the line into the rule first.
- **D14 and D16, lines 134 and 136.** It answers yes. The answer key for GT-i6 says "deferring creation until first use would still leave a later injection using an old context". The report quotes this at line 37, answers yes anyway, and counts both rows as settled.
- **Lines 83 to 95.** The text drops both approved examples, as candidate 1 does. Line 93 uses "this fact", which needs the heading.
- **Line 97.** It gives no wording for the edits in other sections.
- **A20.** It never tests its text against A20.
- **The check.** It gives no cost, and no pass figure for the two fresh batches.

## The base, and why

Use candidate 2.

- **It finds the mechanism the others underplay.** In 21 rows one grader made no entry. Candidate 2 names the gate in the rule's opening line, splits the shared cause rows by who made the entry (12 for grader 2, 5 for grader 1, 3 where each tied the same sentence to a different neighbour), and writes the one rule line that addresses it: "Make an entry for that known problem."
- **It uses the answer key as evidence.** It shows that the key itself says GT-i9 comes "because a TLS context is created unconditionally at import" and says of GT-i6 that deferring creation would leave the problem. No other candidate turns this into a rule.
- **It shows the graders contradicting themselves.** Grader 2 reads only the quoted sentence in D01, D02, D19, D20 and D23, and reads the surrounding comment in D07 to D13. Grader 2 applies the shared cause line to twelve problems the claims never describe and not to D24, D27 and D28, which the owner has ruled yes.
- **Its proposal is the closest to pasteable.** It keeps the owner's approved examples, adds one modelled on ruling 28, gives exact wording for four edits elsewhere, and gives the owner a question with the replacement line for either answer.
- **Its faults are cheap to fix.** Three figures, one overstated conclusion, one name and two badly worded lines. Its structure does not depend on any of them.

Before it goes to the owner, correct line 7 to 14, line 77 to 41 reasons with 21 blank, and line 45. Change the answer on the name from "No" to "not shown". Reword lines 89 and 97 so that the cause is not defined as "what goes wrong" and no sentence says "the answer is no".

## What to graft in

From candidate 1:

1. **The name and the self-contained sentences.** "Identifies the cause as a fault?" carries both conditions and still has an object when the first fact is no. Its closing lines, such as "Record whether the claim identifies the cause as a fault even when the claim gets no credit or is refuted", read without the heading. Use them in place of "this fact" and "the answer is no".
2. **The line on fixes, with D01 as its own case.** "Do not infer a missing diagnosis from a proposed fix. A sentence that actually states the fault can count wherever it appears in the comment." This is clearer than candidate 2's line 90, and it makes D01 a question to put to the owner rather than a yes the text does not support.

From candidate 3:

1. **The design of the check.** Add a version that changes only the name, repeat the full version in fresh sessions, require a record for every pair, and finish on two untouched batches chosen before anyone reads them. Keep candidate 2's costs and numeric pass conditions, and its count of the entries each grader opens.
2. **The open question on D31, and the caution about the rounds.** D31's known problem is a missing warning in the documentation. Candidate 3 asks whether the behaviour that needs the warning counts as that problem's cause when the claim never mentions the warning. Candidates 1 and 2 answer yes without asking. Also take its sentence that P18, P19 and P20 arrived together, so the files cannot pin this round's movement on P19 alone. Candidate 2 says the fall "tracks P19 exactly".

## Where all three are wrong or silent

- **Section 1 still defines the fact the old way.** Line 20 of the rule reads "Section 3 records whether the explanation is right, under "says why?"". That makes the fact the correctness of an explanation of a stated result, which is the reading the owner's question is about. Candidate 2 quotes the line as evidence. All three then change only the name in it. The sentence needs rewriting, not renaming.
- **A20 is not settled by any of the three texts.** A20 is a refuted claim that faults `String(value)`, the line behind GT-r4, for a reason that is false. Ruling 27's ground is that "A comment that faults the right line counts as saying why, whatever problem it faults the line for", and rulings 12 and 13 keep the cause when the example is wrong. Together these point to yes, and both graders agree on no. Candidate 1 says no on a distinction its text does not contain. Candidate 2 says the agreed no answers stay no without testing it. Candidate 3 does not mention it. None gives a test for how exactly the faulted thing must match the cause that sorts A20 from comment C of ruling 27. This is a question for the owner, and A20 should not be used as a control until it is answered.
- **The fix assessment widens with the fact.** Section 5 says "A fix is for a known problem when a claim it addresses says what goes wrong for that problem, or says why." Each proposal records the cause for more known problems per claim, so more fixes will be assessed against more known problems. Credit does not change, but the fix record does. None of the three says so.
- **The owner rules on comments and the graders record claims.** Every ruling gives the two facts for a whole comment. The graders answer for a quoted piece of it, sometimes cut in the middle of a sentence, as in D05 to D08. That mismatch is behind the five scope rows and behind A22, where both graders answer no against ruling 12. Candidate 2's rationale mentions recording the fact per comment as a later option. None of the three puts the choice to the owner, and all three rely on a line between "the explanation of this claim" and "a separate claim" that the graders already draw differently.
- **None of the predictions was run.** All three tables are the author's own reading of their own text. Candidates 1 and 3 say so plainly. Candidate 2 writes "The proposed text gives these answers", which claims more than it has.
