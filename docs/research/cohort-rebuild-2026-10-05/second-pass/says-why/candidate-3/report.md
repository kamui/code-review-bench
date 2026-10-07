# Why the graders disagree

The largest pattern is a shared cause criticized for a different reason. It accounts for 22 of the 31 differences. Another five turn on whether the grader reads only the quoted sentence or also its attached explanation. The name “Says why?” is unclear for the fact the owner chose, but these records do not show that the name caused the disagreement. The definition and inconsistent use of it give stronger explanations.

This report uses only the supplied files in `grounding/`. It treats the code checks reported there as evidence, without independently checking those codebases. The value “no entry” counts as no. An answer here concerns one claim and one known problem, not a whole review.

## The pattern

Each difference appears once in this table. Some have more than one contributing cause. The groups name the main disagreement, rather than claiming to prove what happened inside a grader.

| Kind | Count | IDs |
| --- | ---: | --- |
| Reading the surrounding words | 5 | D01, D02, D19, D20, D23 |
| The same cause, a different complaint | 22 | D03, D04, D05, D06, D07, D08, D09, D10, D11, D12, D13, D14, D15, D16, D17, D22, D24, D26, D27, D28, D30, D31 |
| A false example beside a true cause | 2 | D18, D21 |
| Describing a cause or calling it wrong | 2 | D25, D29 |
| Total | 31 | |

### Reading the surrounding words

In D19, the comment says “A programmatic reset now runs `validation.change` unconditionally.” Grader 1 uses that explanation and the later statement about an uncleared history flag. Grader 2 says, “This quoted statement does not itself explain the changed-once ref or the validation call that creates the error.” D20 and D23 have the same split. D02 uses earlier words to identify which messages the codec rejects.

Section 3 says to judge the claim by its own words. Section 1 says to keep an explanation with the result it explains. Neither states quite how a short prescribed quotation relates to its surrounding explanation. That permits a narrow reading and a contextual reading. D01 adds a separate boundary: grader 1 takes its cause from the proposed fix, while grader 2 keeps that fix separate. Section 1 explicitly says a requested fix is not a claim. My proposal therefore gives D01 no, and the other four yes. This is a clarification about what words count, not evidence that a fix is wrong.

All five were yes/yes in the draft round. Grader 2 changed each to no before P18, P19 and P20, and kept no in the current round. Those later decisions cannot explain those particular moves. Grader 2 still uses surrounding explanations in agreed A01, A03, A07, A10 and A11. A07's quotation is only “raises a raw TypeError”, yet both graders use the surrounding constructor explanation. That makes the narrow reading look inconsistent as well as under-specified.

### The same cause, a different complaint

D28 says “Two different arrays with the same joined string compare equal”. Its attached words identify registering `String(value)`. That registration also causes the known validator problem, although array comparison is a different complaint. Grader 1 records yes. Grader 2 omits the link and says, “This is dirty equality, not a changed validator verdict.” Ruling 27 and P19 expressly settle this case as yes.

D03 to D06 criticize an object test added to a shared type helper. Grader 1 requires the particular generic or unrestricted-context case. Grader 2 records the criticized test as the cause without requiring those cases. D07 to D13 fault copying an unchanged input's properties. That copying also causes the known branded-string and optional-key problems, although the comments discuss other input shapes or where the repair belongs.

D14 to D17 fault creating a security context at import for unnecessary cost. The same creation participates in later backend-selection failures or directly causes import failure when the security module is absent. D22 faults a removed field-state update for mode switching rather than mounting. D24 faults skipped error clearing for a rejected keystroke rather than a checkbox click. D26 and D27 fault programmatic validation for prefill rather than a reset to empty. D30 faults a shared mutable security context for settings leaking between sessions rather than specifically for an adapter's certificate authorities. D31 faults reused database state rather than the missing warning about reuse.

