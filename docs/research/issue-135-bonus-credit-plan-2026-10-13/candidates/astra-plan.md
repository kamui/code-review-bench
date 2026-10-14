# Proposed implementation plan for issue #135

This proposal is grounded in HEAD `b25a00a924b87c07d8c1ebead1732eaef06d30d1` and the supplied complete ticket. It changes current grading and reporting, without rerunning reviews or changing frozen evidence. Recommendations below require adoption; they are not new rulings.

The ticket reports five bonus candidates and preserves Kubernetes 141463 as an audited-clean control. Its proposed Before 2 sentences are not yet in force. The referenced `docs/research/regrade-rulings-2026-10-13/` records are unavailable at this HEAD. Locating and pinning those authoritative records is a prerequisite to any conversion. Ticket ruling numbers are locators, not fabricated adjudication IDs.

## Decisions to adopt

### Credit and its display

Add a separate scorecard dimension called “Older problems caught.” Its primary count is distinct approved bonus families caught per admitted review, summed across the selected reviews. Repeated assertions or manifestations of the same family in one review count once. Independent repetitions can each earn credit. Show the number of distinct bonus families represented and the number of admitted reviews earning credit alongside it, so a total does not imply five different discoveries when five repetitions catch one problem.

Show raw observed counts and assessment coverage in a dedicated `MethodologyViews` tab and in `tools/scorecard.ts`, including JSON output. Do not create a bonus recall denominator, miss count, percentage, blended score, sort preference, or recommendation rule. Existing recall, serious-miss measures, equal-PR weighting, and pairwise recommendations exclude bonus families. An admitted, completely assessed review with no qualifying catch contributes zero bonus credit, not a missed problem. Rejected alternative: adding bonus catches to recall's numerator would permit inflated recall and make an optional discovery affect ordinary detection.

Counts are provisional observations while selected reviews are ungraded, grading is stale, execution is pending, or relevant claims remain unresolved. Return a separate final `Measure<number>` with an unavailable reason until the selected coverage is settled. No admitted reviews means unavailable, not a confirmed zero. Report incomplete historical audit coverage separately; an incomplete catalog does not erase observed catches. All aggregation belongs in `src/lib/scoring.ts`. Apply the existing task and concern selections to bonus counts; show their coverage explicitly.

### Clean controls

A task with no ordinary reference problems may remain an audited-clean control despite approved bonus problems. Preserve Kubernetes's stated status, subject to pinning its actual audit and ruling. Do not infer an audit from the absence of ordinary references.

Keep existing false-alarm semantics. A supported bonus assertion does not alarm a control; an additional refuted or unsupported assertion still does. An unresolved claim still prevents a final clean percentage, and failed output is still missing output. Rename displayed “Correctly silent” to “Without a refuted or unsupported claim,” because a review can legitimately speak about an older defect. Rejected alternative: removing bonus-only tasks from controls would contradict the ticket and discard useful false-alarm evidence.

For first implementation, retain the existing conservative rule that pending candidates make an audited control provisional. Narrowing that rule to proved outside-change candidates can be decided later; a grader's guess about candidate kind must not certify a control.

### Eligibility and intent

A bonus requires an established defect at the pinned revision, reachable under supported use, with a concrete qualifying consequence and independent evidence of its obligation. The change must neither introduce, worsen, expose, nor leave responsibility for that defect under the adopted Before 2 rule. Preserve ordinary references when the PR promises to fix the defect, makes it reachable, or changes its behavior.

“Was it intended?” is evidence about whether behavior is a defect, not a sufficient bonus test. Prefer documented contracts, the function's purpose, tests, and a reproducer to guessed author intent. Unintended but inconsequential behavior does not automatically qualify; deliberate behavior can still violate an obligation. Require the owner to adopt this interpretation before applying it.

The grader must establish factual support before granting bonus eligibility. Older code alone proves neither truth nor scope. Keep `refuted`, `unsupported`, advice, and unresolved outcomes distinct. A true older behavior without a proved obligation or consequence remains uncredited. For outside-change candidates, continue the consequence and supported-use assessment after the responsibility question rather than stopping early at “outside this change.” Keep both credit facts, `says_what` and `identifies_cause`; use the existing wording-based catch rule. A cause-only comment earns no bonus. A novel candidate earns nothing until approved.

### Initial conversion boundary

Inventory exactly the five reported candidates first:

