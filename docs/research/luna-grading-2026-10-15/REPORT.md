# Luna 6 High bounded grading exploration

**Luna 6 High is not ready to replace the bounded grader or decide which cases
can skip investigation.** It produced valid choice-only output reliably, but
credit agreement remained low. Cause matching remains a possible narrower shadow
experiment. No production grades changed.

## Results

The primary comparison uses the 14 historical qualification cases; cause has ten
ruled labels. Percentages are descriptive, not a generalization guarantee.

| Tested workflow | Credit agreement | Cause agreement |
| --- | ---: | ---: |
| Luna 6 High, original inputs, choices only | 9/14 (64%) | 9/10 (90%) |
| Luna 6 High, examples with corrected policy, choices only | 8/14 (57%) | 9/10 (90%) |
| Earlier Jev, original inputs | 7/14 (50%) | 9/10 (90%) |
| Earlier Sol 6.1 High, original inputs, evidence output | 12/14 (86%) | 10/10 (100%) |

Both usable Luna formulations recognized one of the two positive credit cases.
Original inputs falsely awarded credit on three of 12 negatives and abstained on
one negative. Corrected examples falsely awarded credit on five negatives and
never abstained. Examples did not demonstrate improved credit agreement in this
replay. The single-run design cannot attribute the difference to examples alone.

Original cause matching recognized six of seven positives and all three negatives.
Corrected examples recognized all seven positives but incorrectly accepted one
negative. These are different mistakes despite the identical 9/10 totals.

Development credit/cause agreement was 3/6 and 5/6 for original inputs, and 2/6
and 6/6 for corrected examples. The superseded, conflicting-instruction examples
run scored 4/6 and 6/6 on development, and 10/14 and 8/10 on qualification. It is
retained as a diagnostic and is not selected as the best workflow.

## Recommendation

Keep Luna’s results as shadow annotations. Do not use a decisive answer as a
reason to close a case or omit a trace/reproduction. The current credit question
still requires interpretation that these tested Luna workflows did not reliably
match to saved human rulings.

Stop tuning on these same cases. If pursuing Luna further, first assemble fresh,
varied human-labelled packets for a narrower question such as explicit cause
matching, with more negative and missing-evidence cases. Freeze the policy and
output schema, use the same contract for Luna and Sol, and test both accepted and
escalated cases against independent source investigations before estimating
routing safety or savings. This experiment supplies no missed-investigation
estimate. The existing stronger-grader workflow should continue meanwhile.

## What was tested

Fresh, tool-free `gpt-6-luna` sessions at high reasoning effort answered two
questions about a saved review comment and an approved reference description:

- **Credit (`says_what`):** Does the comment itself state this reference problem’s
  user-facing failure, without deriving a consequence from a cause or proposed fix?
- **Cause (`identifies_cause`):** Does it explicitly identify this reference
  problem’s cause as faulty, independently of whether it states the right effect?

These are packet judgments. The model did not inspect source, trace execution,
reproduce a bug, or decide whether an investigation was necessary. Saved human
rulings are the reference; Sol’s answers are a comparator, not ground truth.

The 20 cases cover seven tasks and 19 distinct normalized comments. Six cases
from three tasks formed development; 14 from four other tasks form the primary
comparison. Four cause axes lack a human ruling and remain excluded. This reuses
the Jev experiment’s historical split and previously exposed labels, so it is a
model-transfer replay, not a fresh blind validation. Thirty secondary citations
are unavailable locally; approved reference descriptions are treated as given.

## Prompt and output experiments

The first Luna cohort required a choice, a rationale, and exact evidence excerpts.
It produced 13 accepted results in 18 attempts: three failed excerpt validation,
one failed pointer validation, and one timed out after 600 seconds. The partial
qualification batch is not scored as a completed cohort. All failures remain
preserved, including the timeout whose token usage is unknown.

