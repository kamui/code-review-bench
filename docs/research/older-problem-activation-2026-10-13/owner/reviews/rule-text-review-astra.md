**Judgment:** The draft needs changes before adoption. The main problems are paragraph 1a’s test for locating a fault, conflicting grading instructions, and incomplete recording of A3, A4, A6 and A8.

**Read** below marks quotations and evidence from the supplied files. **Judgment** marks my assessment and proposed wording.

## Findings

### 1. Paragraph 1a does not distinguish a wrong input classification from a fault inside the selected code

**Read, part 1a:**

> Some lines decide how the code handles each kind of input. When the change rewrites those lines, and the rewritten lines still pass a supported input to handling that breaks it, the fault is on the rewritten lines.

> When an input is rightly passed on and fails inside handling that the change did not touch, the fault is not on the rewritten lines.

**Judgment:** The second sentence includes the case the last sentence excludes. A supported input can be passed correctly to code that breaks it. “Rightly passed on” supplies no test for distinguishing that case from a wrong classification. “Lines that decide how the code handles each kind of input” could include almost any conditional, dispatch call or validation check. The first sentence adds no usable limit.

**Read:** [answers.md, A4](/home/jack/.t3/review-work/older-activation/text-review/answers.md) records the owner’s narrower reason: the change rewrote the checks sorting inputs into cases and left arrays and tuples in the wrong case. The explanation distinguishes an optional-field object that reaches the correct case and fails inside the unchanged copy. The owner’s later addition says an array test is unnecessary because the code supports the input and a user could reasonably use it.

**Judgment, concrete case:** A parser adds a CBOR branch and rewrites its dispatch check. Valid JSON still reaches the JSON parser, whose unchanged Unicode handling has an older fault. The second sentence puts the fault on the dispatch check; the last sentence can put it inside the parser. Two graders could reasonably disagree.

**Read:** [rules.next.md, Before 2](/home/jack/.t3/review-work/older-activation/text-review/in-force/rules.next.md) explicitly excludes Kubernetes’ rename and SeaweedFS’s unchanged skip. Paragraph 1a does not expressly preserve the rename exclusion. SeaweedFS’s recorded change adds a line rather than rewriting the skip, so the recorded facts do not satisfy 1a’s rewrite condition. The Astro example is under Before 3: it describes a weakness visible in the diff and later confirmation. It does **not** record an “outside the review” ruling.

**Judgment:** I would not claim that 1a necessarily reverses those examples. Its broad wording creates ambiguity about Kubernetes and nearby unchanged faults. Nothing supplied supports saying it pulls Astro back into the review.

**Judgment, proposed rewrite:**

> When the change rewrites a check that sorts inputs into cases, the check can itself contain an older fault. This happens when it assigns a supported input to the wrong case. The fault belongs to the review even if the same input failed before the change. Rewriting a line only to rename something does not count.
>
> Use the rules for “Promised?” to establish support from evidence available at the cut-off. A project test is not required when the accepted input type supports the use.
>
> If the input belongs in the selected case and the fault is inside unchanged code for that case, rewriting the check does not bring that fault into the review. Apply the other tests in Before 2 separately.

**Read, part 2a:**

> When the answer is "no" because the fault is older than the change, section 7 says whether a comment on it earns the separate credit.

**Judgment:** Age alone does not make the answer “no”, as the adjacent array example demonstrates.

**Judgment, proposed rewrite:**

> When rule Before 2 puts an older fault outside the review, section 7 says whether a comment on it earns the separate credit.

### 2. The candidate instruction can send an ordinary problem through the older-problem fields

**Read, part 3c:**

> When a true claim is about an older fault that is not listed, and the fault breaks something the project promised, set older_candidate to a new candidate id and add a new_candidates entry of kind "older-problem".

> Leave older_problems empty for the unlisted fault, and leave candidate null.

