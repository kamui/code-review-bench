# Why the two graders split on "Says why?", and what to do about the name

Short answer. The 31 differences come from four gaps in the definition of the fact, not from the name. The biggest gap is the one decision P19 opened: one cause can be behind several problems, but nothing says how far a shared cause reaches, so one grader tied a claim to every problem that shares its cause and the other did not. The name "Says why?" is confusing, and the record shows it confused you and the session more than once. The graders' own reasons show they read it as "names the cause" every time. Renaming alone would settle none of the 31. A sharper definition under a plainer name, "Names the cause?", settles 30 of them, reproduces all eleven ruled answers, and fixes one agreed answer that contradicts a ruling.

## 1. The pattern

The 31 fall into four kinds. In 13 of them grader 1 says yes and grader 2 says no. In 18 grader 2 says yes and grader 1 says no, and in 16 of those 18 grader 1 made no entry at all. 21 of the 31 are new this round; 10 carried over from the previous round. Inside the 31, grader 2 moved 18 answers from no to yes this round and grader 1 moved 4, while dropping one yes.

| Kind | What splits the graders | Count | Ids |
| --- | --- | ---: | --- |
| 1 | The cause sits in the comment but outside the quoted sentence | 5 | D01, D02, D19, D20, D23 |
| 2 | One cause behind several problems: how far it reaches | 20 | see the three groups below |
| 3 | A wrong example beside the right cause | 2 | D18, D21 |
| 4 | How exact the named cause has to be | 4 | D03, D04, D05, D06 |
| | Total | 31 | |

### Kind 1. The cause sits outside the quoted sentence. Five: D01, D02, D19, D20, D23.

In all five both graders say the claim says what goes wrong, so the review already has credit. They split only on whether the quoted sentence also names the cause. Grader 1 reads the whole comment. Grader 2 reads the quoted sentence alone.

Grader 2 on D01: "It does not itself identify the discarded CheckValid result or the successful Recv error variable as the coding cause." On D20: "The quoted sentence does not itself identify registering String(value) as the cause." On D23: "The quoted claim states the trigger and false state, without naming the missing mount-time false update as the cause."

Grader 1 on D20 finds the cause in the comment's first sentence: "Why: 'Passing the stringified value to useRegisterFieldControl'." On D01 it finds the cause in the comment's title and proposed fix.

What in the rule lets them differ. Section 3 opens with "Judge the claim by its own words." Section 1 says the opposite for this fact: "Keep an explanation with the result it explains. Do not make it a second claim. Section 3 records whether the explanation is right, under "says why?"." So the explanation is part of the claim by section 1 and not part of its own words by section 3. The rule never says which words count for this fact.

What moved. All five were yes from both graders under the draft rubric. Grader 2 moved to no when the approved text arrived and has stayed there. Your ruling 27 supports grader 1's reading: the array sentence of comment C was credited with naming the cause although the cause, the registration of the text form, is in a different sentence of that comment.

### Kind 2. One cause behind several problems: how far it reaches. Twenty, in three groups.

Decision P19 added this line: "One cause can be behind several problems. A claim says why when it calls this problem's cause a fault. That still counts when the claim's reason is a different problem that the same cause produces." The line above it still says "The cause of a neighbouring problem does not count." Nothing says how a grader decides that two known problems share a cause, or whether to open an entry for a known problem the claim never describes. The preamble's gate, "each known problem the claim is about", does not say whether a claim is about a problem whose cause it faults.

**Group 2a. Grader 2 ties the claim to problems it never describes; grader 1 makes no entry.** Twelve: D07, D08, D09, D10, D11, D12, D13, D14, D15, D16, D17, D31. All twelve are new this round. Grader 2 opens an entry for each known problem that shares the fault and records the cause fact alone. On D07: "That copy causes branded-input loss too; no branded use is named." On D15: "Unconditional module-level context creation is identified as faulty; it causes no-ssl import failure, but that failure is not stated." Its notes say what it is doing: "The other family entries record common causes only." Grader 1 made no entry for any of the twelve, so it left no reason.

