# Rulings on the impact boundary's data reason

Recorded at 2026-10-04. Authority: user. Issue: https://github.com/kamui/code-review-bench/issues/53.

[Impact boundary v3](../../../docs/research/reference-calibration-2026-10-04/impact-boundary.v3.md) allowed no exception against its data reason, S2. After ruling GT-y2 serious under it in the [second cohort receipt](cohort-rebuild.v2.md), the user wrote:

> Hm one thing to note from the last decision, "stored data is lost" I'm not sure is absolute. There must be exceptions as not all data is useful, not all data critical, so this could flag a lot of false positives. We probably need some exceptions here.

Each entry below holds the question as shown, the options and the user's answer. The three questions were asked together.

## Which exception

Asked on 2026-10-04, with several choices allowed:

> Which exceptions to the data-loss reason (S2: 'stored data or durable state is lost') should I draft into a new impact boundary, version 4? Today the rule says no exception applies against S2. Six reference bugs rest on S2: GT-s1 (a live file's index entry removed for good), GT-s2 (a file missing from listings after a 'successful' update), GT-v2 (migrations run against the real database), GT-p1 (form fields missing from a 'successful' parse), GT-u4 (a binary log entry loses its whole payload) and GT-y2 (the Django session bug). Exceptions 1 and 2 would each move only GT-y2, back to other-material; exception 3 moves nothing by itself.

Options shown:

- "1. Already lost before (Recommended)": Other-material when the previous version lost the same data in the same situation and the change makes nobody worse off. Checked by running both commits. Matches your examples GT-v3 and GT-u5 and your 'custom hash override' ruling.
- "2. Temporary and restorable": Other-material when the software itself treats the data as short-lived (a cache, a session, a scratch file) and the affected person gets it back by repeating an ordinary action. Closest to 'not all data is critical', but 'restorable' is a judgment call each time.
- "3. S2 yields to the test question": Remove 'no exception applies against S2'. Borderline data loss is decided case by case by asking whether the implementer had to know before release. Reopens GT-u4 and GT-y2 to argument without fixing a line.
- "None yet, just note it": Change nothing now. I record the concern as an open item in the run record so it is settled before the audit sample is drawn.

The user chose "1. Already lost before (Recommended)".

## What happens to GT-y2

Asked on 2026-10-04:

> If an exception you pick covers GT-y2 (the Django session bug you moved to serious a few minutes ago), what happens to GT-y2? Facts: before the PR every signed-in visitor lost their session and its data after a key rotation; after the PR only visitors with a second in-flight request or a failed first request do. They can sign in again; data held only in the session is gone.

Options shown:

- "Follow the exception (Recommended)": GT-y2 returns to other-material under the new boundary, with a third receipt. This keeps the rule and the label consistent.
- "Keep GT-y2 serious": GT-y2 stays serious on the test question even where the exception's words reach it. I then word the exception so it does not cover GT-y2, or record GT-y2 as a ruled case against it.

The user chose "Follow the exception (Recommended)".

## Where the change goes

Asked on 2026-10-04:

> Where should the boundary change go? A new boundary version re-pins all 32 impact decisions and needs one fresh independent inspection of every card by a Sol 6.1 High session (ChatGPT plan usage, one session, as for versions 2 and 3). Nothing is graded again. It must be settled before the audit sample is drawn.

Options shown:

- "Separate PR after #52 (Recommended)": PR #52 merges as it is now, with GT-y2 serious under boundary v3. The boundary change gets its own issue and PR before grading resumes or alongside it.
- "Add it to PR #52": PR #52 grows by the new boundary, the re-pinned decisions and the inspection before you merge it. One PR, but a larger one that needs another review round.

The user chose "Separate PR after #52 (Recommended)".

## A question the user asked while those were open

> In the last example, is the data lost just client side data or was it serverside data too?

Answered from a [probe](../../../docs/research/impact-boundary-2026-10-04/reproductions/django-session-store/) run at the pull request's head: with database sessions the data is on the server and is not erased. The key cycle copies it to a row under a new key and deletes the old row, and the browser never receives the new key or is told to drop its cookie, so the visitor cannot reach it. With signed-cookie sessions, where the data is in the browser, neither failure occurs. The answer came after the three answers above and changed none of them.

## The exception's wording

Shown to the user after the answers, as the text planned for boundary v4 and open to correction before the inspection ran on it:

> **Already lost before the change.** Data that the previous version also lost in the same situation, where the change leaves nobody worse off than before. The change fails to save it; it does not newly destroy it. This is the only exception that applies against S2. No exception applies against S1 or S4.

The user replied "#52 is merged, start on 53" and changed nothing in it.

## Rulings

- Boundary v4 adds one exception against S2: data that the previous version also lost in the same situation, where the change leaves nobody worse off than before.
- GT-y2 is other-material under that exception.
- No exception was approved for temporary or restorable data, and S2 does not yield to the test question.