That output requirement tested quotation generation as well as classification,
and Jev had never been required to generate quotations. The next, separately
frozen cohort returned only `yes`, `no`, or `cannot_tell`. All 40 calls completed
with valid output: original packets (v1) and packets with development examples
(v7), across both partitions. Removing quotations also changed the policy and
sample mix; this is not a controlled causal estimate of the failure reduction.

A final audit found that v7’s supplied human examples conflicted with an inherited
instruction saying human labels did not apply. Those 20 v7 results remain as a
confounded diagnostic. A new `choices_examples` mode explicitly permits supplied
labelled examples and excludes unsupplied labels. It preserves the same questions,
examples, schema, isolation, and model. The corrected-policy cohort was frozen
before its 20 calls. Development examples exclude the current case’s whole task;
qualification packets use only the six development cases, never qualification labels.

## Usage and reliability

There were **78 Luna attempts**: 60 choice-only calls, all accepted, and the
18 earlier evidence-output attempts described above. Captured usage covers
**77 requests**: **559,470 input tokens** (95,744 cached) and **36,928 output tokens**
(31,705 reasoning), totaling 596,398 recorded tokens. The remaining timeout has
unknown usage; these recorded totals are not a complete billing total. All failed
attempts are retained and recorded usage includes their tokens.

The last completion meter reported **12% weekly Codex usage**, compared with 2%
at the start of the overall Jev/Sol/Luna work. This rounded shared-account reading
includes orchestration and potentially other activity; it is not Luna-only usage.
The run stayed well below the authorized 60% limit. No subscription-to-dollar
conversion or new TypeSafe charge is claimed.

Observed completed-call median elapsed time was 7.61 seconds for the original
40 choice-only calls and 7.38 seconds for the 20 corrected-policy calls. These
include client setup and quota checks, exclude failed attempts from the medians,
and were neither interleaved nor repeated performance trials. They do not measure
full grading, source-evidence preparation, investigations, or end-to-end savings.

## Interpretation limits

Credit in the primary split has only two positive and 12 negative labels. Always
answering “no” scores 12/14; overall accuracy alone cannot qualify a useful grader.
The full set has only three negative cause labels, all in qualification. Apparent
cause success on development, where all six labels are positive, is weak evidence.

The earlier Sol comparison required rationales and quotations, while the final
Luna cohorts require only choices. Policies differ as described above. The table
therefore compares tested workflows, not a controlled estimate of model quality
or an output-cost comparison. No repeated identical-input trials were run to
separate prompt effects from stochastic variation.

There is no calibrated confidence output and no labelled investigation-routing
set. `cannot_tell` can flag uncertainty, but a decisive wrong answer remains
possible. Missed investigations and end-to-end cost savings are **unmeasured**.
No result here licenses skipping source checks or automatically awarding credit.

## Reproduction and evidence

```sh
python3 docs/research/jev-grading-2026-10-15/dataset/generate.py --check
python3 -m unittest discover -s docs/research/jev-grading-2026-10-15/codex -q
python3 -m unittest discover -s docs/research/jev-grading-2026-10-15/runner -q
python3 -m unittest discover -s docs/research/jev-grading-2026-10-15 -p 'test_evaluate.py' -q
python3 docs/research/luna-grading-2026-10-15/summarize.py
```

The evaluator checks full case coverage, frozen input hashes, exact response
contract, saved rollout, model and effort, and session isolation. It refuses
missing/failed responses and batches mixing response modes.

[Metrics and complete local accounting](metrics/summary.json),
[decision log](decisions.tsv), [choices freeze](choices-cohort-freeze.v1.json),
[corrected examples freeze](examples-policy-freeze.v1.json), and
[earlier Jev/Sol report](../jev-grading-2026-10-15/REPORT.md) retain the evidence.
Verification passed: 53 offline tests, the frozen dataset check, and complete
artifact validation for all six choice-only batches. [Check receipts](verification.json)
record the runs. Authentication copies were removed from every attempt.

All comparisons and audit review use OpenAI models; no cross-provider review,
Claude calls, new Jev calls, production changes, commits, or publication occurred
in this Luna exploration.