This group is where the rule gap bites hardest, because the answer key itself draws the line differently for neighbouring problems. For the two no-ssl entries, D15 and D17, the key's own title says the problem happens "because a TLS context is created unconditionally at import", which is exactly what the claim faults. For the two injection entries, D14 and D16, the key says the opposite: "Both involve context creation during import, but deferring creation until first use would still leave a later injection using an old context." Grader 2 gave all four the same yes.

**Group 2b. Grader 1 ties by the shared cause; grader 2 does not.** Five: D22, D24, D26, D27, D28. Three of these, D24, D27 and D28, carry your rulings, and all three rulings answer yes on this fact, as grader 1 does. D28 is the very comment decision P19 was written from. Grader 1 on D28: "Calls the cause a fault ('The registered baseline and value now use `String(value)`') but for a different result, the dirty comparison of ['a,b'] and ['a','b']; it does not say a validator's verdict changes." Grader 2's notes on the same claim never reach the cause: "This is dirty equality, not a changed validator verdict." On D26: "It does not describe the required-error reset of GT-r3."

This group looks like grader inconsistency more than rule. Grader 2 applied P19 freely in group 2a and not at all here. The difference seems to be whether someone else said the causes are shared: in the tRPC and requests cases the comment itself says "the same root cause" and the key says the problems come "from the same module-level block"; in the Base UI cases nobody says so and grader 2 ties by the problem described.

**Group 2c. The same sentence, each grader ties it to a different neighbour.** Three: D25, D29, D30. D29 and D30 are one claim, "leaks to every other Session in the process". Grader 1 records it as the cause of the adapter trust problem, GT-i7, and grader 2 records it as the client certificate problem, GT-i4, each without the other. Grader 1 on D30: "It faults the shared context that pool kwargs can write to, which is GT-i7's cause, but never says CA material from one adapter becomes trusted by other connections." Grader 2 on D29: "The shared-state claim includes cert-chain mutation among the settings whose state reaches other Sessions." In the previous round grader 1 tied this claim to GT-i4 and dropped it this round, which is the one yes it gave up. D25 is the same shape on a different claim. D25 and D29 are also two of the three differences on the first fact, which this report does not touch.

### Kind 3. A wrong example beside the right cause. Two: D18, D21.

Both claims name the step the key names as the cause and give an example grader 2 disproves. Grader 2 then either keeps the entry and answers no, or drops the entry. On D18: "that asserted reader path is disproved, so it does not state GT-r4's failure or correctly fault its validation path." On D21 its notes end: "Thus the particular empty-input mismatch the claim asserts is disproved." and it makes no entry. Grader 1 on D18: the sentence "points at registering the text form as the fault, which is GT-r4's cause; it never says a validator rejects, accepts or skips anything."

What in the rule lets them differ. Section 2 says to stop at the first answer that settles a claim, and a refuted claim is settled at question 1. Section 3 never says whether a refuted claim still gets the cause fact. Your rulings 12 and 13 say it does: ruling 12 was made because the comment "names the exact removed code and gives only examples that behave the same before and after the change", and D21 is ruling 13, which answers yes on this fact. Grader 2 also handled the two refuted claims differently from each other, an entry for D18 and none for D21.

D18 has a second question in it. The comment calls the change "the right fix for the dirty bug" and says only that "It is not clear that every reader tolerates that", before asking for a test. Whether a doubt is calling the step a fault is a line you have not drawn. Ruling 28 says describing the step as consistent is not enough.

### Kind 4. How exact the named cause has to be. Four: D03, D04, D05, D06.

One tRPC sentence faults the new type gate as the wrong fix in the wrong place. The key's titles name that gate as the cause of two problems. Grader 2 counts the gate as the cause of both. Grader 1 wants the mechanism by which the gate fails in each problem. Grader 1 on D03: "Names the `extends object` branch but not that it cannot resolve a generic type parameter, and states no dropped ctx or input property." On D06: "No mention of an `any` context." Grader 2 on D04: "Calls the new shared object gate the wrong fix; does not describe the any-context union or rejected property access." and in its notes: "The gate criticism identifies causes, but states no known user failure."

What in the rule lets them differ. The rule says "Naming the file or the line is not enough" and "The claim has to point at the cause as a fault." It does not say whether naming the faulty construct is enough or whether the claim must also say how the construct produces this problem. Grader 1's demand puts part of "what goes wrong" into the cause fact. All four are new this round, after P19.

