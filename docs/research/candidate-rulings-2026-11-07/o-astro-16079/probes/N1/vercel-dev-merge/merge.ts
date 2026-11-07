// Usage: bun merge.ts
// Runs the query merge that Vercel's own local dev router applies to a route with a dest
// (packages/cli/src/util/dev/router.ts lines 192-195 at vercel/vercel 69a6eabc6, the last commit
// before the cut-off): destQuery = parseQueryString(destSearch); Object.assign(destQuery, reqQuery).
// parse-query-string.ts is a byte copy of packages/cli/src/util/dev/parse-query-string.ts at that commit.
// This is Vercel's development emulator, not its production edge; it says nothing about production.
import url from 'node:url';
import { parseQueryString, formatQueryString } from './parse-query-string';

const cases: [string, string][] = [
	['/one?x_astro_path=/account', '/_isr?x_astro_path=/one'],
	['/one?q=a', '/_isr?x_astro_path=/one'],
	['/one', '/_isr?x_astro_path=/one'],
];
for (const [reqUrl, destPath] of cases) {
	const { search: reqSearch } = url.parse(reqUrl);
	const reqQuery = parseQueryString(reqSearch);
	const { pathname: destPathname, search: destSearch } = url.parse(destPath);
	const destQuery = parseQueryString(destSearch);
	Object.assign(destQuery, reqQuery);
	console.log(`request ${reqUrl}  dest ${destPath}\n  merged: ${destPathname}${formatQueryString(destQuery) ?? ''}  ${JSON.stringify(destQuery)}`);
}
