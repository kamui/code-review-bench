# Investigator A: notes for the assembler

Units: #01, #02 (`j-trpc-5017`), #09, #17, #18 (`m-grpc-go-7390`), #32 (`i-requests-6667`). All six repeat units the owner already decided. Four questions.

| Question | Units | Recommendation | Confidence |
| --- | --- | --- | --- |
| `13-trpc-5017-issue-5020-test-name-repeat` | #01, #02 | first-correct, both | medium (no-precedent) |
| `15-grpc-go-7390-old-shape-race-free-repeat` | #09 | first-correct | medium (no-precedent) |
| `16-grpc-go-7390-user-supplied-connect-hooks-repeat` | #17, #18 | first-correct, both | medium (no-precedent) |
| `14-requests-6667-failed-load-later-requests-repeat` | #32 | first-correct | medium (no-precedent) |

Round 3 asked #01 and #02 as one question (its question 20) and #17 and #18 as one question (its question 05). Both pairs stay one question here. #17 and #18 are claims in two items of one review, and both turn on one fact: no program outside grpc-go can supply the `minConnectTimeout` function or a backoff strategy.

All six drafts pass `bench-tools ruling check`. I tested them in before-records built in scratch from round 3's shape, and wrote nothing under `rulings/`.

## Saved owner decisions that bear on a unit

- **#01, #02**: round 3's units #02 and #04 (question 20), `first-correct`, ground "agreed with recs" (`docs/research/audit-v2-r3-reconciliation-2026-11-09/rulings/20-trpc-5017-issue-5020-test-name.u02.after.json`, `.u04.after.json`).
- **#09**: decided `first-correct` three times. Round 1 #22 (rule version 4), round 2 #21 (version 5) and round 3 #17 (version 6). The after-records are `04-grpc-go-7390-guard-and-lock-hand-off.u22`, `15-grpc-go-7390-old-shape-race-free.u21` and `25-grpc-go-7390-old-shape-race-free.u17`. Nothing has changed since: the same first-grade verdicts (`b774f441…`, last changed 2026-11-07), the same rule text, no saved ruling on grpc-go, and the same probed fact on the same commits.
- **#17, #18**: round 3's units #25 and #26 (question 05), `first-correct`, no ground (`05-grpc-go-7390-user-supplied-connect-hooks.u25.after.json`, `.u26.after.json`).
- **#32**: two calls on two different first grades. Round 2 #54 was `first-error`. It was made on `assessment-2`, which answered question 1 "not shown", and its ground was that a run settles the claim's truth. Round 3 #34 was `first-correct`. It was made on `assessment-6`, the grade sampled again now, which answers question 1 "yes" and holds credit open. That grade was made against GT-i4 as the owner widened it by `candidate-rulings-2026-11-08 1`, filed 47 minutes before the grade was dispatched. So the call changed because the first grade changed, and with it the question: truth in round 2, credit in round 3. Rule version 6's "Leave a claim unresolved only when the rules say to" list also supports the round-3 reading. Neither rule version 6 nor a new fact reversed round 2's reasoning. The current grade meets that reasoning, because it answers "yes". Round 3's reasoning is the one that applies to this round's grade.

None of these after-records is in the rulings index, so they are cited in the reasons and cases, not in `nearest`. Since round 3 closed, the index gained only `soba-scope-candidates-2026-11-10` 1 and 2 and `candidate-rulings-2026-11-10` 1 and 2 (soba and Django). Receipt `cohort-rebuild.v21.md` filed only round 3's first-grader errors, and none of them is on these targets. Receipt v22 is Django only.

**Conflicts.** Round 3's records for #02, #04, #25, #26 and #34 named a conflict between rule clauses, at low confidence. I set `conflict` to null and confidence to medium, as round 3 did for its repeats. Round 2's #13 had a conflict, and round 3's repeat #27 kept none, because the owner had decided that conflict on the same claim under the same text. If the assembler prefers to carry the round-3 conflict strings forward, #01, #02, #17, #18 and #32 become low (`conflict`, `no-precedent`). No outcome changes.

## What is the same and how it was checked