### Where the graders are inconsistent with themselves

- Grader 2 reads only the quoted sentence in kind 1 and reads the surrounding comment in group 2a. Its reasons there begin "Context faults", "Common explanation calls", "Preceding explanation faults" and "Continuation identifies". The same grader in the same round applied opposite scopes.
- Grader 2 applied the shared-cause line to twelve problems the claims never describe, and did not apply it to the five in group 2b, three of which you have ruled yes.
- Grader 1 tied the "leaks" claim to GT-i4 last round and to GT-i7 this round.
- In the agreed sample, A22 is agreed no by both graders and is your ruling 12, which answers yes on this fact. Both graders judged the quoted sentence alone, where the cause is in the comment's first sentence. It is kind 1 in disguise.

On the four differences you have already ruled, D21, D24, D27 and D28, grader 1 gives your answer every time.

## 2. The phrase

What the fact asks, given the decisions and rulings: does the claim point at the step the answer key says is behind this known problem, and say that step is wrong. The step is usually a line of code and can be a setting, an ordering, or a documentation gap, per decision P16. The claim need not describe the problem, per P19 and rulings 27, 30, 13 and 18. The step must be called wrong, not merely described, per ruling 28. The rubric already says this in its own words when it tells the grader what to do with such a claim: "Record that it names the cause of the known problem."

Is "Says why?" a clear name for that when the claim does not say what goes wrong? No. "Why" asks for the reason behind something already stated. Section 1 uses it that way: the explanation of a result. When there is no result, the question has no object, as you say. The record shows the confusion is real, and that it is yours and the session's rather than the graders'.

- In the second look at ruling 26 you wrote: "It has the solution, it has a partial what, but the why isn't a match, it's why is more that the function requires this dependency, not that the query fails, but it is related." There "why" means the reason the gap matters, which is the first fact's territory. The ruling then recorded "Says why: yes" for that comment on the ground that it names the cause.
- In decision P19 you wrote of a draft sentence about this fact: "'When the claim says this problem's cause is a fault, the answer is yes', the answer to what? I dont understand this sentence".
- Your own words for the fact in decision P13 were "cause, symptom, fix, made a case". The names "says what goes wrong?" and "says why?" were the session's, from the wording of decision P12.
- The rubric needs a line to undo what the name implies: "Record "says why?" even when the answer to "says what goes wrong?" is no."

Is there evidence in the 31, or in the agreed answers, that the name misleads the graders? No. All 62 grader reasons in the 31, and all the reasons in the agreed sample, read the fact as "names the cause" or "faults the cause". Not one reads "why" as "why it matters", and not one confuses it with the first fact. Every split is about scope, reach, exactness, or a refuted example. The agreement figures say the same: the first fact, whose name was reworked three times, agrees on 236 of 239; the second fell as its definition grew, from 213 of 225 to 203 of 220 to 208 of 239, and the fall this round tracks P19 exactly. So rename the fact for the people who read the record, and expect agreement to come from the definition.

## 3. A proposal

Name. Call the fact **"Names the cause?"**. It is your word for it, it has an object without the other fact, and it matches the sentence the rubric already uses. "Faults the cause?" was the runner-up; "names" is plainer, and the definition carries the "says it is wrong" condition.

Keep "Says what goes wrong?" as it is. It stands on its own, graders agree on it 236 of 239, and it has been reworded three times already. Changing it risks moving credit for no gain.

Replacement text for the "Says why?" part of section 3, ready to paste:

