# Candidate B: Tiered claim adjudication for the code-review skill benchmark

## Where this stands

**Observed facts** (from the grounding file):
- ADR-0002 says the user decides every new or disputed finding before it can affect official scores. Unmatched findings stay unjudged.
- ADR-0003 freezes reference findings for each release. Accepted discoveries only change scores in the next release, when all comparable retained reviews are regraded.
- Rubric v1 gives every item one of four labels: recovery (`defect:<id>`), `false-finding`, `non-material` or `unresolved`. An `unresolved` item counts toward neither recall nor false findings.
- **SeaweedFS #10735:**
  - Upstream reviewers found an `InsertEntry` race in an earlier PR version. The author accepted it and added compensation before merge.
  - No accessible upstream evidence rules on the separate `UpdateEntry` interleaving (Claim 3).
  - A controlled pinned test used a Redis test double. After TTL expiry and cleanup, an update left the file out of the listing at head but not at base.
  - Earlier graders split between `non-material` and `unresolved`.
- **User rulings on record:**
  - The user has now ruled Claim 3 eligible and a bug. An update that began while the TTL was valid must either end with correct file and index state, or report expiry and refuse the write.
  - That is a saved user decision, not a maintainer ruling. It does not pick a remedy, and it does not establish that base or head is correct.
  - The user also accepted two ripgrep documentation claims about `compinit` placement.

**Everything below is proposed policy.** None of it is implemented. Lanes A–C (Rule 2) would need an ADR amending ADR-0002.

## Recommendation: a hybrid with delegated lanes

Use three sources of evidence, each limited to the question it can actually answer:

| Question | Primary source | Secondary |
|---|---|---|
| Is the claim technically true at the pinned revision? | Benchmark adjudication: pinned code, tests, reproductions | Maintainer's factual statements |
| Is it caused by this PR? | Benchmark adjudication (base vs head) | Maintainer's statements about scope |
| Is it material under the rubric? | Rubric's consequence classes | An explicit maintainer judgment, recorded as evidence |
| Would this project act on it? | Explicit maintainer judgment | Unknown if there is none |
| Anything disputed, judgment-dependent or new to policy | User | — |

Maintainer judgment stays in the record as its own dimension, reported on its own axis, but it is not the gate for inclusion in the reference set. Routine, clear-cut claims settle by rule, and the user sees them only as a batch they can veto.

## Decision rules

**Rule 1: Explicit maintainer judgments are evidence, not verdicts.**
- Weigh upstream statements by strength, as Candidate A proposed: explicit acceptance > author acknowledgment plus maintainer merge > nothing.
- Merges, resolved threads, bot approvals and silence are recorded as "no disposition." They never count as rejection.
- A maintainer's factual refutation becomes a `false-finding` only if benchmark adjudication confirms it against pinned evidence. If the refutation conflicts with a reproduction, the claim goes to the user.

**Rule 2: Delegated lanes** (these need ADR-0002 amended):
- **Lane A: upstream-confirmed.**
  - Requirements: an explicit acceptance of this specific claim by a responsible maintainer; a linked fix commit or test; and adjudication confirming the same mechanism, trigger and consequence survive at the pinned head.
  - Result: approved with authority `rule:A`.
- **Lane B: reproduced and new.**
  - Requirements:
    - A deterministic reproduction that fails at head and passes at base, so the defect is attributable to the PR.
    - A consequence in a pre-declared material class: data loss, wrong result, crash, broken security boundary, or violation of a documented contract.
    - Two independent blinded adjudicators agree.
    - No upstream statement contradicts it.
  - Result: approved with authority `rule:B`.
