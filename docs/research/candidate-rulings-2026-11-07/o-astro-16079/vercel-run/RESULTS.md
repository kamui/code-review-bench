# Vercel run of 2026-11-08: which `x_astro_path` reaches the ISR function

The owner chose "Test on Vercel first" for question 3 ([`../../answers.md`](../../answers.md)) and approved this run on 2026-11-08 ("which can we do in parallel? let's start there.", to a question that described it as one throwaway public site on the free Hobby plan, $0). The run followed [`../vercel-plan.md`](../vercel-plan.md). It cost $0.

## Setup

- **Account.** Vercel CLI 65.12.1 (`bunx vercel@65.12.1`), account `jack-7095`, team `jackchu-com`, Hobby plan. The owner gave a token that expired within an hour. It was read from the T3 secret file for each command, never saved in the vault or in any file here, and that file was deleted after the run. No log here contains it.
- **Site.** [`app/`](app/): `@astrojs/vercel` 10.0.3 and `astro` 6.1.0 from the registry, exact versions (`bun.lock`), `isr: { expiration: 60, exclude: ['/account'], bypassToken }`. The bypass token was 40 random hex characters, and it is redacted here. `src/pages/[...slug].astro` echoes `Astro.url.pathname`, `Astro.request.url` and every `x_astro_path` value in order. `/account`, which is excluded from ISR, echoes the same.
- **Build output.** `_isr.prerender-config.json` holds `allowQuery: ["x_astro_path"]`, `passQuery: true` and the bypass token. `config.json` routes `^(?:/(.*?))?/?$` and `^/404/?$` to `/_isr?x_astro_path=$0` ([`app/routes.txt`](app/routes.txt)).
- **Deploy.** A new project `astro-xap-order-test`, `vercel deploy --prebuilt --prod --yes` at about 19:57 UTC ([`deploy.log`](deploy.log)).

## Probes

Each probe was sent three times about 6 s apart with `curl -sS -D -`. Probes P1 and P3 to P7 sent the cookie `__prerender_bypass=<token>`. Headers and bodies are in [`probes/`](probes/).

| Probe | Request | `x-vercel-cache` | `x_astro_path` the function saw | Page rendered |
| --- | --- | --- | --- | --- |
| P1, the case | `/p1?x_astro_path=/account-1` | BYPASS ×3 | `["/p1"]` | `/p1` |
| P2, no bypass | `/p2?x_astro_path=/account-2` | MISS, HIT, HIT | `["/p2"]` | `/p2` |
| P3, control | `/p3` | BYPASS ×3 | `["/p3"]` | `/p3` |
| P4, GT-o1 | `/_isr?x_astro_path=/account-4` | BYPASS ×3 | `["/account-4"]` | `/account-4` |
| P5, two values | `/p5?x_astro_path=/account-5a&x_astro_path=/account-5b` | BYPASS ×3 | `["/p5"]` | `/p5` |
| P6, another key first | `/p6?foo=1&x_astro_path=%2Faccount-6` | BYPASS ×3 | `["/p6"]` | `/p6` |
| P7, excluded route | `/account?x_astro_path=/p7` | MISS ×3 | `["/p7"]` | `/account` |

The function ran on every bypass probe: each answer has a new `rendered_at`. P2's first request also ran it.

## What it shows

On Vercel on 2026-11-08, a visitor's `x_astro_path` on a rewritten ISR route never reaches the function. The request the function receives carries only the route's own value (`/p1?x_astro_path=%2Fp1`), with the visitor's value and the other query keys gone. Its first row of the plan's table is not what happened. The last row is: Vercel keeps only the route's value. A visitor cannot choose the page `/one` renders this way.

P4 repeats GT-o1: a direct request to `/_isr` renders the page its `x_astro_path` names, as on 2026-10-27. P7 shows that on the excluded `/account` route, which the regular render function serves, the visitor's value reaches the function but does not change the page.

## Limits

This is Vercel's behaviour on 2026-11-08. It cannot show what Vercel did at the cut-off, 2026-03-25. It is one deployment, from one region, with the CLI's default settings.

## Cleanup

`vercel remove astro-xap-order-test --yes` at 20:01 UTC. Then `vercel project ls` reported "No projects found under jackchu-com", and the production and deployment URLs both answered 404 ([`cleanup.log`](cleanup.log)).