**Read:** Before 2 includes some older faults in the review. In particular, A4 retains the array and tuple fault as a known problem. [older_grading.py, `verdict_problems`](/home/jack/.t3/review-work/older-activation/text-review/in-force/older_grading.py) permits `older_candidate` only for a true `outside-this-change` claim. It also forbids combining an older candidate with listed older entries.

**Judgment:** The instruction omits the required outcome. An unlisted older fault on a substantively rewritten line could qualify as an ordinary candidate, yet this sentence directs the grader to use `older_candidate`. “Empty for the unlisted fault” also permits a reading where the same claim retains entries for listed faults, which the validator rejects.

**Read, part 2b:**

> Question 2 does not apply.

**Judgment:** This needs to say “to admission”. Graders still need the responsibility answer to select the correct candidate route. Also, “Question 2” means responsibility in the rubric but “Delivered?” in the appended rules.

**Judgment, proposed rewrite:**

> The responsibility question in section 2 does not apply to admission. It still decides whether the claim is outside this change.
>
> Use `older_candidate` only when `true` is `"yes"` and `outcome` is `"outside-this-change"`. If the unlisted fault meets the admission test, set `older_candidate` to its candidate ID. Add a `new_candidates` entry with `kind` `"older-problem"`. Keep `candidate` null.
>
> A claim with `older_candidate` must have empty `older_problems` and `known_problems` lists. Use the ordinary candidate instructions for a fault that belongs to the review.

**Judgment:** The draft also needs a procedure for an indivisible claim that concerns both a listed older problem and an unlisted candidate. The supplied validator forbids that combination; the text supplies no resolution.

### 3. The prohibition on naming both catalogs conflicts with the existing cause-matching rule

**Read, part 3c:**

> A claim can name several older problems. It cannot name an older problem and a known problem together.

**Read:** [scoring.next.md, section 3](/home/jack/.t3/review-work/older-activation/text-review/in-force/scoring.next.md) requires matching every known problem’s cause, even when the claim describes a different problem. Section 1 says to keep an explanation with the result it explains. Both the contract and validator prohibit one assertion from naming both catalogs.

**Judgment, concrete case:** One claim describes an older problem’s failed outcome and objects to a shared cause that also causes a known problem. Section 3 requires the known-problem entry; part 3c requires the older-problem entry and forbids their coexistence. Splitting the explanation from its result would violate section 1.

**Judgment:** The owner was not asked to choose a precedence rule for this case. The draft should expose the conflict rather than silently decide which match disappears.

**Judgment, proposed rewrite:**

> Split independent claims and grade each separately. Do not split an explanation from the result it explains. If one claim requires both an older-problem entry and a known-problem entry, refer the conflict to the user.

**Judgment:** Before activation, add an approved output procedure for that referral. The supplied schema does not establish one.

### 4. “Unresolved claim” and “unresolved credit” need separate instructions

**Read, part 2b:**

> A claim about an older problem keeps the outcome "outside this change".

**Read, part 3c:**

> Take both facts from the comment's own words, by the test of section 3 of the rubric.

> When you cannot reach the evidence for a claim about a listed older problem, the claim is unresolved. Keep its older_problems entry with says_what "cannot-tell".

**Read:** Rubric section 3 says `"cannot tell"` on “says what goes wrong” leaves the claim unresolved. The older validator instead accepts a true `outside-this-change` claim with `says_what: "cannot-tell"`. Its separate inaccessible-evidence route requires `outcome: "unresolved"`, `true: "cannot-check"`, no `older_candidate`, and every retained older entry’s `says_what` to be `"cannot-tell"`.

**Judgment:** Importing section 3 without limiting the reference to its two wording tests can import incompatible outcome rules. The final sentence also says “keep” an entry that the earlier true-claim gate may have prevented the grader from adding.

**Judgment, proposed rewrite:**

