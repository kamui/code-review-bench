# Proposal: make the next rulings evidence we can learn from

Keep the contract check, but do not call the missed-contract problem solved. Before the 13 open rulings, require a checked preparation record for each question, correct the rules that overstate the rulings, and save each first answer before showing the owner's answer. Keep these 13 decisions with the owner. Do not widen delegation yet.

The recommendations below are ordered by value: 1, verify what reaches the owner; 2, correct the rule; 3, preserve predictions and reasons for referral; 4, enforce those referrals in a future delegation. Do 1 through 3 now. Wait on 4 until delegation work resumes under issue 59.

I reviewed repository head `a850d351c8f4ef84f591158a70a1b98c82741203`. Paths below are relative to that repository. Ruling names such as S9 refer to the saved [second-pass rulings](docs/research/cohort-rebuild-2026-10-05/second-pass/rulings). This is a proposal, not acceptance of new policy or a change to any ruling.

## 1. Missing the contract

### What is mitigated

The new check catches an absent declaration. The brief tells a preparer where to look. The checklist tells the asking session to do its own work. Together they reduce omissions, provided both sessions follow them. They do not establish that either session found the governing contract.

I ran `python3 -B bench/tools/ruling_dossier.py DIRECTORY` against all nine second-pass candidate directories. Eight failed. All 15 candidate entries lacked the required `promise` object. The gRPC directory passed because its only item is a recovery question, which the check skips. These are counts across the saved directories, including already-ruled items, not a count of the 13 open rulings.

The four existing unit tests pass. Three temporary probes also passed when they should not support a recommendation:

| Input | Current result |
| --- | --- |
| `source: "trust me"`, `searched: ["documentation"]`, empty Promised and Delivered sections | Pass |
| Numeric interface owner, boolean source, string instead of search list | Pass |
| `made_by: "none"`, `delivered: null`, but `recommendation: "eligible"` | Pass |

The passing unit test itself contains empty sections. `owner_statement` and `practice` need only exist, and may both be null. No code checks a quotation, date, saved source, search result, opposing contract, or the question shown to the owner. A misspelled `kind` is skipped like a recovery question. Running the unit tests in CI does not run the check over active dossiers.

### Would it have caught the five recorded failures?

"Caught" needs two meanings: stopping an incomplete artifact, and preventing the wrong recommendation. The current check can do the first. None of these cases proves it can do the second.

| Failure | What was missed or misread | Which new piece addresses it | What still gets through |
| --- | --- | --- | --- |
| 02, cipher defaults | Dependency ownership, private status, removal, earlier maintainer objections, documented adapter route | Brief step 4 explicitly asks for all of these. The original candidate summary now fails the check. The asking checklist repeats the search. | A `practice` promise citing the two applications can pass without searching maintainer history. Null `owner_statement` is accepted. A reproduction alone still looks persuasive. |
| R5, `cancel()` | The general customization handbook, beyond the component page and PR sentence | Brief step 4 and the asking checklist explicitly require general interface documentation. | A source quote from the PR is enough for the checker. R5 was a review of a saved first-round ruling, outside the check's normal candidate-directory entry point. |
| S1, pyOpenSSL | Which timing phrase governs, and whether selecting the backend is itself the promised outcome | The brief asks for dependency deadlines. The Delivered question rejects the earlier "requests still succeed" reasoning. The old summary fails if checked. | The original N1 dossier already quoted both timing phrases and argued for the documented operation. This was also a failure to apply evidence already present. Populating `source` would not resolve it. All four later blind runs flagged the deadline ambiguity. |
| S2, key logging | Programs using the documented feature through a different order | Brief step 4 requires public-code searches and dates. The old summary fails if checked. | A written promise based on shell-export documentation, or a claimed unsuccessful search, passes. S2's new examples were explicitly **undated and unrun**. The ruling stands, but it does not prove pre-merge practice was established. |
| S9, certificate path | Intended use of the name, the owner's old answer, and the boundary between reading and assignment | The brief now names purpose, owner statements, and dated examples. The asking checklist covers a re-review. | This was another review of a saved ruling. No checked asking record exists. Even after the extra evidence, the recorder recommended a problem. Four blind answers followed wording derived from this same case. Neither source presence nor agreement predicted the owner's distinction. |

The [discussion](docs/research/cohort-rebuild-2026-10-05/second-pass/DISCUSSION.md) compresses these into missing facts. Preserve the finer distinctions. S1's deadline was already in its dossier. S2 did not date the newly found programs. S9 involved an interpretation the owner supplied. More searching alone does not fix all three.

### Improvement 1, now: check the preparation for the question

Change `docs/claim-adjudication.md`, "Prepare a ruling", by adding this requirement before "Record the blind answers":

