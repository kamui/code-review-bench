# Why the two graders disagreed on comment labels

The [first audit round](../decision-1.md) found close agreement on whether a known problem was caught and wide disagreement on the labels of other comments. The owner treated that as a question about the rubric. Before the rubric was changed, a fresh Codex GPT-6.1 Sol session at high effort read every disagreeing or unmatched unit of the five claim strata, 80 units, with both graders' records, and classified each one ([`prompt.md`](prompt.md), [`report.md`](report.md), [`units.json`](units.json)). It ran no model and changed nothing. Machine paths in its output were replaced with `<repo>` and `<cache>`.

What it found, in its own counts:

| Main cause | Units |
| --- | ---: |
| Same label, but a test answered about a different thing once the claim was rejected | 20 |
| The comment split into claims differently | 15 |
| Same facts, different label or threshold | 10 |
| Different checks, or a different idea of enough evidence | 8 |
| Different treatment of a promised use, a new obligation or a pending candidate | 8 |
| Partial wording read differently against a known problem | 6 |
| Same claims, quoted from different text | 5 |
| "Unsupported" used where the named scenario was disproved | 5 |
| Other | 3 |

It judged that the changes already decided (the two questions, the new labels and the two facts) give a shared answer for 51 of the 80, leave 22 it could not tell and 7 that still differ. These are readings of saved records, not a new measurement of agreement.
