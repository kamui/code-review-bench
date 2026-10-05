# Proposal: the two questions that sort a review comment

For the owner to accept or reject. Nothing in the repository was changed.

## Recommendation

Rename the two questions **Promised?** and **Delivered?**

| Promised | Delivered | Outcome |
| --- | --- | --- |
| yes | no | Problem, on the answer key (then banded) |
| yes | yes | Minor defect |
| no | not asked | Suggestion or observation (improvement, or outside supported use) |

- **Promised?** Did the project give the people affected reason to rely on the software behaving differently from what happens here?
- **Delivered?** Does a person using it in the promised way get the outcome the promise is for: right, complete, and when they ask for it?

The structure you accepted in P8 does not change. The three outcomes keep their names. The band stays a third, separate step.

One thing to know before you accept: the second column flips. Today's "Lost: yes" becomes "Delivered: no". I think the clearer reading is worth it, and I say why below.

## 1. Names

### What a name has to do here

The record shows two failures that a name can help prevent.

- **Ruling 2 and review 5:** the agents had the breakage and had not looked for the promise. A good first name makes the agent go and find the promise, or its absence, and cite it.
- **Reviews 1 and 5:** the recorder read "Lost" as "someone ended up worse off" and recommended a minor defect both times. A good second name must not suggest harm, and must not suggest a before-and-after comparison. You can only lose what you had. That is how "nobody is worse off" crept into the second question.

### Question 1

| Name | Verdict | Why |
| --- | --- | --- |
| **Owed?** (current) | Second place | Neutral and blame-free. But it does not say what is owed. The record already uses it two ways: behaviour the project owes users, and "whether a correction was owed". An agent can answer it from sympathy: people are affected, so something is owed. That was the ruling 2 mistake. |
| **Supported?** | Loses | The most natural word, and "supported use" stays in the prose. As a name it collides with the `unsupported` claim outcome and the rubric's "support" test. `supported: no` beside `unsupported` in one record is a trap. |
| **Covered?** | Loses | Reads as test coverage, or as "covered by the answer key". In this project that is nearly the opposite meaning. |
| **Expected?** | Loses | Your own test in review 5 was "the expectation a user could reasonably have". But "expected behaviour" in software means "works as designed". `expected: yes` would be read backwards. |
| **Reason to rely?** | Loses | It is the wording of your unusual-input rule, so I use it in the definition. As a name it suggests someone must actually be relying. Review 5 was a problem with no application shown using `cancel()` on a field. |
| **Obligation?** | Loses | The ADR's word, and correct. Too abstract for a first line, and no plainer than "owed". |
| **Promised?** | **Wins** | Plain. Already the project's word ("partly kept promises", "a promised outcome" in the definition you liked). It forces a citation: which promise, made where. It also explains the table's third row by itself: where nothing was promised, there is nothing to deliver, so the second question is not asked. With "Owed/Lost" that row is not obvious. In ruling 2 people did lose something. |

The risk with "Promised" is that it reads as "written down". Your rulings count unwritten promises (ruling 35, review 3, review 6). The definition answers this by naming five ways a promise is made. See also the strongest objection in section 6.

### Question 2

| Name | Verdict | Why |
| --- | --- | --- |
| **Lost?** (current) | Loses | Suggests harm and a before-and-after comparison. Both belong to the band. It also collides with "data lost" in the serious band and with the "already lost before the change" exception. |
| **Broken?** | Loses | "Promise broken" is a good idiom. But the band reasoning already says "nothing that worked before broke" (second-pass rulings 6 and 7). An agent would answer "no, it never worked, so nothing broke" on review 2. That is the first-round error again. |
| **Failed?** | Loses | Suggests a crash or an error. "Nothing fails" was the first-round ground for advice on ruling 14, which you later made a problem: a wrong address in page metadata, with no failure anywhere. |
| **Kept?** | Loses | Good idiom, and it matches "partly kept promises". But rulings say "the ruling is kept" and "kept off the answer key" on the same page. |
| **Unmet?** | Loses | Right meaning, right direction. `unmet: no` is a double negative, and agents will slip on it. |
| **Delivered?** | **Wins** | It is the word the record already uses for exactly this: "its outcome is not delivered" (the discussion record), "every promised result is still delivered" (the current rule). It says nothing about harm or about before and after. A wrong value, a missing result and a blocked operation all read naturally as "not delivered". No collision found. |

