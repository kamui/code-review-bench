// A1: does the adapter-internal x_astro_path parameter reach user code on an ISR page?
import { buildProbeFixture, call, show } from './probe-o16079-lib.mjs';

const { isr, render, config, prerenderConfig } = await buildProbeFixture();
const ISR = { headers: { 'x-vercel-isr': '1' } };

console.log('Generated Vercel routes that send pages to the ISR function:');
for (const route of config.routes.filter((r) => String(r.dest).startsWith('/_isr'))) {
	console.log(`    ${JSON.stringify(route)}`);
}
show('Generated prerender config of the ISR function:', prerenderConfig);

show(
	'1. Page /one rendered directly by the normal function (what the page sees without ISR):',
	await call(render, 'https://example.com/one'),
);
show(
	'2. Page /one reached the way the ISR route sends it (/_isr?x_astro_path=/one, x-vercel-isr: 1):',
	await call(isr, 'https://example.com/_isr?x_astro_path=/one', ISR),
);
show(
	'3. Same, with the path percent-encoded in the query (/_isr?x_astro_path=%2Fone):',
	await call(isr, 'https://example.com/_isr?x_astro_path=%2Fone', ISR),
);
show(
	'4. Endpoint /api/a through the ISR function (request.url as an endpoint sees it):',
	await call(isr, 'https://example.com/_isr?x_astro_path=/api/a', ISR),
);