P19 allows a different complaint about the same cause. P20 says a cause-only claim must still be classified on its own merits. Being a suggestion, or describing unsupported use, does not prevent recording the cause. Grader 1 often makes no entry for an advisory claim; grader 2 often makes no entry when the described outcome differs. Both omissions can hide a cause match.

The text nevertheless leaves the size of a cause unclear. It says “the real cause” and distinguishes neighbouring causes, but does not say whether a real step in a longer chain is enough. D14 and D16 name early creation without explaining continued use after a later backend change. The answer key explicitly says postponing creation alone would still allow a later injection to use an old context. I count the named creation step, not a complete diagnosis or a sufficient remedy. D30 names the shared context and pool settings without naming the particular certificate-loading call. I count that specific shared-state mechanism too. A shared file, helper name, or repair alone would not suffice.

D31 remains open. The known problem concerns an absent documentation warning. No-reset reuse explains the software's behaviour, but the comment never criticizes the warning. P16 permits causes in documentation; it does not establish whether behaviour needing a warning itself counts as that documentation problem's cause. I would ask the owner to decide that boundary.

This group had no disagreements in the draft round. Before P18, P19 and P20, only D22 and D24 disagreed. In the current round grader 2 added yes for D03 to D17, while grader 1 added yes for D26, D27, D28 and D30. D31 changed from grader 2 no to yes. D22 and D24 stayed split. That movement fits uneven application of P19, although several rule changes happened together, so the files cannot attribute it to P19 alone.

D24, D27 and D28 now contradict attached owner rulings on grader 2's side. Those are failures to apply the decision, rather than open wording questions. A08 is a useful contrast: both graders count a cause even though the complaint is readability and earns no credit. Requiring a known failure scenario elsewhere is not a consistent standard.

### A false example beside a true cause

D18 faults registering text instead of the original value but wrongly names form collection and reset as readers. Its objection includes “It is not clear that every reader tolerates that”. Grader 1 retains the true cause. Grader 2 rejects the reader path and also records no cause. D21 wrongly says a programmatic null or undefined value empties the visible input. Its attached words still fault the removed mount-time clearing branch: “The old branch that set filled to false for a controlled `''` was removed”. Grader 1 records yes. Grader 2 rejects the example and omits the cause link.

The current rule requires a real cause and criticism of it. It does not require a true example for this second fact. The history of the two facts began with exactly that separation: a review could find the cause and make a poor or false case. Section 2's treatment of a false example can still tempt a grader to stop before recording section 3's independent cause fact. The replacement text explicitly prevents that.

D21 already disagreed in the draft and still does. Its attached owner ruling says yes. D18 was no/no in the draft; grader 1 moved to yes before P19. D18 leaves more interpretive room because its comment also calls the dirty-state repair correct. I read its concern about the registered value and its readers as criticism of that registration, while keeping its false readers false.

Agreement is no guarantee here. A22 is no/no on the current cause fact, but its attached ruling says yes. Its example behaves the same before and after the change. Its surrounding comment still faults the removed clearing branch. The proposal preserves the ruling's yes without inventing a true outcome for that example.

### Describing a cause or calling it wrong

D25 includes “with `check_hostname` forced False on the shared context”. Grader 2 treats this as a second fault concerning shared verification settings. Grader 1 links the claim only to the ignored backend selection. D29's surrounding statement says “urllib3 mutates it (verify_mode, cert chain)”. Grader 2 links that shared-state complaint to client-certificate leakage; grader 1 links it to verification settings alone.

Section 3 rightly requires that the cause be called a fault. It gives a clear negative example of incidental mention, but no clear boundary for an objection implied by surrounding words. D29's whole explanation criticizes the shared mutable context and expressly includes certificate-chain mutation, so I give it yes. D25 remains open: does its complaint about inconsistent backends also criticize the hostname-setting mutation, or merely describe it? The files establish that the flag changes, and also that fallback hostname matching remains. They do not justify inferring weakened verification.

