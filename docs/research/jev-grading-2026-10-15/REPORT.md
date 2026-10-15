# Jev grading experiment — 2026-10-15

**Do not use Jev to automatically award credit or to decide which cases can skip
investigation yet.** Twelve input formulations did not produce a reliable match
to the saved human credit rulings. Cause matching is a possible narrower research
direction, but it also remains unqualified. No live grades were changed.

## Held-out results

These are 14 selected historical cases from four tasks, held out from this
experiment's input development. They are not a representative production sample.
Four cause labels were unruled and are excluded from that axis only.

| Grader / frozen formulation | Credit ruling agreement | Cause ruling agreement |
| --- | ---: | ---: |
| Jev, original full packet (v1) | 7/14 (50%) | 9/10 (90%) |
| Jev, selected shorter formulation (v10) | 7/14 (50%) | 7/10 (70%) |
| Jev, binary probability formulation (v11) | 7/14 (50%) | 7/10 (70%) |
| Sol 6.1 High, original packet | 12/14 (86%) | 10/10 (100%) |

Credit had only two positive labels. Original Jev detected both but incorrectly
awarded credit on seven negatives. Selected Jev detected one positive and
incorrectly awarded credit on six negatives. Sol detected one positive and
incorrectly awarded credit on one negative. Always answering “no” would also score
12/14 overall; accuracy alone therefore does not establish a useful grader.
Sol is a comparison model, not the source of truth. Saved human rulings control.

## Can confidence safely decide escalation?

No. The cutoff was frozen before opening held-out responses: a Choice answer
needed confidence at least 0.95; a binary Noul answer needed probability at least
0.95 or at most 0.05.

| Jev formulation | Credit decisions cleared in shadow | Incorrect among cleared | Cause decisions cleared |
| --- | ---: | ---: | ---: |
| Original v1 | 7/14 | 6/7 | 1/10, correct |
| Selected v10 | 4/14 | 3/4 | 0/10 |
| Binary v11 | 0/14 | Not applicable | 0/10 |

The selected formulation would clear 29% of credit judgments, with three wrong
automatic decisions. The binary version avoids these errors by escalating every
held-out judgment at the frozen cutoff. Neither supports the proposed safety and
utility gates. A model's confidence is not permission to bypass evidence checks.

These are **incorrect packet judgments**, not measured missed source
investigations. This experiment did not label whether each case needed a trace or
reproduction. It cannot estimate that missed-investigation rate. Both accepted
and escalated packets received independent, blinded Sol judgments and were
checked against the human labels. The conditional full source-investigation trial
was not started because bounded classification did not justify automatic routing.

## Input experiments

Six development cases from three whole tasks were used for tuning. No formulation
exceeded 4/6 credit agreement. All six development cause labels were positive,
which made apparent success on that axis weak evidence.

| Version | Change | Development credit | Development cause |
| --- | --- | ---: | ---: |
| v1 | Full normalized comment, reference, rubric | 3/6 | 6/6 |
| v2 | Stronger scope/effect instructions | Incomplete | Incomplete |
| v3 | Separate scope and user-effect questions | 3/6 | 6/6 |
| v4 | Question-specific reference and rubric | 3/6 | 5/6 |
| v5 | Unrelated illustrative examples | 3/6 | 5/6 |
| v6 | Short target description | 4/6 | 4/6 |
| v7 | Human examples from other development tasks | 3/6 | 6/6 |
| v8 | Those examples plus explicit boundary distinctions | 3/6 | 6/6 |
| v9 | Reordered v1 choices | 3/6 | 6/6 |
| v10 | Reordered v6 choices | 4/6 | 6/6 |
| v11 | Binary probability version of v6 | 4/6 | 5/6 |
| v12 | Binary probability version of v1 | 3/6 | 6/6 |

Recurring errors confuse describing a changed internal value or shared cause with
stating this specific failure for a user. More examples and stronger wording did
not reliably fix the distinction. Some cause decisions differed between choice-order variants; repeated identical-input
trials were not run, so ordering effects cannot be separated from stochastic
variation. Neither order solved the credit errors.

The user authorized more revisions after the initial two. Already-generated
held-out responses stayed unread during the additional tuning. The final
[selection and routing freeze](qualification-freeze.v2.json) preceded evaluation.
Held-out v3 and v6 diagnostics also scored 7/14 credit; they were not used to
retune. All versions and incomplete attempts remain available.

## Cost and operational evidence