> Apply section 3’s tests for “says what goes wrong” and “identifies the cause as a fault”. Do not apply its known-problem outcome rules to an older problem.
>
> A true claim outside this change keeps `outcome` `"outside-this-change"`, even when its older credit is unresolved. Record uncertain wording with `"cannot-tell"`.
>
> If needed evidence is inaccessible, set `true` to `"cannot-check"` and `outcome` to `"unresolved"`. Add or retain the relevant `older_problems` entries. Set `says_what` to `"cannot-tell"` in every such entry. Keep `older_candidate` null. Leave unreached questions null and record what would settle the claim in `open`.

**Judgment:** Qualify the rubric sentence too:

> A true claim about an older problem keeps the outcome “outside this change”. An evidence access failure remains unresolved.

### 5. The display paragraph omits an approved gate and loses existing availability rules

**Read, part 2b:**

> It withholds the final counts while a review awaits assessment, while a credit is unresolved, or while a candidate for the task awaits admission.

> When every review is assessed and none has a credit, the count is zero and carries no penalty.

> The scorecard never shows the credit as a share of all older faults, and never combines it with the results for known problems, a ranking or a recommendation.

**Read:** A8 expressly chooses that a pending candidate withholds its task’s older counts **and limits a recommendation**. It also chooses the controls wording “without a refuted or unproven claim”.

**Read:** [older-problem-credit.md, Scorecard](/home/jack/.t3/review-work/older-activation/text-review/in-force/older-problem-credit.md) distinguishes observed counts, final counts and catalog size. No catalog or no admitted review makes final credit unavailable. Incomplete work preserves observed counts. Pending candidates affect every concern selection of their task. Repeated scheduled trials can earn separate credits; failed or superseded attempts cannot.

**Read:** The earlier proposed rubric also explicitly lists missing selection, pending trials and unavailable assessments among the gates.

**Judgment:** “Every review is assessed” can produce zero for an empty review set. “Final counts” can be read to include catalog size. The paragraph omits the recommendation gate and the approved controls correction. Its prohibition on blending credit with recommendations is sound, but does not explain that pending candidates still limit recommendations.

**Judgment, proposed rewrite:**

> Apply the task and concern filters to the panel. Count only admitted reviews. Separate scheduled trials count separately. Failed or superseded attempts earn no credit.
>
> Show observed credits and assessment coverage separately from final credit counts. Keep the number of admitted older problems visible.
>
> Final credit counts are unavailable when there is no catalog, no admitted review, no selection, incomplete assessment, a pending trial, unresolved credit, or a pending older-problem candidate. A pending candidate withholds its task’s final older counts and limits a recommendation. It applies to every concern selection of that task.
>
> Show zero only when an eligible selection is fully assessed, no gate remains, and no review earns credit. Zero carries no penalty.
>
> Credit counts do not affect results for known problems, rankings or recommendations. Pending candidates retain their separate effect on recommendations.
>
> The controls line reads “without a refuted or unproven claim”.

### 6. The admission paragraph overstates when an announcement ends a promise

**Read, part 2b:**

> A choice the project stated by the cut-off ends the promise, as rule Promised 2 says.

**Read:** Promised 2a preserves a promise when something else, such as documentation, still promises it. Promised 2b requires the announcement to address the specific rule, rather than merely state a general aim. Promised 2c requires an accessible statement by the cut-off. A2 adopts the existing test.

**Judgment:** The sentence drops those qualifications. For example, “simplify validation” cannot end a specific documented validation promise merely because the choice was stated.

**Judgment, proposed rewrite:**

> Apply rule Promised 2 to decide whether an announcement ends the particular promise. A general aim is not enough. A conflicting promise still counts. An unstated intention ends no promise.

**Judgment:** Add this reminder if section 7 is intended to stand on its own:

> Later fixes may confirm evidence available at the cut-off. They cannot supply the reason for admission.

**Read:** That reminder is already supported by Before 3 and A2; it adds no new policy.

### 7. The examples omit the candidate decisions and the class-instance exclusion

**Read, part 1c:**

> The checks rightly pass an object with an optional field to the copy, so the fault is outside the review (activation 6, which changed second-pass 7).

> So the fault belongs to the review, although the same input failed before the change too (activation 7).

