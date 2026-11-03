# Wording test of rule version 5, piece 1 ("A result one step away counts")

The owner approved the one-step line on 2026-11-02 and asked for a blind test before the regrade: do Opus, Sonnet and GLM 5.3 Flash apply the wording the same way, and the way the owner decided?

- `cases/`: eleven neutral cases. Eight are copies of the blind cases of reconciliation round `audit-v2-reconciliation-2026-10-31` (units 4, 12, 13, 20, 32, 35, 36 and 39), with "What is asked" cut to the one question. Three are the old "No credit" examples of section 3, written for this test from the review items and the answer key: `x-forked-child` (GT-v6), `x-ensure-role` (GT-v11), `x-request-bodies` (GT-p3).
- `v1/rule.md`, `v2/rule.md`: the part of section 3 on "Says what goes wrong?" with each wording. Examples drawn from the cases under test are left out. v1 allows one project fact a regular contributor knows. v2, chosen by the owner after v1, limits the step to what a thing the claim names is for.
- `run.py`: one session per case, model and repeat (two repeats), no tools. Claude runs `claude -p --safe-mode --tools ""`; GLM runs through Pi on OpenRouter with tools, context files, skills, extensions and MCP off.
- `v1/answers.json`, `v2/answers.json`, and `usage.json` beside each: the parsed answers and the cost. v1 cost $1.86 (Claude) and $0.018 (OpenRouter); v2 cost $1.66 and $0.013.

The owner decided units 12, 13, 20, 32, 35 and 36 as credit and units 4 and 39 as no credit. Units 4 and 39 turned on whether the claim is true and whether its route reaches the problem, which this test's rule excerpt leaves out, so every model's "yes" there says nothing about the wording.

| Case | Owner | v1 Opus | v1 Sonnet | v1 GLM | v2 Opus | v2 Sonnet | v2 GLM |
| --- | --- | --- | --- | --- | --- | --- | --- |
| u12, u13 | credit | yes, yes | yes, yes | yes, yes | yes, yes | yes, yes | yes, yes |
| u35 | credit | yes, no | yes, yes | yes, no | yes, yes | yes, yes | yes, yes |
| u36 | credit | yes, yes | yes, yes | yes, yes | yes, yes | yes, yes | no, yes |
| x-forked-child | to decide | yes, yes | no, no | yes, no | yes, yes | no, no | yes, no |
| x-ensure-role | to decide | yes, no | no, yes | no, yes | yes, yes | no, no | yes, no |
| x-request-bodies | to decide | yes, yes | yes, yes | yes, yes | no, no | no, no | no, no |

One v2 Sonnet answer on u32 wrote a malformed JSON string; its text answers yes.
