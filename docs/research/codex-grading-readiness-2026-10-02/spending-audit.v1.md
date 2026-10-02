# Issue 9 spending audit

Audit snapshot: 2026-10-02. Repository: `/home/jack/.t3/worktrees/code-review-bench/t3code-8d1674bf`. Read-only audit; no model calls, repository edits, credential reads, or changes to frozen evidence. Outputs are under `/tmp`.

The deduplicated known applicable usage is **$27.55463182** in saved-rate equivalents. This is a lower bound, not a reconciled total. Under the earlier $300 cap, the arithmetic balance would be **$272.44536818**, with no proven spendable balance because unresolved usage has no bound. Unknown usage is not zero.

The latest user instructions are "Forget budget with codex" and "dont worry about budget". These supersede the earlier budget gate for current Codex-only work. This report preserves historical accounting and does not make budget reconciliation a blocker for the current Codex task. The Codex-only restriction remains in force.

The earlier saved instructions reaffirmed a $300 aggregate cap for readiness and calibration and restricted new calls to Codex. Historical Claude calls still count when applicable. The audit treats the selected-cohort reviewer controllers and adjudication forwarders as applicable prior activity. It distinguishes those separately dispatched sessions from ordinary interactive implementation work, whose inclusion and cost remain unresolved.

| Applicable saved activity | Amount in USD | Evidence |
| --- | ---: | --- |
| 45 completed selected reviews | 10.163995 | `bench/runs/2026-09-30-selected-prs-review-only/attempts/att-001` through `att-045`, each `attempt.json` and `usage-requests.jsonl` |
| Failed isolation probe | 0.003163 | `bench/runs/2026-09-30-selected-prs-review-only/probes/att-001/attempt.json` |
| Successful replacement isolation probe | 0.001843 | `bench/runs/2026-09-30-selected-prs-review-only/probes/att-002/attempt.json` |
| Opus claim candidate | 4.146218 | `/home/jack/.claude/projects/-home-jack--t3-worktrees-code-review-bench-t3code-7fc50325/4996979a-d410-4191-9441-ed2645e2921e.jsonl` |
| Opus claim cross-judge | 2.103716 | `/home/jack/.claude/projects/-home-jack--t3-worktrees-code-review-bench-t3code-7fc50325/0403c881-d880-4f1f-857d-0b3fe7b96d16.jsonl` |
| codex claim candidate | 0.80350160 | root thread `01a0f3a4-c1f6-7ad3-a29f-e2b8a1a200ca`, exact paths and hashes in `/tmp/issue9-applicable-codex-usage.json` |
| opus claim forwarder | 0.19633760 | root thread `01a0f3a5-2836-7c50-a292-53bb2340fc2c`, exact paths and hashes in `/tmp/issue9-applicable-codex-usage.json` |
| crossjudge claim forwarder | 0.14643280 | root thread `01a0f3bc-b151-76f1-9559-5f19a7320af2`, exact paths and hashes in `/tmp/issue9-applicable-codex-usage.json` |
| sol review controller | 2.02880840 | root thread `01a0f1f9-0a1b-78c0-9c07-f0acfef2addc`, exact paths and hashes in `/tmp/issue9-applicable-codex-usage.json` |
| luna review controller | 0.26024042 | root thread `01a0f1f9-643c-74e1-9430-aac7f56022c9`, exact paths and hashes in `/tmp/issue9-applicable-codex-usage.json` |
| astra review controller | 7.70037600 | root thread `01a0f1f9-b79e-72b0-bff8-7201fed02ebd`, exact paths and hashes in `/tmp/issue9-applicable-codex-usage.json` |

The initial $10.169001 already contains both probes. `charges.jsonl` repeats the probes as ledger rows; it adds no third charge. Recovered copies under `/tmp/issue-9-recovered` and the original WSL worktree are copies of the same evidence and add no charge. The three reviewer model subtotals are $0.069074 Luna, $1.889203 Sol, and $8.205718 Astra.

