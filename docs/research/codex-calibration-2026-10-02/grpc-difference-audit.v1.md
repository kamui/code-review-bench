# Grpc-go calibration difference audit

All 31 paired claims preserve the same substantive recovery, decomposition, remedy, priority/action and unresolved judgments. No evidence-supported scoring regression was found in either condition. One materiality assessment uses a different basis for a refuted claim. The conditions are therefore not identical in every assessment field. Neither output is automatically preferred.

This is post-grading research. Keep the report and its analysis outside every grader input. No mapping, verdict, reference, ruling, runner or packet was changed.

## Pinned inputs and coverage

- Control mapping v1 SHA-256: `cdf4a1522e075e3aee514c7cdd7d303a5ac20ebc4ddbf69d50469fd765c2486b`.
- Enriched mapping v2 SHA-256: `845dbc0bb47d74f9b64e13f4fd91c11472570ca453567719a20d63fc5e348736`.
- `grpc-comparison.v1.json` SHA-256: `4fdd074a3dd679da1d5cb3924a795f48fcd909bfe44186b32ee64ec8578cb8ff`.

Both mappings are under `bench/runs/2026-09-30-selected-prs-review-only/scoring/u-grpc-go-6919`. This audit covers all nine original reviews, 30 items and 31 paired claims. Every pair's original wording, changed notes and changed evidence list was inspected, alongside its unchanged quote, assignment, canonical identity, duplicate group, remedy and candidate. Supporting inspection covers all 82 control and 85 enriched citation entries, including repeated citations, and all eight enriched packets. Packet bytes and every source reference in their preparation provenance match their pinned hashes.

Independent comparison confirms 31 changed notes, 31 changed evidence lists, one changed assessment and no other changed claim fields. Item recovery/remedy/priority projections and all review-level projections match. There are no unmatched quote groups or decomposition changes.

| Attempt | Paired claims checked | Difference assessment |
| --- | --- | --- |
| att-001 | item-0/c1; item-1/c2; item-2/c3 | Neutral judgments; enriched states the conversion-probe limit more explicitly |
| att-002 | item-0/c1; item-1/c2; item-2/c3; item-3/c4 | Neutral judgments; enriched adds the unmarshal-to-Internal source anchor and specifies direct logger probe limits |
| att-003 | item-0/c1; item-1/c2; item-2/c3; item-3/c4 | Neutral; both preserve conditional helper wording without inventing request loss |
| att-014 | item-0/c1; item-1/c2; item-2/c3; item-3/c4 | Neutral; control explains the fluent API shorthand, enriched retains the same actual round-trip interpretation |
| att-015 | item-0/c1 | Neutral nonblocking diagnostic recovery and remedy |
| att-020 | item-0/c1; item-1/c2; item-2/c3; item-3/c4 | Neutral; enriched names the direct logger probe versus production caller distinction |
| att-027 | item-0/c1; item-1/c2; item-2/c3; item-3/c4 | Neutral; both retain static/probe limits and supported compatibility scope |
| att-028 | item-0/c1 | Neutral nonblocking diagnostic recovery and remedy |
| att-041 | item-0/c1; item-1/c2; item-2/c3,c4; item-3/c5; item-4/c6 | Five neutral judgment pairs; c4 has the materiality-basis difference described below |

## Evidence and substantive meaning

All six codec pairs preserve GT-u2 and sufficient V1 adaptation advice while accepting native V2. The new import narrows both assertions, the registered legacy fixture lacks `ProtoReflect`, and the saved probe fails both codec operations at head. Generated callers and the error propagation source establish RPC consequences. Enriched att-002/c1 adds `rpc_util.go:799-805`, which correctly maps unmarshal failure to Internal. This improves citation completeness, not the recovered judgment.

All six status pairs preserve GT-u3 and sufficient `MessageV1Of` remedies. `WithDetails` still accepts registered legacy input on non-OK statuses; `Details` exposes the V2 wrapper rather than the original concrete type. Saved round-trip results and source support both accounts. The enriched notes do not import any extra API-signature allegation or infer eligibility from upstream merge status. Control's att-014 explanation of its shorthand expression is more explicit, but enriched's round-trip interpretation remains correct.

All six reply-logging pairs preserve GT-u4 independently of the default codec failure. The supported custom codec sends the reply; unary server logging receives the original V1 object; the narrowed logger assertion loses its data and length. Both outputs prescribe sufficient legacy recognition/adaptation. Enriched more consistently says that the saved probe constructs a logger entry directly and production reachability comes from source. That is clearer evidence-limit wording, not a new integration result or demonstrated improvement in grading accuracy.

