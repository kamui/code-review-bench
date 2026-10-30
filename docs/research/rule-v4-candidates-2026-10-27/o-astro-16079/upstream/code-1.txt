/**
 * Keep the CMS out of the ISR cache.
 *
 * `@astrojs/vercel`'s `isr.exclude` cannot express the trailing-slash form of a
 * path. It runs each entry through `escapeRegex()`, which splits on `/` and
 * drops empty segments, so `'/admin/'` and `'/admin'` both come out as
 * `^/admin$`. Its own route table then emits `^\/admin\/?$` pointing at the ISR
 * function, and Vercel takes the first match — so `/admin` is excluded and
 * `/admin/` is not.
 *
 * That matters here because `vercel.json` sets `trailingSlash: true`, which
 * means `/admin/` is the form actually requested. Left alone, Vercel would
 * cache the admin shell and — far worse — the API: one editor's draft served
 * from `/api/content/` to the next visitor, and a cached `/api/session/`.
 *
 * So this rewrites the generated route table after the build, putting
 * slash-tolerant rules for the CMS ahead of everything else. It is verified by
 * `npm run build` failing loudly below if any CMS route still resolves to ISR.
 */
import { readdirSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';

const outputDir = resolve(import.meta.dirname, '../.vercel/output');
const configFile = resolve(outputDir, 'config.json');

/**
 * Vercel runtimes this build may emit.
 *
 * `@astrojs/vercel@7` only knows Node 18 and 20 (`SUPPORTED_NODE_VERSIONS` in
 * its adapter). Building on any other major — 22 locally, say — silently falls
 * through to `nodejs18.x`, which Vercel has since removed. The build then
 * succeeds and the *deploy* fails with "invalid runtime", which is a slow way
 * to find out.
 *
 * Vercel has since discontinued Node 20 as well, and `engines.node` now pins the
 * build to 24 — which the adapter does not know either, so it emits
 * `nodejs18.x` there too. This rewrites every runtime the adapter can emit
 * (including `nodejs20.x`, for a build on Node 20) to the one the project runs
 * on, so the output is deployable whichever Node produced it.
 */
const VALID_RUNTIME = 'nodejs24.x';
const STALE_RUNTIMES = new Set(['nodejs20.x', 'nodejs18.x', 'nodejs16.x', 'nodejs14.x']);

function fixRuntimes() {
  const fnDir = join(outputDir, 'functions');
  let found;
  try {
    found = readdirSync(fnDir);
  } catch {
    return;
  }

  for (const entry of found) {
    const cfg = join(fnDir, entry, '.vc-config.json');
    try {
      if (!statSync(cfg).isFile()) continue;
    } catch {
      continue;
    }
    const conf = JSON.parse(readFileSync(cfg, 'utf8'));
    if (STALE_RUNTIMES.has(conf.runtime)) {
      console.log(`  [runtime] ${entry}: ${conf.runtime} -> ${VALID_RUNTIME}`);
      conf.runtime = VALID_RUNTIME;
      writeFileSync(cfg, `${JSON.stringify(conf, null, 2)}
`);
    } else {
      console.log(`  [runtime] ${entry}: ${conf.runtime}`);
    }
  }
}

let config;
try {
  config = JSON.parse(readFileSync(configFile, 'utf8'));
} catch {
  // No adapter output — a plain `astro build` with no Vercel target.
  console.log('[isr] no .vercel/output/config.json; nothing to do');
  process.exit(0);
}

/** Paths that must always hit the live function. */
const LIVE = ['/admin', '/preview', '/api/[^/]+'];

const pattern = (path) => `^${path}/?$`;
const isLive = (src) => LIVE.some((p) => src === pattern(p) || new RegExp(`^\\^\\\\?/${p.replace('[^/]+', '')}`).test(src));

const routes = config.routes ?? [];

// Drop the adapter's own partial excludes and any ISR rule for these paths;
// they are replaced wholesale below.
const cleaned = routes.filter((r) => {
  const src = String(r.src ?? '');
  const bare = src.replace(/\\/g, '');
  const touchesCms = /\^\/(admin|preview|api)/.test(bare);
  return !touchesCms;
});

const guards = LIVE.map((path) => ({ src: pattern(path), dest: '_render' }));

// The filesystem handler has to stay ahead of a catch-all, so splice the guards
// in directly after it rather than at index 0.
const fsIndex = cleaned.findIndex((r) => r.handle === 'filesystem');
const at = fsIndex === -1 ? 0 : fsIndex + 1;
config.routes = [...cleaned.slice(0, at), ...guards, ...cleaned.slice(at)];

writeFileSync(configFile, `${JSON.stringify(config, null, 2)}\n`);

/* ── verify ─────────────────────────────────────────────────────────────── */

const probes = [
  '/admin',
  '/admin/',
  '/preview',
  '/preview/',
  '/api/content',
  '/api/content/',
  '/api/session/',
  '/api/publish/',
];

const resolveDest = (path) => {
  for (const route of config.routes) {
    if (!route.src) continue;
    if (new RegExp(route.src).test(path)) return route.dest ?? '(no dest)';
  }
  return '(unmatched)';
};

const wrong = probes.filter((p) => String(resolveDest(p)).includes('_isr'));

for (const p of probes) {
  console.log(`  ${p.padEnd(18)} -> ${resolveDest(p)}`);
}

if (wrong.length) {
  console.error(`\n[isr] these CMS paths would be cached: ${wrong.join(', ')}`);
  process.exit(1);
}

fixRuntimes();

console.log(`[isr] ${guards.length} CMS route guards in place; nothing cached that should not be`);