D25 went from no/no in both earlier rounds to grader 2 yes. D29 went from grader 2 yes in the draft to grader 1 yes in the next round, then back to grader 2 yes now. That reversal, with unchanged comment text, points to unstable interpretation. A19 provides the contrast: both graders correctly reject a cause match when a comment describes an effect but calls the arrangement fine and faults only its explanation.

### What the totals do and do not show

The supplied counts give cause agreement of 213/225, then 203/220, then 208/239. The denominators changed, so the decline alone does not measure the effect of the wording. Among today's 31 differences, only two differed in the draft, ten differed in the next round, and all 31 differ now. Those counts normalize no entry to no.

Within these 31, grader 2 changed 18 no answers to yes since the previous round. Grader 1 changed four no answers to yes and one yes to no. Across all 239 current pairs, the saved counts report 31 no-to-yes changes for grader 2 and 13 for grader 1, with one yes-to-no change each. The two-fact disagreement has shifted toward adding cause links, not simply failing to recognize causes. Fourteen current differences are no entries from grader 1; seven are no entries from grader 2. Checking only existing links would miss much of the problem.

## The phrase

The fact really asks whether the claim singles out an actual cause of this known problem and objects to it. It does not ask whether the claim explains its own stated outcome. It does not require the known harm, a complete chain of causes, a correct example, or an effective fix. A documentation omission can qualify. A criticism for another reason can qualify. A neutral description cannot.

“Says why?” is a poor name when the first fact is no. It suggests an explanation of something the review has already said goes wrong. P19 goes further: the recorded cause can belong to a known problem whose outcome the review never describes. The owner deliberately chose a separate record of cause recognition, rather than extra credit or a separate judgment of persuasion, in P13.

The reasons support confusion about the definition's boundaries. They do not prove confusion caused by the name. No supplied reason says the missing “what” made “why” unanswerable. A04 gives both graders yes for an undocumented dependency requirement without a stated failure. A08 gives both yes for a cause criticized on readability grounds. Those agreements show that graders can apply the intended independence under the current name. A22 shows that they can also agree on an answer inconsistent with the intended independence. Renaming should accompany a clearer test, and a separate check should test the name alone.

## Proposed text

Use **“Calls a real cause a fault?”**. The name states both requirements and identifies what is being criticized. Keep **“Says what goes wrong?”** unchanged. Its name describes the credit test clearly, and renaming it merely for symmetry would add change without solving these disagreements.

Replace only the current “Says why?” block in section 3, ending before “What follows from the two facts”, with this text:

> **Calls a real cause a fault?** Record yes when the claim identifies an actual cause of this known problem and presents that cause as wrong or needing improvement.
>
> Read the quoted claim with the words that explain it or establish what it refers to. Those words can be elsewhere in the comment. Do not borrow a cause from a separate claim. A requested fix alone does not count as a statement of the cause.
>
> A cause can be a specific action, a missing step, or an unstated requirement. It can be in code, documentation, configuration, timing, infrastructure, or elsewhere. A real step in the chain of causes is enough. The claim need not explain the whole chain. An input that triggers a failure is not enough unless the claim also identifies what is wrong in handling it.
>
> Naming a file, a line, or a function is not enough. Describing an action without objecting to it is not enough. Record no when the claimed cause is not real or belongs only to another problem.
>
> One cause can produce several problems. Record yes for each known problem that the criticized cause actually produces. The claim may criticize it for a different result, for readability, or in an unsupported use. Sharing a location or a proposed fix does not establish a shared cause.
>
> Record this fact even when the claim says nothing about what goes wrong for this known problem. A false example does not erase a separately stated, real cause that the claim criticizes. Keep the false example's own assessment. Do not add missing steps or outcomes to make it true.
>
> Record cannot tell when the evidence leaves either the cause match or the criticism unclear. Say which is unclear and what would settle it. Follow saved owner rulings on both facts.