> Before every question, including a review of a past ruling, save a preparation record that pins the dossier, the evidence supplement, the rules used, and the exact question and options. Run the preparation check on that record. A passed check means the required evidence and assessments are recorded; it does not mean the recommendation is correct. Do not turn missing evidence into "Promised: no". Show the missing fact and leave the decision unresolved if it prevents a fair ruling.
>
> The asking session must open the cited contract and its relevant limits, and check the strongest contrary source. Record what it checked and what changed in its first recommendation. If the contract is already adequately established, reuse the saved evidence and verify its applicability. Repeat a search when coverage, dates, source identity, or the operation remains doubtful.
>
> Put the decisive quotation, contrary evidence, evidence limits, and referral reasons inside the question shown to the owner. A fact shown only in a preceding message does not satisfy this requirement.

This replaces an unlimited instruction to redo every investigation with a specific verification duty. The asking session still owns the recommendation.

Leave the existing dossier summaries unchanged. Put the detailed contract below in the preparation record, with any new source material in a pinned supplement. Update the brief to require this record for new dossiers too. The old summary remains evidence of what its preparer first recommended; the preparation record states what is ready to ask now. Do not insert newly learned evidence into an old first recommendation.

Use this schema contract in a new `bench/schema/ruling-preparation.schema.json`. Section 3 supplies the remaining preparation fields. Reuse the repository's `{path, sha256}` source-pin pattern.

```text
Pin = {path: nonempty repository-relative string, sha256: 64 hex digits}
Evidence = {
  id: nonempty string,
  source: Pin,
  locator: nonempty string,
  quote: nonempty string,
  origin: nonempty URL or pinned repository revision/path,
  available_at_review: "yes" | "no" | "unknown",
  date_basis: nonempty string
}
Coverage = {
  area: "project-docs" | "dependency-docs" | "owner-status" |
        "pr-purpose" | "code-tests" | "maintainer-history" |
        "user-practice" | "documented-alternative" | "changed-owner",
  status: "checked" | "not-applicable" | "blocked",
  scope: nonempty string,
  records: Pin[],
  finding: nonempty string
}
promise = {
  operation: nonempty string,
  expected_outcome: nonempty string,
  whose_interface: nonempty string,
  answer: "yes" | "no" | "unknown",
  made_by: ["written" | "announced" | "built" | "established" | "practice"],
  supports: Evidence-id[],
  opposes: Evidence-id[],
  coverage: Coverage[],
  conclusion: nonempty string
}
delivery = {
  answer: "yes" | "no" | "not-asked" | "unknown",
  outcome_observed: nonempty string,
  evidence: Evidence-id[]
}
asker_check = {
  checked_evidence: Evidence-id[],
  strongest_counterevidence: Evidence-id[],
  counterevidence_result: nonempty string,
  remaining_uncertainty: string | null
}
```

Require one coverage entry per area. "Checked" needs saved records. For documentation, save the relevant page or pinned source. For searches, save the exact query or command, searched repository/tracker and version or date range, retrieval time, coverage limit, and raw result. A zero-result search is a record too. "Not applicable" needs a concrete reason. "Blocked" is visible evidence debt, not a completed negative search. A dependency-free operation need not invent a dependency search.

`made_by` is an array because a written promise and an established practice may both matter. `answer: no` is a conclusion supported by the search and counterevidence, not another way a promise is made. Unknown has its own state. Missing facts must not be forced into advice to satisfy the format.

Extend `bench/tools/ruling_dossier.py` with `--before FILE` to validate this record. Keep its directory mode for existing briefs. The new mode must check:

- Field types, nonblank values, distinct IDs, valid kinds and resolved references. No silently skipped unknown kinds.
- Source hashes and that quoted excerpts occur at the recorded location. Extract a text excerpt from saved structured responses where necessary, and pin both the response and excerpt.
- All coverage areas, with a saved search result or a reason it could not be searched. Null alone cannot mean "searched and found nothing".
- Supporting evidence for a "yes", and explicit reasoning for a "no". "Unknown" remains unresolved. A settled problem requires Promised yes and Delivered no. A minor defect requires both yes and a stated actual fault. Duplicate grouping is a separate decision, not an eligibility outcome inferred from this table.
- Candidate eligibility has `promise` and `delivery`. Recovery instead pins the family's current definition and the original comment, with its own mechanism/consequence assessment. It must not reopen eligibility merely to fill these fields.
- `asker_check` and the pinned question are present. Changed evidence, rule, family definition or question requires a new preparation version and new assessments where the change matters.

On success, `--before` prints the pinned question for the asking session to use; on failure, it prints diagnostics and exits nonzero without printing a ready question. This makes checking part of preparing the text to show. It still cannot prevent a session from bypassing the tool.

The check can verify that the question contains the selected excerpts and recorded caveats. The asking session must still judge whether they give a fair account. A file cannot prove what the UI actually displayed; if the owner reports missing context, repost the complete question and save that version.

Add preparation-mode tests to `bench/tools/test_ruling_dossier.py` for the three passing counterexamples above, wrong or missing source hashes, absent coverage, unavailable searches becoming "no", stale preparation pins, and recovery records with changed family definitions. Add one valid negative search and one valid recovery case so the check does not require a fictional promise. Retain directory-mode compatibility tests. These test refusal behavior, not wording quality.

