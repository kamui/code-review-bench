# Verdict: the three mitigations

Judged 2026-10-05 against `rubric.md`. I read both proposals and rationales, the files the task lists, the rulings each claim names, and candidate 1's `prototype/` and `scratch/` files. I ran the repository's check on all nine candidate directories, candidate 1's probe and candidate 1's prototype tests. I ran no model and changed nothing in the repository.

**Base: candidate 1, 27 points to 21.**

## Scores

| Criterion | Candidate 1 | Candidate 2 |
| --- | --- | --- |
| 1. Finds real gaps | **5.** It wrote each of the five failures up as it stood and ran the current check on them; I reran the probe and all pass, and its S2 detail is exact (`search-keylog.json` and `search-late-keylog.json` are issue searches with 3 and 4 hits). | **4.** Its three probes are true of the code (a boolean `source`, a string `searched` and `made_by: none` with `recommendation: eligible` all pass) and its count of 15 candidate entries is right, but its walk through the five says less about which piece would catch each. |
| 2. The mitigation binds | **4.** A working check refuses a search with no query or no saved response and a recommendation that does not follow from its promise, and the record refuses a failing dossier; but any place passes with `{"none": "x"}` and nothing fails if the record is never written. | **3.** The gate sits in the right place (the question, including reviews and recovery, with "unknown" as its own state), but all of it is an unbuilt specification and the existing directory check is left as weak as it is. |
| 3. The rule is faithful | **4.** It sorts each clause into shown, inferred or unsupported, catches that every ground in ruling 2 held together, and writes almost nothing new; it passes the outside-data clause and the merge cut-off under "citations as given". | **3.** It catches more clauses (the cut-off, outside data, "built"), but keeps ruling 2 as "private or told not to", removes the rule order on its own authority, and writes a sentence that supersedes two rules the owner set. |
| 4. Escalation is operable | **4.** A tool computes the reasons and the surprises from recorded answers, and the clause table catches the S9 circle; it has no trigger for the kind of decision, and today every clause fires as untested. | **4.** Its record is better for later calibration (confidence per dimension, fresh or replay, no default "no", an after-file that pins the before-file), but each trigger is declared by the session and the "smallest piece" has about forty fields. |
| 5. Concrete and cheap | **5.** Exact replacement text, 14 passing prototype tests, line counts, minutes per ruling, nine improvements ordered with now or wait. | **3.** Exact rule text and a useful table of what to recheck in each open dossier, but no code, no sizes, and a "now" list of two schemas, two tool modes, a facts export and a queue. |
| 6. Honest | **5.** Its objection names a cheaper rival (stop recommending, show the blind answers), the evidence for it, and a one-change fallback; it says it did not reread the 44 first-round files. | **4.** Limits are stated for every improvement and it says nothing was built or tested; it does not mark its own rule changes as its own. |
| **Total** | **27** | **21** |

## Factual errors

**Candidate 1**

- "Eight of the nine open directories fail it." Nine directories exist and eight fail, but four are already ruled (requests, tRPC, ripgrep, Hono; `DISCUSSION.md`, "What decided each ruling"). Five hold open questions. Its other counts are right: 8 open candidates, 3 recommending `duplicate`, 10 recovery questions of 18 open groups.
- The proposal says the new check refuses "a place missing". The prototype accepts `{"none": "<any text>"}` for every one of the six places (`prototype/ruling_dossier.py:26`). "Null means did not look" comes back as "none".
- The proposal says the record "refuses a candidate record whose dossier fails". The prototype skips that test when `reconstructed` is true or the kind is not `candidate` (`prototype/ruling_record.py:25`).
- Improvement 6 puts the delegation refusal in `calibration.py check`. That function checks impact, scope, controls and the audit (`bench/tools/calibration.py:270`). Nothing under `bench/tools` mentions delegation; ADR-0006 records a delegated decision only in its `reason` prose.
- "Everything else: supported by the citations as given" covers two clauses the files do not support. The outside-data sentence rests on two family ids, not rulings (its own table, `P3c`), and `docs/finding-threshold.md:28` calls GT-u5 "the doubtful one" that "was not ruled again". "Before the merge" is not settled: `finding-threshold.md:36` says "the exact cut-off is open" and `SYNTHESIS.md` item 5 says the same.
- Its example record names rulings two ways (`nearest: ["S2", "R3"]` against `"first-round 11"`, `"second-pass 03"` in the clause table). The circle check compares strings.

**Candidate 2**