**Read:** A3 chooses “Outside, older-problem candidate”. A4 retains GT-j7 as other-material for arrays and tuples only. It moves the class-instance use outside this change and sends it forward as an older-problem candidate with GT-j5.

**Judgment:** The draft records the optional field’s exclusion but not its candidacy. It never records the class-instance decision. The omitted negative example is particularly useful for understanding “rightly passed on”.

**Judgment, proposed addition:**

> GT-j5 is outside this change and is an older-problem candidate. It has not been admitted.
>
> GT-j7 remains a known problem, labelled other-material, for arrays and tuples only. The class-instance use is outside this change and is an older-problem candidate. It has not been admitted.

**Judgment, proposed addition to the array explanation:**

> The accepted input type supports arrays and tuples. A user can reasonably use them this way, even without a project test showing the use.

**Read:** A4’s later addition supports that reason. Promised 6 already distinguishes accepted input types from merely being able to assign to a variable.

### 8. The label rule omits when the owner decides it and conflicts with the unchanged contract

**Read, part 2b:**

> The user gives each older problem the label, serious or other-material, that the same fault would get if the change had introduced it.

**Read:** A6 chooses a label **with each admission**, after preparing an impact card and a blind label. The accompanying explanation says labels stay out of grading inputs. The contract’s Contracts section currently says an older-problem entry “has no impact band”. Part 4 proposes adding text to that page without removing that sentence.

**Judgment:** The counterfactual label rule is faithful. Its workflow is incomplete, and appending it leaves a direct contract contradiction.

**Judgment, proposed rewrite and companion correction:**

> Prepare an impact card and an independent blind label before asking for admission. The user decides admission and the serious or other-material label together. Apply the impact boundary as if the change had introduced the fault. Keep the label out of grading inputs. Both labels earn one credit.

**Judgment:** Replace the contract’s “It has no impact band” sentence with the adopted label requirement. Part 4’s exclusion of exception 5 and reading rule 3 is a reasonable application of A6’s hypothetical, rather than a separate change to ordinary labels.

### 9. “Claims nothing about the change” can discard the comments A1 makes eligible

**Read, part 3c:**

> When a claim is true and its outcome is outside-this-change, check it against each older problem.

**Read:** Rubric section 1 and the grader instructions still classify an item that “claims nothing about the change” as `not-a-finding`, with no claims. A1 allows older problems anywhere in the project.

**Judgment, concrete case:** “Unrelated to this patch: method override ignores lowercase method names” can be discarded before part 3c runs. It names a project fault while explicitly denying a connection to the change. Another grader could retain it for older-problem assessment.

**Judgment, proposed replacement for the filtering instruction in both files:**

> A comment that alleges a fault or proposes an improvement has claims, even when it says the issue is outside this change. Keep those claims for grading. A plan, greeting, progress note or summary with no assertion of its own is not a finding.

### 10. The schema prose needs repair, and the supplied validator does not enforce exactly that prose

**Read, part 3b:**

> Each new_candidates entry has exactly id, kind, claim, evidence, limits, relevance, would_settle and items. kind is "ordinary" or "older-problem".

**Read:** The next unchanged sentence in [grader.next.md](/home/jack/.t3/review-work/older-activation/text-review/in-force/grader.next.md) says “The first six are non-empty strings”. Inserting `kind` changes which fields those words cover and leaves `would_settle` outside that requirement.

**Judgment, proposed rewrite:**

> Each `new_candidates` entry has exactly `id`, `kind`, `claim`, `evidence`, `limits`, `relevance`, `would_settle` and `items`. `kind` is `"ordinary"` or `"older-problem"`. `id`, `claim`, `evidence`, `limits`, `relevance` and `would_settle` are non-empty strings.

**Read, part 3c:**