### The cost of "Delivered": the column flips

"Delivered: no" is the bad answer. So the table is no longer "yes, yes = problem".

Why I still recommend it:

- "Promised: yes. Delivered: no." needs no definition. "Owed: yes. Lost: yes." does.
- "Promised: yes. Delivered: yes." tells the reader what a minor defect is: the user got what was promised, and something is still wrong on the way.
- Only three combinations are valid. A record check can reject the rest.

If you want the old direction kept, the fallback is **Promised? / Not delivered?** I think it reads worse.

### Answers and fields

- Fields: `promised` (yes, no) and `delivered` (yes, no, or empty when not asked).
- When `promised` is yes, record where the promise comes from: `written`, `announced`, `built`, `established` or `practice` (defined in section 2).
- When `promised` is no, record the kind, as P8 already does: `improvement` or `outside supported use`. Store the second as `outside-supported-use`, not `unsupported-use`, to stay clear of the `unsupported` claim outcome.

### The three outcome names

Keep all three. You accepted them today and chose not to rename.

One caution on "minor defect". "Minor" is a size word, and size is the band's business. Define it by the table, not by size: **a minor defect is a fault in promised use where the promised outcome is still delivered.** It is not a small problem and it is not a third band.

## 2. Definitions and rules

### Before the questions

These are not new. They are the rubric's first two tests.

1. **The facts are established.** If a needed fact is unknown, such as how an outside platform behaves, the comment is unproven or unresolved. It is not a "no" on either question. (Ruling 15 before the live check; ruling 17.)
2. **The fault belongs to this review.** This is your older-faults rule, unchanged. An older fault is in if the change introduces it, worsens it, makes it reachable, makes it show sooner or more clearly, or leaves it in lines it touches or in the thing it says it fixes. An older fault the change does none of that to is outside review scope. A fault no supported request can reach at this change, which a later change to the project opens, belongs to the later change (ruling 18).
3. **What evidence counts.** Judge on what a reviewer could see before the merge. Later evidence may confirm three things: that a weak point visible in the diff really fails (review 4), that a practice already existed at the merge (reports from code written before it), and what the authors intended (the author later calling an error "noise", ruling 22). Later evidence may not supply a promise, and may not withdraw one. A maintainer saying "that order is misuse" after the reports come in does not undo ruling 31. A visible weak point with no confirmation stays unproven (ruling 17).

### Question 1: Promised?

**Did the project give the people affected reason to rely on the software behaving differently from what happens here?**

Ask it about the exact use that goes wrong, not the feature as a whole. How common the use is does not decide it.

A promise is made in five ways:

- **Written.** The project's documentation describes or offers the use.
- **Announced.** This change says so: its description, release note, a code comment stating purpose, or the documentation and tests it adds.
- **Built.** Code, a test or a type exists to make that use work.
- **Established.** It worked before the change for ordinary or documented use.
- **Practice.** Users demonstrably do it, and no owner has told them to stop.

Rules, in order. The first that applies decides.

