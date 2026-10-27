# M1a: first-pass orphan cleanup costs two extra serial commands

Base `db5a086d048c5c2d6e51e82bb070d20df04d688d`; head `6c8fde6642cd428e884cc9c9d88c9aaa189c4bf1`; last push **2026-08-13T14:12:47Z**. Every pinned source fact and behaviour below was knowable at that cut-off. Experiments were performed for this packet now. **Run** means executed here, **read** means source or saved public record, and **reported** means another party's assertion. Later public facts are explicitly marked later. The neutral facts and exact source excerpts are in `../cases/M1a.md`.

**Already decided? No exact ruling found.** The packet's complete reference-family and pre-existing-problem statements, ruled claims and saved-ruling index were checked. The saved decisions concern lost names, failed compensation, the cleanup gap, child indexes, pagination, replica settings and concurrent deletes. None names this exact trigger, mechanism and consequence. `already_decided_by` is null. `grader_misread` is null: no present record falsely attributes an outcome to a saved ruling. The packet's control is `known-problems`, so this question touches no audited-clean control.

> **What happens:** a first listing of genuinely dead names performs GET, ZREM and EXISTS serially for each name, then later listings avoid that work. **Promised: cannot tell**, because the cost is demonstrated but Promised 7b leaves its status as an intended cleanup tradeoff or an avoidable performance regression to the owner. **Delivered: not established**, for that cost obligation; reclamation itself is delivered. **So: unproven.** **Band, decided separately:** none proposed until the performance question is decided.

The unresolved form is deliberate: assigning yes or no to the first question would conceal the precise missing judgment. This is a recommendation for the owner's ruling, not a decision.

## Problem

The group describes first-pass latency of cleanup in different words. Its command-count core is one thing. **Reported**: some text estimates three or four waits and possible deadline failures. **Run**: genuinely absent values incur three per-name commands, not four; ZADD is conditional on a value reappearing or an existence-check error. Deadline failures were not reproduced.

M1 is split into **M1a** (latency) and **M1b** (the separate assertion about V(0) log noise in comment-faad2cb0). Different source lines cause them, and changing logging need not change command ordering. All five candidate ids remain mapped to M1a; M1b records its source comment separately.

## What changed

At head line 208 the listing calls the new helper. Lines 238 and 245 synchronously remove a name and recheck its value. Line 250 can restore it. Complete diff and exact cut-off body: `upstream/change.diff`, `upstream/description-at-cutoff.txt` (**read**).

The task description at the cut-off says:

> `LIST` costs one `GET` round trip per name *ever created* in that directory, not per live entry;

And describes the remedy:

> One line: `ZREM` the member in the not-found branch, mirroring what the expiry branch already does four lines below, against the same `dirListKey` variable.

That earlier-description promise is still considered as what the change says. It does not state a maximum first-pass cost or prohibit existence checks. The head's comment explains why recreation needs a recheck. Description edits 81 seconds and 153 seconds after the last push are later, and are not used to alter the promise.

**Run**, five alternating runs for each workload, real store and Redis: 64 names with no artificial delay take median **8.829 → 21.661ms**. With an artificial 1ms sleep after each command, 1024 names take **1289.561 → 3844.139ms**. This is a controlled command-wait illustration, not a production RTT benchmark. No first-call error occurred; all dead names were removed at head. The second call needs only the range at head and repeats N GETs at base. Counts, ranges, correctness checks, limiter arithmetic and limitations are saved in `probes/M1a/measurement.md`; all outputs and samples are retained.

## Promised?

SeaweedFS owns this public directory-listing operation. Redis owns command semantics, not a SeaweedFS latency guarantee. The general wiki offers efficient listing and calls its sorted set efficient. It states no first-repair deadline. The code deliberately performs a compensating check to preserve concurrent recreations; that is evidence for an intended cost, not proof that every sequential wait is an intended tradeoff.

The cut-off body quotes the original recurring one-GET cost and promises reclamation. It delivers reclamation. The public code search does not establish a production backlog or a latency contract. #4508 already proposed pipelining in 2023; the maintainer asked about an expired first batch rather than adopting a timing requirement. **Read**, exact record saved under upstream/pr-4508*.

| Place | Check and saved evidence |
| --- | --- |
| project-docs | 29 of 435 discovery hits read; `upstream/M1a-project-docs.json`. Pinned general listing/Redis pages describe efficient listing, not a first-cleanup bound; source states why recreation is checked. Other matching lines belong to unrelated component/docs; selected complete general listing, storage, TTL and event pages plus README were read. Discovery matches are lines, not deployments. |
| owner-docs | not-applicable: SeaweedFS owns the exact listing/event/index operation. No dependency promises its notification policy, directory reclamation, latency bound or log level; Redis command semantics were independently run. |
| change | 3 of 3 discovery hits read; `upstream/description-at-cutoff.txt`. Description promises removal of redis2 dead names and explains repeated GET/log costs. Source and added tests preserve cleanup and concurrent recreation; no explicit first-pass latency or silence bound.  |
| maintainers | 1 of 10 discovery hits read; `upstream/M1a-maintainers-tight.json`. Read #4508 and its maintainer reply, plus the focal PR; other titles concern dependencies, installation or other operations. No first-cleanup latency bound found in those records. Selected directly relevant records were read; remaining hits were not read in full. Day-granularity hits after last push are later. Broad preliminary responses and exact additional fetches are retained. |
| public-code | 2 of 9 discovery hits read; `upstream/M1a-code-call.json`. Read current official implementation (later) and logweed fork dated 2022-11-15; implementation copies do not show an affected first-cleanup deployment. Remaining hits were not read; forks, implementation copies and unrelated configs cannot establish prevalence. No benchmark-repository code result was read. Initial rate-limit failures retained; refined queries succeeded. |
| documented-way | 3 of 3 discovery hits read; `probes/M1a/result-head.txt`. TTL metadata expiry, listing and live-file control execute; head reclaims names but first pass adds two commands. Full HTTP path not run.  |

