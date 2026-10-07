# Paper test of the second fact, 2026-10-07

Before the user accepted the text of [decision P22](../../rulings/P22-identifies-the-cause-as-a-fault.md), two model families answered the same 63 saved pairs of a comment and a known problem under four versions of the rule. Each session had to give an answer for every pair.

- **The pairs** ([`cases.json`](cases.json)): the 31 answers on which the trial's two graders differed on "says why?", 24 on which they agreed, and 8 more comments the user has ruled on. Every earlier answer and ruling was left out and the order was shuffled for each session. [`key.json`](key.json) maps each pair to its source and holds the expected answers.
- **The versions:** the rule as it stood ([`rule-current.md`](rule-current.md)); the same with only the name changed ([`rule-name-only.md`](rule-name-only.md)); the proposed text before the user's two rewrites ([`rule-full.md`](rule-full.md)); and the text as approved ([`rule-final.md`](rule-final.md)). The last two were each run twice.
- **The readers:** Claude Opus 5.5 and Codex GPT-6.1 Sol, both at high effort, each in a fresh session with [`prompt.md`](prompt.md). Their answers are under [`answers/`](answers/).

`python3 docs/research/cohort-rebuild-2026-10-05/second-pass/says-why/paper-test/score.py --detail` prints the tables below.

| Version | The two families agree, of 63 | On the 31 differences |
| --- | ---: | ---: |
| Current text | 57 | 27 |
| Name changed only | 50 | 20 |
| Proposed text | 57 | 30 |
| Proposed text, repeat | 55 | 27 |
| Approved text | 56 | 29 |
| Approved text, repeat | 59 | 30 |

What it shows:

- **Asked for every pair, the two families agree on 27 of the 31 under the current text.** Most of the graders' differences came from whether a grader considered a known problem for a claim at all, which this test cannot reproduce.
- **Changing only the name did not help.** In its one run the second family answered no more often, and agreement fell to 20 of 31.
- **The approved text reached 29 and 30 of the 31.** Both families answer no on the pair the answer key rules out, in every run of the proposed and approved text. Under the current text they differ on it.
- **The same text varies by about three answers between runs.**

What it does not show:

- **Whether graders will now consider every known problem.** Only a rerun of real batches can.
- **Anything about truth.** Each pair was given whether its claim was true. For the eight added ruled comments that field was set to "true" without a check, which is wrong for the two comments of ruling 16, whose stated situation the runs disprove. One family answered yes on the first fact for those two in every version. Their answers on the first fact are not evidence.

Two things it turned up:

- **Both families answer yes on the second fact for the two comments of ruling 16, in every version.** The ruling recorded no. It is shown to the user again with clean blind answers ([`../../assessors/review-16/`](../../assessors/review-16/)).
- **One expected answer was wrong.** The key says no for a grpc-go comment, on the view that its cause is stated only in its proposed fix. One family finds the cause in the comment's own title in every run. The answer key's entry for that known problem states the result and not the cause, which is why the two differ.

At list price the twelve sessions cost about $18 for Claude and a small amount for Codex.