| Ticket locator | Task | Candidate requiring pinned evidence |
| --- | --- | --- |
| 2 | `x-kubernetes-141463` | Byte truncation splitting a character; constant copying and renaming preserve behavior |
| 3 | `s-seaweedfs-10735` | Expired names fill a listing page that returns empty |
| 4a | `s-seaweedfs-10735` | Recreation during expiry retains a record but loses its name |
| 4b | `s-seaweedfs-10735` | Creation during listing has its new record deleted |
| 5 | Astro 16079 | Edge middleware loses the query string |

Resolve Astro's exact target ID from the selected registry during intake. Verify each candidate against the selected packet and cut-off. Keep 4a and 4b separate pending a causal-grouping ruling. Approval of outside-change status does not itself approve bonus status, impact, grouping, or individual-comment credit. Do not bulk-convert every `scope-excluded` claim. Earlier rulings enter a later inventory and audit with its own explicit scope.

## Data contract and implementation sequence

1. **Save the policy and evidence prerequisites.** Locate the missing ruling directory or its authoritative successor; record commit and path/hash pins, receipt passages, revision applicability, and unresolved discrepancies. Prepare a versioned rule proposal containing the ticket's two sentences, explicitly preserving the existing introduces/worsens/exposes and promised-fix conditions. Have the owner read that version before adoption. Record candidate, grouping, recovery, and control decisions through `docs/adjudication-record.md`, including neutral cases, pinned precedents, required independent answers, before/after records, and `ruling_record.py route`. Rule-text adoption uses its saved receipt rather than inventing a candidate decision. New bonus families remain an owner decision; do not infer expanded delegation authority.

2. **Make family purpose explicit.** In current references, introduce required `kind: "ordinary" | "bonus"` on causal families. Retain shared stable identity, revision, obligation, trigger, mechanism, evidence, grouping, and eligibility provenance. Ordinary families retain impact. Bonus families have no scored impact requirement; validate that variant rather than filling `unknown` and polluting impact queues. Add `bonus` as an approved claim/eligibility outcome with a family-kind consistency check. Existing nonbonus scope exclusions stay intact. Update `current-reference`, `current-adjudication`, `current-claim`, `current-grade`, and affected candidate/credit schemas and examples together. Use a versioned current-format migration and reject ambiguous missing kinds after migration; do not silently interpret historical records as converted judgments.

   Use a discriminated assessment union: ordinary recovery remains `caught | missed | unresolved`; bonus assessment is `caught | not-caught | unresolved`, with reasons and supporting claim IDs. `not-caught` describes a completed assessment and has no penalty. Enforce approved receipt, same-task/revision joins, and valid bonus claim anchors. Retain safety assessments for every proposed remedy. Separate bonus fix sufficiency from ordinary caught-family sufficiency totals.

3. **Extend the grading path end to end.** The active `validation-policy.json` pins `scoring.next.md`, `rules.next.md`, `grader.next.md`, and `current-verdicts/v2`; their `.next` names do not mean inactive. `claim_grading.py` currently hardcodes claim outcomes and derives recovery in `family_recovery_v2`. `grading_validation.py` constrains canonical outcomes in `pinned_matches_v2`. Extend both through a new verdict contract, proposed `current-verdicts/v3`, with explicit bonus eligibility evidence and family kinds. Update `grade.py` preparation, validation, mapping, candidate capture, and recomputation in `current_grading.validate_review_v2` through a version-specific successor. Preserve old contract readers for immutable assessment receipts; current grades must match the active policy.

   Update `claims.py` context and assignment checks, `ruling_record.py` outcome validation, and any delegation mapping that could incorrectly approve a new bonus family. Grade workspaces expose the family definition and kind needed for assessment, while keeping reviewer identities, native priorities, private keys, and earlier grades out. Reviewer sessions receive none of these records.

4. **Correct current validation and queues.** `current_grading.validate_documents` currently rejects audited-clean controls with any family and requires every approved family to be `eligible`. Replace those assumptions with kind-aware checks. Update `control_state`, `coverage_status`, `calibration.py` impact/control queues, and audit strata so bonus families do not require impact cards or imply ordinary known-problem status. Keep ordinary pending candidates and impact uncertainty behavior intact. Ensure `grading_inputs` fingerprints kind, bonus rulings, credit decisions, canonical links, and the new validation policy. Its existing family serialization already excludes only impact, so test that every new relevant field actually changes the fingerprint.

