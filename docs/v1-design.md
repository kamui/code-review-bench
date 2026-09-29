# V1 design

Status: accepted and implemented in the local explorer. Aggregate fix-quality comparisons are deferred at the user's request.

## Outcome

A local web app compares practical code review configurations using the existing twelve PR tasks. Users can explore the score, cost, output tokens, and false findings, then inspect the reviews and adjudication evidence behind each point.

The initial dataset contains historical, model-assisted judgments. Show that provenance and keep a human-audited official release distinct from the import.

## Extraction

Use skills commit `6457c79f955c2d6740fe730689a16af6f3aefacb` as the source.

- Preserve task revisions, packets, reference registers, run manifests, configuration definitions, attempts, mappings, result versions, per-request usage, rates, and provenance.
- Copy referenced transcript archives with a checksum manifest. Both primary runs have verified archives: 83 baseline and 74 Sonnet 5.5 rebench attempts. Check availability for additional imported experiments rather than assuming it.
- Retain existing skill experiments as historical evidence, hidden from the default built-in comparison.
- Keep the Python runner and grading tools. Build the explorer with Bun, TypeScript, React, TanStack Start, and Mantine. Mantine Charts and Recharts render the interactive scatter plots. Preserve the grading templates at the paths the runner expects.
- Keep dependency caches and repository mirrors in a configurable external cache. Preserve provisioning recipes and pinned identities; private seal keys remain outside the repository.
- Omit unrelated changelogs and skill-development narratives. Keep methodology needed to interpret the results.
- Verify the extracted copy before removing anything from the skills repository.

## Identity and comparison

A review configuration records its method, client/version, model, effort, prompts or skill revision, tools, permissions, and execution limits. Native tools and delegation are permitted within the common evidence restrictions. The reviewer cannot access the reference findings or later fixes.

Built-in review logic belongs to the client harness. Record the observed harness version and prompt hash on each attempt and show both in its evidence. Distinct harness versions or prompt variants are separate configurations, including when they use the same model. Missing observations stay unknown.

A review trial identifies one scheduled repetition of a configuration on a task. Infrastructure replacement attempts remain linked to that trial. Original attempts and their measured costs remain available.

The default chart uses task versions comparable across all selected configurations, currently nine for the imported built-ins. The task catalog contains all twelve. Show omitted tasks and why they are unavailable or incomparable; never represent an unrun task as a zero-scoring attempt.

## Measurements

For each buggy task, divide distinct recovered reference problems by eligible reference problems. Average repetitions within the task, then average those task scores equally and multiply by 100. Each problem has equal weight within its task. A qualifying finding identifies the problem and its material consequence; no fix suggestion is required.

Clean tasks have no detection denominator. They contribute to false-finding and resource measurements. If a selection contains no eligible reference problems, show detection score as unavailable.

Valid findings from an incomplete review still earn detection credit. Show completion separately. A finished trial with no usable findings earns zero recovery; an unrun or still-pending trial is missing data. A poor review is not eligible for replacement. Infrastructure failures can receive replacement attempts, with their usage included in the trial's total.

Keep historical metrics under their original definitions. Any recalculation with revised trial handling or credit rules must identify its metric version and the source judgments. Retained outputs cannot be given new detection credit without supporting adjudication.

False findings are distinct adjudicated false claims per review. Display duplicates, harmless noise, and unresolved claims separately. Pending adjudication makes affected noise measurements provisional. Proposed fixes never reduce or increase detection credit.

Reference severity is independently adjudicated as Critical, High, Medium, or Low using impact and realistic trigger conditions. Report Critical + High as high-severity detection and offer Critical-only filtering. Severity does not weight the main findings score. None of the imported references currently has these labels: display them as unclassified until adjudicated, preserve reviewer-assigned priority separately, and show severity-based scores as unavailable when no classified references qualify. Expose classification coverage alongside any severity-based score.

Cost includes recorded review usage, subagents, and retries, priced using dated rates. Identify reported API costs and list-price equivalents. Output tokens include reasoning and subagent output. Grading and environment setup costs are separate. Missing usage is unavailable, not zero. Use the same selected tasks and repetitions for resource comparisons, including clean tasks.

Freeze references and scoring rules per benchmark release. Accepted discoveries appear separately until the next release; regrade all comparable saved outputs against the next reference set while preserving earlier results.

## Explorer

- A scatter plot keeps findings score on the vertical axis and switches the horizontal axis among average review cost, output tokens, and false findings. Zero is on the right.
- Dotted lines connect configurations of the same review method and version across models, ordered along the selected horizontal axis. Built-ins group by harness release, skills by skill revision. These lines show the method/version family, not an efficiency frontier or a claim that all other settings match.
- Configuration controls expose method, model, client/version, effort, and historical versus current metric versions. Every plotted point exposes its task coverage, repetition count, and evidence status.
- Task profiles support overlapping change-kind, code-area, technology, and review-concern labels. Show task and reference-finding counts for each category.
- Task filters select PRs. A finding-concern view restricts detection to reference findings carrying that concern, rather than counting every bug in a tagged PR as a bug of that category.
- Configuration and task details lead to individual reviews, reference findings, adjudication notes, usage, failures, and transcript evidence.
- Preserve fix suggestions and existing sufficiency grades in review details. Design aggregate fix-quality comparisons later.
- Show failure causes and links between original and replacement attempts. Repairs or revised configurations produce new evidence rather than overwriting old results.

Run launch controls and adjudication editing are outside the initial app. Existing command-line workflows remain available.

## Verification

- Check copied evidence against source checksums and enumerate missing artifacts.
- Reproduce the published historical figures using their original metric definitions and common task set.
- Verify deduplication, averaging, retry accounting, missing-data behavior, and clean-task handling with small hand-calculated examples.
- Exercise the local app: switch all three chart axes, select configurations, filter task profiles, inspect a review and its raw evidence, and trace a failed attempt to its replacement.
- Confirm the app and extracted tools resolve paths without the skills checkout. Record remaining external runtime and cache requirements.

## Follow-up after extraction

Audit historical reference findings and propose severity labels for adjudication. Investigate the recorded failures and apply warranted repairs while preserving historical outcomes. Revisit aggregate fix-quality comparisons after the explorer exposes the existing suggestions and sufficiency evidence.
