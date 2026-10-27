# RESULTS — Vercel ISR cache test for astro#16079 (task o-astro-16079)

Run date: 2026-10-27 (UTC). Operator: Vercel CLI 65.4.0 (native), account `jack-7095`
(deployments created under team scope `jackchu-com`), free Hobby plan, no payment method added.
Runtime: `nodejs24.x` (adapter default). All deployments were production (`--prebuilt --prod`),
public (Standard Protection exempts production domains). No secrets used; `session` cookie values
are dummies (`alice`, `carol`).

Versions tested:
- PR head (vulnerable): `@astrojs/vercel@10.0.3` + `astro@6.1.0`. Deployment `https://app-1003.vercel.app`.
- Pre-bug control:     `@astrojs/vercel@10.0.2` + `astro@6.0.7`. Deployment `https://app-1002.vercel.app`.
- Patched `11.0.3`: NOT run (optional in plan; it requires `astro@^7`, a different major and a
  less comparable baseline). The 10.0.2 control already isolates the behavior to the PR, so 11.0.3
  was skipped to limit spend. Record this as an intentional omission, not a failure.

Both builds emitted the same ISR primitive:
`_isr.prerender-config.json = {"expiration":120,"allowQuery":["x_astro_path"],"passQuery":true}`
and the same route table (`/account`→`_render` excluded; `/one`,`/`,`/404`→`/_isr?x_astro_path=$0`).
The only code difference is the adapter entrypoint: 10.0.3 adds
`else if (x-vercel-isr === "1") realPath = url.searchParams.get("x_astro_path")` (no secret);
10.0.2 resolves `realPath` only from the `x-astro-path` header when `middlewareSecret` matches.

CAVEAT (applies to every finding): these are observations of **today's Vercel platform**
(2026-10-27), not necessarily its behavior in March 2026 when the PR was merged. A TRUE result is
strong and time-robust; the FALSE result (Group C) is weaker as evidence about review-time behavior.

Evidence files (full status line, all headers via `curl -sS -D -`, and body) in this directory:
- step03-deploy-10.0.3.log, step04-sanity-10.0.3.log, step05-attack-10.0.3.log
- step08-deploy-10.0.2.log, step09-attack-10.0.2.log
Each cache test was repeated 3x with ~6s pauses (ISR storage can lag); `x-vercel-cache` and the
`rendered_at`/`nonce` markers in the body distinguish a cached HIT from a fresh render.

---

## CORE (GT-o1, forced render) — confirmed live on 10.0.3
Request: `GET /_isr?x_astro_path=/account` with `Cookie: session=alice`.
10.0.3 → `HTTP/2 200`, `x-vercel-cache: MISS`, body `ACCOUNT PAGE session=alice rendered_at=1793071449467`.
10.0.2 → `HTTP/2 404` (the 404 page). The excluded/per-user `/account` route is rendered by the
`_isr` function from the attacker's query value, with no secret, only on the PR head. *run*

---

## Group A — storage under the attacker-chosen key, re-served to later visitors
Claims 3-5 (fable att-027 item-1, ce-opus att-006 item-1, ce-opus att-026 item-0).

Sequence (10.0.3), key `x_astro_path=/account`:
1. `GET /_isr?x_astro_path=/account` (Cookie: session=alice) → 200, **MISS**, body `...rendered_at=1793071449467`.
2. `GET /_isr?x_astro_path=/account` (no cookie), x3 over ~18s → 200, **HIT** each, identical body
   `ACCOUNT PAGE session=alice rendered_at=1793071449467` (same timestamp ⇒ not re-rendered).
3. Group A2: same URL with `Cookie: session=carol`, x3 → 200, **HIT**, same alice body (cookie is
   not part of the cache key; only `x_astro_path` is).

Control (10.0.2): same sequence → `404` MISS then `404` HIT (the 404 page is what gets stored).

Observed `x-vercel-cache`: MISS → HIT → HIT…; bodies matched byte-for-byte across all HITs.
**Finding (run): Group A TRUE under today's Vercel.** One crafted `/_isr?x_astro_path=/account`
request stores the excluded `/account` render in the ISR cache under the attacker-chosen
`x_astro_path` key, and Vercel re-serves that stored response to every later request for the same
key for the `expiration` window. On 10.0.2 only the 404 page is stored, so the behavior is the PR's,
not Vercel's.

---

## Group B — cross-user cookie/session leak
Claim 1 (fable att-006 item-1).