- "The original N1 dossier already quoted both timing phrases." It quotes the first and paraphrases the second (`candidates/i-requests-6667/dossiers/N1.md:31`). The substance holds and matters: `DISCUSSION.md` says none of these facts "was in the dossier", and for S1 that is wrong.
- "D2/D6 precede D9 under 'first applies', so the exception may never be reached." "The first that applies decides" stands over the Promised rules only (`two-questions.v3.md:27`); Delivered says "Rules, in order" (line 45). Version 2 had the same order, and all four blind runs found the controlled case of ruling 21 as a minor defect (`SYNTHESIS.md:43`).
- "Owner called private or said not to use it: 02 supports this." Ruling 2 had private, removed and "Please do not do it this way" together, and the owner chose the recommended option without a ground (`rulings/02-requests-N2a.md`). Its replacement text keeps "or".
- "How common the use is does not decide it: supported by R5." That sentence in `R5` is the recorder's. The owner asked whether the use was common or documented. Its own caveat lands where candidate 1 does.

Checked and true, both: all 15 candidate entries lack `promise`; `rule_gap` is absent everywhere; the heading "Rules the user set" contradicts P10 and P9; S2's two files are "undated and unrun"; the cost clause drops the owner's "likely"; the ranks in Delivered 9 have one ruled case.

## The base

Candidate 1. The rubric asks for mitigations that bind, and only candidate 1 delivers something that can be adopted this afternoon: a replacement check and a ruling record that run, with tests, sized in lines and minutes. Its audit of the rule is the more faithful one because its corrected text says less than version 3, not different things. Candidate 2 sees further in places (unknown as a state, recovery and grouping, replay against fresh) but asks the owner to accept a large schema that nobody has built before 13 rulings that are the only out-of-sample test available.

## Take from candidate 2

- **An "unknown" state.** "Do not turn missing evidence into 'Promised: no'." Replace the prototype's `none` escape with `not-applicable` (with a reason) and `blocked`; a blocked place is a reason the ruling stays with the owner.
- **The kind of decision as a trigger.** Recovery, grouping, bands and controls are never delegable (ADR-0006). Candidate 1's tool prints "no recorded reason" for a recovery question with agreeing assessors.
- **Do not exempt `duplicate`.** A duplicate that widens a family needs its promise. SeaweedFS N3 says so itself (`dossiers/N3.md:89`), and S1 was exactly this.
- **Leave old summaries alone.** Put the refreshed promise in a supplement, as ruling 2 and the reviews already did, so the preparer's first recommendation survives for the record.
- **The after-file pins the before-file's hash.** One field; it replaces "trust the commit order".
- **`exposure` (fresh, replay, known answer) and `derived_from` apart from `tested_on`.** Reconstructed records stay out of the fresh sample.
- **Export facts for the blind assessors.** The dossiers carry recommendations and saved outcomes outside the Recommendation heading.
- **Record the real model families.** Two assessors from one other family do not meet ADR-0006's condition 1.
- **The recheck table for the open dossiers and the queue.** Group count is not ruling count: SeaweedFS N1/N2 and Django N2/N3 share a question. Django 17914 N1's advice does rest on "no harmed consumer" (`dossiers/N1.md:64`, `:80`).
- **Three audit findings:** the open cut-off in Before 3; the outside-data sentence; "built" needing more than a public name.
- **Replace `claim-adjudication.md:84`.** "Add the clause to the rule" lets a session write policy; `DISCUSSION.md:3` says a pattern becomes a rule only when the owner accepts it. Pair this with candidate 1's improvement 5.
- **Success message.** "Preparation complete; interpretation still requires review", not "every candidate dossier states its promise".
- **For the waiting item:** the ADR-0006 addendum text, and enforcement through a `decision_route` in the adjudication schema, not a reason string.

## Where they disagree

| Question | Candidate 1 | Candidate 2 | The files support |
| --- | --- | --- | --- |
| Old dossiers | Re-prepare step 4 in place | Leave them; add a preparation record | **2.** The existing pattern is a supplement beside an untouched dossier, and issue 59 measures the first recommendation. |
| `duplicate`, `unproven` | Need no promise | Grouping is a separate decision | **2.** N3's dossier and S1. The exemption is also chosen by the agent it is meant to check. |
| The rule order | Keep "the first that applies decides" | Remove it; weigh clauses together | **1.** The ordered rule is the one that was tested, and two models agreed on 40 and 41 of 43 under it. No ruling asks for the change. Record conflicts in the answer and send them up. |
| Ruling 2's clause | All grounds held together; one alone is not ruled | Private or told not to | **1.** The ruling file. |
| The harm-based rules | Flag the contradiction, ask the owner | Write "supersedes" for two of the three | **1.** P9: the wording "is put to the user separately". R6 accepted the meaning "in substance". Candidate 2 also leaves out unusual input. |
| Inherited clauses | Taken as cited | Outside data, cut-off, "built", default channel flagged | **2** on the first two (`finding-threshold.md:28`, `:36`). I did not check the other two against the first-round files. |
| How triggers are found | Computed from recorded answers | Declared yes, no or unknown with a reason | **1** for the four that can be computed; **2** for "no default no" on the two that cannot. |
| The record | One file, `decision` added later | Immutable before and after | **2.** Cheap, and the tool can then check what candidate 1 leaves to the commit log. |
| Where delegation is enforced | `calibration.py check` | Adjudication schema and `current_grading.py` | **2.** See the errors above. |
| Size of the search record | Six places, counts, a saved file | Nine areas, hashes, excerpt at a location | **1.** Proportionate and built. Excerpt checking is not worth its cost before the 13. |

