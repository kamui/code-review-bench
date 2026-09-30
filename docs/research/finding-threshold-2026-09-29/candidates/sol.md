We should **define the threshold, test it against borderline examples, then encode it in a new rubric**. We’ll keep existing scores unchanged while working through the remaining recommendations.

My proposed rule is:

> An eligible finding identifies a supported problem caused or worsened by the change, under a credible scenario, with a concrete consequence that warrants correction or an explicit maintainer decision to accept it.

That gives us four questions for each claim:

1. **Is it supported?** Source reasoning, a contract, a static counterexample, or reproduction establishes the problem. Running a reproduction isn’t mandatory.
2. **Does it belong to this change?** The PR introduces it, worsens it, or creates a new obligation—such as documenting a working setup.
3. **Is the trigger credible?** It fits supported usage or deployment conditions. Rare concurrency failures can qualify.
4. **Does the consequence matter?** It breaks behavior, compromises safety, loses meaningful test protection, or creates a demonstrated architectural or maintenance problem.

A proposed fix remains optional. Whether to block merging is a separate judgment.

We should also distinguish the reasons a claim doesn’t qualify:

| Outcome | Meaning |
|---|---|
| Useful advice | Correct, specific improvement with a concrete benefit, below the defect threshold |
| Inconsequential observation | Accurate, but little demonstrated benefit |
| Scope exclusion | Real problem outside this change’s review contract |
| Refuted finding | Evidence contradicts the alleged problem |
| Unsupported assertion | Necessary support is missing after an adequate check |
| Unresolved | Available evidence prevents a fair decision |

**Then we calibrate the difficult boundaries one at a time:** testing, architecture/maintainability, and speculative risk. Our three approved claims become positive examples; they don’t need another ruling.

I recommend starting with **testing**. The [GraphQL reference](/home/jack/.t3/worktrees/code-review-bench/t3code-64b6072b/bench/targets/k-graphql-js-1582/register.v1.json) accepts a test that silently stopped exercising its intended branch. That supports this starting rule:

> Demonstrated loss of meaningful test protection can qualify without a production failure. “Add more tests” needs a specific missing obligation or failure the current tests cannot detect.

Next, we should compare that accepted example with a rejected testing claim and decide exactly where the boundary falls.