Sequence (10.0.3):
- Seed: `GET /_isr?x_astro_path=/account` with `Cookie: session=alice` → body rendered with alice's
  cookie (`session=alice`), stored (confirmed by Group A).
- Victim: `GET /_isr?x_astro_path=/account` with **no cookie**, x3 → 200, **HIT**, body
  `ACCOUNT PAGE session=alice rendered_at=1793071449467` — i.e. a second visitor with no session
  receives the page rendered from **alice's** cookie. Group A2 shows the same for `carol`.

Why it stores: the `/account` responses carry no `Set-Cookie`, no `Vary`, `cache-control:
public, max-age=0, must-revalidate`. Vercel's `vary_key_denied:cookie` non-storage guard therefore
never fires (it keys off a per-visitor `Vary`, which Astro does not emit here). Request cookies DO
reach the `_isr` function (the seed body reflects `session=alice`).

Control (10.0.2): every variant returns the 404 page; no account content, so no leak.

Observed `x-vercel-cache`: MISS (seed) → HIT (victims); victim body == seeded alice body.
**Finding (run): Group B TRUE under today's Vercel.** A response rendered with one visitor's cookie
is stored and served to other visitors — a cross-user cache leak. Surprising relative to the plan's
doubt: the absence of `Vary`/`Set-Cookie` on Astro's pages means the platform's cookie-based
non-storage guard does not protect this case.

---

## Group C — poisoning `/one` via a merged query
Claim 2 (fable att-017 item-2).

Sequence (10.0.3):
1. Baseline `GET /one` x2 → 200 `text/html`, `ONE PAGE rendered_at=1793071510636 nonce=dgu91kmthat`
   (MISS then HIT; a stable cached entry for `/one`).
2. `GET /one?x_astro_path=/account` with `Cookie: session=alice`, x3 → 200 **HIT**, body is the
   **/one** page (same `rendered_at`/`nonce`), `content-type: text/html` — NOT the `/account`
   (`text/plain`) render. The client-supplied `x_astro_path=/account` did not win.
3. Recheck `GET /one` x3 → 200 **HIT**, unchanged `/one` body.

Control (10.0.2): all of the above return the 404 page.

Observed `x-vercel-cache`: HIT throughout; `/one` body never changed to `/account` content.
**Finding (run): Group C FALSE under today's Vercel.** Vercel's internal rewrite for `/one`
(`dest:/_isr?x_astro_path=$0`, `$0=one`) is not overridden by the client's own `?x_astro_path=`:
the `/one` cache entry was neither rendered as `/account` (C1 false) nor poisoned for normal
visitors (C2 false). This is the weakest result as review-time evidence (platform-routing dependent
and may have changed since March 2026), so it does not by itself clear claim 2 for the merge window.

---

## Notes / surprises
- 10.0.2 returns the 404 page for ALL `/_isr`-routed requests, including a normal `/one`, because
  that build resolves the ISR path only from the secret-gated `x-astro-path` header, never the
  query. Vercel's ISR *storage* is identical between the two builds (MISS→HIT on the same key); the
  sole difference is which route the `_isr` function renders. That makes 10.0.2 a clean control:
  storage mechanics constant, only the PR's query-path branch changes the outcome.
- The cache key for these ISR entries is the `x_astro_path` value alone; request cookies are
  forwarded into the render but excluded from the key, which is what produces the Group B leak.

## Teardown
(appended below after removal)

### Teardown confirmation (2026-10-27 ~03:32 UTC)
Commands (full output in step10-step15 logs):
- `npx vercel remove app-10.0.3 --yes` → "Success! Removed 1 project — app-10.0.3"
- `npx vercel remove app-10.0.2 --yes` → "Success! Removed 1 project — app-10.0.2"
  (removing the project removes all its deployments)

Verification:
- `npx vercel project ls` → **"No projects found under jackchu-com"** (authoritative: both projects deleted).
- URL probes after removal → all **HTTP/2 404** (deployment not found):
  `app-1003.vercel.app/one`, `app-1002.vercel.app/one`, and both `*-jackchu-com.vercel.app` URLs.
- `npx vercel remove <both deployment URLs> --yes` → "Could not find any deployments or projects
  matching …" (nothing left to remove).
- `npx vercel ls` still listed the two deployments as Ready for several minutes — this is the
  global deployment listing lagging after project deletion; it is contradicted by the three
  authoritative checks above (empty project list, 404 on every URL, CLI cannot find them to remove).

No payment method was added; no account settings changed beyond creating and deleting these two
test projects. No paid features enabled; no upgrade prompts encountered.
