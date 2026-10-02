# Paired calibration comparison

`compare.py` reads two rubric-v2 mappings for the same cohort, target, register, rubric, review identities, and item identities. It prints JSON to stdout and writes nothing. Exit 0 means the inventory was produced, including when judgments differ. Exit 2 means an input failed validation or the pair does not share the required identities.

```sh
python docs/research/codex-calibration-2026-10-02/compare.py \
  --control <control-mapping.json> --enriched <enriched-mapping.json> \
  --control-dispatch <control-evidence.json> \
  --enriched-dispatch <enriched-evidence.json> > <new-comparison.json>
```

Repeat a dispatch flag to include failed or replaced attempts as well as the accepted attempt. Identify their disposition in the independent completion audit. Each dispatch input can be a direct `work/dispatch.json` or an archived `evidence.json`; the latter verifies the archive SHA-256 and reads its `work/dispatch.json` without extraction. `--root` defaults to this repository and resolves archive references. `--run` overrides the default source cohort at `bench/runs/<run_id>`.

## Identity and differences

Reviews pair by `attempt_id`, and items pair by `item_id`. The CLI validates the mapping schema and verifies the source attempt's run and target and its normalized item identities. It records hashes of both mappings, source reviews, attempt records, dispatch receipts, and any dispatch archives. This is comparison support, not a substitute for the controller's completed grading validation or evidence audit.

Local claim IDs and blind tokens do not pair claims. Claims pair by exact quote within an item. Exact semantic matches cancel first with their multiplicity preserved. A single remaining claim on each side with the same quote produces field differences. Multiple remaining claims sharing a quote remain an ambiguous unmatched group, with every claim retained. Changed quotes, splits, and merges remain unmatched. No substring, fuzzy match, or equal assignment substitutes for equal quotes.

The decomposition inventory compares quote multisets. All substantive claim fields remain available: assignment, canonical claim link, assessment, candidate, fix sufficiency, notes, and evidence. Claim notes carry the reason in the current schema. Canonical claim links identify shared adjudication cases and remain compared fields; only local claim `id` labels are ignored. Duplicate-group names become sorted lists of stable item/quote members, preserving repetitions, so randomized group names do not hide a membership change. Evidence strings and lists remain exact fields; different citations, limitations, or ordering remain visible.

Each item also compares its recorded assignment, remedy assessment, duplicate membership, priority error, and notes. Quote ordering and changes to the first claim are reported separately because the existing scoring projection assigns the item's priority error only to that first claim. Each review retains both sides' assignment counts, recovered reference identities, full unresolved claims, and review-level differences. `native_input` contains the common normalized source item, including its original priority, action, and proposed fix. `review_arm` identifies the original review projection. Native inputs are observations; the comparator does not infer correctness or recalculate grading rules.

`evidence_context_differences` compares mapping claim snapshots, including packet/source hashes and withheld evidence. Mapping versions, scorer metadata, and timestamps remain in the pinned input files. They do not count as judgment differences.

Usage summarizes all explicitly supplied dispatches and retains each complete recorded usage object, model, effort, client version, elapsed dispatch time, exit status, and audit violations. Missing costs and request counts remain unknown. Repeated session IDs within or across conditions are refused. Token category totals remain unavailable because Codex dispatch receipts currently record priced cost and billed requests, not those category totals. File sizes never become token estimates. The historical `bench/tools/grading_profile.py` profiles Claude transcripts; its transcript parser and pricing model are not applied to these Codex receipts.

The report has no automatic quality judgment or rollout recommendation. The researcher must investigate each changed or unmatched claim against the original item, source evidence, and saved ruling. Unchanged labels do not prove a sound judgment.

## Difficult boundaries to inspect

The identities below come from matching `triage.v1.json` original items against the cohort's saved normalized reviews. The outcome constraints come from the saved [approved batch](../selected-pr-triage-arena-2026-09-30/approved-batch.v1.json) and its referenced rulings, not new decisions.

| Original token | Stable review/item | Required inspection |
| --- | --- | --- |
| R010 | `att-041/item-2` | Preserve eligible unary reply recovery under `GT-u4` and separately assess the explicit production request-loss assertion as refuted. A single combined claim or a lost component requires investigation. |
| R003 | `att-007/item-0` | Separate the incorrect `a01a`/`a1aa` example from the supported general non-total-order mechanism. The canonical `GT-w1` decision does not settle this item's recovery. Check quote, attribution, notes, evidence, and remedy independently. |
| R022 | `att-001/item-1` | Positive overflow is advisory. Do not transfer the negative-overflow panic into this item. |
| R047 | `att-041/item-3` | Negative overflow is eligible under `GT-u5`. Compare the same newly admitted input at base and head; an older panic for other negative inputs does not settle attribution. Remedy completeness remains a separate judgment. |
| R021 | `att-014/item-2` | Do not invent a request-loss assertion from conditional descriptions of narrowed internal type assertions. Preserve the eligible reply mechanism. |
| R050 | `att-003/item-2` | Apply the same quote-level request-loss boundary as R021. |