Update references to the second fact in sections 1, 3 and 5, and in grader instructions, to use the new name. This is a naming change, not a change to their procedures. Keep the first fact, the credit rule, and fix assessment unchanged. Keep P20's requirement to classify a cause-only claim and raise a possible new problem. Do not let an earlier claim label suppress this independent cause record.

The choice to count a real causal step makes an existing loose boundary explicit. It is my interpretation, not a new owner ruling. P19's breadth is intentional, but it makes the record depend on how broadly the answer key describes a cause. I would ask the owner whether this real-step test is the intended boundary, using D14/D16 beside D30/D31. I would also ask whether D25 actually criticizes the setting change. Until then the proposal leaves D25 and D31 open rather than changing a ruling.

### Check against the supplied decisions

| Ruled comment | Says what goes wrong? | Calls a real cause a fault? | Why the proposal preserves it |
| --- | --- | --- | --- |
| 15, forked pool | no | yes | The missing workers are the criticized cause. No consequence is supplied. |
| 22, Q1 and Q2 against timezone failure | no | no | Naming twins and removing the role method do not identify bypassed timezone dispatch. |
| 25, removed role method | no | yes | The API removal is criticized, without a stated failed override or wrong role. |
| S11, replacing 26, undocumented pool version | no | yes | The dependency requirement and documentation omission are named as faults. No older-package failure is supplied. |
| 27, array equality against validator failure | no | yes | The criticized registration causes both results. |
| 28, serialized validation described as consistent | no | no | The behaviour is described without an objection to it. |
| 30, rejected keystroke against checkbox failure | no | yes | The same clearing path is faulted. Its own claim remains a suggestion. |

The attached rulings in the comparison files also remain intact: D21, D24, D27 and D28 retain no/yes; A04 retains no/yes; A13 retains no/no; A22 retains no/yes. A22 consequently changes the graders' agreed answer. The original credit answer in ruling 26 is superseded by S11, not retained as a competing rule. The synthesis and P13 also preserve cause recognition despite poor examples and give it no recovery credit.

## What the proposal would do

These are my predicted applications, not new grades or owner decisions. No entry counts as no when identifying the agreeing grader. “Still open” means cannot tell pending the stated boundary decision.