> **Names the cause?** Yes when the claim points at the step behind this known problem and says that step is wrong.
>
> - The step behind a known problem is what the answer key says goes wrong. It is usually a line or a call in the code. It can be a setting, the order things happen in, the infrastructure, or something the documentation leaves out.
> - For this fact, read the claim together with the explanation the comment gives for it. The explanation can be in any field of the same comment, including the reason the comment gives for a fix. A fix on its own is not an explanation. A sentence that explains a different claim of the comment does not count for this one.
> - Naming the step and calling it wrong is enough. The claim does not have to say how the step leads to this problem, and it does not have to describe this problem at all.
> - Naming the file or the line is not enough. The step appearing somewhere in the comment is not enough. The claim has to say the step is wrong.
>   - *Not enough:* a comment says a function is too long and lists six things it does. One of the six is the step behind the bug. The comment never says that step is wrong.
>   - *Not enough:* a comment says what the code now does at the step and presents it as consistent with the rest of the code, or as the right change.
>   - *Enough:* "a forked child also inherits a pool whose worker threads do not exist." It calls the inherited pool wrong, though it never says what happens next.
> - One step can be behind several known problems. A claim that calls the step wrong names the cause of each of them. That holds when the claim's reason is a different known problem, a problem that is not on the list, or an example that turns out to be wrong.
> - The step behind a neighbouring problem does not count unless it is also the step behind this one. When the answer key says this problem has a different cause from the step the claim faults, or that fixing that step would leave this problem in place, the answer is no.
> - A claim that question 1 refutes or leaves unproven can still name the cause. Record this fact for it anyway. "Says what goes wrong?" is then no.
> - Record this fact for every known problem whose step the claim calls wrong, even when "says what goes wrong?" is no for that problem and the claim describes nothing of it. Make an entry for that known problem.

Four companion edits, wording only. In section 1, the line "Section 3 records whether the explanation is right, under "says why?"" ends "under "names the cause?"". In section 3, the paragraph "A claim that says why and not what gets no credit" begins "A claim that names the cause and does not say what goes wrong gets no credit". In section 5, "or says why" becomes "or names the cause". Section 4's "Record the user's two facts for that comment" stays.

What stays the same. Credit still comes only from "Says what goes wrong?", whose text is untouched. Every ruled comment keeps your answer: ruling 15 yes, the inherited pool is the step; ruling 22 no for both comments, a shared name and a cached value are not the step of skipped time zone setup; ruling 25 yes, the removed method is the step; the second look at ruling 26 yes, the documentation gap is the step; ruling 27 yes, the registered text form is called wrong for the dirty reason; ruling 28 no, the step is presented as consistent; ruling 30 yes, the skipped clear for an unchanged value is the step; rulings 12, 13 and 18 yes, a right step beside a wrong example or a different problem.

One of your decisions does make the fact hard to keep reliable, and the text above keeps it rather than fights it. Decision P19 lets a claim name the cause of a problem it never mentions. That is what the fan-out in group 2a is. The text bounds it with the answer key's own statements about separate causes, which gives yes for the no-ssl entries and no for the injection entries. I would ask you one question with that pair in hand: when the answer key says two problems come from the same block of code but have different remedies, do you want a claim that faults the block to count for both? If your answer is no, the reach line should read "When the answer key says this problem is a separate fault from the one the claim faults, the answer is no", and D15, D17 and D31 move to no. The second thing I would ask is D18: whether a comment that calls the step "the right fix" and adds a doubt has called it wrong.

## 4. What it would do

The proposed text gives these answers. "Agrees with" names the grader whose current answer matches.

