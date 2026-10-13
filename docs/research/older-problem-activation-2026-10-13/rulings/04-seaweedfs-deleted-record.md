# Older problems, admission 4: SeaweedFS PR 10735, a create during a listing has its new record deleted

Prepared 2026-10-13. Not asked yet. The fault is the one the owner ruled outside this change on 2026-10-13 (regrade ruling 4, its second half) and marked a candidate for this credit. That ruling is not reopened. The new question is whether it is admitted as an older problem under the proposed test, which the owner has not yet adopted. The saved claim is `CL-s-expiry-deleted-record`. The dossier is [`../s-seaweedfs-10735/dossiers/OP-candidate-s-3.md`](../s-seaweedfs-10735/dossiers/OP-candidate-s-3.md); the case every blind session read is [`../s-seaweedfs-10735/cases/OP-candidate-s-3.md`](../s-seaweedfs-10735/cases/OP-candidate-s-3.md). The first answers are in the before-record beside this file, committed before the owner is asked.

## The facts

All established in the regrade round and cited from [its dossier](../../regrade-rulings-2026-10-13/s-seaweedfs-10735/dossiers/M2b.md). Nothing was run again.

- The expiry branch of the listing reads a file's record, decides it has expired, and then deletes the record by path (read, unchanged lines 214 to 219).
- A create of the same path that lands between the listing's `GET` and its `DEL` reports success and its new record is deleted (run at the base, the first commit and the head, order forced through a client hook, no error injected). The file is then not found by path and is in no listing. No error is returned and nothing is logged.
- It is identical at all three commits (run).
- A file stored again before its TTL runs out reaches the branch without any forcing (run).
- The listing's `DEL` removes only the record in Redis. What the upload wrote to a volume server is not touched by it (read; not run). Storing the file once more works (read; not run after this sequence).
- Against the lost name of admission 3 (run): separate smallest fixes and different results.
- The saved claim has 1 selected review linked to it (read, `claims.json`).

## What the change promised, and whether it delivers it

- Nothing written by the cut-off mentions a deleted record (read): not the description, the two commit messages, the code comments or the review threads.
- The change neither touches these lines nor claims anything about them.

## What the code is for

- The expiry branch exists to remove a file whose time-to-live has passed when a listing meets it (read).
- The wiki: "The Filer Store persists all file metadata and directory information." (read, as of 2026-08-11).

## First answers

| Party | Answer | Promised | Delivered | Meant | Confidence | Reason |
| --- | --- | --- | --- | --- | --- | --- |
| the asking session (Claude Opus 5.5) | admitted | yes, written | no | not-meant | medium (no-precedent) | A create that returns success has its new record deleted by a listing that had read the expired copy, so the file is then not found by path and no error is returned. |
| Codex gpt-6.1-sol, blind | admitted | yes, written | no | not-shown | medium (no-precedent) | The documentation promises persistence of file metadata, but the run shows a successful create of a replacement with no TTL followed by the listing deleting its record and name at both base and head, satisfying older-7 despite unchanged behaviour. |
| Codex gpt-6-astra, blind | admitted | yes, written | no | not-shown | medium (no-precedent) | The read documentation promises persistence of file metadata, but runs B and F show a successful replacement create with no TTL losing its record to an older listing at both base and head, satisfying older-7 without change responsibility. |

## Recommendation

All three parties answered **admitted** at medium confidence, each short of high only because no earlier admission exists. The recommendation is **admit**: the documentation says the store persists file metadata, and a create that returns success has its record deleted by a listing that had read the expired copy, with no error. The strongest argument against is that the window is narrow and was forced, with nobody shown hitting it. It is kept separate from the lost name of admission 3: the two have separate smallest fixes and different results (run), and a grouping would be a decision of its own.

## The label, as its own part

- The proposed text gives an older problem no label: an entry "has no impact band". The owner asks for the label's reasoning whenever a problem is recommended, so it is given here. It changes nothing in the proposed contract.
- Other-material. The boundary's words for other-material are "A real bug that earns credit when a review raises it, but it does not have to be raised." That is what the owner said of an older problem on 2026-10-13: "I would not fault a code review for not pointing it out. If a reviewer could point it out, that's great". Its words for serious are "If it ships without them knowing, the review has failed", which an older problem cannot meet, because a review that stays quiet about it loses nothing.
- The words of S2 fit: "An operation returns success with data dropped". The fifth exception answers it in its own words: the previous version deleted the same record in the same situation, and the change leaves nobody worse off. In the regrade round the asking session and the blind inspector both answered other-material.
- The boundary's second reading rule says "Label the change as submitted". An older problem is not the change's doing, so the serious reasons, which describe what a change does to people, apply to it only by stretching. Whether older problems get a label at all is an open question for the owner, listed in rule-text.md.

## Route

`ruling_record.py route` prints **ASK**: older-admission stays on its existing owner or reconciler route; two blind high-confidence answers and the medium-or-high recommender must agree; an earlier decision of this type is still pending; this decision type has not earned level 1 from earlier closed rounds.

An admission stays with the owner whatever the parties answer. Nothing in this question could be settled by delegation.

## The question as it would be asked

"SeaweedFS 10735: in the older expiry branch a create that lands between a listing's read and its delete reports success and has its new record deleted. You ruled it outside this change and a candidate. Is it admitted as an older problem?"

Options:

1. Admit it as an older problem, separate from the lost name (Recommended)
2. Do not admit it: the window is forced and nobody is shown hitting it
3. Hold it

What each choice does:

- Admit: one entry in the older-problem catalog of `s-seaweedfs-10735`. Each of the task's 41 admitted reviews is assessed for it at the regrade.
- Do not admit: no entry and no credit. The one linked comment stays a true comment outside this change.
- Hold: a pending candidate keeps the final count of older problems caught unavailable for a selection that holds this task.

## Answer

Not asked. The after-record is written when the owner has answered.
