# Owner questions: three rulings chosen on 2026-11-04

Round `owner-rulings-2026-11-04`, issue #30. In the round 2 reconciliation of evaluator audit v2 you chose three changes in principle. This file asks you to approve each one's exact text, one piece at a time. Each question gives the facts first, then one question. How each fact is known: **run** means executed here with the output saved; **read** means seen in the code, a document or a saved record.

Ask them in this order. Question 0 has to come first, because it decides whether the records for questions 1 and 2 can be finished.

## 0. Blind answers for the records

- The record tool (`bench-tools ruling check`) needs two blind answers for a decision about a candidate, from a model family other than mine. I am Claude (read, `rust/bench-tools/src/ruling/record.rs`).
- This round was told to use Claude models only. Blind Claude answers count only if you allow them for this round in a saved receipt, as you did for round `rule-v4-post-filing-candidates` (`bench/grading/rulings/same-family-blind.v1.md`) (read).
- Until then, the three candidate records for questions 1 and 2 are saved as drafts in `drafts/`. Apart from the blind answers, they pass the check (run, with placeholder answers that were not kept).
- Round 2's 35 blind Claude sessions cost $7.38 at list price on the subscription (read). This round needs 4 to 6 sessions, about $1 to $1.50.

**Question:** Allow two blind Claude answers (Opus 5.5 and Sonnet 5.5, high effort) for this round?

- Allow Claude blind answers (Recommended)
- Use Codex assessors instead
- Skip blind answers and decide without records

## 1. GT-p4: a parameter after `boundary` (Hono 5067)

Case: [`01-gt-p4/cases/NC-2d6d8bcc45c7.md`](01-gt-p4/cases/NC-2d6d8bcc45c7.md). Draft text: [`01-gt-p4/wording.md`](01-gt-p4/wording.md). What it changes: [`01-gt-p4/affected.md`](01-gt-p4/affected.md).

### 1a. Close the candidate as GT-p4

- With `multipart/form-data; boundary=X; charset=utf-8`, `validator('form')` hands its callback `{}`, while `parseBody()` reads the form. That holds at the base and the head, on Node, Bun and Deno (run).
- The same line causes it as GT-p4's two header forms: the pattern on `validator.ts` line 25 (read).
- The patch from ruling 14 fixes it. So does the combined patch that fixed both of GT-p4's forms in October. On Node, Bun and Deno, the combined patch fixes all three forms (run, this round).
- Ruling 14's cause sentence fits: "the validator's hand-written header pattern rejects valid header forms that MIME allows" (read).
- On 2026-11-04 you wrote "maybe we should expand GT-p4 to before and after?" and chose "Widen it (Recommended)" (read).
- Later, on 2026-11-01, Hono fixed both orders with one change to line 25 (read).

**Question:** Close candidate NC-2d6d8bcc45c7 as part of GT-p4?

- Part of GT-p4 (Recommended)
- A separate problem
- Not a problem

### 1b. GT-p4's new wording

- The draft adds "or after" to the title, obligation and trigger. It adds one example header, `multipart/form-data; boundary=X; charset=utf-8`, and one sentence to the mechanism: the boundary value cannot hold a `;` or a space, so a parameter before or after it fails.
- The grouping reason now covers three header forms and cites this ruling. The impact card gets the same changes. The label stays serious.
- Every one of the 20 Hono batches becomes stale, because a grade's inputs include the whole answer-key entry. One saved result changes: `2026-10-10-last-push-claude-thermo-opus`, `att-017`, `c11`, from unresolved to caught (read).

**Question:** Accept GT-p4's wording as drafted in `01-gt-p4/wording.md`?

- Accept as drafted (Recommended)
- Change some first
- Show me the full text

## 2. GT-i7: an adapter whose `cert_verify` sets `conn.ca_certs` (requests 6667)

Case: [`02-gt-i7/cases/conn-ca-certs.md`](02-gt-i7/cases/conn-ca-certs.md). The three tests: [`02-gt-i7/grouping-tests.md`](02-gt-i7/grouping-tests.md). Draft text: [`02-gt-i7/wording.md`](02-gt-i7/wording.md). What it changes: [`02-gt-i7/affected.md`](02-gt-i7/affected.md).

### 2a. Close the two candidates as GT-i7

- An adapter whose `cert_verify` sets `conn.ca_certs` to a private CA makes every other session trust that CA after one request. That happens at the head and not at the base (run).
- Both routes leak through the same added line, which hands every `verify=True` pool the shared context (read). GT-i7's route also needs a second changed line, where `cert_verify` stopped overwriting `ca_certs`; this route does not (read). So test 1, "the same lines", holds for the line that causes the leak, but not for every line each route needs.
- Giving each pool its own context stops both routes. Each adapter still reaches its own server (run, this round). A fix aimed only at GT-i7's route leaves this one leaking (run). Ruling 14's lesson says that last fact does not decide grouping (read).
- One cause sentence fits both: "CA material an adapter attaches to its own pools is loaded into the context every verify=True pool shares" (read).
- One difference: at the base GT-i7's route did nothing, while this route worked and stayed private (run).
- Five public adapters that override `cert_verify` were read. All five clear `conn.ca_certs`, and none sets a private CA, so how common this route is is not known (read).
- In the saved grades, one review gets GT-i7 credit for this route and another review is left unresolved on it (read).
- On 2026-11-04 you chose "Opus wrong + widen GT-i7 (Recommended)" (read).

