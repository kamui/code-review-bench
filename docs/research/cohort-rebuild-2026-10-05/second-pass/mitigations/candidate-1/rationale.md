# Rationale: alternatives considered and rejected

**Keep the check as built and rely on the brief.** The brief does list every fact that was missed. Rejected because the lesson after ruling 2 was the same kind of text and was missed two hours later, and because the check as built passes every one of the five failures written as they stood (`scratch/current_check_gaps.py`). A check that cannot fail on its own motivating cases is a formality.

**Make the check judge the search's adequacy** (for example, require a code search for every candidate whose trigger is a setting). Rejected: the tool would then encode rules about which searches matter, which change with each round, and the preparation agent would satisfy the letter. Requiring the query, the saved response and the counts makes adequacy visible to the reader without the tool deciding it.

**Require a saved search only for maintainers and public code, not for documentation.** Cheaper, and these two are where ruling 2, S2 and S9 failed. Rejected because review 5 failed in the project's own documentation, and the hit list for a name in a documentation tree is the one search that is both cheap and would have shown the handbook.

**Replace the recorder's recommendation with the blind assessors' answer.** The record says the assessors with a plain rule were closer to the owner on the sixteen reviews. Rejected as the main proposal: S6 had six blind runs wrong and S9 had four right by applying a sentence written from the ruling itself. Kept as the strongest objection, because it may still be right once the record can show it.

**Put the ruling record into `bench/grading/current/adjudications.json` under the existing schema.** It already has `independent_checks` and a receipt. Rejected for now: those records are the authority for eligibility and have a provenance contract checked by `calibration.py`; the learning record wants to hold things the authority does not (a first recommendation that was wrong, a surprise, a circular clause). Issue 59 says the receipts stay the authority and the log is an index. A sidecar JSON beside each ruling file keeps them apart; a later step can link the two by id.

**One log file for the round (`decisions.jsonl`) instead of one record per ruling.** Simpler to summarise. Rejected because the record is written in two halves at two times by possibly two sessions, and a per-ruling file commits cleanly before and after the question without touching other rulings' lines. The summary command can read a glob.

**Derive the clause provenance table by parsing the rule's markdown.** No second file to maintain. Rejected: v3 cites rulings inside parentheses at the sentence level and a numbered rule holds several sentences from different rulings; a parser would either split sentences or lose the grain the trigger needs. A small JSON table is honest about what each sentence rests on and doubles as the record of this audit.

**Prove "before the answer" with a hash shown in the question.** The recommender's answer block hashed, the hash pasted into the question as shown, so the saved ruling file pins it. Cheap and tamper-evident. Rejected for now as more ceremony than the 13 rulings need; commit order gives the same evidence in this repository, where the question and answer are saved as soon as given. Noted in the proposal as what a tool cannot prove.

**Amend ADR-0006 now.** The list in `claim-adjudication.md` constrains no delegation until the policy knows it. Rejected as "now": nothing among the 13 is delegated, the clause-provenance trigger currently fires on nearly everything, and a policy version is the owner's to adopt, not a session's to draft into place. Proposed as the first "wait" item with the one-sentence limit written out.

**Rewrite the three harm-based rules in `finding-threshold.md` to match the two questions.** The contradiction is real. Rejected as a session edit: those are rules the owner set and ruled on; the two questions were accepted in substance but their wording was not. The proposal flags the contradiction in the text and asks for a decision rather than making one.

**Exempt nothing from the promise requirement.** Simpler check. Rejected because a refuted or duplicate claim has no promise to state (the rule's own "facts first" says so), and demanding one teaches the preparation agent to invent a form entry, the opposite of what the check is for.