I re-metered all 47 original transcript archives. They contain 359 unique billed response IDs, 346 for reviews and 13 for probes, with no duplicate response IDs across the archives. Per-attempt rounded costs reproduce the saved total. Archive roots are `artifacts/transcripts/reviews/2026-09-30-selected-prs-review-only` and `artifacts/transcripts/2026-09-30-selected-prs-review-only`. `/tmp/issue9-original-review-meter.json` records the result.

The two adjudication transcripts match `docs/research/selected-pr-triage-arena-2026-09-30/model-provenance.v1.json` byte for byte:

- Session `4996979a-d410-4191-9441-ed2645e2921e`, SHA-256 `cc9146d024d137195f287986b595d6c6b30d3f0d61542884e4438e2cb251b687`.
- Session `0403c881-d880-4f1f-857d-0b3fe7b96d16`, SHA-256 `3301402b4ddf0689cdf9e21097dc95105c9e7ee6795238d4a5bcff510a844a17`.

Tracked job receipts are `/home/jack/.codex/plugins/data/cc-sendbird/state/eb96b25bddaa/jobs/task-muogfrv1-8fddbb.json` and `task-muohgl9h-nm64y9.json`. They identify the same sessions and add no separate charge. `bench/tools/transcript_usage.py` deduplicates the 144 billed assistant lines into 73 requests. All cache-write tiers are known. The measured token totals are 146 fresh input, 311720 one-hour cache writes, 7301551 cache reads, and 114764 output tokens including thinking.

Historical pricing uses `bench/rates.json` for Opus: $4 fresh input, $8 one-hour writes, $0.2 cache reads and $20 output per million tokens. The original Codex rates come from `bench/runs/2026-09-30-selected-prs-review-only/rates.json`. The script receives cache-read multiplier 0.05 for Sol and Opus, 0.1 for Luna and Astra. All priced Codex requests are below the saved 272000-token tier threshold. These are historical-rate calculations, not new pricing claims or invoice verification. Codex OAuth subscription usage consumes quota and its saved amounts are list-price equivalents. Saved Opus rates declare API dollars, but an invoice or account usage export has not established the actual debit.

The complete adjudication measurement is `/tmp/issue9-adjudication-usage.json`. The candidate and controller measurements are `/tmp/issue9-applicable-codex-usage.json`. These preserve source paths and hashes, categories, request counts and unpriced descendant records. The 519 priced Codex candidate, forwarder and controller request IDs are unique across their source transcripts.

Unresolved records with actual local meters:

- `/home/jack/.codex/sessions/2026/09/30/rollout-2026-09-30T14-48-02-01a0f3a5-285d-7d50-ae56-231dc1425b58.jsonl` has 2 requests under `codex-auto-review`. The token meter exists, but neither a provider billing identity nor a saved price/zero-charge settlement exists. It is a descendant of the opus claim forwarder.
- `/home/jack/.codex/sessions/2026/09/30/rollout-2026-09-30T15-13-44-01a0f3bc-b17a-7370-9610-088e4e726743.jsonl` has 2 requests under `codex-auto-review`. The token meter exists, but neither a provider billing identity nor a saved price/zero-charge settlement exists. It is a descendant of the crossjudge claim forwarder.
- `/home/jack/.codex/sessions/2026/09/30/rollout-2026-09-30T07-00-25-01a0f1f9-0a41-75b3-be25-4873388db7c1.jsonl` has 13 requests under `codex-auto-review`. The token meter exists, but neither a provider billing identity nor a saved price/zero-charge settlement exists. It is a descendant of the sol review controller.
- `/home/jack/.codex/sessions/2026/09/30/rollout-2026-09-30T07-00-48-01a0f1f9-6464-75e3-8f5e-e76a22240160.jsonl` has 22 requests under `codex-auto-review`. The token meter exists, but neither a provider billing identity nor a saved price/zero-charge settlement exists. It is a descendant of the luna review controller.
- `/home/jack/.codex/sessions/2026/09/30/rollout-2026-09-30T07-01-09-01a0f1f9-b7c4-7aa1-8878-64549ac8f3e8.jsonl` has 7 requests under `codex-auto-review`. The token meter exists, but neither a provider billing identity nor a saved price/zero-charge settlement exists. It is a descendant of the astra review controller.