1. **An owner's "no" beats practice.** If the use is undocumented, and the project or the dependency that owns the interface called it private, unsupported or "do not do this" before the merge, the answer is no. It stays no however many people do it and however many reports follow (second-pass ruling 2). Assigning to a module's variable is not a supported interface unless documentation offers it as a setting; documented for reading is not documented for writing (ruling 30).
2. **An announcement takes away only what it names.** Behaviour the change announces by name, where a reviewer could read it, is not promised otherwise (ruling 24; the self-healing cases of ruling 13). A general aim does not cover a consequence it never mentions (ruling 22; the recursive delete of ruling 13). Deliberate is not announced: a choice the author made, or defended in review, without telling users withdraws nothing (rulings 35 and 37, review 1).
3. **Invalid input from the caller is not promised.** That means input the specification or the declared type rules out (rulings 4, 8, 28). Two exceptions. The invalid input has become a pattern senders rely on (your condition in ruling 4). Or the input comes from outside and the software exists to check it: its promise to reject or survive bad data stands (GT-u1, GT-u5).
4. **Written counts, at the level it is written.** A use the documentation offers is promised even when rare and even when discouraged as style (ruling 39). A general contract, such as a handbook, covers every component that exposes the interface, unless that component's own documentation states an exception. Undocumented behaviour of one component does not narrow it. Where the component cannot honour part of it, it still holds for the part the component controls (review 5: the text stays typed, the dirty and filled marks are the component's).
5. **Announced counts, at the breadth it is written.** A flat sentence in a description or a purpose comment is a promise for the ordinary forms of that use. Narrower documentation elsewhere does not shrink it (review 2; ruling 1; review 5). It promises only what it speaks about (ruling 6: nothing was said about arrays).
6. **Built counts.** This covers a composition the project's own tests use (review 6), a code path that exists for the case, read with documentation that reasonably includes it (ruling 35), a valid option of the platform (second-pass ruling 9), an input the accepted type allows (second-pass rulings 6 and 7), and public documented calls used together, including at the same time (ruling 3).
7. **Established counts for ordinary or documented use.** What worked before is promised unless rule 2 took it away (ruling 22, review 1).
8. **Practice counts when three things hold.** It is a variation on a documented use or a call to a public interface. Users are shown doing it. No owner said no before the merge (review 3; ruling 31). Without shown users, an undocumented order or route is not promised (second-pass rulings 1 and 3; ruling 20).
9. **Otherwise, no.** Record the kind. *Improvement*: nothing stops working; the comment says it could be better (ruling 19). *Outside supported use*: something does stop working, for a use no promise reaches (second-pass ruling 2).

### Question 2: Delivered?

**Does a person using it in the promised way get the outcome the promise is for: right, complete, and when they ask for it?**

Asked only when Promised is yes. Rules, in order.