> Add an older_problems entry for each one where either fact is "yes" or "cannot-tell". An entry has exactly problem (the older problem's id), says_what, identifies_cause and reason.

**Read:** The validator requires a listed ID, unique IDs within the list, literal values `"yes"`, `"no"` or `"cannot-tell"`, and a non-empty string `reason`. It does **not** reject an entry whose two facts are both `"no"`. It does not inspect `new_candidates`, validate their `kind`, or resolve `older_candidate` to an entry.

**Judgment:** Therefore the answer to “does this validator accept exactly what the grader is told to write?” is no. Some differences require clearer instructions; others require validation elsewhere or a validator change.

**Judgment, proposed rewrite:**

> Add at most one entry for each listed older problem. Add it only when at least one fact is `"yes"` or `"cannot-tell"`. Each entry has exactly `problem`, `says_what`, `identifies_cause` and `reason`. Both facts use `"yes"`, `"no"` or `"cannot-tell"`. `reason` is a non-empty string explaining both answers.
>
> `older_candidate` is null or a non-empty ID that names a `new_candidates` entry with `kind` `"older-problem"`. The claim’s own `kind` remains null.

**Read:** The supplied module delegates ordinary claim validation to `claim_grading`, whose source is not included here.

**Judgment:** This review establishes the visible older-field constraints, not end-to-end validation of candidate records. Do not describe `older_grading.py` alone as enforcing the complete output contract.

## Sentences checked and found sound

**Judgment:** These sentences are sound as written in their stated context. They do not resolve the omissions identified above.

- **Read, 1a:** “It belongs to the review even though that input failed before the change too.” **Judgment:** Faithful to A4 when the fault is in the rewritten classification.
- **Read, 1b:** “A comment on a fault that is outside the review can still earn the separate credit for older problems, once the user has admitted the fault (rubric, section 7).” **Judgment:** Sound.
- **Read, 1b:** “Saying nothing about the fault costs nothing.” **Judgment:** Sound.
- **Read, 1c:** “tRPC's type helper copies an object key by key, and that copy makes an optional field required.” **Judgment:** Consistent with A3.
- **Read, 1c:** “The change rewrote the two checks above the copy and left the copy as it was.” **Judgment:** Consistent with A3’s recorded diff evidence.
- **Read, 1c:** “An array or a tuple is a supported input.” **Judgment:** Consistent with A4 and Promised 6.
- **Read, 1c:** “Django's `login()` is unchanged and still discards the session after a key rotation.” **Judgment:** Consistent with A5.
- **Read, 1c:** “The release note that the change adds says the change fixed ‘invalidation of sessions when rotating secret keys’, and names no exception.” **Judgment:** Consistent with A5.
- **Read, 1c:** “`login()` is part of what the change says it fixes, so the fault belongs to the review (activation 8).” **Judgment:** Faithful to A5’s reason.
- **Read, 1d:** “tRPC accepts a Zod branded string as an input type.” **Judgment:** Preserves the existing example.
- **Read, 1d:** “A resolver that then cannot use it as a string has a problem (second-pass 6).” **Judgment:** Preserves the existing ruling.
- **Read, 1d:** “tRPC promises an optional input field in the same way.” **Judgment:** Clear in its accepted-type context.
- **Read, 1d:** “The fault with that field is older than the change and is outside the review (activation 6).” **Judgment:** Faithful to A3.
- **Read, 2a:** “The array failed before the change too.” **Judgment:** Consistent with A4.
- **Read, 2b:** “An older problem is a fault that was in the project before the change, that is outside the review by rule Before 2, and that the user has admitted.” **Judgment:** Sound definition.
- **Read, 2b:** “It can be anywhere in the project.” **Judgment:** Faithful to A1.
- **Read, 2b:** “The user admits an older fault when the answer to ‘Promised?’ is yes and the answer to ‘Delivered?’ is no, judged on what a reviewer could know at the cut-off.” **Judgment:** Faithful to A2.
- **Read, 2b:** “An intention that nobody stated does not end the promise.” **Judgment:** Faithful to A2.
- **Read, 2b:** “The user does not admit a fault that would be a minor defect or a suggestion.” **Judgment:** Consistent with A2 and A6.
- **Read, 2b:** “Admission, the grouping of two faults as one older problem, and a disputed credit are the user's decisions.” **Judgment:** Consistent with the contract.
- **Read, 2b:** “An agent never settles them.” **Judgment:** Consistent with the contract.
- **Read, 2b:** “The label does not change the credit.” **Judgment:** Faithful to A6.
- **Read, 2b:** “It earns the credit when its own words say what goes wrong, by the test of section 3.” **Judgment:** Sound for a true claim about an admitted older problem.
- **Read, 2b:** “The grader also records whether the claim identifies the cause as a fault.” **Judgment:** Sound.
- **Read, 2b:** “That second fact does not affect the credit.” **Judgment:** Sound.
- **Read, 2b:** “One review earns one credit for one older problem, however many of its comments raise it.” **Judgment:** Sound.
- **Read, 2b:** “A review that does not raise an older problem has no miss, and the scorecard counts nothing against it.” **Judgment:** Sound.
- **Read, 2b:** “A task whose only entries are older problems is still a clean control.” **Judgment:** Faithful to A8.
- **Read, 2b:** “A false claim, an unresolved claim, an unsafe fix and a candidate that awaits a ruling keep their ordinary effects.” **Judgment:** Consistent with the contract.
- **Read, 2b:** “A comment that rightly points at an older fault and wrongly says the change introduced it holds two claims.” **Judgment:** Sound for the independent allegations described.
- **Read, 2b:** “The first is outside this change and the second is refuted.” **Judgment:** Sound in that older-problem context.
- **Read, 2b:** “The scorecard shows older-problem credit in its own panel.” **Judgment:** Faithful to A8.
- **Read, 2b:** “The panel gives the credits, the older problems raised and the reviews with a credit, each split by label.” **Judgment:** Consistent with A6 and A8.
- **Read, 2b:** “It also gives the number of older problems and the number of reviews assessed.” **Judgment:** Sound, though the coverage description needs the additions above.
- **Read, 3a:** `"older_problems": []` and `"older_candidate": null`. **Judgment:** Both mandatory fields match the validator.
- **Read, 3b:** `kind is "ordinary" or "older-problem".` **Judgment:** The candidate values match the contract.
- **Read, 3c:** “references.json lists the older problems of this task under older_problems.” **Judgment:** Matches the contract.
- **Read, 3c:** “The user has admitted each one.” **Judgment:** Matches the catalog’s admission requirement.
- **Read, 3c:** “They are not known problems.” **Judgment:** Correctly distinguishes the catalogs.
- **Read, 3c:** “A review that says nothing about one has missed nothing.” **Judgment:** Sound.
- **Read, 3c:** “A claim that identifies the cause as a fault and does not say what goes wrong earns no credit.” **Judgment:** Sound.
- **Read, 3c:** “When a comment rightly points at an older fault and wrongly says the change introduced it, split it into two claims, one outside-this-change and one refuted.” **Judgment:** Consistent with the contract.
- **Read, 3c:** “Judge a proposed fix separately.” **Judgment:** Preserves rubric section 5.
- **Read, 3c:** “A harmful fix does not take the credit away.” **Judgment:** Consistent with separate claim and fix assessment.
- **Read, 3c:** “In the entry's evidence, say what was promised, when the fault occurs, what causes it and what the person does not get.” **Judgment:** Supplies useful admission evidence.
- **Read, 3c:** “Never give credit for a candidate, and never treat a ruling that a fault is outside the review as an admission.” **Judgment:** Preserves owner admission authority.
- **Read, 3c:** “Follow a ruling in claims.md that settles one comment's credit for an older problem.” **Judgment:** Consistent with the contract.
- **Read, 4:** “The fifth exception and the third reading rule of the boundary compare the change with the version before it.” **Judgment:** Accurate description of the boundary.
- **Read, 4:** “The user labels an older problem as if the change had introduced the fault, so neither applies.” **Judgment:** Consistent with A6’s impact-based hypothetical.