All nine diagnostic pairs preserve eligible, nonblocking GT-u1 and sufficient correction advice. Successful receive leaves the earlier error nil, failed `CheckValid` discards its own cause, and the warning loses the intended explanation. Source anchors correctly cover the receive guard, changed binding, operator warning and unchanged retry path. Both conditions keep correction optional as a review action and do not require a crash. att-001's title supplies enough remedy guidance in both conditions.

RLS overflow remains inconsequential. The maxAge cap, staleAge normalization and timeout consumer are correctly cited. Enriched explicitly distinguishes conversion-library probes from a full parser run. Positive LRS overflow remains advisory and receives no panic credit; both accounts distinguish a centuries-long accepted interval from a newly material reporting loss. The source/conversion evidence supports that limit without supplying production frequency.

The exact negative overflowing LRS input remains GT-u5 with sufficient range-rejection advice. Both notes preserve same-input attribution, the unsupported endpoint-granularity exclusion, and the fact that older small-negative intervals already reached the ticker panic. Enriched separates item-4's validation-failure diagnostic from the preceding item's validation-success panic path. Neither condition claims a live remote stream or incidence estimate.

The hard mixed-claim boundaries remain intact. R010, att-041/item-2, retains reply recovery in c3 and explicit request-loss refutation in c4. R021, att-014/item-2, and R050, att-003/item-2, each keep one reply-recovery claim and do not acquire an invented request allegation. R022, att-001/item-1, stays advisory; R047, att-041/item-3, recovers the exact negative panic.

The inspected enriched citations correctly establish all request byte-slice paths: `server.go:1347,1372-1374`; `rpc_util.go:728-733`; `stream.go:894,908-911,1747-1749,1766`; and the logger's unchanged byte branch at `method_logger.go:247-248`. Control cites the same mechanism with somewhat different ranges. Direct V1 helper probes do not override these production callers. The remaining shared source/probe citations agree with the earlier complete control audit and the pinned base/head records.

## The one assessment difference

att-041/item-2/c4 is refuted in both outputs. Support remains contradicted, attribution introduced, reachability unreachable, remedy n/a and candidate null. Its materiality changes from control `below-threshold` to enriched `material`.

Control explains that the inspected callers establish no actual material request regression. Enriched explicitly says that losing request payloads would be material if it occurred, while the production trigger is refuted. These assess different objects: the realized supported regression versus the hypothetical consequence of the false allegation. The source supports the common factual conclusion, not an actual request loss.

The rubric separates support, attribution, reachability and materiality, but does not prescribe whether refuted claims' materiality must describe the hypothetical alleged consequence or the surviving actual behavior. The enriched flag is not evidence that the request-loss allegation became eligible, and the control flag does not establish that request payload loss would be harmless. Preserve both raw assessments and this explanation. Do not claim complete field equivalence, silently normalize either flag, or treat the disagreement as a new eligibility ruling. It does not change scoring or require a new user ruling for the already refuted claim.

## Remedies, projections and usage

All 28 recovery claims retain sufficient remedies; the advisory, inconsequential and refuted claims retain n/a. All 30 priority-error projections match, all native actions remain absent, and the preserved P-number rule has no inversion. Absolute severity is not independently established by that projection. All nine reviews remain completed, incorrect-patch verdicts with recovery, no approval-on-buggy and no false-clean result. There are no unresolved candidates or duplicates.

Successful-session known usage is $1.723182 control and $1.799256 enriched. Enriched costs $0.076074 more, about 4.41%, in this pair. This is not a savings result. The comparator also records the failed control's known $0.476636, bringing known control-attempt usage to $2.199818; its unknown upper usage remains unknown. The comparison's receipt-duration totals are not controller wall time, and one pair does not establish a general efficiency or quality effect.

## Limits and disposition

Evidence references become easier to trace to bounded packets in enriched output, while some control notes retain useful explicit qualifications. Neither condition shows a recovery, remedy, decomposition or supporting-evidence regression. The single assessment convention difference prevents a claim of total judgment-field identity. The known `headless Claude Code` label remains a separate mapping provenance defect for correction after live calibration calls, not a substantive grader judgment.

Source and saved probes were inspected read-only, including pinned mirror excerpts for the added unmarshal error and request-byte anchors. Enriched execution evidence shows source inspections and searches, not new codec/network/log-sink/remote-stream tests. No source probe, provider call, grading dispatch, clone reconstruction, new eligibility decision or frozen-artifact mutation occurred in this audit. It does not establish full calibration acceptance, remaining-target quality or final publication readiness.