1. **Do not weigh harm.** Not who is hurt, how many, how badly, how likely, or whether they are worse off than before. That is the band. The only reach test is that a person in promised use can get there. A run, the code or a report shows it. A report is not required.
2. **Wrong, missing or blocked is not delivered.** An operation fails or will not build. A value is wrong, however small (ruling 14). Data or a protection is lost. An operation reports success for something it did not do (ruling 13). A documented instruction does not work as written (rulings 40 and 41). A needed message is absent or says the wrong thing (GT-u1). A test no longer protects what it names (GT-k1). A documented state mark is wrong, even when its only effect is styling (review 5, GT-r2).
3. **Partly delivered is not delivered**, for the part left out (ruling 1, review 2, review 5, second-pass ruling 6).
4. **Fails first and works on retry is not delivered** (review 3). The promise is for when it is asked. A workaround, or another route that works, does not deliver the promised one (second-pass ruling 7).
5. **Never delivered before is still not delivered** (review 2, review 6, second-pass rulings 6 and 7). "No worse than before" is band evidence.
6. **The software's own report of state must be true and on time.** An error or mark shown when the documented rules say it should not be is not delivered (ruling 22, review 6).
7. **Two promises that conflict.** Find the one that governs: the one the documentation ranks first; failing that, the one more specific to this use; failing that, the one the user's own explicit action calls on. If the governing promise is delivered, the answer is yes, and the other promise's wording being untrue here is the fault. If neither governs, the answer is no. (Review 5, the controlled field: the app stored the value, so "the field follows the stored value" governs and is delivered. The description's flat sentence is untrue for that path. Minor defect.)
8. **A resource cost.** More memory or time is not delivered when all four hold: a run shows it on a workload the project presents the feature for; it is large enough that work which fit before can stop fitting; no one announced it as a tradeoff; and nothing shows it is the unavoidable price of the fix. That the fix is sound does not change this (review 1). A cost nobody could notice is not a fault at all, and is "Promised: no, improvement" (ruling 19).
9. **Otherwise it is delivered.** Everything promised arrives and the fault is only one of these: an extra message that reports nothing false and blocks nothing (second-pass ruling 9); redundant work; a leftover that cleans itself up with no wrong result shown to anyone; a diagnostic that moved to another place the software writes by default (ruling 44). The outcome is a minor defect.

The nine cases you asked for:

| Case | Decided by | Answer |
| --- | --- | --- |
| Undocumented practice on an interface its owner calls private or discourages | Promised, rule 1 | Not promised |
| A description or code comment broader than the documentation | Promised, rule 5 | Promised, at the breadth written |
| A general handbook contract against one component's behaviour | Promised, rule 4 | The handbook governs unless the component documents an exception |
| An older fault left alone, exposed or made to fire sooner | "Before the questions", item 2; Delivered, rule 5 | In the review; then the same two questions |
| Evidence from after the merge | "Before the questions", item 3 | Confirms; never makes or removes a promise |
| A resource cost of a sound fix | Delivered, rule 8 | Not delivered when measured, large, unannounced and avoidable |
| Two promises that conflict | Delivered, rule 7 | The governing promise decides |
| Fails once, works on retry | Delivered, rule 4 | Not delivered |
| A harmless extra message | Delivered, rule 9 | Delivered; a minor defect |

## 3. Fit to the evidence

"Matches" compares with your final ruling. P = Promised, D = Delivered.

### Second-pass rulings

| Ruling | P | D | Outcome | Matches |
| --- | --- | --- | --- | --- |
| 01 pyOpenSSL after import | no: the order is outside the dependency's documented one, and no user is shown doing it | n/a | Observation, outside supported use | Yes, narrowly. See below. |
| 02 cipher defaults | no: private, and discouraged since 2017 | n/a | Observation, outside supported use | Yes |
| 03 key logging after import | no: the documented way still works, and no user is shown doing it | n/a | Observation, outside supported use | Yes |
| 04, 05, 08 | These ask whether a comment catches a family, not how a fault is sorted. The questions do not apply. | | | Not applicable |
| 06 branded string | yes: built, a documented Zod type | no: will not compile | Problem | Yes |
| 07 optional field | yes: built, the project's own "with optional keys" test | no: the valid call will not compile | Problem | Yes |
| 09 KSH_ARRAYS error line | yes: announced source method, valid option | yes: completion loads and works | Minor defect | Yes |
| 10 second `parseBody()` | A grouping ruling: part of GT-p1, which is already a problem. Not sorted separately. | | | Not applicable |

### The six reviews

| Review | P | D | Outcome | Matches |
| --- | --- | --- | --- | --- |
| R1 (5) upload memory | yes: written (uploads) and established | no: rule 8 | Problem | Yes. Rule 8 was written from this one case. |
| R2 (10) `source _rg` | yes: announced, in the description and a comment | no: the error the change set out to remove | Problem | Yes |
| R3 (11) renamed file | yes: practice, with zsh's own mechanism | no: the first Tab does nothing | Problem | Yes |
| R4 (16) unchecked path | yes: cached pages on Vercel | no: weak point visible in the diff, confirmed later | Problem | Yes |
| R5 (21) uncontrolled field | yes: written (handbook) and announced | no: dirty and filled are still set | Problem | Yes |
| R5 (21) controlled field | yes | yes: the governing promise is delivered | Minor defect | Yes. Rule 7 was written from this one case. |
| R6 (27) combobox | yes: built, a tested composition | no: a valid submit is blocked and a false error shown | Problem | Yes |

### First-round rulings that were advice, or were changed

| Ruling | P | D | Outcome | Matches |
| --- | --- | --- | --- | --- |
| 4 duplicate Content-Type | no: invalid input, not a relied-on pattern | n/a | Observation, outside supported use | Yes |
| 5, 10, 11, 16, 21, 27 | See R1 to R6 | | | Yes |
| 6 array typed as a long object | no: nothing said about arrays, nothing stops working | n/a | Suggestion, improvement | Yes |
| 8 nil message sent as empty | no: the caller's invalid input; the later note confirms intent | n/a | Observation, outside supported use | Yes |
| 13 recursive delete | yes | no: reports success, the file survives | Problem | Yes |
| 13 the two self-healing cases | no: the description announces the three-step form and that it converges | n/a | Suggestion, improvement | Yes, narrowly |
| 14 `x_astro_path` on the address | yes | no: a wrong address in page metadata | Problem | Yes |
| 15 percent-escapes | yes: valid addresses | no: 500, 404, or the wrong page | Problem; the `+` and `&` parts are refuted | Yes |
| 18 dormant crash on a body | no: no supported request reaches it at this change | n/a | Observation, outside supported use | Yes |
| 19 lock wait | no: nothing is at fault | n/a | Suggestion, improvement | Yes |
| 20 prevented input event | no: script-made events on a controlled field have no documentation, test or shown user | n/a | Observation, outside supported use | Yes |
| 24 validator gets the app's value | no: announced by name | n/a | Suggestion, improvement | Yes |
| 26 disabled control validates | no: the stated exemption is for a disabled field, not a disabled control | n/a | Suggestion, improvement | Yes, weakly |
| 28 null value | no: outside the declared type | n/a | Observation, outside supported use | Yes |
| 30 reassigned certificate path | no: the name is documented for reading only | n/a | Observation, outside supported use | Yes |
| 44 cause only in the pool log | no: nothing promises the exception's text. If you read it as yes, it is delivered, because the cause is logged on every attempt. | n/a | Off the answer key either way | Yes |

### The blind test, recounted

After your six changes, the old wording applied blind matches you on 41 of 43 cases (Sol) and 40 of 43 (Astra). The three left over are the controlled field of ruling 21, ruling 22 and ruling 30. Rules 7, 2 and 1 above decide them. That is by construction. I wrote the rules after reading the rulings, and I could not run a model. It is not a test.

### Cases that do not fit cleanly

No ruling is contradicted. Five fit only narrowly. For each I say which should give way.

1. **Second-pass ruling 1 (pyOpenSSL).** It fits on "not promised" alone. The ground recorded at the time, "nothing fails and both checks hold", is the old meaning of Lost and no longer carries weight. If "Promised" were yes, it would be a problem: the backend the user asked for is silently not used. One assessor gave it low confidence. Keep the ruling and record the new ground. It is the first one I would show you again.
2. **Ruling 26 (disabled control).** The assessors split. It is the same mechanism as ruling 22, which is a problem. I would show it to you again and not write a rule for it.
3. **Ruling 13, the listing that misses a file once.** This looks like "fails once, works on retry". It fits only because the description announced the three-step form. If you read that announcement as silent about a missed listing, the ruling gives way, not rule 4.
4. **Review 5, the controlled field.** Rule 7 exists for this case. Both blind assessors called it a problem. If rule 7 looks too clever, the simpler course is to make this case a problem too, and say that conflicting promises are not delivered.
5. **Review 1 (memory).** Rule 8 rests on one case and one sentence of yours, which said "likely". The evidence has two points: microseconds (not a fault) and 40 to 60 percent more peak memory (a problem). Where the line sits between them is not known.

Two distinctions are mine and rest on few cases. Say so if they look wrong.

- A public function called in an undocumented order with users shown doing it is promised (ruling 31). Assigning to a module variable is not (ruling 30, second-pass ruling 2).
- A later outside event can confirm a weak point visible in the diff (review 4). A later change to the project itself cannot (ruling 18).

## 4. What changes in the written rules

| Existing rule | Effect | What changes |
| --- | --- | --- |
| Unusual input (P1) | Restated and extended | Becomes Promised rules 1, 3, 4, 6 and 8. It is no longer limited to regressions. New: an owner's "no" beats practice. Clarified: "nothing shows users producing it" matters only for undocumented use. A documented use needs no shown user (review 5). |
| Older faults (P4) | Restated; one clause replaced | The scope test is unchanged. "Someone is harmed in supported use" becomes "promised and not delivered". "Not counted when the impact is trivial" goes: every correct comment now gets a label, and trivial ones are minor defects or suggestions. |
| Partly kept promises (P5) | Replaced by Delivered rules 1 and 3 | "Can realistically take a real loss" becomes "a person in promised use can reach the part left out". Still shown by a run, the code or a report. Missing reports no longer count toward "not a problem" here. They remain evidence about whether a practice exists, and evidence for the band. The first-round rule from ruling 21, "a real bug needs someone shown to be worse off", is withdrawn. Review 5 overturned it. |
| Documentation gaps | Restated | Delivered rule 2. A requirement the instructions leave out, which ordinary use will not meet, is not delivered. |
| Review-time information (P7) | Narrowed | "Before the questions", item 3. Your words stand. Review 4 adds the test for "confirms a suspicion": the weak point must be visible in the diff. It also adds the limits: later evidence makes no promise and removes none. |

The grader rubric's four tests map like this:

| Rubric test | Becomes |
| --- | --- |
| Support | The first check before the questions. Unchanged. |
| Change attribution | The second check before the questions. The rubric's wording is narrower than your older-faults rule; the finding-threshold page already says so. |
| Supported reachability | **Promised?** Wider than the old test: it also asks whose interface it is and what was said about it. |
| Material consequence | **Delivered?** This is the real change. The old test invited weighing harm. The new one asks only whether the promised outcome happened. Harm moves entirely to the band. The word "material" then lives only in the band name. |

So: eligible means supported, in this review, promised, and not delivered. The rubric text should change with the relabel and its single regrade, not before.

## 5. The first line of a recommendation

The exact wording:

> **What happens:** [one clause]. **Promised: yes / no**, [which promise and where it is made, or why there is none]. **Delivered: no / yes / not asked**, [the outcome that did or did not happen]. **So: problem / minor defect / suggestion or observation ([kind]).** **Band, decided separately:** [serious / other-material, with the reason; or "none"].

Two rules for whoever writes it:

- Name the source of the promise, or name what was searched to find none: the project's general documentation, the dependency's, the change's own words, tests, shown users.
- Band words stay out of the first four parts: hurt, how many, how badly, how common, worse than before.

Three examples:

> **What happens:** with a renamed completion file, the first Tab in each new shell does nothing. **Promised: yes**, by practice: zsh binds by the `#compdef` line, oh-my-zsh shipped `_ripgrep` for five years, nobody said stop. **Delivered: no**, the first completion is missing. **So: problem.** **Band, decided separately:** other-material; the second Tab works and renaming cures it.

> **What happens:** one error line at each shell start with `KSH_ARRAYS` on. **Promised: yes**, the change offers the source method and zsh documents the option. **Delivered: yes**, completion loads and works. **So: minor defect.** **Band, decided separately:** none.

> **What happens:** a cipher override through urllib3's `DEFAULT_CIPHERS` is ignored and a weak-key server is refused. **Promised: no**, the value is private to urllib3 and a requests maintainer wrote "Please do not do it this way" in 2017. **Delivered: not asked.** **So: observation (outside supported use).** **Band, decided separately:** none.

The last one says both halves you asked for after ruling 9: something does stop working, and the project never promised it.

## 6. Rejected alternatives, and the strongest objection

**Keep "Owed" and "Lost" and fix only the definitions.** This is cheapest, and the blind assessors did well with those words. Rejected because the one who misread "Lost" was the recorder, who carried the first-round debate into it, and you had to ask what the word meant. A name that needs its definition read out has not done its job in a first line.

**Owed? / Delivered?** A fair second choice if "Promised" feels too narrow. It loses only because "Owed" does not make the agent cite anything.

**Three questions: Wrong? Promised? Delivered?** The first question has two parts: something is at fault, and the project answers for it. A third question would separate them. Rejected because you accepted two questions today, and the kind recorded on a "no" already separates them.

**Keep "yes, yes = problem" with "Unmet?" or "Not delivered?"** Rejected for the double negative.

**Rename "minor defect".** "Blemish" or "flaw" would avoid the size word. Rejected because you chose not to rename it today. The definition by the table is enough.

**The strongest objection.** "Promised" will be read as "written down". The first round leaned to advice. Six of seven reviews moved to a problem, and in at least four the promise was unwritten or indirect: a practice, a tested composition, a handbook sentence nobody had read, a description broader than the documentation. A name that nudges agents toward "show me the sentence" could bring the lean back. And the old names, applied blind, now match you on 41 of 43.

My answer is partial. The five ways a promise is made are part of the definition, and the first line must name one of them. But this is untested. Before adopting, run the blind test again with both sets of names on the same 46 cases, and on the 13 rulings still open in this pass, which neither rule was written from. If "Promised" produces more wrong "no" answers than "Owed", keep "Owed" for question 1 and take "Delivered" for question 2 regardless. Renaming "Lost" is the part the evidence supports most strongly.
