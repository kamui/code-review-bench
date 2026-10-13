# Arena synthesis for issue 135

The synthesized artifact is [the implementation plan](README.md). Astra 6 high and Opus 5.5 high produced separate plans from the same ticket and repository brief. Sol 6.1 high judged both after they completed. All requested seats completed; there were no substitutions or dropouts.

## Phases

- [x] Frame
- [x] Fan out
- [x] Cross-judge
- [x] Pick
- [x] Graft
- [x] Verify

## Rubric and scores

Each criterion is scored from 0 to 4. The columns are parent scores followed by Sol's scores.

| Criterion | Astra, parent | Opus, parent | Astra, Sol | Opus, Sol |
| --- | ---: | ---: | ---: | ---: |
| All five policy decisions, alternatives and existing rulings | 4 | 3 | 4 | 3 |
| Repository grounding and complete, coherent change propagation | 3 | 3 | 3 | 3 |
| Counting, controls, unsupported claims and unavailable values | 4 | 2 | 4 | 2 |
| Versioned migration, authority and complete reconciliation | 3 | 3 | 3 | 3 |
| Ordered deliverables and behavioral acceptance checks | 4 | 3 | 4 | 3 |
| Total | 18 | 14 | 18 | 14 |

The parent initially preferred Opus's separate collection boundary. Reading both rationales and Sol's verdict resolved the overall choice in favor of Astra's more complete plan, while retaining that boundary as a graft. Sol received the parent's initial boundary preference near the end of its review and still recommended Astra. This was a collaborative design judgment, not a blinded benchmark.

## Base and grafts

Use [Astra's plan](candidates/astra-plan.md) as the base for policy, migration and acceptance requirements. It precisely defines the counting unit, preserves observations while final measures are unavailable, handles mixed claims and unsafe remedies, and covers fingerprints, rollback, equivalent comments and historical evidence. Its shared-family representation would require ordinary-only exclusions throughout recovery and reporting.

Graft these parts from [Opus's plan](candidates/opus-plan.md):

1. Store `older_problems` beside ordinary `families`. Existing ordinary family consumers retain their meaning. Add explicit canonical joins, receipts and fingerprints so the separate list does not become a second disconnected grading system.
2. Reconcile ordinary rulings affected by narrowed Before 2 before activation. Keep that mandatory check separate from the optional search for further historical bonus candidates. Coordinate activation to avoid unnecessary repeated invalidation.
3. Show per-problem detail and catalog limitations. Explicitly handle a correct older-fault observation accompanied by a false attribution to the PR.

## Rejections and corrections

- Reject a mandatory intent test. The saved Kubernetes decision supports purpose-based evidence in that case, not a universal requirement to prove author intent.
- Reject a pending-candidate exemption from current control and recommendation gates in the first release. A proposed classification does not establish its final scope.
- Reject counting all outside-change labels, bonus recall rates and automatic historical conversion.
- Distinguish catalog size, observed catches and final assessed counts. A missing or incomplete catalog does not erase an observed approved catch or establish that no older faults exist.
- Explicitly model pending bonus eligibility separately from a resolved outside-change claim. The present v2 novel-candidate invariant does not permit that shape.
- Do not prescribe an export version already in use or require another issue to merge as a policy constraint. Integrate the actual v2 compatibility prerequisite and advance the version from that baseline.
- Remove rough historical keyword counts and assumptions that all candidate behavior is identical before and after. Neither is needed to implement the first release.

## Evidence and verification

Both initial prompts reported the October 13 ruling directory missing from the task checkout and default branch. The parent later located committed records on `t3code/issue-30-regrade-rulings`. [The provenance manifest](ruling-provenance.json) records their source commit, paths and hashes. Sol read all five source files and verified those hashes. The synthesis preserves their final answers and their outstanding adoption/filing conditions.

The parent verified the active v2 policy, explicit exporter rejection, legacy web claim schema, family-based control restriction, recovery behavior, scoring paths, referenced implementation files and available package scripts. It checked the final local Markdown links, source hashes, candidate-copy integrity and whitespace. No application tests, grading or benchmarks ran: the only repository changes are this planning package.

Raw candidate plans and rationales remain unchanged under `candidates/`. Their stale missing-source notices and rejected recommendations are retained as arena evidence; the implementation plan is the current recommendation.

Opus ran through the [cc:rescue skill](/home/jack/.codex/plugins/cache/sendbird/cc/1.5.0/skills/rescue/SKILL.md). Automatic approval review initially rejected the external API/write invocation. The user explicitly approved the disclosed data transfer and restricted output location, and the retried task completed. No permission remains pending.