- **read** The first grade of each unit is byte for byte the one round 3 reconciled. `grades.json` points at the same assessments: tRPC `assessment-11`, both grpc-go `assessment-5`, requests `assessment-6`. Each `verdicts.json` was last changed before round 3 opened. The receipts have round 3's SHA-256, except tRPC's. The account redaction (`de617bb42`) changed one line of that receipt, a dependency-cache manifest hash: `98cb514f…` became `c7f8e494…`. Round 3's case 20 still cites the old prefix.
- **read** This round's `rule/rubric.first.md` and round 3's have the same SHA-256 (`4a680252…`).
- **read** Since round 3 closed (`d08e330b5`), nothing changed under `bench/targets/` for the three targets, in the five reviews, or in GT-i4. The only entry of `references.json` that changed is Django's GT-v16.
- **read** I unpacked the second grader's evidence archives for the four batches, rounds 3 and 4, into scratch. In each pair, `rubric.md` (`a0dfb723…`), `packet.md` and `references.json` are byte for byte the same, and `claims.md` differs only in review tokens. So each second grade is a re-roll on the same inputs. Its results match round 3's for all six units. The wording changed, and the substance did not, except for the points below.
- **read** New in the second grader's words:
  - **#01, #02**: it now says outright that the stated facts hold whatever #5020 contains. This is the reading the owner already weighed.
  - **#09**: it now calls the double-update ordering "unlikely" and "already race-free" "a little strong". It ran no test of that ordering.
  - **#17**: it now agrees that the hook is reachable only through an unexported option. It still folds `c2` into `c1`. Its evidence leaves out `WithConnectParams`'s constant closure.
  - **#32**: it gives a shorter ground and drops its discussion of "later".

## Facts settled and how

- No new probe. Round 3's grpc-go probes cover #09, #17 and #18, on the same commits: `probes/grpc-go-7390-double-update/` and `probes/grpc-go-7390-connect-hooks/`. For #32, round 2's `probes/requests-6667-failed-load-cert-chain/` and the N1 probe of `candidate-rulings-2026-11-08` cover the facts. Its open point is a reading question, which no run settles. Each case cites the probes by path.
- **#01, #02**: issue trpc/trpc#5020 is saved under `docs/research/last-push-rulings-2026-10-11/j-trpc-5017/upstream/`, and the owner had it in round 3. I made no network call.

## What the owner would need if a unit went the other way

- **#01, #02** first-error: a reason why "If the claim is true no matter what that fact is, answer yes" now outweighs keeping the comment's stated result with its explanation. The owner weighed exactly that on 2026-11-10. The same first grader answered "yes" and "suggestion" for four like comments in batch `2026-09-28-sonnet-5-5-rebench`, so a first-error call would make those consistent and these the outliers.
- **#09** first-error: a reading that "already race-free" is an overstated word and not the premise of "unnecessary". The owner has rejected that reading three times.
- **#17** first-error: a ruling that a fragment inside the result sentence of an equivalent item takes the saved claim's answer, although it adds a situation (a slow user hook) that cannot occur. **#18** first-error: a reading of the "caller that does not exist yet" rule that covers a hook no user can install. The second grader's outside-this-change would then follow.
- **#32** first-error: a reading that "later requests" covers the requests that overlap the failed load. GT-i4's mechanism says requests one at a time are as at the merge-base. The review already has GT-i4 through `c1` and `c2`, so this changes only the claim's label.

## Could one cause reach other batches?

I searched every current first grade of each target, quotes and notes, with a throwaway script in scratch.

- **`j-trpc-5017`**: six claims about the 5020 file name. The two here are "cannot check". The four in `2026-09-28-sonnet-5-5-rebench` (att-007 c11, att-020 c6, att-025 c9, att-033 c9) are "yes, suggestion". Round 3 found the same split. These are not disagreement units, but the same grader answers the same point two ways across batches.
- **`m-grpc-go-7390`**: only `att-025#c3` mentions "race-free", and only `att-004` c2 and c7 refute a user hook or custom strategy. No other batch rests a refutation on these grounds.
- **`i-requests-6667`**: only `att-020#c3` concerns a failed certificate load. Two other claims (`2026-10-10-last-push-claude-ce-opus` att-021 c4, `2026-10-10-last-push-codex-ce-luna` att-023 c1) say "later requests" about the leaked certificate, GT-i4's first form, and both have GT-i4 credit. That reading is not in question.

No wider cause found.

## Tool notes

- `bench-tools ruling check` accepted all six drafts. #17 and #18 name `first-19`, which the index marks `earlier`, so they name `no-precedent`. #32 names `candidate-rulings-2026-11-08 1`, which is `current`. It still names `no-precedent`, as round 3 did, because that ruling decided grouping and not credit.
- The evidence archives were unpacked by hand into scratch and read only. No dependency archive was unpacked, no probe ran, no network call was made, and no paid model call was made.
- Scratch `~/.cache/recon-r4-A/` is deleted.
