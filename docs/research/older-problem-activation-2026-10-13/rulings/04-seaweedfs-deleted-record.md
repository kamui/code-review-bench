# Older problems, admission 4: SeaweedFS PR 10735, a create during a listing has its new record deleted

Prepared 2026-10-13. Not asked yet. The fault is the one the owner ruled outside this change on 2026-10-13 (regrade ruling 4, its second half) and marked a candidate for this credit. That ruling is not reopened. The question is whether it is admitted as an older problem and, if so, with which label. The saved claim is `CL-s-expiry-deleted-record`. The dossier is [`../s-seaweedfs-10735/dossiers/OP-candidate-s-3.md`](../s-seaweedfs-10735/dossiers/OP-candidate-s-3.md); the case every blind session read is [`../s-seaweedfs-10735/cases/OP-candidate-s-3.md`](../s-seaweedfs-10735/cases/OP-candidate-s-3.md). The first answers are in the before-record beside this file, committed before the owner is asked.

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

## Who is hurt, and how badly

Someone whose program stores a file again while a listing is expiring its earlier copy. The store request returns success, and afterwards the file is not found by its path and is in no listing (run, with the order of commands forced). No error is returned and nothing is logged, so the program is not told to store the file again. Storing it once more works (read; not run after this sequence). What the first store wrote is gone from the filer's view. The window is one Redis round trip plus the time to decode a record; nobody measured how often a create lands in it.

## First answers on admission

The three answers were given under the proposed test: "An admitted older problem satisfies the existing Promised? and Delivered? tests without change responsibility. Intent is evidence, neither a mandatory nor a sufficient test." No session was run again after the owner chose the test.

| Party | Answer | Promised | Delivered | Meant | Confidence | Reason |
| --- | --- | --- | --- | --- | --- | --- |
| the asking session (Claude Opus 5.5) | admitted | yes, written | no | not-meant | medium (no-precedent) | A create that returns success has its new record deleted by a listing that had read the expired copy, so the file is then not found by path and no error is returned. |
| Codex gpt-6.1-sol, blind | admitted | yes, written | no | not-shown | medium (no-precedent) | The documentation promises persistence of file metadata, but the run shows a successful create of a replacement with no TTL followed by the listing deleting its record and name at both base and head, satisfying older-7 despite unchanged behaviour. |
| Codex gpt-6-astra, blind | admitted | yes, written | no | not-shown | medium (no-precedent) | The read documentation promises persistence of file metadata, but runs B and F show a successful replacement create with no TTL losing its record to an older listing at both base and head, satisfying older-7 without change responsibility. |

The option the owner chose on 2026-10-13 for the admission test reads, as the asking session reports it: "Promised yes and delivered no, judged on what a reviewer could know at the cut-off. A choice the project stated by then ends the promise; an unstated intention does not count. Later fixes only confirm." That answer is not filed yet. Nothing written by the cut-off mentions a deleted record (read), so no stated choice of the project ends the promise, and the three answers stand on the promise and the runs.

## Recommendation on admission

All three parties answered **admitted** at medium confidence, each short of high only because no earlier admission exists. The recommendation is **admit**: the documentation says the store persists file metadata, and a create that returns success has its record deleted by a listing that had read the expired copy, with no error. The strongest argument against is that the window is narrow and was forced, with nobody shown hitting it. It is kept separate from the lost name of admission 3: the two have separate smallest fixes and different results (run), and a grouping would be a decision of its own.

## The label, asked in the same question

The owner decided on 2026-10-13 that an admitted older problem is labelled serious or other-material, "as the same fault would be if the change had introduced it" (reported by the asking session; not filed, and the sentence is a draft). So the boundary's fifth exception, "Already lost before the change", and its third reading rule, "Discount what bad input already did", are not applied. Both labels earn the same one credit. A fault that would be a minor defect is not admitted.

The impact card is [`../impact/records/OP-candidate-s-3.json`](../impact/records/OP-candidate-s-3.json), written as if the change had introduced the fault. The blind inspector read it as [`../impact/inspection/cards/OP-candidate-s-3.md`](../impact/inspection/cards/OP-candidate-s-3.md), with the labelling sentence over boundary v4 without its anchor tables, and no key to the card's sources. The asking session's label was committed before that session ran. Both first labels are in [`04-seaweedfs-deleted-record.OP-candidate-s-3-label.before.json`](04-seaweedfs-deleted-record.OP-candidate-s-3-label.before.json).

| Party | Label | The rule it names | Confidence | Reason | The strongest point against, in its words |
| --- | --- | --- | --- | --- | --- |
| the asking session (Claude Opus 5.5) | serious | S2 and S4 | medium (no-precedent) | "An operation returns success with data dropped": the store request succeeds and the new record is deleted, with no error and nothing logged. | The window is narrow and storing the file again repairs it. |
| Codex gpt-6.1-sol, blind | serious | S2, S4 | medium (no-precedent) | The expiry listing deletes a newly stored file's record after its store reports success, making the file unavailable by path and listing. | Another store can recreate the file and permanent loss of its volume payload was not run, but the successful store's new metadata record is demonstrably deleted without an error. |

- The two labels agree: serious. Both parties named S2 and S4.
- For serious: S2, "An operation returns success with data dropped or garbled." The store request succeeds and the new record is deleted, with no error and nothing logged.
- For other-material: the window is narrow and was forced, not raced, and storing the file again repairs it.
- In the regrade round, under the boundary as it is written, the asking session and the blind inspector both answered other-material. The fifth exception, which answers S2 under the boundary as it is written, is not applied to an older problem.
- The recommended label is **serious**, at medium confidence. The nearest filed label is GT-p1 on Hono, where a read "returns success with the real fields missing", serious by S2 and S4.

For the label `ruling_record.py route` prints **ASK**: band stays on its existing owner or reconciler route; two blind high-confidence answers and the medium-or-high recommender must agree; an earlier decision of this type is still pending; this decision type has not earned level 1 from earlier closed rounds.

## Route

For the admission `ruling_record.py route` prints **ASK**: older-admission stays on its existing owner or reconciler route; two blind high-confidence answers and the medium-or-high recommender must agree; an earlier decision of this type is still pending; this decision type has not earned level 1 from earlier closed rounds.

An admission and a label stay with the owner whatever the parties answer. Nothing in this question could be settled by delegation.

## The question as it would be asked

"SeaweedFS 10735: in the older expiry branch a create that lands between a listing's read and its delete reports success and has its new record deleted. You ruled it outside this change and a candidate. Is it admitted as an older problem, and with which label?"

Options:

1. Admit, serious, separate from the lost name (Recommended)
2. Admit, other-material: the window is narrow and storing the file again repairs it
3. Not admitted: the window is forced and nobody is shown hitting it

What each choice does:

- Admit, with either label: one entry in the older-problem catalog of `s-seaweedfs-10735`. Each of the task's 41 admitted reviews is assessed for it at the regrade. A review that raised it earns one credit whatever the label; the label decides only which of the panel's two counts the entry is in.
- Not admitted: no entry and no credit. The one linked comment stays a true comment outside this change.

## Answer

Not asked. The after-record is written when the owner has answered.