Before the next ruling, prepare a round queue mapping each of the 13 questions to its groups and preparation file. This matters because group count is not ruling count. SeaweedFS N1/N2 and Django pooling N2/N3 already share a proposed question. Check each queued preparation just before it is shown. Existing dossier failures are not permission to overwrite history or silently carry forward their recommendations.

For this batch, the contract refresh has concrete targets:

| Open dossier | Recheck before recommending |
| --- | --- |
| Base UI N1 | General baseline and dirty-state contract, alongside the announcement. Separate this prefill question from S6's reset error. |
| SeaweedFS N1/N2/N3 | PR promise to handle orphan recovery, rather than whether eviction itself is desirable. N3's saved expiry ruling expressly leaves eviction unestablished. Grouping cannot approve that trigger. |
| Django 17914 N1 | General backend-extension contract, not just whether `ensure_role` has a page or a known harmed consumer. The current advice reasoning carries the old harm requirement. |
| Django 17914 N2/N3 | User-facing installation requirement versus the newer test dependency pin. A fresh install working does not establish delivery for every promised setup. |
| Django 16631 N1 | Rotation instructions, deployment scope, and whether the later maintainer statement confirms old intent. It cannot withdraw a promise after the merge. |
| All recovery questions | Current family and receipt scope, original comment wording, and relevant 04/05/08/S1 distinctions. A fresh execution cannot supply words the review never said. |

**Why this is stronger:** the asking session must produce inspectable source evidence and its own check before asking. Old dossiers and past-ruling reviews enter the same path. The evidence can contradict the recommendation in a way another reader can see.

**Cost:** one small supplement and preparation record per actual question, plus bounded source verification. Share evidence between duplicate groups. Reuse valid probes; do not rerun unrelated environments to fill a form.

**Limit:** an agent can still claim a search it did not do, fetch the wrong version, select misleading excerpts, or miss a source entirely. Saved commands, results and source checks make that inspectable, not impossible. The checker must never print "contract verified"; use "preparation complete; interpretation still requires review".

## 2. The written rule for an undocumented use

### What the owner accepted

P8 accepts the bucket structure. P9 accepts the names, and explicitly leaves the rules' wording for a separate decision. R6 accepts the outcome-based meaning of the second question. P10 authorizes writing the lesson down, but says the owner has not read the resulting text. It also calls the referral list the recorder's proposal.

Thus the saved rulings are authority for their cases. Neither the heading "Rules the user set" nor a link to a ruling makes every generalization under it accepted policy. The working text needs that distinction at its top.

### Clause audit

This covers every clause in the second-pass threshold section and every numbered rule in v3. "Supported" means the cited receipts support that substance. It does not mean the owner read the exact prose.

| Threshold clause | Assessment and correction |
| --- | --- |
| Introductory count; guidance approves nothing | Supported by the receipts. Keep. Say that the wording is a proposed synthesis. |
| Two questions, three outcomes, actual fault for a minor defect | Supported by P8, P9 and R6. Keep. Apply after facts and review scope are established, not to every correct comment automatically. |
| Harm belongs to the band; old harm reading is replaced | Supported by R3, R5, R6, S1 and S5. Keep. Explicitly cover the older-faults paragraph too; "nobody worse off" cannot survive there as a second eligibility test. |
| Documented feature used as its documentation allows | S1 supports this application, but the two timing phrases were ambiguous. Do not turn it into universal compatibility with every documented dependency option. State the operation and relevant timing limits. |
| Variation on documented use **or any public-interface call**, with users and no owner objection | R3 and S2 support variations on documented features. They do not establish that publicity plus callers makes every use promised. Remove the freestanding public-call alternative. The five-way `Practice` definition in v3 is even broader and needs the same repair. |
| Owner called private or said not to use it | 02 supports this when it is the governing owner's position for that operation/version. Keep dates and scope. A project's explicit promise of its own adapter behavior needs separate consideration even if the implementation uses dependency internals. |
| Name documented for **a different purpose** | S9 supports the narrower reading-versus-assignment distinction. A documentation example with another purpose is not automatically an exclusion. Retain S9's exact boundary, not the general phrase. |
| Undocumented order with no shown users is never promised | Too broad without an exception for deliberate support or an explicit PR promise. P1 allows deliberate support; R2 found a promise without shown users. Require the absence of these other grounds before practice becomes necessary. |
| How common the use is does not decide it | Supported by R5. Keep. This does not remove the need for shown users when practice is the only asserted basis. |
| Unsupported breakage can be useful advice; assignment-error example | Supported by S9. Keep as an example, not a requirement that every unsupported-use observation has value. |
| "This extends ... to any fault" | Session generalization, not a separately accepted rule in P10. Propose the same contract test for in-scope older faults, supported by 06/07 and P4, while leaving this extension explicitly proposed. |
| Rubric not changed; saved decisions carry the rulings | Correct as workflow status. Keep. The following explanation that older uncovered faults would be "advice" is now incomplete: some are problems under 06/07/R6. Do not predict all their labels without applying the new questions. |