For the diagnostic's nine equivalent links, inspect every `CL-u-lrs-diagnostic` occurrence, its `GT-u1` assignment, native priority/action, item projection, and fix sufficiency. The [saved user ruling](../../../bench/claims/rulings/CL-u-lrs-diagnostic.v2.md) grants eligibility and explicitly makes the finding nonblocking. Eligibility does not require a must-fix action, and the implementor may decline repair. An advisory label or a forced blocking recommendation needs independent investigation. Do not fabricate a crash, production incident, or failed remediation to justify the finding.

## Verification

```sh
python docs/research/codex-calibration-2026-10-02/compare-test.py
```

The focused synthetic cases retain a combined claim's split into eligible reply recovery and refuted request loss despite randomized IDs; expose changed fix, reason, and evidence fields for a shared quote; distinguish group renaming from membership changes; preserve unresolved claims; refuse mismatched review identities; and reject duplicate usage sessions while leaving missing cost unknown.

Fresh calibration results require an independent comparison after the four sessions complete. This tool does not dispatch models, change mappings, modify claims, or authorize publication.

## Codex transcript usage

`measure.py` independently reads saved Codex transcripts through `bench/tools/codex_usage.py`. It does not change the mapping comparator or use the Claude transcript parser. Pass accepted and failed or replaced receipts explicitly so retry usage stays separate:

```sh
python docs/research/codex-calibration-2026-10-02/measure.py \
  --accepted <accepted-evidence.json> --accepted <another-accepted-evidence.json> \
  --failed <failed-or-replaced-evidence.json> > <new-usage-report.json>
python docs/research/codex-calibration-2026-10-02/measure-test.py
```

Each flag can repeat. Positional receipt paths are also accepted. A positional receipt with a failed dispatch or no verdict becomes `failed`; a successful positional receipt remains `unclassified` until the completion audit identifies it as accepted. Failed or replaced receipts may have successfully completed a session, so label those with `--failed`. The meter refuses `--accepted` when the receipt has a failed exit, audit violation, or no verdict. It does not otherwise adjudicate attempt validity.

Inputs can be archived `evidence.json` or direct `work/dispatch.json`. Archives resolve against `--root`, which defaults to this repository. The meter checks the archive hash and the receipt's pinned hashes for `work/dispatch.json` and every selected Codex rollout member. It reads those members without extracting them into the repository. Direct inputs capture dispatch and transcript hashes at measurement time; they do not prove an earlier archived hash. Both forms verify the transcript root session, descendants, subagent count, and observed model against the dispatch receipt. Duplicate sessions or response identities across attempt inputs are refused.

Each attempt reports fresh input, cache writes, cache reads, output including reasoning, and reasoning separately. Reasoning tokens are a subset of output and are not charged twice. `turns` counts deduplicated billed response records. `tool_turns` counts those records associated with at least one tool call; `tool_calls` counts their associated calls; `text_only_turns` counts records without tools. `recorded_tool_calls` also includes calls without a following usage record, and `unattributed_tool_calls` retains that difference. Missing billed records never become estimated tokens or requests.

The meter prices the saved records using the dispatch receipt's dated rate, including the dispatcher's per-request long-context rule where applicable. It requires the observed cost to match the recorded subtotal rounded to six decimal places and checks the recorded request count. It pins the parser's current file hash in the report so later parser changes remain visible. There is no report-byte adjustment or production-shaped estimate.

`raw_total` includes every supplied attempt. `by_disposition` separates accepted, failed or replaced, and unclassified attempts. `known_observed_cost_usd_sum` sums saved, priced usage even when an attempt's final charge upper bound is unknown. `unknown_charge_upper_attempts` preserves that uncertainty, and `charge_upper_usd` remains null whenever any supplied attempt has a null recorded upper bound. A timeout's observed subtotal is not a final billing total. Empty saved usage remains an attempt without an observed cost, rather than an inferred zero-charge proof.

Dispatch duration comes from receipt timestamps. Controller preparation, waiting, archiving, and total wall time need a separate controller measurement. Summed dispatch seconds are reported as a sum, not as elapsed parallel wall time. All costs are list-price equivalents and do not measure an invoice or account quota.

The focused meter tests cover duplicate response records, token categories, unattributed final tool calls, known accepted cost alongside unknown timeout cost, outer archive and selected-member tampering, session and model mismatch, duplicate attempt inputs, and absent usage that cannot be inferred from tools.