5. **Project facts and calculate scores.** Update `tools/export_explorer.py`'s `assessment` and `write_export`, then `src/lib/data.ts` Zod schemas and joins. There is no `src/lib/types.ts` at this HEAD; types derive from `data.ts`. Bump export schema version 4 for the incompatible variants. Add the bonus aggregation to `scorecard()` and make `references()`, `familyRows()`, `referenceCoverage()`, and `reviewFacts()` distinguish ordinary and bonus units. Audit `remedies()`, controls, pending-candidate limits, and recommendation inputs for accidental inclusion. Claims remain separately classified, so a mixed item can earn bonus credit and contain an unsupported allegation.

   Address an existing compatibility gap in this path: `graded_v2` writes native outcomes and a `known_problems` array without claim `family_id`, while `assessment()` and `export_attempt()` index `family_id` and the web schema expects legacy outcomes. Add an explicit contract-aware projection for v2 and v3, preserving multiple known-problem facts rather than choosing the first family. Test native v2 `problem`, `outside-this-change`, `unproven`, cause-only suggestions, and multi-family claims before adding bonus output. Do not mechanically normalize saved evidence.

   Render kernel results in `MethodologyViews.tsx`, `Explorer.tsx`, and the CLI. Update fixture builders, inspection details, explanatory text, and `metrics.ts` hero counts. Keep the existing known-problem count defined as ordinary problems; report bonus catalog size separately. Full-dataset hero counts must not change with active chart filters.

## Migration and rollout

First land code, contracts, fixtures, and proposed documentation without activating new policy or changing current judgments. Then apply the accepted rule version and migrate current data in a controlled release. Capture a before manifest of selected inputs and fingerprints. Convert existing families structurally to ordinary without changing their judgments. Add only individually approved bonus families and claim decisions. Retain all raw reviews, frozen manifests, historical registries, and earlier assessment directories.

For each affected task, inventory all selected saved review items using `claims.py inventory`; inspect equivalent and related links, combined allegations, past rejections, quiet reviews, and all competing configurations. Preserve canonical identities when appropriate and save reconciliation versions with old/new outcomes and source pins. Current records keep one current value, as their contract requires; version the reconciliation and assessment receipts rather than fabricating supersedes chains in current claims.

Run fingerprint checks and create an explicit regrading queue. Family changes invalidate every selected batch of the task; switching the global verdict policy may invalidate all selected batches. Budget and authorize that actual scope before dispatch. Do not narrow it silently to the five discovering comments. Use fresh blinded graders, saved evidence, current authorization, and `regrade.py --workers` for any bounded concurrency. Preserve failed attempts, replacement usage, and runner deviations. Map new assessments only after validation; prune verified successful workspaces through the existing cleanup path. No reviewer benchmark is needed.

Publish regenerated current data only with its actual completeness status. A partial rollout may show unavailable final bonus totals and current evidence limits; it must not show historical unassessed comments as zeros. Rollback restores the prior coherent current-policy/data snapshot while retaining new evidence and failed migration records. Audit earlier outside-change rulings later, using a declared population and coverage report rather than implying the first five exhaust the catalog.

## Verification and acceptance

Extend the existing Python suites for current grading, claim grading, grade preparation/mapping, calibration, ruling records, and export. Prove that bonus approval requires an applicable owner receipt; wrong-kind family links fail; changed kinds/rulings stale every affected task batch; disputed links block mapping; unsupported older claims cannot become bonus catches; pending candidates remain pending; and old raw assessments remain readable.

In `scoring.test.ts`, cover duplicate bonus mentions, repeated trials, two independent families, mixed bonus/unsupported items, bonus-only controls, no admission, missing/stale assessments, unresolved claims, and pending execution. Adding an approved bonus with no catch must leave every ordinary detection result unchanged. A caught bonus must not change recall or recommendation ordering. Test remedy safety independently, including an unsafe proposed fix for a correct bonus claim.

Run focused Python tests, `bun test ./src`, `bun run typecheck`, export/CLI tests, then `bun run verify:claims`, `bun run verify:current`, `bun run data`, and `bun run scorecard`. Inspect the bonus and control states with `bun run dev:fixture`; fixture data is never benchmark evidence. Before publication, run `bun run verify:import` and preserve intended imported edits using the command it prints. Report blocked or unavailable checks without substituting guessed results.

Remaining owner decisions are the bonus eligibility threshold and intent interpretation, exact Before 2 adoption, count/display semantics, impact exclusion, initial five approvals and grouping, comment-credit disputes, historical audit scope, and any paid regrading authorization. Missing authoritative ruling records block conversion, not implementation against synthetic fixtures.