- **Jev:** 140 HTTP requests, 139 accepted responses and one parser-rejected
  response. Validated input usage: 325,774 tokens. At the pinned rate, recorded
  cost is **$0.013682508**, plus **$0.005505024** conservatively reserved for the
  rejected response. Total budget commitment is **$0.019187532**; **$4.930812468**
  remains under the authorized cap. This is local accounting, not a queried
  account-balance receipt. No top-up was made or is justified by these results.
- **Codex:** 21 model requests, including the invalid-output attempt; 20 usable
  case results. Captured usage is 153,242 input tokens, including 12,672 cached,
  and 8,899 output tokens, including 4,537 reasoning tokens. One additional
  preflight refusal made no model request. The weekly meter rose from 2% at the
  start to 6% at the last classifier completion; it includes orchestration and
  potentially other activity and is rounded. No API-dollar conversion is claimed.
- **Observed Jev request time:** median 0.127 seconds, range 0.087–0.215 seconds
  across 139 accepted requests. This times the HTTP request and validation, not
  extraction, evidence construction, investigations, or audits. It is not a
  controlled performance comparison and proves no end-to-end cost reduction.

The price and primitive contract were checked against the
[TypeSafe model documentation](https://docs.typesafe.ai/models) and
[binary-question documentation](https://docs.typesafe.ai/primitives/noul).
The [documented input sensitivities](https://docs.typesafe.ai/model-jaggedness/jev-1.13)
motivated context reduction and option-order tests; our observations, rather than
those descriptions, determine the recommendation.

## Failures and limits

The Jev v2 batch stopped when a valid-looking response's rounded probabilities
summed to 0.99. The original validator required a nearly exact sum. Its failure
and budget reservation were preserved; subsequent calls allow up to 0.015
rounding error across three probabilities. No failed response was silently scored
or retried. A test covers rounded and invalid distributions.

Codex initially refused a weekly reset timestamp that moved by one second. The
adapter now permits at most two seconds of timestamp jitter while still checking
the weekly window and 55% launch / 60% total limits. A separate model response
returned evidence pointers prefixed with `/state`, contrary to the contract.
It remained invalid and was replaced once with an unchanged, fresh request;
both requests' usage is included. Replacement lineage is saved.

The dataset contains 20 pairs, 19 distinct normalized comments, seven tasks, and
15 task/family groups. It uses current human rulings and exact saved normalized
review fields, not a new extraction of every original native review. Historical
references and rubric wording were themselves influenced by the rulings. Thirty
secondary reference citations are unavailable locally; this test treats the
approved reference description as given and does not re-establish source truth.
Four unruled cause axes remain masked. Pre-existing cases are included in the
small matching set, but no separate pre-existing grading workflow was qualified.

There are too few independent tasks to establish a 2% error bound. Even zero
errors in 14 independent cases would have a one-sided 95% upper error bound of
about 19%; these cases are additionally clustered. Input development has now
ended for this dataset. Any future tuning needs a new held-out set.

## Recommendation and next steps

Keep code in charge of mandatory evidence and policy prerequisites. Keep an LLM
responsible for investigations, with human rulings authoritative for disputed
grading interpretation. Jev may produce shadow annotations, but its confidence
must not close cases or suppress investigation on the evidence collected here.

Do not fund a larger run of this same credit classifier. A useful follow-up would
first obtain fresh, varied human-labelled cases for a simpler question—such as
cause matching or whether a packet contains a required explicit assertion—and
test it in shadow. Include negative and missing-evidence cases, independent task
groups, accepted-case audits, and the cost of obtaining the input evidence. The
remaining authorized balance already exceeds what this pilot needed.

Reproduce the checks and metrics:

```sh
python3 docs/research/jev-grading-2026-10-15/dataset/generate.py --check
python3 -m unittest discover -s docs/research/jev-grading-2026-10-15/runner -q
python3 -m unittest discover -s docs/research/jev-grading-2026-10-15/codex -q
python3 -m unittest discover -s docs/research/jev-grading-2026-10-15 -p 'test_evaluate.py' -q
python3 docs/research/jev-grading-2026-10-15/summarize.py
```

[Verified metrics and accounting](metrics/summary.json), [dataset manifest](dataset/manifest.json),
[decision log](decisions.tsv), [Jev caller](runner/README.md), and
[Codex adapter](codex/README.md) retain the detailed evidence. No Claude calls,
live-grade changes, commits, or publication were performed. The comparisons and
report review used OpenAI models only; there was no independent cross-provider
review.
