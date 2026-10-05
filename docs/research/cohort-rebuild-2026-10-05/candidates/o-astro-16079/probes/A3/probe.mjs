// A3: which requests to the ISR function get the path rewrite, depending on the x-vercel-isr header?
// Whether Vercel sends x-vercel-isr: 1 on every kind of ISR invocation is NOT run here.
import { buildProbeFixture, call, show } from './probe-o16079-lib.mjs';

const { isr, prerenderConfig } = await buildProbeFixture();
const url = 'https://example.com/_isr?x_astro_path=/one';
const bypass = prerenderConfig.bypassToken;

const brief = (r) => (r.threw ? { threw: r.threw } : { status: r.status, rendered: r.route ?? r.nonProbeBody });

show('1. x-vercel-isr: 1', brief(await call(isr, url, { headers: { 'x-vercel-isr': '1' } })));
show('2. no x-vercel-isr header', brief(await call(isr, url)));
show('3. x-vercel-isr: true', brief(await call(isr, url, { headers: { 'x-vercel-isr': 'true' } })));
show('4. x-vercel-isr: 0', brief(await call(isr, url, { headers: { 'x-vercel-isr': '0' } })));
show(
	'5. on-demand revalidation header only (x-prerender-revalidate: <bypassToken>), no x-vercel-isr',
	brief(await call(isr, url, { headers: { 'x-prerender-revalidate': bypass } })),
);
show(
	'6. on-demand revalidation header together with x-vercel-isr: 1',
	brief(await call(isr, url, { headers: { 'x-prerender-revalidate': bypass, 'x-vercel-isr': '1' } })),
);
show(
	'7. HEAD request with x-vercel-isr: 1',
	brief(await call(isr, url, { method: 'HEAD', headers: { 'x-vercel-isr': '1' } })),
);