For v3, use `P1` through `P9` below for its Promised rules and `D1` through `D10` for Delivered. These are clause identifiers, not ruling filenames.

| v3 clause | Assessment and correction |
| --- | --- |
| Preface, "clauses the user decided" | Overstates approval of wording. Replace with "proposed synthesis of the saved rulings; not independently validated". |
| Before 1: false/missing/uncheckable; split assertions | Consistent with the existing threshold workflow. Keep its distinction between unsupported after an adequate check and unresolved because a necessary check is unavailable. Splitting is a preparation convention, not a newly approved bucket. |
| Before 2: attribution, worsened, exposed, touched, partly fixed | Supported by P4, 06/07 and R6. Keep. Explain detectability from touched lines; do not include an unrelated old fault merely because it is nearby. |
| Before 3: review-time evidence; later confirmation | Supported by P7 and R4. The exact cutoff remains open. Pin the actual review snapshot; do not silently make merge-time information available to an earlier review. "No confirmation" must include proof from code, not require a later incident. |
| Question definition and exact operation/owner | Supported by P9, 02, R5, S1 and S9. Keep. |
| Five ways: written, announced, built, established, practice | Useful organization, not five separately approved universal sufficiency tests. Written and announced have clear examples. Built needs evidence of deliberate support. Established needs ordinary/documented use or an established safeguard. Practice needs the limits above. Permit more than one basis. |
| "First that applies decides" | Unsafe. It can terminate at a broad clause before examining contrary evidence. Remove. Resolve scope and explicit accepted exceptions before assigning either answer. |
| P1: owner's no; read-only name | 02 and S9 support the concrete cases. "Read-only" is the owner's interpretation: the docs showed reading and did not literally prohibit assignment. Preserve that difference. Use the narrower replacement below. |
| P2: named announcements; general aims; silent intent; late announcements | Supported by S6/S7/S3. Keep at those scopes. An announcement does not automatically override a separately named contract. Conflicting promises require a ruling. |
| P3: invalid input; earlier rejection; all outside-data software must reject/survive **and say why** | P1 and S3 support the first distinction and established rejection. The universal outside-data guarantee is unsupported. A validator may deliberately return a boolean; not every parser promises survival of every bad input. Require the actual validation/error-handling contract. |
| P4: rare/stylistically discouraged; dependency deadline; general contract covers every exposed component | R5 supports the handbook's application to Field; S1 supports the selected backend despite discouragement. "Every component unless its docs state an exception" is stronger. Check that the general contract actually covers this operation and implementation. Escalate competing scopes. |
| P5: broad announcement; narrower docs cannot shrink it | R2 supports the ordinary bare-name source command despite the FAQ's narrower recipe. It does not settle every conflict between a PR and documentation. Keep R2 as a precedent; refer unresolved contradictions. |
| P6: tests, platform options, types, public calls together | R6/07/06/09 support particular compositions and inputs. Mere type acceptance or two public calls is insufficient proof of deliberate support. Otherwise S9's assignable name would qualify. Narrow Built to the behavior the code/test/type is intended to support. |
| P7: established behavior; unannounced avoidable cost | Ordinary/documented reliance fits the cases. R1 says unintentional, potentially preventable degradation is **likely** a problem. V3 makes it categorical and replaces "intended" with "announced". Neither change was separately accepted. Preserve the qualifier and record both intent and announcement. |
| P8: bounded practice; absolute absence-of-users veto | R3/S2 support the positive examples, with S2's dating limit. The final veto conflicts with deliberate support and R2 if applied universally. Use the replacement below. |
| P9: otherwise no; improvement versus outside-supported-use | Accepted kinds in P8. "Otherwise" is allowed only after an adequate check. Missing facts or conflicting rules stay unresolved. |
| Delivered question and D1: promised outcome; harm separate; reachable proof | Supported by R6/R5/S1 and P5. Keep. Code, a run or a report can establish reachability; measured frequency is unnecessary. |
| D2: failures, wrong values, protection, false success, instructions, needed messages, state marks, lost test protection | Core outcome test is supported by 06/07, R2/R6, S1/S2/S3/S5. The testing clause follows the existing testing-calibration distinction; it is not newly tested by these second-pass cases. Keep it tied to a named protection, not any proposed test. |
| D3: partial delivery; milder manifestations in one family | 10, S1 and S8 support the outcomes. Shared cause is evidence for grouping, not authority to widen a saved family. Require a separate grouping/scope ruling; SeaweedFS N3 makes this live. |
| D4: retry/workaround does not deliver the promised route | Supported by R3 and S8. Keep. |
| D5: never delivered before; no-worse belongs to band | Supported by 06/07, R2/R5/R6. Keep. |
| D6: reports of state true and timely | S5/S6 support documented state and display conditions. Keep scoped to the actual promise. |
| D7: unspecified channel delivered somewhere default; promised value | The value half follows the outcome test, with S7 showing the absence of a named alternative. The default-channel rule has no decisive test among these receipts. Keep as a proposed boundary requiring a precedent, not a reason for automatic settlement. |
| D8: workload no longer fits | R1 shows this is sufficient evidence in that case. It does not make a measured capacity failure necessary for every performance problem. Do not add a universal execution requirement or invent a numeric cutoff. |
| D9: documentation rank, specificity, explicit action, then "neither governs = no" | R5/S4 establish the controlled-field exception. They do not establish that general hierarchy or default. Unresolved conflicts go to the owner. Also, D2/D6 precede D9 under "first applies", so the exception may never be reached. Resolve the applicable promise first. |
| D10: extra message, redundant work, equivalent result, broad wording | 09 and S8 support particular minor defects; R5/S4 support the controlled-value case. Extra-message safeguards are sensible proposed limits. Redundant work may instead be a performance problem under R1. Broad wording may be a genuine unkept promise under R2/R5. Do not let the examples cancel the two-question test. |
| Outcomes and closing paragraph | P8/P9 support the table and need for an actual fault. Keep. "Everything promised arrives" needs the controlled-field exception expressed as the governing promise, not an unexplained contradiction. |