| ID | Proposed answer | Agrees with | Reason |
| --- | --- | --- | --- |
| D01 | no | Grader 2 | The requested fix supplies the discarded-error explanation. The claim and its attached description state the lost diagnostic. |
| D02 | yes | Grader 1 | The title and antecedent fault the codec rejecting the previously accepted message kind. |
| D03 | yes | Grader 2 | The criticized object gate is the gate in the known cause. |
| D04 | yes | Grader 2 | The same criticized gate also produces this context failure. |
| D05 | yes | Grader 2 | The sentence continuation identifies the criticized object gate. |
| D06 | yes | Grader 2 | The same gate causes this other context failure. |
| D07 | yes | Grader 2 | The attached explanation faults merging unchanged input through property copying. |
| D08 | yes | Grader 2 | That same copying loses optional markers. |
| D09 | yes | Grader 2 | The array explanation faults the copying that also loses branded string identity. |
| D10 | yes | Grader 2 | The attached explanation expressly faults copying that strips optional markers. |
| D11 | yes | Grader 2 | The missing no-op means unchanged input still goes through the criticized copy. |
| D12 | yes | Grader 2 | That criticized copy also loses optional markers. |
| D13 | yes | Grader 2 | The continuation and attached explanation fault the same property copying. |
| D14 | yes | Grader 2 | Building the context at import is a real step in the known failure, criticized for its cost. |
| D15 | yes | Grader 2 | The criticized unconditional import-time creation causes the no-ssl failure. |
| D16 | yes | Grader 2 | The named stored context is created at import, a real step in the known failure. |
| D17 | yes | Grader 2 | The criticized unconditional creation is the failing operation without ssl. |
| D18 | yes | Grader 1 | Registering the string is truly named and questioned. The false collection/reset readers do not erase it. |
| D19 | yes | Grader 1 | The attached words identify unconditional validation and the uncleared history flag. |
| D20 | yes | Grader 1 | The attached words identify registering the string as the cause. |
| D21 | yes | Grader 1 | The attached words fault the removed mount-time clearing branch. The attached ruling confirms yes. |
| D22 | yes | Grader 1 | The attached words fault the same removed clearing branch, for a mode-switch case. |
| D23 | yes | Grader 1 | The attached words identify the missing mount-time clearing and the absent first-render update. |
| D24 | yes | Grader 1 | The same skipped clearing path is faulted for a rejected keystroke. Ruling 30 confirms yes. |
| D25 | still open | still open | Does the backend-mismatch complaint also fault the shared hostname-setting mutation, or merely describe it? |
| D26 | yes | Grader 1 | The attached explanation faults code-driven validation and the changed-once flag. |
| D27 | yes | Grader 1 | The same validation/history mechanism is faulted for prefill. The attached ruling confirms yes. |
| D28 | yes | Grader 1 | The attached words fault registering String(value) for array comparison. Ruling 27 confirms yes. |
| D29 | yes | Grader 2 | The attached explanation faults passing one mutable context across sessions and expressly includes certificate-chain mutation. |
| D30 | yes | Grader 1 | The same criticized shared context receives pool settings. The known cause loads pool CA settings into it. |
| D31 | still open | still open | Is no-reset reuse itself a cause for this documentation problem, or must the claim fault the absent warning? |

The proposal settles 29 of the 31 supplied differences: 28 yes and one no. It agrees with grader 1 on 12 and grader 2 on 17. Two stay open. This prediction does not establish that two fresh graders will follow the text reliably.

## A cheap check before 199 batches

First use the fixed 31 differences and 24 agreed pairs, for 55 claim/problem pairs. Preserve their whole comments, exact prescribed quotations and answer-key entries. Remove the saved graders' answers, their reasons and the owner rulings from the test inputs. Supply the established facts needed to check causes, so an inability to run these repositories does not become a wording test. Do not let the models split claims again.

Give both model families three separate, fresh runs: the current rule; the current rule with only the new name; and the full proposed text. That is 330 pair judgments across the three conditions and two families. Keep everything else identical and shuffle the order. Require each cause yes to cite the comment's cause words and its objection, and briefly explain the match. Require an explicit record for every assigned pair, so omission cannot masquerade as agreement.

Check answers against the saved owner rulings and the predictions above. Include the ruled comments in `decisions/` that are absent from the 55 pairs as a separate small check, including neutral comment D and the forked-pool sentence. Hide their expected answers too. In particular, count correction of A22 as improvement, rather than loss of agreement. Keep the two open cases separate until the owner decides them.

Repeat the full-text condition once in fresh sessions. The first check works if both families reach the same predicted answers on the 29 settled differences, reproduce every supplied ruling on both facts, and keep those answers on repetition. The first fact must keep its existing credit decisions. Inspect any changed agreed answer against the rule rather than demanding preservation of an error. No answer may earn credit merely because its cause matches.

The name-only condition answers the owner's naming question. If it fixes the scope and shared-cause errors while the definition stays identical, that supports a naming effect. If improvement appears only with the full text, it supports clearer instructions rather than the name alone. A single run is weak evidence of either.

Finally try the full text on two previously untouched batches, chosen before seeing their comments. Use both families, preserve their claim lists, and inspect every new disagreement. These 55 examples helped form the proposal, so success on them alone is not enough. Proceed to the full regrade only after the fresh batches show that graders can identify the criticized cause without inventing a consequence or treating every shared helper as a shared fault. Persistent splits at the same boundaries mean the owner must settle those boundaries before the larger run.