- `/home/jack/.codex/sessions/2026/09/30/rollout-2026-09-30T04-45-33-01a0f17d-90b2-7d30-ba8f-9082b20e95b6.jsonl` is the original interactive root. It contains multiple task phases, and parsing encountered a malformed JSON line. Do not invent zero usage or a complete cost from the surviving records. Export the undamaged session/request usage and classify root turns by applicable selected-cohort work before pricing them. Early `/root/sol_candidate`, `/root/opus_runner`, and `/root/crossjudge_runner` descendants may be task-discovery work. They require task-level scope reconciliation before inclusion.
- `/home/jack/.codex/sessions/2026/10/02/rollout-2026-10-02T00-11-37-01a0facf-7ce5-7411-af85-4e6c7670df49.jsonl` is the current interactive recovery/readiness root. Its descendant transcripts have actual local token meters. These include `/root/issue9_review`, `/root/cache_readiness_report`, `/root/cache_code_audit`, `/root/cache_dependency_inventory`, `/root/cache_final_review`, `/root/codex_grading_adapter`, and `/root/readiness_spend_audit`. The root remains active. A final usage snapshot, dated rates for each observed model, descendant response-ID deduplication, and a saved decision about interactive implementation usage are required. The current audit is itself one such interactive descendant.

I found no saved paid selected calibration or rollout archive under `bench/regrading` in this checkout. The recovery and cache-readiness reports explicitly state those calls have not run. Their preparation, fake API, offline smoke and provisioning tests created no paid reviewer or grader sessions. Those tests are zero model-call events; that does not settle the interactive work that produced them. The Mac handoff records that its prior session made no paid calls. No provider-level export proves that no applicable calls exist outside the recovered evidence.

Excluded historical activity: `bench/regrading/rubric-v2-2026-09-30` and `docs/research/grading-evidence-2026-10-01/baseline.v1.json` cover targets i through t and explicitly exclude the selected 45-review cohort. Their $126.120264 accepted plus $10.568445 failed/replaced cost is not included. Imported historical global benchmark runs also stay excluded without a concrete relationship to this readiness/calibration cap.

Historical reconciliation actions, optional for the current Codex work after the latest instruction:

1. Preserve a final usage-only export of the original interactive root and the current recovery/readiness root with all descendants. Include response IDs, thread lineage, timestamp, model, token categories, auth/billing mode, and terminal status. Do not export credentials or complete prompts. Restore the malformed original-root segment from an undamaged local copy or provider/session export. Deduplicate responses across copies and descendants.
2. Obtain the account usage or billing/quota export for the two known Opus session IDs, the applicable Codex thread IDs, and the five listed `codex-auto-review` descendants. It must establish actual billing identity or an explicit zero-charge policy for the guardian records. Keep list-price equivalents and actual invoice dollars in separate columns.
3. Save each inclusion/exclusion and unknown settlement in a versioned applicable-spend ledger. Confirm whether ordinary interactive implementation and orchestration work consumes this same cap. Until then, the conservative audit leaves it unresolved. Subtract all applicable known amounts and a justified bounded reserve for any remaining unknowns from $300.
4. If the earlier aggregate cap is reinstated, use the reconciled ledger to reserve each of four fresh paired calibration sessions, any rollout sessions, and explicit failed-attempt/retry capacity. A queue or workspace change does not reset an aggregate cap. The OAuth CLI lacks a verified per-call dollar hard stop. This limitation is recorded for any future bounded execution; it does not block the current Codex work after the user waived the budget concern.

Scope sources are `/tmp/issue-9-wsl-handoff.md`, `docs/research/selected-cache-rebuild-2026-10-02/user-constraints.v1.md`, both selected-cohort research READMEs, `bench/runs/2026-09-30-selected-prs-review-only/authorization.json`, and the selected recovery/verification receipts. The original frozen authorization permits reviews only. It is historical evidence and does not capture the latest Codex-only instructions. Save those latest instructions with current execution provenance without changing the frozen authorization.
