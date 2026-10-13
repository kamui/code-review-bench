// M1, the other branch: what the same assignment does with a "?" in the value when the value comes from the
// x_astro_path query parameter (the branch this pull request adds) instead of the x-astro-path header.
// In-process, a site with ISR. No network, no Vercel platform.
import { buildFixture, call, loadFunction, show } from './probe-m1-lib.mjs';

const fixture = await buildFixture('./fixtures/probe-m1-isr/');
const isr = await loadFunction(fixture, '_isr');
const render = await loadFunction(fixture, '_render');
const ISR = { headers: { 'x-vercel-isr': '1' } };

console.log('== 1. A value with a literal "?" written into the query by hand (a genuine rewrite puts only the matched path there)');
show(
	'_isr, GET /_isr?x_astro_path=/one?a=1 with x-vercel-isr: 1',
	await call(isr, 'https://example.com/_isr?x_astro_path=/one?a=1', ISR),
);

console.log('\n== 2. A visitor path that contains an escaped question mark, %3F, compared with the normal function');
for (const visitorPath of ['/blog/a%3Fb', '/one%3Fa=1']) {
	show(`_render, GET ${visitorPath}`, await call(render, `https://example.com${visitorPath}`));
	const once = decodeURIComponent(visitorPath);
	show(
		`_isr, x_astro_path = the path decoded once and percent-encoded (${encodeURIComponent(once)})`,
		await call(isr, `https://example.com/_isr?x_astro_path=${encodeURIComponent(once)}`, ISR),
	);
	show(
		`_isr, x_astro_path = the path percent-encoded as typed (${encodeURIComponent(visitorPath)})`,
		await call(isr, `https://example.com/_isr?x_astro_path=${encodeURIComponent(visitorPath)}`, ISR),
	);
}
