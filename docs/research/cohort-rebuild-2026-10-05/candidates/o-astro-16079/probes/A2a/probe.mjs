// A2: is the route path changed by being carried in a query value and assigned to url.pathname?
// For each path as a browser would send it, compare:
//   direct  - the normal function called with that path (the non-ISR result)
//   isr-raw - the ISR function, if Vercel copies the matched path into the query unchanged
//   isr-enc - the ISR function, if Vercel percent-encodes the matched path (encodeURIComponent)
//   isr-ref - the ISR function, if Vercel does what its open-source development router does
//             (vercel/vercel packages/cli/src/util/dev/router.ts and parse-query-string.ts, as read):
//             copy the match into the destination unchanged, split the query on "&" and "=",
//             decodeURIComponent each value, then write it back with encodeURIComponent.
//             This column is an emulation of that code as read, not a run of it.
// Which of these Vercel's production proxy does is NOT run here: the platform is not available.
import { buildProbeFixture, call } from './probe-o16079-lib.mjs';

const { isr, render } = await buildProbeFixture();
const ISR = { headers: { 'x-vercel-isr': '1' } };

const paths = [
	'/blog/hello',
	'/blog/a%20b',
	'/blog/caf%C3%A9',
	'/blog/a+b',
	'/blog/c++',
	'/blog/a&b',
	'/blog/r%26d',
	'/blog/a%2Fb',
	'/blog/a%3Fb',
	'/blog/a%23b',
	'/blog/100%25',
	'/blog/x%2F..%2F..%2Fone',
];

const brief = (r) =>
	r.threw
		? `THREW ${r.threw}`
		: `${r.status} ${r.route ? `route=${r.route}` : r.nonProbeBody}${'slug' in r ? ` slug=${JSON.stringify(r.slug)}` : ''}${r.pathname ? ` pathname=${r.pathname}` : ''}`;

for (const path of paths) {
	const direct = await call(render, `https://example.com${path}`);
	const raw = await call(isr, `https://example.com/_isr?x_astro_path=${path}`, ISR);
	const enc = await call(
		isr,
		`https://example.com/_isr?x_astro_path=${encodeURIComponent(path)}`,
		ISR,
	);
	let refValue;
	try {
		refValue = decodeURIComponent(`x_astro_path=${path}`.split('&')[0].split('=')[1]);
	} catch (error) {
		refValue = null;
	}
	const ref =
		refValue === null
			? { threw: 'emulated router could not decode the value' }
			: await call(
					isr,
					`https://example.com/_isr?x_astro_path=${encodeURIComponent(refValue)}`,
					ISR,
				);
	console.log(`request path ${path}`);
	console.log(`    direct : ${brief(direct)}`);
	console.log(`    isr-raw: ${brief(raw)}${brief(raw) === brief(direct) ? '' : '   <-- differs from direct'}`);
	console.log(`    isr-enc: ${brief(enc)}${brief(enc) === brief(direct) ? '' : '   <-- differs from direct'}`);
	console.log(`    isr-ref: ${brief(ref)}${brief(ref) === brief(direct) ? '' : '   <-- differs from direct'}`);
}

console.log('empty override value (/_isr?x_astro_path=)');
console.log(`    isr    : ${brief(await call(isr, 'https://example.com/_isr?x_astro_path=', ISR))}`);
