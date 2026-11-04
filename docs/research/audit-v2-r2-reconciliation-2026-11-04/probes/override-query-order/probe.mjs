// Usage: node probe.mjs <mirror.git>
// Runs the head's path-override lines of packages/integrations/vercel/src/serverless/entrypoint.ts,
// taken verbatim from the mirror, against requests whose query holds x_astro_path once, twice
// (in either order) or beside a visitor parameter. Prints the URL Astro would route.
import { execFileSync } from 'node:child_process';

const mirror = process.argv[2];
const HEAD = '71ae513388df11d7dad6b1e0077c402ad03d0d62';
const show = (sha, path) => execFileSync('git', ['-C', mirror, 'show', `${sha}:${path}`], { encoding: 'utf8' });

const entry = show(HEAD, 'packages/integrations/vercel/src/serverless/entrypoint.ts').split('\n');
const start = entry.findIndex((l) => l.includes('const url = new URL(request.url);'));
const end = entry.findIndex((l, i) => i > start && l.includes('url.pathname = realPath;'));
const slice = entry.slice(start, end + 1).join('\n') + '\n}\nreturn url.toString();';
console.log(`entrypoint.ts at ${HEAD.slice(0, 9)}, lines ${start + 1}-${end + 1}, run as written:\n${entry.slice(start, end + 1).join('\n')}\n`);

const index = show(HEAD, 'packages/integrations/vercel/src/index.ts');
const constant = (name) => index.match(new RegExp(`export const ${name} = '([^']+)'`))[1];
const ASTRO_PATH_HEADER = constant('ASTRO_PATH_HEADER');
const ASTRO_PATH_PARAM = constant('ASTRO_PATH_PARAM');
const ASTRO_MIDDLEWARE_SECRET_HEADER = constant('ASTRO_MIDDLEWARE_SECRET_HEADER');
const middlewareSecret = 'per-build-secret';

const override = new Function(
	'request', 'ASTRO_PATH_HEADER', 'ASTRO_PATH_PARAM', 'ASTRO_MIDDLEWARE_SECRET_HEADER', 'middlewareSecret',
	slice,
);

const cases = [
	['dest value only (a normal ISR rewrite of /one)', '/_isr?x_astro_path=/one'],
	['visitor copy first, dest copy second', '/_isr?x_astro_path=/excluded/admin&x_astro_path=/one'],
	['dest copy first, visitor copy second', '/_isr?x_astro_path=/one&x_astro_path=/excluded/admin'],
	['dest value plus a visitor parameter', '/_isr?x_astro_path=/one&q=a'],
];
for (const [label, path] of cases) {
	const request = new Request(`https://example.com${path}`, { headers: { 'x-vercel-isr': '1' } });
	const routed = override(request, ASTRO_PATH_HEADER, ASTRO_PATH_PARAM, ASTRO_MIDDLEWARE_SECRET_HEADER, middlewareSecret);
	console.log(`${label}\n  request ${path}  (x-vercel-isr: 1)\n  routed  ${routed}\n`);
}
