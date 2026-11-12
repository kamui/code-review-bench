// Usage: node probe.mjs <astro-git-dir> <state>
// state: base | head | head+fix-o1 | head+fix-n1 | head+fix-o1+fix-n1
// Runs the path-override lines of packages/integrations/vercel/src/serverless/entrypoint.ts,
// taken verbatim from the commit (with the named patch applied as text), against the query
// strings the ISR function could receive under each way Vercel might combine a visitor's own
// ?x_astro_path= with the route's dest /_isr?x_astro_path=$0. Prints the path Astro would route.
import { execFileSync } from 'node:child_process';

const [repo, state] = process.argv.slice(2);
const BASE = 'be661fb9fd1348ffb038f561f9f053b1c64a3696';
const HEAD = '71ae513388df11d7dad6b1e0077c402ad03d0d62';
const sha = state === 'base' ? BASE : HEAD;
const show = (path) => execFileSync('git', ['-C', repo, 'show', `${sha}:${path}`], { encoding: 'utf8' });

let entry = show('packages/integrations/vercel/src/serverless/entrypoint.ts');
const patches = {
	// GT-o1's upstream fix (#17370) at this branch: honour the query only with the build's token.
	'fix-o1': [
		"} else if(request.headers.get('x-vercel-isr') === '1') {",
		'} else if(url.searchParams.get(ASTRO_PATH_TOKEN_PARAM) === middlewareSecret) {',
	],
	// This group's place alone: refuse an override when x_astro_path occurs more than once.
	'fix-n1': [
		'realPath = url.searchParams.get(ASTRO_PATH_PARAM);',
		'{ const values = url.searchParams.getAll(ASTRO_PATH_PARAM); realPath = values.length === 1 ? values[0] : undefined; }',
	],
};
const applied = state.split('+').filter((s) => s.startsWith('fix-'));
for (const name of applied) {
	const [from, to] = patches[name];
	if (!entry.includes(from)) throw new Error(`${name} does not apply at ${state}`);
	entry = entry.replace(from, to);
}
const lines = entry.split('\n');
const start = lines.findIndex((l) => l.includes('const url = new URL(request.url);'));
const end = lines.findIndex((l, i) => i > start && l.includes('url.pathname = realPath;'));
const slice = lines.slice(start, end + 1).join('\n') + '\n}\nreturn url.pathname;';
console.log(`state ${state}: entrypoint.ts at ${sha.slice(0, 9)}${applied.length ? ' with ' + applied.join(', ') : ''}, lines ${start + 1}-${end + 1}, run as written:`);
console.log(lines.slice(start, end + 1).join('\n') + '\n');

const index = show('packages/integrations/vercel/src/index.ts');
const constant = (name) => index.match(new RegExp(`export const ${name} = '([^']+)'`))[1];
const ASTRO_PATH_HEADER = constant('ASTRO_PATH_HEADER');
const ASTRO_PATH_PARAM = constant('ASTRO_PATH_PARAM');
const ASTRO_MIDDLEWARE_SECRET_HEADER = constant('ASTRO_MIDDLEWARE_SECRET_HEADER');
const ASTRO_PATH_TOKEN_PARAM = 'x_astro_path_token';
const middlewareSecret = 'per-build-secret';
const override = new Function(
	'request', 'ASTRO_PATH_HEADER', 'ASTRO_PATH_PARAM', 'ASTRO_MIDDLEWARE_SECRET_HEADER', 'ASTRO_PATH_TOKEN_PARAM', 'middlewareSecret',
	slice,
);

// The build's dest for the route /one. Under fix-o1 the build appends its token, as #17370 does.
const tokenBuild = applied.includes('fix-o1');
const dest = `x_astro_path=/one${tokenBuild ? `&x_astro_path_token=${middlewareSecret}` : ''}`;
const visitor = 'x_astro_path=/account';
const replaced = `x_astro_path=/account${tokenBuild ? `&x_astro_path_token=${middlewareSecret}` : ''}`;
const cases = [
	['A genuine rewrite of /one, visitor sent no query', `/_isr?${dest}`],
	['B visitor asked /one?x_astro_path=/account; visitor copy kept first', `/_isr?${visitor}&${dest}`],
	['C visitor asked /one?x_astro_path=/account; dest copy kept first', `/_isr?${dest}&${visitor}`],
	["D visitor asked /one?x_astro_path=/account; visitor value replaces dest's (vercel dev's Object.assign rule)", `/_isr?${replaced}`],
	["E visitor asked /one?x_astro_path=/account; dest value replaces visitor's", `/_isr?${dest}`],
	['F GT-o1 case: direct request for /_isr?x_astro_path=/account', `/_isr?${visitor}`],
];
for (const [label, path] of cases) {
	const request = new Request(`https://example.com${path}`, { headers: { 'x-vercel-isr': '1' } });
	const routed = override(request, ASTRO_PATH_HEADER, ASTRO_PATH_PARAM, ASTRO_MIDDLEWARE_SECRET_HEADER, ASTRO_PATH_TOKEN_PARAM, middlewareSecret);
	console.log(`${label}\n  query reaching the function: ${path}  (x-vercel-isr: 1)\n  path Astro routes: ${routed}\n`);
}
