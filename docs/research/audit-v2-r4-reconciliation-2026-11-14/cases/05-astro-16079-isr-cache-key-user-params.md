# Case: Astro 16079: "the ISR cache key also gains this param, since `allowQuery` includes it, so user query params may be mishandled"

Round `audit-v2-r4-reconciliation-2026-11-14`, evaluator audit `evaluator-audit-v2-2026-10-29-round-4`. This file is neutral: it gives the facts and both graders' answers and makes no recommendation. Each fact is marked **run** (executed, output saved under `probes/`), **read** (seen in source, a diff, the rubric or a saved record) or **reported** (someone's statement, not checked here). Facts are judged on what a reviewer could know at the cut-off; later records only confirm.

Units in this case: #30. "The first grader" made the grade under audit (Claude Opus 5.5, high effort, rule version 6). "The second grader" is the audit's independent grade (Claude Sonnet 5.5, high effort). The first grader's rule is `docs/research/audit-v2-r4-reconciliation-2026-11-14/rule/rubric.first.md` (sha256 `4a680252…`), the composition its receipts pin; the second grader read the audit's composition built from `bench/rubric/scoring.safety-second.v6.md` (the receipt's `rubric.md`, sha256 `a0dfb723…`).

This unit mirrors #25 (`cases/06-astro-16079-isr-visitor-query-repeat.md`): another review of the same batch, the same Vercel settings, and the graders' answers the other way round. There the first grader said **unresolved** and the second **unproven**; here the first said **unproven** and the second **unresolved**.

## The change

`o-astro-16079`: withastro/astro#16079, "fix(vercel): Fix ISR path rewrite to prevent 404", base `be661fb`, head `71ae513`, merge base `b089b90`, cut-off the last push, 2026-03-25T12:02:24Z. The graders had `bench/targets/o-astro-16079/packet.v2.md` (sha256 `fcb2d424…`). Round 2's case `docs/research/audit-v2-r2-reconciliation-2026-11-04/cases/06-astro-16079-isr-visitor-query.md` quotes the pull-request body and the whole diff, which changes only `packages/integrations/vercel/src/serverless/entrypoint.ts` (plus a changeset): requests with `x-vercel-isr: 1` and no valid middleware secret now take their path from `url.searchParams.get('x_astro_path')`.

## Answer key

