# Phase 2: Admin Shell and Design System Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give the client the finished Evergreen admin frame early: themed tokens, a permission-aware shell (sidebar, topbar, mobile sheet, ⌘K), a typed data layer, dashboard primitives and a v1 dashboard on data that already exists, an audit-log table, a schema-driven form foundation with an Account page, and visual/accessibility QA.

**Architecture:** One isomorphic navigation registry (`lib/admin/nav.ts`) drives the sidebar, the ⌘K search API and the server-side page guard, so a link can never be shown to a role that the page then refuses (or the reverse). The guarded `(panel)` layout reads the session once (`requireActor`), hands the `/me` payload to a client `AdminActorProvider`, and wraps every page in the TanStack Query + nuqs providers and the shell. Client data goes through typed `hc` clients per Hono sub-app and `parseResponse` → `ApiError`; RSC prefetches call the same Hono stack in-process so server and browser data have exactly one shape.

**Tech Stack:** Next.js 16.3.8 (Cache Components) · React 19.2.8 · Tailwind CSS 4 · shadcn 4.21.1 `base-nova` on `@base-ui/react` 1.8.0 · next-themes 0.4.6 · sonner 2.0.8 · cmdk 1.1.1 (through shadcn `command`) · `@tanstack/react-query` 5.104.1 · `@tanstack/react-table` 9.2.8 · Recharts 3.8.0 (through shadcn `chart`) · nuqs 2.10.2 · react-hook-form 7.89.0 + `@hookform/resolvers` 5.9.1 · Zod 4.6.5 · date-fns 4.4.0 + `@date-fns/tz` 1.5.0 · Hono 4.13.13 (`hono/client`) · Better Auth 1.7.7 · Vitest 5.0.3 (+ jsdom 30, Testing Library) · Playwright 1.64 + `@axe-core/playwright` 4.13.0 · Lighthouse 13.5.0.

**Spec:** [`docs/brief.md`](../brief.md) §5.3–5.4 (layout, layering), §7.2–7.3 (roles, enforcement layers), §8.1–8.4 (Hono, conventions, route map, admin client), §9.1–9.4 (IA, Evergreen design language, key screens, UX standards). The contract is the "Phase 2: Admin shell and design system" scope in [`docs/plan.md`](../plan.md) (line ~5404). The visual reference is [`docs/design/admin-inspiration.png`](../design/admin-inspiration.png).

## Global Constraints

Copied verbatim from `docs/plan.md` § Global Constraints; every task's requirements implicitly include them.

- Next.js `16.3.x` App Router with `cacheComponents: true`, on the Node runtime only. Do not upgrade to 16.4 inside this plan.
- `proxy.ts` sits at the repo root (no `src/`). It does optimistic cookie checks only and **never touches the database**.
- Hono `4.13.x` with `@hono/vercel` `1.0.x` and `@hono/zod-validator` `0.9.x`. **No Hono v5**, and never import `hono/vercel`.
- Better Auth `1.7.x`. The CLI is the `auth` package (`pnpm dlx auth@1.7.7 …`), not `@better-auth/cli`.
- Zod `4.6.x`, always imported as `import * as z from "zod"`.
- Database casing is `snake_case`, primary keys are `uuid`, and timestamps are `timestamptz`.
- **No database access at build time.** Every DB read is a `'use cache'` query called after `await io()` inside a `<Suspense>` boundary. Never read env or the DB at module scope. CI builds with an unreachable `DATABASE_URL`.
- Never use `NEXT_PUBLIC_*` env vars. Client-visible values are passed down from the server as props.
- Every `server/**` module begins with `import "server-only"`, and client components never import it. Node scripts that load server modules run with `NODE_OPTIONS=--conditions=react-server`.
- Public pages never `fetch` our own `/api/*`. Hono serves only the admin, the forms and cron.
- Hono route handlers do three things: validate (Zod), authorise (`can()`), call a service. Services own the audit log and cache invalidation.
- Client code never queries `document` for page content. It uses `findTarget()` and `activePageRoot()` from `lib/page-root.ts`, because Cache Components keeps hidden routes in the DOM through `<Activity>`.
- shadcn `base-nova` is built on Base UI. Compose with the `render` prop (never `asChild`), and pass `nativeButton={false}` when rendering a non-button.
- The public site is light-only. The admin supports light, dark and system. No single-key hotkeys.
- Every public site section aligns to `components/layout/container.tsx` (max width 1440px, 66px gutter).
- Prettier: no semicolons, double quotes, `trailingComma: "es5"`, width 80.
- File names are kebab-case.
- Use named exports. Default exports are only for `page`, `layout` and `route` files and configs.
- Comments explain *why*.
- Any task that touches the public site must pass `pnpm test:parity` at ≤ 0.1% pixel difference per page.
- Every task ends green on `pnpm lint`, `pnpm typecheck`, its tests and `pnpm build`, then makes one Conventional Commit.

Phase-specific pins (from the brief and the installed tree; verified against `node_modules` / `pnpm view` while writing this plan):

- Add dependencies at exactly these versions: `@tanstack/react-query@5.104.1`, `@tanstack/react-table@9.2.8`, `recharts@3.8.0` (the version shadcn's `chart` item pins), `react-is@19.2.8`, `react-hook-form@7.89.0`, `@hookform/resolvers@5.9.1`, `date-fns@4.4.0`, `@date-fns/tz@1.5.0`; dev: `postcss@8.5.29`, `@testing-library/react@16.3.3`, `@testing-library/dom@10.4.2`, `@testing-library/user-event@14.6.7`, `@axe-core/playwright@4.13.0`.
- shadcn components are added with the repo's own CLI (`pnpm exec shadcn add …`, shadcn 4.21.1, style `base-nova`). Never pass `--overwrite`: `button`, `input`, `separator`, `field`, `card`, `label`, `checkbox`, `sonner` are already customised.
- TanStack Query: `prefetchQuery` is deprecated; prefetch with `queryClient.query(options).catch(noop)`. Server detection is `environmentManager.isServer()` (the `isServer` constant is deprecated).
- TanStack Table v9: `useTable({ features, columns, data, … })` with `features = tableFeatures({ … })` defined at module scope; controlled slices use `state.<slice>` + `on<Slice>Change` with `functionalUpdate`.
- Recharts 3.8: `Cell` is deprecated; customise bars with the `shape` prop. Charts set `accessibilityLayer={false}` because each chart ships its own screen-reader table.
- nuqs parsers shared by RSC and client code are imported from `nuqs/server` (the `nuqs` entry is a client module); hooks come from `nuqs`.
- `error.tsx` receives `retry` (stable in 16.3), not `unstable_retry`.
- Dates are shown in `Europe/Dublin`; weeks start on Monday.

## Review Focus

1. **Navigation and page guard drift.** A role sees a sidebar or ⌘K link and then lands on "no access", or a hidden section still opens by URL. Expected: what is listed is exactly what opens. → Task 2.3 (`lib/admin/nav.test.ts` "every listed link opens, every unlisted one is refused" for all seven roles) and Task 2.5 (`navigation.spec.ts` intake/viewer click-through).
2. **Theme flash or hydration mismatch.** With a dark OS preference, or a saved "light" choice under a dark OS, the first paint is the wrong theme or React logs a hydration error. Expected: the right class and background before hydration, no hydration warnings. → Task 2.2 (`theme.spec.ts`).
3. **⌘K leaks or misfires.** The palette lists destinations the role cannot open, or a bare `k` / `d` typed into a field opens the palette or flips the theme. Expected: only reachable destinations; only ⌘K / Ctrl+K opens it. → Task 2.6 (`tests/integration/api/search.test.ts`, `use-command-hotkey.test.tsx`, `command-palette.spec.ts`).
4. **Sidebar state on narrow viewports.** At 834 px (tablet) with a "collapsed" cookie saved on desktop, the navigation must be a closed sheet that opens at full width, not an icon rail or nothing. → Task 2.5 (`navigation.spec.ts`, tablet and mobile projects).
5. **Keyboard trap in the mobile sheet.** Escape must close the sheet and return focus to the trigger; following a link must close it. → Task 2.5 (`navigation.spec.ts`, mobile project).

---

## How to run things

- Unit: `pnpm test:unit` (Vitest `unit` project; React component tests opt into jsdom with a `// @vitest-environment jsdom` first line, like `lib/page-root.test.ts`).
- Integration: `pnpm db:up` once, then `pnpm test:integration` (real PostgreSQL 17, one file at a time).
- E2E: `pnpm db:up`, then `pnpm test:e2e <path>` (builds, starts on port 3100, seeds the `mk_e2e` database). Add `E2E_SKIP_BUILD=1` after one build to iterate faster.
- Gates for every task: `pnpm lint && pnpm typecheck && pnpm test:unit && pnpm build`, plus the suites the task names.

## File structure

| Path | Responsibility | Task |
| --- | --- | --- |
| `app/(admin)/admin.css` | Evergreen tokens (light/dark), type scale, status/hatch/hero/promo utilities, `@source not`, reduced motion | 2.1, 2.11 |
| `app/(site)/site.css` | `@source not` for `admin/**` and `docs/**` | 2.1 |
| `tests/css/*.test.ts` | Token completeness + WCAG contrast; Tailwind source scoping | 2.1 |
| `components/theme-provider.tsx` | next-themes provider for the admin (no hotkey) | 2.2 |
| `app/(admin)/layout.tsx` | Admin root layout: ThemeProvider, Toaster following the theme | 2.2 |
| `lib/admin/nav.ts` | Isomorphic nav registry, `visibleNav`, `reachableLinks`, `linkForPath`, `isActivePath` | 2.3 |
| `lib/auth/me.ts` | `MePayload`, `toMePayload` (shared by `/me` and the panel layout) | 2.3 |
| `server/auth/session.ts` | + `requirePermission`, `NO_ACCESS_PATH` | 2.3 |
| `server/modules/session/routes.ts` | `GET /admin/me` sub-app | 2.3 |
| `admin/lib/actor-context.tsx` | `AdminActorProvider`, `useActor`, `usePermission` | 2.3 |
| `app/(admin)/admin/(panel)/[section]/[[...rest]]/page.tsx` | Guarded "coming soon" page for every not-yet-built nav destination | 2.3 |
| `app/(admin)/admin/(panel)/{no-access,not-found}` | No-access screen, panel 404 | 2.3 |
| `admin/lib/{api,api-error,query-client,query-keys,query-provider}.ts(x)` | Typed `hc` clients, `parseResponse` → `ApiError`, query client, key factory, providers | 2.4 |
| `server/api/in-process.ts` | RSC → Hono in-process clients (same stack, no network) | 2.4 |
| `eslint.config.mjs` | §5.4 layering rule (client/shared code never imports `@/server`) | 2.4 |
| `components/ui/*`, `hooks/use-mobile.ts` | shadcn base-nova components (added per task) | 2.5–2.10 |
| `admin/components/shell/*` | Sidebar, nav, website card, topbar, user menu, notifications, shell | 2.5 |
| `app/(admin)/admin/(panel)/{layout,error}.tsx` | Guarded shell layout, error boundary with retry | 2.5 |
| `tests/e2e/admin/auth.setup.ts`, `tests/e2e/support/storage.ts` | Per-role storage states (owner with 2FA) | 2.5 |
| `server/modules/search/*` | `GET /admin/search` (navigation hits) | 2.6 |
| `admin/components/command/*` | ⌘K palette, hotkey, actions registry, search pill | 2.6 |
| `admin/components/dashboard/*` | PageHeader, KpiCard, PillBarChart, Gauge, StatusPill, ActivityFeed, EmptyState, skeletons | 2.7 |
| `app/(admin)/admin/(panel)/design/page.tsx`, `admin/modules/design/*` | Owner-only showcase on sample data | 2.7, 2.10 |
| `server/modules/dashboard/*` | `GET /admin/dashboard` (repo, week maths, service, route) | 2.8 |
| `admin/modules/dashboard/*`, `app/(admin)/admin/(panel)/page.tsx` | Dashboard v1 | 2.8 |
| `server/modules/audit/*` | Audit repo, filters (actor, action, date), facets | 2.9 |
| `admin/components/data-table/*`, `admin/modules/activity/*` | DataTable (TanStack Table v9 + nuqs), `/admin/activity` | 2.9 |
| `admin/components/schema-form/*`, `admin/modules/account/*` | SchemaForm foundation, Account page | 2.10 |
| `tests/e2e/admin/{visual,a11y}/*`, `scripts/lighthouse-admin.mjs` | Screenshots, axe, keyboard, reduced motion, Lighthouse | 2.11 |

---

## Task 2.1: Evergreen tokens, type scale and Tailwind source scoping

**Files:**
- Modify: `app/(admin)/admin.css` (whole file shown below)
- Modify: `app/(site)/site.css:1-5`
- Create: `tests/css/admin-tokens.test.ts`
- Create: `tests/css/tailwind-sources.test.ts`
- Modify: `package.json` (devDependency `postcss@8.5.29`)

**Interfaces:**
- Consumes: nothing new.
- Produces (CSS, used by every later task):
  - colours `bg-success`/`text-success`, `bg-success-soft`/`text-success-soft`, the same for `warning` and `danger`;
  - utilities `bg-hatch` (CSS stripes), `bg-hero` (KPI gradient + white text), `bg-promo` (website card);
  - type scale `text-title` (36 px/600), `text-card-title` (18 px/600), `text-kpi` (44 px/600); radius `rounded-card` (22 px);
  - raw tokens `--hero-from`, `--hero-to`, `--hero-foreground`, `--promo-from`, `--promo-to`, `--promo-foreground`, `--hatch` (the pill chart and gauge read `--hatch` and `--chart-*` through CSS variables).

- [ ] **Step 1: Write the failing token test**

```ts
// tests/css/admin-tokens.test.ts
import { readFileSync } from "node:fs"
import path from "node:path"

import { describe, expect, it } from "vitest"

const css = readFileSync(
  path.join(import.meta.dirname, "../../app/(admin)/admin.css"),
  "utf8"
)

// The declarations inside the first `<selector> {` block, as written.
function declarations(selector: string) {
  const start = css.indexOf(`${selector} {`)
  if (start === -1) throw new Error(`admin.css has no ${selector} block`)
  const body = css.slice(start, css.indexOf("}", start))
  return Object.fromEntries(
    [...body.matchAll(/--([\w-]+):\s*([^;]+);/g)].map(([, name, value]) => [
      name,
      value.trim().toLowerCase(),
    ])
  )
}

const light = declarations(':root[data-theme="admin"]')
// Dark only overrides; anything it leaves out falls back to light.
const dark = { ...light, ...declarations(':root[data-theme="admin"].dark') }

function luminance(hex: string) {
  const [r, g, b] = [1, 3, 5].map((i) => {
    const channel = parseInt(hex.slice(i, i + 2), 16) / 255
    return channel <= 0.03928
      ? channel / 12.92
      : ((channel + 0.055) / 1.055) ** 2.4
  })
  return 0.2126 * r + 0.7152 * g + 0.0722 * b
}

function contrast(a: string, b: string) {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x)
  return (hi + 0.05) / (lo + 0.05)
}

const REQUIRED = [
  "background",
  "foreground",
  "card",
  "muted-foreground",
  "primary",
  "primary-foreground",
  "accent",
  "accent-foreground",
  "border",
  "ring",
  "sidebar",
  "chart-1",
  "chart-5",
  "success",
  "success-soft",
  "warning",
  "warning-soft",
  "danger",
  "danger-soft",
  "hatch",
  "hero-from",
  "hero-to",
  "hero-foreground",
  "promo-from",
  "promo-to",
  "promo-foreground",
]

// Text/background pairs the components actually use. Muted text never sits
// on the bare canvas (only on cards and panels), so that pair is not here.
const PAIRS: [text: string, surface: string][] = [
  ["foreground", "background"],
  ["foreground", "card"],
  ["foreground", "sidebar"],
  ["muted-foreground", "card"],
  ["muted-foreground", "sidebar"],
  ["primary-foreground", "primary"],
  ["primary", "card"],
  ["accent-foreground", "accent"],
  ["success", "success-soft"],
  ["warning", "warning-soft"],
  ["danger", "danger-soft"],
  ["hero-foreground", "hero-to"],
  ["promo-foreground", "promo-to"],
]

describe.each([
  ["light", light],
  ["dark", dark],
])("%s Evergreen tokens", (_name, tokens) => {
  it.each(REQUIRED)("defines --%s", (token) => {
    expect(tokens[token]).toBeTruthy()
  })

  it.each(PAIRS)("%s on %s meets WCAG AA (4.5:1)", (text, surface) => {
    expect(contrast(tokens[text], tokens[surface])).toBeGreaterThanOrEqual(4.5)
  })
})

describe("type scale", () => {
  it.each(["--text-title:", "--text-card-title:", "--text-kpi:"])(
    "declares %s",
    (token) => {
      expect(css).toContain(token)
    }
  )
})
```

- [ ] **Step 2: Write the failing source-scoping test**

The sentinel class names are assembled from pieces so this test file (which Tailwind also scans) cannot itself emit them.

```ts
// tests/css/tailwind-sources.test.ts
import { readFile } from "node:fs/promises"
import path from "node:path"

import tailwind from "@tailwindcss/postcss"
import postcss from "postcss"
import { describe, expect, it } from "vitest"

const root = path.join(import.meta.dirname, "../..")

async function build(file: string) {
  const from = path.join(root, file)
  const result = await postcss([tailwind({ base: root })]).process(
    await readFile(from, "utf8"),
    { from }
  )
  return result.css
}

// Arbitrary-value classes are generated for any file Tailwind scans, so they
// show exactly which sources each stylesheet reads.
// Used only in admin/modules/auth/two-factor-setup.tsx:
const ADMIN_ONLY = `.${["tracking", "\\[0\\.3em\\]"].join("-")}`
// Used only in features/how-it-works/components/how-it-works-section.tsx:
const FEATURES_ONLY = `.${["leading", "\\[1\\.18\\]"].join("-")}`

describe("Tailwind sources", () => {
  it("the site stylesheet ignores admin/**", async () => {
    const css = await build("app/(site)/site.css")
    expect(css).toContain(FEATURES_ONLY)
    expect(css).not.toContain(ADMIN_ONLY)
  }, 60_000)

  it("the admin stylesheet ignores features/**", async () => {
    const css = await build("app/(admin)/admin.css")
    expect(css).toContain(ADMIN_ONLY)
    expect(css).not.toContain(FEATURES_ONLY)
  }, 60_000)
})
```

- [ ] **Step 3: Install postcss and run both tests to verify they fail**

Run: `pnpm add -D postcss@8.5.29 && pnpm vitest run --project unit tests/css`
Expected: FAIL — `defines --success` (and the other new tokens) fail with `expected undefined to be truthy`; `the site stylesheet ignores admin/**` fails with `expected '…' not to contain '.tracking-…'`.

- [ ] **Step 4: Replace `app/(admin)/admin.css`**

```css
@import "tailwindcss";
@import "tw-animate-css";
@import "shadcn/tailwind.css";

/* The admin never renders the public site's sections, and the planning docs
   are full of example class names: scanning either only bloats this file. */
@source not "../../features";
@source not "../../docs";

@custom-variant dark (&:is(.dark *));

/* Evergreen type scale and card radius (docs/brief.md §9.2). */
@theme {
  --text-title: 2.25rem;
  --text-title--line-height: 1.1;
  --text-title--letter-spacing: -0.025em;
  --text-title--font-weight: 600;
  --text-card-title: 1.125rem;
  --text-card-title--line-height: 1.35;
  --text-card-title--font-weight: 600;
  --text-kpi: 2.75rem;
  --text-kpi--line-height: 1;
  --text-kpi--letter-spacing: -0.03em;
  --text-kpi--font-weight: 600;
  --radius-card: 1.375rem;
}

/* "Evergreen" admin tokens (docs/brief.md §9.2). Static for now; Phase 9
   generates them from the theme engine. */
@theme inline {
  --font-sans: var(--font-jakarta);
  --font-mono: var(--font-geist-mono);
  --color-background: var(--background);
  --color-foreground: var(--foreground);
  --color-card: var(--card);
  --color-card-foreground: var(--card-foreground);
  --color-popover: var(--popover);
  --color-popover-foreground: var(--popover-foreground);
  --color-primary: var(--primary);
  --color-primary-foreground: var(--primary-foreground);
  --color-secondary: var(--secondary);
  --color-secondary-foreground: var(--secondary-foreground);
  --color-muted: var(--muted);
  --color-muted-foreground: var(--muted-foreground);
  --color-accent: var(--accent);
  --color-accent-foreground: var(--accent-foreground);
  --color-destructive: var(--destructive);
  --color-border: var(--border);
  --color-input: var(--input);
  --color-ring: var(--ring);
  --color-chart-1: var(--chart-1);
  --color-chart-2: var(--chart-2);
  --color-chart-3: var(--chart-3);
  --color-chart-4: var(--chart-4);
  --color-chart-5: var(--chart-5);
  --color-sidebar: var(--sidebar);
  --color-sidebar-foreground: var(--sidebar-foreground);
  --color-sidebar-primary: var(--sidebar-primary);
  --color-sidebar-primary-foreground: var(--sidebar-primary-foreground);
  --color-sidebar-accent: var(--sidebar-accent);
  --color-sidebar-accent-foreground: var(--sidebar-accent-foreground);
  --color-sidebar-border: var(--sidebar-border);
  --color-sidebar-ring: var(--sidebar-ring);
  --color-success: var(--success);
  --color-success-soft: var(--success-soft);
  --color-warning: var(--warning);
  --color-warning-soft: var(--warning-soft);
  --color-danger: var(--danger);
  --color-danger-soft: var(--danger-soft);
  --radius-sm: calc(var(--radius) * 0.6);
  --radius-md: calc(var(--radius) * 0.8);
  --radius-lg: var(--radius);
  --radius-xl: calc(var(--radius) * 1.4);
  --radius-2xl: calc(var(--radius) * 1.8);
  --radius-3xl: calc(var(--radius) * 2.2);
  --radius-4xl: calc(var(--radius) * 2.6);
}

:root[data-theme="admin"] {
  --radius: 0.75rem;
  --background: #f2f3f2;
  --foreground: #111411;
  --card: #ffffff;
  --card-foreground: #111411;
  --popover: #ffffff;
  --popover-foreground: #111411;
  --primary: #16823a;
  --primary-foreground: #ffffff;
  --secondary: #e9ece9;
  --secondary-foreground: #111411;
  --muted: #eef0ee;
  --muted-foreground: #6b716c;
  --accent: #e6f4eb;
  --accent-foreground: #14532d;
  --destructive: #dc2626;
  --border: #e5e8e5;
  --input: #e5e8e5;
  --ring: #22a055;
  --chart-1: #1e6b35;
  --chart-2: #3f9d5e;
  --chart-3: #6cc08c;
  --chart-4: #2f4f38;
  --chart-5: #a7d9b8;
  --sidebar: #f7f8f7;
  --sidebar-foreground: #111411;
  --sidebar-primary: #16823a;
  --sidebar-primary-foreground: #ffffff;
  --sidebar-accent: #e6f4eb;
  --sidebar-accent-foreground: #14532d;
  --sidebar-border: #e5e8e5;
  --sidebar-ring: #22a055;
  /* Status pills: soft fill, strong text (Published, Draft, New…). */
  --success: #15803d;
  --success-soft: #dcfce7;
  --warning: #b45309;
  --warning-soft: #fef3c7;
  --danger: #b91c1c;
  --danger-soft: #fee2e2;
  /* Comparison-period stripes in charts and legends. */
  --hatch: #8fcca5;
  /* Hero KPI card and the sidebar's "Your website" card. */
  --hero-from: #0f4d23;
  --hero-to: #2b6e3e;
  --hero-foreground: #ffffff;
  --promo-from: #06120a;
  --promo-to: #12381f;
  --promo-foreground: #ffffff;
}

:root[data-theme="admin"].dark {
  --background: #0c100d;
  --foreground: #eef2ee;
  --card: #151c17;
  --card-foreground: #eef2ee;
  --popover: #151c17;
  --popover-foreground: #eef2ee;
  --primary: #34b567;
  --primary-foreground: #06130a;
  --secondary: #1c241e;
  --secondary-foreground: #eef2ee;
  --muted: #1c241e;
  --muted-foreground: #9aa39c;
  --accent: #18301f;
  --accent-foreground: #c9ecd6;
  --destructive: #f87171;
  --border: #243026;
  --input: #243026;
  --ring: #3fcb75;
  --chart-1: #3fcb75;
  --chart-2: #6cc08c;
  --chart-3: #a7d9b8;
  --chart-4: #2f7a45;
  --chart-5: #1f4a2b;
  --sidebar: #111712;
  --sidebar-foreground: #eef2ee;
  --sidebar-primary: #34b567;
  --sidebar-primary-foreground: #06130a;
  --sidebar-accent: #18301f;
  --sidebar-accent-foreground: #c9ecd6;
  --sidebar-border: #243026;
  --sidebar-ring: #3fcb75;
  --success: #4ade80;
  --success-soft: #12301c;
  --warning: #fbbf24;
  --warning-soft: #33270c;
  --danger: #f87171;
  --danger-soft: #3a1414;
  --hatch: #2f7a45;
}

@utility bg-hatch {
  background-image: repeating-linear-gradient(
    -45deg,
    var(--hatch) 0 3px,
    transparent 3px 7px
  );
}

@utility bg-hero {
  background-image: linear-gradient(160deg, var(--hero-from), var(--hero-to));
  color: var(--hero-foreground);
}

@utility bg-promo {
  background-image: linear-gradient(
    160deg,
    var(--promo-from),
    var(--promo-to)
  );
  color: var(--promo-foreground);
}

@layer base {
  * {
    @apply border-border outline-ring/50;
  }
  body {
    @apply bg-background font-sans text-foreground antialiased;
  }
  button:not(:disabled),
  [role="button"]:not(:disabled) {
    cursor: pointer;
  }
}
```

- [ ] **Step 5: Scope the site stylesheet**

In `app/(site)/site.css`, directly after the three `@import` lines (before `@custom-variant dark …`), add:

```css
/* The admin UI and the planning docs never render on the public site. */
@source not "../../admin";
@source not "../../docs";
```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `pnpm vitest run --project unit tests/css`
Expected: PASS (both files; 26 contrast checks across light and dark). Tailwind skips git-ignored paths (including `.git/info/exclude`); if a sentinel shows up anyway, an untracked copy of the repo (such as a local `.kilo/worktrees/` checkout) is being scanned — exclude it there, not in the test.

- [ ] **Step 7: Prove the public site did not move**

Run: `pnpm build && pnpm test:parity`
Expected: PASS on desktop, tablet and mobile, ≤ 0.1% difference on every route (only unused selectors left the site CSS).

- [ ] **Step 8: Gates**

Run: `pnpm lint && pnpm typecheck && pnpm test:unit`
Expected: all green.

- [ ] **Step 9: Commit**

```bash
git add "app/(admin)/admin.css" "app/(site)/site.css" tests/css package.json pnpm-lock.yaml
git commit -m "feat(admin): complete Evergreen tokens and scope Tailwind sources per stylesheet"
```

---

## Task 2.2: Light, dark and system theme switching

**Files:**
- Modify: `components/theme-provider.tsx` (whole file)
- Modify: `app/(admin)/layout.tsx`
- Create: `components/theme-provider.test.tsx`
- Create: `components/ui/sonner.test.tsx`
- Create: `tests/e2e/admin/theme.spec.ts`
- Create: `tests/setup/dom.ts`; Modify: `vitest.config.ts` (unit project `setupFiles`)
- Modify: `package.json` (devDependencies `@testing-library/react@16.3.3`, `@testing-library/dom@10.4.2`, `@testing-library/user-event@14.6.7`)

**Interfaces:**
- Consumes: Task 2.1 dark tokens.
- Produces:
  - `ThemeProvider` (named export, `components/theme-provider.tsx`): next-themes with `attribute="class"`, `defaultTheme="system"`, `enableSystem`, `disableTransitionOnChange`, `storageKey="mk-admin-theme"`.
  - `ADMIN_THEME_STORAGE_KEY = "mk-admin-theme"` (same file) — E2E and the user menu rely on it.
  - `useTheme()` from `next-themes` works anywhere under the admin root layout; the shared `Toaster` follows it.

- [ ] **Step 1: Install Testing Library and give jsdom tests a clean DOM**

Run: `pnpm add -D @testing-library/react@16.3.3 @testing-library/dom@10.4.2 @testing-library/user-event@14.6.7`
Expected: three devDependencies added.

Vitest runs without globals, so Testing Library cannot register its own cleanup. Create `tests/setup/dom.ts`:

```ts
import { afterEach } from "vitest"

// Only jsdom files have a document; node-environment tests skip all of this.
if (typeof window !== "undefined") {
  // jsdom has no ResizeObserver; Base UI (scroll area, popups) and Recharts
  // expect one.
  globalThis.ResizeObserver ??= class {
    observe() {}
    unobserve() {}
    disconnect() {}
  }
}

afterEach(async () => {
  if (typeof document === "undefined") return
  const { cleanup } = await import("@testing-library/react")
  cleanup()
})
```

In `vitest.config.ts`, add `setupFiles: ["tests/setup/dom.ts"],` to the `unit` project's `test` block (after `environment: "node",`).

- [ ] **Step 2: Write the failing unit tests**

```tsx
// components/theme-provider.test.tsx
// @vitest-environment jsdom
import { render, screen, waitFor } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import { useTheme } from "next-themes"
import { beforeEach, describe, expect, it, vi } from "vitest"

import { ADMIN_THEME_STORAGE_KEY, ThemeProvider } from "./theme-provider"

// jsdom has no matchMedia; next-themes reads it for "system".
function preferDark(dark: boolean) {
  Object.defineProperty(window, "matchMedia", {
    configurable: true,
    writable: true,
    value: vi.fn((query: string) => ({
      matches: dark && query.includes("dark"),
      media: query,
      onchange: null,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
      addListener: vi.fn(),
      removeListener: vi.fn(),
      dispatchEvent: vi.fn(),
    })),
  })
}

function Probe() {
  const { theme, resolvedTheme } = useTheme()
  return <p data-testid="probe">{`${theme}/${resolvedTheme}`}</p>
}

const html = () => document.documentElement

beforeEach(() => {
  localStorage.clear()
  html().className = ""
  preferDark(false)
})

describe("ThemeProvider", () => {
  it("defaults to the system theme", async () => {
    render(
      <ThemeProvider>
        <Probe />
      </ThemeProvider>
    )
    await waitFor(() =>
      expect(screen.getByTestId("probe").textContent).toBe("system/light")
    )
    expect(html().classList.contains("light")).toBe(true)
  })

  it("follows a dark system preference", async () => {
    preferDark(true)
    render(
      <ThemeProvider>
        <Probe />
      </ThemeProvider>
    )
    await waitFor(() => expect(html().classList.contains("dark")).toBe(true))
  })

  it("restores a saved choice from its own storage key", async () => {
    localStorage.setItem(ADMIN_THEME_STORAGE_KEY, "dark")
    render(
      <ThemeProvider>
        <Probe />
      </ThemeProvider>
    )
    await waitFor(() =>
      expect(screen.getByTestId("probe").textContent).toBe("dark/dark")
    )
  })

  // docs/plan.md Global Constraints: no single-key hotkeys.
  it("ignores a bare 'd' keypress", async () => {
    render(
      <ThemeProvider>
        <Probe />
      </ThemeProvider>
    )
    await waitFor(() => expect(html().classList.contains("light")).toBe(true))
    await userEvent.keyboard("d")
    expect(html().classList.contains("dark")).toBe(false)
    expect(screen.getByTestId("probe").textContent).toBe("system/light")
  })
})
```

```tsx
// components/ui/sonner.test.tsx
// @vitest-environment jsdom
import { act, render, waitFor } from "@testing-library/react"
import { toast } from "sonner"
import { beforeEach, expect, it, vi } from "vitest"

import { ThemeProvider } from "@/components/theme-provider"

import { Toaster } from "./sonner"

beforeEach(() => {
  localStorage.clear()
  Object.defineProperty(window, "matchMedia", {
    configurable: true,
    writable: true,
    value: vi.fn((query: string) => ({
      matches: false,
      media: query,
      onchange: null,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
      addListener: vi.fn(),
      removeListener: vi.fn(),
      dispatchEvent: vi.fn(),
    })),
  })
})

it("renders toasts in the admin's current theme", async () => {
  render(
    <ThemeProvider defaultTheme="dark">
      <Toaster />
    </ThemeProvider>
  )
  act(() => {
    toast("Saved")
  })
  await waitFor(() =>
    expect(
      document
        .querySelector("[data-sonner-toaster]")
        ?.getAttribute("data-sonner-theme")
    ).toBe("dark")
  )
})
```

- [ ] **Step 3: Run them to verify they fail**

Run: `pnpm vitest run --project unit components/theme-provider.test.tsx components/ui/sonner.test.tsx`
Expected: FAIL — `ADMIN_THEME_STORAGE_KEY` is not exported (`expected undefined`), and "ignores a bare 'd' keypress" fails because `ThemeHotkey` toggles to dark.

- [ ] **Step 4: Rewrite `components/theme-provider.tsx`**

```tsx
"use client"

import { ThemeProvider as NextThemesProvider } from "next-themes"
import type * as React from "react"

// The admin's own key, so a theme chosen here can never leak into anything
// else on this origin (the public site is light-only and has no provider).
export const ADMIN_THEME_STORAGE_KEY = "mk-admin-theme"

// Light, dark or system (docs/brief.md §9.4). There is deliberately no
// single-key hotkey: the theme changes from the user menu or ⌘K.
export function ThemeProvider({
  children,
  ...props
}: React.ComponentProps<typeof NextThemesProvider>) {
  return (
    <NextThemesProvider
      attribute="class"
      defaultTheme="system"
      enableSystem
      disableTransitionOnChange
      storageKey={ADMIN_THEME_STORAGE_KEY}
      {...props}
    >
      {children}
    </NextThemesProvider>
  )
}
```

- [ ] **Step 5: Wire it into the admin root layout**

Replace the body of `app/(admin)/layout.tsx` from the comment above `AdminRootLayout` to the end:

```tsx
// Separate root layout: no Lenis, GSAP or site chrome, and its own tokens.
// next-themes sets `.light`/`.dark` on <html> from an inline script before
// hydration, hence suppressHydrationWarning on that element only.
export default function AdminRootLayout({
  children,
}: Readonly<{ children: React.ReactNode }>) {
  return (
    <html
      lang="en-IE"
      data-theme="admin"
      className={cn(sans.variable, mono.variable)}
      suppressHydrationWarning
    >
      <body>
        <ThemeProvider>
          {children}
          <Toaster position="top-center" />
        </ThemeProvider>
      </body>
    </html>
  )
}
```

and add the import below the `Toaster` import:

```tsx
import { ThemeProvider } from "@/components/theme-provider"
```

- [ ] **Step 6: Run the unit tests to verify they pass**

Run: `pnpm vitest run --project unit components/theme-provider.test.tsx components/ui/sonner.test.tsx`
Expected: PASS (5 tests).

- [ ] **Step 7: Write the E2E pinning Review Focus #2**

```ts
// tests/e2e/admin/theme.spec.ts
import { expect, test } from "@playwright/test"

// Review Focus #2: the first paint must already be in the right theme, and
// React must not report a hydration mismatch on <html>.
test("a dark system preference paints dark before hydration", async ({
  page,
}) => {
  await page.emulateMedia({ colorScheme: "dark" })
  await page.addInitScript(() => {
    document.addEventListener("DOMContentLoaded", () => {
      const probe = window as unknown as Record<string, string>
      probe.__themeAtDomReady = document.documentElement.className
      probe.__bodyAtDomReady = getComputedStyle(document.body).backgroundColor
    })
  })
  const errors: string[] = []
  page.on("console", (message) => {
    if (message.type() === "error") errors.push(message.text())
  })

  await page.goto("/admin/sign-in")

  const probe = await page.evaluate(() => {
    const w = window as unknown as Record<string, string>
    return { theme: w.__themeAtDomReady, body: w.__bodyAtDomReady }
  })
  expect(probe.theme).toMatch(/\bdark\b/)
  expect(probe.body).toBe("rgb(12, 16, 13)")
  await expect(page.locator("html")).toHaveClass(/\bdark\b/)
  expect(errors.filter((text) => /hydrat/i.test(text))).toEqual([])
})

test("a saved light choice wins over a dark system preference", async ({
  page,
}) => {
  await page.emulateMedia({ colorScheme: "dark" })
  await page.addInitScript(() =>
    localStorage.setItem("mk-admin-theme", "light")
  )
  await page.goto("/admin/sign-in")
  await expect(page.locator("html")).toHaveClass(/\blight\b/)
  await expect(page.locator("html")).not.toHaveClass(/\bdark\b/)
})

test("the public site stays light under a dark preference", async ({
  page,
}) => {
  await page.emulateMedia({ colorScheme: "dark" })
  await page.goto("/")
  await expect(page.locator("html")).not.toHaveClass(/\bdark\b/)
})
```

- [ ] **Step 8: Run the E2E**

Run: `pnpm test:e2e tests/e2e/admin/theme.spec.ts`
Expected: PASS (3 tests, desktop project — `tests/e2e/admin/**` is desktop-only until Task 2.5).

- [ ] **Step 9: Gates**

Run: `pnpm lint && pnpm typecheck && pnpm test:unit && pnpm build`
Expected: all green.

- [ ] **Step 10: Commit**

```bash
git add components/theme-provider.tsx components/theme-provider.test.tsx components/ui/sonner.test.tsx "app/(admin)/layout.tsx" tests/e2e/admin/theme.spec.ts tests/setup vitest.config.ts package.json pnpm-lock.yaml
git commit -m "feat(admin): light, dark and system themes without a single-key hotkey"
```

---

## Task 2.3: Navigation registry, `requirePermission`, actor context and guarded section stubs

**Files:**
- Create: `lib/admin/nav.ts`, `lib/admin/nav.test.ts`
- Create: `lib/auth/me.ts`, `lib/auth/me.test.ts`
- Create: `server/modules/session/routes.ts`
- Modify: `server/api/routes/admin.ts`
- Modify: `server/auth/session.ts`, `server/auth/session.test.ts`
- Create: `admin/lib/actor-context.tsx`, `admin/lib/actor-context.test.tsx`
- Modify: `app/(admin)/admin/(panel)/layout.tsx`
- Create: `app/(admin)/admin/(panel)/[section]/[[...rest]]/page.tsx`
- Create: `app/(admin)/admin/(panel)/no-access/page.tsx`
- Create: `app/(admin)/admin/(panel)/not-found.tsx`
- Create: `app/(admin)/admin/(panel)/guarded-pages.test.ts`
- Create: `tests/integration/api/permission-matrix.test.ts`

**Interfaces:**
- Consumes: `hasPermission`, `Permissions`, `RoleName`, `ROLE_NAMES`, `permissionMap` (`lib/auth/permissions.ts`); `requireActor`, `getSession` (`server/auth/session.ts`); `safeNext` (`lib/auth/safe-next.ts`); `ADMIN_PATH_HEADER`.
- Produces:
  - `lib/admin/nav.ts`:
    ```ts
    type NavGroupId = "menu" | "growth" | "general"
    type NavLink = { id: string; label: string; href: string; permission: Permissions; keywords?: readonly string[] }
    type NavItem = NavLink & { group: NavGroupId; children?: readonly NavLink[]; badge?: "new-leads" }
    type Destination = NavLink & { parent: string | null }
    const NAV_GROUPS: readonly { id: NavGroupId; label: string }[]
    const NAV: readonly NavItem[]
    const HELP_LINK: NavLink
    const EXTRA_LINKS: readonly NavLink[]           // account, design
    function visibleNav(roles: readonly RoleName[]): NavItem[]
    function reachableLinks(roles: readonly RoleName[]): Destination[]
    function linkForPath(pathname: string): NavLink | undefined
    function isActivePath(pathname: string, href: string): boolean
    ```
  - `lib/auth/me.ts`: `type MePayload = { user: { id; email; name }; roles: RoleName[]; permissions: Permissions; twoFactorEnabled: boolean; impersonatedBy: string | null }`, `toMePayload(actor): MePayload`.
  - `server/auth/session.ts`: `NO_ACCESS_PATH = "/admin/no-access"`, `requirePermission(permissions: Permissions, next?: string): Promise<Actor>`.
  - `server/modules/session/routes.ts`: `meRoutes`, `type MeRoutes = typeof meRoutes`.
  - `admin/lib/actor-context.tsx`: `AdminActorProvider({ value: MePayload, children })`, `useActor(): MePayload`, `usePermission(permissions: Permissions): boolean`.

- [ ] **Step 1: Write the failing nav tests**

```ts
// lib/admin/nav.test.ts
import { describe, expect, it } from "vitest"

import { hasPermission, ROLE_NAMES, type RoleName } from "@/lib/auth/permissions"

import {
  EXTRA_LINKS,
  HELP_LINK,
  isActivePath,
  linkForPath,
  NAV,
  reachableLinks,
  visibleNav,
} from "./nav"

const ids = (role: RoleName) => visibleNav([role]).map((item) => item.id)

describe("visibleNav (docs/brief.md §7.2, §9.1)", () => {
  it("shows intake only the dashboard and enquiries", () => {
    expect(ids("intake")).toEqual(["dashboard", "leads"])
  })

  it("shows a viewer read-only content sections and no settings", () => {
    expect(ids("viewer")).toEqual([
      "dashboard",
      "pages",
      "insights",
      "collections",
      "seo",
    ])
    const seo = visibleNav(["viewer"]).find((item) => item.id === "seo")
    expect(seo?.children?.map((child) => child.id)).toEqual([
      "seo-overview",
      "seo-defaults",
    ])
  })

  it("keeps enquiries away from a marketer", () => {
    expect(ids("marketer")).toEqual([
      "dashboard",
      "pages",
      "insights",
      "collections",
      "media",
      "newsletter",
      "seo",
      "marketing",
    ])
    const marketing = visibleNav(["marketer"]).find(
      (item) => item.id === "marketing"
    )
    expect(marketing?.children?.map((child) => child.id)).toEqual([
      "marketing-tracking",
      "marketing-consent",
    ])
  })

  it("shows an owner everything, and an admin everything but custom code", () => {
    expect(ids("owner")).toEqual(NAV.map((item) => item.id))
    const marketing = visibleNav(["admin"]).find(
      (item) => item.id === "marketing"
    )
    expect(marketing?.children?.map((child) => child.id)).not.toContain(
      "marketing-code"
    )
  })

  it("merges several roles", () => {
    expect(visibleNav(["intake", "marketer"]).map((item) => item.id)).toContain(
      "leads"
    )
  })
})

// Review Focus #1: the sidebar, ⌘K and the page guard share one source, so
// what is listed is exactly what opens.
describe.each(ROLE_NAMES)("%s: listing and guard agree", (role) => {
  const listed = new Set(reachableLinks([role]).map((link) => link.href))
  const everyHref = [
    ...NAV.flatMap((item) => [item, ...(item.children ?? [])]),
    HELP_LINK,
    ...EXTRA_LINKS,
  ].map((link) => link.href)

  it.each(everyHref)("%s", (href) => {
    const guard = linkForPath(href)
    expect(guard).toBeDefined()
    expect(listed.has(href)).toBe(hasPermission([role], guard!.permission))
  })
})

describe("reachableLinks", () => {
  it("lists each destination once, with its section", () => {
    const links = reachableLinks(["owner"])
    expect(new Set(links.map((link) => link.href)).size).toBe(links.length)
    expect(links.find((link) => link.id === "seo-redirects")?.parent).toBe(
      "SEO"
    )
    expect(links.find((link) => link.id === "dashboard")?.parent).toBeNull()
  })

  it("includes Help and Account for everyone, Design only for owners", () => {
    const intake = reachableLinks(["intake"]).map((link) => link.id)
    expect(intake).toEqual(
      expect.arrayContaining(["dashboard", "leads", "help", "account"])
    )
    expect(intake).not.toContain("design")
    expect(reachableLinks(["owner"]).map((link) => link.id)).toContain(
      "design"
    )
  })
})

describe("linkForPath", () => {
  it("matches hrefs exactly", () => {
    expect(linkForPath("/admin/seo/redirects")?.id).toBe("seo-redirects")
    expect(linkForPath("/admin/seo/redirects/123")).toBeUndefined()
    expect(linkForPath("/admin/nope")).toBeUndefined()
  })
})

describe("isActivePath", () => {
  it("highlights the dashboard only on /admin", () => {
    expect(isActivePath("/admin", "/admin")).toBe(true)
    expect(isActivePath("/admin/pages", "/admin")).toBe(false)
  })

  it("highlights a section on its sub-pages, not on look-alikes", () => {
    expect(isActivePath("/admin/seo/defaults", "/admin/seo")).toBe(true)
    expect(isActivePath("/admin/seo-tools", "/admin/seo")).toBe(false)
  })
})
```

```ts
// lib/auth/me.test.ts
import { expect, it } from "vitest"

import { toMePayload } from "./me"

it("maps an actor to the /me payload with merged permissions", () => {
  const payload = toMePayload({
    userId: "u1",
    email: "v@example.com",
    name: "Vera Viewer",
    roles: ["viewer"],
    twoFactorEnabled: false,
    impersonatedBy: null,
  })
  expect(payload).toEqual({
    user: { id: "u1", email: "v@example.com", name: "Vera Viewer" },
    roles: ["viewer"],
    permissions: expect.objectContaining({ page: ["read"] }),
    twoFactorEnabled: false,
    impersonatedBy: null,
  })
  expect(payload.permissions.lead).toBeUndefined()
})
```

- [ ] **Step 2: Run them to verify they fail**

Run: `pnpm vitest run --project unit lib/admin lib/auth/me.test.ts`
Expected: FAIL with `Failed to resolve import "./nav"` and `"./me"`.

- [ ] **Step 3: Implement `lib/admin/nav.ts`**

```ts
import {
  hasPermission,
  type Permissions,
  type RoleName,
} from "@/lib/auth/permissions"

// docs/brief.md §9.1. One isomorphic registry feeds the sidebar, the ⌘K
// search API and the page guard of every not-yet-built section, so a link is
// listed exactly when its page opens (Review Focus #1).
export type NavGroupId = "menu" | "growth" | "general"

export type NavLink = {
  id: string
  label: string
  href: string
  permission: Permissions
  keywords?: readonly string[]
}

export type NavItem = NavLink & {
  group: NavGroupId
  children?: readonly NavLink[]
  // Phase 6 fills this with the new-enquiry count.
  badge?: "new-leads"
}

export type Destination = NavLink & { parent: string | null }

export const NAV_GROUPS: readonly { id: NavGroupId; label: string }[] = [
  { id: "menu", label: "Menu" },
  { id: "growth", label: "Growth" },
  { id: "general", label: "General" },
]

const collection = (slug: string, label: string): NavLink => ({
  id: `collections-${slug}`,
  label,
  href: `/admin/collections/${slug}`,
  permission: { collection: ["read"] },
})

const setting = (slug: string, label: string): NavLink => ({
  id: `settings-${slug}`,
  label,
  href: `/admin/settings/${slug}`,
  permission: { settings: ["read"] },
})

export const NAV: readonly NavItem[] = [
  {
    id: "dashboard",
    group: "menu",
    label: "Dashboard",
    href: "/admin",
    permission: { dashboard: ["view"] },
    keywords: ["home", "overview"],
  },
  {
    id: "pages",
    group: "menu",
    label: "Pages",
    href: "/admin/pages",
    permission: { page: ["read"] },
    keywords: ["home page", "about", "services", "builder"],
  },
  {
    id: "insights",
    group: "menu",
    label: "Insights",
    href: "/admin/insights",
    permission: { article: ["read"] },
    keywords: ["blog", "articles", "posts"],
    children: [
      {
        id: "insights-articles",
        label: "Articles",
        href: "/admin/insights",
        permission: { article: ["read"] },
      },
      {
        id: "insights-categories",
        label: "Categories",
        href: "/admin/insights/categories",
        permission: { article: ["read"] },
      },
      {
        id: "insights-authors",
        label: "Authors",
        href: "/admin/insights/authors",
        permission: { article: ["read"] },
      },
    ],
  },
  {
    id: "collections",
    group: "menu",
    label: "Collections",
    href: "/admin/collections",
    permission: { collection: ["read"] },
    keywords: ["services", "plans", "testimonials", "faqs"],
    children: [
      collection("services", "Services"),
      collection("plans", "Plans"),
      collection("testimonials", "Testimonials"),
      collection("faqs", "FAQs"),
      collection("steps", "Steps"),
      collection("cta-bands", "CTA bands"),
    ],
  },
  {
    id: "media",
    group: "menu",
    label: "Media",
    href: "/admin/media",
    permission: { media: ["read"] },
    keywords: ["images", "uploads", "library"],
  },
  {
    id: "leads",
    group: "menu",
    label: "Enquiries",
    href: "/admin/leads",
    permission: { lead: ["read"] },
    keywords: ["leads", "contact", "inbox", "messages"],
    badge: "new-leads",
  },
  {
    id: "newsletter",
    group: "menu",
    label: "Newsletter",
    href: "/admin/newsletter",
    permission: { newsletter: ["read"] },
    keywords: ["subscribers", "email list"],
  },
  {
    id: "seo",
    group: "growth",
    label: "SEO",
    href: "/admin/seo",
    permission: { seo: ["read"] },
    keywords: ["search", "google", "meta", "sitemap"],
    children: [
      {
        id: "seo-overview",
        label: "Overview",
        href: "/admin/seo",
        permission: { seo: ["read"] },
      },
      {
        id: "seo-defaults",
        label: "Defaults",
        href: "/admin/seo/defaults",
        permission: { seo: ["read"] },
      },
      {
        id: "seo-redirects",
        label: "Redirects",
        href: "/admin/seo/redirects",
        permission: { redirect: ["read"] },
      },
    ],
  },
  {
    id: "marketing",
    group: "growth",
    label: "Marketing",
    href: "/admin/marketing",
    permission: { tracking: ["read"] },
    keywords: ["analytics", "gtm", "ga4", "pixel", "tracking"],
    children: [
      {
        id: "marketing-tracking",
        label: "Tracking",
        href: "/admin/marketing",
        permission: { tracking: ["read"] },
      },
      {
        id: "marketing-consent",
        label: "Consent banner",
        href: "/admin/marketing/consent",
        permission: { tracking: ["read"] },
      },
      {
        id: "marketing-code",
        label: "Custom code",
        href: "/admin/marketing/code",
        permission: { code: ["update"] },
      },
    ],
  },
  {
    id: "appearance",
    group: "general",
    label: "Appearance",
    href: "/admin/appearance",
    permission: { appearance: ["read"] },
    keywords: ["theme", "colours", "colors", "brand", "logo"],
  },
  {
    id: "settings",
    group: "general",
    label: "Site settings",
    href: "/admin/settings",
    permission: { settings: ["read"] },
    keywords: ["identity", "contact", "footer", "navigation", "privacy"],
    children: [
      setting("identity", "Identity"),
      setting("contact", "Contact & clinic"),
      setting("navigation", "Navigation"),
      setting("footer", "Footer"),
      setting("forms", "Forms & microcopy"),
      setting("notifications", "Notifications"),
      setting("privacy", "Privacy"),
    ],
  },
  {
    id: "users",
    group: "general",
    label: "Users & roles",
    href: "/admin/users",
    permission: { user: ["list"] },
    keywords: ["team", "invite", "people", "permissions"],
  },
  {
    id: "activity",
    group: "general",
    label: "Activity log",
    href: "/admin/activity",
    permission: { audit: ["read"] },
    keywords: ["audit", "history", "changes"],
  },
]

// Sits below the groups with "Log out", outside the gated navigation.
export const HELP_LINK: NavLink = {
  id: "help",
  label: "Help",
  href: "/admin/help",
  permission: { dashboard: ["view"] },
  keywords: ["support", "guide", "how to"],
}

// Not in the sidebar: reached from the user menu and ⌘K. The design showcase
// is owner-only; code.update is the one permission only owners hold.
export const EXTRA_LINKS: readonly NavLink[] = [
  {
    id: "account",
    label: "Account",
    href: "/admin/account",
    permission: { dashboard: ["view"] },
    keywords: ["profile", "password", "sessions", "security", "two-factor"],
  },
  {
    id: "design",
    label: "Design system",
    href: "/admin/design",
    permission: { code: ["update"] },
    keywords: ["components", "showcase", "kpi", "chart"],
  },
]

const allowed = (roles: readonly RoleName[], link: NavLink) =>
  hasPermission(roles, link.permission)

export function visibleNav(roles: readonly RoleName[]): NavItem[] {
  return NAV.filter((item) => allowed(roles, item)).map((item) =>
    item.children
      ? {
          ...item,
          children: item.children.filter((child) => allowed(roles, child)),
        }
      : item
  )
}

// Every page the actor may open, once each (a section's first child often
// shares its href), labelled with its section for ⌘K.
export function reachableLinks(roles: readonly RoleName[]): Destination[] {
  const seen = new Set<string>()
  const out: Destination[] = []
  const add = (link: NavLink, parent: string | null) => {
    if (seen.has(link.href)) return
    seen.add(link.href)
    out.push({ ...link, parent })
  }
  for (const item of visibleNav(roles)) {
    add(item, null)
    for (const child of item.children ?? []) add(child, item.label)
  }
  for (const link of [HELP_LINK, ...EXTRA_LINKS]) {
    if (allowed(roles, link)) add(link, null)
  }
  return out
}

// The registry entry a path stands for. Exact matches only: a section's
// sub-pages are their own entries, and unknown paths are 404s.
export function linkForPath(pathname: string): NavLink | undefined {
  for (const item of NAV) {
    if (item.href === pathname) return item
    const child = item.children?.find((link) => link.href === pathname)
    if (child) return child
  }
  return [HELP_LINK, ...EXTRA_LINKS].find((link) => link.href === pathname)
}

export function isActivePath(pathname: string, href: string) {
  if (href === "/admin") return pathname === "/admin"
  return pathname === href || pathname.startsWith(`${href}/`)
}
```

- [ ] **Step 4: Implement `lib/auth/me.ts`**

```ts
import { permissionMap, type Permissions, type RoleName } from "./permissions"

// The `GET /api/v1/admin/me` body (docs/brief.md §8.3). The panel layout
// builds the same object on the server and hands it to the client, so the
// shell never waits on a request to know who is signed in.
export type MePayload = {
  user: { id: string; email: string; name: string }
  roles: RoleName[]
  permissions: Permissions
  twoFactorEnabled: boolean
  impersonatedBy: string | null
}

export function toMePayload(actor: {
  userId: string
  email: string
  name: string
  roles: RoleName[]
  twoFactorEnabled: boolean
  impersonatedBy: string | null
}): MePayload {
  return {
    user: { id: actor.userId, email: actor.email, name: actor.name },
    roles: actor.roles,
    permissions: permissionMap(actor.roles),
    twoFactorEnabled: actor.twoFactorEnabled,
    impersonatedBy: actor.impersonatedBy,
  }
}
```

- [ ] **Step 5: Run the unit tests to verify they pass**

Run: `pnpm vitest run --project unit lib/admin lib/auth/me.test.ts`
Expected: PASS.

- [ ] **Step 6: Move `/me` into its own sub-app**

```ts
// server/modules/session/routes.ts
import "server-only"

import { Hono } from "hono"

import { toMePayload } from "@/lib/auth/me"
import type { AppEnv } from "@/server/api/types"

// Mounted before the two-factor gate: an owner without 2FA can still learn
// who they are, so the client can send them to setup.
export const meRoutes = new Hono<AppEnv>().get("/", (c) =>
  c.json(toMePayload(c.get("actor")!))
)

export type MeRoutes = typeof meRoutes
```

Replace `server/api/routes/admin.ts`:

```ts
import "server-only"

import { Hono } from "hono"

import { auditRoutes } from "@/server/modules/audit/routes"
import { meRoutes } from "@/server/modules/session/routes"
import { usersRoutes } from "@/server/modules/users/routes"

import { session, signedIn, twoFactorComplete } from "../middleware/auth"
import { sameOrigin } from "../middleware/same-origin"
import type { AppEnv } from "../types"

export const adminRoutes = new Hono<AppEnv>()
  .use(session, signedIn, sameOrigin)
  .route("/me", meRoutes)
  // Everything below needs 2FA set up for owners and admins. /me stays above
  // it: its handler answers before this runs.
  .use(twoFactorComplete)
  .route("/audit", auditRoutes)
  .route("/users", usersRoutes)

export type AdminRoutes = typeof adminRoutes
```

Run: `pnpm test:integration tests/integration/api/me.test.ts tests/integration/api/two-factor.test.ts`
Expected: PASS (unchanged behaviour).

- [ ] **Step 7: Write the failing `requirePermission` tests**

Append to `server/auth/session.test.ts` (it already mocks `next/headers`, `next/navigation` and `./fresh-session`). Change the dynamic import line to:

```ts
const { requireActor, requirePermission, requireSignedIn } = await import(
  "./session"
)
```

and add at the end of the file:

```ts
describe("requirePermission", () => {
  beforeEach(() => {
    requestHeaders.delete("x-mk-admin-path")
  })

  it("returns the actor when a role grants the permission", async () => {
    current = signedInAs("viewer", false)
    await expect(requirePermission({ page: ["read"] })).resolves.toMatchObject(
      { roles: ["viewer"] }
    )
  })

  it("sends a role without it to the no-access screen, remembering where", async () => {
    current = signedInAs("viewer", false)
    requestHeaders.set("x-mk-admin-path", "/admin/leads?tab=new")
    await expect(requirePermission({ lead: ["read"] })).rejects.toThrow(
      "REDIRECT /admin/no-access?from=%2Fadmin%2Fleads%3Ftab%3Dnew"
    )
  })

  it("never echoes an unsafe path into the no-access link", async () => {
    current = signedInAs("viewer", false)
    requestHeaders.set("x-mk-admin-path", "//evil.example")
    await expect(requirePermission({ lead: ["read"] })).rejects.toThrow(
      "REDIRECT /admin/no-access?from=%2Fadmin"
    )
  })

  it("still enforces sign-in and two-factor first", async () => {
    current = null
    await expect(requirePermission({ page: ["read"] })).rejects.toThrow(
      /^REDIRECT \/admin\/sign-in/
    )
    current = signedInAs("owner", false)
    await expect(requirePermission({ page: ["read"] })).rejects.toThrow(
      "REDIRECT /admin/two-factor-setup"
    )
  })
})
```

Run: `pnpm vitest run --project unit server/auth/session.test.ts`
Expected: FAIL with `requirePermission is not a function`.

- [ ] **Step 8: Implement `requirePermission`**

In `server/auth/session.ts` add to the imports:

```ts
import { hasPermission, type Permissions } from "@/lib/auth/permissions"
```

and append:

```ts
export const NO_ACCESS_PATH = "/admin/no-access"

// Page-level gate (docs/brief.md §7.3, layer 2): spares a user a page of
// 403s. The API still authorises every call on its own.
export async function requirePermission(
  permissions: Permissions,
  next?: string
): Promise<Actor> {
  const actor = await requireActor(next)
  if (!hasPermission(actor.roles, permissions)) {
    const from = safeNext(next ?? (await headers()).get(ADMIN_PATH_HEADER))
    redirect(`${NO_ACCESS_PATH}?from=${encodeURIComponent(from)}`)
  }
  return actor
}
```

Run: `pnpm vitest run --project unit server/auth/session.test.ts`
Expected: PASS.

- [ ] **Step 9: Write the failing actor-context test**

```tsx
// admin/lib/actor-context.test.tsx
// @vitest-environment jsdom
import { renderHook } from "@testing-library/react"
import { expect, it } from "vitest"

import { toMePayload } from "@/lib/auth/me"

import { AdminActorProvider, useActor, usePermission } from "./actor-context"

const viewer = toMePayload({
  userId: "u1",
  email: "v@example.com",
  name: "Vera Viewer",
  roles: ["viewer"],
  twoFactorEnabled: false,
  impersonatedBy: null,
})

const wrapper = ({ children }: { children: React.ReactNode }) => (
  <AdminActorProvider value={viewer}>{children}</AdminActorProvider>
)

it("answers permission checks for the signed-in actor", () => {
  const read = renderHook(() => usePermission({ page: ["read"] }), { wrapper })
  const write = renderHook(() => usePermission({ page: ["update"] }), {
    wrapper,
  })
  expect(read.result.current).toBe(true)
  expect(write.result.current).toBe(false)
})

it("exposes the /me payload", () => {
  const { result } = renderHook(() => useActor(), { wrapper })
  expect(result.current.user.name).toBe("Vera Viewer")
})

it("fails loudly outside the provider", () => {
  expect(() => renderHook(() => useActor())).toThrow(/AdminActorProvider/)
})
```

Run: `pnpm vitest run --project unit admin/lib/actor-context.test.tsx`
Expected: FAIL with `Failed to resolve import "./actor-context"`.

- [ ] **Step 10: Implement `admin/lib/actor-context.tsx`**

```tsx
"use client"

import { createContext, use } from "react"

import type { MePayload } from "@/lib/auth/me"
import { hasPermission, type Permissions } from "@/lib/auth/permissions"

const ActorContext = createContext<MePayload | null>(null)

export function AdminActorProvider({
  value,
  children,
}: {
  value: MePayload
  children: React.ReactNode
}) {
  return <ActorContext value={value}>{children}</ActorContext>
}

export function useActor(): MePayload {
  const actor = use(ActorContext)
  if (!actor) throw new Error("useActor must be used inside AdminActorProvider")
  return actor
}

// Hides or disables controls. Convenience only, never security: the API and
// requirePermission decide (docs/brief.md §7.3, layer 5).
export function usePermission(permissions: Permissions): boolean {
  return hasPermission(useActor().roles, permissions)
}
```

Run: `pnpm vitest run --project unit admin/lib/actor-context.test.tsx`
Expected: PASS.

- [ ] **Step 11: Give the panel layout the actor**

Replace `Guarded` in `app/(admin)/admin/(panel)/layout.tsx` and add the imports:

```tsx
import { AdminActorProvider } from "@/admin/lib/actor-context"
import { toMePayload } from "@/lib/auth/me"
```

```tsx
async function Guarded({ children }: { children: React.ReactNode }) {
  const actor = await requireActor()
  return (
    <AdminActorProvider value={toMePayload(actor)}>
      <div className="min-h-svh bg-background p-3 sm:p-4">{children}</div>
    </AdminActorProvider>
  )
}
```

- [ ] **Step 12: Add the guarded stubs, no-access screen and panel 404**

```tsx
// app/(admin)/admin/(panel)/[section]/[[...rest]]/page.tsx
import type { Metadata } from "next"
import Link from "next/link"
import { notFound } from "next/navigation"
import { Suspense } from "react"

import { linkForPath } from "@/lib/admin/nav"
import { requireActor, requirePermission } from "@/server/auth/session"

export const metadata: Metadata = { title: "Coming soon" }

type Params = { section: string; rest?: string[] }

// Every sidebar destination that a later phase has not built yet lands here.
// A real route folder (e.g. pages/page.tsx) takes precedence over this
// dynamic one as soon as it exists.
export default function SectionPage({ params }: { params: Promise<Params> }) {
  return (
    <Suspense fallback={<div className="h-48 animate-pulse rounded-card" />}>
      <Section params={params} />
    </Suspense>
  )
}

async function Section({ params }: { params: Promise<Params> }) {
  const { section, rest = [] } = await params
  const path = ["/admin", section, ...rest].join("/")
  // Signed in first, so a 404 never tells a stranger which paths exist.
  await requireActor(path)
  const link = linkForPath(path)
  if (!link) notFound()
  await requirePermission(link.permission, path)
  return (
    <section
      aria-labelledby="section-title"
      className="flex flex-col items-start gap-4"
    >
      <h1 id="section-title" className="text-title">
        {link.label}
      </h1>
      <p className="max-w-prose text-sm text-muted-foreground">
        This part of the admin arrives in a later release. Everything you can
        see in the menu is already set up for your role.
      </p>
      <Link
        href="/admin"
        className="text-sm font-semibold text-primary underline-offset-4 hover:underline"
      >
        Back to the dashboard
      </Link>
    </section>
  )
}
```

```tsx
// app/(admin)/admin/(panel)/no-access/page.tsx
import type { Metadata } from "next"
import Link from "next/link"
import { Suspense } from "react"

import { linkForPath } from "@/lib/admin/nav"
import { safeNext } from "@/lib/auth/safe-next"
import { requireActor } from "@/server/auth/session"

export const metadata: Metadata = { title: "No access" }

type SearchParams = Promise<Record<string, string | string[] | undefined>>

// Where requirePermission sends a role that may not open a page
// (docs/brief.md §7.3).
export default function NoAccessPage({
  searchParams,
}: {
  searchParams: SearchParams
}) {
  return (
    <Suspense fallback={null}>
      <NoAccess searchParams={searchParams} />
    </Suspense>
  )
}

async function NoAccess({ searchParams }: { searchParams: SearchParams }) {
  await requireActor()
  const { from } = await searchParams
  const target = safeNext(typeof from === "string" ? from : null)
  const link = linkForPath(target.split(/[?#]/)[0])
  return (
    <section
      aria-labelledby="no-access-title"
      className="mx-auto flex max-w-md flex-col items-center gap-4 py-16 text-center"
    >
      <h1 id="no-access-title" className="text-card-title">
        You don&apos;t have access to {link ? link.label : "this page"}
      </h1>
      <p className="text-sm text-muted-foreground">
        Your role doesn&apos;t include it. Ask an owner or admin if you need
        it.
      </p>
      <Link
        href="/admin"
        className="text-sm font-semibold text-primary underline-offset-4 hover:underline"
      >
        Back to the dashboard
      </Link>
    </section>
  )
}
```

```tsx
// app/(admin)/admin/(panel)/not-found.tsx
import Link from "next/link"

export default function PanelNotFound() {
  return (
    <section className="mx-auto flex max-w-md flex-col items-center gap-4 py-16 text-center">
      <h1 className="text-card-title">That page doesn&apos;t exist</h1>
      <p className="text-sm text-muted-foreground">
        Check the address, or use the menu to find what you need.
      </p>
      <Link
        href="/admin"
        className="text-sm font-semibold text-primary underline-offset-4 hover:underline"
      >
        Back to the dashboard
      </Link>
    </section>
  )
}
```

- [ ] **Step 13: Write the "every page is guarded" check (carry-over from Tasks 1.9/1.11)**

```ts
// app/(admin)/admin/(panel)/guarded-pages.test.ts
import { readdirSync, readFileSync } from "node:fs"
import path from "node:path"

import { describe, expect, it } from "vitest"

const repo = path.join(import.meta.dirname, "../../../..")

function files(dir: string, match: (name: string) => boolean): string[] {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const full = path.join(dir, entry.name)
    if (entry.isDirectory()) return files(full, match)
    return match(entry.name) ? [full] : []
  })
}

const rel = (file: string) => path.relative(repo, file)

// proxy.ts only checks that a cookie exists. Every panel page and every
// Server Action must read the real session itself.
describe("panel pages read the session", () => {
  const pages = files(import.meta.dirname, (name) => name === "page.tsx")
  it.each(pages.map(rel))("%s", (file) => {
    expect(readFileSync(path.join(repo, file), "utf8")).toMatch(
      /\brequire(Actor|Permission)\(/
    )
  })
})

describe("Server Actions authorise themselves", () => {
  const sources = [
    ...files(path.join(repo, "admin"), (name) => /\.tsx?$/.test(name)),
    ...files(path.join(repo, "app/(admin)"), (name) => /\.tsx?$/.test(name)),
  ].filter((file) => /^\s*["']use server["']/m.test(readFileSync(file, "utf8")))

  // One test over the list (which is empty today) rather than it.each, so
  // the check exists before the first action does.
  it("every \"use server\" file calls requireActor or requirePermission", () => {
    const unguarded = sources
      .filter(
        (file) => !/\brequire(Actor|Permission)\(/.test(readFileSync(file, "utf8"))
      )
      .map(rel)
    expect(unguarded).toEqual([])
  })
})
```

Run: `pnpm vitest run --project unit "app/(admin)/admin/(panel)/guarded-pages.test.ts"`
Expected: PASS (`page.tsx`, `no-access/page.tsx`, `[section]/[[...rest]]/page.tsx` all call a guard; no `"use server"` files yet).

- [ ] **Step 14: Write the `can()` HTTP matrix (carry-over from Task 1.5)**

```ts
// tests/integration/api/permission-matrix.test.ts
import { afterAll, beforeEach, describe, expect, it } from "vitest"

import { ROLE_NAMES, type RoleName } from "@/lib/auth/permissions"
import { closeDb } from "@/server/db/client"

import {
  adminRequest,
  createUser,
  signIn,
  signInWithTwoFactor,
} from "../helpers/auth"
import { resetDb } from "../helpers/db"

beforeEach(resetDb)
afterAll(closeDb)

async function cookieFor(role: RoleName) {
  await createUser(role)
  const email = `${role}@example.com`
  return role === "owner" || role === "admin"
    ? signInWithTwoFactor(email)
    : signIn(email)
}

// Written out from docs/brief.md §7.2 rather than derived from the role
// definitions, so drift between code and spec fails here. 400 on invite
// means "past can(), stopped by validation" (the body is empty).
const MATRIX: Record<
  RoleName,
  { audit: number; invite: number }
> = {
  owner: { audit: 200, invite: 400 },
  admin: { audit: 200, invite: 400 },
  editor: { audit: 403, invite: 403 },
  author: { audit: 403, invite: 403 },
  marketer: { audit: 403, invite: 403 },
  intake: { audit: 403, invite: 403 },
  viewer: { audit: 403, invite: 403 },
}

describe.each(ROLE_NAMES)("can() for %s", (role) => {
  it(`GET /audit → ${MATRIX[role].audit}`, async () => {
    const response = await adminRequest("/audit", await cookieFor(role))
    expect(response.status).toBe(MATRIX[role].audit)
    if (response.status === 403) {
      expect((await response.json()).error.code).toBe("FORBIDDEN")
    }
  })

  it(`POST /users/invite → ${MATRIX[role].invite}`, async () => {
    const response = await adminRequest(
      "/users/invite",
      await cookieFor(role),
      { method: "POST", body: {} }
    )
    expect(response.status).toBe(MATRIX[role].invite)
  })
})

describe("can() without a session", () => {
  it.each(["/me", "/audit"])("GET %s → 401", async (path) => {
    expect((await adminRequest(path, "")).status).toBe(401)
  })
})
```

Run: `pnpm test:integration tests/integration/api/permission-matrix.test.ts`
Expected: PASS (16 tests). If a row fails, the role definitions drifted from §7.2: fix `lib/auth/permissions.ts`, not the matrix.

- [ ] **Step 15: Gates**

Run: `pnpm lint && pnpm typecheck && pnpm test:unit && pnpm test:integration && pnpm build`
Expected: all green; `pnpm build` lists `/admin/[section]/[[...rest]]` and `/admin/no-access` as dynamic (ƒ) routes.

- [ ] **Step 16: Commit**

```bash
git add lib/admin lib/auth/me.ts lib/auth/me.test.ts server/modules/session server/api/routes/admin.ts server/auth/session.ts server/auth/session.test.ts admin/lib/actor-context.tsx admin/lib/actor-context.test.tsx "app/(admin)/admin/(panel)" tests/integration/api/permission-matrix.test.ts
git commit -m "feat(admin): permission-aware nav registry, requirePermission and guarded section stubs"
```

---

## Task 2.4: Typed data layer (hc clients, ApiError, TanStack Query, nuqs) and the layering lint rule

**Files:**
- Modify: `package.json` (`@tanstack/react-query@5.104.1`)
- Create: `admin/lib/api-error.ts`
- Create: `admin/lib/api.ts`, `admin/lib/api.test.ts`
- Create: `admin/lib/query-client.ts`, `admin/lib/query-client.test.ts`
- Create: `admin/lib/query-keys.ts`, `admin/lib/query-keys.test.ts`
- Create: `admin/lib/query-provider.tsx`
- Create: `server/api/in-process.ts`
- Modify: `server/modules/audit/routes.ts` (export `AuditRoutes`)
- Modify: `app/(admin)/admin/(panel)/layout.tsx`
- Modify: `eslint.config.mjs`
- Create: `tests/lint/layering.test.ts`
- Create: `tests/integration/api/in-process.test.ts`

**Interfaces:**
- Consumes: `MeRoutes` (Task 2.3), `auditRoutes`, `app` (`server/api/app.ts`), `ErrorCode` (`server/api/errors.ts`, type only).
- Produces:
  - `admin/lib/api-error.ts`: `type ApiErrorCode = ErrorCode | "NETWORK"`; `class ApiError extends Error { code; status: number; requestId: string | null; fieldErrors: Record<string, string[]> }`.
  - `admin/lib/api.ts`: `meApi = hc<MeRoutes>("/api/v1/admin/me")`, `auditApi = hc<AuditRoutes>("/api/v1/admin/audit")`; `parseResponse(request)` (Hono's typed result, errors rethrown as `ApiError`); `toApiError(error: unknown): ApiError`. Later tasks add `searchApi` (2.6) and `dashboardApi` (2.8) here.
  - `admin/lib/query-client.ts`: `getQueryClient(): QueryClient` (new per server call, one per browser tab).
  - `admin/lib/query-keys.ts`: `queryKeys` (shape below) and `type AuditListParams = { page: number; pageSize: number; actor?: string; action?: string; from?: string; to?: string }`.
  - `admin/lib/query-provider.tsx`: `AdminQueryProvider({ children })` (QueryClientProvider + nuqs `NuqsAdapter`).
  - `server/api/in-process.ts`: `inProcessAuditApi(cookie?: string)` — an `hc<AuditRoutes>` that calls `app.request` with the current request's cookie.
  - `server/modules/audit/routes.ts`: `type AuditRoutes = typeof auditRoutes`.

- [ ] **Step 1: Install TanStack Query**

Run: `pnpm add @tanstack/react-query@5.104.1`
Expected: dependency added.

- [ ] **Step 2: Write the failing client tests**

```ts
// admin/lib/api.test.ts
import { Hono } from "hono"
import { hc } from "hono/client"
import { describe, expect, it } from "vitest"

import { parseResponse } from "./api"
import { ApiError } from "./api-error"

// Stands in for the API: same envelope (docs/brief.md §8.2), no database.
const fake = new Hono()
  .get("/ok", (c) => c.json({ hello: "world" }))
  .get("/forbidden", (c) =>
    c.json(
      {
        error: {
          code: "FORBIDDEN",
          message: "You don't have permission to do that",
          requestId: "r1",
        },
      },
      403
    )
  )
  .post("/invalid", (c) =>
    c.json(
      {
        error: {
          code: "VALIDATION_FAILED",
          message: "Some fields need attention",
          requestId: "r2",
          fieldErrors: { email: ["Enter a valid email"] },
        },
      },
      400
    )
  )
  .get("/gateway", (c) => c.html("<h1>Bad gateway</h1>", 502))

const client = hc<typeof fake>("http://test", { fetch: fake.request })

describe("parseResponse", () => {
  it("returns typed data on success", async () => {
    const data = await parseResponse(client.ok.$get())
    expect(data.hello).toBe("world")
  })

  it("turns the error envelope into an ApiError", async () => {
    const error = await parseResponse(client.forbidden.$get()).catch(
      (e: unknown) => e
    )
    expect(error).toBeInstanceOf(ApiError)
    expect(error).toMatchObject({
      code: "FORBIDDEN",
      status: 403,
      requestId: "r1",
      message: "You don't have permission to do that",
    })
  })

  it("keeps field errors for forms", async () => {
    const error = await parseResponse(client.invalid.$post()).catch(
      (e: unknown) => e
    )
    expect(error).toMatchObject({
      code: "VALIDATION_FAILED",
      fieldErrors: { email: ["Enter a valid email"] },
    })
  })

  it("maps a non-JSON 5xx to INTERNAL", async () => {
    const error = await parseResponse(client.gateway.$get()).catch(
      (e: unknown) => e
    )
    expect(error).toMatchObject({ code: "INTERNAL", status: 502 })
  })

  it("maps a request that never got an answer to NETWORK", async () => {
    const error = await parseResponse(
      Promise.reject(new TypeError("fetch failed"))
    ).catch((e: unknown) => e)
    expect(error).toMatchObject({ code: "NETWORK", status: 0 })
  })
})
```

```ts
// admin/lib/query-client.test.ts
import { describe, expect, it } from "vitest"

import { ApiError } from "./api-error"
import { getQueryClient } from "./query-client"

describe("getQueryClient on the server", () => {
  it("never shares a client between calls (requests, users)", () => {
    expect(getQueryClient()).not.toBe(getQueryClient())
  })

  it("does not retry 4xx answers, retries anything else once", () => {
    const retry = getQueryClient().getDefaultOptions().queries?.retry
    if (typeof retry !== "function") throw new Error("retry is not a function")
    const forbidden = new ApiError("FORBIDDEN", "No", 403)
    const down = new ApiError("UNAVAILABLE", "Down", 503)
    expect(retry(0, forbidden)).toBe(false)
    expect(retry(0, down)).toBe(true)
    expect(retry(1, down)).toBe(false)
  })
})
```

```ts
// admin/lib/query-keys.test.ts
import { expect, it } from "vitest"

import { queryKeys } from "./query-keys"

it("normalises search terms so equal searches share a cache entry", () => {
  expect(queryKeys.search("  Users ")).toEqual(queryKeys.search("users"))
})

it("nests every audit key under one prefix for invalidation", () => {
  const list = queryKeys.audit.list({ page: 2, pageSize: 20, action: "x" })
  expect(list.slice(0, 1)).toEqual([...queryKeys.audit.all])
  expect(list).not.toEqual(queryKeys.audit.list({ page: 1, pageSize: 20 }))
})
```

- [ ] **Step 3: Run them to verify they fail**

Run: `pnpm vitest run --project unit admin/lib/api.test.ts admin/lib/query-client.test.ts admin/lib/query-keys.test.ts`
Expected: FAIL with `Failed to resolve import "./api"` (and the other two modules).

- [ ] **Step 4: Implement the client modules**

```ts
// admin/lib/api-error.ts
import type { ErrorCode } from "@/server/api/errors"

// The API's error envelope (docs/brief.md §8.2) as the admin sees it, plus
// NETWORK for a request that never got an answer. Type-only import: the
// union stays in step with the server without bundling server code.
export type ApiErrorCode = ErrorCode | "NETWORK"

export class ApiError extends Error {
  constructor(
    readonly code: ApiErrorCode,
    message: string,
    readonly status: number,
    readonly requestId: string | null = null,
    readonly fieldErrors: Record<string, string[]> = {}
  ) {
    super(message)
    this.name = "ApiError"
  }
}
```

```ts
// admin/lib/api.ts
import {
  DetailedError,
  hc,
  parseResponse as parseHonoResponse,
  type ClientResponse,
} from "hono/client"

import type { AuditRoutes } from "@/server/modules/audit/routes"
import type { MeRoutes } from "@/server/modules/session/routes"

import { ApiError, type ApiErrorCode } from "./api-error"

// One client per sub-app keeps TypeScript fast (docs/brief.md §8.1). Same
// origin, so the session cookie flows without configuration.
export const meApi = hc<MeRoutes>("/api/v1/admin/me")
export const auditApi = hc<AuditRoutes>("/api/v1/admin/audit")

type Envelope = {
  error?: {
    code?: string
    message?: string
    requestId?: string
    fieldErrors?: Record<string, string[]>
  }
}

const UNREACHABLE =
  "Couldn't reach the server. Check your connection and try again."

export function toApiError(error: unknown): ApiError {
  if (error instanceof ApiError) return error
  if (error instanceof DetailedError) {
    const status = Number(error.statusCode ?? 0)
    const data = (error.detail as { data?: unknown } | undefined)?.data
    const envelope =
      typeof data === "object" && data !== null
        ? (data as Envelope).error
        : undefined
    const code = (envelope?.code ??
      (status >= 500 ? "INTERNAL" : "BAD_REQUEST")) as ApiErrorCode
    return new ApiError(
      code,
      envelope?.message ?? "Something went wrong",
      status,
      envelope?.requestId ?? null,
      envelope?.fieldErrors ?? {}
    )
  }
  // fetch() only rejects when no answer arrived at all.
  return new ApiError("NETWORK", UNREACHABLE, 0)
}

// Hono's typed parser, with failures as ApiError so queries, forms and
// toasts handle one error type.
export async function parseResponse<T extends ClientResponse<unknown>>(
  request: T | Promise<T>
) {
  try {
    return await parseHonoResponse(request)
  } catch (error) {
    throw toApiError(error)
  }
}
```

```ts
// admin/lib/query-client.ts
import {
  defaultShouldDehydrateQuery,
  environmentManager,
  QueryClient,
} from "@tanstack/react-query"

import { ApiError } from "./api-error"

function makeQueryClient() {
  return new QueryClient({
    defaultOptions: {
      queries: {
        // Data prefetched on the server is not refetched the moment it
        // hydrates.
        staleTime: 30_000,
        // A 4xx will not fix itself on a retry; anything else gets one.
        retry: (failures, error) =>
          !(error instanceof ApiError && error.status >= 400 && error.status < 500) &&
          failures < 1,
      },
      dehydrate: {
        // Prefetches that are still running stream to the browser too.
        shouldDehydrateQuery: (query) =>
          defaultShouldDehydrateQuery(query) || query.state.status === "pending",
      },
    },
  })
}

let browserClient: QueryClient | undefined

// A fresh client per server call (never shared between users) and one per
// browser tab, as in TanStack Query's App Router guide.
export function getQueryClient() {
  if (environmentManager.isServer()) return makeQueryClient()
  browserClient ??= makeQueryClient()
  return browserClient
}
```

```ts
// admin/lib/query-keys.ts
// The admin's query-key factory (docs/brief.md §8.4). Table state from the
// URL is part of the key, so each filter combination is its own entry.
export type AuditListParams = {
  page: number
  pageSize: number
  actor?: string
  action?: string
  from?: string
  to?: string
}

export const queryKeys = {
  me: ["me"] as const,
  search: (q: string) => ["search", q.trim().toLowerCase()] as const,
  dashboard: ["dashboard"] as const,
  audit: {
    all: ["audit"] as const,
    list: (params: AuditListParams) => ["audit", "list", params] as const,
    facets: ["audit", "facets"] as const,
  },
  account: {
    sessions: ["account", "sessions"] as const,
  },
}
```

```tsx
// admin/lib/query-provider.tsx
"use client"

import { QueryClientProvider } from "@tanstack/react-query"
import { NuqsAdapter } from "nuqs/adapters/next/app"

import { getQueryClient } from "./query-client"

export function AdminQueryProvider({ children }: { children: React.ReactNode }) {
  return (
    <QueryClientProvider client={getQueryClient()}>
      <NuqsAdapter>{children}</NuqsAdapter>
    </QueryClientProvider>
  )
}
```

In `server/modules/audit/routes.ts` append:

```ts
export type AuditRoutes = typeof auditRoutes
```

- [ ] **Step 5: Run the unit tests to verify they pass**

Run: `pnpm vitest run --project unit admin/lib`
Expected: PASS.

- [ ] **Step 6: Write the failing in-process test**

```ts
// tests/integration/api/in-process.test.ts
import { afterAll, beforeEach, describe, expect, it } from "vitest"

import { parseResponse } from "@/admin/lib/api"
import { ApiError } from "@/admin/lib/api-error"
import { inProcessAuditApi } from "@/server/api/in-process"
import { closeDb } from "@/server/db/client"

import { createUser, signIn, signInWithTwoFactor } from "../helpers/auth"
import { resetDb } from "../helpers/db"

beforeEach(resetDb)
afterAll(closeDb)

describe("inProcessAuditApi", () => {
  it("returns exactly the JSON the browser would get", async () => {
    await createUser("owner")
    const cookie = await signInWithTwoFactor("owner@example.com")
    const page = await parseResponse(
      inProcessAuditApi(cookie).index.$get({
        query: { page: "1", pageSize: "5" },
      })
    )
    expect(page).toMatchObject({ page: 1, pageSize: 5 })
    expect(page.items.length).toBeGreaterThan(0)
    // Serialised like the HTTP response: ISO strings, not Date objects.
    expect(typeof page.items[0].createdAt).toBe("string")
  })

  it("goes through the same authorisation", async () => {
    await createUser("viewer")
    const cookie = await signIn("viewer@example.com")
    const error = await parseResponse(
      inProcessAuditApi(cookie).index.$get({ query: {} })
    ).catch((e: unknown) => e)
    expect(error).toBeInstanceOf(ApiError)
    expect(error).toMatchObject({ code: "FORBIDDEN", status: 403 })
  })
})
```

Run: `pnpm test:integration tests/integration/api/in-process.test.ts`
Expected: FAIL with `Failed to resolve import "@/server/api/in-process"`.

- [ ] **Step 7: Implement `server/api/in-process.ts`**

```ts
import "server-only"

import { hc } from "hono/client"
import { headers } from "next/headers"

import type { AuditRoutes } from "@/server/modules/audit/routes"

import { app } from "./app"

// Admin RSC prefetches call the Hono stack in-process: the same session,
// 2FA gate, can() and Zod as the browser, no network hop, and data in
// exactly the shape the client queries expect. Never for public pages.
const ORIGIN = "http://in-process"

async function forwarded(cookie?: string) {
  return { cookie: cookie ?? (await headers()).get("cookie") ?? "" }
}

export function inProcessAuditApi(cookie?: string) {
  return hc<AuditRoutes>(`${ORIGIN}/api/v1/admin/audit`, {
    fetch: app.request,
    headers: () => forwarded(cookie),
  })
}
```

Run: `pnpm test:integration tests/integration/api/in-process.test.ts`
Expected: PASS.

- [ ] **Step 8: Write the failing layering test (docs/brief.md §5.4)**

```ts
// tests/lint/layering.test.ts
import path from "node:path"

import { ESLint } from "eslint"
import { describe, expect, it } from "vitest"

const eslint = new ESLint({ cwd: path.join(import.meta.dirname, "../..") })

async function restricted(code: string, filePath: string) {
  const [result] = await eslint.lintText(code, { filePath })
  return result.messages.filter(
    (message) => message.ruleId === "@typescript-eslint/no-restricted-imports"
  )
}

const VALUE = 'import { getDb } from "@/server/db/client"\nexport const db = getDb\n'
const TYPE =
  'import type { Actor } from "@/server/auth/actor"\nexport type A = Actor\n'

describe("client and shared code never import server modules", () => {
  it.each([
    "admin/lib/probe.ts",
    "components/probe.tsx",
    "lib/probe.ts",
    "features/probe.ts",
  ])("refuses a value import in %s", async (file) => {
    expect(await restricted(VALUE, file)).toHaveLength(1)
  }, 30_000)

  it("allows type-only imports", async () => {
    expect(await restricted(TYPE, "admin/lib/probe.ts")).toHaveLength(0)
  }, 30_000)

  it("leaves routes free to call server code", async () => {
    expect(
      await restricted(VALUE, "app/(admin)/admin/(panel)/probe.tsx")
    ).toHaveLength(0)
  }, 30_000)
})
```

Run: `pnpm vitest run --project unit tests/lint`
Expected: FAIL — `refuses a value import` gets `expected [] to have a length of 1`.

- [ ] **Step 9: Add the rule**

In `eslint.config.mjs`, add this object to the `defineConfig([...])` array, after `...nextTs,`:

```js
  // docs/brief.md §5.4: dependencies point downward. Client and shared code
  // may use server types, never server values (they would pull secrets and
  // the database into the browser bundle).
  {
    files: [
      "admin/**/*.{ts,tsx}",
      "components/**/*.{ts,tsx}",
      "features/**/*.{ts,tsx}",
      "lib/**/*.{ts,tsx}",
      "blocks/**/*.{ts,tsx}",
    ],
    rules: {
      "@typescript-eslint/no-restricted-imports": [
        "error",
        {
          patterns: [
            {
              group: ["@/server", "@/server/**"],
              allowTypeImports: true,
              message:
                "Client and shared code must not import server modules (docs/brief.md §5.4). Type-only imports are fine.",
            },
          ],
        },
      ],
    },
  },
```

Run: `pnpm vitest run --project unit tests/lint && pnpm lint`
Expected: PASS, and `pnpm lint` reports no new errors (no existing file under those folders imports `@/server`).

- [ ] **Step 10: Provide the query client and URL state to every panel page**

In `app/(admin)/admin/(panel)/layout.tsx` import `AdminQueryProvider` from `@/admin/lib/query-provider` and wrap the inner `div`:

```tsx
    <AdminActorProvider value={toMePayload(actor)}>
      <AdminQueryProvider>
        <div className="min-h-svh bg-background p-3 sm:p-4">{children}</div>
      </AdminQueryProvider>
    </AdminActorProvider>
```

- [ ] **Step 11: Gates**

Run: `pnpm lint && pnpm typecheck && pnpm test:unit && pnpm test:integration && pnpm build`
Expected: all green.

- [ ] **Step 12: Commit**

```bash
git add admin/lib server/api/in-process.ts server/modules/audit/routes.ts "app/(admin)/admin/(panel)/layout.tsx" eslint.config.mjs tests/lint tests/integration/api/in-process.test.ts package.json pnpm-lock.yaml
git commit -m "feat(admin): typed hc clients, ApiError, query client and layering lint rule"
```

---

## Task 2.5: The shell — permission-filtered sidebar, mobile sheet, topbar, user menu, per-role E2E on all viewports

**Files:**
- Create (shadcn CLI): `components/ui/{sidebar,sheet,skeleton,tooltip,avatar,badge,dropdown-menu,kbd,popover,scroll-area,breadcrumb}.tsx`, `hooks/use-mobile.ts`
- Modify: `components/ui/sidebar.tsx` (breakpoint classes, floating radius, sheet title), `hooks/use-mobile.ts`
- Create: `admin/components/shell/{admin-shell,app-sidebar,nav-entry,website-card,topbar,user-menu,notifications-button}.tsx`
- Create: `admin/components/shell/{nav-icons,constants,format-badge}.ts`, `admin/components/shell/format-badge.test.ts`
- Create: `admin/lib/initials.ts`, `admin/lib/initials.test.ts`
- Create: `admin/modules/auth/use-sign-out.ts`; Modify: `admin/modules/auth/sign-out-button.tsx`
- Modify: `app/(admin)/admin/(panel)/layout.tsx`
- Create: `app/(admin)/admin/(panel)/error.tsx`, `app/(admin)/admin/(panel)/error.test.tsx`
- Modify: `playwright.config.ts`, `tests/e2e/fixtures/users.ts`, `tests/e2e/support/admin.ts`, `.gitignore`
- Create: `tests/e2e/support/storage.ts`, `tests/e2e/admin/auth.setup.ts`
- Move: `tests/e2e/admin/{sign-in,password-reset,two-factor}.spec.ts` → `tests/e2e/admin/auth/`
- Create: `tests/e2e/admin/shell/navigation.spec.ts`

**Interfaces:**
- Consumes: `visibleNav`, `NAV_GROUPS`, `HELP_LINK`, `isActivePath`, `NavItem` (2.3); `useActor`, `usePermission`, `AdminActorProvider` (2.3); `AdminQueryProvider` (2.4); `ThemeProvider` / `useTheme` (2.2); `BrandMark`, `Swirl` (Phase 1).
- Produces:
  - `AdminShell({ defaultOpen: boolean; siteUrl: string; children })` — skip link, sidebar, topbar, `<main id="main">`.
  - `Topbar()` — renders `<div className="flex min-w-0 flex-1" data-slot="topbar-search" />`; Task 2.6 puts the search pill there.
  - `UserMenu()`, `NotificationsButton()`; `ICON_BUTTON` (class string for circular topbar buttons) in `admin/components/shell/constants.ts`.
  - `useSignOut(): { signOut: () => Promise<void>; pending: boolean }`.
  - `initials(name: string): string`, `formatBadge(count: number): string`.
  - `SIDEBAR_COOKIE = "sidebar_state"` (the cookie `components/ui/sidebar.tsx` writes).
  - E2E: `STORAGE` paths (`editor`, `intake`, `viewer`, `owner`, `account`) and `signOut(page)` in `tests/e2e/support/admin.ts`; owner state has 2FA enabled. Accessible names later specs rely on: button "Toggle navigation", dialog "Navigation", navigation "Main" and "Account", button `Account menu for <name>`, menu item "Sign out", radio items "Light" / "Dark" / "System".

- [ ] **Step 1: Add the shadcn components**

Run: `pnpm exec shadcn add sidebar avatar badge dropdown-menu kbd popover scroll-area breadcrumb tooltip --yes`
Expected: new files under `components/ui/` plus `hooks/use-mobile.ts`; existing files are skipped (do not pass `--overwrite`). Then:

Run: `git status --short components hooks "app/(admin)/admin.css" package.json`
Expected: only new files. If the CLI touched `app/(admin)/admin.css` or an existing `components/ui/*` file, restore it with `git checkout -- <file>` (the sidebar tokens already exist in `:root[data-theme="admin"]`).

- [ ] **Step 2: Adapt the sidebar to the Evergreen shell and the 1024 px breakpoint**

Replace `hooks/use-mobile.ts`:

```ts
import * as React from "react"

// The admin's sidebar becomes a sheet below 1024px (docs/brief.md §9.4); the
// sidebar component's own `lg:` classes use the same breakpoint.
const MOBILE_BREAKPOINT = 1024

export function useIsMobile() {
  const [isMobile, setIsMobile] = React.useState<boolean | undefined>(
    undefined
  )

  React.useEffect(() => {
    const mql = window.matchMedia(`(max-width: ${MOBILE_BREAKPOINT - 1}px)`)
    const onChange = () => {
      setIsMobile(window.innerWidth < MOBILE_BREAKPOINT)
    }
    mql.addEventListener("change", onChange)
    setIsMobile(window.innerWidth < MOBILE_BREAKPOINT)
    return () => mql.removeEventListener("change", onChange)
  }, [])

  return !!isMobile
}
```

Then edit `components/ui/sidebar.tsx`:

```bash
# Server HTML and the client agree below 1024px: the desktop sidebar is CSS-hidden until lg.
perl -pi -e 's/(?<![\w-])md:/lg:/g' components/ui/sidebar.tsx
# Evergreen floating panel: 22px radius, no ring or shadow (docs/brief.md §9.2).
perl -pi -e 's/group-data-\[variant=floating\]:rounded-lg group-data-\[variant=floating\]:shadow-sm group-data-\[variant=floating\]:ring-1 group-data-\[variant=floating\]:ring-sidebar-border/group-data-[variant=floating]:rounded-[22px]/' components/ui/sidebar.tsx
# The floating container is padded p-4 (16px gutters) instead of p-2.
perl -pi -e 's/\(--spacing\(4\)\)/(--spacing(8))/g' components/ui/sidebar.tsx
# The mobile sheet is announced as "Navigation".
perl -pi -e 's/<SheetTitle>Sidebar<\/SheetTitle>/<SheetTitle>Navigation<\/SheetTitle>/; s/Displays the mobile sidebar\./Main navigation and account links./' components/ui/sidebar.tsx
```

Run: `grep -c "md:" components/ui/sidebar.tsx; grep -c "rounded-\[22px\]" components/ui/sidebar.tsx; grep -c "spacing(8)" components/ui/sidebar.tsx; grep -c "<SheetTitle>Navigation" components/ui/sidebar.tsx`
Expected: `0`, `1`, `2`, `1`. (If a count differs, the CLI changed the class order: make the same four edits by hand in `Sidebar`.)

The shortcut the component ships with is ⌘B / Ctrl+B (modifier required), which the "no single-key hotkeys" rule allows.

- [ ] **Step 3: Write the failing unit tests for the small helpers and the error boundary**

```ts
// admin/lib/initials.test.ts
import { expect, it } from "vitest"

import { initials } from "./initials"

it.each([
  ["Olive Owner", "OO"],
  ["eddie", "E"],
  ["  Mary  Ann  Byrne ", "MA"],
  ["", "?"],
])("initials(%j) is %j", (name, expected) => {
  expect(initials(name)).toBe(expected)
})
```

```ts
// admin/components/shell/format-badge.test.ts
import { expect, it } from "vitest"

import { formatBadge } from "./format-badge"

it.each([
  [3, "3"],
  [99, "99"],
  [120, "99+"],
])("formatBadge(%i) is %j", (count, expected) => {
  expect(formatBadge(count)).toBe(expected)
})
```

```tsx
// app/(admin)/admin/(panel)/error.test.tsx
// @vitest-environment jsdom
import { render, screen } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import { expect, it, vi } from "vitest"

import PanelError from "./error"

it("offers a retry that re-fetches the segment", async () => {
  vi.spyOn(console, "error").mockImplementation(() => {})
  const retry = vi.fn()
  render(
    <PanelError
      error={Object.assign(new Error("boom"), { digest: "abc123" })}
      retry={retry}
    />
  )
  expect(screen.getByRole("alert").textContent).toContain("abc123")
  await userEvent.click(screen.getByRole("button", { name: "Try again" }))
  expect(retry).toHaveBeenCalledOnce()
})
```

Run: `pnpm vitest run --project unit admin/lib/initials.test.ts admin/components/shell "app/(admin)/admin/(panel)/error.test.tsx"`
Expected: FAIL with unresolved imports `./initials`, `./format-badge`, `./error`.

- [ ] **Step 4: Implement the helpers and the error boundary**

```ts
// admin/lib/initials.ts
// Up to two initials for avatar fallbacks; "?" when there is no name.
export function initials(name: string) {
  const parts = name.trim().split(/\s+/).filter(Boolean).slice(0, 2)
  return parts.length
    ? parts.map((part) => part[0]!.toUpperCase()).join("")
    : "?"
}
```

```ts
// admin/components/shell/format-badge.ts
// The sidebar's count pill (the inspiration's "12+").
export const formatBadge = (count: number) =>
  count > 99 ? "99+" : String(count)
```

```ts
// admin/components/shell/constants.ts
// The cookie components/ui/sidebar.tsx writes (SIDEBAR_COOKIE_NAME there);
// the panel layout reads it so the first paint matches the saved state.
export const SIDEBAR_COOKIE = "sidebar_state"

// Circular icon buttons (docs/brief.md §9.2, signature component 7).
export const ICON_BUTTON = "size-11 rounded-full border-border bg-card"
```

```tsx
// app/(admin)/admin/(panel)/error.tsx
"use client"

import { useEffect } from "react"

import { Button } from "@/components/ui/button"

// docs/brief.md §9.4: error boundaries with retry. `retry` re-fetches and
// re-renders the segment (stable in Next 16.3).
export default function PanelError({
  error,
  retry,
}: {
  error: Error & { digest?: string }
  retry: () => void
}) {
  useEffect(() => {
    console.error(error)
  }, [error])

  return (
    <section
      role="alert"
      className="mx-auto flex max-w-md flex-col items-center gap-4 py-16 text-center"
    >
      <h1 className="text-card-title">Something went wrong</h1>
      <p className="text-sm text-muted-foreground">
        This part of the admin didn&apos;t load. Nothing you saved is lost.
        {error.digest ? (
          <>
            {" "}
            Reference <code className="font-mono">{error.digest}</code>.
          </>
        ) : null}
      </p>
      <Button
        onClick={() => retry()}
        className="h-11 rounded-xl px-5 font-semibold"
      >
        Try again
      </Button>
    </section>
  )
}
```

Run: `pnpm vitest run --project unit admin/lib/initials.test.ts admin/components/shell "app/(admin)/admin/(panel)/error.test.tsx"`
Expected: PASS.

- [ ] **Step 5: Extract the sign-out logic**

```ts
// admin/modules/auth/use-sign-out.ts
"use client"

import { useRouter } from "next/navigation"
import { useCallback, useState } from "react"
import { toast } from "sonner"

import { authClient } from "@/admin/lib/auth-client"
import { useResetOnHide } from "@/admin/lib/use-reset-on-hide"

// Shared by the user menu, the sidebar's "Log out" and ⌘K.
export function useSignOut() {
  const router = useRouter()
  const [pending, setPending] = useState(false)
  useResetOnHide(() => setPending(false))

  const signOut = useCallback(async () => {
    setPending(true)
    let failed = false
    try {
      failed = Boolean((await authClient.signOut()).error)
    } catch {
      failed = true
    }
    if (failed) {
      setPending(false)
      toast.error("Couldn't sign out. Try again.")
      return
    }
    // Stays pending: the sign-in screen replaces this one.
    router.replace("/admin/sign-in")
    router.refresh()
  }, [router])

  return { signOut, pending }
}
```

Replace `admin/modules/auth/sign-out-button.tsx`:

```tsx
"use client"

import { LogOutIcon } from "lucide-react"

import { Button } from "@/components/ui/button"

import { useSignOut } from "./use-sign-out"

export function SignOutButton() {
  const { signOut, pending } = useSignOut()
  return (
    <Button
      variant="outline"
      disabled={pending}
      className="h-11 rounded-xl border-primary px-4 font-semibold text-primary hover:bg-accent hover:text-accent-foreground"
      onClick={signOut}
    >
      <LogOutIcon data-icon="inline-start" />
      Sign out
    </Button>
  )
}
```

- [ ] **Step 6: Build the shell components**

```ts
// admin/components/shell/nav-icons.ts
import {
  CircleHelpIcon,
  FileTextIcon,
  HistoryIcon,
  ImageIcon,
  InboxIcon,
  LayersIcon,
  LayoutDashboardIcon,
  MailIcon,
  MegaphoneIcon,
  NewspaperIcon,
  PaletteIcon,
  SearchCheckIcon,
  SettingsIcon,
  UsersIcon,
  type LucideIcon,
} from "lucide-react"

// Icons live with the UI; lib/admin/nav.ts stays free of React.
export const NAV_ICONS: Record<string, LucideIcon> = {
  dashboard: LayoutDashboardIcon,
  pages: FileTextIcon,
  insights: NewspaperIcon,
  collections: LayersIcon,
  media: ImageIcon,
  leads: InboxIcon,
  newsletter: MailIcon,
  seo: SearchCheckIcon,
  marketing: MegaphoneIcon,
  appearance: PaletteIcon,
  settings: SettingsIcon,
  users: UsersIcon,
  activity: HistoryIcon,
  help: CircleHelpIcon,
}
```

```tsx
// admin/components/shell/nav-entry.tsx
"use client"

import Link from "next/link"

import {
  SidebarMenuBadge,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarMenuSub,
  SidebarMenuSubButton,
  SidebarMenuSubItem,
} from "@/components/ui/sidebar"
import { isActivePath, type NavItem } from "@/lib/admin/nav"

import { formatBadge } from "./format-badge"
import { NAV_ICONS } from "./nav-icons"

// Muted label and icon; the active item gets a bold label and a filled
// green icon (docs/brief.md §9.2, signature component 1).
export const NAV_BUTTON =
  "h-11 gap-3 rounded-xl px-3 text-[15px] text-muted-foreground hover:bg-sidebar-accent/60 data-active:bg-transparent data-active:font-semibold data-active:text-foreground [&_svg]:size-5 data-active:[&_svg]:fill-primary/15 data-active:[&_svg]:text-primary"

export function NavEntry({
  item,
  pathname,
  badge,
  onNavigate,
}: {
  item: NavItem
  pathname: string
  badge?: number
  onNavigate: () => void
}) {
  const Icon = NAV_ICONS[item.id]
  const active = isActivePath(pathname, item.href)
  const children = item.children ?? []
  return (
    <SidebarMenuItem>
      {/* The 4px active bar on the panel's left edge. */}
      {active ? (
        <span
          aria-hidden
          className="absolute top-1/2 -left-4 h-8 w-1 -translate-y-1/2 rounded-r-full bg-primary group-data-[collapsible=icon]:-left-2"
        />
      ) : null}
      <SidebarMenuButton
        size="lg"
        isActive={active}
        tooltip={item.label}
        className={NAV_BUTTON}
        render={
          <Link
            href={item.href}
            onClick={onNavigate}
            aria-current={active ? "page" : undefined}
          />
        }
      >
        {Icon ? <Icon /> : null}
        <span>{item.label}</span>
      </SidebarMenuButton>
      {badge ? (
        <SidebarMenuBadge className="top-3! right-2 h-6 rounded-full bg-primary px-2 text-[11px] font-semibold text-primary-foreground">
          {formatBadge(badge)}
        </SidebarMenuBadge>
      ) : null}
      {active && children.length > 1 ? (
        <SidebarMenuSub className="mt-1">
          {children.map((child) => {
            const current = pathname === child.href
            return (
              <SidebarMenuSubItem key={child.id}>
                <SidebarMenuSubButton
                  isActive={current}
                  className="h-8"
                  render={
                    <Link
                      href={child.href}
                      onClick={onNavigate}
                      aria-current={current ? "page" : undefined}
                    />
                  }
                >
                  <span>{child.label}</span>
                </SidebarMenuSubButton>
              </SidebarMenuSubItem>
            )
          })}
        </SidebarMenuSub>
      ) : null}
    </SidebarMenuItem>
  )
}
```

```tsx
// admin/components/shell/website-card.tsx
import { ExternalLinkIcon } from "lucide-react"

import { Swirl } from "@/admin/components/swirl"
import { Button } from "@/components/ui/button"

// The inspiration's "Download our Mobile App" slot, as "Your website"
// (docs/brief.md §9.1). Phase 4 adds "Preview drafts" next to "Open site".
export function WebsiteCard({ siteUrl }: { siteUrl: string }) {
  return (
    <div className="relative isolate overflow-hidden rounded-[18px] bg-promo p-5 group-data-[collapsible=icon]:hidden">
      <Swirl className="absolute inset-0 -z-10 size-full" />
      <p className="flex items-center gap-2 text-xs font-medium opacity-80">
        <span aria-hidden className="size-2 rounded-full bg-chart-3" />
        Live
      </p>
      <p className="mt-3 text-lg leading-tight font-semibold">Your website</p>
      <p className="mt-1 truncate text-xs opacity-75">{new URL(siteUrl).host}</p>
      <Button
        nativeButton={false}
        render={<a href={siteUrl} target="_blank" rel="noreferrer" />}
        className="mt-4 h-11 w-full rounded-xl font-semibold"
      >
        Open site
        <ExternalLinkIcon data-icon="inline-end" />
        <span className="sr-only"> (opens in a new tab)</span>
      </Button>
    </div>
  )
}
```

```tsx
// admin/components/shell/app-sidebar.tsx
"use client"

import { LogOutIcon } from "lucide-react"
import Link from "next/link"
import { usePathname } from "next/navigation"

import { BrandMark } from "@/admin/components/brand-mark"
import { useActor } from "@/admin/lib/actor-context"
import { useSignOut } from "@/admin/modules/auth/use-sign-out"
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarGroupLabel,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  useSidebar,
} from "@/components/ui/sidebar"
import {
  HELP_LINK,
  isActivePath,
  NAV_GROUPS,
  visibleNav,
} from "@/lib/admin/nav"

import { NAV_BUTTON, NavEntry } from "./nav-entry"
import { NAV_ICONS } from "./nav-icons"
import { WebsiteCard } from "./website-card"

export function AppSidebar({
  siteUrl,
  badges = {},
}: {
  siteUrl: string
  badges?: Partial<Record<"new-leads", number>>
}) {
  const { roles } = useActor()
  const pathname = usePathname()
  const { signOut, pending } = useSignOut()
  const { isMobile, setOpenMobile } = useSidebar()
  const items = visibleNav(roles)
  // Following a link inside the mobile sheet closes it (Review Focus #5).
  const onNavigate = () => {
    if (isMobile) setOpenMobile(false)
  }
  const HelpIcon = NAV_ICONS.help

  return (
    <Sidebar variant="floating" collapsible="icon" className="p-4">
      <SidebarHeader className="px-4 pt-6 pb-4 group-data-[collapsible=icon]:px-1">
        <Link
          href="/admin"
          onClick={onNavigate}
          aria-label="Magda Kennedy admin home"
          className="rounded-xl outline-none focus-visible:ring-3 focus-visible:ring-ring/50"
        >
          <BrandMark className="group-data-[collapsible=icon]:[&>span:last-child]:hidden" />
        </Link>
      </SidebarHeader>
      <SidebarContent>
        <nav aria-label="Main">
          {NAV_GROUPS.map((group) => {
            const inGroup = items.filter((item) => item.group === group.id)
            if (inGroup.length === 0) return null
            return (
              <SidebarGroup key={group.id}>
                <SidebarGroupLabel className="px-3 text-xs font-semibold tracking-wider text-muted-foreground uppercase">
                  {group.label}
                </SidebarGroupLabel>
                <SidebarMenu className="gap-1">
                  {inGroup.map((item) => (
                    <NavEntry
                      key={item.id}
                      item={item}
                      pathname={pathname}
                      badge={item.badge ? badges[item.badge] : undefined}
                      onNavigate={onNavigate}
                    />
                  ))}
                </SidebarMenu>
              </SidebarGroup>
            )
          })}
        </nav>
        <nav aria-label="Account" className="mt-auto">
          <SidebarGroup>
            <SidebarMenu className="gap-1">
              <SidebarMenuItem>
                <SidebarMenuButton
                  size="lg"
                  tooltip={HELP_LINK.label}
                  isActive={isActivePath(pathname, HELP_LINK.href)}
                  className={NAV_BUTTON}
                  render={<Link href={HELP_LINK.href} onClick={onNavigate} />}
                >
                  <HelpIcon />
                  <span>{HELP_LINK.label}</span>
                </SidebarMenuButton>
              </SidebarMenuItem>
              <SidebarMenuItem>
                <SidebarMenuButton
                  size="lg"
                  tooltip="Log out"
                  disabled={pending}
                  className={NAV_BUTTON}
                  onClick={signOut}
                >
                  <LogOutIcon />
                  <span>Log out</span>
                </SidebarMenuButton>
              </SidebarMenuItem>
            </SidebarMenu>
          </SidebarGroup>
        </nav>
      </SidebarContent>
      <SidebarFooter className="p-3">
        <WebsiteCard siteUrl={siteUrl} />
      </SidebarFooter>
    </Sidebar>
  )
}
```

```tsx
// admin/components/shell/user-menu.tsx
"use client"

import {
  LogOutIcon,
  MonitorIcon,
  MoonIcon,
  ShieldCheckIcon,
  SunIcon,
  UserIcon,
} from "lucide-react"
import Link from "next/link"
import { useTheme } from "next-themes"

import { useActor } from "@/admin/lib/actor-context"
import { initials } from "@/admin/lib/initials"
import { useSignOut } from "@/admin/modules/auth/use-sign-out"
import { Avatar, AvatarFallback } from "@/components/ui/avatar"
import { Badge } from "@/components/ui/badge"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"

const THEMES = [
  { value: "light", label: "Light", icon: SunIcon },
  { value: "dark", label: "Dark", icon: MoonIcon },
  { value: "system", label: "System", icon: MonitorIcon },
] as const

// The avatar block: name, email, role badge (docs/brief.md §9.1).
export function UserMenu() {
  const { user, roles } = useActor()
  const { theme, setTheme } = useTheme()
  const { signOut, pending } = useSignOut()
  const roleLabel = roles.join(", ")

  return (
    <DropdownMenu>
      <DropdownMenuTrigger
        aria-label={`Account menu for ${user.name}`}
        className="flex min-w-0 items-center gap-3 rounded-full p-1 outline-none hover:bg-muted focus-visible:ring-3 focus-visible:ring-ring/50 sm:pr-3"
      >
        <Avatar className="size-11">
          <AvatarFallback className="bg-primary font-semibold text-primary-foreground">
            {initials(user.name)}
          </AvatarFallback>
        </Avatar>
        <span className="hidden min-w-0 flex-col text-left sm:flex">
          <span className="truncate text-sm font-semibold">{user.name}</span>
          <span className="truncate text-xs text-muted-foreground">
            {user.email}
          </span>
        </span>
        {roleLabel ? (
          <Badge variant="secondary" className="hidden capitalize xl:inline-flex">
            {roleLabel}
          </Badge>
        ) : null}
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" className="w-60">
        <DropdownMenuGroup>
          <DropdownMenuLabel className="flex flex-col">
            <span className="truncate font-semibold text-foreground">
              {user.name}
            </span>
            <span className="truncate text-xs font-normal">{user.email}</span>
          </DropdownMenuLabel>
        </DropdownMenuGroup>
        <DropdownMenuSeparator />
        <DropdownMenuGroup>
          <DropdownMenuItem render={<Link href="/admin/account" />}>
            <UserIcon />
            Account
          </DropdownMenuItem>
          <DropdownMenuItem render={<Link href="/admin/account?tab=security" />}>
            <ShieldCheckIcon />
            Security
          </DropdownMenuItem>
        </DropdownMenuGroup>
        <DropdownMenuSeparator />
        <DropdownMenuGroup>
          <DropdownMenuLabel>Theme</DropdownMenuLabel>
          <DropdownMenuRadioGroup
            value={theme ?? "system"}
            onValueChange={(value) => setTheme(String(value))}
          >
            {THEMES.map(({ value, label, icon: Icon }) => (
              <DropdownMenuRadioItem key={value} value={value}>
                <Icon />
                {label}
              </DropdownMenuRadioItem>
            ))}
          </DropdownMenuRadioGroup>
        </DropdownMenuGroup>
        <DropdownMenuSeparator />
        <DropdownMenuItem disabled={pending} onClick={signOut}>
          <LogOutIcon />
          Sign out
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  )
}
```

```tsx
// admin/components/shell/notifications-button.tsx
"use client"

import { BellIcon } from "lucide-react"

import { Button } from "@/components/ui/button"
import {
  Popover,
  PopoverContent,
  PopoverHeader,
  PopoverTitle,
  PopoverTrigger,
} from "@/components/ui/popover"
import { ScrollArea } from "@/components/ui/scroll-area"

import { ICON_BUTTON } from "./constants"

// The bell (docs/brief.md §9.1). Phase 11 fills it from
// GET /api/v1/admin/notifications; until then it is honestly empty.
export function NotificationsButton() {
  return (
    <Popover>
      <PopoverTrigger
        render={
          <Button
            variant="outline"
            size="icon-lg"
            className={ICON_BUTTON}
            aria-label="Notifications"
          />
        }
      >
        <BellIcon />
      </PopoverTrigger>
      <PopoverContent align="end" className="w-80 gap-0 p-0">
        <PopoverHeader className="border-b px-4 py-3">
          <PopoverTitle>Notifications</PopoverTitle>
        </PopoverHeader>
        <ScrollArea className="max-h-80">
          <p className="px-4 py-8 text-center text-sm text-muted-foreground">
            You&apos;re all caught up. New enquiries, posts going live and
            failed deliveries will show here.
          </p>
        </ScrollArea>
      </PopoverContent>
    </Popover>
  )
}
```

```tsx
// admin/components/shell/topbar.tsx
"use client"

import { MailIcon } from "lucide-react"
import Link from "next/link"

import { usePermission } from "@/admin/lib/actor-context"
import { Button } from "@/components/ui/button"
import { SidebarTrigger } from "@/components/ui/sidebar"

import { ICON_BUTTON } from "./constants"
import { NotificationsButton } from "./notifications-button"
import { UserMenu } from "./user-menu"

// The 72px topbar panel (docs/brief.md §9.1–9.2).
export function Topbar() {
  const canReadLeads = usePermission({ lead: ["read"] })
  return (
    <header className="flex h-[72px] shrink-0 items-center gap-2 rounded-card bg-sidebar px-3 sm:gap-3 sm:px-5">
      <SidebarTrigger
        aria-label="Toggle navigation"
        className="size-10 rounded-full"
      />
      <div className="flex min-w-0 flex-1" data-slot="topbar-search" />
      {canReadLeads ? (
        <Button
          variant="outline"
          size="icon-lg"
          nativeButton={false}
          className={ICON_BUTTON}
          aria-label="Enquiries"
          render={<Link href="/admin/leads" />}
        >
          <MailIcon />
        </Button>
      ) : null}
      <NotificationsButton />
      <UserMenu />
    </header>
  )
}
```

```tsx
// admin/components/shell/admin-shell.tsx
"use client"

import { SidebarProvider } from "@/components/ui/sidebar"
import { TooltipProvider } from "@/components/ui/tooltip"

import { AppSidebar } from "./app-sidebar"
import { Topbar } from "./topbar"

// Floating panels on the canvas with 16px gutters: a 288px sidebar panel
// (20rem minus the container's p-4), the 72px topbar and the content panel
// (docs/brief.md §9.2).
export function AdminShell({
  defaultOpen,
  siteUrl,
  children,
}: {
  defaultOpen: boolean
  siteUrl: string
  children: React.ReactNode
}) {
  return (
    <TooltipProvider>
      <SidebarProvider
        defaultOpen={defaultOpen}
        style={{ "--sidebar-width": "20rem" } as React.CSSProperties}
      >
        <a
          href="#main"
          className="sr-only z-50 rounded-xl bg-card px-4 py-2 text-sm font-semibold shadow-lg focus:not-sr-only focus:fixed focus:top-4 focus:left-4"
        >
          Skip to content
        </a>
        <AppSidebar siteUrl={siteUrl} />
        <div className="flex min-h-svh min-w-0 flex-1 flex-col gap-4 p-3 sm:p-4 lg:pl-0">
          <Topbar />
          <main
            id="main"
            tabIndex={-1}
            className="flex-1 rounded-card bg-sidebar p-4 outline-none sm:p-6 lg:p-8"
          >
            {children}
          </main>
        </div>
      </SidebarProvider>
    </TooltipProvider>
  )
}
```

- [ ] **Step 7: Render the shell from the guarded layout**

Replace `app/(admin)/admin/(panel)/layout.tsx`:

```tsx
import { cookies } from "next/headers"
import { Suspense } from "react"

import { AdminShell } from "@/admin/components/shell/admin-shell"
import { SIDEBAR_COOKIE } from "@/admin/components/shell/constants"
import { AdminActorProvider } from "@/admin/lib/actor-context"
import { AdminQueryProvider } from "@/admin/lib/query-provider"
import { toMePayload } from "@/lib/auth/me"
import { requireActor } from "@/server/auth/session"
import { getEnv } from "@/server/env"

// The session read is request-time data, so it sits behind Suspense (Cache
// Components). requireActor also sends an owner or admin without 2FA to
// setup (docs/brief.md §7.4). This check is for UX; the API authorises, and
// enforces 2FA on, every call itself.
export default function PanelLayout({
  children,
}: {
  children: React.ReactNode
}) {
  return (
    <Suspense fallback={<div className="min-h-svh bg-background" />}>
      <Guarded>{children}</Guarded>
    </Suspense>
  )
}

async function Guarded({ children }: { children: React.ReactNode }) {
  const actor = await requireActor()
  // Only an explicit "false" collapses; a first visit starts open.
  const sidebarOpen = (await cookies()).get(SIDEBAR_COOKIE)?.value !== "false"
  return (
    <AdminActorProvider value={toMePayload(actor)}>
      <AdminQueryProvider>
        <AdminShell defaultOpen={sidebarOpen} siteUrl={getEnv().SITE_URL}>
          {children}
        </AdminShell>
      </AdminQueryProvider>
    </AdminActorProvider>
  )
}
```

Run: `pnpm typecheck && pnpm build`
Expected: green.

- [ ] **Step 8: Per-role storage states and admin E2E on every viewport (carry-over from Ruling F4)**

Add users to `tests/e2e/fixtures/users.ts` (inside `E2E_USERS`, after `resetter`):

```ts
  // Shared, read-only sessions for the shell specs (tests/e2e/admin/auth.setup.ts).
  intake: { email: "intake@e2e.test", name: "Ivy Intake", role: "intake" },
  viewer: { email: "viewer@e2e.test", name: "Vera Viewer", role: "viewer" },
  // An owner the setup project enrols in 2FA; never signs out.
  shellOwner: {
    email: "owner-shell@e2e.test",
    name: "Sam Shell",
    role: "owner",
  },
  // Only the account spec changes this one (name, sessions).
  accountUser: {
    email: "account@e2e.test",
    name: "Andy Account",
    role: "editor",
  },
```

```ts
// tests/e2e/support/storage.ts
import path from "node:path"

// Written by tests/e2e/admin/auth.setup.ts (the "setup" project) before any
// other project runs. Specs that use a state must never sign out of it.
const dir = path.join(import.meta.dirname, "../.auth")

export const STORAGE = {
  editor: path.join(dir, "editor.json"),
  intake: path.join(dir, "intake.json"),
  viewer: path.join(dir, "viewer.json"),
  owner: path.join(dir, "owner.json"),
  account: path.join(dir, "account.json"),
} as const
```

```ts
// tests/e2e/admin/auth.setup.ts
import { expect, test as setup } from "@playwright/test"
import { TOTP } from "otpauth"

import { E2E_ORIGIN } from "../fixtures/env"
import { E2E_USERS, PASSWORD, signIn } from "../support/admin"
import { resetTwoFactor } from "../support/db"
import { STORAGE } from "../support/storage"

const PLAIN = [
  ["editor", E2E_USERS.editor],
  ["intake", E2E_USERS.intake],
  ["viewer", E2E_USERS.viewer],
  ["account", E2E_USERS.accountUser],
] as const

for (const [key, user] of PLAIN) {
  setup(`signed in as ${user.role} (${key})`, async ({ page }) => {
    await page.goto("/admin/sign-in")
    await signIn(page, user)
    await expect(page).toHaveURL(/\/admin$/)
    await page.context().storageState({ path: STORAGE[key] })
  })
}

// Owners must use 2FA (docs/brief.md §7.4). Enrol through Better Auth's own
// endpoints rather than the UI (the UI flow is covered by two-factor.spec).
setup("signed in as an owner with two-factor", async ({ page }) => {
  const user = E2E_USERS.shellOwner
  await resetTwoFactor(user.email)
  await page.goto("/admin/sign-in")
  await signIn(page, user)
  await expect(page).toHaveURL(/\/admin\/two-factor-setup$/)

  const headers = { origin: E2E_ORIGIN }
  const enable = await page.request.post("/api/auth/two-factor/enable", {
    data: { password: PASSWORD },
    headers,
  })
  expect(enable.ok()).toBe(true)
  const { totpURI } = (await enable.json()) as { totpURI: string }
  const secret = new URL(totpURI).searchParams.get("secret") ?? ""
  const verify = await page.request.post("/api/auth/two-factor/verify-totp", {
    data: { code: new TOTP({ secret, digits: 6, period: 30 }).generate() },
    headers,
  })
  expect(verify.ok()).toBe(true)

  await page.goto("/admin")
  await expect(page).toHaveURL(/\/admin$/)
  await page.context().storageState({ path: STORAGE.owner })
})
```

Append to `tests/e2e/support/admin.ts`:

```ts
// Signs out through the user menu (the topbar's avatar block).
export async function signOut(page: Page) {
  await page.getByRole("button", { name: /^Account menu for / }).click()
  await page.getByRole("menuitem", { name: "Sign out" }).click()
}
```

Move the account-mutating specs into their own folder, which stays desktop-only:

```bash
mkdir -p tests/e2e/admin/auth
git mv tests/e2e/admin/sign-in.spec.ts tests/e2e/admin/password-reset.spec.ts tests/e2e/admin/two-factor.spec.ts tests/e2e/admin/auth/
perl -pi -e 's#"\.\./support/#"../../support/#g; s#"\.\./fixtures/#"../../fixtures/#g; s#"\.\./\.\./\.\./admin/#"../../../../admin/#g' tests/e2e/admin/auth/*.spec.ts
echo "/tests/e2e/.auth/" >> .gitignore
```

In `playwright.config.ts` replace the `DESKTOP_ONLY` block and `projects`:

```ts
// Specs that change accounts (passwords, 2FA, sign-out) run once, on
// desktop. Everything else, admin shell included, runs on all viewports.
const DESKTOP_ONLY = ["**/admin/auth/**"]
```

```ts
  projects: [
    // Signs each E2E role in once and saves its storage state.
    {
      name: "setup",
      testMatch: /admin\/auth\.setup\.ts$/,
      use: { ...devices["Desktop Chrome"] },
    },
    {
      name: "desktop",
      dependencies: ["setup"],
      use: {
        ...devices["Desktop Chrome"],
        viewport: { width: 1440, height: 900 },
      },
    },
    {
      name: "tablet",
      dependencies: ["setup"],
      testIgnore: DESKTOP_ONLY,
      use: {
        ...devices["Desktop Chrome"],
        viewport: { width: 834, height: 1112 },
      },
    },
    {
      name: "mobile",
      dependencies: ["setup"],
      testIgnore: DESKTOP_ONLY,
      use: { ...devices["Pixel 7"] },
    },
  ],
```

- [ ] **Step 9: Write the shell E2E (Review Focus #1, #4, #5)**

```ts
// tests/e2e/admin/shell/navigation.spec.ts
import { expect, test, type Locator, type Page } from "@playwright/test"

import { E2E_ORIGIN } from "../../fixtures/env"
import { E2E_USERS, signIn, signOut } from "../../support/admin"
import { STORAGE } from "../../support/storage"

const onDesktop = () => test.info().project.name === "desktop"

// The main navigation, opening the sheet first below 1024px.
async function mainNav(page: Page): Promise<Locator> {
  if (!onDesktop()) {
    await page.getByRole("button", { name: "Toggle navigation" }).click()
    const sheet = page.getByRole("dialog", { name: "Navigation" })
    await expect(sheet).toBeVisible()
    return sheet.getByRole("navigation", { name: "Main" })
  }
  return page.getByRole("navigation", { name: "Main" })
}

async function linkLabels(nav: Locator) {
  return (await nav.getByRole("link").allTextContents()).map((t) => t.trim())
}

test.describe("as intake", () => {
  test.use({ storageState: STORAGE.intake })

  test("sees only the dashboard and enquiries", async ({ page }) => {
    await page.goto("/admin")
    const nav = await mainNav(page)
    expect(await linkLabels(nav)).toEqual(["Dashboard", "Enquiries"])
    await expect(nav.getByText("Growth", { exact: true })).toHaveCount(0)
  })

  test("opens every link it is shown", async ({ page }) => {
    await page.goto("/admin")
    await (await mainNav(page)).getByRole("link", { name: "Enquiries" }).click()
    await expect(page).toHaveURL(/\/admin\/leads$/)
    await expect(
      page.getByRole("heading", { level: 1, name: "Enquiries" })
    ).toBeVisible()
  })

  test("is turned away from a section outside the role", async ({ page }) => {
    await page.goto("/admin/pages")
    await expect(page).toHaveURL(/\/admin\/no-access\?from=%2Fadmin%2Fpages$/)
    await expect(
      page.getByRole("heading", { name: "You don't have access to Pages" })
    ).toBeVisible()
  })
})

test.describe("as a viewer", () => {
  test.use({ storageState: STORAGE.viewer })

  test("sees read-only content sections and no settings", async ({ page }) => {
    await page.goto("/admin")
    expect(await linkLabels(await mainNav(page))).toEqual([
      "Dashboard",
      "Pages",
      "Insights",
      "Collections",
      "SEO",
    ])
    await page.goto("/admin/settings")
    await expect(
      page.getByRole("heading", {
        name: "You don't have access to Site settings",
      })
    ).toBeVisible()
  })
})

test.describe("as an editor", () => {
  test.use({ storageState: STORAGE.editor })

  test("sees content sections and SEO", async ({ page }) => {
    await page.goto("/admin")
    expect(await linkLabels(await mainNav(page))).toEqual([
      "Dashboard",
      "Pages",
      "Insights",
      "Collections",
      "Media",
      "SEO",
    ])
  })

  test("the theme chosen in the user menu sticks", async ({ page }) => {
    await page.emulateMedia({ colorScheme: "light" })
    await page.goto("/admin")
    await page.getByRole("button", { name: /^Account menu for / }).click()
    await page.getByRole("menuitemradio", { name: "Dark" }).click()
    await expect(page.locator("html")).toHaveClass(/\bdark\b/)
    await page.reload()
    await expect(page.locator("html")).toHaveClass(/\bdark\b/)
  })

  test("a collapsed sidebar stays collapsed after a reload", async ({
    page,
  }) => {
    test.skip(!onDesktop(), "desktop only: below 1024px it is a sheet")
    await page.goto("/admin")
    const sidebar = page.locator('[data-slot="sidebar"][data-state]')
    await expect(sidebar).toHaveAttribute("data-state", "expanded")
    await page.getByRole("button", { name: "Toggle navigation" }).click()
    await expect(sidebar).toHaveAttribute("data-state", "collapsed")
    await page.reload()
    await expect(sidebar).toHaveAttribute("data-state", "collapsed")
  })

  // Review Focus #4 and #5.
  test("below 1024px the navigation is a sheet a keyboard can leave", async ({
    page,
    context,
  }) => {
    test.skip(onDesktop(), "tablet and mobile only")
    // A preference saved on desktop must not shrink the sheet to icons.
    await context.addCookies([
      { name: "sidebar_state", value: "false", url: E2E_ORIGIN },
    ])
    await page.goto("/admin")
    const trigger = page.getByRole("button", { name: "Toggle navigation" })
    const sheet = page.getByRole("dialog", { name: "Navigation" })
    await expect(sheet).toBeHidden()

    await trigger.click()
    await expect(sheet).toBeVisible()
    await expect(sheet.getByRole("link", { name: "Pages" })).toBeVisible()
    await page.keyboard.press("Escape")
    await expect(sheet).toBeHidden()
    await expect(trigger).toBeFocused()

    await trigger.click()
    await sheet.getByRole("link", { name: "Pages" }).click()
    await expect(page).toHaveURL(/\/admin\/pages$/)
    await expect(sheet).toBeHidden()
  })
})

test.describe("as an owner", () => {
  test.use({ storageState: STORAGE.owner })

  test("sees every section and a section's sub-pages", async ({ page }) => {
    await page.goto("/admin/seo")
    const nav = await mainNav(page)
    for (const label of [
      "Dashboard",
      "Enquiries",
      "Newsletter",
      "Marketing",
      "Appearance",
      "Site settings",
      "Users & roles",
      "Activity log",
    ]) {
      await expect(nav.getByRole("link", { name: label, exact: true })).toBeVisible()
    }
    await expect(nav.getByRole("link", { name: "SEO", exact: true })).toHaveAttribute(
      "aria-current",
      "page"
    )
    await expect(nav.getByRole("link", { name: "Redirects" })).toBeVisible()
  })
})

test.describe("signing out", () => {
  // A fresh session: the shared storage states must never be signed out.
  test.use({ storageState: { cookies: [], origins: [] } })

  test("from the user menu ends the session", async ({ page }) => {
    await page.goto("/admin/sign-in")
    await signIn(page, E2E_USERS.editor)
    await expect(page).toHaveURL(/\/admin$/)
    await signOut(page)
    await expect(page).toHaveURL(/\/admin\/sign-in/)
    await page.goto("/admin")
    await expect(page).toHaveURL(/\/admin\/sign-in/)
  })
})
```

- [ ] **Step 10: Run the E2E to verify**

Run: `pnpm test:e2e tests/e2e/admin`
Expected: PASS on desktop, tablet and mobile (`auth/**` only on desktop). If "a keyboard can leave" fails on `toBeFocused`, Base UI did not restore focus to the external trigger: pass `finalFocus` to `SheetContent` in `components/ui/sidebar.tsx` with a ref to the topbar trigger, and keep the test.

- [ ] **Step 11: Parity and gates**

Run: `pnpm test:parity && pnpm lint && pnpm typecheck && pnpm test:unit && pnpm build`
Expected: all green (the public site is untouched).

- [ ] **Step 12: Commit**

```bash
git add components/ui hooks admin "app/(admin)/admin/(panel)" playwright.config.ts tests/e2e .gitignore package.json pnpm-lock.yaml
git commit -m "feat(admin): Evergreen shell with permission-filtered sidebar, mobile sheet and user menu"
```

---

## Task 2.6: ⌘K command palette and `GET /api/v1/admin/search`

**Files:**
- Create (shadcn CLI): `components/ui/{command,dialog,input-group}.tsx` (+ `textarea.tsx` if the CLI brings it in for `input-group`)
- Create: `server/modules/search/service.ts`, `server/modules/search/service.test.ts`, `server/modules/search/routes.ts`
- Modify: `server/api/routes/admin.ts`
- Modify: `admin/lib/api.ts` (`searchApi`)
- Create: `admin/components/command/{use-command-hotkey.ts,use-command-hotkey.test.tsx,actions.ts,actions.test.ts,palette-context.tsx,command-palette.tsx,search-pill.tsx}`
- Modify: `admin/components/shell/topbar.tsx`, `admin/components/shell/admin-shell.tsx`
- Create: `tests/integration/api/search.test.ts`
- Modify: `tests/integration/api/permission-matrix.test.ts`
- Create: `tests/e2e/admin/shell/command-palette.spec.ts`

**Interfaces:**
- Consumes: `reachableLinks`, `Destination` (2.3); `can`, `validationHook`, `AppEnv`; `parseResponse`, `queryKeys.search` (2.4); `useSignOut`, `ADMIN_THEME_STORAGE_KEY` via `useTheme` (2.2/2.5); `useResetOnHide`.
- Produces:
  - `server/modules/search/service.ts`: `type SearchHit = { id: string; kind: "page"; label: string; href: string }`; `searchNavigation(roles: readonly RoleName[], q: string): SearchHit[]`; `normaliseSearch(text: string): string`.
  - `GET /api/v1/admin/search?q=` → `{ items: SearchHit[] }` (`dashboard.view`; `q` ≤ 100 chars). `searchRoutes`, `type SearchRoutes`.
  - `admin/lib/api.ts`: `searchApi = hc<SearchRoutes>("/api/v1/admin/search")`.
  - `isPaletteShortcut(event)`, `useCommandHotkey(onTrigger)`.
  - `buildActions({ setTheme, signOut, siteUrl }): PaletteAction[]`, `matchesQuery(q, label, keywords): boolean`, `type PaletteAction = { id: string; label: string; keywords: readonly string[]; run: () => void }`.
  - `CommandPaletteProvider({ children })`, `useCommandPalette(): { open: boolean; setOpen: (open: boolean) => void }`, `CommandPalette({ siteUrl })`, `SearchPill()`.
  - Accessible names used by E2E: dialog "Search the admin", groups "Go to" / "Actions", button "Search" (with `aria-keyshortcuts`).

- [ ] **Step 1: Add the components**

Run: `pnpm exec shadcn add command dialog input-group --yes`
Expected: `components/ui/command.tsx`, `dialog.tsx`, `input-group.tsx` (and `textarea.tsx`) created; `cmdk@1.1.1` added to `package.json`. Restore any modified existing file with `git checkout -- <file>`.

Run: `pnpm ls cmdk`
Expected: `cmdk 1.1.1`.

- [ ] **Step 2: Write the failing search tests**

```ts
// server/modules/search/service.test.ts
import { describe, expect, it } from "vitest"

import { normaliseSearch, searchNavigation } from "./service"

const hrefs = (roles: Parameters<typeof searchNavigation>[0], q: string) =>
  searchNavigation(roles, q).map((hit) => hit.href)

describe("searchNavigation", () => {
  it("lists every reachable destination for an empty query", () => {
    expect(hrefs(["intake"], "")).toEqual([
      "/admin",
      "/admin/leads",
      "/admin/help",
      "/admin/account",
    ])
  })

  it("matches labels and keywords, ignoring case and accents", () => {
    expect(hrefs(["owner"], "INVITE")).toContain("/admin/users")
    expect(hrefs(["owner"], "colours")).toContain("/admin/appearance")
    expect(normaliseSearch("Café ")).toBe("cafe")
  })

  // Review Focus #3.
  it("never offers a destination the role cannot open", () => {
    expect(hrefs(["editor"], "users")).toEqual([])
    expect(hrefs(["marketer"], "enquiries")).toEqual([])
    expect(hrefs(["admin"], "custom code")).toEqual([])
  })

  it("labels sub-pages with their section", () => {
    const hit = searchNavigation(["owner"], "redirects")[0]
    expect(hit).toEqual({
      id: "nav:seo-redirects",
      kind: "page",
      label: "SEO › Redirects",
      href: "/admin/seo/redirects",
    })
  })
})
```

```ts
// tests/integration/api/search.test.ts
import { afterAll, beforeEach, describe, expect, it } from "vitest"

import { closeDb } from "@/server/db/client"

import {
  adminRequest,
  createUser,
  signIn,
  signInWithTwoFactor,
} from "../helpers/auth"
import { resetDb } from "../helpers/db"

beforeEach(resetDb)
afterAll(closeDb)

async function search(cookie: string, q: string) {
  const response = await adminRequest(
    `/search?q=${encodeURIComponent(q)}`,
    cookie
  )
  return { status: response.status, body: await response.json() }
}

describe("GET /api/v1/admin/search", () => {
  it("answers with the caller's own destinations only", async () => {
    await createUser("intake")
    const cookie = await signIn("intake@example.com")
    const { status, body } = await search(cookie, "")
    expect(status).toBe(200)
    const hrefs = body.items.map((hit: { href: string }) => hit.href)
    expect(hrefs).toContain("/admin/leads")
    expect(hrefs).not.toContain("/admin/pages")
    expect(hrefs).not.toContain("/admin/users")
  })

  it("finds Users & roles for an owner by keyword", async () => {
    await createUser("owner")
    const cookie = await signInWithTwoFactor("owner@example.com")
    const { body } = await search(cookie, "invite")
    expect(body.items).toContainEqual(
      expect.objectContaining({ href: "/admin/users", label: "Users & roles" })
    )
  })

  it("rejects an over-long query", async () => {
    await createUser("viewer")
    const cookie = await signIn("viewer@example.com")
    const { status, body } = await search(cookie, "x".repeat(101))
    expect(status).toBe(400)
    expect(body.error.code).toBe("VALIDATION_FAILED")
  })
})
```

Run: `pnpm vitest run --project unit server/modules/search && pnpm test:integration tests/integration/api/search.test.ts`
Expected: FAIL — unresolved `./service`; the integration test gets `404` from the route.

- [ ] **Step 3: Implement the search service and route**

```ts
// server/modules/search/service.ts
import "server-only"

import { reachableLinks } from "@/lib/admin/nav"
import type { RoleName } from "@/lib/auth/permissions"

// Navigation hits only in Phase 2; later phases add pages, articles,
// enquiries and media as further sources (docs/brief.md §9.4).
export type SearchHit = {
  id: string
  kind: "page"
  label: string
  href: string
}

export const normaliseSearch = (text: string) =>
  text
    .normalize("NFKD")
    .replace(/\p{Diacritic}/gu, "")
    .toLocaleLowerCase("en-IE")
    .trim()

export function searchNavigation(
  roles: readonly RoleName[],
  q: string
): SearchHit[] {
  const needle = normaliseSearch(q)
  return reachableLinks(roles)
    .filter(
      (link) =>
        !needle ||
        [link.label, link.parent ?? "", ...(link.keywords ?? [])].some(
          (text) => normaliseSearch(text).includes(needle)
        )
    )
    .map((link) => ({
      id: `nav:${link.id}`,
      kind: "page" as const,
      label: link.parent ? `${link.parent} › ${link.label}` : link.label,
      href: link.href,
    }))
}
```

`searchNavigation(["owner"], "redirects")[0]` is the Redirects child: no other label, section or keyword contains "redirects".

```ts
// server/modules/search/routes.ts
import "server-only"

import { zValidator } from "@hono/zod-validator"
import { Hono } from "hono"
import * as z from "zod"

import { validationHook } from "@/server/api/errors"
import { can } from "@/server/api/middleware/auth"
import type { AppEnv } from "@/server/api/types"

import { searchNavigation } from "./service"

const SearchQuery = z.object({
  q: z.string().trim().max(100).default(""),
})

export const searchRoutes = new Hono<AppEnv>().get(
  "/",
  can({ dashboard: ["view"] }),
  zValidator("query", SearchQuery, validationHook),
  (c) => {
    const { q } = c.req.valid("query")
    return c.json({ items: searchNavigation(c.get("actor")!.roles, q) })
  }
)

export type SearchRoutes = typeof searchRoutes
```

In `server/api/routes/admin.ts` import `searchRoutes` from `@/server/modules/search/routes` and mount it after `.use(twoFactorComplete)`:

```ts
  .use(twoFactorComplete)
  .route("/search", searchRoutes)
  .route("/audit", auditRoutes)
  .route("/users", usersRoutes)
```

Add a row to `tests/integration/api/permission-matrix.test.ts` inside `describe.each(ROLE_NAMES)`:

```ts
  it("GET /search → 200 (dashboard.view)", async () => {
    const response = await adminRequest("/search", await cookieFor(role))
    expect(response.status).toBe(200)
  })
```

Run: `pnpm vitest run --project unit server/modules/search && pnpm test:integration tests/integration/api/search.test.ts tests/integration/api/permission-matrix.test.ts`
Expected: PASS.

- [ ] **Step 4: Write the failing hotkey and actions tests**

```tsx
// admin/components/command/use-command-hotkey.test.tsx
// @vitest-environment jsdom
import { render, screen } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import { describe, expect, it, vi } from "vitest"

import { isPaletteShortcut, useCommandHotkey } from "./use-command-hotkey"

function Probe({ onTrigger }: { onTrigger: () => void }) {
  useCommandHotkey(onTrigger)
  return <input aria-label="Field" />
}

describe("useCommandHotkey", () => {
  it("opens on ⌘K and Ctrl+K", async () => {
    const onTrigger = vi.fn()
    render(<Probe onTrigger={onTrigger} />)
    await userEvent.keyboard("{Meta>}k{/Meta}")
    await userEvent.keyboard("{Control>}k{/Control}")
    expect(onTrigger).toHaveBeenCalledTimes(2)
  })

  // Review Focus #3 and the "no single-key hotkeys" rule.
  it("ignores a bare k or d, including while typing in a field", async () => {
    const onTrigger = vi.fn()
    render(<Probe onTrigger={onTrigger} />)
    await userEvent.keyboard("kd")
    await userEvent.type(screen.getByLabelText("Field"), "kdkd")
    expect(onTrigger).not.toHaveBeenCalled()
  })

  it("leaves ⌘⇧K and ⌥⌘K to the browser", () => {
    const base = { key: "k", metaKey: true, ctrlKey: false }
    expect(isPaletteShortcut({ ...base, shiftKey: true, altKey: false })).toBe(
      false
    )
    expect(isPaletteShortcut({ ...base, shiftKey: false, altKey: true })).toBe(
      false
    )
    expect(isPaletteShortcut({ ...base, shiftKey: false, altKey: false })).toBe(
      true
    )
  })
})
```

```ts
// admin/components/command/actions.test.ts
import { describe, expect, it, vi } from "vitest"

import { buildActions, matchesQuery } from "./actions"

describe("palette actions", () => {
  const setTheme = vi.fn()
  const signOut = vi.fn()
  const actions = buildActions({
    setTheme,
    signOut,
    siteUrl: "https://example.com",
  })

  it("matches by label or keyword", () => {
    const ids = actions
      .filter((action) => matchesQuery("dark", action.label, action.keywords))
      .map((action) => action.id)
    expect(ids).toEqual(["theme-dark"])
    expect(matchesQuery("log out", "Sign out", ["log out"])).toBe(true)
    expect(matchesQuery("", "Anything", [])).toBe(true)
  })

  it("runs the theme switch", () => {
    actions.find((action) => action.id === "theme-dark")!.run()
    expect(setTheme).toHaveBeenCalledWith("dark")
  })
})
```

Run: `pnpm vitest run --project unit admin/components/command`
Expected: FAIL with unresolved `./use-command-hotkey` and `./actions`.

- [ ] **Step 5: Implement the hotkey, actions, context, palette and pill**

```ts
// admin/components/command/use-command-hotkey.ts
"use client"

import { useEffect } from "react"

type KeyLike = Pick<
  KeyboardEvent,
  "key" | "metaKey" | "ctrlKey" | "altKey" | "shiftKey"
>

// ⌘K on macOS, Ctrl+K elsewhere. A modifier is required, so typing in a
// field never opens the palette (no single-key hotkeys).
export function isPaletteShortcut(event: KeyLike) {
  return (
    event.key.toLowerCase() === "k" &&
    (event.metaKey || event.ctrlKey) &&
    !event.altKey &&
    !event.shiftKey
  )
}

export function useCommandHotkey(onTrigger: () => void) {
  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if (event.repeat || !isPaletteShortcut(event)) return
      event.preventDefault()
      onTrigger()
    }
    window.addEventListener("keydown", onKeyDown)
    return () => window.removeEventListener("keydown", onKeyDown)
  }, [onTrigger])
}
```

```ts
// admin/components/command/actions.ts
// Client-side actions in ⌘K (docs/brief.md §9.4). Navigation comes from the
// search API, so the server decides what each role may reach.
export type PaletteAction = {
  id: string
  label: string
  keywords: readonly string[]
  run: () => void
}

const normalise = (text: string) =>
  text
    .normalize("NFKD")
    .replace(/\p{Diacritic}/gu, "")
    .toLocaleLowerCase("en-IE")
    .trim()

export function matchesQuery(
  q: string,
  label: string,
  keywords: readonly string[]
) {
  const needle = normalise(q)
  return (
    !needle || [label, ...keywords].some((text) => normalise(text).includes(needle))
  )
}

export function buildActions({
  setTheme,
  signOut,
  siteUrl,
}: {
  setTheme: (theme: string) => void
  signOut: () => void
  siteUrl: string
}): PaletteAction[] {
  return [
    {
      id: "theme-light",
      label: "Switch to light theme",
      keywords: ["appearance", "mode", "light"],
      run: () => setTheme("light"),
    },
    {
      id: "theme-dark",
      label: "Switch to dark theme",
      keywords: ["appearance", "mode", "night"],
      run: () => setTheme("dark"),
    },
    {
      id: "theme-system",
      label: "Use the system theme",
      keywords: ["appearance", "mode", "auto"],
      run: () => setTheme("system"),
    },
    {
      id: "open-site",
      label: "Open the live site",
      keywords: ["website", "view", "public"],
      run: () => window.open(siteUrl, "_blank", "noopener,noreferrer"),
    },
    {
      id: "sign-out",
      label: "Sign out",
      keywords: ["log out", "logout", "exit"],
      run: signOut,
    },
  ]
}
```

"theme-dark" is the only action matching "dark": "Switch to light theme" and "Use the system theme" contain no "dark", and no keyword does.

```tsx
// admin/components/command/palette-context.tsx
"use client"

import { createContext, use, useCallback, useMemo, useState } from "react"

import { useCommandHotkey } from "./use-command-hotkey"

type PaletteState = { open: boolean; setOpen: (open: boolean) => void }

const PaletteContext = createContext<PaletteState | null>(null)

export function CommandPaletteProvider({
  children,
}: {
  children: React.ReactNode
}) {
  const [open, setOpen] = useState(false)
  const toggle = useCallback(() => setOpen((value) => !value), [])
  useCommandHotkey(toggle)
  const value = useMemo(() => ({ open, setOpen }), [open])
  return <PaletteContext value={value}>{children}</PaletteContext>
}

export function useCommandPalette(): PaletteState {
  const state = use(PaletteContext)
  if (!state) {
    throw new Error("useCommandPalette must be used inside CommandPaletteProvider")
  }
  return state
}
```

In `admin/lib/api.ts` add:

```ts
import type { SearchRoutes } from "@/server/modules/search/routes"
```

```ts
export const searchApi = hc<SearchRoutes>("/api/v1/admin/search")
```

```tsx
// admin/components/command/command-palette.tsx
"use client"

import { keepPreviousData, queryOptions, useQuery } from "@tanstack/react-query"
import { ArrowUpRightIcon, SparklesIcon } from "lucide-react"
import { useRouter } from "next/navigation"
import { useTheme } from "next-themes"
import { useDeferredValue, useMemo, useState } from "react"

import { parseResponse, searchApi } from "@/admin/lib/api"
import { queryKeys } from "@/admin/lib/query-keys"
import { useResetOnHide } from "@/admin/lib/use-reset-on-hide"
import { useSignOut } from "@/admin/modules/auth/use-sign-out"
import {
  Command,
  CommandDialog,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
} from "@/components/ui/command"

import { buildActions, matchesQuery } from "./actions"
import { useCommandPalette } from "./palette-context"

const searchQuery = (q: string) =>
  queryOptions({
    queryKey: queryKeys.search(q),
    queryFn: () => parseResponse(searchApi.index.$get({ query: { q } })),
    staleTime: 60_000,
  })

export function CommandPalette({ siteUrl }: { siteUrl: string }) {
  const { open, setOpen } = useCommandPalette()
  const [q, setQ] = useState("")
  const term = useDeferredValue(q)
  const router = useRouter()
  const { setTheme } = useTheme()
  const { signOut } = useSignOut()
  const { data, isError } = useQuery({
    ...searchQuery(term),
    enabled: open,
    placeholderData: keepPreviousData,
  })
  // A hidden route (<Activity>) must not come back with the palette open.
  useResetOnHide(() => {
    setOpen(false)
    setQ("")
  })

  const actions = useMemo(
    () =>
      buildActions({ setTheme, signOut, siteUrl }).filter((action) =>
        matchesQuery(q, action.label, action.keywords)
      ),
    [q, setTheme, signOut, siteUrl]
  )

  function close() {
    setOpen(false)
    setQ("")
  }

  return (
    <CommandDialog
      open={open}
      onOpenChange={(next) => (next ? setOpen(true) : close())}
      title="Search the admin"
      description="Jump to a page or run an action"
      className="sm:max-w-xl"
    >
      {/* The server already filtered navigation by role and query. */}
      <Command shouldFilter={false} loop>
        <CommandInput
          placeholder="Search pages and actions…"
          value={q}
          onValueChange={setQ}
        />
        <CommandList>
          <CommandEmpty>
            {isError
              ? "Search is unavailable right now. Try again in a moment."
              : data
                ? `Nothing matches “${q}”.`
                : "Searching…"}
          </CommandEmpty>
          {data?.items.length ? (
            <CommandGroup heading="Go to">
              {data.items.map((hit) => (
                <CommandItem
                  key={hit.id}
                  value={hit.id}
                  onSelect={() => {
                    close()
                    router.push(hit.href)
                  }}
                >
                  <ArrowUpRightIcon aria-hidden />
                  {hit.label}
                </CommandItem>
              ))}
            </CommandGroup>
          ) : null}
          {actions.length ? (
            <CommandGroup heading="Actions">
              {actions.map((action) => (
                <CommandItem
                  key={action.id}
                  value={action.id}
                  onSelect={() => {
                    close()
                    action.run()
                  }}
                >
                  <SparklesIcon aria-hidden />
                  {action.label}
                </CommandItem>
              ))}
            </CommandGroup>
          ) : null}
        </CommandList>
      </Command>
    </CommandDialog>
  )
}
```

```tsx
// admin/components/command/search-pill.tsx
"use client"

import { SearchIcon, SlidersHorizontalIcon } from "lucide-react"
import { useSyncExternalStore } from "react"

import { Kbd, KbdGroup } from "@/components/ui/kbd"

import { useCommandPalette } from "./palette-context"

const subscribe = () => () => {}
const isApple = () => /Mac|iPhone|iPad/.test(navigator.userAgent)

// The topbar's search pill with its ⌘K hint (docs/brief.md §9.1). The
// server render assumes ⌘; the browser corrects it without a mismatch.
export function SearchPill() {
  const { setOpen } = useCommandPalette()
  const apple = useSyncExternalStore(subscribe, isApple, () => true)
  return (
    <button
      type="button"
      onClick={() => setOpen(true)}
      aria-label="Search"
      aria-keyshortcuts="Meta+K Control+K"
      className="flex h-11 w-full max-w-sm min-w-0 items-center gap-3 rounded-full bg-card px-4 text-sm text-muted-foreground ring-1 ring-border transition-shadow outline-none hover:ring-ring/40 focus-visible:ring-3 focus-visible:ring-ring/50"
    >
      <SearchIcon aria-hidden className="size-4 shrink-0" />
      <span className="flex-1 truncate text-left">Search</span>
      <KbdGroup aria-hidden className="hidden sm:inline-flex">
        <Kbd>{apple ? "⌘" : "Ctrl"}</Kbd>
        <Kbd>K</Kbd>
      </KbdGroup>
      <SlidersHorizontalIcon aria-hidden className="size-4 shrink-0" />
    </button>
  )
}
```

In `admin/components/shell/topbar.tsx` import `SearchPill` from `@/admin/components/command/search-pill` and replace the slot div with:

```tsx
      <div className="flex min-w-0 flex-1" data-slot="topbar-search">
        <SearchPill />
      </div>
```

Replace `admin/components/shell/admin-shell.tsx` so the palette provider wraps the shell and the palette renders once:

```tsx
"use client"

import { CommandPalette } from "@/admin/components/command/command-palette"
import { CommandPaletteProvider } from "@/admin/components/command/palette-context"
import { SidebarProvider } from "@/components/ui/sidebar"
import { TooltipProvider } from "@/components/ui/tooltip"

import { AppSidebar } from "./app-sidebar"
import { Topbar } from "./topbar"

// Floating panels on the canvas with 16px gutters: a 288px sidebar panel
// (20rem minus the container's p-4), the 72px topbar and the content panel
// (docs/brief.md §9.2).
export function AdminShell({
  defaultOpen,
  siteUrl,
  children,
}: {
  defaultOpen: boolean
  siteUrl: string
  children: React.ReactNode
}) {
  return (
    <TooltipProvider>
      <CommandPaletteProvider>
        <SidebarProvider
          defaultOpen={defaultOpen}
          style={{ "--sidebar-width": "20rem" } as React.CSSProperties}
        >
          <a
            href="#main"
            className="sr-only z-50 rounded-xl bg-card px-4 py-2 text-sm font-semibold shadow-lg focus:not-sr-only focus:fixed focus:top-4 focus:left-4"
          >
            Skip to content
          </a>
          <AppSidebar siteUrl={siteUrl} />
          <div className="flex min-h-svh min-w-0 flex-1 flex-col gap-4 p-3 sm:p-4 lg:pl-0">
            <Topbar />
            <main
              id="main"
              tabIndex={-1}
              className="flex-1 rounded-card bg-sidebar p-4 outline-none sm:p-6 lg:p-8"
            >
              {children}
            </main>
          </div>
        </SidebarProvider>
        <CommandPalette siteUrl={siteUrl} />
      </CommandPaletteProvider>
    </TooltipProvider>
  )
}
```

Run: `pnpm vitest run --project unit admin/components/command`
Expected: PASS.

- [ ] **Step 6: Write the palette E2E**

```ts
// tests/e2e/admin/shell/command-palette.spec.ts
import { expect, test } from "@playwright/test"

import { STORAGE } from "../../support/storage"

test.describe("as an owner", () => {
  test.use({ storageState: STORAGE.owner })

  test("⌘K finds a page by keyword and goes there", async ({ page }) => {
    await page.goto("/admin")
    await page.keyboard.press("ControlOrMeta+k")
    const palette = page.getByRole("dialog", { name: "Search the admin" })
    await expect(palette).toBeVisible()
    await palette.getByRole("combobox").fill("invite")
    await palette.getByRole("option", { name: "Users & roles" }).click()
    await expect(page).toHaveURL(/\/admin\/users$/)
    await expect(palette).toBeHidden()
  })

  test("the search pill opens it and Escape closes it", async ({ page }) => {
    await page.goto("/admin")
    const pill = page.getByRole("button", { name: "Search", exact: true })
    await pill.click()
    const palette = page.getByRole("dialog", { name: "Search the admin" })
    await expect(palette).toBeVisible()
    await page.keyboard.press("Escape")
    await expect(palette).toBeHidden()
    await expect(pill).toBeFocused()
  })

  test("runs an action", async ({ page }) => {
    await page.emulateMedia({ colorScheme: "light" })
    await page.goto("/admin")
    await page.keyboard.press("ControlOrMeta+k")
    const palette = page.getByRole("dialog", { name: "Search the admin" })
    await palette.getByRole("combobox").fill("dark")
    await palette.getByRole("option", { name: "Switch to dark theme" }).click()
    await expect(page.locator("html")).toHaveClass(/\bdark\b/)
  })
})

// Review Focus #3.
test.describe("as intake", () => {
  test.use({ storageState: STORAGE.intake })

  test("lists only what intake can open", async ({ page }) => {
    await page.goto("/admin")
    await page.keyboard.press("ControlOrMeta+k")
    const palette = page.getByRole("dialog", { name: "Search the admin" })
    const goTo = palette.getByRole("group", { name: "Go to" })
    await expect(goTo.getByRole("option")).toHaveText([
      "Dashboard",
      "Enquiries",
      "Help",
      "Account",
    ])
    await palette.getByRole("combobox").fill("users")
    await expect(palette.getByRole("option", { name: "Users & roles" })).toHaveCount(0)
  })
})
```

Run: `pnpm test:e2e tests/e2e/admin/shell/command-palette.spec.ts`
Expected: PASS on all three projects. (`ControlOrMeta` resolves to ⌘ on macOS and Ctrl on Linux CI.)

- [ ] **Step 7: Gates**

Run: `pnpm lint && pnpm typecheck && pnpm test:unit && pnpm test:integration && pnpm build`
Expected: all green.

- [ ] **Step 8: Commit**

```bash
git add components/ui server/modules/search server/api/routes/admin.ts admin tests/integration/api tests/e2e/admin/shell/command-palette.spec.ts package.json pnpm-lock.yaml
git commit -m "feat(admin): ⌘K palette with a role-aware navigation search API"
```

---

## Task 2.7: Dashboard primitives and the owner-only `/admin/design` showcase

**Files:**
- Create (shadcn CLI): `components/ui/{chart,empty,item}.tsx` (adds `recharts@3.8.0`)
- Modify: `package.json` (`react-is@19.2.8`)
- Create: `admin/components/noise.ts`; Modify: `app/(admin)/admin/(auth)/layout.tsx` (import `NOISE`)
- Create: `admin/lib/use-reduced-motion.ts`, `admin/lib/use-reduced-motion.test.tsx`
- Create: `admin/components/dashboard/{format.ts,format.test.ts,chart-math.ts,chart-math.test.ts}`
- Create: `admin/components/dashboard/{page-header,dashboard-card,kpi-card,pill-bar-chart,gauge,status-pill,activity-feed,empty-state,skeletons}.tsx`
- Create: `admin/components/dashboard/{kpi-card.test.tsx,status-pill.test.tsx,activity-feed.test.tsx}`
- Create: `admin/modules/design/{sample-data.ts,showcase.tsx}`
- Create: `app/(admin)/admin/(panel)/design/page.tsx`
- Create: `tests/e2e/admin/shell/design.spec.ts`

**Interfaces:**
- Consumes: Task 2.1 utilities (`bg-hero`, `bg-hatch`, `text-kpi`, `text-title`, `text-card-title`, `rounded-card`, status colours); `requirePermission` (2.3); `initials` (2.5); `Avatar`, `ScrollArea`, `Skeleton` (2.5).
- Produces (all in `admin/components/dashboard/`):
  - `format.ts`: `formatNumber(n)`, `formatPercent(ratio)`, `type Delta = { label: string; direction: "up" | "down" | "flat" }`, `formatDelta(current, previous): Delta | null`, `relativeTime(iso: string, now: number): string`.
  - `chart-math.ts`: `type GaugePart = { key: string; label: string; value: number; tone: "solid" | "dark" | "hatched" }`, `gaugeSummary(parts): { parts: GaugePart[]; total: number; ratio: number }`, `peakIndex(values: readonly number[]): number`.
  - `PageHeader({ title, description?, actions?, breadcrumbs?, titleTestId? })`, `PRIMARY_ACTION`, `SECONDARY_ACTION` (button-pair classes).
  - `DashboardCard({ title, action?, children, className? })` (a `<section>` with an `<h2>`).
  - `KpiCard({ label, value, caption, delta?, href?, variant?: "hero" | "default" })`.
  - `PillBarChart({ title, data: readonly PillBarDatum[], currentLabel, previousLabel })`, `type PillBarDatum = { key: string; label: string; current: number; previous: number }`.
  - `Gauge({ title, parts: readonly GaugePart[], centerLabel })`.
  - `StatusPill({ tone, children })`, `type StatusTone = "success" | "warning" | "danger" | "info" | "neutral"`, `toneForStatus(status: string): StatusTone`.
  - `ActivityFeed({ entries: readonly ActivityEntry[], now: number, emptyTitle, emptyDescription })`, `type ActivityEntry = { id: string; name: string; description: string; at: string; status?: { label: string; tone: StatusTone } }`.
  - `EmptyState({ title, description, action?, icon? })`; `KpiRowSkeleton()`, `CardSkeleton({ className? })`.
  - `admin/lib/use-reduced-motion.ts`: `useReducedMotion(): boolean` (server snapshot `true`).

- [ ] **Step 1: Add the components and the Recharts peer**

Run: `pnpm exec shadcn add chart empty item --yes && pnpm add react-is@19.2.8`
Expected: `components/ui/chart.tsx`, `empty.tsx`, `item.tsx`; `recharts` `3.8.0` in `package.json`.

Run: `pnpm ls recharts react-is`
Expected: `recharts 3.8.0`, `react-is 19.2.8`.

- [ ] **Step 2: Write the failing unit tests**

```ts
// admin/components/dashboard/format.test.ts
import { describe, expect, it } from "vitest"

import { formatDelta, formatNumber, formatPercent, relativeTime } from "./format"

describe("formatDelta", () => {
  it("signs the change", () => {
    expect(formatDelta(112.5, 100)).toEqual({ label: "+12.5%", direction: "up" })
    expect(formatDelta(50, 100)).toEqual({ label: "-50%", direction: "down" })
  })

  it("calls a tiny change no change", () => {
    expect(formatDelta(1000, 1000.1)).toEqual({
      label: "No change",
      direction: "flat",
    })
  })

  it("has no percentage without a baseline", () => {
    expect(formatDelta(4, 0)).toBeNull()
    expect(formatDelta(0, 0)).toEqual({ label: "No change", direction: "flat" })
  })
})

describe("formatting", () => {
  it("uses Irish English", () => {
    expect(formatNumber(12345)).toBe("12,345")
    expect(formatPercent(0.41)).toBe("41%")
  })

  it("says how long ago", () => {
    const now = Date.parse("2026-10-09T10:00:00Z")
    expect(relativeTime("2026-10-09T09:59:30Z", now)).toBe("just now")
    expect(relativeTime("2026-10-09T09:55:00Z", now)).toBe("5 minutes ago")
    expect(relativeTime("2026-10-09T08:00:00Z", now)).toBe("2 hours ago")
    expect(relativeTime("2026-10-08T10:00:00Z", now)).toBe("yesterday")
  })
})
```

```ts
// admin/components/dashboard/chart-math.test.ts
import { describe, expect, it } from "vitest"

import { gaugeSummary, peakIndex } from "./chart-math"

describe("gaugeSummary", () => {
  it("shares the first segment of the total", () => {
    const summary = gaugeSummary([
      { key: "done", label: "Complete", value: 41, tone: "solid" },
      { key: "work", label: "Needs work", value: 30, tone: "dark" },
      { key: "missing", label: "Missing", value: 29, tone: "hatched" },
    ])
    expect(summary.total).toBe(100)
    expect(summary.ratio).toBeCloseTo(0.41)
  })

  it("treats negative or missing values as zero", () => {
    const summary = gaugeSummary([
      { key: "a", label: "A", value: -3, tone: "solid" },
      { key: "b", label: "B", value: Number.NaN, tone: "dark" },
    ])
    expect(summary).toMatchObject({ total: 0, ratio: 0 })
    expect(summary.parts.map((part) => part.value)).toEqual([0, 0])
  })
})

describe("peakIndex", () => {
  it("finds the first highest value", () => {
    expect(peakIndex([3, 9, 4, 9])).toBe(1)
    expect(peakIndex([])).toBe(-1)
  })
})
```

```tsx
// admin/components/dashboard/kpi-card.test.tsx
// @vitest-environment jsdom
import { render, screen } from "@testing-library/react"
import { expect, it, vi } from "vitest"

import { KpiCard } from "./kpi-card"

// next/link needs the App Router at runtime; a plain anchor is enough here.
vi.mock("next/link", () => ({
  default: ({
    href,
    children,
    ...props
  }: React.ComponentProps<"a"> & { href: string }) => (
    <a href={href} {...props}>
      {children}
    </a>
  ),
}))

it("renders the hero KPI with its delta and arrow chip", () => {
  render(
    <KpiCard
      variant="hero"
      label="New enquiries"
      value="24"
      delta={{ label: "+3.4%", direction: "up" }}
      caption="vs last month"
      href="/admin/leads"
    />
  )
  const card = screen.getByRole("article")
  expect(card.dataset.variant).toBe("hero")
  expect(screen.getByRole("heading", { name: "New enquiries" })).toBeTruthy()
  expect(card.textContent).toContain("+3.4% vs last month")
  expect(
    screen.getByRole("link", { name: "Open New enquiries" }).getAttribute("href")
  ).toBe("/admin/leads")
})

it("renders without a chip or delta", () => {
  render(<KpiCard label="Subscribers" value="312" caption="All time" />)
  expect(screen.queryByRole("link")).toBeNull()
  expect(screen.getByRole("article").dataset.variant).toBe("default")
})
```

```tsx
// admin/components/dashboard/status-pill.test.tsx
// @vitest-environment jsdom
import { render, screen } from "@testing-library/react"
import { expect, it } from "vitest"

import { StatusPill, toneForStatus } from "./status-pill"

it.each([
  ["Published", "success"],
  ["draft", "neutral"],
  ["Scheduled", "info"],
  ["In progress", "warning"],
  ["Pending", "danger"],
  ["Something else", "neutral"],
])("toneForStatus(%j) is %s", (status, tone) => {
  expect(toneForStatus(status)).toBe(tone)
})

it("pairs a soft fill with strong text", () => {
  render(<StatusPill tone="success">Published</StatusPill>)
  const pill = screen.getByText("Published")
  expect(pill.className).toContain("bg-success-soft")
  expect(pill.className).toContain("text-success")
})
```

```tsx
// admin/components/dashboard/activity-feed.test.tsx
// @vitest-environment jsdom
import { render, screen } from "@testing-library/react"
import { expect, it } from "vitest"

import { ActivityFeed } from "./activity-feed"

const now = Date.parse("2026-10-09T10:00:00Z")

it("lists entries with avatar initials, relative time and status", () => {
  render(
    <ActivityFeed
      now={now}
      emptyTitle="Nothing yet"
      emptyDescription="Activity shows here."
      entries={[
        {
          id: "1",
          name: "Niamh Walsh",
          description: "Published “Sleep and stress”",
          at: "2026-10-09T09:55:00Z",
          status: { label: "Published", tone: "success" },
        },
      ]}
    />
  )
  const items = screen.getAllByRole("listitem")
  expect(items).toHaveLength(1)
  expect(items[0].textContent).toContain("NW")
  expect(items[0].textContent).toContain("5 minutes ago")
  expect(screen.getByText("Published").dataset.tone).toBe("success")
})

it("shows the empty state when there is nothing", () => {
  render(
    <ActivityFeed
      now={now}
      entries={[]}
      emptyTitle="Nothing yet"
      emptyDescription="Activity shows here."
    />
  )
  expect(screen.getByText("Nothing yet")).toBeTruthy()
  expect(screen.queryByRole("listitem")).toBeNull()
})
```

```tsx
// admin/lib/use-reduced-motion.test.tsx
// @vitest-environment jsdom
import { renderHook } from "@testing-library/react"
import { expect, it, vi } from "vitest"

import { useReducedMotion } from "./use-reduced-motion"

function prefersReduced(matches: boolean) {
  Object.defineProperty(window, "matchMedia", {
    configurable: true,
    writable: true,
    value: vi.fn((query: string) => ({
      matches,
      media: query,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
    })),
  })
}

it.each([true, false])("follows prefers-reduced-motion = %s", (matches) => {
  prefersReduced(matches)
  expect(renderHook(() => useReducedMotion()).result.current).toBe(matches)
})
```

Run: `pnpm vitest run --project unit admin/components/dashboard admin/lib/use-reduced-motion.test.tsx`
Expected: FAIL with unresolved imports for every module under test.

- [ ] **Step 3: Implement the pure helpers and the motion hook**

```ts
// admin/components/dashboard/format.ts
const NUMBER = new Intl.NumberFormat("en-IE")
const PERCENT = new Intl.NumberFormat("en-IE", {
  style: "percent",
  maximumFractionDigits: 0,
})
const CHANGE = new Intl.NumberFormat("en-IE", {
  style: "percent",
  maximumFractionDigits: 1,
  signDisplay: "exceptZero",
})
const RELATIVE = new Intl.RelativeTimeFormat("en-IE", { numeric: "auto" })

export const formatNumber = (n: number) => NUMBER.format(n)
export const formatPercent = (ratio: number) => PERCENT.format(ratio)

export type Delta = { label: string; direction: "up" | "down" | "flat" }

// The KPI caption's "+3.4%". Without a baseline there is no honest
// percentage, so the caller shows the caption alone.
export function formatDelta(current: number, previous: number): Delta | null {
  if (previous === 0) {
    return current === 0 ? { label: "No change", direction: "flat" } : null
  }
  const change = (current - previous) / previous
  if (Math.abs(change) < 0.0005) return { label: "No change", direction: "flat" }
  return { label: CHANGE.format(change), direction: change > 0 ? "up" : "down" }
}

const UNITS: [Intl.RelativeTimeFormatUnit, number][] = [
  ["year", 31_536_000],
  ["month", 2_592_000],
  ["week", 604_800],
  ["day", 86_400],
  ["hour", 3_600],
  ["minute", 60],
]

// `now` is passed in so server and browser render the same words.
export function relativeTime(iso: string, now: number) {
  const seconds = Math.round((Date.parse(iso) - now) / 1000)
  if (Math.abs(seconds) < 45) return "just now"
  for (const [unit, size] of UNITS) {
    if (Math.abs(seconds) >= size) {
      return RELATIVE.format(Math.round(seconds / size), unit)
    }
  }
  return RELATIVE.format(Math.round(seconds / 60), "minute")
}
```

```ts
// admin/components/dashboard/chart-math.ts
export type GaugePart = {
  key: string
  label: string
  value: number
  tone: "solid" | "dark" | "hatched"
}

// The gauge's centred percentage is the first segment's share.
export function gaugeSummary(parts: readonly GaugePart[]) {
  const clean = parts.map((part) => ({
    ...part,
    value: Number.isFinite(part.value) ? Math.max(0, part.value) : 0,
  }))
  const total = clean.reduce((sum, part) => sum + part.value, 0)
  return {
    parts: clean,
    total,
    ratio: total === 0 ? 0 : (clean[0]?.value ?? 0) / total,
  }
}

// Where the pill chart's floating value tag sits.
export function peakIndex(values: readonly number[]) {
  let best = -1
  values.forEach((value, index) => {
    if (best === -1 || value > values[best]!) best = index
  })
  return best
}
```

```ts
// admin/lib/use-reduced-motion.ts
"use client"

import { useSyncExternalStore } from "react"

const QUERY = "(prefers-reduced-motion: reduce)"

function subscribe(onChange: () => void) {
  const media = window.matchMedia(QUERY)
  media.addEventListener("change", onChange)
  return () => media.removeEventListener("change", onChange)
}

// Charts animate only when the user has not asked for less motion
// (docs/brief.md §9.2). The server assumes reduced: nothing animates there.
export function useReducedMotion() {
  return useSyncExternalStore(
    subscribe,
    () => window.matchMedia(QUERY).matches,
    () => true
  )
}
```

- [ ] **Step 4: Implement the components**

```ts
// admin/components/noise.ts
// Fine grain over the dark-green gradients (docs/brief.md §9.2).
export const NOISE =
  "url(\"data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='160' height='160'%3E%3Cfilter id='n'%3E%3CfeTurbulence type='fractalNoise' baseFrequency='0.9' numOctaves='2' stitchTiles='stitch'/%3E%3C/filter%3E%3Crect width='100%25' height='100%25' filter='url(%23n)'/%3E%3C/svg%3E\")"
```

In `app/(admin)/admin/(auth)/layout.tsx`, delete the local `NOISE` constant and its comment and add `import { NOISE } from "@/admin/components/noise"`.

```tsx
// admin/components/dashboard/page-header.tsx
// "Good morning, Magda" with the button pair (docs/brief.md §9.3).
export const PRIMARY_ACTION =
  "h-12 gap-2 rounded-xl px-5 text-[15px] font-semibold"
export const SECONDARY_ACTION =
  "h-12 gap-2 rounded-xl border-primary px-5 text-[15px] font-semibold text-primary hover:bg-accent hover:text-accent-foreground"

export function PageHeader({
  title,
  description,
  actions,
  breadcrumbs,
  titleTestId,
}: {
  title: React.ReactNode
  description?: React.ReactNode
  actions?: React.ReactNode
  breadcrumbs?: React.ReactNode
  titleTestId?: string
}) {
  return (
    <header className="flex flex-col gap-4 lg:flex-row lg:items-end lg:justify-between">
      <div className="flex min-w-0 flex-col gap-2">
        {breadcrumbs}
        <h1 className="text-title text-balance" data-testid={titleTestId}>
          {title}
        </h1>
        {description ? (
          <p className="text-[15px] text-muted-foreground">{description}</p>
        ) : null}
      </div>
      {actions ? <div className="flex flex-wrap gap-3">{actions}</div> : null}
    </header>
  )
}
```

```tsx
// admin/components/dashboard/dashboard-card.tsx
import { useId } from "react"

import { cn } from "@/lib/utils"

export function DashboardCard({
  title,
  action,
  children,
  className,
}: {
  title: string
  action?: React.ReactNode
  children: React.ReactNode
  className?: string
}) {
  const id = useId()
  return (
    <section
      aria-labelledby={id}
      className={cn(
        "flex min-w-0 flex-col gap-5 rounded-card bg-card p-5 text-card-foreground sm:p-6",
        className
      )}
    >
      <div className="flex items-center justify-between gap-3">
        <h2 id={id} className="text-card-title">
          {title}
        </h2>
        {action}
      </div>
      {children}
    </section>
  )
}
```

```tsx
// admin/components/dashboard/kpi-card.tsx
import { ArrowUpRightIcon } from "lucide-react"
import Link from "next/link"

import { NOISE } from "@/admin/components/noise"
import { cn } from "@/lib/utils"

import type { Delta } from "./format"

// The hero KPI (deep-green gradient, white text, filled arrow chip) and the
// default KPI (outlined chip, delta caption): docs/brief.md §9.2.
export function KpiCard({
  label,
  value,
  caption,
  delta,
  href,
  variant = "default",
}: {
  label: string
  value: string
  caption: string
  delta?: Delta | null
  href?: string
  variant?: "hero" | "default"
}) {
  const hero = variant === "hero"
  return (
    <article
      data-variant={variant}
      className={cn(
        "relative isolate flex min-h-[164px] flex-col justify-between gap-6 overflow-hidden rounded-card p-5 sm:p-6",
        hero ? "bg-hero" : "bg-card text-card-foreground"
      )}
    >
      {hero ? (
        <div
          aria-hidden
          className="pointer-events-none absolute inset-0 -z-10 opacity-[0.14] mix-blend-overlay"
          style={{ backgroundImage: NOISE }}
        />
      ) : null}
      <div className="flex items-start justify-between gap-3">
        <h3 className="text-[15px] leading-snug font-semibold">{label}</h3>
        {href ? (
          <Link
            href={href}
            aria-label={`Open ${label}`}
            className={cn(
              "flex size-10 shrink-0 items-center justify-center rounded-full outline-none focus-visible:ring-3",
              hero
                ? "bg-white text-foreground focus-visible:ring-white/60 dark:text-background"
                : "ring-1 ring-foreground/70 hover:bg-muted focus-visible:ring-ring/50"
            )}
          >
            <ArrowUpRightIcon aria-hidden className="size-5" />
          </Link>
        ) : null}
      </div>
      <div className="flex flex-col gap-2">
        <p className="text-kpi tabular-nums">{value}</p>
        <p
          className={cn(
            "text-xs font-medium",
            hero ? "opacity-90" : "text-muted-foreground"
          )}
        >
          {delta ? (
            <span
              className={cn(
                "font-semibold",
                !hero && delta.direction === "up" && "text-success",
                !hero && delta.direction === "down" && "text-danger"
              )}
            >
              {delta.label}
            </span>
          ) : null}
          {delta ? " " : null}
          {caption}
        </p>
      </div>
    </article>
  )
}
```

```tsx
// admin/components/dashboard/status-pill.tsx
import { cn } from "@/lib/utils"

export type StatusTone = "success" | "warning" | "danger" | "info" | "neutral"

const TONE_CLASS: Record<StatusTone, string> = {
  success: "bg-success-soft text-success",
  warning: "bg-warning-soft text-warning",
  danger: "bg-danger-soft text-danger",
  info: "bg-accent text-accent-foreground",
  neutral: "bg-muted text-muted-foreground",
}

// Published / Draft / Scheduled / New / Contacted… (docs/brief.md §9.2).
const STATUS_TONES: Record<string, StatusTone> = {
  published: "success",
  completed: "success",
  booked: "success",
  "signed in": "success",
  draft: "neutral",
  closed: "neutral",
  scheduled: "info",
  new: "info",
  invited: "info",
  "in progress": "warning",
  contacted: "warning",
  pending: "danger",
  failed: "danger",
}

export const toneForStatus = (status: string): StatusTone =>
  STATUS_TONES[status.trim().toLowerCase()] ?? "neutral"

export function StatusPill({
  tone,
  children,
  className,
}: {
  tone: StatusTone
  children: React.ReactNode
  className?: string
}) {
  return (
    <span
      data-tone={tone}
      className={cn(
        "inline-flex h-7 shrink-0 items-center rounded-full px-3 text-xs font-semibold whitespace-nowrap",
        TONE_CLASS[tone],
        className
      )}
    >
      {children}
    </span>
  )
}
```

```tsx
// admin/components/dashboard/empty-state.tsx
import type { LucideIcon } from "lucide-react"

import {
  Empty,
  EmptyContent,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty"

// Empty states always say what will appear and, when there is one, offer
// the next step (docs/brief.md §9.4).
export function EmptyState({
  title,
  description,
  action,
  icon: Icon,
}: {
  title: string
  description: string
  action?: React.ReactNode
  icon?: LucideIcon
}) {
  return (
    <Empty className="rounded-card border border-dashed">
      <EmptyHeader>
        {Icon ? (
          <EmptyMedia variant="icon">
            <Icon />
          </EmptyMedia>
        ) : null}
        <EmptyTitle className="text-base font-semibold">{title}</EmptyTitle>
        <EmptyDescription>{description}</EmptyDescription>
      </EmptyHeader>
      {action ? <EmptyContent>{action}</EmptyContent> : null}
    </Empty>
  )
}
```

```tsx
// admin/components/dashboard/activity-feed.tsx
import { initials } from "@/admin/lib/initials"
import { Avatar, AvatarFallback } from "@/components/ui/avatar"
import {
  Item,
  ItemActions,
  ItemContent,
  ItemDescription,
  ItemGroup,
  ItemMedia,
  ItemTitle,
} from "@/components/ui/item"
import { ScrollArea } from "@/components/ui/scroll-area"

import { EmptyState } from "./empty-state"
import { relativeTime } from "./format"
import { StatusPill, type StatusTone } from "./status-pill"

export type ActivityEntry = {
  id: string
  name: string
  description: string
  at: string
  status?: { label: string; tone: StatusTone }
}

// The inspiration's "Team Collaboration" list as team activity.
export function ActivityFeed({
  entries,
  now,
  emptyTitle,
  emptyDescription,
}: {
  entries: readonly ActivityEntry[]
  now: number
  emptyTitle: string
  emptyDescription: string
}) {
  if (entries.length === 0) {
    return <EmptyState title={emptyTitle} description={emptyDescription} />
  }
  return (
    <ScrollArea className="max-h-[360px]">
      <ItemGroup className="gap-1">
        {entries.map((entry) => (
          <Item key={entry.id} role="listitem" size="sm" className="px-0">
            <ItemMedia>
              <Avatar className="size-10">
                <AvatarFallback className="bg-accent font-semibold text-accent-foreground">
                  {initials(entry.name)}
                </AvatarFallback>
              </Avatar>
            </ItemMedia>
            <ItemContent className="min-w-0">
              <ItemTitle>{entry.name}</ItemTitle>
              <ItemDescription>
                {entry.description} ·{" "}
                <time dateTime={entry.at}>{relativeTime(entry.at, now)}</time>
              </ItemDescription>
            </ItemContent>
            {entry.status ? (
              <ItemActions>
                <StatusPill tone={entry.status.tone}>
                  {entry.status.label}
                </StatusPill>
              </ItemActions>
            ) : null}
          </Item>
        ))}
      </ItemGroup>
    </ScrollArea>
  )
}
```

```tsx
// admin/components/dashboard/pill-bar-chart.tsx
"use client"

import { useId } from "react"
import { Bar, BarChart, Rectangle, XAxis, type BarShapeProps } from "recharts"

import { useReducedMotion } from "@/admin/lib/use-reduced-motion"
import { ChartContainer, type ChartConfig } from "@/components/ui/chart"

import { peakIndex } from "./chart-math"
import { formatNumber } from "./format"

export type PillBarDatum = {
  key: string
  label: string
  current: number
  previous: number
}

// Capsule bars, hatched for the comparison period, with a floating value
// tag on the busiest bar (docs/brief.md §9.2, signature component 3). The
// SVG is hidden from assistive tech; the table below carries the numbers.
export function PillBarChart({
  title,
  data,
  currentLabel,
  previousLabel,
}: {
  title: string
  data: readonly PillBarDatum[]
  currentLabel: string
  previousLabel: string
}) {
  const hatchId = `hatch-${useId().replace(/:/g, "")}`
  const reduced = useReducedMotion()
  const peak = peakIndex(data.map((datum) => datum.current))
  const config = {
    current: { label: currentLabel, color: "var(--chart-2)" },
    previous: { label: previousLabel, color: "var(--hatch)" },
  } satisfies ChartConfig

  return (
    <figure className="flex flex-col gap-4">
      <ChartContainer
        config={config}
        className="aspect-auto h-60 w-full"
        aria-hidden
      >
        <BarChart
          data={[...data]}
          barGap={6}
          barCategoryGap="18%"
          margin={{ top: 40, right: 4, bottom: 0, left: 4 }}
          accessibilityLayer={false}
        >
          <defs>
            <pattern
              id={hatchId}
              width="8"
              height="8"
              patternUnits="userSpaceOnUse"
              patternTransform="rotate(45)"
            >
              <rect width="8" height="8" fill="var(--color-previous)" fillOpacity={0.25} />
              <rect width="3.5" height="8" fill="var(--color-previous)" />
            </pattern>
          </defs>
          <XAxis
            dataKey="label"
            tickLine={false}
            axisLine={false}
            tickMargin={10}
          />
          <Bar
            dataKey="previous"
            fill={`url(#${hatchId})`}
            radius={999}
            isAnimationActive={!reduced}
          />
          <Bar
            dataKey="current"
            fill="var(--color-current)"
            radius={999}
            isAnimationActive={!reduced}
            shape={(props: BarShapeProps) => (
              <PillWithTag {...props} showTag={props.index === peak} />
            )}
          />
        </BarChart>
      </ChartContainer>
      <figcaption className="flex flex-wrap items-center gap-4 text-xs text-muted-foreground">
        <span className="flex items-center gap-2">
          <span aria-hidden className="size-3 rounded-full bg-chart-2" />
          {currentLabel}
        </span>
        <span className="flex items-center gap-2">
          <span aria-hidden className="size-3 rounded-full bg-hatch" />
          {previousLabel}
        </span>
      </figcaption>
      <table className="sr-only">
        <caption>{title}</caption>
        <thead>
          <tr>
            <th scope="col">Day</th>
            <th scope="col">{currentLabel}</th>
            <th scope="col">{previousLabel}</th>
          </tr>
        </thead>
        <tbody>
          {data.map((datum) => (
            <tr key={datum.key}>
              <th scope="row">{datum.label}</th>
              <td>{datum.current}</td>
              <td>{datum.previous}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </figure>
  )
}

function PillWithTag({
  showTag,
  ...props
}: BarShapeProps & { showTag: boolean }) {
  const { x, y, width, height, fill } = props
  const value = formatNumber((props.payload as PillBarDatum).current)
  const tagWidth = Math.max(36, value.length * 8 + 18)
  return (
    <g>
      <Rectangle
        x={x}
        y={y}
        width={width}
        height={height}
        fill={fill}
        radius={999}
      />
      {showTag ? (
        <g transform={`translate(${x + width / 2}, ${y - 10})`}>
          <rect
            x={-tagWidth / 2}
            y={-26}
            width={tagWidth}
            height={22}
            rx={8}
            fill="var(--card)"
            stroke="var(--border)"
          />
          <text
            y={-11}
            textAnchor="middle"
            fontSize={11}
            fontWeight={600}
            fill="var(--foreground)"
          >
            {value}
          </text>
          <circle r={4} fill="var(--card)" stroke="var(--color-current)" strokeWidth={2} />
        </g>
      ) : null}
    </g>
  )
}
```

```tsx
// admin/components/dashboard/gauge.tsx
"use client"

import { useId } from "react"
import { RadialBar, RadialBarChart } from "recharts"

import { useReducedMotion } from "@/admin/lib/use-reduced-motion"
import { ChartContainer, type ChartConfig } from "@/components/ui/chart"
import { cn } from "@/lib/utils"

import { gaugeSummary, type GaugePart } from "./chart-math"
import { formatNumber, formatPercent } from "./format"

const SWATCH: Record<GaugePart["tone"], string> = {
  solid: "bg-chart-2",
  dark: "bg-chart-4",
  hatched: "bg-hatch",
}

// Semicircle of stacked solid, dark and hatched arcs with a big centred
// percentage and a legend (docs/brief.md §9.2, signature component 4).
export function Gauge({
  title,
  parts,
  centerLabel,
}: {
  title: string
  parts: readonly GaugePart[]
  centerLabel: string
}) {
  const hatchId = `gauge-hatch-${useId().replace(/:/g, "")}`
  const reduced = useReducedMotion()
  const summary = gaugeSummary(parts)
  const fill: Record<GaugePart["tone"], string> = {
    solid: "var(--chart-2)",
    dark: "var(--chart-4)",
    hatched: `url(#${hatchId})`,
  }
  const row: Record<string, number> = summary.total
    ? Object.fromEntries(summary.parts.map((part) => [part.key, part.value]))
    : { empty: 1 }
  const config = Object.fromEntries(
    summary.parts.map((part) => [part.key, { label: part.label }])
  ) satisfies ChartConfig

  return (
    <figure className="flex flex-col items-center gap-5">
      <div className="relative w-full max-w-xs">
        <ChartContainer
          config={config}
          className="aspect-[2/1] w-full"
          aria-hidden
        >
          <RadialBarChart
            data={[row]}
            startAngle={180}
            endAngle={0}
            cy="100%"
            innerRadius="150%"
            outerRadius="200%"
            margin={{ top: 8, right: 8, bottom: 0, left: 8 }}
            accessibilityLayer={false}
          >
            <defs>
              <pattern
                id={hatchId}
                width="8"
                height="8"
                patternUnits="userSpaceOnUse"
                patternTransform="rotate(45)"
              >
                <rect width="8" height="8" fill="var(--hatch)" fillOpacity={0.25} />
                <rect width="3.5" height="8" fill="var(--hatch)" />
              </pattern>
            </defs>
            {summary.total ? (
              summary.parts.map((part) => (
                <RadialBar
                  key={part.key}
                  dataKey={part.key}
                  stackId="gauge"
                  fill={fill[part.tone]}
                  cornerRadius={12}
                  isAnimationActive={!reduced}
                />
              ))
            ) : (
              <RadialBar
                dataKey="empty"
                fill="var(--muted)"
                cornerRadius={12}
                isAnimationActive={false}
              />
            )}
          </RadialBarChart>
        </ChartContainer>
        <div className="absolute inset-x-0 bottom-0 flex flex-col items-center">
          <p className="text-kpi tabular-nums">
            {summary.total ? formatPercent(summary.ratio) : "—"}
          </p>
          <p className="text-sm text-muted-foreground">{centerLabel}</p>
        </div>
      </div>
      <figcaption>
        <ul className="flex flex-wrap justify-center gap-x-5 gap-y-2 text-xs font-medium">
          {summary.parts.map((part) => (
            <li key={part.key} className="flex items-center gap-2">
              <span
                aria-hidden
                className={cn("size-4 rounded-full", SWATCH[part.tone])}
              />
              {part.label}
              <span className="text-muted-foreground tabular-nums">
                {formatNumber(part.value)}
              </span>
            </li>
          ))}
        </ul>
      </figcaption>
      <table className="sr-only">
        <caption>{title}</caption>
        <tbody>
          {summary.parts.map((part) => (
            <tr key={part.key}>
              <th scope="row">{part.label}</th>
              <td>{part.value}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </figure>
  )
}
```

```tsx
// admin/components/dashboard/skeletons.tsx
import { Skeleton } from "@/components/ui/skeleton"
import { cn } from "@/lib/utils"

export function KpiRowSkeleton() {
  return (
    <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-4" aria-hidden>
      {[0, 1, 2, 3].map((index) => (
        <Skeleton key={index} className="h-[164px] rounded-card" />
      ))}
    </div>
  )
}

export function CardSkeleton({ className }: { className?: string }) {
  return (
    <Skeleton aria-hidden className={cn("h-72 rounded-card", className)} />
  )
}
```

Run: `pnpm vitest run --project unit admin/components/dashboard admin/lib/use-reduced-motion.test.tsx`
Expected: PASS.

- [ ] **Step 5: Build the showcase on sample data**

```ts
// admin/modules/design/sample-data.ts
import type { GaugePart } from "@/admin/components/dashboard/chart-math"
import type { ActivityEntry } from "@/admin/components/dashboard/activity-feed"
import type { PillBarDatum } from "@/admin/components/dashboard/pill-bar-chart"

// Fixed sample data for client demos and visual tests. Real dashboard data
// arrives in Task 2.8 (what exists now) and Phase 11 (enquiries, content).
export const SAMPLE_NOW = Date.parse("2026-10-09T10:00:00Z")

export const SAMPLE_KPIS = [
  {
    label: "New enquiries",
    value: "24",
    delta: { label: "+3.4%", direction: "up" as const },
    caption: "vs last month",
    href: "/admin/leads",
    variant: "hero" as const,
  },
  {
    label: "Published articles",
    value: "10",
    delta: { label: "+2.1%", direction: "up" as const },
    caption: "vs last month",
    href: "/admin/insights",
  },
  {
    label: "Newsletter subscribers",
    value: "312",
    delta: { label: "+1.1%", direction: "up" as const },
    caption: "vs last month",
    href: "/admin/newsletter",
  },
  {
    label: "SEO health",
    value: "86%",
    delta: null,
    caption: "4 pages need attention",
    href: "/admin/seo",
  },
]

export const SAMPLE_WEEK: PillBarDatum[] = [
  { key: "mon", label: "Mon", current: 4, previous: 3 },
  { key: "tue", label: "Tue", current: 6, previous: 5 },
  { key: "wed", label: "Wed", current: 5, previous: 7 },
  { key: "thu", label: "Thu", current: 9, previous: 6 },
  { key: "fri", label: "Fri", current: 3, previous: 8 },
  { key: "sat", label: "Sat", current: 1, previous: 2 },
  { key: "sun", label: "Sun", current: 2, previous: 3 },
]

export const SAMPLE_TEAM: ActivityEntry[] = [
  {
    id: "a1",
    name: "Niamh Walsh",
    description: "Published “Sleep and stress”",
    at: "2026-10-09T09:20:00Z",
    status: { label: "Published", tone: "success" },
  },
  {
    id: "a2",
    name: "Ciarán Doyle",
    description: "Editing the About page",
    at: "2026-10-09T08:05:00Z",
    status: { label: "In progress", tone: "warning" },
  },
  {
    id: "a3",
    name: "Aoife Byrne",
    description: "New enquiry from the contact form",
    at: "2026-10-08T16:40:00Z",
    status: { label: "New", tone: "info" },
  },
  {
    id: "a4",
    name: "Seán Murphy",
    description: "Scheduled “Finding calm at work”",
    at: "2026-10-07T11:00:00Z",
    status: { label: "Pending", tone: "danger" },
  },
]

export const SAMPLE_CONTENT_HEALTH: GaugePart[] = [
  { key: "complete", label: "Complete", value: 41, tone: "solid" },
  { key: "work", label: "Needs work", value: 30, tone: "dark" },
  { key: "missing", label: "Missing SEO or alt", value: 29, tone: "hatched" },
]
```

```tsx
// admin/modules/design/showcase.tsx
"use client"

import { InboxIcon, PlusIcon } from "lucide-react"
import Link from "next/link"

import { ActivityFeed } from "@/admin/components/dashboard/activity-feed"
import { DashboardCard } from "@/admin/components/dashboard/dashboard-card"
import { EmptyState } from "@/admin/components/dashboard/empty-state"
import { Gauge } from "@/admin/components/dashboard/gauge"
import { KpiCard } from "@/admin/components/dashboard/kpi-card"
import {
  PageHeader,
  PRIMARY_ACTION,
  SECONDARY_ACTION,
} from "@/admin/components/dashboard/page-header"
import { PillBarChart } from "@/admin/components/dashboard/pill-bar-chart"
import {
  CardSkeleton,
  KpiRowSkeleton,
} from "@/admin/components/dashboard/skeletons"
import {
  StatusPill,
  toneForStatus,
} from "@/admin/components/dashboard/status-pill"
import { Button } from "@/components/ui/button"

import {
  SAMPLE_CONTENT_HEALTH,
  SAMPLE_KPIS,
  SAMPLE_NOW,
  SAMPLE_TEAM,
  SAMPLE_WEEK,
} from "./sample-data"

const STATUSES = ["Published", "Draft", "Scheduled", "New", "Contacted", "Pending"]

// Every Evergreen component on fixed data: for client demos, the
// side-by-side review against docs/design/admin-inspiration.png, and the
// visual tests.
export function Showcase() {
  return (
    <div className="flex flex-col gap-10">
      <PageHeader
        title="Design system"
        description="Every Evergreen component on sample data."
        actions={
          <>
            <Button className={PRIMARY_ACTION}>
              <PlusIcon data-icon="inline-start" />
              New article
            </Button>
            <Button variant="outline" className={SECONDARY_ACTION}>
              View site
            </Button>
          </>
        }
      />

      <section aria-labelledby="sample-dashboard" className="flex flex-col gap-4">
        <h2 id="sample-dashboard" className="text-card-title">
          Dashboard sample
        </h2>
        <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-4">
          {SAMPLE_KPIS.map((kpi) => (
            <KpiCard key={kpi.label} {...kpi} />
          ))}
        </div>
        <div className="grid gap-4 xl:grid-cols-[minmax(0,1fr)_340px]">
          <DashboardCard title="Enquiries this week">
            <PillBarChart
              title="Enquiries this week"
              data={SAMPLE_WEEK}
              currentLabel="This week"
              previousLabel="Last week"
            />
          </DashboardCard>
          <DashboardCard title="Next up">
            <div className="flex flex-1 flex-col justify-between gap-6">
              <div className="flex flex-col gap-3">
                <p className="text-2xl leading-tight font-semibold text-primary">
                  Call back Aoife Byrne
                </p>
                <p className="text-sm text-muted-foreground">
                  Asked about online sessions · today, 14:00
                </p>
              </div>
              <Button
                className={PRIMARY_ACTION}
                nativeButton={false}
                render={<Link href="/admin/leads" />}
              >
                <InboxIcon data-icon="inline-start" />
                Open enquiry
              </Button>
            </div>
          </DashboardCard>
        </div>
        <div className="grid gap-4 xl:grid-cols-[minmax(0,1fr)_380px]">
          <DashboardCard title="Team activity">
            <ActivityFeed
              entries={SAMPLE_TEAM}
              now={SAMPLE_NOW}
              emptyTitle="No activity yet"
              emptyDescription="Changes by your team will show here."
            />
          </DashboardCard>
          <DashboardCard title="Content health">
            <Gauge
              title="Content health"
              parts={SAMPLE_CONTENT_HEALTH}
              centerLabel="Complete"
            />
          </DashboardCard>
        </div>
      </section>

      <section aria-labelledby="sample-status" className="flex flex-col gap-4">
        <h2 id="sample-status" className="text-card-title">
          Status pills
        </h2>
        <div className="flex flex-wrap gap-2">
          {STATUSES.map((status) => (
            <StatusPill key={status} tone={toneForStatus(status)}>
              {status}
            </StatusPill>
          ))}
        </div>
      </section>

      <section aria-labelledby="sample-states" className="flex flex-col gap-4">
        <h2 id="sample-states" className="text-card-title">
          Empty and loading states
        </h2>
        <EmptyState
          icon={InboxIcon}
          title="No enquiries yet"
          description="New messages from the contact form will appear here."
          action={
            <Button variant="outline" className={SECONDARY_ACTION}>
              Open the contact page
            </Button>
          }
        />
        <KpiRowSkeleton />
        <CardSkeleton />
      </section>
    </div>
  )
}
```

```tsx
// app/(admin)/admin/(panel)/design/page.tsx
import type { Metadata } from "next"
import { Suspense } from "react"

import { Showcase } from "@/admin/modules/design/showcase"
import { requirePermission } from "@/server/auth/session"

export const metadata: Metadata = { title: "Design system" }

export default function DesignPage() {
  return (
    <Suspense fallback={null}>
      <Design />
    </Suspense>
  )
}

// Owner-only: code.update is the one permission only owners hold
// (docs/brief.md §7.2).
async function Design() {
  await requirePermission({ code: ["update"] })
  return <Showcase />
}
```

- [ ] **Step 6: Write and run the showcase E2E**

```ts
// tests/e2e/admin/shell/design.spec.ts
import { expect, test } from "@playwright/test"

import { STORAGE } from "../../support/storage"

test.describe("as an owner", () => {
  test.use({ storageState: STORAGE.owner })

  test("shows every Evergreen component on sample data", async ({ page }) => {
    await page.goto("/admin/design")
    await expect(
      page.getByRole("heading", { level: 1, name: "Design system" })
    ).toBeVisible()
    const sample = page.getByRole("region", { name: "Dashboard sample" })
    await expect(sample.getByRole("article")).toHaveCount(4)
    await expect(
      sample.getByRole("heading", { name: "Enquiries this week" })
    ).toBeVisible()
    await expect(sample.locator(".recharts-surface").first()).toBeVisible()
    await expect(
      sample.getByRole("heading", { name: "Content health" })
    ).toBeVisible()
  })
})

test.describe("as an editor", () => {
  test.use({ storageState: STORAGE.editor })

  test("is not allowed in", async ({ page }) => {
    await page.goto("/admin/design")
    await expect(
      page.getByRole("heading", { name: "You don't have access to Design system" })
    ).toBeVisible()
  })
})
```

Run: `pnpm test:e2e tests/e2e/admin/shell/design.spec.ts`
Expected: PASS on all three projects.

- [ ] **Step 7: Gates**

Run: `pnpm lint && pnpm typecheck && pnpm test:unit && pnpm build`
Expected: all green.

- [ ] **Step 8: Commit**

```bash
git add components/ui admin "app/(admin)/admin/(auth)/layout.tsx" "app/(admin)/admin/(panel)/design" tests/e2e/admin/shell/design.spec.ts package.json pnpm-lock.yaml
git commit -m "feat(admin): Evergreen dashboard primitives and an owner-only design showcase"
```

---

## Task 2.8: Dashboard v1 on the data that exists in Phase 2

**Files:**
- Modify: `package.json` (`date-fns@4.4.0`, `@date-fns/tz@1.5.0`)
- Create: `server/modules/dashboard/{week.ts,week.test.ts,repo.ts,service.ts,routes.ts}`
- Modify: `server/api/routes/admin.ts`
- Modify: `admin/lib/api.ts` (`dashboardApi`)
- Create: `admin/modules/dashboard/{present.ts,present.test.ts,queries.ts,dashboard-view.tsx,dashboard-skeleton.tsx}`
- Modify: `app/(admin)/admin/(panel)/page.tsx` (whole file)
- Delete: `admin/modules/auth/sign-out-button.tsx` (its only user was the placeholder page)
- Modify: `tests/e2e/admin/auth/sign-in.spec.ts`, `tests/e2e/admin/auth/two-factor.spec.ts`
- Create: `tests/integration/api/dashboard.test.ts`; Modify: `tests/integration/api/permission-matrix.test.ts`
- Create: `tests/e2e/admin/shell/dashboard.spec.ts`

**Interfaces:**
- Consumes: `hasPermission`, `parseRoles`, `roleRequiresTwoFactor`, `Actor`; `auditLogs`, `users`; dashboard primitives (2.7); `parseResponse`, `queryKeys.dashboard` (2.4); `usePermission` (2.3).
- Produces:
  - `server/modules/dashboard/week.ts`: `DUBLIN`, `weekWindow(now: Date): { start: Date; end: Date; previousStart: Date; days: WeekDay[]; previousDays: WeekDay[] }`, `type WeekDay = { key: string /* yyyy-MM-dd */; label: string /* Mon */ }`.
  - `GET /api/v1/admin/dashboard` (`dashboard.view`) → `DashboardDto`:
    ```ts
    type DashboardDto = {
      generatedAt: string
      activeUsers: { value: number; previous: number }
      signIns: { days: { key: string; label: string; current: number; previous: number }[]; current: number; previous: number }
      security: { protected: number; requiredMissing: number; optionalMissing: number; total: number } | null  // user.list only
      changes: { value: number; previous: number } | null                                                    // audit.read only
      recentActivity: { id: string; actorName: string | null; actorEmail: string | null; action: string; summary: string; createdAt: string }[] | null // audit.read only
      nextUp: { id: "two-factor" | "invite"; title: string; description: string; cta: string; href: string; done: boolean }[]
    }
    ```
    `dashboardRoutes`, `type DashboardRoutes`.
  - `admin/lib/api.ts`: `dashboardApi = hc<DashboardRoutes>("/api/v1/admin/dashboard")`.
  - `admin/modules/dashboard/present.ts`: `greetingFor(date: Date): "Good morning" | "Good afternoon" | "Good evening"` (Europe/Dublin), `firstName(name)`, `actionStatus(action): { label; tone } | undefined`.
  - `DashboardView({ greeting, siteUrl })` (client; fetches with `useQuery`, so the visual suite can pin data with `page.route`), `DashboardSkeleton()`; the greeting `h1` carries `data-testid="greeting"`.

- [ ] **Step 1: Install the date libraries**

Run: `pnpm add date-fns@4.4.0 @date-fns/tz@1.5.0`
Expected: two dependencies added.

- [ ] **Step 2: Write the failing week and presentation tests**

```ts
// server/modules/dashboard/week.test.ts
import { describe, expect, it } from "vitest"

import { weekWindow } from "./week"

const HOUR = 3_600_000

describe("weekWindow (Europe/Dublin, Monday first)", () => {
  it("starts at Monday 00:00 Irish time", () => {
    // Thursday 00:30 IST.
    const week = weekWindow(new Date("2026-10-07T23:30:00Z"))
    expect(week.start.toISOString()).toBe("2026-10-04T23:00:00.000Z")
    expect(week.end.toISOString()).toBe("2026-10-11T23:00:00.000Z")
    expect(week.days.map((day) => day.key)).toEqual([
      "2026-10-05",
      "2026-10-06",
      "2026-10-07",
      "2026-10-08",
      "2026-10-09",
      "2026-10-10",
      "2026-10-11",
    ])
    expect(week.days[0].label).toBe("Mon")
  })

  it("puts Sunday 23:30 UTC (Monday in Dublin) in the new week", () => {
    const week = weekWindow(new Date("2026-10-11T23:30:00Z"))
    expect(week.start.toISOString()).toBe("2026-10-11T23:00:00.000Z")
  })

  it("spans the October clock change as a 169-hour week", () => {
    const week = weekWindow(new Date("2026-10-25T12:00:00Z"))
    expect(week.start.toISOString()).toBe("2026-10-18T23:00:00.000Z")
    expect(week.end.toISOString()).toBe("2026-10-26T00:00:00.000Z")
    expect((week.end.getTime() - week.start.getTime()) / HOUR).toBe(169)
    expect(week.days.at(-1)?.key).toBe("2026-10-25")
  })

  it("gives the previous week's days, ending where this week starts", () => {
    const week = weekWindow(new Date("2026-10-25T12:00:00Z"))
    expect(week.previousStart.toISOString()).toBe("2026-10-11T23:00:00.000Z")
    expect(week.previousDays[0].key).toBe("2026-10-12")
    expect(week.previousDays.at(-1)?.key).toBe("2026-10-18")
  })
})
```

```ts
// admin/modules/dashboard/present.test.ts
import { describe, expect, it } from "vitest"

import { actionStatus, firstName, greetingFor } from "./present"

describe("greetingFor (Irish time)", () => {
  it.each([
    ["2026-10-09T05:59:00Z", "Good morning"], // 06:59 IST
    ["2026-10-09T10:59:00Z", "Good morning"], // 11:59 IST
    ["2026-10-09T11:00:00Z", "Good afternoon"], // 12:00 IST
    ["2026-10-09T16:59:00Z", "Good afternoon"], // 17:59 IST
    ["2026-10-09T17:00:00Z", "Good evening"], // 18:00 IST
    ["2026-12-09T23:30:00Z", "Good evening"], // 23:30 GMT
  ])("%s → %s", (iso, expected) => {
    expect(greetingFor(new Date(iso))).toBe(expected)
  })
})

describe("firstName", () => {
  it.each([
    ["Sam Shell", "Sam"],
    ["  Magda  ", "Magda"],
    ["", "there"],
  ])("%j → %j", (name, expected) => {
    expect(firstName(name)).toBe(expected)
  })
})

describe("actionStatus", () => {
  it("labels known audit actions", () => {
    expect(actionStatus("auth.sign_in")).toEqual({
      label: "Signed in",
      tone: "success",
    })
    expect(actionStatus("user.invite")).toEqual({
      label: "Invited",
      tone: "info",
    })
    expect(actionStatus("something.else")).toBeUndefined()
  })
})
```

Run: `pnpm vitest run --project unit server/modules/dashboard admin/modules/dashboard`
Expected: FAIL with unresolved `./week` and `./present`.

- [ ] **Step 3: Implement the week maths and presentation helpers**

```ts
// server/modules/dashboard/week.ts
import "server-only"

import { tz } from "@date-fns/tz"
import { addDays, format, startOfWeek, subDays } from "date-fns"

// Dashboard weeks are Irish weeks: Monday 00:00 Europe/Dublin, so a sign-in
// at 00:30 on a Monday in summer (23:30 UTC Sunday) counts for Monday.
export const DUBLIN = tz("Europe/Dublin")

export type WeekDay = { key: string; label: string }

const plain = (date: Date) => new Date(date.getTime())

function daysFrom(start: Date): WeekDay[] {
  return Array.from({ length: 7 }, (_, index) => {
    const day = addDays(start, index, { in: DUBLIN })
    return {
      key: format(day, "yyyy-MM-dd", { in: DUBLIN }),
      label: format(day, "EEE", { in: DUBLIN }),
    }
  })
}

export function weekWindow(now: Date) {
  const start = startOfWeek(now, { weekStartsOn: 1, in: DUBLIN })
  const previousStart = subDays(start, 7, { in: DUBLIN })
  return {
    start: plain(start),
    end: plain(addDays(start, 7, { in: DUBLIN })),
    previousStart: plain(previousStart),
    days: daysFrom(start),
    previousDays: daysFrom(previousStart),
  }
}
```

```ts
// admin/modules/dashboard/present.ts
import {
  toneForStatus,
  type StatusTone,
} from "@/admin/components/dashboard/status-pill"

const HOUR = new Intl.DateTimeFormat("en-IE", {
  hour: "numeric",
  hourCycle: "h23",
  timeZone: "Europe/Dublin",
})

// "Good morning, {name}" (docs/brief.md §9.3), by the clock in Ireland.
export function greetingFor(date: Date) {
  const hour = Number(HOUR.format(date))
  if (hour < 12) return "Good morning"
  if (hour < 18) return "Good afternoon"
  return "Good evening"
}

export const firstName = (name: string) =>
  name.trim().split(/\s+/)[0] || "there"

const ACTION_LABELS: Record<string, string> = {
  "auth.sign_in": "Signed in",
  "auth.impersonate": "Viewed as",
  "user.invite": "Invited",
  "user.bootstrap-owner": "Created",
}

export function actionStatus(
  action: string
): { label: string; tone: StatusTone } | undefined {
  const label = ACTION_LABELS[action]
  return label ? { label, tone: toneForStatus(label) } : undefined
}
```

`toneForStatus` maps "Signed in" → success and "Invited" → info (Task 2.7 table); "Viewed as" and "Created" fall back to neutral.

Run: `pnpm vitest run --project unit server/modules/dashboard admin/modules/dashboard`
Expected: PASS.

- [ ] **Step 4: Write the failing dashboard integration test**

```ts
// tests/integration/api/dashboard.test.ts
import { afterAll, beforeEach, describe, expect, it } from "vitest"

import { closeDb, getDb } from "@/server/db/client"
import { auditLogs } from "@/server/db/schema"
import { weekWindow } from "@/server/modules/dashboard/week"

import {
  adminRequest,
  createUser,
  signIn,
  signInWithTwoFactor,
} from "../helpers/auth"
import { resetDb } from "../helpers/db"

beforeEach(resetDb)
afterAll(closeDb)

const HOUR = 3_600_000
const DAY = 24 * HOUR
const at = (base: Date, ms: number) => new Date(base.getTime() + ms)

async function signedIn(actorId: string, createdAt: Date) {
  await getDb().insert(auditLogs).values({
    actorId,
    action: "auth.sign_in",
    entityType: "user",
    entityId: actorId,
    summary: "Signed in",
    createdAt,
  })
}

async function dashboard(cookie: string) {
  const response = await adminRequest("/dashboard", cookie)
  expect(response.status).toBe(200)
  return response.json()
}

describe("GET /api/v1/admin/dashboard", () => {
  it("counts sign-ins per Irish day, this week against last", async () => {
    const owner = await createUser("owner")
    const cookie = await signInWithTwoFactor("owner@example.com")
    // Drop the sign-in the helper just recorded; only fixtures count.
    await getDb().delete(auditLogs)
    const week = weekWindow(new Date())
    await signedIn(owner.id, at(week.start, 2 * HOUR))
    await signedIn(owner.id, at(week.start, 3 * HOUR))
    await signedIn(owner.id, at(week.previousStart, 2 * HOUR))

    const body = await dashboard(cookie)

    expect(body.signIns).toMatchObject({ current: 2, previous: 1 })
    expect(body.signIns.days).toHaveLength(7)
    expect(body.signIns.days[0]).toEqual({
      key: week.days[0].key,
      label: "Mon",
      current: 2,
      previous: 1,
    })
  })

  it("counts distinct people signing in over 30 days against the 30 before", async () => {
    const owner = await createUser("owner")
    const viewer = await createUser("viewer")
    const cookie = await signInWithTwoFactor("owner@example.com")
    await getDb().delete(auditLogs)
    const now = new Date()
    await signedIn(owner.id, at(now, -1 * DAY))
    await signedIn(owner.id, at(now, -2 * DAY))
    await signedIn(viewer.id, at(now, -3 * DAY))
    await signedIn(viewer.id, at(now, -40 * DAY))

    expect((await dashboard(cookie)).activeUsers).toEqual({
      value: 2,
      previous: 1,
    })
  })

  it("gives a viewer aggregates only (docs/brief.md §7.2)", async () => {
    await createUser("viewer")
    const body = await dashboard(await signIn("viewer@example.com"))
    expect(body.security).toBeNull()
    expect(body.changes).toBeNull()
    expect(body.recentActivity).toBeNull()
    expect(body.nextUp.map((item: { id: string }) => item.id)).toEqual([
      "two-factor",
    ])
  })

  it("gives an owner security health, changes and recent activity", async () => {
    await createUser("owner")
    await createUser("admin")
    await createUser("viewer")
    const cookie = await signInWithTwoFactor("owner@example.com")

    const body = await dashboard(cookie)

    expect(body.security).toEqual({
      protected: 1,
      requiredMissing: 1,
      optionalMissing: 1,
      total: 3,
    })
    expect(body.changes.value).toBeGreaterThanOrEqual(1)
    expect(body.recentActivity[0]).toMatchObject({
      action: "auth.sign_in",
      actorName: "owner",
    })
    expect(typeof body.recentActivity[0].createdAt).toBe("string")
    expect(body.nextUp).toEqual([
      expect.objectContaining({ id: "two-factor", done: true }),
      expect.objectContaining({ id: "invite", done: true }),
    ])
  })
})
```

Add to `tests/integration/api/permission-matrix.test.ts` inside `describe.each(ROLE_NAMES)`:

```ts
  it("GET /dashboard → 200 (dashboard.view)", async () => {
    const response = await adminRequest("/dashboard", await cookieFor(role))
    expect(response.status).toBe(200)
  })
```

Run: `pnpm test:integration tests/integration/api/dashboard.test.ts`
Expected: FAIL — `expected 404 to be 200`.

- [ ] **Step 5: Implement repo, service and route**

```ts
// server/modules/dashboard/repo.ts
import "server-only"

import { and, count, countDistinct, desc, eq, gte, lt, sql } from "drizzle-orm"

import { getDb } from "@/server/db/client"
import { auditLogs, users } from "@/server/db/schema"

const SIGN_IN = "auth.sign_in"

const between = (from: Date, to: Date) =>
  and(gte(auditLogs.createdAt, from), lt(auditLogs.createdAt, to))

export function signInsByDay(from: Date, to: Date) {
  const day = sql<string>`to_char(${auditLogs.createdAt} at time zone 'Europe/Dublin', 'YYYY-MM-DD')`
  return getDb()
    .select({ day, count: count() })
    .from(auditLogs)
    .where(and(eq(auditLogs.action, SIGN_IN), between(from, to)))
    .groupBy(day)
}

export async function distinctSignedIn(from: Date, to: Date) {
  const [row] = await getDb()
    .select({ value: countDistinct(auditLogs.actorId) })
    .from(auditLogs)
    .where(and(eq(auditLogs.action, SIGN_IN), between(from, to)))
  return row?.value ?? 0
}

export async function auditCount(from: Date, to: Date) {
  const [row] = await getDb()
    .select({ value: count() })
    .from(auditLogs)
    .where(between(from, to))
  return row?.value ?? 0
}

// Accounts that can sign in (banned ones excluded).
export function activeAccounts() {
  return getDb()
    .select({ role: users.role, twoFactorEnabled: users.twoFactorEnabled })
    .from(users)
    .where(sql`coalesce(${users.banned}, false) = false`)
}

export function recentAudit(limit: number) {
  return getDb()
    .select({
      id: auditLogs.id,
      actorName: users.name,
      actorEmail: auditLogs.actorEmail,
      action: auditLogs.action,
      summary: auditLogs.summary,
      createdAt: auditLogs.createdAt,
    })
    .from(auditLogs)
    .leftJoin(users, eq(users.id, auditLogs.actorId))
    .orderBy(desc(auditLogs.createdAt))
    .limit(limit)
}
```

```ts
// server/modules/dashboard/service.ts
import "server-only"

import { hasPermission, parseRoles } from "@/lib/auth/permissions"
import { roleRequiresTwoFactor } from "@/lib/auth/two-factor-policy"
import type { Actor } from "@/server/auth/actor"

import * as repo from "./repo"
import { weekWindow } from "./week"

export type DashboardDto = {
  generatedAt: string
  activeUsers: { value: number; previous: number }
  signIns: {
    days: { key: string; label: string; current: number; previous: number }[]
    current: number
    previous: number
  }
  security: {
    protected: number
    requiredMissing: number
    optionalMissing: number
    total: number
  } | null
  changes: { value: number; previous: number } | null
  recentActivity:
    | {
        id: string
        actorName: string | null
        actorEmail: string | null
        action: string
        summary: string
        createdAt: string
      }[]
    | null
  nextUp: {
    id: "two-factor" | "invite"
    title: string
    description: string
    cta: string
    href: string
    done: boolean
  }[]
}

const DAY = 86_400_000

function security(accounts: { role: string | null; twoFactorEnabled: boolean | null }[]) {
  let protectedCount = 0
  let requiredMissing = 0
  let optionalMissing = 0
  for (const account of accounts) {
    if (account.twoFactorEnabled) protectedCount += 1
    else if (roleRequiresTwoFactor(parseRoles(account.role))) requiredMissing += 1
    else optionalMissing += 1
  }
  return {
    protected: protectedCount,
    requiredMissing,
    optionalMissing,
    total: accounts.length,
  }
}

// Dashboard v1 (docs/plan.md Phase 2, item 7): only data that exists before
// the content phases. Every widget respects RBAC: people and audit details
// need user.list / audit.read; everyone else gets aggregates.
export async function getDashboard(
  actor: Actor,
  now: Date = new Date()
): Promise<DashboardDto> {
  const week = weekWindow(now)
  const canAudit = hasPermission(actor.roles, { audit: ["read"] })
  const canListUsers = hasPermission(actor.roles, { user: ["list"] })
  const canInvite = hasPermission(actor.roles, { user: ["create"] })
  const monthAgo = new Date(now.getTime() - 30 * DAY)
  const twoMonthsAgo = new Date(now.getTime() - 60 * DAY)

  const [byDay, active, activeBefore, accounts, changes, changesBefore, recent] =
    await Promise.all([
      repo.signInsByDay(week.previousStart, week.end),
      repo.distinctSignedIn(monthAgo, now),
      repo.distinctSignedIn(twoMonthsAgo, monthAgo),
      canListUsers || canInvite ? repo.activeAccounts() : null,
      canAudit ? repo.auditCount(week.start, week.end) : null,
      canAudit ? repo.auditCount(week.previousStart, week.start) : null,
      canAudit ? repo.recentAudit(8) : null,
    ])

  const counts = new Map(byDay.map((row) => [row.day, row.count]))
  const days = week.days.map((day, index) => ({
    key: day.key,
    label: day.label,
    current: counts.get(day.key) ?? 0,
    previous: counts.get(week.previousDays[index]!.key) ?? 0,
  }))
  const sum = (key: "current" | "previous") =>
    days.reduce((total, day) => total + day[key], 0)

  const nextUp: DashboardDto["nextUp"] = [
    {
      id: "two-factor",
      title: actor.twoFactorEnabled ? "Two-factor is on" : "Turn on two-factor",
      description:
        "A code from your phone keeps your account safe even if your password leaks.",
      cta: "Set up two-factor",
      href: "/admin/two-factor-setup",
      done: actor.twoFactorEnabled,
    },
  ]
  if (canInvite) {
    nextUp.push({
      id: "invite",
      title: "Invite your team",
      description: "Give each person their own account and only the role they need.",
      cta: "Invite teammates",
      href: "/admin/users",
      done: (accounts?.length ?? 0) > 1,
    })
  }

  return {
    generatedAt: now.toISOString(),
    activeUsers: { value: active, previous: activeBefore },
    signIns: { days, current: sum("current"), previous: sum("previous") },
    security: canListUsers && accounts ? security(accounts) : null,
    changes:
      changes === null || changesBefore === null
        ? null
        : { value: changes, previous: changesBefore },
    recentActivity:
      recent?.map((row) => ({
        ...row,
        createdAt: row.createdAt.toISOString(),
      })) ?? null,
    nextUp,
  }
}
```

```ts
// server/modules/dashboard/routes.ts
import "server-only"

import { Hono } from "hono"

import { can } from "@/server/api/middleware/auth"
import type { AppEnv } from "@/server/api/types"

import { getDashboard } from "./service"

export const dashboardRoutes = new Hono<AppEnv>().get(
  "/",
  can({ dashboard: ["view"] }),
  async (c) => c.json(await getDashboard(c.get("actor")!))
)

export type DashboardRoutes = typeof dashboardRoutes
```

Mount it in `server/api/routes/admin.ts` (import `dashboardRoutes` from `@/server/modules/dashboard/routes`):

```ts
  .use(twoFactorComplete)
  .route("/search", searchRoutes)
  .route("/dashboard", dashboardRoutes)
  .route("/audit", auditRoutes)
  .route("/users", usersRoutes)
```

Run: `pnpm test:integration tests/integration/api/dashboard.test.ts tests/integration/api/permission-matrix.test.ts`
Expected: PASS.

- [ ] **Step 6: Build the client view**

In `admin/lib/api.ts` add:

```ts
import type { DashboardRoutes } from "@/server/modules/dashboard/routes"
```

```ts
export const dashboardApi = hc<DashboardRoutes>("/api/v1/admin/dashboard")
```

```ts
// admin/modules/dashboard/queries.ts
import { queryOptions } from "@tanstack/react-query"
import type { InferResponseType } from "hono/client"

import { dashboardApi, parseResponse } from "@/admin/lib/api"
import { queryKeys } from "@/admin/lib/query-keys"

export type Dashboard = InferResponseType<typeof dashboardApi.index.$get, 200>

export const dashboardQuery = () =>
  queryOptions({
    queryKey: queryKeys.dashboard,
    queryFn: () => parseResponse(dashboardApi.index.$get()),
  })
```

```tsx
// admin/modules/dashboard/dashboard-skeleton.tsx
import {
  CardSkeleton,
  KpiRowSkeleton,
} from "@/admin/components/dashboard/skeletons"

export function DashboardSkeleton() {
  return (
    <div className="flex flex-col gap-4">
      <KpiRowSkeleton />
      <div className="grid gap-4 xl:grid-cols-[minmax(0,1fr)_340px]">
        <CardSkeleton />
        <CardSkeleton />
      </div>
    </div>
  )
}
```

```tsx
// admin/modules/dashboard/dashboard-view.tsx
"use client"

import { useQuery } from "@tanstack/react-query"
import { CircleCheckIcon, CircleIcon, UserPlusIcon } from "lucide-react"
import Link from "next/link"

import { ActivityFeed } from "@/admin/components/dashboard/activity-feed"
import { DashboardCard } from "@/admin/components/dashboard/dashboard-card"
import { EmptyState } from "@/admin/components/dashboard/empty-state"
import {
  formatDelta,
  formatNumber,
  formatPercent,
} from "@/admin/components/dashboard/format"
import { Gauge } from "@/admin/components/dashboard/gauge"
import { KpiCard } from "@/admin/components/dashboard/kpi-card"
import {
  PageHeader,
  PRIMARY_ACTION,
  SECONDARY_ACTION,
} from "@/admin/components/dashboard/page-header"
import { PillBarChart } from "@/admin/components/dashboard/pill-bar-chart"
import { usePermission } from "@/admin/lib/actor-context"
import { Button } from "@/components/ui/button"
import { cn } from "@/lib/utils"

import { DashboardSkeleton } from "./dashboard-skeleton"
import { actionStatus } from "./present"
import { dashboardQuery, type Dashboard } from "./queries"

// Fetched in the browser (not prefetched) so the shell paints at once and
// the visual suite can pin the numbers with page.route.
export function DashboardView({
  greeting,
  siteUrl,
}: {
  greeting: string
  siteUrl: string
}) {
  const { data, isPending, isError, refetch } = useQuery(dashboardQuery())
  const canInvite = usePermission({ user: ["create"] })

  return (
    <div className="flex flex-col gap-6">
      <PageHeader
        title={greeting}
        titleTestId="greeting"
        description="Here's what's happening across your admin."
        actions={
          <>
            {canInvite ? (
              <Button
                className={PRIMARY_ACTION}
                nativeButton={false}
                render={<Link href="/admin/users" />}
              >
                <UserPlusIcon data-icon="inline-start" />
                Invite teammate
              </Button>
            ) : null}
            <Button
              variant="outline"
              className={SECONDARY_ACTION}
              nativeButton={false}
              render={<a href={siteUrl} target="_blank" rel="noreferrer" />}
            >
              View site
              <span className="sr-only"> (opens in a new tab)</span>
            </Button>
          </>
        }
      />
      {isPending ? (
        <DashboardSkeleton />
      ) : isError ? (
        <EmptyState
          title="The dashboard didn't load"
          description="Check your connection, then try again."
          action={
            <Button className={PRIMARY_ACTION} onClick={() => refetch()}>
              Try again
            </Button>
          }
        />
      ) : (
        <DashboardBody data={data} />
      )}
    </div>
  )
}

function DashboardBody({ data }: { data: Dashboard }) {
  // Relative times are measured from the server's clock, so they read the
  // same however long the tab has been open.
  const now = Date.parse(data.generatedAt)
  const kpis = [
    {
      label: "Active users",
      value: formatNumber(data.activeUsers.value),
      delta: formatDelta(data.activeUsers.value, data.activeUsers.previous),
      caption: "signed in, last 30 days",
      variant: "hero" as const,
    },
    {
      label: "Sign-ins this week",
      value: formatNumber(data.signIns.current),
      delta: formatDelta(data.signIns.current, data.signIns.previous),
      caption: "vs last week",
    },
    data.security
      ? {
          label: "Security health",
          value: formatPercent(
            data.security.total ? data.security.protected / data.security.total : 0
          ),
          delta: null,
          caption: `${data.security.requiredMissing + data.security.optionalMissing} without two-factor`,
          href: "/admin/users",
        }
      : null,
    data.changes
      ? {
          label: "Changes this week",
          value: formatNumber(data.changes.value),
          delta: formatDelta(data.changes.value, data.changes.previous),
          caption: "vs last week",
          href: "/admin/activity",
        }
      : null,
  ].filter((kpi) => kpi !== null)

  const next = data.nextUp.find((item) => !item.done)

  return (
    <>
      <div
        className={cn(
          "grid gap-4 sm:grid-cols-2",
          kpis.length > 2 && "xl:grid-cols-4"
        )}
      >
        {kpis.map((kpi) => (
          <KpiCard key={kpi.label} {...kpi} />
        ))}
      </div>
      <div className="grid gap-4 xl:grid-cols-[minmax(0,1fr)_340px]">
        <DashboardCard title="Sign-ins this week">
          <PillBarChart
            title="Sign-ins this week"
            data={data.signIns.days}
            currentLabel="This week"
            previousLabel="Last week"
          />
        </DashboardCard>
        <DashboardCard title="Next up">
          <div className="flex flex-1 flex-col justify-between gap-6">
            <div className="flex flex-col gap-3">
              <p className="text-2xl leading-tight font-semibold text-primary">
                {next ? next.title : "You're all set"}
              </p>
              {next ? (
                <p className="text-sm text-muted-foreground">
                  {next.description}
                </p>
              ) : null}
            </div>
            <ul aria-label="Setup checklist" className="flex flex-col gap-2">
              {data.nextUp.map((item) => (
                <li key={item.id} className="flex items-center gap-2 text-sm">
                  {item.done ? (
                    <CircleCheckIcon aria-hidden className="size-4 text-success" />
                  ) : (
                    <CircleIcon aria-hidden className="size-4 text-muted-foreground" />
                  )}
                  <span className={cn(item.done && "text-muted-foreground")}>
                    {item.title}
                  </span>
                  <span className="sr-only">{item.done ? "(done)" : "(to do)"}</span>
                </li>
              ))}
            </ul>
            {next ? (
              <Button
                className={PRIMARY_ACTION}
                nativeButton={false}
                render={<Link href={next.href} />}
              >
                {next.cta}
              </Button>
            ) : null}
          </div>
        </DashboardCard>
      </div>
      {data.recentActivity || data.security ? (
        <div className="grid gap-4 xl:grid-cols-[minmax(0,1fr)_380px]">
          {data.recentActivity ? (
            <DashboardCard title="Team activity">
              <ActivityFeed
                now={now}
                emptyTitle="No activity yet"
                emptyDescription="Sign-ins, invites and changes by your team will show here."
                entries={data.recentActivity.map((row) => ({
                  id: row.id,
                  name: row.actorName ?? row.actorEmail ?? "System",
                  description: row.summary,
                  at: row.createdAt,
                  status: actionStatus(row.action),
                }))}
              />
            </DashboardCard>
          ) : null}
          {data.security ? (
            <DashboardCard title="Security health">
              <Gauge
                title="Security health"
                centerLabel="Protected"
                parts={[
                  {
                    key: "protected",
                    label: "Two-factor on",
                    value: data.security.protected,
                    tone: "solid",
                  },
                  {
                    key: "required",
                    label: "Required, not set up",
                    value: data.security.requiredMissing,
                    tone: "dark",
                  },
                  {
                    key: "optional",
                    label: "Optional, not set up",
                    value: data.security.optionalMissing,
                    tone: "hatched",
                  },
                ]}
              />
            </DashboardCard>
          ) : null}
        </div>
      ) : null}
    </>
  )
}
```

Replace `app/(admin)/admin/(panel)/page.tsx`:

```tsx
import type { Metadata } from "next"
import { io } from "next/cache"
import { Suspense } from "react"

import { DashboardSkeleton } from "@/admin/modules/dashboard/dashboard-skeleton"
import { DashboardView } from "@/admin/modules/dashboard/dashboard-view"
import { firstName, greetingFor } from "@/admin/modules/dashboard/present"
import { requirePermission } from "@/server/auth/session"
import { getEnv } from "@/server/env"

export const metadata: Metadata = { title: "Dashboard" }

export default function DashboardPage() {
  return (
    <Suspense fallback={<DashboardSkeleton />}>
      <Dashboard />
    </Suspense>
  )
}

async function Dashboard() {
  const actor = await requirePermission({ dashboard: ["view"] })
  // The greeting depends on the time of day: request time, never the
  // static shell.
  await io()
  return (
    <DashboardView
      greeting={`${greetingFor(new Date())}, ${firstName(actor.name)}`}
      siteUrl={getEnv().SITE_URL}
    />
  )
}
```

Delete the placeholder's sign-out button:

Run: `git rm admin/modules/auth/sign-out-button.tsx && grep -rn "sign-out-button" admin app || echo "no references"`
Expected: `no references`.

- [ ] **Step 7: Update the auth specs for the new dashboard**

In `tests/e2e/admin/auth/sign-in.spec.ts`:

- change the support import to `import { E2E_USERS, signIn, signOut } from "../../support/admin"`;
- replace `const welcome = (name: string) => ({ name: \`Welcome, ${name}\` })` with:

```ts
// The dashboard greets by first name, by the time of day in Ireland.
const welcome = (name: string) => ({
  level: 1 as const,
  name: new RegExp(
    `^Good (morning|afternoon|evening), ${name.split(" ")[0]}$`
  ),
})
```

- replace both `await page.getByRole("button", { name: "Sign out" }).click()` with `await signOut(page)`.

In `tests/e2e/admin/auth/two-factor.spec.ts`:

- import `signOut as signOutFromMenu` from `"../../support/admin"` alongside the existing names;
- change the body of the local `signOut(page)` to `await signOutFromMenu(page)` followed by the existing `toHaveURL` assertion;
- replace both `page.getByRole("heading", { name: \`Welcome, ${user.name}\` })` with `page.getByRole("heading", { level: 1, name: /^Good (morning|afternoon|evening), Tess$/ })`;
- replace `page.getByRole("heading", { name: /Welcome/ })` with `page.getByRole("heading", { name: /^Good (morning|afternoon|evening)/ })`.

The final "The way out works" step stays as it is: it clicks the setup page's own "Sign out" button.

- [ ] **Step 8: Write the dashboard E2E ("viewer has no edit actions")**

```ts
// tests/e2e/admin/shell/dashboard.spec.ts
import { expect, test } from "@playwright/test"

import { STORAGE } from "../../support/storage"

const GREETING = /^Good (morning|afternoon|evening), /

test.describe("as an owner", () => {
  test.use({ storageState: STORAGE.owner })

  test("shows every widget the role may see", async ({ page }) => {
    await page.goto("/admin")
    await expect(
      page.getByRole("heading", { level: 1, name: GREETING })
    ).toHaveText(/, Sam$/)
    await expect(page.getByRole("article")).toHaveCount(4)
    await expect(page.getByRole("heading", { name: "Security health" }).first()).toBeVisible()
    const activity = page.getByRole("region", { name: "Team activity" })
    await expect(activity.getByText("Signed in").first()).toBeVisible()
    await expect(page.getByRole("link", { name: "Invite teammate" })).toBeVisible()
  })
})

test.describe("as a viewer", () => {
  test.use({ storageState: STORAGE.viewer })

  test("sees aggregates and no edit actions", async ({ page }) => {
    await page.goto("/admin")
    await expect(
      page.getByRole("heading", { level: 1, name: GREETING })
    ).toBeVisible()
    await expect(page.getByRole("article")).toHaveCount(2)
    await expect(page.getByRole("link", { name: "Invite teammate" })).toHaveCount(0)
    await expect(page.getByRole("region", { name: "Team activity" })).toHaveCount(0)
    await expect(page.getByRole("region", { name: "Security health" })).toHaveCount(0)
    await expect(page.getByRole("list", { name: "Setup checklist" })).toBeVisible()
  })
})
```

(`.first()` on "Security health" because the KPI card and the gauge card both carry that heading.)

Run: `pnpm test:e2e tests/e2e/admin`
Expected: PASS on all projects, including the updated `auth/**` specs on desktop.

- [ ] **Step 9: Gates**

Run: `pnpm lint && pnpm typecheck && pnpm test:unit && pnpm test:integration && pnpm build`
Expected: all green.

- [ ] **Step 10: Commit**

```bash
git add server/modules/dashboard server/api/routes/admin.ts admin "app/(admin)/admin/(panel)/page.tsx" tests/integration/api tests/e2e/admin package.json pnpm-lock.yaml
git commit -m "feat(admin): dashboard v1 with sign-ins, active users, security health and activity"
```

---

## Task 2.9: DataTable foundation and the `/admin/activity` page (audit filters)

**Files:**
- Modify: `package.json` (`@tanstack/react-table@9.2.8`)
- Create (shadcn CLI): `components/ui/{table,select,button-group}.tsx`
- Create: `server/modules/audit/{schema.ts,repo.ts}`; Modify: `server/modules/audit/{service.ts,routes.ts}`
- Modify: `tests/integration/api/audit.test.ts`
- Create: `admin/components/data-table/{data-table.tsx,data-table-pagination.tsx,data-table-pagination.test.tsx}`
- Create: `admin/modules/activity/{params.ts,params.test.ts,queries.ts,activity-view.tsx}`
- Create: `app/(admin)/admin/(panel)/activity/page.tsx`
- Create: `tests/e2e/admin/shell/activity.spec.ts`

**Interfaces:**
- Consumes: `auditApi`, `parseResponse` (2.4), `inProcessAuditApi` (2.4), `getQueryClient` (2.4), `queryKeys.audit`, `AuditListParams` (2.4), `PageHeader`, `EmptyState`, `CardSkeleton`, `formatNumber` (2.7), `requirePermission` (2.3).
- Produces:
  - `GET /api/v1/admin/audit?page&pageSize&actor&action&from&to` → `{ items: AuditRow[]; page; pageSize; total }` where `AuditRow = { id; actorId: string | null; actorEmail: string | null; actorName: string | null; action; entityType; entityId: string | null; summary; createdAt: string }`. `from`/`to` are `YYYY-MM-DD` Irish days, inclusive; `from > to` → 400 with `fieldErrors.to`.
  - `GET /api/v1/admin/audit/facets` → `{ actions: string[]; actors: { id: string; label: string }[] }`.
  - `server/modules/audit/repo.ts`: `dublinMidnight(day: string, offsetDays?: number): Date`, `listAudit(filters, page, pageSize)`, `auditFacets()`.
  - `DataTable({ table, caption, empty, renderCard, isFetching? })` (generic over TanStack Table v9 features/data; cards below `sm`, table above).
  - `DataTablePagination({ page, pageSize, total, onPageChange })`.
  - `admin/modules/activity/params.ts`: `activityParsers` (nuqs, from `nuqs/server`), `loadActivityParams`, `toAuditParams(state): AuditListParams`.
  - `admin/modules/activity/queries.ts`: `auditListQuery(params, client?)`, `auditFacetsQuery(client?)` (the `client` argument lets the RSC pass `inProcessAuditApi()`).

- [ ] **Step 1: Install and add components**

Run: `pnpm add @tanstack/react-table@9.2.8 && pnpm exec shadcn add table select button-group --yes`
Expected: dependency and three components added; restore any modified existing file.

- [ ] **Step 2: Write the failing audit filter tests**

Append inside `describe("GET /api/v1/admin/audit", …)` in `tests/integration/api/audit.test.ts`, and add the imports `import { getDb } from "@/server/db/client"`, `import { auditLogs } from "@/server/db/schema"` (merge with the existing `closeDb` import):

```ts
  async function entry(values: {
    actorId: string | null
    action: string
    createdAt: Date
  }) {
    await getDb().insert(auditLogs).values({
      ...values,
      entityType: "user",
      summary: values.action,
    })
  }

  async function ownerWithEntries() {
    const owner = await createUser("owner")
    const editor = await createUser("editor")
    const cookie = await signInWithTwoFactor("owner@example.com")
    await getDb().delete(auditLogs)
    // 23:30 UTC on 4 Oct is 00:30 on 5 Oct in Ireland (IST).
    await entry({ actorId: owner.id, action: "user.invite", createdAt: new Date("2026-10-04T23:30:00Z") })
    await entry({ actorId: editor.id, action: "auth.sign_in", createdAt: new Date("2026-10-05T12:00:00Z") })
    await entry({ actorId: owner.id, action: "auth.sign_in", createdAt: new Date("2026-10-07T09:00:00Z") })
    return { cookie, owner, editor }
  }

  const actions = (body: { items: { action: string }[] }) =>
    body.items.map((item) => item.action)

  it("filters by action", async () => {
    const { cookie } = await ownerWithEntries()
    const body = await (await adminRequest("/audit?action=auth.sign_in", cookie)).json()
    expect(body.total).toBe(2)
    expect(actions(body)).toEqual(["auth.sign_in", "auth.sign_in"])
  })

  it("filters by actor and names them", async () => {
    const { cookie, editor } = await ownerWithEntries()
    const body = await (await adminRequest(`/audit?actor=${editor.id}`, cookie)).json()
    expect(body.total).toBe(1)
    expect(body.items[0]).toMatchObject({ actorId: editor.id, actorName: "editor" })
  })

  it("filters by Irish calendar days, inclusive", async () => {
    const { cookie } = await ownerWithEntries()
    const body = await (await adminRequest("/audit?from=2026-10-05&to=2026-10-05", cookie)).json()
    expect(actions(body)).toEqual(["auth.sign_in", "user.invite"])
  })

  it("rejects a range that ends before it starts", async () => {
    const { cookie } = await ownerWithEntries()
    const response = await adminRequest("/audit?from=2026-10-07&to=2026-10-05", cookie)
    expect(response.status).toBe(400)
    expect((await response.json()).error.fieldErrors.to).toBeDefined()
  })

  it("rejects a malformed actor id or date", async () => {
    const { cookie } = await ownerWithEntries()
    expect((await adminRequest("/audit?actor=nope", cookie)).status).toBe(400)
    expect((await adminRequest("/audit?from=2026-02-30", cookie)).status).toBe(400)
  })

  it("lists the actions and people to filter by", async () => {
    const { cookie, owner, editor } = await ownerWithEntries()
    const body = await (await adminRequest("/audit/facets", cookie)).json()
    expect(body.actions).toEqual(["auth.sign_in", "user.invite"])
    expect(body.actors.map((actor: { id: string }) => actor.id).sort()).toEqual(
      [owner.id, editor.id].sort()
    )
  })
```

Run: `pnpm test:integration tests/integration/api/audit.test.ts`
Expected: FAIL — filters are ignored (`expected 3 to be 2`) and `/audit/facets` is 404.

- [ ] **Step 3: Implement schema, repo, service and routes**

```ts
// server/modules/audit/schema.ts
import "server-only"

import * as z from "zod"

// docs/brief.md §8.2–8.3: list conventions plus the audit filters. Dates are
// Irish calendar days (YYYY-MM-DD), both ends inclusive.
export const AuditListQuery = z
  .object({
    page: z.coerce.number().int().min(1).default(1),
    pageSize: z.coerce.number().int().min(1).max(100).default(20),
    actor: z.uuid().optional(),
    action: z.string().trim().min(1).max(64).optional(),
    from: z.iso.date().optional(),
    to: z.iso.date().optional(),
  })
  .refine((query) => !query.from || !query.to || query.from <= query.to, {
    message: "The end date must be on or after the start date",
    path: ["to"],
  })

export type AuditListQuery = z.output<typeof AuditListQuery>
```

```ts
// server/modules/audit/repo.ts
import "server-only"

import { TZDate } from "@date-fns/tz"
import { and, asc, count, desc, eq, gte, lt, type SQL } from "drizzle-orm"

import { getDb } from "@/server/db/client"
import { auditLogs, users } from "@/server/db/schema"

export type AuditFilters = {
  actorId?: string
  action?: string
  from?: string
  to?: string
}

// Midnight at the start of an Irish calendar day (YYYY-MM-DD), plus
// `offsetDays` days; DST-safe because TZDate does the calendar maths.
export function dublinMidnight(day: string, offsetDays = 0) {
  const [year, month, date] = day.split("-").map(Number)
  return new Date(
    new TZDate(year!, month! - 1, date! + offsetDays, "Europe/Dublin").getTime()
  )
}

function where(filters: AuditFilters): SQL | undefined {
  const parts: SQL[] = []
  if (filters.actorId) parts.push(eq(auditLogs.actorId, filters.actorId))
  if (filters.action) parts.push(eq(auditLogs.action, filters.action))
  if (filters.from) {
    parts.push(gte(auditLogs.createdAt, dublinMidnight(filters.from)))
  }
  if (filters.to) {
    parts.push(lt(auditLogs.createdAt, dublinMidnight(filters.to, 1)))
  }
  return parts.length ? and(...parts) : undefined
}

export async function listAudit(
  filters: AuditFilters,
  page: number,
  pageSize: number
) {
  const db = getDb()
  const condition = where(filters)
  const [items, [{ total }]] = await Promise.all([
    db
      .select({
        id: auditLogs.id,
        actorId: auditLogs.actorId,
        actorEmail: auditLogs.actorEmail,
        actorName: users.name,
        action: auditLogs.action,
        entityType: auditLogs.entityType,
        entityId: auditLogs.entityId,
        summary: auditLogs.summary,
        createdAt: auditLogs.createdAt,
      })
      .from(auditLogs)
      .leftJoin(users, eq(users.id, auditLogs.actorId))
      .where(condition)
      .orderBy(desc(auditLogs.createdAt))
      .limit(pageSize)
      .offset((page - 1) * pageSize),
    db.select({ total: count() }).from(auditLogs).where(condition),
  ])
  return { items, total }
}

export async function auditFacets() {
  const db = getDb()
  const [actions, actors] = await Promise.all([
    db
      .selectDistinct({ action: auditLogs.action })
      .from(auditLogs)
      .orderBy(asc(auditLogs.action)),
    db
      .selectDistinct({ id: users.id, name: users.name, email: users.email })
      .from(auditLogs)
      .innerJoin(users, eq(users.id, auditLogs.actorId))
      .orderBy(asc(users.name)),
  ])
  return {
    actions: actions.map((row) => row.action),
    actors: actors.map((row) => ({
      id: row.id,
      label: `${row.name} (${row.email})`,
    })),
  }
}
```

Replace `server/modules/audit/service.ts`:

```ts
import "server-only"

import * as repo from "./repo"
import type { AuditListQuery } from "./schema"

// Plain DTOs: ISO dates, and never the IP hash or user agent.
export async function listAuditEntries(query: AuditListQuery) {
  const { page, pageSize, actor, action, from, to } = query
  const { items, total } = await repo.listAudit(
    { actorId: actor, action, from, to },
    page,
    pageSize
  )
  return {
    items: items.map((item) => ({
      ...item,
      createdAt: item.createdAt.toISOString(),
    })),
    page,
    pageSize,
    total,
  }
}

export const getAuditFacets = () => repo.auditFacets()
```

Replace `server/modules/audit/routes.ts`:

```ts
import "server-only"

import { zValidator } from "@hono/zod-validator"
import { Hono } from "hono"

import { validationHook } from "@/server/api/errors"
import { can } from "@/server/api/middleware/auth"
import type { AppEnv } from "@/server/api/types"

import { AuditListQuery } from "./schema"
import { getAuditFacets, listAuditEntries } from "./service"

export const auditRoutes = new Hono<AppEnv>()
  .get(
    "/",
    can({ audit: ["read"] }),
    zValidator("query", AuditListQuery, validationHook),
    async (c) => c.json(await listAuditEntries(c.req.valid("query")))
  )
  .get("/facets", can({ audit: ["read"] }), async (c) =>
    c.json(await getAuditFacets())
  )

export type AuditRoutes = typeof auditRoutes
```

Run: `pnpm test:integration tests/integration/api/audit.test.ts tests/integration/api/in-process.test.ts tests/integration/api/permission-matrix.test.ts`
Expected: PASS (the earlier tests keep passing: same envelope, `createdAt` was already an ISO string on the wire).

- [ ] **Step 4: Write the failing client tests**

```ts
// admin/modules/activity/params.test.ts
import { describe, expect, it } from "vitest"

import { toAuditParams } from "./params"

describe("toAuditParams", () => {
  it("maps URL state to the API's query, dropping empty filters", () => {
    expect(
      toAuditParams({
        page: 2,
        pageSize: 20,
        action: "auth.sign_in",
        actor: null,
        from: new Date("2026-10-05T00:00:00Z"),
        to: null,
      })
    ).toEqual({
      page: 2,
      pageSize: 20,
      action: "auth.sign_in",
      actor: undefined,
      from: "2026-10-05",
      to: undefined,
    })
  })

  it("keeps a hand-edited URL inside the API's limits", () => {
    const params = toAuditParams({
      page: -3,
      pageSize: 5000,
      action: null,
      actor: null,
      from: null,
      to: null,
    })
    expect(params).toMatchObject({ page: 1, pageSize: 100 })
  })
})
```

```tsx
// admin/components/data-table/data-table-pagination.test.tsx
// @vitest-environment jsdom
import { render, screen } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import { expect, it, vi } from "vitest"

import { DataTablePagination } from "./data-table-pagination"

it("pages forward and back within bounds", async () => {
  const onPageChange = vi.fn()
  render(
    <DataTablePagination page={1} pageSize={20} total={45} onPageChange={onPageChange} />
  )
  expect(screen.getByText("1–20 of 45")).toBeTruthy()
  expect(
    (screen.getByRole("button", { name: "Previous page" }) as HTMLButtonElement)
      .disabled
  ).toBe(true)
  await userEvent.click(screen.getByRole("button", { name: "Next page" }))
  expect(onPageChange).toHaveBeenCalledWith(2)
})

it("stops on the last page", () => {
  render(
    <DataTablePagination page={3} pageSize={20} total={45} onPageChange={() => {}} />
  )
  expect(screen.getByText("41–45 of 45")).toBeTruthy()
  expect(
    (screen.getByRole("button", { name: "Next page" }) as HTMLButtonElement)
      .disabled
  ).toBe(true)
})
```

Run: `pnpm vitest run --project unit admin/modules/activity admin/components/data-table`
Expected: FAIL with unresolved imports.

- [ ] **Step 5: Implement the DataTable foundation**

```tsx
// admin/components/data-table/data-table-pagination.tsx
"use client"

import { ChevronLeftIcon, ChevronRightIcon } from "lucide-react"

import { formatNumber } from "@/admin/components/dashboard/format"
import { Button } from "@/components/ui/button"
import { ButtonGroup } from "@/components/ui/button-group"

export function DataTablePagination({
  page,
  pageSize,
  total,
  onPageChange,
}: {
  page: number
  pageSize: number
  total: number
  onPageChange: (page: number) => void
}) {
  const pageCount = Math.max(1, Math.ceil(total / pageSize))
  const first = total === 0 ? 0 : (page - 1) * pageSize + 1
  const last = Math.min(total, page * pageSize)
  return (
    <nav
      aria-label="Pagination"
      className="flex flex-wrap items-center justify-between gap-3 text-sm"
    >
      <p aria-live="polite" className="text-muted-foreground tabular-nums">
        {total === 0
          ? "No entries"
          : `${first}–${last} of ${formatNumber(total)}`}
      </p>
      <ButtonGroup>
        <Button
          variant="outline"
          aria-label="Previous page"
          disabled={page <= 1}
          onClick={() => onPageChange(page - 1)}
        >
          <ChevronLeftIcon aria-hidden />
        </Button>
        <Button
          variant="outline"
          aria-label="Next page"
          disabled={page >= pageCount}
          onClick={() => onPageChange(page + 1)}
        >
          <ChevronRightIcon aria-hidden />
        </Button>
      </ButtonGroup>
    </nav>
  )
}
```

```tsx
// admin/components/data-table/data-table.tsx
"use client"

import type {
  ReactTable,
  Row,
  RowData,
  TableFeatures,
} from "@tanstack/react-table"

import {
  Table,
  TableBody,
  TableCaption,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table"
import { cn } from "@/lib/utils"

// Headless TanStack Table v9 instance in, Evergreen table out. Below 640px
// rows become cards (docs/brief.md §9.4).
export function DataTable<TFeatures extends TableFeatures, TData extends RowData>({
  table,
  caption,
  empty,
  renderCard,
  isFetching = false,
}: {
  table: ReactTable<TFeatures, TData>
  caption: string
  empty: React.ReactNode
  renderCard: (row: Row<TFeatures, TData>) => React.ReactNode
  isFetching?: boolean
}) {
  const rows = table.getRowModel().rows
  if (rows.length === 0) return <>{empty}</>
  return (
    <div
      aria-busy={isFetching || undefined}
      className={cn("flex flex-col gap-3 transition-opacity", isFetching && "opacity-70")}
    >
      <ul aria-label={caption} className="flex flex-col gap-3 sm:hidden">
        {rows.map((row) => (
          <li key={row.id} className="rounded-card bg-card p-4">
            {renderCard(row)}
          </li>
        ))}
      </ul>
      <div className="hidden overflow-hidden rounded-card bg-card sm:block">
        <Table>
          <TableCaption className="sr-only">{caption}</TableCaption>
          <TableHeader>
            {table.getHeaderGroups().map((group) => (
              <TableRow key={group.id}>
                {group.headers.map((header) => (
                  <TableHead key={header.id} scope="col" className="h-12 px-4">
                    {header.isPlaceholder ? null : (
                      <table.FlexRender header={header} />
                    )}
                  </TableHead>
                ))}
              </TableRow>
            ))}
          </TableHeader>
          <TableBody>
            {rows.map((row) => (
              <TableRow key={row.id}>
                {row.getAllCells().map((cell) => (
                  <TableCell key={cell.id} className="px-4 py-3">
                    <table.FlexRender cell={cell} />
                  </TableCell>
                ))}
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </div>
    </div>
  )
}
```

- [ ] **Step 6: Implement the activity module and page**

```ts
// admin/modules/activity/params.ts
import {
  createLoader,
  parseAsInteger,
  parseAsIsoDate,
  parseAsString,
} from "nuqs/server"

import type { AuditListParams } from "@/admin/lib/query-keys"

// URL state for /admin/activity, shared by the RSC prefetch and the client
// (docs/brief.md §8.4: table state mirrors into the URL and is the key).
export const activityParsers = {
  page: parseAsInteger.withDefault(1),
  pageSize: parseAsInteger.withDefault(20),
  action: parseAsString,
  actor: parseAsString,
  from: parseAsIsoDate,
  to: parseAsIsoDate,
}

export const loadActivityParams = createLoader(activityParsers)

// parseAsIsoDate reads YYYY-MM-DD as UTC midnight; slicing the ISO string
// gives the same calendar day back.
const day = (date: Date | null) =>
  date ? date.toISOString().slice(0, 10) : undefined

export function toAuditParams(state: {
  page: number
  pageSize: number
  action: string | null
  actor: string | null
  from: Date | null
  to: Date | null
}): AuditListParams {
  return {
    page: Math.max(1, Math.trunc(state.page)),
    pageSize: Math.min(100, Math.max(1, Math.trunc(state.pageSize))),
    action: state.action ?? undefined,
    actor: state.actor ?? undefined,
    from: day(state.from),
    to: day(state.to),
  }
}
```

```ts
// admin/modules/activity/queries.ts
import { queryOptions } from "@tanstack/react-query"
import type { InferResponseType } from "hono/client"

import { auditApi, parseResponse } from "@/admin/lib/api"
import { queryKeys, type AuditListParams } from "@/admin/lib/query-keys"

type AuditClient = typeof auditApi

export type AuditPage = InferResponseType<typeof auditApi.index.$get, 200>
export type AuditRow = AuditPage["items"][number]

function toQuery(params: AuditListParams) {
  return {
    page: String(params.page),
    pageSize: String(params.pageSize),
    ...(params.action ? { action: params.action } : {}),
    ...(params.actor ? { actor: params.actor } : {}),
    ...(params.from ? { from: params.from } : {}),
    ...(params.to ? { to: params.to } : {}),
  }
}

// `client` is the browser's auditApi, or inProcessAuditApi() in the RSC.
export const auditListQuery = (
  params: AuditListParams,
  client: AuditClient = auditApi
) =>
  queryOptions({
    queryKey: queryKeys.audit.list(params),
    queryFn: () => parseResponse(client.index.$get({ query: toQuery(params) })),
  })

export const auditFacetsQuery = (client: AuditClient = auditApi) =>
  queryOptions({
    queryKey: queryKeys.audit.facets,
    queryFn: () => parseResponse(client.facets.$get()),
    staleTime: 5 * 60_000,
  })
```

```tsx
// admin/modules/activity/activity-view.tsx
"use client"

import { keepPreviousData, useQuery } from "@tanstack/react-query"
import {
  createColumnHelper,
  functionalUpdate,
  rowPaginationFeature,
  tableFeatures,
  useTable,
} from "@tanstack/react-table"
import { HistoryIcon } from "lucide-react"
import { useQueryStates } from "nuqs"

import { EmptyState } from "@/admin/components/dashboard/empty-state"
import { PageHeader } from "@/admin/components/dashboard/page-header"
import { CardSkeleton } from "@/admin/components/dashboard/skeletons"
import { DataTable } from "@/admin/components/data-table/data-table"
import { DataTablePagination } from "@/admin/components/data-table/data-table-pagination"
import { Button } from "@/components/ui/button"
import { Field, FieldLabel } from "@/components/ui/field"
import { Input } from "@/components/ui/input"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"

import { activityParsers, toAuditParams } from "./params"
import {
  auditFacetsQuery,
  auditListQuery,
  type AuditRow,
} from "./queries"

const features = tableFeatures({ rowPaginationFeature })
const helper = createColumnHelper<typeof features, AuditRow>()
const EMPTY: AuditRow[] = []
const ALL = "__all__"

const WHEN = new Intl.DateTimeFormat("en-IE", {
  day: "numeric",
  month: "short",
  year: "numeric",
  hour: "2-digit",
  minute: "2-digit",
  timeZone: "Europe/Dublin",
})

const who = (row: AuditRow) => row.actorName ?? row.actorEmail ?? "System"

const columns = helper.columns([
  helper.accessor("createdAt", {
    header: "When",
    cell: (info) => (
      <time dateTime={info.getValue()} className="whitespace-nowrap tabular-nums">
        {WHEN.format(new Date(info.getValue()))}
      </time>
    ),
  }),
  helper.accessor((row) => who(row), { id: "actor", header: "Who" }),
  helper.accessor("action", {
    header: "Action",
    cell: (info) => <code className="font-mono text-xs">{info.getValue()}</code>,
  }),
  helper.accessor("summary", { header: "What" }),
])

const toInputDay = (date: Date | null) =>
  date ? date.toISOString().slice(0, 10) : ""

export function ActivityView() {
  const [state, setState] = useQueryStates(activityParsers, {
    history: "replace",
  })
  const params = toAuditParams(state)
  const list = useQuery({
    ...auditListQuery(params),
    placeholderData: keepPreviousData,
  })
  const facets = useQuery(auditFacetsQuery())
  const pagination = { pageIndex: params.page - 1, pageSize: params.pageSize }

  const table = useTable({
    features,
    columns,
    data: list.data?.items ?? EMPTY,
    getRowId: (row) => row.id,
    manualPagination: true,
    rowCount: list.data?.total ?? 0,
    state: { pagination },
    onPaginationChange: (updater) => {
      const next = functionalUpdate(updater, pagination)
      void setState({ page: next.pageIndex + 1, pageSize: next.pageSize })
    },
  })

  const actionItems = [
    { value: ALL, label: "All actions" },
    ...(facets.data?.actions ?? []).map((action) => ({
      value: action,
      label: action,
    })),
  ]
  const actorItems = [
    { value: ALL, label: "Everyone" },
    ...(facets.data?.actors ?? []).map((actor) => ({
      value: actor.id,
      label: actor.label,
    })),
  ]
  // Any filter change starts again from page 1.
  const filter = (values: Partial<typeof state>) =>
    void setState({ ...values, page: 1 })

  return (
    <div className="flex flex-col gap-6">
      <PageHeader
        title="Activity log"
        description="Who changed what, and when. Times are Irish time."
      />
      <div role="group" aria-label="Filters" className="flex flex-wrap items-end gap-3">
        <Field className="w-full sm:w-56">
          <FieldLabel htmlFor="activity-action">Action</FieldLabel>
          <Select
            items={actionItems}
            value={state.action ?? ALL}
            onValueChange={(value) =>
              filter({ action: value === ALL ? null : String(value) })
            }
          >
            <SelectTrigger id="activity-action" className="w-full">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {actionItems.map((item) => (
                <SelectItem key={item.value} value={item.value}>
                  {item.label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </Field>
        <Field className="w-full sm:w-64">
          <FieldLabel htmlFor="activity-actor">Person</FieldLabel>
          <Select
            items={actorItems}
            value={state.actor ?? ALL}
            onValueChange={(value) =>
              filter({ actor: value === ALL ? null : String(value) })
            }
          >
            <SelectTrigger id="activity-actor" className="w-full">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {actorItems.map((item) => (
                <SelectItem key={item.value} value={item.value}>
                  {item.label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </Field>
        <Field className="w-[calc(50%-0.375rem)] sm:w-44">
          <FieldLabel htmlFor="activity-from">From</FieldLabel>
          <Input
            id="activity-from"
            type="date"
            value={toInputDay(state.from)}
            onChange={(event) =>
              filter({ from: event.target.value ? new Date(event.target.value) : null })
            }
          />
        </Field>
        <Field className="w-[calc(50%-0.375rem)] sm:w-44">
          <FieldLabel htmlFor="activity-to">To</FieldLabel>
          <Input
            id="activity-to"
            type="date"
            value={toInputDay(state.to)}
            onChange={(event) =>
              filter({ to: event.target.value ? new Date(event.target.value) : null })
            }
          />
        </Field>
        <Button
          variant="ghost"
          className="h-9"
          onClick={() => filter({ action: null, actor: null, from: null, to: null })}
        >
          Reset filters
        </Button>
      </div>
      {list.isPending ? (
        <CardSkeleton />
      ) : list.isError ? (
        <EmptyState
          title="The activity log didn't load"
          description={list.error.message}
          action={<Button onClick={() => list.refetch()}>Try again</Button>}
        />
      ) : (
        <>
          <DataTable
            table={table}
            caption="Activity log"
            isFetching={list.isFetching}
            empty={
              <EmptyState
                icon={HistoryIcon}
                title="Nothing matches these filters"
                description="Try a wider date range or reset the filters."
              />
            }
            renderCard={(row) => (
              <div className="flex flex-col gap-1 text-sm">
                <p className="font-semibold">{who(row.original)}</p>
                <p>{row.original.summary}</p>
                <p className="text-xs text-muted-foreground">
                  <code className="font-mono">{row.original.action}</code> ·{" "}
                  {WHEN.format(new Date(row.original.createdAt))}
                </p>
              </div>
            )}
          />
          <DataTablePagination
            page={params.page}
            pageSize={params.pageSize}
            total={list.data.total}
            onPageChange={(page) => table.setPageIndex(page - 1)}
          />
        </>
      )}
    </div>
  )
}
```

```tsx
// app/(admin)/admin/(panel)/activity/page.tsx
import { dehydrate, HydrationBoundary } from "@tanstack/react-query"
import type { Metadata } from "next"
import { Suspense } from "react"

import { CardSkeleton } from "@/admin/components/dashboard/skeletons"
import { getQueryClient } from "@/admin/lib/query-client"
import { ActivityView } from "@/admin/modules/activity/activity-view"
import { loadActivityParams, toAuditParams } from "@/admin/modules/activity/params"
import {
  auditFacetsQuery,
  auditListQuery,
} from "@/admin/modules/activity/queries"
import { inProcessAuditApi } from "@/server/api/in-process"
import { requirePermission } from "@/server/auth/session"

export const metadata: Metadata = { title: "Activity log" }

type SearchParams = Promise<Record<string, string | string[] | undefined>>

export default function ActivityPage({
  searchParams,
}: {
  searchParams: SearchParams
}) {
  return (
    <Suspense fallback={<CardSkeleton />}>
      <Activity searchParams={searchParams} />
    </Suspense>
  )
}

async function Activity({ searchParams }: { searchParams: SearchParams }) {
  await requirePermission({ audit: ["read"] })
  const params = toAuditParams(loadActivityParams(await searchParams))
  const queryClient = getQueryClient()
  const client = inProcessAuditApi()
  // A failed prefetch is not fatal: the client query retries and shows its
  // own error state.
  await Promise.all([
    queryClient.query(auditListQuery(params, client)),
    queryClient.query(auditFacetsQuery(client)),
  ]).catch(() => undefined)
  return (
    <HydrationBoundary state={dehydrate(queryClient)}>
      <ActivityView />
    </HydrationBoundary>
  )
}
```

Run: `pnpm vitest run --project unit admin/modules/activity admin/components/data-table && pnpm typecheck`
Expected: PASS and no type errors.

- [ ] **Step 7: Write the activity E2E**

```ts
// tests/e2e/admin/shell/activity.spec.ts
import { expect, test } from "@playwright/test"

import { STORAGE } from "../../support/storage"

test.describe("as an owner", () => {
  test.use({ storageState: STORAGE.owner })

  test("filters by action, keeps the filter in the URL and across a reload", async ({
    page,
  }) => {
    await page.goto("/admin/activity")
    await expect(
      page.getByRole("heading", { level: 1, name: "Activity log" })
    ).toBeVisible()
    await page.getByRole("combobox", { name: "Action" }).click()
    await page.getByRole("option", { name: "auth.sign_in" }).click()
    await expect(page).toHaveURL(/[?&]action=auth\.sign_in/)

    const rows = test.info().project.name === "mobile"
      ? page.getByRole("list", { name: "Activity log" }).getByRole("listitem")
      : page.getByRole("table").getByRole("row").filter({ has: page.locator("td") })
    await expect(rows.first()).toContainText("auth.sign_in")
    for (const text of await rows.allTextContents()) {
      expect(text).toContain("auth.sign_in")
    }

    await page.reload()
    await expect(page.getByRole("combobox", { name: "Action" })).toHaveText(
      "auth.sign_in"
    )
  })
})

test.describe("as an editor", () => {
  test.use({ storageState: STORAGE.editor })

  test("is not allowed in", async ({ page }) => {
    await page.goto("/admin/activity")
    await expect(
      page.getByRole("heading", { name: "You don't have access to Activity log" })
    ).toBeVisible()
  })
})
```

Run: `pnpm test:e2e tests/e2e/admin/shell/activity.spec.ts`
Expected: PASS on all three projects (Pixel 7 is below 640 px, so it checks the card list; tablet and desktop the table).

- [ ] **Step 8: Gates**

Run: `pnpm lint && pnpm typecheck && pnpm test:unit && pnpm test:integration && pnpm build`
Expected: all green.

- [ ] **Step 9: Commit**

```bash
git add components/ui server/modules/audit admin "app/(admin)/admin/(panel)/activity" tests/integration/api/audit.test.ts tests/e2e/admin/shell/activity.spec.ts package.json pnpm-lock.yaml
git commit -m "feat(admin): DataTable with URL state and a filterable activity log"
```

---

## Task 2.10: SchemaForm foundation and the Account page

**Files:**
- Modify: `package.json` (`react-hook-form@7.89.0`, `@hookform/resolvers@5.9.1`)
- Create (shadcn CLI): `components/ui/{switch,tabs,textarea,radio-group,toggle-group,toggle,spinner}.tsx` (skips any that exist)
- Create: `admin/components/schema-form/{registry.ts,to-fields.ts,to-fields.test.ts,schema-form.tsx,schema-form.test.tsx,widgets.tsx}`
- Create: `admin/modules/account/{schemas.ts,describe-agent.ts,describe-agent.test.ts,profile-form.tsx,password-form.tsx,sessions-list.tsx,theme-preference.tsx,account-view.tsx}`
- Create: `app/(admin)/admin/(panel)/account/page.tsx`
- Create: `admin/modules/design/demo-form.tsx`; Modify: `admin/modules/design/showcase.tsx`
- Create: `tests/e2e/admin/auth/account.spec.ts`

**Interfaces:**
- Consumes: `authClient` (Phase 1), `authRequest` (`admin/modules/auth/auth-errors.ts`), `useActor` (2.3), `queryKeys.account.sessions` (2.4), `StatusPill`, `relativeTime`, `CardSkeleton` (2.7), `Field*` (`components/ui/field.tsx`), `ADMIN_THEME_STORAGE_KEY` via `useTheme` (2.2).
- Produces:
  - `admin/components/schema-form/registry.ts`: `type Widget = "text" | "textarea" | "number" | "toggle" | "select" | "radio" | "list" | "group"`, `type UiMeta = { label: string; description?; placeholder?; widget?: Widget; options?: readonly { value: string; label: string }[]; inputType?: "text" | "email" | "password" | "url" | "tel"; autoComplete?: string }`, `ui = z.registry<UiMeta>()`. Convention: call `.register(ui, …)` last, after `.optional()` / `.default()`.
  - `to-fields.ts`: `type FormField = { name; label; widget: Widget; required: boolean; description?; placeholder?; inputType?; autoComplete?; maxLength?: number; minItems?: number; maxItems?: number; options?: { value; label }[]; fields?: FormField[]; item?: FormField }`, `toJsonSchema(schema)`, `toFields(schema: z.ZodObject): FormField[]`.
  - `schema-form.tsx`: `SchemaForm({ schema, defaultValues, onSubmit, submitLabel, resetOnSuccess? })`, `type SubmitResult = void | { formError?: string; fieldErrors?: Record<string, string[]> }`.
  - Account: `ProfileSchema`, `PasswordSchema`, `describeAgent(userAgent: string | null | undefined): string`, `AccountView()` with tabs `profile | security | sessions` in `?tab=`.

- [ ] **Step 1: Install and add components**

Run: `pnpm add react-hook-form@7.89.0 @hookform/resolvers@5.9.1 && pnpm exec shadcn add switch tabs textarea radio-group toggle-group spinner --yes`
Expected: dependencies and components added; existing files skipped.

- [ ] **Step 2: Write the failing SchemaForm tests**

```ts
// admin/components/schema-form/to-fields.test.ts
import * as z from "zod"
import { describe, expect, it } from "vitest"

import { ui } from "./registry"
import { toFields } from "./to-fields"

const Schema = z
  .object({
    name: z.string().trim().min(1).max(80).register(ui, { label: "Full name", placeholder: "Magda Kennedy" }),
    bio: z.string().max(400).optional().register(ui, { label: "Short bio", widget: "textarea" }),
    sessions: z.number().int().min(1).max(10).register(ui, { label: "Sessions" }),
    newsletter: z.boolean().default(true).register(ui, { label: "Send me the newsletter" }),
    mode: z.enum(["online", "in-person"]).register(ui, {
      label: "Mode",
      widget: "radio",
      options: [
        { value: "online", label: "Online" },
        { value: "in-person", label: "In person" },
      ],
    }),
    tone: z.enum(["warm", "plain"]).register(ui, { label: "Tone" }),
    tags: z.array(z.string().max(30)).min(1).max(3).register(ui, { label: "Tags" }),
    address: z
      .object({ city: z.string().register(ui, { label: "City" }) })
      .register(ui, { label: "Address" }),
    confirm: z.string(),
  })
  .refine((value) => value.name === value.confirm, { path: ["confirm"], message: "x" })

describe("toFields", () => {
  const fields = Object.fromEntries(toFields(Schema).map((f) => [f.name, f]))

  it("reads labels and hints from the UI registry", () => {
    expect(fields.name).toMatchObject({
      widget: "text",
      label: "Full name",
      placeholder: "Magda Kennedy",
      required: true,
      maxLength: 80,
    })
  })

  it("infers widgets from the JSON Schema when none is given", () => {
    expect(fields.sessions.widget).toBe("number")
    expect(fields.newsletter).toMatchObject({ widget: "toggle", required: false })
    expect(fields.tone).toMatchObject({
      widget: "select",
      options: [
        { value: "warm", label: "Warm" },
        { value: "plain", label: "Plain" },
      ],
    })
    expect(fields.tags).toMatchObject({ widget: "list", minItems: 1, maxItems: 3 })
    expect(fields.address.fields?.map((f) => f.name)).toEqual(["address.city"])
  })

  it("honours an explicit widget and options", () => {
    expect(fields.bio).toMatchObject({ widget: "textarea", required: false })
    expect(fields.mode.options?.[1]).toEqual({ value: "in-person", label: "In person" })
  })

  it("humanises a field with no metadata", () => {
    expect(fields.confirm.label).toBe("Confirm")
  })
})
```

```tsx
// admin/components/schema-form/schema-form.test.tsx
// @vitest-environment jsdom
import { render, screen } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import * as z from "zod"
import { describe, expect, it, vi } from "vitest"

import { ui } from "./registry"
import { SchemaForm } from "./schema-form"

const Schema = z
  .object({
    name: z.string().trim().min(1, "Enter your name").max(20).register(ui, { label: "Full name" }),
    newsletter: z.boolean().default(false).register(ui, { label: "Newsletter" }),
    password: z.string().min(4, "Too short").register(ui, { label: "Password", inputType: "password" }),
    confirm: z.string().register(ui, { label: "Confirm password", inputType: "password" }),
  })
  .refine((v) => v.password === v.confirm, {
    path: ["confirm"],
    message: "The passwords don't match",
  })

const defaults = { name: "", newsletter: false, password: "", confirm: "" }

describe("SchemaForm", () => {
  it("renders labelled widgets with a live character counter", async () => {
    render(<SchemaForm schema={Schema} defaultValues={defaults} submitLabel="Save" onSubmit={vi.fn()} />)
    expect(screen.getByText("0/20")).toBeTruthy()
    await userEvent.type(screen.getByLabelText("Full name"), "Magda")
    expect(screen.getByText("5/20")).toBeTruthy()
    expect(screen.getByRole("switch", { name: "Newsletter" })).toBeTruthy()
    expect(screen.getByLabelText("Password").getAttribute("type")).toBe("password")
  })

  // toJSONSchema drops refinements; validation must still use them.
  it("validates with the Zod schema, refinements included", async () => {
    const onSubmit = vi.fn()
    render(<SchemaForm schema={Schema} defaultValues={defaults} submitLabel="Save" onSubmit={onSubmit} />)
    await userEvent.type(screen.getByLabelText("Full name"), "Magda")
    await userEvent.type(screen.getByLabelText("Password"), "secret")
    await userEvent.type(screen.getByLabelText("Confirm password"), "other")
    await userEvent.click(screen.getByRole("button", { name: "Save" }))
    expect(await screen.findByText("The passwords don't match")).toBeTruthy()
    expect(onSubmit).not.toHaveBeenCalled()
  })

  it("submits the parsed output", async () => {
    const onSubmit = vi.fn()
    render(<SchemaForm schema={Schema} defaultValues={defaults} submitLabel="Save" onSubmit={onSubmit} />)
    await userEvent.type(screen.getByLabelText("Full name"), "  Magda  ")
    await userEvent.type(screen.getByLabelText("Password"), "secret")
    await userEvent.type(screen.getByLabelText("Confirm password"), "secret")
    await userEvent.click(screen.getByRole("switch", { name: "Newsletter" }))
    await userEvent.click(screen.getByRole("button", { name: "Save" }))
    await vi.waitFor(() =>
      expect(onSubmit).toHaveBeenCalledWith({
        name: "Magda",
        newsletter: true,
        password: "secret",
        confirm: "secret",
      })
    )
  })

  it("shows field and form errors from the server", async () => {
    const onSubmit = vi.fn(async () => ({
      fieldErrors: { name: ["That name is taken"] },
      formError: "Nothing was saved",
    }))
    render(<SchemaForm schema={Schema} defaultValues={{ ...defaults, name: "M", password: "abcd", confirm: "abcd" }} submitLabel="Save" onSubmit={onSubmit} />)
    await userEvent.click(screen.getByRole("button", { name: "Save" }))
    expect(await screen.findByText("That name is taken")).toBeTruthy()
    expect(screen.getByText("Nothing was saved")).toBeTruthy()
  })
})
```

Run: `pnpm vitest run --project unit admin/components/schema-form`
Expected: FAIL with unresolved `./registry`, `./to-fields`, `./schema-form`.

- [ ] **Step 3: Implement the registry and the JSON Schema walker**

```ts
// admin/components/schema-form/registry.ts
import * as z from "zod"

// docs/brief.md §9.4: each Zod schema carries UI metadata in a registry,
// and toJSONSchema copies it into `x-ui`. Register last (after .optional()
// or .default()) so the metadata sits on the schema the form sees.
export type Widget =
  | "text"
  | "textarea"
  | "number"
  | "toggle"
  | "select"
  | "radio"
  | "list"
  | "group"

export type UiMeta = {
  label: string
  description?: string
  placeholder?: string
  widget?: Widget
  options?: readonly { value: string; label: string }[]
  inputType?: "text" | "email" | "password" | "url" | "tel"
  autoComplete?: string
}

export const ui = z.registry<UiMeta>()
```

```ts
// admin/components/schema-form/to-fields.ts
import * as z from "zod"

import { ui, type UiMeta, type Widget } from "./registry"

export type FormField = {
  name: string
  label: string
  widget: Widget
  required: boolean
  description?: string
  placeholder?: string
  inputType?: UiMeta["inputType"]
  autoComplete?: string
  maxLength?: number
  minItems?: number
  maxItems?: number
  options?: { value: string; label: string }[]
  fields?: FormField[]
  item?: FormField
}

type JsonNode = {
  type?: string | string[]
  properties?: Record<string, JsonNode>
  required?: string[]
  items?: JsonNode
  enum?: unknown[]
  maxLength?: number
  minItems?: number
  maxItems?: number
  "x-ui"?: UiMeta
}

// Input shape (defaults make fields optional). Refinements are not
// representable and are dropped here; validation uses the Zod schema itself.
export function toJsonSchema(schema: z.ZodType): JsonNode {
  return z.toJSONSchema(schema, {
    io: "input",
    unrepresentable: "any",
    override: ({ zodSchema, jsonSchema }) => {
      const meta = ui.get(zodSchema)
      if (meta) (jsonSchema as Record<string, unknown>)["x-ui"] = meta
    },
  }) as JsonNode
}

const humanise = (name: string) => {
  const last = name.split(".").at(-1) ?? ""
  const words = last.replace(/([a-z0-9])([A-Z])/g, "$1 $2").replace(/[-_]/g, " ")
  return words ? words[0]!.toUpperCase() + words.slice(1).toLowerCase() : ""
}

function inferWidget(node: JsonNode): Widget {
  if (node.enum) return "select"
  const type = Array.isArray(node.type)
    ? node.type.find((t) => t !== "null")
    : node.type
  switch (type) {
    case "boolean":
      return "toggle"
    case "number":
    case "integer":
      return "number"
    case "object":
      return "group"
    case "array":
      return "list"
    default:
      return (node.maxLength ?? 0) > 160 ? "textarea" : "text"
  }
}

function toField(node: JsonNode, name: string, required: boolean): FormField {
  const meta = node["x-ui"]
  const widget = meta?.widget ?? inferWidget(node)
  const base: FormField = {
    name,
    widget,
    required,
    label: meta?.label ?? humanise(name),
    description: meta?.description,
    placeholder: meta?.placeholder,
    inputType: meta?.inputType,
    autoComplete: meta?.autoComplete,
  }
  if (widget === "group") return { ...base, fields: fieldsOf(node, name) }
  if (widget === "list") {
    return {
      ...base,
      minItems: node.minItems,
      maxItems: node.maxItems,
      item: toField(node.items ?? {}, "", true),
    }
  }
  if (widget === "select" || widget === "radio") {
    return {
      ...base,
      options:
        meta?.options?.map((option) => ({ ...option })) ??
        (node.enum ?? []).map((value) => ({
          value: String(value),
          label: humanise(String(value)),
        })),
    }
  }
  return { ...base, maxLength: node.maxLength }
}

function fieldsOf(node: JsonNode, prefix: string): FormField[] {
  const required = new Set(node.required ?? [])
  return Object.entries(node.properties ?? {}).map(([key, child]) =>
    toField(child, prefix ? `${prefix}.${key}` : key, required.has(key))
  )
}

export function toFields(schema: z.ZodObject): FormField[] {
  return fieldsOf(toJsonSchema(schema), "")
}
```

Run: `pnpm vitest run --project unit admin/components/schema-form/to-fields.test.ts`
Expected: PASS.

- [ ] **Step 4: Implement the widgets and the form**

```tsx
// admin/components/schema-form/widgets.tsx
"use client"

import { ChevronDownIcon, ChevronUpIcon, PlusIcon, TrashIcon } from "lucide-react"
import { useId } from "react"
import {
  Controller,
  type Control,
  type ControllerRenderProps,
  type FieldValues,
} from "react-hook-form"

import { Button } from "@/components/ui/button"
import {
  Field,
  FieldContent,
  FieldDescription,
  FieldError,
  FieldGroup,
  FieldLabel,
  FieldLegend,
  FieldSet,
} from "@/components/ui/field"
import { Input } from "@/components/ui/input"
import { RadioGroup, RadioGroupItem } from "@/components/ui/radio-group"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { Switch } from "@/components/ui/switch"
import { Textarea } from "@/components/ui/textarea"

import type { FormField } from "./to-fields"

type RenderInput = ControllerRenderProps<FieldValues, string>

export function FieldWidget({
  field,
  control,
}: {
  field: FormField
  control: Control<FieldValues>
}) {
  if (field.widget === "group") {
    return (
      <FieldSet>
        <FieldLegend>{field.label}</FieldLegend>
        {field.description ? (
          <FieldDescription>{field.description}</FieldDescription>
        ) : null}
        <FieldGroup>
          {(field.fields ?? []).map((child) => (
            <FieldWidget key={child.name} field={child} control={control} />
          ))}
        </FieldGroup>
      </FieldSet>
    )
  }
  return (
    <Controller
      name={field.name}
      control={control}
      render={({ field: input, fieldState }) =>
        field.widget === "list" ? (
          <ListWidget field={field} input={input} error={fieldState.error?.message} />
        ) : (
          <ScalarWidget field={field} input={input} error={fieldState.error?.message} />
        )
      }
    />
  )
}

function ScalarWidget({
  field,
  input,
  error,
}: {
  field: FormField
  input: RenderInput
  error?: string
}) {
  const id = useId()
  const invalid = Boolean(error)
  const counted =
    field.maxLength !== undefined &&
    (field.widget === "textarea" ||
      (field.widget === "text" && field.inputType !== "password"))
  const describedBy =
    [
      field.description && `${id}-description`,
      counted && `${id}-count`,
      error && `${id}-error`,
    ]
      .filter(Boolean)
      .join(" ") || undefined
  const aria = {
    "aria-invalid": invalid || undefined,
    "aria-describedby": describedBy,
    "aria-required": field.required || undefined,
  }
  const description = field.description ? (
    <FieldDescription id={`${id}-description`}>{field.description}</FieldDescription>
  ) : null
  const errorText = error ? <FieldError id={`${id}-error`}>{error}</FieldError> : null
  const options = field.options ?? []

  if (field.widget === "toggle") {
    return (
      <Field orientation="horizontal" data-invalid={invalid || undefined}>
        <Switch
          id={id}
          checked={Boolean(input.value)}
          onCheckedChange={(checked) => input.onChange(checked)}
          {...aria}
        />
        <FieldContent>
          <FieldLabel htmlFor={id}>{field.label}</FieldLabel>
          {description}
          {errorText}
        </FieldContent>
      </Field>
    )
  }

  if (field.widget === "radio") {
    return (
      <FieldSet data-invalid={invalid || undefined}>
        <FieldLegend variant="label">{field.label}</FieldLegend>
        {description}
        <RadioGroup
          value={input.value ?? ""}
          onValueChange={(value) => input.onChange(value)}
          {...aria}
        >
          {options.map((option) => (
            <Field key={option.value} orientation="horizontal">
              <RadioGroupItem value={option.value} id={`${id}-${option.value}`} />
              <FieldLabel htmlFor={`${id}-${option.value}`} className="font-normal">
                {option.label}
              </FieldLabel>
            </Field>
          ))}
        </RadioGroup>
        {errorText}
      </FieldSet>
    )
  }

  let control: React.ReactNode
  if (field.widget === "textarea") {
    control = (
      <Textarea
        id={id}
        name={input.name}
        ref={input.ref}
        rows={4}
        placeholder={field.placeholder}
        value={input.value ?? ""}
        onBlur={input.onBlur}
        onChange={(event) => input.onChange(event.target.value)}
        {...aria}
      />
    )
  } else if (field.widget === "number") {
    control = (
      <Input
        id={id}
        name={input.name}
        ref={input.ref}
        type="number"
        inputMode="numeric"
        placeholder={field.placeholder}
        value={input.value ?? ""}
        onBlur={input.onBlur}
        onChange={(event) =>
          input.onChange(event.target.value === "" ? undefined : event.target.valueAsNumber)
        }
        {...aria}
      />
    )
  } else if (field.widget === "select") {
    control = (
      <Select
        items={options}
        value={input.value ?? null}
        onValueChange={(value) => input.onChange(value)}
      >
        <SelectTrigger id={id} className="w-full" onBlur={input.onBlur} {...aria}>
          <SelectValue placeholder="Choose…" />
        </SelectTrigger>
        <SelectContent>
          {options.map((option) => (
            <SelectItem key={option.value} value={option.value}>
              {option.label}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    )
  } else {
    control = (
      <Input
        id={id}
        name={input.name}
        ref={input.ref}
        type={field.inputType ?? "text"}
        autoComplete={field.autoComplete}
        placeholder={field.placeholder}
        value={input.value ?? ""}
        onBlur={input.onBlur}
        onChange={(event) => input.onChange(event.target.value)}
        {...aria}
      />
    )
  }

  return (
    <Field data-invalid={invalid || undefined}>
      <div className="flex items-baseline justify-between gap-3">
        <FieldLabel htmlFor={id}>{field.label}</FieldLabel>
        {counted ? (
          <span id={`${id}-count`} className="text-xs text-muted-foreground tabular-nums">
            {String(input.value ?? "").length}/{field.maxLength}
          </span>
        ) : null}
      </div>
      {control}
      {description}
      {errorText}
    </Field>
  )
}

// Phase 2 lists hold text items and reorder with buttons (fully keyboard
// operable). Drag handles (dnd-kit) arrive with the page builder in Phase 4.
function ListWidget({
  field,
  input,
  error,
}: {
  field: FormField
  input: RenderInput
  error?: string
}) {
  const items: string[] = Array.isArray(input.value) ? input.value : []
  const min = field.minItems ?? 0
  const max = field.maxItems ?? Number.POSITIVE_INFINITY
  const set = (next: string[]) => input.onChange(next)
  const move = (from: number, to: number) => {
    const next = [...items]
    const [moved] = next.splice(from, 1)
    next.splice(to, 0, moved!)
    set(next)
  }
  return (
    <FieldSet data-invalid={Boolean(error) || undefined}>
      <FieldLegend variant="label">{field.label}</FieldLegend>
      <ol className="flex flex-col gap-2">
        {items.map((value, index) => (
          <li key={index} className="flex items-center gap-2">
            <Input
              aria-label={`${field.label} ${index + 1}`}
              value={value}
              onBlur={input.onBlur}
              onChange={(event) =>
                set(items.map((item, i) => (i === index ? event.target.value : item)))
              }
            />
            <Button
              type="button"
              variant="outline"
              size="icon"
              aria-label={`Move ${field.label} ${index + 1} up`}
              disabled={index === 0}
              onClick={() => move(index, index - 1)}
            >
              <ChevronUpIcon aria-hidden />
            </Button>
            <Button
              type="button"
              variant="outline"
              size="icon"
              aria-label={`Move ${field.label} ${index + 1} down`}
              disabled={index === items.length - 1}
              onClick={() => move(index, index + 1)}
            >
              <ChevronDownIcon aria-hidden />
            </Button>
            <Button
              type="button"
              variant="ghost"
              size="icon"
              aria-label={`Remove ${field.label} ${index + 1}`}
              disabled={items.length <= min}
              onClick={() => set(items.filter((_, i) => i !== index))}
            >
              <TrashIcon aria-hidden />
            </Button>
          </li>
        ))}
      </ol>
      <div>
        <Button
          type="button"
          variant="outline"
          disabled={items.length >= max}
          onClick={() => set([...items, ""])}
        >
          <PlusIcon data-icon="inline-start" />
          Add to {field.label.toLowerCase()}
        </Button>
      </div>
      {Number.isFinite(max) ? (
        <FieldDescription>
          {min > 0 ? `Between ${min} and ${max} items.` : `Up to ${max} items.`}
        </FieldDescription>
      ) : null}
      {error ? <FieldError>{error}</FieldError> : null}
    </FieldSet>
  )
}
```

```tsx
// admin/components/schema-form/schema-form.tsx
"use client"

import { zodResolver } from "@hookform/resolvers/zod"
import { useMemo } from "react"
import {
  useForm,
  type DefaultValues,
  type FieldValues,
  type Resolver,
} from "react-hook-form"
import type * as z from "zod"

import { Button } from "@/components/ui/button"
import { FieldGroup } from "@/components/ui/field"
import { Spinner } from "@/components/ui/spinner"

import { toFields } from "./to-fields"
import { FieldWidget } from "./widgets"

export type SubmitResult =
  | void
  | { formError?: string; fieldErrors?: Record<string, string[]> }

// docs/brief.md §9.4: the form is generated from the schema's JSON Schema
// (with x-ui hints) and validated by the same Zod schema via zodResolver.
// Server errors (e.g. an ApiError's fieldErrors) map back onto fields.
export function SchemaForm<S extends z.ZodObject>({
  schema,
  defaultValues,
  onSubmit,
  submitLabel,
  resetOnSuccess = false,
}: {
  schema: S
  defaultValues: z.input<S>
  onSubmit: (values: z.output<S>) => Promise<SubmitResult> | SubmitResult
  submitLabel: string
  resetOnSuccess?: boolean
}) {
  const fields = useMemo(() => toFields(schema), [schema])
  const form = useForm<FieldValues, unknown, FieldValues>({
    resolver: zodResolver(
      schema as unknown as z.ZodType<FieldValues, FieldValues>
    ) as Resolver<FieldValues, unknown, FieldValues>,
    defaultValues: defaultValues as DefaultValues<FieldValues>,
    mode: "onTouched",
  })

  const submit = form.handleSubmit(async (values) => {
    const result = await onSubmit(values as z.output<S>)
    if (result?.fieldErrors) {
      for (const [name, messages] of Object.entries(result.fieldErrors)) {
        if (messages[0]) form.setError(name, { message: messages[0] })
      }
    }
    if (result?.formError) form.setError("root", { message: result.formError })
    if (!result && resetOnSuccess) form.reset(defaultValues as FieldValues)
  })

  const rootError = form.formState.errors.root?.message
  const pending = form.formState.isSubmitting

  return (
    <form onSubmit={submit} noValidate className="flex flex-col gap-6">
      <FieldGroup>
        {fields.map((field) => (
          <FieldWidget key={field.name} field={field} control={form.control} />
        ))}
      </FieldGroup>
      {rootError ? (
        <p role="alert" className="text-sm text-destructive">
          {rootError}
        </p>
      ) : null}
      <div>
        <Button
          type="submit"
          disabled={pending}
          className="h-11 rounded-xl px-5 font-semibold"
        >
          {pending ? <Spinner data-icon="inline-start" /> : null}
          {submitLabel}
        </Button>
      </div>
    </form>
  )
}
```

Run: `pnpm vitest run --project unit admin/components/schema-form`
Expected: PASS (8 tests).

- [ ] **Step 5: Write the failing Account helper test**

```ts
// admin/modules/account/describe-agent.test.ts
import { expect, it } from "vitest"

import { describeAgent } from "./describe-agent"

it.each([
  ["Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0 Safari/537.36", "Chrome on macOS"],
  ["Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Mobile/15E148 Safari/604.1", "Safari on iOS"],
  ["Mozilla/5.0 (Linux; Android 15; Pixel 7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0 Mobile Safari/537.36", "Chrome on Android"],
  ["Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0 Safari/537.36 Edg/131.0", "Edge on Windows"],
  ["Mozilla/5.0 (X11; Linux x86_64; rv:133.0) Gecko/20100101 Firefox/133.0", "Firefox on Linux"],
  [null, "Unknown device"],
])("%s → %s", (agent, expected) => {
  expect(describeAgent(agent)).toBe(expected)
})
```

Run: `pnpm vitest run --project unit admin/modules/account`
Expected: FAIL with unresolved `./describe-agent`.

- [ ] **Step 6: Implement the Account page**

```ts
// admin/modules/account/describe-agent.ts
const BROWSERS: [RegExp, string][] = [
  [/Edg\//, "Edge"],
  [/Firefox\//, "Firefox"],
  [/Chrome\//, "Chrome"],
  [/Safari\//, "Safari"],
]
// Order matters: iOS and Android user agents also mention Mac and Linux.
const SYSTEMS: [RegExp, string][] = [
  [/iPhone|iPad/, "iOS"],
  [/Android/, "Android"],
  [/Windows/, "Windows"],
  [/Mac OS X|Macintosh/, "macOS"],
  [/Linux/, "Linux"],
]

// "Chrome on macOS" for the sessions list. Enough to recognise a device,
// not a fingerprint.
export function describeAgent(agent: string | null | undefined) {
  if (!agent) return "Unknown device"
  const browser = BROWSERS.find(([pattern]) => pattern.test(agent))?.[1]
  const system = SYSTEMS.find(([pattern]) => pattern.test(agent))?.[1]
  if (browser && system) return `${browser} on ${system}`
  return browser ?? system ?? "Unknown device"
}
```

```ts
// admin/modules/account/schemas.ts
import * as z from "zod"

import { ui } from "@/admin/components/schema-form/registry"

export const ProfileSchema = z.object({
  name: z
    .string()
    .trim()
    .min(1, "Enter your name")
    .max(80, "Use 80 characters or fewer")
    .register(ui, { label: "Full name", autoComplete: "name" }),
})

// Better Auth enforces 12 characters too (docs/brief.md §7.1).
export const PasswordSchema = z
  .object({
    currentPassword: z
      .string()
      .min(1, "Enter your current password")
      .register(ui, {
        label: "Current password",
        inputType: "password",
        autoComplete: "current-password",
      }),
    newPassword: z
      .string()
      .min(12, "Use at least 12 characters")
      .max(128, "Use 128 characters or fewer")
      .register(ui, {
        label: "New password",
        inputType: "password",
        autoComplete: "new-password",
        description: "At least 12 characters. A short sentence works well.",
      }),
    confirmPassword: z.string().register(ui, {
      label: "Confirm new password",
      inputType: "password",
      autoComplete: "new-password",
    }),
    revokeOtherSessions: z
      .boolean()
      .default(true)
      .register(ui, { label: "Sign out of my other devices" }),
  })
  .refine((value) => value.newPassword === value.confirmPassword, {
    path: ["confirmPassword"],
    message: "The passwords don't match",
  })
```

```tsx
// admin/modules/account/profile-form.tsx
"use client"

import { useRouter } from "next/navigation"
import { toast } from "sonner"

import { SchemaForm } from "@/admin/components/schema-form/schema-form"
import { useActor } from "@/admin/lib/actor-context"
import { authClient } from "@/admin/lib/auth-client"
import { authRequest } from "@/admin/modules/auth/auth-errors"

import { ProfileSchema } from "./schemas"

export function ProfileForm() {
  const { user } = useActor()
  const router = useRouter()
  return (
    <SchemaForm
      schema={ProfileSchema}
      defaultValues={{ name: user.name }}
      submitLabel="Save profile"
      onSubmit={async ({ name }) => {
        const { failure } = await authRequest(() => authClient.updateUser({ name }))
        if (failure) {
          return { formError: failure.message ?? "Couldn't save your profile. Try again." }
        }
        toast.success("Profile saved")
        // The shell's name and avatar come from the server layout.
        router.refresh()
      }}
    />
  )
}
```

```tsx
// admin/modules/account/password-form.tsx
"use client"

import { toast } from "sonner"

import { SchemaForm } from "@/admin/components/schema-form/schema-form"
import { authClient } from "@/admin/lib/auth-client"
import { authRequest } from "@/admin/modules/auth/auth-errors"

import { PasswordSchema } from "./schemas"

export function PasswordForm() {
  return (
    <SchemaForm
      schema={PasswordSchema}
      defaultValues={{
        currentPassword: "",
        newPassword: "",
        confirmPassword: "",
        revokeOtherSessions: true,
      }}
      submitLabel="Change password"
      resetOnSuccess
      onSubmit={async ({ currentPassword, newPassword, revokeOtherSessions }) => {
        const { failure } = await authRequest(() =>
          authClient.changePassword({ currentPassword, newPassword, revokeOtherSessions })
        )
        if (failure?.kind === "rejected") {
          return { fieldErrors: { currentPassword: ["That password isn't right."] } }
        }
        if (failure) return { formError: failure.message }
        toast.success("Password changed")
      }}
    />
  )
}
```

```tsx
// admin/modules/account/sessions-list.tsx
"use client"

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"
import { MonitorSmartphoneIcon } from "lucide-react"
import { toast } from "sonner"

import { relativeTime } from "@/admin/components/dashboard/format"
import { CardSkeleton } from "@/admin/components/dashboard/skeletons"
import { StatusPill } from "@/admin/components/dashboard/status-pill"
import { authClient } from "@/admin/lib/auth-client"
import { queryKeys } from "@/admin/lib/query-keys"
import { Button } from "@/components/ui/button"

import { describeAgent } from "./describe-agent"

// Better Auth's client resolves to { data, error } instead of throwing;
// TanStack Query needs a thrown error to show its error state.
async function unwrap<T>(
  call: Promise<
    { data: T; error: null } | { data: null; error: { message?: string } }
  >,
  fallback: string
): Promise<T> {
  const result = await call
  if (result.error) throw new Error(result.error.message ?? fallback)
  return result.data as T
}

// docs/brief.md §7.4: users list and revoke their own sessions.
export function SessionsList() {
  const queryClient = useQueryClient()
  const { data: current } = authClient.useSession()
  const sessions = useQuery({
    queryKey: queryKeys.account.sessions,
    queryFn: () => unwrap(authClient.listSessions(), "Couldn't load your devices"),
  })
  const refresh = () =>
    queryClient.invalidateQueries({ queryKey: queryKeys.account.sessions })
  const revoke = useMutation({
    mutationFn: (token: string) =>
      unwrap(authClient.revokeSession({ token }), "Couldn't sign that device out"),
    onSuccess: () => {
      toast.success("Signed out of that device")
      return refresh()
    },
    onError: (error) => toast.error(error.message),
  })
  const revokeOthers = useMutation({
    mutationFn: () =>
      unwrap(authClient.revokeOtherSessions(), "Couldn't sign the other devices out"),
    onSuccess: () => {
      toast.success("Signed out of your other devices")
      return refresh()
    },
    onError: (error) => toast.error(error.message),
  })

  if (sessions.isPending) return <CardSkeleton className="h-40" />
  if (sessions.isError) {
    return (
      <p role="alert" className="text-sm text-destructive">
        {sessions.error.message}
      </p>
    )
  }

  const now = Date.now()
  const list = [...(sessions.data ?? [])].sort(
    (a, b) => new Date(b.updatedAt).getTime() - new Date(a.updatedAt).getTime()
  )
  return (
    <div className="flex flex-col gap-4">
      <ul aria-label="Signed-in devices" className="flex flex-col divide-y">
        {list.map((session) => {
          const isCurrent = session.token === current?.session.token
          return (
            <li key={session.id} className="flex flex-wrap items-center gap-3 py-3">
              <MonitorSmartphoneIcon aria-hidden className="size-5 text-muted-foreground" />
              <div className="flex min-w-0 flex-1 flex-col">
                <span className="font-semibold">{describeAgent(session.userAgent)}</span>
                <span className="text-xs text-muted-foreground">
                  Last active{" "}
                  {relativeTime(new Date(session.updatedAt).toISOString(), now)}
                </span>
              </div>
              {isCurrent ? (
                <StatusPill tone="success">This device</StatusPill>
              ) : (
                <Button
                  variant="outline"
                  disabled={revoke.isPending}
                  onClick={() => revoke.mutate(session.token)}
                >
                  Sign out
                  <span className="sr-only"> {describeAgent(session.userAgent)}</span>
                </Button>
              )}
            </li>
          )
        })}
      </ul>
      {list.length > 1 ? (
        <div>
          <Button
            variant="outline"
            disabled={revokeOthers.isPending}
            onClick={() => revokeOthers.mutate()}
          >
            Sign out other devices
          </Button>
        </div>
      ) : null}
    </div>
  )
}
```

`relativeTime` uses `Date.now()` here only because sessions are fetched in the browser (no server render to mismatch).

```tsx
// admin/modules/account/theme-preference.tsx
"use client"

import { MonitorIcon, MoonIcon, SunIcon } from "lucide-react"
import { useTheme } from "next-themes"
import { useSyncExternalStore } from "react"

import { Skeleton } from "@/components/ui/skeleton"
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group"

const subscribe = () => () => {}

// The saved choice only exists in the browser; render after mount so the
// server and client never disagree about which item is pressed.
export function ThemePreference() {
  const mounted = useSyncExternalStore(subscribe, () => true, () => false)
  const { theme, setTheme } = useTheme()
  if (!mounted) return <Skeleton className="h-9 w-64" />
  return (
    <ToggleGroup
      aria-label="Theme"
      value={[theme ?? "system"]}
      onValueChange={(value) => {
        if (value[0]) setTheme(String(value[0]))
      }}
      variant="outline"
    >
      <ToggleGroupItem value="light">
        <SunIcon aria-hidden /> Light
      </ToggleGroupItem>
      <ToggleGroupItem value="dark">
        <MoonIcon aria-hidden /> Dark
      </ToggleGroupItem>
      <ToggleGroupItem value="system">
        <MonitorIcon aria-hidden /> System
      </ToggleGroupItem>
    </ToggleGroup>
  )
}
```

```tsx
// admin/modules/account/account-view.tsx
"use client"

import { ShieldCheckIcon } from "lucide-react"
import Link from "next/link"
import { parseAsStringLiteral, useQueryState } from "nuqs"

import { DashboardCard } from "@/admin/components/dashboard/dashboard-card"
import { PageHeader } from "@/admin/components/dashboard/page-header"
import { StatusPill } from "@/admin/components/dashboard/status-pill"
import { useActor } from "@/admin/lib/actor-context"
import { Button } from "@/components/ui/button"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"

import { PasswordForm } from "./password-form"
import { ProfileForm } from "./profile-form"
import { SessionsList } from "./sessions-list"
import { ThemePreference } from "./theme-preference"

const TABS = ["profile", "security", "sessions"] as const

export function AccountView() {
  const { user, twoFactorEnabled } = useActor()
  const [tab, setTab] = useQueryState(
    "tab",
    parseAsStringLiteral(TABS).withDefault("profile")
  )
  return (
    <div className="flex flex-col gap-6">
      <PageHeader title="Account" description={user.email} />
      <Tabs value={tab} onValueChange={(value) => void setTab(value as (typeof TABS)[number])}>
        <TabsList>
          <TabsTrigger value="profile">Profile</TabsTrigger>
          <TabsTrigger value="security">Security</TabsTrigger>
          <TabsTrigger value="sessions">Devices</TabsTrigger>
        </TabsList>
        <TabsContent value="profile" className="mt-4 grid gap-4 xl:grid-cols-2">
          <DashboardCard title="Profile">
            <ProfileForm />
          </DashboardCard>
          <DashboardCard title="Appearance">
            <ThemePreference />
          </DashboardCard>
        </TabsContent>
        <TabsContent value="security" className="mt-4 grid gap-4 xl:grid-cols-2">
          <DashboardCard title="Password">
            <PasswordForm />
          </DashboardCard>
          <DashboardCard title="Two-factor authentication">
            <div className="flex flex-col items-start gap-4">
              {twoFactorEnabled ? (
                <StatusPill tone="success">On</StatusPill>
              ) : (
                <StatusPill tone="warning">Off</StatusPill>
              )}
              <p className="text-sm text-muted-foreground">
                A code from an authenticator app is asked for when you sign in
                on a new device.
              </p>
              {twoFactorEnabled ? null : (
                <Button
                  nativeButton={false}
                  render={<Link href="/admin/two-factor-setup" />}
                  className="h-11 rounded-xl px-5 font-semibold"
                >
                  <ShieldCheckIcon data-icon="inline-start" />
                  Set up two-factor
                </Button>
              )}
            </div>
          </DashboardCard>
        </TabsContent>
        <TabsContent value="sessions" className="mt-4">
          <DashboardCard title="Signed-in devices">
            <SessionsList />
          </DashboardCard>
        </TabsContent>
      </Tabs>
    </div>
  )
}
```

```tsx
// app/(admin)/admin/(panel)/account/page.tsx
import type { Metadata } from "next"
import { Suspense } from "react"

import { CardSkeleton } from "@/admin/components/dashboard/skeletons"
import { AccountView } from "@/admin/modules/account/account-view"
import { requireActor } from "@/server/auth/session"

export const metadata: Metadata = { title: "Account" }

export default function AccountPage() {
  return (
    <Suspense fallback={<CardSkeleton />}>
      <Account />
    </Suspense>
  )
}

// Every user manages their own profile, password and sessions
// (docs/brief.md §7.2), so signing in is the only requirement.
async function Account() {
  await requireActor()
  return <AccountView />
}
```

- [ ] **Step 7: Show every widget in the design showcase**

```tsx
// admin/modules/design/demo-form.tsx
"use client"

import { toast } from "sonner"
import * as z from "zod"

import { ui } from "@/admin/components/schema-form/registry"
import { SchemaForm } from "@/admin/components/schema-form/schema-form"

// One field per widget, for reviews and visual tests.
const DemoSchema = z.object({
  title: z.string().trim().min(1, "Add a title").max(60).register(ui, { label: "Title", placeholder: "Finding calm at work" }),
  excerpt: z.string().max(200).register(ui, { label: "Excerpt", widget: "textarea", description: "Shown on cards and in search results." }),
  readingTime: z.number().int().min(1).max(60).register(ui, { label: "Reading time (minutes)" }),
  featured: z.boolean().default(false).register(ui, { label: "Feature on the home page" }),
  category: z.enum(["stress", "sleep", "work"]).register(ui, { label: "Category" }),
  format: z.enum(["online", "in-person"]).register(ui, {
    label: "Session format",
    widget: "radio",
    options: [
      { value: "online", label: "Online" },
      { value: "in-person", label: "In person" },
    ],
  }),
  keyPoints: z.array(z.string().max(80)).min(1).max(4).register(ui, { label: "Key points" }),
  cta: z
    .object({
      label: z.string().max(30).register(ui, { label: "Button label" }),
      href: z.string().max(200).register(ui, { label: "Link", inputType: "url" }),
    })
    .register(ui, { label: "Call to action" }),
})

export function DemoForm() {
  return (
    <SchemaForm
      schema={DemoSchema}
      defaultValues={{
        title: "",
        excerpt: "",
        readingTime: 5,
        featured: false,
        category: "stress",
        format: "online",
        keyPoints: ["Notice the first signs"],
        cta: { label: "Book a session", href: "/contact" },
      }}
      submitLabel="Validate"
      onSubmit={() => {
        toast.success("Looks good")
      }}
    />
  )
}
```

In `admin/modules/design/showcase.tsx`, import `DemoForm` from `./demo-form` and add this section after the "Status pills" section:

```tsx
      <section aria-labelledby="sample-form" className="flex max-w-2xl flex-col gap-4">
        <h2 id="sample-form" className="text-card-title">
          Schema form
        </h2>
        <DemoForm />
      </section>
```

- [ ] **Step 8: Write the Account E2E (desktop only: it changes an account)**

```ts
// tests/e2e/admin/auth/account.spec.ts
import { expect, test } from "@playwright/test"

import { E2E_USERS, signIn } from "../../support/admin"
import { STORAGE } from "../../support/storage"

test.use({ storageState: STORAGE.account })
// Both tests change the same account.
test.describe.configure({ mode: "serial" })

test("changes the display name and the shell follows", async ({ page }) => {
  await page.goto("/admin/account")
  const name = page.getByLabel("Full name")
  await name.fill("Andy Renamed")
  await page.getByRole("button", { name: "Save profile" }).click()
  await expect(page.getByText("Profile saved")).toBeVisible()
  await expect(
    page.getByRole("button", { name: "Account menu for Andy Renamed" })
  ).toBeVisible()

  // Put it back so a rerun starts from the seed.
  await name.fill(E2E_USERS.accountUser.name)
  await page.getByRole("button", { name: "Save profile" }).click()
  await expect(
    page.getByRole("button", { name: `Account menu for ${E2E_USERS.accountUser.name}` })
  ).toBeVisible()
})

test("refuses a mismatched new password before calling the server", async ({
  page,
}) => {
  await page.goto("/admin/account?tab=security")
  await page.getByLabel("Current password").fill("anything-at-all")
  await page.getByLabel("New password", { exact: true }).fill("a-long-new-password")
  await page.getByLabel("Confirm new password").fill("a-different-password")
  await page.getByRole("button", { name: "Change password" }).click()
  await expect(page.getByText("The passwords don't match")).toBeVisible()
})

test("signs another device out", async ({ page, browser }) => {
  const other = await browser.newContext({ storageState: { cookies: [], origins: [] } })
  const otherPage = await other.newPage()
  await otherPage.goto("/admin/sign-in")
  await signIn(otherPage, E2E_USERS.accountUser)
  await expect(otherPage).toHaveURL(/\/admin$/)

  await page.goto("/admin/account?tab=sessions")
  const devices = page.getByRole("list", { name: "Signed-in devices" })
  await expect(devices.getByText("This device")).toBeVisible()
  await page.getByRole("button", { name: "Sign out other devices" }).click()
  await expect(page.getByText("Signed out of your other devices")).toBeVisible()
  await expect(devices.getByRole("listitem")).toHaveCount(1)

  expect((await otherPage.request.get("/api/v1/admin/me")).status()).toBe(401)
  await other.close()
})
```

Run: `pnpm test:e2e tests/e2e/admin/auth/account.spec.ts tests/e2e/admin/shell/design.spec.ts`
Expected: PASS (account on desktop only; design on all three).

- [ ] **Step 9: Gates**

Run: `pnpm lint && pnpm typecheck && pnpm test:unit && pnpm build`
Expected: all green; `guarded-pages.test.ts` now also covers `account/page.tsx`.

- [ ] **Step 10: Commit**

```bash
git add components/ui admin "app/(admin)/admin/(panel)/account" tests/e2e/admin/auth/account.spec.ts package.json pnpm-lock.yaml
git commit -m "feat(admin): schema-driven forms and an Account page for profile, password and devices"
```

---

## Task 2.11: Visual QA, accessibility, reduced motion and Lighthouse

**Files:**
- Modify: `package.json` (devDependencies `@axe-core/playwright@4.13.0`, `lighthouse@13.5.0`)
- Modify: `app/(admin)/admin.css` (reduced-motion rule)
- Create: `tests/e2e/fixtures/dashboard.ts`
- Create: `tests/e2e/admin/visual/shell.spec.ts` (+ baselines under `tests/e2e/__screenshots__/darwin/{desktop,tablet,mobile}/`)
- Create: `tests/e2e/admin/a11y/{axe,keyboard,motion,lighthouse}.spec.ts`

**Interfaces:**
- Consumes: everything above; `DashboardDto` (2.8, type only); `STORAGE` (2.5); `data-testid="greeting"` (2.8).
- Produces: per-platform screenshot baselines `dashboard-{light,dark}.png` and `design-{light,dark}.png` for each viewport; the a11y gate the acceptance criteria name (axe: no serious/critical; Lighthouse accessibility ≥ 95 on `/admin`).

- [ ] **Step 1: Install the tools**

Run: `pnpm add -D @axe-core/playwright@4.13.0 lighthouse@13.5.0`
Expected: two devDependencies added (Lighthouse 13.5 needs Node ≥ 22.19).

- [ ] **Step 2: Write the reduced-motion test and watch it fail**

```ts
// tests/e2e/admin/a11y/motion.spec.ts
import { expect, test } from "@playwright/test"

import { STORAGE } from "../../support/storage"

test.use({ storageState: STORAGE.editor, reducedMotion: "reduce" })

// docs/brief.md §9.2: motion is restrained and reduced-motion is respected.
test("a reduced-motion preference switches transitions off", async ({ page }) => {
  await page.goto("/admin")
  const pill = page.getByRole("button", { name: "Search", exact: true })
  const duration = await pill.evaluate(
    (element) => getComputedStyle(element).transitionDuration
  )
  expect(parseFloat(duration)).toBeLessThan(0.001)
})
```

Run: `pnpm test:e2e tests/e2e/admin/a11y/motion.spec.ts`
Expected: FAIL — `expected 0.15 to be less than 0.001`.

- [ ] **Step 3: Add the rule to `app/(admin)/admin.css`**

Append after the `@layer base { … }` block:

```css
/* docs/brief.md §9.2: restrained motion, and none for people who ask for
   less. Charts read the same preference and skip their grow-in. */
@media (prefers-reduced-motion: reduce) {
  :root[data-theme="admin"] *,
  :root[data-theme="admin"] *::before,
  :root[data-theme="admin"] *::after {
    animation-duration: 0.01ms !important;
    animation-iteration-count: 1 !important;
    transition-duration: 0.01ms !important;
    scroll-behavior: auto !important;
  }
}
```

Run: `pnpm test:e2e tests/e2e/admin/a11y/motion.spec.ts`
Expected: PASS on all three projects.

- [ ] **Step 4: Write the axe, keyboard and Lighthouse checks**

```ts
// tests/e2e/admin/a11y/axe.spec.ts
import AxeBuilder from "@axe-core/playwright"
import { expect, test, type Page } from "@playwright/test"

import { STORAGE } from "../../support/storage"

const TAGS = ["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22aa"]

// Acceptance: axe reports no serious or critical violations.
async function blocking(page: Page) {
  await page.waitForLoadState("networkidle", { timeout: 5_000 }).catch(() => {})
  const { violations } = await new AxeBuilder({ page }).withTags(TAGS).analyze()
  return violations
    .filter((v) => v.impact === "serious" || v.impact === "critical")
    .map((v) => `${v.id}: ${v.nodes.map((node) => node.target.join(" ")).join(", ")}`)
}

const PANEL = [
  "/admin",
  "/admin/design",
  "/admin/activity",
  "/admin/account",
  "/admin/account?tab=security",
  "/admin/account?tab=sessions",
  "/admin/pages",
  "/admin/no-access?from=%2Fadmin%2Fpages",
]

test.describe("signed in as an owner", () => {
  test.use({ storageState: STORAGE.owner })

  for (const colorScheme of ["light", "dark"] as const) {
    for (const path of PANEL) {
      test(`${path} in ${colorScheme}`, async ({ page }) => {
        await page.emulateMedia({ colorScheme })
        await page.goto(path)
        expect(await blocking(page)).toEqual([])
      })
    }
  }

  test("with the ⌘K palette open", async ({ page }) => {
    await page.goto("/admin")
    await page.keyboard.press("ControlOrMeta+k")
    await expect(page.getByRole("dialog", { name: "Search the admin" })).toBeVisible()
    expect(await blocking(page)).toEqual([])
  })

  test("with the user menu open", async ({ page }) => {
    await page.goto("/admin")
    await page.getByRole("button", { name: /^Account menu for / }).click()
    await expect(page.getByRole("menu")).toBeVisible()
    expect(await blocking(page)).toEqual([])
  })
})

test.describe("signed out", () => {
  test.use({ storageState: { cookies: [], origins: [] } })

  for (const path of ["/admin/sign-in", "/admin/forgot-password"]) {
    test(path, async ({ page }) => {
      await page.goto(path)
      expect(await blocking(page)).toEqual([])
    })
  }
})
```

```ts
// tests/e2e/admin/a11y/keyboard.spec.ts
import { expect, test } from "@playwright/test"

import { STORAGE } from "../../support/storage"

test.use({ storageState: STORAGE.editor })

test("the first Tab reaches a visible skip link that jumps to the content", async ({
  page,
}) => {
  await page.goto("/admin")
  await page.keyboard.press("Tab")
  const skip = page.getByRole("link", { name: "Skip to content" })
  await expect(skip).toBeFocused()
  await expect(skip).toBeVisible()
  await page.keyboard.press("Enter")
  await expect(page.locator("#main")).toBeFocused()
})

// Acceptance: full keyboard navigation, with visible focus everywhere.
test("every control in the shell is reachable and shows focus", async ({
  page,
}) => {
  test.skip(test.info().project.name !== "desktop", "the sheet has its own test")
  await page.goto("/admin")
  const reached: string[] = []
  for (let step = 0; step < 40; step++) {
    await page.keyboard.press("Tab")
    const focused = await page.evaluate(() => {
      const element = document.activeElement as HTMLElement | null
      if (!element || element === document.body) return null
      const style = getComputedStyle(element)
      return {
        name: (element.getAttribute("aria-label") ?? element.textContent ?? "").trim(),
        visible:
          style.boxShadow !== "none" ||
          (style.outlineStyle !== "none" && style.outlineWidth !== "0px"),
      }
    })
    if (!focused) break
    expect(focused.visible, `${focused.name} shows a focus indicator`).toBe(true)
    reached.push(focused.name)
  }
  expect(reached).toEqual(
    expect.arrayContaining([
      "Skip to content",
      "Magda Kennedy admin home",
      "Dashboard",
      "Pages",
      "Log out",
      "Toggle navigation",
      "Search",
      "Notifications",
      "Account menu for Eddie Editor",
    ])
  )
})

test("the user menu works from the keyboard", async ({ page }) => {
  await page.goto("/admin")
  const trigger = page.getByRole("button", { name: /^Account menu for / })
  await trigger.focus()
  await page.keyboard.press("Enter")
  await expect(page.getByRole("menu")).toBeVisible()
  await expect(page.locator('[role="menuitem"]:focus, [role="menuitemradio"]:focus')).toHaveCount(1)
  await page.keyboard.press("Escape")
  await expect(page.getByRole("menu")).toBeHidden()
  await expect(trigger).toBeFocused()
})
```

```ts
// tests/e2e/admin/a11y/lighthouse.spec.ts
import { mkdtempSync, readFileSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import { chromium, expect, test } from "@playwright/test"
import lighthouse from "lighthouse"

import { E2E_ORIGIN } from "../../fixtures/env"
import { STORAGE } from "../../support/storage"

const PORT = 9333

// Acceptance: Lighthouse accessibility ≥ 95 on /admin. Lighthouse drives
// its own tab, so it runs in a persistent context that holds the owner's
// cookies and exposes a debugging port.
test("Lighthouse accessibility is at least 95 on /admin", async () => {
  test.skip(test.info().project.name !== "desktop", "one run is enough")
  test.setTimeout(120_000)
  const context = await chromium.launchPersistentContext(
    mkdtempSync(path.join(tmpdir(), "mk-lighthouse-")),
    { args: [`--remote-debugging-port=${PORT}`] }
  )
  try {
    const { cookies } = JSON.parse(readFileSync(STORAGE.owner, "utf8")) as {
      cookies: Parameters<typeof context.addCookies>[0]
    }
    await context.addCookies(cookies)
    const result = await lighthouse(`${E2E_ORIGIN}/admin`, {
      port: PORT,
      onlyCategories: ["accessibility"],
      disableStorageReset: true,
      logLevel: "error",
      output: "json",
    })
    const score = Math.round((result?.lhr.categories.accessibility.score ?? 0) * 100)
    expect(score, "Lighthouse accessibility score").toBeGreaterThanOrEqual(95)
  } finally {
    await context.close()
  }
})
```

Run: `pnpm test:e2e tests/e2e/admin/a11y`
Expected: PASS. Fix any axe finding at its source (component or token), never by excluding the rule. Typical first-run findings and their fixes: a nested interactive element inside a `SidebarMenuButton` (move it out), an icon-only button without `aria-label` (add one), a `role="list"` without `listitem` children (pass `role="listitem"`).

- [ ] **Step 5: Write the visual suite**

```ts
// tests/e2e/fixtures/dashboard.ts
import type { DashboardDto } from "@/server/modules/dashboard/service"

// Fixed dashboard data for screenshots. Typed against the API's DTO so a
// change to the endpoint breaks this file, not a silent screenshot.
export const DASHBOARD_FIXTURE = {
  generatedAt: "2026-10-09T10:00:00.000Z",
  activeUsers: { value: 6, previous: 5 },
  signIns: {
    days: [
      { key: "2026-10-05", label: "Mon", current: 4, previous: 3 },
      { key: "2026-10-06", label: "Tue", current: 6, previous: 5 },
      { key: "2026-10-07", label: "Wed", current: 5, previous: 7 },
      { key: "2026-10-08", label: "Thu", current: 9, previous: 6 },
      { key: "2026-10-09", label: "Fri", current: 3, previous: 8 },
      { key: "2026-10-10", label: "Sat", current: 0, previous: 2 },
      { key: "2026-10-11", label: "Sun", current: 0, previous: 1 },
    ],
    current: 27,
    previous: 32,
  },
  security: { protected: 4, requiredMissing: 1, optionalMissing: 2, total: 7 },
  changes: { value: 18, previous: 12 },
  recentActivity: [
    {
      id: "a1",
      actorName: "Niamh Walsh",
      actorEmail: "niamh@example.com",
      action: "auth.sign_in",
      summary: "Signed in",
      createdAt: "2026-10-09T09:40:00.000Z",
    },
    {
      id: "a2",
      actorName: "Ciarán Doyle",
      actorEmail: "ciaran@example.com",
      action: "user.invite",
      summary: "Invited aoife@example.com as editor",
      createdAt: "2026-10-09T08:15:00.000Z",
    },
    {
      id: "a3",
      actorName: "Aoife Byrne",
      actorEmail: "aoife@example.com",
      action: "auth.sign_in",
      summary: "Signed in",
      createdAt: "2026-10-08T16:05:00.000Z",
    },
    {
      id: "a4",
      actorName: null,
      actorEmail: null,
      action: "user.bootstrap-owner",
      summary: "Created the first owner",
      createdAt: "2026-10-01T09:00:00.000Z",
    },
  ],
  nextUp: [
    {
      id: "two-factor",
      title: "Two-factor is on",
      description:
        "A code from your phone keeps your account safe even if your password leaks.",
      cta: "Set up two-factor",
      href: "/admin/two-factor-setup",
      done: true,
    },
    {
      id: "invite",
      title: "Invite your team",
      description: "Give each person their own account and only the role they need.",
      cta: "Invite teammates",
      href: "/admin/users",
      done: false,
    },
  ],
} satisfies DashboardDto
```

```ts
// tests/e2e/admin/visual/shell.spec.ts
import { expect, test } from "@playwright/test"

import { DASHBOARD_FIXTURE } from "../../fixtures/dashboard"
import { STORAGE } from "../../support/storage"

// Shell and dashboard in light and dark, on every viewport (docs/plan.md
// Phase 2, item 10). Charts skip their animation under reduced motion, so
// the capture is stable; the greeting depends on the hour and is masked.
test.use({ storageState: STORAGE.owner, reducedMotion: "reduce" })

for (const colorScheme of ["light", "dark"] as const) {
  test.describe(`${colorScheme} theme`, () => {
    test.use({ colorScheme })

    test("dashboard", async ({ page }) => {
      await page.route("**/api/v1/admin/dashboard", (route) =>
        route.fulfill({ json: DASHBOARD_FIXTURE })
      )
      await page.goto("/admin")
      await expect(
        page.getByRole("region", { name: "Team activity" }).getByRole("listitem")
      ).toHaveCount(4)
      await page.evaluate(() => document.fonts.ready)
      await expect(page).toHaveScreenshot(`dashboard-${colorScheme}.png`, {
        fullPage: true,
        mask: [page.getByTestId("greeting")],
      })
    })

    test("design system", async ({ page }) => {
      await page.goto("/admin/design")
      await expect(page.locator(".recharts-surface").first()).toBeVisible()
      await page.evaluate(() => document.fonts.ready)
      await expect(page).toHaveScreenshot(`design-${colorScheme}.png`, {
        fullPage: true,
      })
    })
  })
}
```

Run: `pnpm test:e2e tests/e2e/admin/visual`
Expected: FAIL — `A snapshot doesn't exist at …/dashboard-light.png, writing actual.` for each of the 12 captures.

- [ ] **Step 6: Record and prove the baselines**

Run: `pnpm test:e2e tests/e2e/admin/visual --update-snapshots`
Expected: 12 PNGs under `tests/e2e/__screenshots__/darwin/{desktop,tablet,mobile}/`.

Run: `pnpm test:e2e tests/e2e/admin/visual && pnpm test:e2e tests/e2e/admin/visual`
Expected: PASS twice in a row (the captures are stable). Linux baselines for CI are generated in the Playwright Docker image in Phase 11.

- [ ] **Step 7: Side-by-side review against the inspiration (acceptance)**

Open `docs/design/admin-inspiration.png` next to `tests/e2e/__screenshots__/darwin/desktop/design-light.png` and `dashboard-light.png`, and check each mapping:

| Inspiration | Ours |
| --- | --- |
| Sidebar: logo, MENU / GENERAL, active green bar + bold label, "12+" pill | BrandMark, Menu / Growth / General, 4 px bar + filled icon, Enquiries badge slot |
| Dark "Download our Mobile App" card | "Your website" promo card (swirl, Open site) |
| Top bar: search pill with filter icon, mail, bell, avatar block | Search pill with ⌘K, Enquiries mail button, notifications bell, user menu |
| "Dashboard" + Add Project / Import data | Greeting + Invite teammate / View site (button pair) |
| Total / Ended / Running / Pending projects (first card dark green) | Active users (hero) / Sign-ins / Security health / Changes |
| Project Analytics hatched pill bars with "75%" tag | Sign-ins this week, hatched = last week, value tag on the busiest day |
| Reminders card with Start Meeting | Next up card with its CTA and checklist |
| Team Collaboration with status pills | Team activity with status pills |
| Project Progress gauge 41% + legend | Security health gauge + legend (Content health on the showcase) |

Tune spacing, radii and colours in the components (not in the screenshots), re-record with `--update-snapshots`, then ask the client to sign off. Record "Phase 2 visual sign-off: <date>, <who>" in `.superpowers/sdd/plan/progress.md`.

- [ ] **Step 8: Full gates**

Run: `pnpm lint && pnpm typecheck && pnpm test:unit && pnpm test:integration && pnpm build && pnpm test:e2e && pnpm test:parity`
Expected: all green; parity unchanged at ≤ 0.1%.

- [ ] **Step 9: Commit**

```bash
git add "app/(admin)/admin.css" tests/e2e package.json pnpm-lock.yaml
git commit -m "test(admin): visual baselines, axe, keyboard, reduced-motion and Lighthouse checks"
```

---

## Spec coverage map

Phase 2 contract (`docs/plan.md` "Phase 2: Admin shell and design system") and the carry-overs owed to this phase:

| Requirement | Task |
| --- | --- |
| 1. Complete `admin.css` light and dark tokens | 2.1 |
| 1. Hatch utility; `--success`/`--warning`/`--danger` + `-soft` | 2.1 |
| 1. `@source not`: site ignores `admin/**`, admin ignores `features/**` | 2.1 |
| 1. next-themes provider in the admin (system default); remove `ThemeHotkey` | 2.2 |
| 2. shadcn base-nova components: sidebar, sheet, skeleton, tooltip, avatar, badge, breadcrumb, dropdown-menu, kbd, popover, scroll-area | 2.5 |
| 2. command, dialog, input-group | 2.6 |
| 2. chart, empty, item | 2.7 |
| 2. table, select, button-group | 2.9 |
| 2. switch, tabs, textarea, toggle-group, radio-group, spinner | 2.10 |
| 3. Permission-filtered sidebar (§9.1), active-bar indicator, badge slot, "Your website" card, mobile sheet | 2.3 (registry), 2.5 (UI) |
| 3. Persisted sidebar state | 2.5 |
| 3. Topbar: ⌘K search pill, icon buttons, user menu with theme switch and sign-out | 2.5, 2.6 (pill) |
| 3. `AdminActorProvider` from the guarded layout's `/me` payload, `usePermission()` | 2.3 |
| 3. `requirePermission(permissions)` in the DAL with a "no access" screen (§7.3) — carry-over (Ruling F8b) | 2.3 |
| 4. ⌘K: navigation + actions registry, `GET /api/v1/admin/search` | 2.6 |
| 5. TanStack Query provider, `getQueryClient`, `queryClient.query()` prefetch | 2.4 (provider), 2.9 (prefetch) |
| 5. Typed `hc` clients per sub-app; query-key factory; `parseResponse` → `ApiError` | 2.4 (+ 2.6, 2.8 add clients) |
| 5. nuqs-backed table state | 2.9 |
| §5.4 layering lint rule (`docs/plan.md` coverage map: "lint rule added in 2.5") | 2.4 |
| 6. PageHeader, KpiCard (hero + default), PillBarChart, Gauge, StatusPill, ActivityFeed, EmptyState, skeletons | 2.7 |
| 6. Owner-only `/admin/design` showcase | 2.7 (+ 2.10 forms) |
| 7. `GET /api/v1/admin/dashboard`: active users, sign-ins this vs last week, 2FA coverage gauge, recent activity, Next up | 2.8 |
| 8. DataTable (TanStack Table v9 `useTable` + `tableFeatures`, nuqs URL state); `/admin/activity` with actor, action, date filters | 2.9 |
| 9. SchemaForm: Zod UI registry → `z.toJSONSchema(…, { io: "input", override })` → widgets; `zodResolver`; Account page (profile, password, sessions with revoke) | 2.10 |
| 10. Playwright screenshots (light/dark × 3 viewports) + axe | 2.11 |
| Acceptance: side-by-side review signed off | 2.11 Step 7 |
| Acceptance: full keyboard navigation | 2.5 (sheet), 2.6 (palette), 2.11 (keyboard.spec) |
| Acceptance: axe no serious/critical | 2.11 |
| Acceptance: dark mode complete | 2.1 (tokens + contrast), 2.2, 2.11 (dark screenshots + axe in dark) |
| Acceptance: per-role nav visibility by E2E (intake: Dashboard + Enquiries; viewer: no edit actions) | 2.5 (`navigation.spec.ts`), 2.8 (`dashboard.spec.ts`) |
| Acceptance: Lighthouse accessibility ≥ 95 on `/admin` | 2.11 |
| Carry-over (Task 1.5): `can()` HTTP-layer matrix tests | 2.3 (+ rows in 2.6, 2.8) |
| Carry-over (Ruling F16): admin light/dark/system switching, Sonner follows the theme | 2.2, 2.5 (user menu), 2.6 (⌘K actions) |
| Carry-over (Ruling F4): admin E2E on tablet/mobile | 2.5 (setup project, per-role storage states, `DESKTOP_ONLY = **/admin/auth/**`) |
| Carry-over (Tasks 1.9/1.11): every Server Action / RSC data path calls `requireActor` | 2.3 (`guarded-pages.test.ts`), every page in 2.3–2.10 |
| Brief §9.4: error boundaries with retry; Activity-aware transient state | 2.5 (`error.tsx`), 2.6 (`useResetOnHide` in the palette) |
| Brief §9.4: below 1024 px the sidebar is a sheet; tables become cards on phones | 2.5, 2.9 |
| Brief §9.2: motion 150–250 ms, reduced motion respected, no GSAP/Lenis in the admin | 2.7 (`useReducedMotion`), 2.11 (CSS rule) |

Not in this phase (by the contract): enquiry/content KPIs and notifications data (Phase 11), Users & roles screens (Phase 10), drag-and-drop lists (Phase 4), Linux visual baselines in CI (Phase 11).

## Self-review notes

- **Placeholders:** every code step carries the code; the one prose-only step is the human side-by-side review (Task 2.11 Step 7), which is an acceptance criterion by nature.
- **Type consistency:** `NavLink`/`NavItem`/`Destination` (2.3) are used unchanged by the sidebar (2.5) and search (2.6); `MePayload` (2.3) feeds `AdminActorProvider`; `AuditListParams` (2.4) is produced by `toAuditParams` (2.9); `DashboardDto` (2.8) types the visual fixture (2.11); `ICON_BUTTON` and `SIDEBAR_COOKIE` live in `admin/components/shell/constants.ts`.
- **Review Focus:** each of the five lines has its pinning test in the owning task (2.3, 2.2, 2.6, 2.5, 2.5). The Dublin week across the October clock change is pinned in 2.8 (`week.test.ts`) as well.