- **Lane C: clearly false.**
  - Requirements: the claim is contradicted mechanically by pinned code or a test (for example, it names behavior that isn't there), and two adjudicators agree.
  - Result: `false-finding` with authority `rule:C`.
- **Veto window.** Lane decisions build up until a reference release freezes. The user gets one digest and can veto any item. An item not vetoed becomes canonical at release. ADR-0003's release gate is the natural checkpoint, so no score moves without a chance to veto.
- **Everything else goes to the user.** That covers:
  - adjudicators disagreeing;
  - materiality that depends on judgment (performance without measurement, design preference, documentation);
  - timing-only races without a controlled reproduction;
  - conflicts between a maintainer and a reproduction;
  - disputed remedies.

**Rule 3: How maintainer dispositions map to the rubric.**
- **"Valid, but deferred or not worth it now,"** when caused by the PR: still a recovery. Record `maintainer_disposition: deferred`.
- **"Pre-existing, not introduced here":** `non-material` for this PR's register. It is not a false finding.
- **"By design," citing a documented contract:** a `false-finding` if the claim alleged a contract violation that the contract contradicts. Otherwise `non-material`.
- **Maintainer-stated severity** is the reference for `priority_error`.

**Rule 4: Fix sufficiency is judged against the required outcome, not against base behavior.**
- A remedy is sufficient if it produces the outcome the user or maintainer required.
- "Revert to base" is not automatically sufficient.
- Matching the upstream patch is not required.

## Evidence provenance

Each upstream judgment gets saved with these fields:
- `source_url`, plus the comment or review ID;
- the verbatim quoted text;
- the author's login, and their role when they wrote it (merge rights, or a CODEOWNERS/MAINTAINERS entry for the touched path at that date);
- the PR revision the comment was attached to (force-pushes matter: the SeaweedFS insertion race was raised against an earlier version);
- the fix commit or test, if any;
- when the comment was written, when it was retrieved, and the hash of an archived snapshot;
- `disposition`: accepted / rejected-factual / rejected-scope / deferred / by-design / none;
- `authority`: `maintainer-explicit` / `author-ack` / `rule:A|B|C` / `human`.

Handling rules:
- Post-merge evidence (follow-up fixes, reverts, issues) is admissible for grading, with timestamps. If a later explicit judgment supersedes an earlier one, record it as a new mapping version.
- None of this evidence goes into reviewer inputs, consistent with clean-context.md.
- Claim records say "coverage as of retrieval," never "upstream never considered it."

## Repository and maintainer selection

**Keep the current PRs.** Audit them; don't re-select them. The criteria below apply to future additions and are measured on the repository's merged PRs from before the target date, before anyone looks at target outcomes:
- **Reasoned reviews:** a stated threshold share of substantive review threads gives a reason for accepting or rejecting.
- **Enforced checks:** CI with required tests. Compatibility and API policy are documented.
- **At least two distinct people with merge rights,** so the author and the approver are not always the same person.
- **Identifiable responsibility:** CODEOWNERS or a MAINTAINERS file, or an observable proxy — the reviewer authored or reviewed commits on the touched paths in the prior 12 months. This replaces Candidate A's "reviewer has expertise," which can't be observed.
- **Traceability:** reverts and follow-up fixes link back to their PRs.
- **Balanced sample:** include PRs that deserved approval, so false positives get measured. Cap the share of any one repository.

## Scoring consequences

- **Headline recall** uses only registered defects: approved by lane or by the user, and frozen in a release.
- **New secondary columns:**
  - *upstream-endorsed recall*: recoveries of defects with `maintainer-explicit` acceptance;
  - *beyond-upstream recoveries*: registered defects that upstream never raised, like Claim 3.

  This keeps "found what reviewers found" separate from "found more than they did," and measures maintainer-grade usefulness without letting historical silence cap recall.
- **Unreviewed new findings** remain `unresolved`, as today. They count as neither recall nor false findings, and they block any success claim that depends on them.
- **Report adjudication coverage for each arm** (pending items ÷ all items). An arm that produces many unverifiable claims can't hide behind the fact that unresolved items don't count as false findings.
- **When a pending item is resolved,** it enters the next release, and all comparable retained reviews are regraded (per ADR-0003).
- **False findings** come only from Lane C, from a user ruling, or from a maintainer's factual refutation that adjudication has confirmed. A maintainer declining a fix is never enough.

**Claim 3 under these rules:**
- Adjudicators disagreed, so the rules would have sent it to the user, which is correct routing. It is now a user-approved claim with authority `human`.
- It enters the next release as a defect separate from the upstream-accepted insertion race.
- Its record has `maintainer_disposition: none`, with coverage stated as of retrieval.
- Fix-sufficiency criterion: the update completes with consistent file and index state, or it reports expiry and refuses the write.
- The record makes no claim that base or head is correct.
- The upstream PR's historical reviews count as not having raised Claim 3.
- Historical scores don't change. Earlier `non-material` grades on items that raised this claim get regraded in the new release.

## Where Candidate A falls short

1. **The historical record can't be complete ground truth.** Candidate A treats maintainer judgment as the preferred source of usefulness. But upstream reviewers are also the baseline being compared against, so their coverage becomes a ceiling on recall. Claim 3 shows the problem: it is a bug the user has now ruled on, with no upstream ruling. Under Candidate A it would stay at "disposition unknown" indefinitely.
2. **It never says how anything is scored.** "Disposition unknown," "deferred" and "rejected on scope" never map to rubric assignments, recall, or the false-finding count.
3. **It doesn't reduce the user's workload.** Silence is the most common upstream outcome, so under "the user resolves gaps" most claims still reach the user. Candidate A has no rule-based way to settle clear-cut cases.
4. **It merges two separate questions:** "material" and "worth changing now in this project." A deferred bug is still a defect. The rubric's threshold is about consequence, not project willingness.
5. **Its provenance is thin.** It doesn't record the PR revision a comment targeted, the maintainer's role at the time, verbatim text, or archival. It also doesn't use post-merge evidence.
6. **One selection criterion can't be observed.** "Reviewer has expertise in the affected subsystem" has no measurable definition.
7. **It doesn't say whether a fix is sufficient.** A maintainer accepting a fix doesn't establish that the fix meets the required outcome, and a user ruling doesn't either.

**Worth keeping from Candidate A:**
- recording technical correctness and project action as separate judgments;
- the evidence-strength ordering;
- "accepting a suggested change does not establish every consequence the reviewer alleged";
- keeping reasons for rejection;
- applying selection criteria before looking at outcomes.

## Alternatives rejected

- **Maintainer-led as final authority.** Silence dominates the historical record. The approach is circular, since the baseline reviewers become the answer key. Contacting maintainers wasn't authorized, puts a burden on them, and invites bias toward their own merged work. It remains a possible later pilot as a separate "persuasiveness" track, not a grading authority.
- **Fully adjudicator-led (automated).** ADR-0002's rationale still holds, and Claim 3's split grades show automated judges disagree on exactly the cases that matter. Lanes limit automation to mechanically checkable cases and keep a veto.
- **Majority vote across graders.** Agreement among graders doesn't establish truth. Votes only decide routing, never outcomes.
- **Ignoring maintainers entirely.** That throws away the best available evidence of actionability and priority.

## Adoption sequence

1. **Policy (the one user decision needed).** Draft an ADR amending ADR-0002 to define Lanes A–C, the veto digest and Rule 3. The user approves or edits it.
2. **Provenance backfill.** Extract explicit upstream judgments for the existing PRs into the schema above. No score changes.
3. **Shadow run.** Apply the lanes to past and pending claims. Go live only if no lane decision contradicts a saved user ruling (ripgrep ×2, Claim 3). Disagreements tighten the lane criteria.
4. **Release.** Freeze the next reference release with lane approvals that passed the veto window, the ripgrep claims and Claim 3 (with its outcome-based fix criterion). Regrade all comparable retained reviews and add the secondary columns.
5. **Review.** After one release, compare the size of the veto digest and the user's override rate. Narrow or widen the lanes based on those numbers, not preference.

**Remaining uncertainty:**
- Lane B's material classes and the reasoned-review threshold need calibration against actual veto rates.
- Claim 3's evidence uses a test double and controlled timing. The user's ruling settles eligibility, not how often the problem happens in production.