| Id | Known problem | Proposed answer | Agrees with | Why |
| --- | --- | --- | --- | --- |
| D01 | GT-u1 | yes | grader 1 | The comment says the validation error is dropped from the message, which the key names as the step |
| D02 | GT-u2 | yes | grader 1 | The comment says the codec no longer accepts v1-only messages and calls it wrong |
| D03 | GT-j1 | yes | grader 2 | The gate is the step in the key's title; the claim calls the gate the wrong fix |
| D04 | GT-j2 | yes | grader 2 | Same sentence, same gate |
| D05 | GT-j1 | yes | grader 2 | The sentence calls the new gate policy wrong |
| D06 | GT-j2 | yes | grader 2 | Same |
| D07 | GT-j4 | yes | grader 2 | With its explanation, the claim calls the merge of unchanged input through the helper wrong; the key names that merge |
| D08 | GT-j5 | yes | grader 2 | Same |
| D09 | GT-j4 | yes | grader 2 | The comment's explanation faults the mapped type turning arrays into property bags, which is the key's step |
| D10 | GT-j5 | yes | grader 2 | The same explanation faults stripping the optional markers, which is the key's step |
| D11 | GT-j4 | yes | grader 2 | The explanation faults merging an unchanged input at all |
| D12 | GT-j5 | yes | grader 2 | Same |
| D13 | GT-j4 | yes | grader 2 | The explanation faults the helper copying an array's members because arrays pass the object test |
| D14 | GT-i6 | no | grader 1 | The key says deferring creation would leave this problem in place |
| D15 | GT-i9 | yes | grader 2 | The key's title names unconditional creation at import as the cause; the claim calls it wrong |
| D16 | GT-i6 | no | grader 1 | As D14 |
| D17 | GT-i9 | yes | grader 2 | As D15 |
| D18 | GT-r4 | still open | | The claim names the step and is refuted, which the text covers, but calls the change "the right fix" and adds a doubt; whether a doubt calls the step wrong is for you |
| D19 | GT-r3 | yes | grader 1 | The comment says the reset now runs validation and the dirty flag is never cleared |
| D20 | GT-r4 | yes | grader 1 | The comment's first sentence names the registered text form |
| D21 | GT-r2 | yes | grader 1 | Ruling 13; a refuted example beside the right step |
| D22 | GT-r2 | yes | grader 1 | The comment's first sentence faults the mount effect that no longer clears filled |
| D23 | GT-r2 | yes | grader 1 | The comment's first sentence faults the same mount effect |
| D24 | GT-r5 | yes | grader 1 | Ruling 30 |
| D25 | GT-i5 | yes | grader 2 | The claim says hostname checking is forced off on the shared context, a verification setting written into it |
| D26 | GT-r3 | yes | grader 1 | The claim says a code-driven change is treated as a user edit, which is the key's step |
| D27 | GT-r3 | yes | grader 1 | Ruling 18 |
| D28 | GT-r4 | yes | grader 1 | Ruling 27 |
| D29 | GT-i4 | yes | grader 2 | The comment faults the shared context whose certificate chain urllib3 changes |
| D30 | GT-i7 | yes | grader 1 | The comment faults the shared context that pool settings write to |
| D31 | GT-v10 | yes | grader 2 | The claim says a session setting persists after return to the pool, which the key names as the step |

Settled: 30 of 31. Still open: 1, D18. Of the 30, 14 agree with grader 1 and 16 with grader 2. All four ruled differences come out as you ruled. In the agreed sample, A22 moves from an agreed no to yes, which is your ruling 12; the other eleven agreed no answers stay no, and the twelve agreed yes answers stay yes.

Two things the table does not do. It does not decide the first fact for D25 and D29, where the graders also differ on "says what goes wrong?". And it records more entries than grader 1 makes today, because the last bullet tells graders to open an entry for every known problem whose step the claim faults. That is what P19 already asks for; the text only says it out loud.

## 5. A cheap check

Do it in two steps, the first costing an hour and the second about five per cent of the regrade.

Step one, on paper. Give a reader from a third model family the new text, the answer key entries and the 55 claims in these two files, with every grader answer and ruling removed. Ask for "names the cause?" alone. Pass if the reader matches the table above on at least 28 of the 30 settled rows, turns A22 to yes, and leaves the other eleven agreed no answers at no. A miss concentrated in one kind says that kind's sentence is still unclear; fix the sentence and run the hour again.

Step two, the real check. Re-run both graders on the ten batches of the trial in decision P16, the 44 reviews and 422 claims that hold every ruled comment, with the new section 3 and no rulings shown. Count an entry one grader makes and the other does not as a no, as the counts file does today. The result that would mean it worked:

- Differences on "names the cause?" fall from 31 to single figures, and none of the ones left are of kinds 1, 2 or 4. The first fact's figure, 3 of 239, is the ceiling to aim at, not the bar.
- Both graders give your answer on all eleven ruled answers: rulings 15, 22 on two comments, 25, the second look at 26, 27, 28, 30, 12, 13 and 18.
- "Says what goes wrong?" stays at 236 of 239 or better, which shows the new text moved no credit.
- The number of cause-only entries each grader opens is within a handful of the other's. A large gap means the reach line is still being read two ways.

If the second fact still splits on ten or more, read the new splits before touching the text again. If they are about which step the answer key names, the fix is in the answer key, a one-line "cause" field per known problem, and not in the rubric.
