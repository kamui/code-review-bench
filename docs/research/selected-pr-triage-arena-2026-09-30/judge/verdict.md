# Cross-judge verdict: selected-PR triage arena

**Recommended base: B.** Graft A's evidence execution, its R010 claim split, and its V2 and V4 eligibility reasoning into it. After the corrections below, the human queue holds three questions. Everything else goes to routine ADR-0002 batch approval or ordinary grading.

## Coverage

Both audits contain exactly 68 rows, and each row maps to a single grounding token (R001 to R068). The union of each audit's canonical tokens is also complete. The token groupings are identical, except that A also splits R010's request-loss clause into its own refuted claim.

## Scores (0 to 4)

| Criterion | A | B |
| --- | ---: | ---: |
| Complete intake | 4 | 3 |
| Evidence fidelity | 3 | 3 |
| Escalation discipline | 2 | 3 |
| Policy accuracy | 4 | 4 |
| Review usability | 3 | 4 |
| **Total** | **16** | **17** |

A has the stronger evidence execution: reruns on isolated copies, an exact comparator rerun, and `stream.go` copied from the clone. However, A sends the wrong question to the user. Its only escalation, Django custom session-hash rotation, is settled by pinned documentation and upstream design evidence. A also marks the two real boundaries (U4 and W1) as clear.

B's queue matches the evidence better on the central task. B also over-escalates V2 and closes R047 too early.

## Disagreements and rulings

| Issue (tokens) | A | B | Recommendation | Deciding evidence |
| --- | --- | --- | --- | --- |
| Y2 custom session hash (R032, R042) | needs-human, lean eligible | advisory | **advisory, clear** | `settings.txt:2270-2274` (unchanged from base) ties secret-key session use to the *default* `get_session_auth_hash()`. The diff documents `get_session_auth_fallback_hash()` as a password-field HMAC. A maintainer chose a separate method to protect the documented API. Custom-hash users are flushed on both revisions. |
| U4 LRS `<nil>` diagnostic (9 items) | advisory, clear | needs-human, lean eligible | **human queue, recommend advisory** | Rejection, the field name and backoff retry are unchanged (`loadreport.go:122-124`). Diagnostics have no calibration, and the ruling sets a precedent. |
| W1 comparator collision (7 items) | eligible, clear | needs-human, lean advisory | **human queue, recommend advisory** | Base already used `sortValueNode` (naturalCompare) for input-object fields. The trigger requires digit runs beyond 2^53. |
| U6 negative overflow (R047) | eligible, clear | scope-excluded, clear | **human queue, recommend scope-excluded** | Probes show that -1s and 0 already panic at base. The attribution rule for enlarged trigger sets is uncalibrated. |
| V2 empty pool dict (6 items) | eligible, clear | needs-human, lean advisory | **eligible, clear** | `head base.py:214-215` normalizes `True` to `{}` ("default options"), while the guard at `:205` disables pooling for `{}`. The docs promise "a dict". |
| V4 psycopg2 docs (R026) | eligible | advisory | **eligible (low), clear** | The docs say "ignored with psycopg2", but `base.py:290-292` raises. The same change introduces both. |
| R010 request clause | separate claim, refuted | flag only, EQ1 open | **A's split** | Every stream binlog message carries bytes (`stream.go:908-935, 1671-1749`). The unary request is also bytes (`server.go:1372`), and only the reply is an object (`server.go:1470`). |
| R022 positive overflow | advisory | inconsequential | advisory (ordinary grading) | Base warned and retried, while head stalls silently. Neither outcome is a false finding. |
| R061 RLS overflow | advisory | inconsequential | inconsequential (ordinary grading) | maxAge is clamped to 5 minutes regardless, and a 292-year timeout behaves the same as the requested value. |
| H2 role hook | separate, advisory | fold into H1 | separate advisory, linked as related | Its trigger and consequence differ from the QuestDB timezone regression. |
| H4 timing | unsupported | refuted | refuted | Comparisons are whole-hash constant-time, and the objection was retracted with reasons. |

Both candidates agree on U1, U2, U3, V1, V3, W2, Y1 and H1 as eligible and on H3 as refuted, and the saved probes support each.

## Recommended human queue

1. **HQ1, U4 (grpc-go #6919, 9 items).** An introduced wrong-variable bug makes the LRS rejection warning read `invalid load_reporting_interval: <nil>`. Behavior, the field name and retry are unchanged.
   - **Choice:** does losing only the cause of an existing diagnostic meet the correction threshold?
   - **Recommendation:** advisory. The field is still named, protection is unchanged, and no failed diagnosis is shown.
   - **Alternative:** eligible, treating any content regression in an existing diagnostic as a defect.
   - **Why it needs you:** this is a reusable precedent for a frequent review class, and no calibration covers diagnostics.
2. **HQ2, W1 (graphql-js #3457, 7 items).** Argument canonicalization via naturalCompare rejects a valid reordered query whose argument names differ only beyond 2^53.
   - **Choice:** is a deterministic false validation error with a spec-valid but practically unused trigger material? The same flaw already affected object fields at base.
   - **Recommendation:** advisory, with moderate confidence.
   - **Alternative:** eligible, treating spec validity as sufficient reachability for a validator.
   - R003's wrong example stays an ordinary grading matter under either outcome.
3. **HQ3, R047 (grpc-go #6919, 1 item, lowest priority).** Head admits -10000000000s, and it reaches the NewTicker panic. On base, -1s and 0 already reach it.
   - **Choice:** does admitting more inputs into an identical, already-reachable crash count as worsened or as pre-existing?
   - **Recommendation:** scope-excluded.
   - **Alternative:** eligible, because base rejected this exact input.

## Not human questions

- **Routine batch approval.** Clear proposals: U1, U2, U3, V1, V2, V3, V4, W2 and Y1 eligible. Y2 and R022 advisory, R061 inconsequential, and R010's request clause refuted. Research hypotheses: H1 eligible, H2 advisory, H3 and H4 refuted. The closest clear calls to glance at are V2, V4 and Y2.
- **Evidence.** Nothing is blocking. B's EQ1 is now closed. The optional upstream-disposition checks (grpc-go and Django mirrors) and a live PostgreSQL run for V1 can inform the queue but cannot decide it.
- **Ordinary grading.** R003's example, R010's reply recovery, R021/R050 wording, R022/R061 labels, W2 timing figures and native priority labels.

## Limits

I read both audits' canonical issues, queues, research sections, reports and rationales in full and machine-checked all 136 rows. I sampled per-item prose only on the disputed tokens. I re-executed nothing, relying instead on hash-listed saved probes, A's rerun receipts, the pinned source and the archived upstream records. Every outcome is a proposal; ADR-0002 authority is unchanged.