From `bench/grading/current/references.json` (**read**): GT-o1 (any caller who sends the header and the query value chooses the page), GT-o2 (`x_astro_path` stays in the URL pages see, and in cached HTML), GT-o3 (one level of percent-encoding lost), GT-o4 (an unusable value used as the path), and PE-o1 (edge middleware's query string written into the path). None is about which visitor query keys the ISR cache keys on or passes on. Both graders credit GT-o2 through `c11`, the sentence before this claim. Neither links `c12` to any problem.

## Facts

- **read** Unchanged `index.ts` at the head, lines 709 to 714, writes the ISR function's prerender config with `allowQuery: [ASTRO_PATH_PARAM]` (`'x_astro_path'`) and `passQuery: true`. `git log -S allowQuery` puts these lines in the ISR feature of 2024 (`ff8b9d06c`, #9714) and the later single-entrypoint change (`9439f61e3`). The head's test `packages/integrations/vercel/test/isr.test.js` lines 16 to 25 checks that config. Line 63 routes ISR pages to `/_isr?x_astro_path=$0`.
- **read** Nothing in the repository at the head or in the packet says what Vercel does with a query key that is not in `allowQuery`: whether it is left out of the cache key, removed from the request the function gets, or both (`rg -i "allowQuery|passQuery|cached independently"` over the packet; round 2's case 06 searched the repository). The graders had no network and no Vercel deployment.
- **read** The first grader, in the same batch and the same grade, recorded the neighbouring question for #25 as a blocked check on "Vercel platform (ISR prerender function query handling)": "whether, with allowQuery: ['x_astro_path'] and passQuery: true, Vercel hands the ISR function the visitor's other query parameters". For `c12` it recorded no check and answered "not shown".
- **read** Earlier first grades of this same quote (`bench/grading/current/assessments/2026-10-08-last-push-claude-sonnet/o-astro-16079/`): assessment 2 unproven; assessment 3 refuted; assessment 4 outside this change; assessment 5 (rule version 5) **outside this change**, with the notes "allowQuery: [x_astro_path] (index.ts:712) keys the ISR cache on that parameter only, so other query parameters are not part of the key; that is what 'may be mishandled' points at. The line is not in the diff and behaves the same before and after."; assessment 6 (rule version 6) unproven; assessment 7 (the grade under audit) unproven.
- **read**, from the dossier of the owner's candidate round of 2026-11-07 (`docs/research/candidate-rulings-2026-11-07/rulings/03-astro-visitor-x-astro-path.md`, "What Vercel and others said by the cut-off"): Vercel's Build Output API primitives page, Wayback snapshot of 2026-03-11, two weeks before the cut-off: `allowQuery`: "List of query string parameter names that will be cached independently. If an empty array, query values are not considered for caching. If undefined each unique query value is cached independently"; `passQuery`: "When true, the query string will be present on the `request` argument passed to the invoked function. The `allowQuery` filter still applies." That page was public at the cut-off. It was not in the packet.
- **reported**, same dossier: withastro/astro #15092 (2025-12-31, by the author of this change, closed 2026-03-24): "the ISR page https://isr-with-query.vercel.app/one?param-1=is-allowed&param-3=not-allowed correctly consumes the 'param-1' key, but not the 'param-3' key, per the configuration."
- **read**, later evidence only (ground rule 3): the owner's Vercel run of 2026-11-08 (`docs/research/candidate-rulings-2026-11-07/o-astro-16079/vercel-run/RESULTS.md`), `@astrojs/vercel` 10.0.3, probe P6 `/p6?foo=1&x_astro_path=%2Faccount-6`: the function saw only the route's own value, "with the visitor's value and the other query keys gone". It shows Vercel on 2026-11-08, not at the cut-off.
- **read** The owner's candidate ruling `candidate-rulings-3` (index name, reading current; 2026-11-08) closed a claim on how Vercel merges a visitor's `x_astro_path` with the route's as **unproven**, with the ground "If we dont know this was the case when the PR was posted, we cna't say it's actually false can we?" (after-record `docs/research/candidate-rulings-2026-11-07/rulings/03-astro-visitor-x-astro-path.NC-a0a4cf9ad59c.after.json`). That round had read Vercel's pages from before the cut-off, run Vercel's local development server's merge code, and run on Vercel. It filed `CL-o-isr-visitor-path-first`. That claim is not linked to this review (the second grader's `claims.md` links only `CL-o-isr-param-left` and `CL-o-rewrite-body-duplex` to this item, both "related"), and the first grade (dispatched 2026-11-07T06:21:48Z) came before the ruling.
- **read** Round 2's owner call on #25's claim (`docs/research/audit-v2-r2-reconciliation-2026-11-04/rulings/06-astro-16079-isr-visitor-query.u32.after.json`, first-correct; reason in `bench/grading/current/audit/evaluator-audit-v2-2026-10-29-round-2/reconciliation.json`): "whether the visitor's query is lost depends only on which parameters Vercel passes with passQuery and allowQuery, a hosting fact nobody here could check." It is a reconciliation record, not in the rulings index.
- **read** Rubric, question 1: "not shown | A proper check found nothing for it and nothing against it." "cannot check | You cannot make a needed check here, or the check waits on a ruling." "If you could not make a needed check, answer "cannot check", not "not shown", and name the check." "You can't check facts that aren't in the code. The CI's time zone, how a hosting service behaves, or what's on a package registry are examples. If the claim depends on one of these, answer "cannot check" and say which fact. If the claim is true no matter what that fact is, answer yes." "You have no services, such as PostgreSQL, Redis, Docker or a cloud platform. If a needed check depends on one, record a blocked check that names the service". Section 3: "Leave a claim unresolved only when the rules say to." Its list includes "a fact the code can't tell you, such as how a hosting service behaves, when the claim's truth depends on it", and "These don't make a claim unclear: **A hedge.**"

## What users see

If Vercel leaves keys outside `allowQuery` out of the cache key, visitors who ask for the same ISR page with different query strings get one cached copy. If it also strips them from the request, the page's code never sees them. Vercel's documentation at the cut-off says the first and suggests the second; the 2026-11-08 run shows the second on that date.

## Maintainer action

None found for this point.

## The unit

### Unit #30: `2026-10-08-last-push-claude-sonnet/att-028#c12`

- Unit type: claim; stratum: unproven; configuration `claude-builtin-sonnet-5-5`; attempt att-028 of run `2026-10-08-last-push-claude-sonnet`.
- The first grader's result: **unproven**; the second grader's result: **unresolved** (blocked check, Vercel ISR cache).
- First grade receipt: `bench/grading/current/assessments/2026-10-08-last-push-claude-sonnet/o-astro-16079/assessment-7/receipt.json` (sha256 `a1d59cbc1d04db63…`; review token `blind-f71ffe`); second grade: `bench/grading/current/audit/evaluator-audit-v2-2026-10-29-round-4/second/2026-10-08-last-push-claude-sonnet/o-astro-16079/assessment-1/verdicts.json` (sha256 `ee625baaaf927b51…`; token `blind-7b642b`).

Review item-5 (verdicts key `6`), as the review wrote it:

```json
{
 "file": "packages/integrations/vercel/src/serverless/entrypoint.ts",
 "line_start": 28,
 "line_end": 28,
 "claim": "The rewritten `Request` does not preserve the original request's other properties. The `x_astro_path` param is left in the URL after the override.",
 "consequence": "The rebuilt `new Request(url, { method, headers, body })` drops `duplex` (needed when a streaming body is passed) and the signal. After the ISR rewrite, the URL still carries `?x_astro_path=...`, so `Astro.url.searchParams` and `Astro.url.search` expose an internal param to user code. The ISR cache key also gains this param, since `allowQuery` includes it, so user query params may be mishandled.",
 "proposed_fix": null,
 "kind": "finding"
}
```

Claims the first grader listed for this item: `c10` "The rebuilt `new Request(url, { method, headers, body })` drops `duplex` (needed when a streaming body is passed) and the signal." (first **suggestion**, second **suggestion**); `c11` "After the ISR rewrite, the URL still carries `?x_astro_path=...`, so `Astro.url.searchParams` and `Astro.url.search` expose an internal param to user code." (first **problem**, second **problem**, both GT-o2); `c12` "The ISR cache key also gains this param, since `allowQuery` includes it, so user query params may be mishandled." (first **unproven**, second **unresolved**).

The first grader's claim record, with its notes and evidence:

```json
{
 "id": "c12",
 "quote": "The ISR cache key also gains this param, since `allowQuery` includes it, so user query params may be mishandled.",
 "true": "not-shown",
 "this_change": null,
 "promised": null,
 "promise_source": [],
 "delivered": null,
 "outcome": "unproven",
 "kind": null,
 "known_problems": [],
 "pre_existing_problems": [],
 "pre_existing_candidate": null,
 "canonical_claim_id": null,
 "duplicate_group": null,
 "candidate": null,
 "open": null,
 "notes": "allowQuery: ['x_astro_path'] is in the unchanged prerender config and keying the cache on that value is what makes each ISR path cache separately; the comment names no user query parameter that is mishandled. Looked in index.ts buildISRFolder and the ISR route destinations; nothing shows a mishandled user parameter either way.",
 "evidence": [
  "clone/packages/integrations/vercel/src/index.ts:63,702-714",
  "clone/packages/integrations/vercel/test/isr.test.js:23"
 ]
}
```

The second grader's claim record, with its notes and evidence:

```json
{
 "id": "c12",
 "quote": "The ISR cache key also gains this param, since `allowQuery` includes it, so user query params may be mishandled.",
 "true": "cannot-check",
 "this_change": null,
 "promised": null,
 "promise_source": [],
 "delivered": null,
 "outcome": "unresolved",
 "kind": null,
 "known_problems": [],
 "pre_existing_problems": [],
 "pre_existing_candidate": null,
 "canonical_claim_id": null,
 "duplicate_group": null,
 "candidate": null,
 "open": {
  "kind": "blocked-check",
  "service": "Vercel ISR cache",
  "would_settle": "whether Vercel's ISR cache, keyed on allowQuery [x_astro_path] with passQuery true, drops, merges or mis-keys the user's own query params."
 },
 "notes": "allowQuery does list x_astro_path (index.ts:712), and the cache has to vary by it. Whether user query params are mishandled depends on Vercel's cache behaviour, which cannot be run here. The claim is hedged ('may').",
 "evidence": [
  "packages/integrations/vercel/src/index.ts:710-713; Vercel's ISR cache is unavailable."
 ]
}
```

## The question

Does the truth of "user query params may be mishandled" depend on how Vercel treats query keys outside `allowQuery`, a hosting fact the graders could not check, so that "cannot check" (unresolved) was the answer? Or was the first grader's search of the code a proper check that found nothing either way, so that "not shown" (unproven) was right?