**Question:** Close candidates NC-2a3425e24057 and NC-e9470b7251c2 as part of GT-i7?

- Part of GT-i7 (Recommended)
- A separate problem
- Not a problem

### 2b. GT-i7's new wording

- The draft names the second route in the title, obligation and trigger: CA files a `cert_verify` override sets on the pool for `verify=True`. It quotes `cert_verify`'s docstring, "only exposed for use when subclassing".
- The mechanism adds what the route does at the base and the head, with the run (121 to 122 CAs in the shared context). The trigger says `conn.ca_cert_dir` was not run.
- The grouping reason gives the three tests as above. The impact card gets the same changes. The label stays serious.
- Every one of the 20 requests batches becomes stale. One saved result changes: `2026-10-24-thermo-nuclear-review-claude-fable`, `att-017`, `c3`, from unresolved to caught (read).

**Question:** Accept GT-i7's wording as drafted in `02-gt-i7/wording.md`?

- Accept as drafted (Recommended)
- Change some first
- Show me the full text

## 3. Rule version 6: claims split from one thought

The drafts, the two passes and the checks are in [`03-rule-v6/pieces.md`](03-rule-v6/pieces.md). What it changes: [`03-rule-v6/affected.md`](03-rule-v6/affected.md). The rubric files are written only after all three pieces are approved.

### 3a. Piece 1: judge a cause and its result together (section 1)

- The Django comment's sentence 3 names a cause: the cursor runs on a pooled connection to the very database being dropped. Sentence 4 begins "In both cases" and gives the result: the drop fails. The claim list split them into c4 and c5 (read).
- You read c4 and c5 as one claim (round 2, question 38) (read).
- Section 1 already says to keep an explanation with the result it explains. The draft adds "even when the result comes in the next sentence".
- The draft also adds a rule for a claim list that splits a pair anyway. Judge the two claims as one claim, for every question. The pair counts when the comment says both in one sentence, or the second sentence points back with words like "so", "this means" or "in both cases".
- The example is the Django pair. Judged together, the drop fails because the cursor is connected to the database being dropped. That is true and says what goes wrong. Blaming the pool is a wrong detail.
- This matches your calls on round 2 units #1 and #2. It departs from round 1 units #6 and #7 on the same review, which stay as recorded (read).

**Question:** Accept piece 1 as drafted?

- Accept as drafted (Recommended)
- Change the wording first
- Leave rule version 5 as it is

### 3b. Piece 2: "can't borrow" applies to separate thoughts (section 3)

- The rule stays for two claims that are separate thoughts, each with its own cause. It now says a cause and its result are one thought, and points to section 1.
- The Django example is removed. The new example is made up: a cache key that leaves out the user's id, and a cleanup job that deletes live entries because of a time-zone mix-up. No saved ruling gives a clean real case (read).

**Question:** Accept piece 2 as drafted?

- Accept as drafted (Recommended)
- Use a real example instead
- Change the wording first

### 3c. Piece 3: a joined pair counts as one claim for credit (section 3)

- Section 3 says "Judge the claim by its own words." The draft adds: "Two claims judged together under section 1 count as one claim here."
- Section 6 (pre-existing problems) uses section 3's test, so it needs no change (read).

**Question:** Accept piece 3 as drafted?

- Accept as drafted (Recommended)
- Change the wording first

## 4. When to regrade

- Filing rule version 6 makes all 258 batches stale. One full regrade costs about $266 at list price, from the current receipts (read).
- Filing rulings 1 and 2 alone makes 40 batches stale: about $23 for Hono and $30 for requests (read).
- A full regrade under version 6 grades those 40 batches too.
- You said "regrade the affected Hono batches after the reconciliation" (read). Round 2 of the audit failed, so the audit plan goes back to you for a new plan (read, `docs/evaluator-audit.md`).

**Question:** File all three, then run one full regrade under rule version 6?

- File all three, one full regrade (Recommended)
- Regrade the 40 Hono and requests batches now, then again under version 6
- Decide later, with the new audit plan

## A note, not a question

`2026-10-02-claude-builtin-selected/v-django-17914`, `att-014`, `c9` is one claim in the current grade. It says the cursor comes from the pool of "the DB being created or dropped" and that the drop fails, and it blames the pool. Its PE-v1 result is not raised. Rule version 6 does not reach it, because the claim is not split. Your reading of unit #1, a true point with a wrong detail, would likely raise it. The regrade grader will see it under the same rules; if it stays open, it comes back to you as its own question.
