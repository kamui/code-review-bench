1. **Finish filing the accepted confirmations and GT-o4 corrections.** Records: `AD-CL-q-lfs-log-unused-read`, `AD-CL-r-flip-render`, `AD-CL-r-marked-dirty-stale`, and `GT-o4`; [plan.v1.json:785](docs/research/regrade-v10-rulings-2026-10-17/filing/plan.v1.json:785).

   During this review, `filing/answers.md` appeared with the owner's **“accept all recommendations”** answer ([line 14](docs/research/regrade-v10-rulings-2026-10-17/filing/answers.md:14)). Its lines 19–21 explicitly approve ruling 05's scope exclusion, ruling 18's two improvement kinds, and GT-o4's corrected grouping openings. The original ruling files alone do not settle those three filing choices; this later answer supplies the confirmation.

   The generated records still predate that answer. Receipt `cohort-rebuild.v11.md` contains no “Answers on the filing” section or acceptance answer, although all three decisions pin it as their human authority. Their reasons still say **“The outcome is proposed”** or **“The kind is proposed; the user has not named it.”** Those statements also remain in the current plan's `ruled` strings at lines 785, 860 and 899, so regeneration alone will preserve them. GT-o4's [reference grouping:3050](bench/grading/current/references.json:3050) still begins **“one fault in accepting a platform-supplied placeholder”**, and its [card:18](bench/grading/current/impact-cards/GT-o4.json:18) still defines the common fault as an **“unfilled platform placeholder”**. The newer plan already contains the approved replacements.

   **Fix:** reconcile those three plan reasons with the accepted answer, then use the script's undo/refile workflow to regenerate the receipt, records, cards, approval preview and queue. Confirm the new receipt includes `answers.md` and refresh every affected pin. Do not manually change the already pinned receipt in isolation.

2. **Bound GT-p3's latency wording to the comparisons actually established.** [plan.v1.json:642](docs/research/regrade-v10-rulings-2026-10-17/filing/plan.v1.json:642), `CL-p-first-read-time`, and [GT-p3.json:10](bench/grading/current/impact-cards/GT-p3.json:10).

   The added card says **“every request that reads a form with `c.req.parseBody()` take longer”** and **“every form read … pays it.”** The saved [case table:74](docs/research/regrade-v10-rulings-2026-10-17/p-hono-5067/cases/M1.md:74) gives a streamed 1 MB Node upload as **“2.2 [0.9-2.6]”** before and **“1.8 [1.8-1.9]”** after; the Bun 8 MB row likewise has overlapping ranges and no measured slowdown. The plan itself says the 1 MB upload takes the same time. Thus the universal consequence overstates the measured regression.

   The new claim also omits the timing comparison's lowercase media-type prerequisite and asserts unchanged fields/files. The [case:145](docs/research/regrade-v10-rulings-2026-10-17/p-hono-5067/cases/M1.md:145) explicitly distinguishes mixed case: **“the base returns `{}` without reading the body”**, while **“The head returns the fields”**. That additional parsing is the intended repair, not the same-result comparison described by the claim.

   **Fix:** name the previously parsed media types used in the comparison, retain the demonstrated large-upload Node slowdown, and qualify the card's exposure by runtime, body size and delivery method. Carry those qualifications into the canonical claim and generated card.

3. **Correct PE-x2's blanket percent-expansion mechanism.** [plan.v1.json:25](docs/research/regrade-v10-rulings-2026-10-17/filing/plan.v1.json:25), [references.json:6768](bench/grading/current/references.json:6768), and `CL-x-percent-in-event-note`.

   The trigger says **“Any `%` alters the note”**, and the mechanism says **“The note grows by several bytes for each `%`.”** The saved [case:88](docs/research/regrade-v10-rulings-2026-10-17/x-kubernetes-141463/cases/P1.md:88) shows **“`load is 95%% of capacity` | 24 | 23 bytes”**: doubled percent contracts instead of expanding. [Line 95](docs/research/regrade-v10-rulings-2026-10-17/x-kubernetes-141463/cases/P1.md:95) shows **“one `%` at byte 1050, cut away by the shortening”** producing an accepted 1024-byte event without a formatting alteration.

   **Fix:** say that percent sequences surviving truncation are interpreted as formatting instructions; unescaped directives can add missing-argument markers, `%%` becomes `%`, and truncated-away percent signs have no formatting effect. Apply the correction to both the reference and duplicated claim text. The admitted problem and its label can remain.

Review scope: dirty working tree at HEAD `5f188bf7c3aaf479f7dbd577f6f45d417f8b26b6`, including the filing answers and plan/script updates observed during the review. No repository files were edited, staged or committed.

Checked and found correct:

- Apart from the filing synchronization issue above, the 23 rulings' admissions, exclusions, groupings and retained labels match the saved answers; all 14 credit pairs match their `says_what` / `identifies_cause` facts and exact review items. All 16 candidate closures are present.
- The 99 generated receipt passages each occur once and are cited once. All 33 repointed decisions retain their previous receipt pins as evidence.
- The mixed tRPC M3 candidate closes as eligible through widened GT-j7 while explicitly retaining M3b's refutation; its comment remains no/yes for GT-j7 credit.
- Ruling 20 confirms the existing Django claim, adds its review links, preserves its outcome and prior receipt, and indexes `regrade-v10 20` as an alias of `regrade-19`.
- Static reconstruction of undo restores all five record files, the 11 previous cards and ruling index to HEAD; the regenerated queue also matches HEAD. The code removes the new receipt and six new cards. Undo was not executed; the generated approval draft remains outside that restoration.
- `python3 bench/tools/current_grading.py check` passes; `python3 bench/tools/calibration.py check` passes with 71/71 family bands and 14/14 pre-existing labels approved. Calibration's blinded-card check found no exposed reviewer identifiers. The ruling tests could not run because `pytest` is unavailable.
