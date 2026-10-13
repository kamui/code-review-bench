// C1: the statements of the comment, checked by running.
//   "The new `else if (request.headers.get('x-vercel-isr') === '1')` branch reads `x_astro_path` from the
//    query string and rewrites `url.pathname` with no secret check. #15959 removed this capability for
//    untrusted callers."
// The built functions of a site with ISR are called in-process. No network, no Vercel platform.
// None of the requests below carries the valid middleware secret.
import { buildFixture, call, loadFunction, show } from './probe-m1-lib.mjs';

const fixture = await buildFixture('./fixtures/probe-m1-isr/');
const isr = await loadFunction(fixture, '_isr');
const render = await loadFunction(fixture, '_render');
const ISR = { 'x-vercel-isr': '1' };

console.log('== the normal function (_render)');
show(
	'1. GET /api/public?x_astro_path=/api/private, no special header',
	await call(render, 'https://example.com/api/public?x_astro_path=/api/private'),
);
show(
	'2. the same request with x-vercel-isr: 1',
	await call(render, 'https://example.com/api/public?x_astro_path=/api/private', { headers: ISR }),
);
show(
	'3. the same request with x-vercel-isr: 1 and a wrong x-astro-middleware-secret',
	await call(render, 'https://example.com/api/public?x_astro_path=/api/private', {
		headers: { ...ISR, 'x-astro-middleware-secret': 'wrong' },
	}),
);
show(
	'4. GET /api/public with header x-astro-path: /api/private and no secret',
	await call(render, 'https://example.com/api/public', { headers: { 'x-astro-path': '/api/private' } }),
);

console.log('\n== the cached-page function (_isr)');
show(
	'5. GET /_isr?x_astro_path=/one with x-vercel-isr: 1 (the shape of a genuine ISR rewrite)',
	await call(isr, 'https://example.com/_isr?x_astro_path=/one', { headers: ISR }),
);
show(
	'6. GET /_isr?x_astro_path=/one without the header',
	await call(isr, 'https://example.com/_isr?x_astro_path=/one'),
);
show(
	'7. GET /_isr?x_astro_path=/two with x-vercel-isr: 1 (page two is excluded from ISR in the site config)',
	await call(isr, 'https://example.com/_isr?x_astro_path=/two', { headers: ISR }),
);
show(
	'8. GET /_isr?x_astro_path=/api/private with x-vercel-isr: 1',
	await call(isr, 'https://example.com/_isr?x_astro_path=/api/private', { headers: ISR }),
);