## The improvements: when, and who decides

| Improvement | Who proposes it | Before the 13? | Owner's decision? |
| --- | --- | --- | --- |
| Rename "Rules the user set"; mark the wording as the session's | Both | Yes | No. It removes a claim P10 contradicts. |
| Corrected rule text as version 4, read once | Both | Yes | **Yes.** This is the reading P9 promised. |
| Freeze the rule version for the 13; no clause from one applied to another | Both | Yes | No |
| The check shows the search: query, saved response, counts; null is not "searched" | Both | Yes | No |
| The recommendation must follow from promised and delivered | Both | Yes | No |
| "Unknown" and "blocked" as states; no free `none` | 2 | Yes | No |
| The same check gates the asking session and reviews of saved rulings | Both | Yes | No |
| Refresh the promise for the five open candidates that are not duplicates, and for the duplicates that widen a family | Both | Yes | No; it costs preparation runs, so say how many. |
| A machine-readable record of every party's first answer, written before asking | Both | Yes. The 13 are the only cases the rule was not written from. | No. It is an index; the ruling files stay the authority. |
| After-file pins the before-file | 2 | Yes | No |
| Clause table: which rulings each clause was written from | Both | Yes | No |
| Two more triggers: recommender against the blind assessors; evidence incomplete | Both | Yes | Show the owner the whole list with the rule; it is still the recorder's proposal (P10). Adding triggers only sends more up, so use them meanwhile. |
| Kind of decision as a trigger | 2 | Yes | No. ADR-0006 already says it. |
| State the rule sentence in the option; stop adding clauses on the session's say | Both, differently | Yes | No. It applies `DISCUSSION.md:3`. |
| Cost clause back to "likely"; drop the ranks in Delivered 9 | Both | Yes, inside version 4 | **Yes**, with version 4. `SYNTHESIS.md` item 4 already says the cost line is the owner's. |
| The three harm-based rules | Both, differently | Ask before; it bears on Django 17914 N1 | **Yes** |
| Update the "same family" pattern in `DISCUSSION.md` | 1 | Either | No |
| The limit in ADR-0006 and its enforcement | Both | No. Nothing among the 13 is delegated. | **Yes.** A policy version. |
| Walk the real dossier directories in the bench tests | 1 | After the refresh | No |
| Backfill and the summary command | Both | No | No |

## What both miss

- **Nothing fails when the record is not written.** Each tool checks a record that exists. Add one test: every ruling file from the 11th on has a record. `tools/run_bench_tests.py` picks up any `test_*.py` and CI runs it (`pages.yml`), so a skipped record turns the build red afterwards. That is the cheapest structural hold on the asking session, and neither proposes it.
- **A precedent decided under the old reading.** "The recommendation would change a saved ruling" does not fire when the recommendation follows a saved advice ruling made on "nobody is worse off" and never shown again. Of the sixteen saved rulings that were shown again, eleven changed. Django 17914 N1 is this case today. It needs its own trigger.
- **One name per ruling.** S9 is also first-round 30 and R2a. Both designs test circularity by comparing ids; neither fixes the ids or gives an alias table.
- **The diagnosis on file is wrong for S1.** `DISCUSSION.md` ("none was in the dossier") and `claim-adjudication.md:72` (a deadline "fetched during a later review") say the fact was missing. The dossier had it. Both candidates notice; neither proposes correcting the two sentences every later agent will read. The corrected sentence matters: two of the five failures were misreadings, which no search check reaches.
- **The lean.** The record names the direction of the error: recommendations leaned to advice. Neither record or summary reports direction, only match or miss.
- **The cost of the blind assessors.** Both require two per question and neither counts the runs. Ten of the 18 open groups are recovery questions, for which there is no written rule to apply, only the patterns in `DISCUSSION.md`.