### Improvement 2, now: adopt narrower text, then freeze it for the next cases

Rename the threshold heading to "Second-pass decisions and proposed guidance, 2026-10-05". Add this opening:

> The owner accepted the outcome structure and question names. The receipts settle their named cases. The general wording below is proposed guidance until the owner accepts this version. A supporting ruling is not a blind test of wording derived from it. These rules guide preparation; they do not approve a family, band, grouping change, or regrade.

Replace its "Undocumented use" passage with the following text, also used in a new `terms/two-questions.v4.md`. Keep v3 unchanged as the version used by earlier work.

> Ask about the exact operation, its owner, and the versions involved. A documented use does not need an application report to count as promised. Read the general contract and its relevant restrictions as well as the specific example. In S1, the owner accepted injection after importing requests and before the first HTTP request. The competing wording about first use of urllib3 was a real ambiguity; cite the ruling when relying on that interpretation.
>
> An undocumented variation on a documented feature can be promised when users are shown using it and the governing owner has not rejected it. R3 and S2 are examples. Save the programs, the exact operation, and what establishes their dates. S2's examples were undated; retain that limit. Public visibility, type acceptance, and absence of an objection do not by themselves establish a promise.
>
> A private classification or an explicit instruction not to use the operation, applicable before the review cutoff, defeats a claim based only on practice. Ruling 02 establishes that boundary. If the project separately promises the behavior, record the conflict and ask the owner.
>
> S9 treats assigning DEFAULT_CA_BUNDLE_PATH as outside supported use even though programs did it and no maintainer had forbidden assignment. The owner relied on the name's intended use for reading. The fetched documentation showed reading; it did not literally declare assignment forbidden. Do not infer that every undocumented operation conflicts with documented intent. Where that interpretation decides another case, show the evidence and ask the owner.
>
> Without documentation, an explicit promise from the change, deliberate support, established ordinary use, or demonstrated practice, an adequately checked unusual route has no established promise. An unfinished search is unresolved. Rarity does not defeat a promise. When practice is the only proposed basis, finding actual users matters.
>
> A broken use outside the promise may still deserve an observation explaining its consequences. S9's suggestion to reject unsupported assignment is one example. This does not make it an answer-key problem.

In v4, retain the supported core clauses in the audit and make these further replacements. These are the substantive changes; cosmetic rewrites are unnecessary.

| Replace | Exact proposed text |
| --- | --- |
| Five-way introduction and Built/Practice definitions | "A promise may have several sources. Cite each relevant source and the contrary evidence. Built means code, a test or a type deliberately supports the disputed behavior; being executable is not enough. Practice means the bounded use described in the undocumented-use rule." |
| Both ordered-rule introductions | "Consider the relevant clauses together. Identify the applicable promise before judging delivery. A conflict not settled by an accepted rule or applicable ruling remains unresolved and goes to the owner." |
| Before 3 cutoff/confirmation | "Use the pinned review context for what was knowable. Record uncertainty about the cutoff. Later evidence can confirm a defect or earlier intent; it cannot by itself create or withdraw the promise. Code reasoning or a run can establish a failure without a later incident." |
| P3 outside-data sentence | "Invalid input does not remove a promise to reject or handle it. Establish that promise from the interface's contract or a deliberately established safeguard, as in S3." |
| P4 general-contract sentence | "Apply the general interface contract where its scope covers this operation. Check exceptions and contrary documentation. R5 establishes its application to Field's internal state; it does not settle every component boundary." |
| P5 precedence assertion | "A change's stated purpose can promise the ordinary forms of the use it names. R2 includes sourcing the saved completion file by bare name. A narrower example elsewhere is not by itself a restriction. An explicit conflicting restriction requires a ruling." |
| P6 list | "Project tests, types and platform options can establish deliberate support. State which behavior they support in this case. Do not infer arbitrary combinations from public visibility or type acceptance alone." |
| P7 cost and D8 | "The owner said unintended performance degradation that can potentially be mitigated or prevented is likely a problem. R1 is a positive example, including a demonstrated capacity failure. Record intent, announcement, workload and measured effect. This record sets no universal size threshold or requirement that every performance problem exhaust capacity." |
| D3 grouping sentence | "A milder manifestation may belong to an existing family. Record the shared mechanism and proposed scope. Widening the family's approved trigger or deciding recovery remains a separate question for the owner." |
| D7 default-channel sentence | "Where no contract names the channel, assess whether the promised information still arrives. The general rule for alternative default channels is not calibrated by this round; refer a case that depends on it." |
| D9 | "In the controlled-field cases R5 and S4, the application both vetoed the event and stored the value. The owner ruled that the stored value governs. Apply that precedent to matching facts. For another conflict, record both promises and ask which governs; do not invent a general ranking or default to failed delivery." |
| D10 | "A minor defect requires an actual fault while the governing promised outcome arrives. Ruling 09's extra startup error and S8's unseen stale name are examples. An extra message cannot replace needed information, misstate promised status, block an operation, or violate promised output. Assess measurable redundant work under the cost rule. Do not dismiss an unkept promise as merely broad wording." |