Exact queries are in summary.json and saved *.command.txt files, with raw responses. Counts are not prevalence. For project-docs a hit is a matching line; selected relevant sections of general pages were read. Maintainer discovery searches are intentionally broader than the records read. Unread material is disclosed, and silence in selected hits is not represented as a comprehensive absence of a promise. Initial public-code rate-limit failures were retained; successful retries and refined searches followed. Public programs can show use, not create a promise.

Promised 7b is the applicable cost provision. Whether this first-pass cost is outside the intended tradeoff remains unresolved. Promised 8 cannot be used to say no merely because no fixed numeric bound was found.

## Delivered?

Index reclamation is delivered in every genuine-absence run. If the owner treats avoidable first-pass latency as part of the promise, the measured extra waits bear on Delivered 2. No production deadline, full HTTP latency or representative deployment was run. A fixed threefold production slowdown is not established.

## Intended or announced

The helper's recheck is built deliberately and explained by the added comment. The cut-off description gives a one-command earlier design, not an announcement accepting all first-pass costs. No later wording is used to announce a change retroactively. The change is included in release 4.42 with follow-ups; the exact pinned helper was superseded before release. First release of this precise cost shape was not independently established, so summary.shipped is unknown.

## What the affected person sees

**Run**: a slower successful first store listing, no dead entries returned, no error; later calls do less work. Users of remote Redis can incur accumulated serial latency (**read**, synchronous code), but their real RTTs, deadlines and deployment frequency are unknown. Nothing proves that every listing remains slower or that a caller is stuck.

## What the maintainers did

**Read, before cut-off**: #4508's maintainer question about the first batch being expired. No maintainer commented on #10735 before the last push. **Read, later**: #12042, opened 2026-10-24, pipelined the GET path while keeping orphan cleanup. Its reported LAN benchmark concerns live entries, not this allegation. It confirms interest in listing latency but decides neither question here.

## How each fact is known

- **Run:** source-commit binaries, expiry, command counts, output checks, index state, live control and repeated timings; `probes/M1a/`.
- **Run, surrogate only:** forced stale single-command GET/EXISTS with a real standalone live value; no real replica is used.
- **Read:** task description, source diff, added comment, pinned wiki, dated maintainer reply and later follow-up.
- **Reported and not established:** client deadline failures, production prevalence and universal wall-time multipliers.

## Relation to existing reference families and ruled claims

GT-s1 is the nearest shared-source comparison: authoritative existence versus latency at the same helper. Four patches were run, with all output retained. The GT-s1 EVAL check keeps three serial commands and the first-pass cost. Pipelining ZREM/EXISTS reduces independent cleanup waits for genuine absence. The stale-reply surrogate retains the name under pipelining because pipeline commands bypass its injection hook; that is **not** evidence of actual replica routing under the patch. A real lagging cluster was not run. Thus against GT-s1: **same_lines yes; one_fix cannot-tell; one_sentence no**. One cause is stale non-authoritative absence, the other additional synchronous waits. This limitation prevents a grouping recommendation from those surrogate results alone.

GT-s2 needs a pending update; GT-s3 a failed compensation; GT-s4 another reader during the gap; GT-s5 a missing directory's surviving child index. No such prerequisite occurs in this genuine-absence run. PE-s1 is about wrong pagination, not the time taken; PE-s2/3/5 concurrent mutation; PE-s4 option naming; PE-s6 directory child-index handling. Their full tests were not rerun for latency and no claim is made that the diagnostic pipeline fixes them. The saved grouping rulings first-12, last-push-1/P26 and regrade-v3-4 do not decide a latency symptom.

## Both sides

For a problem: the new per-name waits are measured; TTL backlogs are the motivating workload; a two-command pipeline demonstrably reduces independent waits on standalone Redis. Promised 7b allows a performance cost without a deadline failure.

Against: cleanup adds durable reclamation that the old version never did, and the recheck explicitly protects recreations. The cost is paid once per member; repeated work vanishes. A transport-safe complete alternative and representative user impact are not established. A modest cleanup tradeoff can be an improvement suggestion.

## Recommendation

**Unproven**, low confidence. `short_of_high`: no-single-rule, no-precedent, flip-fact-open. The owner must decide the Promised 7b tradeoff before this becomes a problem or suggestion. The rule expressly reserves the intermediate-cost boundary; that reservation is not an invented gap in the rule. No impact card is required for an unproven recommendation. The fact that would reverse this is an established supported workload and a cut-off statement or owner determination that these extra serial waits are outside the accepted repair tradeoff. The measured cost alone does not settle that judgment.

Strongest argument against: the avoidable cost is already noticeable in the run and the description identifies listing cost as motivation, so the owner may find the current evidence enough to propose a problem.
