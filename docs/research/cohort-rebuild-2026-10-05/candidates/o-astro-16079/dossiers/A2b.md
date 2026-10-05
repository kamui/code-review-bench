# A2b: the function uses whatever is in `x_astro_path` as the path, without checking that it is one

Candidates covered: none. No grader candidate record states this problem. It appears as a second point inside the review text of groups A1 and A2 ("the value is applied without validation", "only a value that starts with a single `/` should be accepted", "an empty value sets the pathname to `/`"). I split it out because it is a different problem from A1 and A2a and because the upstream project later had a production incident on exactly this point.

Background is in A1: Vercel sends each cached page to `/_isr?x_astro_path=$0` and is expected to replace `$0` with the requested path.

## Problem

The cached-page function accepts any text in `x_astro_path` and renders it as the path. The only check is that a value is present. If Vercel does not fill in `$0`, the function receives the two characters `$0`, treats them as the path `/$0`, and answers for that nonsense path. Vercel then caches the answer under the address of a real page.

## What changed

`packages/integrations/vercel/src/serverless/entrypoint.ts`:

```ts
+		} else if(request.headers.get('x-vercel-isr') === '1') {
+			realPath = url.searchParams.get(ASTRO_PATH_PARAM);
+		}
 		if (typeof realPath === 'string') {
 			url.pathname = realPath;
```

The added branch takes the value from the query. The unchanged check below it, `typeof realPath === 'string'`, is true for any text, including `$0`, `one` and the empty string.

The thing that can deliver a bad value is older and is not in this change: `const ISR_PATH = '/_isr?x_astro_path=$0'` in `src/index.ts`, in place since February 2024, with the comment "This isn't documented by vercel anywhere".

## Intended or announced

Not mentioned. The description covers only the 404 on cached pages. No test was added. Shipped in `@astrojs/vercel` 10.0.3 on 2026-03-26.

## What the affected person sees

Probe `probes/A2b`, in-process, cached-page function called with `x-vercel-isr: 1`.

At the head:

| Value of `x_astro_path` | Site with default settings | Site with `trailingSlash: 'always'` |
| --- | --- | --- |
| `/one` (normal) | page One, 200 | page One, 200 |
| `$0` (not filled in) | Astro's 404 page | `301` with `Location: /$0/?x_astro_path=$0` |
| `%240` (the same, percent-encoded) | not run | `301` with `Location: /$0/?x_astro_path=%240` |
| `one` (no leading slash) | page One, 200 | not run |
| empty | 404 page (the fixture has no home page) | not run |
| `//other.example/one` | 404 page | not run |

The `301` line has the same shape as the production symptom a user reported upstream six months later (issue #18028, on version 11.0.10): "`GET https://<site>/<some>/<valid>/<isr-path>/` → `301 https://<site>/$0/?x_astro_path=%240`", served from cache with an age of about five hours, in some regions, until the next deployment.

Who is affected in that report: every visitor to the affected page in the affected regions, for as long as the cache entry lives. The site owner can only redeploy.

What was not run: whether and when Vercel leaves `$0` unfilled. That is the user's report, and it needs Vercel's production proxy. The other values in the table (no leading slash, empty, double slash) can only come from someone calling `/_isr` by hand. The probe shows nothing harmful for them beyond rendering a page the caller chose, which is GT-o1.

Compared with before:

- Commit before the change: every cached request is the 404 page, whatever the value.
- 10.0.0 and 10.0.1: identical to the head (run with the older file).
- 9.x and earlier: the value was also used unchecked (`req.url = realPath`). From reading the old source.

## What the maintainers did

- Issue #18028, "ISR routes can serve a cached redirect to /$0 when Vercel doesn't substitute the undocumented $0 token", opened 2026-09-16 by a user, labelled "P4: important". It names two causes: the `$0` in the route table, and "The entrypoint accepts whatever is in `x_astro_path` without validation".
- Fix #18044, merged by maintainer matthewp on 2026-09-17, shipped in 11.0.11. It replaces `$0` with `$1` and adds a check that the value starts with `/`. Its description calls the check "defense in depth". The release note: "Fixes a rare issue where ISR pages on Vercel could intermittently be served a cached redirect to a nonsense URL (such as `/$0/`) instead of the page itself."
- matthewp on the issue, about the proposed fix: "Bot, I can confirm this fixes the issue." https://github.com/withastro/astro/issues/18028#issuecomment-5715984881
- Nobody named pull request #16079. The issue and the fix place the root cause in the `$0` route destination, which dates from 2024.
- The issue also notes that the token added for the security fix does not help here: "`x_astro_path_token` does **not** guard against this".

## How each fact is known

| Fact | How known |
| --- | --- |
| Results for each value at the head | run (`probes/A2b/result-head.txt`) |
| Every cached request is 404 at the commit before | run (`probes/A2b/result-base.txt`) |
| The file from before #15959 behaves like the head | run (`probes/A2b/result-pre-15959.txt`) |
| The diff lines and the unchanged `typeof` check | read (mirror diff) |
| `$0` route destination dates from February 2024 | read (mirror, `src/index.ts` history; #9714) |
| Vercel sometimes leaves `$0` unfilled in production | reported (user in `upstream/issue-18028.json`); not run |
| Vercel support says `$0` is outside the contract | reported (user's quote in `upstream/issue-18028-comments.json`) |
| Vercel's development router does not replace `$0` | read (`upstream/vercel-cli-dev-router.ts`, the pattern `/\$([1-9a-zA-Z]+)/g`) |
| Issue label, fix content, maintainer comment, release | read (`upstream/issue-18028.json`, `issue-18028-comments.json`, `pr-18044.json`, `pr-18044-files.json`, `release-vercel-11.0.11.json`) |

## Relation to existing reference bugs and ruled claims

Part of this overlaps GT-o1 and part does not.

- A bad value sent on purpose by a caller is GT-o1: the caller is choosing the page. Checking for a leading slash does not stop that, and it should not be counted again here.
- A bad value produced by the platform (`$0` left unfilled) is not GT-o1. No one is attacking, and the upstream security fix did not prevent it.

It is also separate from A2a, which is about a correct path being altered in transit. There are no ruled claims.

## Both sides

For counting it as a bug of this change:

- The branch that accepts the value unchecked is added by this change, and the check that upstream later added sits in that branch.
- The consequence, when the platform misbehaves, was serious for the people hit: a real page answered with a cached redirect to nonsense for hours.
- The upstream project rated the incident "P4: important" and fixed it.
- The code comment next to the route destination already warned that the `$0` behaviour was undocumented.

Against:

- The trigger is not in this change. It is the `$0` route destination from 2024 together with an intermittent fault in Vercel's proxy that was first reported six months after this change.
- As raised in the review text, the examples were a missing leading slash, a double slash and an empty value. None of them produces a failure in the probe except a caller picking a page, which is GT-o1. The review text does not name the `$0` case.
- The maintainers described the check as defence in depth and treated the route destination as the cause.
- Before this change the same request returned the 404 page, like every other cached request. Nothing that worked was broken by accepting the value.
- The unchecked use is as old as the feature.

## Recommendation

`advisory`, with medium confidence.

Reason: "check that the value is a path" is correct hardening advice, and the project adopted it later. At the time of this change no failing behaviour followed from its absence, apart from what GT-o1 already covers.

Strongest argument against: the missing check was half of the fix for a real production incident, and it belongs in the branch this change added. A reviewer who asked for it was asking for the thing that would have turned a cached redirect into a plain 404. If the owner counts restoring an unchecked read of an undocumented placeholder as a defect of this change, the facts support `eligible`.