Add after the older first-pass rules in `docs/finding-threshold.md`:

> For preparation under the second-pass decisions, the outcome test below supersedes the harm-based eligibility wording in both Older faults and Partly kept promises. Review scope still follows the older-faults rule. Historical receipts and pinned grading rubrics keep their original versions.

Update the brief and asking checklist to point to the accepted v4 if the owner accepts it. If the owner declines it, keep v3 identified as a working draft and flag disputed clauses in each question. Do not describe the remaining cases as already decided by either version.

**Why this is stronger:** it removes conflicting defaults and separates case authority from inferred policy. An assessor must show which promise governs rather than win by choosing the first broad clause.

**Cost:** one reviewable wording change and a version pin for the next batch. Ambiguous cases may take more owner time now.

**Limit:** this text is untested on new decisions. The v2 result of 37/43 against a 39/43 pass mark does not validate v3 or v4. Later changes to the owner's rulings also change the comparison labels. Do not report an in-sample replay as evidence that delegation is safe.

## 3. Cases that stay with the owner

### Make each trigger answerable before the ruling

The current list is useful for a careful reader, but it is not yet an enforceable gate. "Case's shape" is undefined. Clause ancestry is not indexed. Disagreement has no specified dimension. There is no required record of a negative check. Confidence cannot repair those gaps.

| Current trigger | Detection at preparation time | Required record; what the tool can check |
| --- | --- | --- |
| Two rules point different ways | Each assessor states the competing conclusions, including a clause it thinks another clause overrides. The asking session compares them. | Clause IDs, both applications and the unresolved distinction. Tool checks IDs and that a reason exists; it cannot infer semantic conflict reliably. |
| Deciding clause comes from one ruling, or this ruling | Read clause ancestry, not just the nearest citation. Track cases used to write or tune it separately from later supporting cases. | Rule-version pin, clause IDs, `derived_from` case IDs, `tested_on` case IDs, acceptance receipt if any. Tool detects self-use and one-case derivation. Applying the same clause to its source case never becomes a fresh test. |
| No earlier ruling has this shape | Identify the nearest precedent by exact operation, promise basis, owner restriction, delivery failure and decision kind. State the material difference. A matching project name is insufficient. | Precedent pin and `match: same / different / uncertain`, with reason. Empty or uncertain cannot clear the trigger. A tool checks completeness, not the analogy's truth. |
| Assessors disagree or name a gap | Compare each answer per dimension: promise, delivery, outcome, grouping, recovery and band where asked. Ask each assessor explicitly for gaps. | Separate answers and `gaps`, even if empty. Agreement on "problem" does not erase disagreement on the promise. Any named gap goes to the owner. |
| Recommendation changes a saved ruling | Compare target, subject, revision, trigger and scope with the saved receipt. Widening a family also counts. | Prior receipt and exact scope; `change: none / outcome / scope / wording-only`, with reason. Tool catches declared outcome differences; a reader must identify semantic scope changes. SeaweedFS N3 is already an example. |

Add two reasons the list currently misses: necessary evidence is missing or conflicting; and the recorder disagrees with the blind assessors even if they agree with one another. S9 demonstrates the second. Also record reliance on unaccepted rule text. A high-confidence declaration does not clear any of these reasons.

Distinguish "needs more facts" from "facts are established, but the owner must choose the boundary". The former may be resolved by investigation. The latter should not prompt another expensive search for a rule that does not exist.

### Improvement 3, now: one immutable before/after pair per question

Add the following to "Prepare a ruling" and to the dossier brief:

> Record the preparer's first recommendation and the asking session's first recommendation before either is revised. Have the two blind assessors answer from the same pinned facts and rule version, without recommendations, previous outcomes for this case, or each other's answers. Save each answer before revealing the others. Then save the asking session's final pre-answer recommendation. Show every party's pick, confidence and reason to the owner, with any earlier ruling clearly identified.
>
> A case used to write the rule is a replay, even if its old label is hidden. Mark known exposure. Do not call it an independent test of that rule. If new facts arrive during the ruling, save a new preparation and keep the earlier predictions. Never replace a first recommendation with the corrected one.
>
> After the answer, save the exact question, options and verbatim answer. Record a missing owner rationale as missing. Separate the owner's words from the recorder's inferred lesson. A proposed lesson does not become an active rule until the owner accepts that text.

Use `second-pass/rulings/prepared/<id>.before.v1.json` and `<id>.after.v1.json`. The paired JSON files index the existing Markdown receipts; they do not replace them as authority. This is the smallest next piece of issue 59. Do not build a historical backfill, dashboard or automatic confidence threshold yet.

Complete the preparation schema from section 1 with these fields:

```text
before = {
  schema_version: 1, id, target, revision,
  kind: "eligibility" | "recovery" | "grouping" | "impact" | "control",
  subjects: nonempty string[],
  dossier: Pin, original_summary: Pin, supplement: Pin | null,
  facts: Pin, rule: Pin,
  question: Pin,
  promise, delivery, evidence, asker_check,
  predictions: Prediction[],
  rule_applications: [{clause, derived_from: case-id[], tested_on: case-id[],
                       acceptance: Pin | null, reasoning}],
  nearest_precedent: {receipt: Pin | null, match, reason},
  prior_ruling: {receipt: Pin, scope, change, reason} | null,
  referrals: {conflict, single_source, self_derived, no_precedent,
              assessor_disagreement, assessor_gap, changes_saved_ruling,
              evidence_gap, recorder_disagreement, unaccepted_rule},
  prepared_at: UTC timestamp
}
Prediction = {
  role: "preparer-first" | "asker-first" | "blind-1" | "blind-2" |
        "asker-final-before-answer",
  model, model_family, effort, input: Pin, output: Pin,
  answers: [{dimension, subject, choice,
             confidence: "high" | "medium" | "low" | "unrecorded",
             reason, gaps: string[]}],
  recorded_at: UTC timestamp | null,
  provenance: "contemporaneous" | "reconstructed",
  exposure: "fresh" | "replay" | "known-answer" | "unknown",
  would_settle_if_authorized: boolean
}
after = {
  schema_version: 1, id, before: Pin, receipt: Pin, receipt_scope,
  owner_answers: [{dimension, subject, choice}],
  owner_reason_quote: string | null,
  comparison: [{prediction_role, dimension, subject,
                result: "match" | "miss" | "unresolved" | "not-comparable"}],
  surprise: {present: boolean,
             cause: "new-fact" | "missed-source" | "rule-application" |
                    "policy-boundary" | "mixed" | "unknown",
             explanation: string},
  proposed_lesson: string | null,
  supersedes: Pin | null,
  recorded_at: UTC timestamp
}
```

Strings above are nonblank unless expressly nullable. Use the existing revision object for base/head and task-packet identity. `promise` and `delivery` are null for questions that do not assess eligibility; their facts and decision dimensions are still required. Each referral is `{state: "yes" | "no" | "unknown", reason: string, evidence: Pin[]}`. No missing keys and no default "no". An unknown trigger requires referral, not automatic rejection of the claim. An unavailable independent assessment is recorded as missing and also prevents clearing the preparation for delegation.

Confidence has this limited meaning for now: high means the assessor would settle that dimension on the supplied facts and rule if authorized; medium means it favors the answer but sees a material doubt; low means it cannot choose reliably. Confidence describes a prediction, not permission. Preserve earlier confidence verbatim and mark absent confidence `unrecorded`; never infer it from decisive prose. Record confidence per dimension so confidence in a reproduction cannot stand in for confidence in eligibility or a band.

Extend `ruling_dossier.py` with `--after FILE`. Check the before pin, receipt scope, answer dimensions and mechanically computed comparisons. Require immutable new versions when facts or rulings change. Test self-derived clauses, a missing trigger answer, assessor unanimity with recorder dissent, an after-record whose before-file changed, and preservation of a missed first answer after a revised answer matches. Test that unknown or reconstructed timing does not enter the fresh-prediction sample.

For blindness, export facts rather than sending the whole dossier with its Recommendation heading removed. The current dossiers include recommendations, saved outcomes and family judgments in other sections. Pin the exact exported input. Keep rule ancestry and prior-answer fields outside the blind packet. If an assessor has already seen the answer, record that exposure. Fresh eligibility cases can still have rules derived from related cases; record that ancestry too.

The two assessors required by the checklist may both be from the same other model family. That does not on its own satisfy ADR-0006's independent agreement across different families. Record the actual families and which outputs were independent. Do not equate two sessions with two families.

Replace the checklist's automatic "add the clause to the rule" sentence with:

> When the owner chooses against a recorded prediction, preserve the prediction and classify the difference. Record new facts and the owner's stated reason, if any. Propose a rule change separately. Until the owner accepts it, the lesson is an observation. A newly accepted clause remains untested on independent cases; acceptance and predictive evidence are different facts.

A surprise is not limited to "both assessors were wrong". Record each party's miss and disagreements about reasons. In S9, the blind labels matched but the recorder's recommendation did not; the blind labels also came from case-derived wording. In S6, all six blind runs opposed the saved ruling. Both are valuable evidence, for different reasons.

After the 13 questions, report simple counts by decision kind, rule version, confidence and referral status. Count the first recommendation against the latest authoritative answer; retain the original answer and reversals. Report abstentions, missing confidence and new-fact cases separately without deleting them from the history. Keep two views for re-asked cases: first decision with first evidence, and revised decision with revised evidence. Multiple assessors, related variants and repeated asks are not independent extra cases.

Use these records to distinguish confident mistakes from well-calibrated uncertainty. Do not turn 13 mixed questions, many involving grouping or recovery, into a claimed eligibility accuracy rate. Rules changed during the batch get a new version; cases used to change them are no longer unseen tests of that version.

**Why this is stronger:** the record fixes the prediction and input before the answer is known. It makes abstention, disagreement and surprise visible. It prevents a revised correct recommendation from erasing an earlier confident miss.

**Cost:** small sidecars, a facts export and the blind assessments the checklist already requires. The first record may be slower to prepare. This proposal does not authorize or run any model calls.

**Limit:** timestamps and hashes do not authenticate a person's answer or prove that an assessor was blind. They preserve inspectable artifacts. Owner decisions may also change, so every reported agreement rate needs a named answer version and a denominator.

### Improvement 4, wait: connect referral reasons to authority

ADR-0002 reserves new and disputed decisions to the owner. ADR-0006 permits eligibility-only delegation with independent agreement across two model families, a saved base/head reproduction, and a fetched explicit maintainer acknowledgement. Confidence alone grants nothing. Bands, grouping, controls and boundary wording remain with the owner; recovery is also outside that delegation.

The new checklist says "never settle" these cases, but P10 identifies that as a proposal and ADR-0006 has not adopted the exclusions. Ask for explicit adoption of the restriction, not an implied expansion of authority. Proposed policy text for an addendum to ADR-0006:

> Delegation policy v1's evidence conditions remain necessary. The accepted referral check is an additional restriction: every referral state must be "no" before an eligibility decision may be settled under delegation. A "yes", "unknown", or missing assessment sends the question to the owner. This grants no authority for another decision kind. Changes to these restrictions require an owner-adopted policy version.

Before future delegation relies on this, extend `bench/schema/current-adjudication.schema.json` with an explicit decision route and policy/preparation pins. Validate it in `bench/tools/current_grading.py:validate_documents`:

```text
decision_route = "owner-ruling" | "delegated-eligibility"
policy = Pin | null
preparation = Pin

if decision_route == "delegated-eligibility":
    require dimension == "eligibility"
    require adopted policy and its receipt scope
    require all referral states == "no"
    require ADR-0006's three evidence conditions
else:
    require an applicable saved owner ruling and its receipt scope
```

Apply this through a versioned schema change for new decisions, with an explicit legacy path for existing records. Do not rewrite old authority as if the item had a personal ruling. `authority: human` remains the repository's convention for both direct and delegated decisions; `decision_route` explains which it was.

The current validator verifies saved receipt provenance and scope membership, but has no structured route or referral data with which to enforce this distinction. A reason string saying "delegated" cannot be a reliable machine gate. Add rejection tests before using the new route: wrong decision kind, any referral or unknown, missing policy acceptance, same-family-only agreement, missing reproduction and missing exact acknowledgement.

**Why this is stronger:** a future writer of an approved decision cannot clear delegation merely by asserting confidence and quoting the delegation receipt. It must provide the checked evidence and route.

**Cost:** an accepted policy addendum, a schema version and integration tests. This touches the authority boundary and deserves a separate change. Wait while the owner personally rules on the current 13; implement before using the exclusions to authorize any future automated settlement.

**Limit:** code can enforce declared evidence and declared referrals, not discover every hidden conflict. "Single-source" must not become a permanent ban based only on how a clause was born. After genuinely new cases, the owner can adopt a tested general rule and explicitly revise that restriction. Otherwise this mechanism would frustrate issue 59's goal by sending familiar cases back forever.

### Strongest objection

This can become elaborate paperwork that rewards a convincing evidence trail. The same session may select the sources, write the rule, check the analogy and claim that no referral applies. A hash does not fix that. The strongest defense is to keep the next step small: use one preparation/answer pair for each of these 13 owner decisions, reopen the decisive sources, and inspect whether it catches omissions the current brief missed. If the fields add work without exposing mistakes or useful uncertainty, remove them before extending the process. Wider delegation should depend on that record, not on how complete the forms look.
