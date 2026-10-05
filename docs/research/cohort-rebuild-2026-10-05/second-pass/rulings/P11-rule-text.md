# Second pass, decision P11: the wording of the rule (partly answered)

Put to the user on 2026-10-05.

Decision P9 fixed the names of the two questions and left "the wording of the rules under each question" to be "put to the user separately". Decision P10 wrote the rule for an undocumented use into `docs/finding-threshold.md` under the heading "Rules the user set" before the user had read it. Two independent audits found sentences there and in version 3 of the rule that said more than their rulings (`docs/research/cohort-rebuild-2026-10-05/second-pass/mitigations/SYNTHESIS.md`).

What the user is asked to read:

1. `docs/research/cohort-rebuild-2026-10-05/second-pass/terms/two-questions.v4.md`, the working rule. Each clause has an identifier and names its rulings. Two clauses are marked *shown* because the user chose an option that stated them (P2b, P3b). Every other sentence is the recording session's wording.
2. `docs/finding-threshold.md`, "Rules from the second pass, 2026-10-05": the two questions and the cases ruled for a use the project's documentation does not describe, with what has not been ruled listed as such.

What the user is asked to decide:

1. For each text: accept it, change it, or strike sentences.
2. The three rules of the first round that still turn on harm (older faults: "someone is harmed in supported use"; partly kept promises: "a real loss"; unusual input: "nothing shows users producing it"): reword them to the two questions, or keep them and say which governs when they part.
3. The list in `docs/claim-adjudication.md` of cases that stay with the user: accept it as how questions are prepared. Adopting it as a limit on delegation is a separate, later decision.

Until answered, version 4 is the frozen working text for the 13 rulings open on 2026-10-05, and questions flag any clause they rest on.

## Answers so far

The three were asked together after a formatted message (what the two audits found; what was built for each of the user's three questions; what it still cannot do; the cost before the 13; and the corrected passage for a use the documentation does not describe, quoted in full).

1. "The corrected rule text (the passage above, and version 4 of the two questions): how do you want to handle it?" Options: "Use it as the working text (Recommended)", "I accept the passage above", "I want to change something". The user answered: "I need more context around the corrected rule text, these are corrections documented for rules we adjudicated? How will this be used? Also, I cannot open that two-questions.md document for some resaon, it says failed to read workspace file." Open; the text is to be shown in full with what it is for.
2. "Three first-round rules still turn on harm ('someone is harmed', 'a real loss', 'nothing shows users producing it'), while your recent rulings turn on Promised and Delivered. What should happen to them?" Options: "Reword them, show me (Recommended)", "Keep them, flag conflicts", "Decide after the 13". The user chose "Reword them, show me (Recommended)". Each reworded sentence is to be put to the user as an option before it is written.
3. "The list of cases that stay with you (computed from each record): accept it as how questions are prepared?" Options: "Yes, use it (Recommended)", "Yes, and adopt it as a delegation limit now", "Not yet". The user chose "Yes, use it (Recommended)". The list is accepted as how questions are prepared. It is not adopted as a limit on delegation.

## The reading, rule by rule

The file would not open for the user, so the text was posted in full and the user took it one rule at a time.

**Rule 2.** The user: "Clarify 'An announcement takes away only what it names.' This is confusing phrasing. How can we make it clearer?"; "'Behaviour the change announces by name is not promised otherwise' not disagreeing, but again phrasing for me is confusing to read/parse"; "P2c 'takes away nothing' again odd phrasing, can we make it clearer?" The session proposed "A PR ends an old promise only by saying so, and only for what it says", with P2a "stated outright", P2b "a general aim is not enough" and P2c "it has to be said in the PR, before the merge", each with an example from the rulings, and three options. The user chose option 1 (use the wording and give the other rules an example each) and asked: "although does P2c mean it can only be said in a PR, and not documentation, or a ticket?" The session answered no and proposed "stated where a reviewer could read it before the merge: the change's description and code comments, the documentation it adds or changes, a ticket it links, or the review discussion", marking the ticket and the review discussion as its own reading. Open: the user has not chosen among the four options given for P2c.

**The old terms.** The user: "BTW you still used the old terms like 'owed' in this text, when we decided on promised and delivered no?" P3a ("the software owes nothing") was reworded to "No behaviour is promised for input the specification or the declared type rules out", and "data or a protection is lost" in D2 to "data is destroyed or a protection stops working".

**Rule 1.** The user: "My reading is that when a repository's owners either explicitly do not support a piece of functionality, users that use or abuse that functionality do not create a promise for that behavior. Is that right?" and then "strike the word 'either'". The sentence without that word is now P1a, in the user's words. It settles that explicit non-support alone is enough; in second-pass ruling 2 three grounds had held together. The session's additions (the owner may be a dependency; the statement predates the merge) and P1b's wording are marked as its own. Open: an owner who said yes and later no.

Asked whether to add "the owner's latest statement before the merge governs", the user chose "Use the wording above, and leave 'yes, then no' as not ruled" and added: "Hmm.. I want to omit this and leave it for the agent to judge. I might want to add something like, if the owners change stance, but the last one sounds definitive/authoritative, then respect that answer. If you are not sure as a reader, then play it the safest with respect to ruling it as a promise or not. One example, and not the only one might be there different repository owners have different answers, or the author indicates they are not sure with language like maybe, probably, or i think language."

The session asked which side is "safest": send it to the user (its recommendation), lean to not promised, or lean to promised. The user answered: "2. Lean to 'not promised'. If the documentation does not specify, and a owner is not sure if they support that, and the code doesn't seem to strongly indicate it's a supported public behavior, then it's not promised."

That is clause P1c, with the user's sentence quoted in it. The session's reading of its reach, told to the user: it covers an owner who is unsure or owners who differ; where no owner said anything, a practice with shown users still counts under P8a, as ruled for the renamed completion file and for key logging. Open: P1b's wording and P2c.

**P2c, answered.** Asked whether a linked ticket and the review discussion count as places a change can be announced, the user answered: "2. Yes, if the linked ticket is accessible to the reviewer. although this is not as strong evidence as an announcement in the PR or documentation/code." P2c and the definition of "announced" now say so. The review discussion was not addressed and stays the session's reading.

**P1b, asked again.** The user: "this is still confusing, can you give me the reworded rule entirely again? i think what reads as confusing is 'name', when a project documents a name? Name for what? That reads as confusing".
