# Plan: a throwaway Vercel run to see the order of `x_astro_path`

Not run. The owner chose "Test on Vercel first" for question 3 on 2026-11-07 ([`../answers.md`](../answers.md)). This plan says what the run would take. Nothing was deployed, no one signed in, and nothing was spent.

## How the 2026-10-27 run was done

All of it is under [`../../rule-v4-candidates-2026-10-27/o-astro-16079/vercel-run/`](../../rule-v4-candidates-2026-10-27/o-astro-16079/vercel-run/) (`RESULTS.md`, `scripts/`, `step01` to `step15` logs, `app-10.0.3/`, `app-10.0.2/`).

- **Approval.** The owner wrote "Yes to vercel test, $40 cap is o\nok" (`../../rule-v4-candidates-2026-10-27/blind-limits.v1.json`). The $40 cap covered the round's blind sessions; the Vercel run cost nothing.
- **Account.** Vercel CLI 65.4.0, account `jack-7095`, team scope `jackchu-com`, the free Hobby plan with no payment method (`RESULTS.md` lines 3 to 4). The saved files name no token or environment variable. The run used a CLI session that was already signed in, on another host. This host has no Vercel CLI installed.
- **Site.** Two projects, `app-10.0.3` (`@astrojs/vercel` 10.0.3, astro 6.1.0, the head) and `app-10.0.2` (10.0.2, astro 6.0.7, the commit before). The adapters came from local tarballs. `isr: { expiration: 120, exclude: ['/account'] }`. A middleware answered `/account` from a dummy `session` cookie. The build wrote `allowQuery: ["x_astro_path"]` and `passQuery: true`, and routed `/one`, `/` and `/404` to `/_isr?x_astro_path=$0`.
- **Commands.** `npm i`, `astro build`, `npx vercel deploy --prebuilt --prod --yes`, then `curl -sS -D -` probes, each three times about 6 s apart (`scripts/attack.sh`, `req.sh`, `sanity.sh`).
- **What it showed.** `/_isr?x_astro_path=/account` rendered the account page at 10.0.3 and was cached for later visitors. `/one?x_astro_path=/account` returned `/one`'s cached copy (`x-vercel-cache: HIT`) three times. The function never ran, so the order Vercel uses was not seen.
- **Cleanup.** `npx vercel remove <project> --yes` for both, then `vercel project ls` ("No projects found under jackchu-com") and every URL answering 404 (`step10` to `step15`). `vercel ls` listed both for a few minutes afterwards, which `RESULTS.md` reads as listing lag.

## Why the cache hid the order

`allowQuery: ["x_astro_path"]` keys the ISR cache on the `x_astro_path` value alone. On 2026-10-27 the request hit `/one`'s entry, which suggests the cache key used the route's value. The function's own input is a separate question, and only a run in which the function executes can answer it.

## The run

1. **Site.** Copy `app-10.0.3/`. Its `package.json` points at a local adapter tarball that the repository doesn't hold, so install `@astrojs/vercel@10.0.3` and `astro@6.1.0` from the registry with `bun install`, using exact versions. Change the config to `isr: { expiration: 60, exclude: ['/account'], bypassToken: '<random 32+ characters>' }`. Add an echo page, `src/pages/[...slug].astro`, that prints `Astro.url.pathname`, `Astro.request.url`, every value of `new URL(Astro.request.url).searchParams.getAll('x_astro_path')` in order, and `Date.now()`. Make `/account` (excluded) print the same plus the cookie. The adapter rewrites only the path, so the query the function received stays readable on the request.
2. **Build and check.** Confirm that `.vercel/output/functions/_isr.prerender-config.json` holds `allowQuery`, `passQuery: true` and the `bypassToken`, and that `config.json` routes `/p1`, `/p2` and so on to `/_isr?x_astro_path=$0`.
3. **Deploy** to a new Hobby project: `bunx vercel deploy --prebuilt --prod --yes`. The October run used `npm` and `npx`; this plan uses `bun` and `bunx` for the same commands.
4. **Make the function run.** Send every probe with the cookie `__prerender_bypass=<bypassToken>`. Vercel's documentation says this Draft Mode bypasses "any caching that Vercel would normally provide" (`../../regrade-v10-rulings-2026-10-17/o-astro-16079/upstream/vercel-docs-build-output-api_features.md` line 146). As a second guard, give each probe a path and a visitor value used nowhere else, so any cache key is new. The `x-prerender-revalidate` header only makes the next request fresh (line 158), so it is a fallback.
5. **Probes**, each three times with `curl -sS -D -`, saving headers and bodies:
   - P1, the case: `GET /p1?x_astro_path=/account-1`.
   - P2, the same without the bypass cookie, a new value: `GET /p2?x_astro_path=/account-2`.
   - P3, control: `GET /p3`.
   - P4, GT-o1's direct request: `GET /_isr?x_astro_path=/account-4`.
6. **Read P1's echo** (and P2's, if the function ran):

   | Echoed `x_astro_path` values | Page rendered | Vercel does |
   | --- | --- | --- |
   | `/account-1`, `/p1` | `/account-1` | puts the visitor's value first |
   | `/p1`, `/account-1` | `/p1` | puts the route's value first |
   | `/account-1` | `/account-1` | replaces the route's value with the visitor's (what its dev server does) |
   | `/p1` | `/p1` | keeps only the route's value |

   P3 should echo only `/p3`, and P4 should render `/account-4`, as on 2026-10-27. If P1 still shows `x-vercel-cache: HIT` or no echo, the function did not run, and the run shows nothing.
7. **Clean up:** `bunx vercel remove <project> --yes`. Then check that `bunx vercel project ls` lists nothing and that every URL answers 404. Save the logs under `o-astro-16079/vercel-run/` in this round.

Time: about 20 to 30 minutes, most of it installing and building. Cost: $0 on the Hobby plan, as on 2026-10-27.

## What it can and can't show

It shows how Vercel combines the two query strings today, on 2026-11-07 or later. It cannot show what Vercel did at the cut-off, 2026-03-25, and the case file says so for the October run too. If the visitor's value reaches the function first or alone, the run shows that a visitor can choose the page the function renders. It does not settle the grouping with GT-o1; that stays the owner's question.

## What's needed from the owner

1. Approval to deploy one throwaway public site on Vercel, send the probes above, and delete it right after.
2. A Vercel account to use: `jack-7095` on team `jackchu-com` again, on the free Hobby plan with no payment method.
3. A way to sign in from this host, which has no Vercel CLI: the owner runs `bunx vercel login` here, or provides a token for the run through the vault. The token is deleted afterwards.
4. Confirmation that a $0 free-tier run is the limit, with no paid features.
