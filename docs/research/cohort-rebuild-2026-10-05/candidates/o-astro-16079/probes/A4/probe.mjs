// A4: what happens to a request with a body that goes through the path rewrite?
// Whether Vercel ever delivers a POST with a body to the ISR function is NOT run here.
import { buildProbeFixture, call, readMiddlewareSecret, show } from './probe-o16079-lib.mjs';

const { isr, render } = await buildProbeFixture();
// The origin and content-type headers keep Astro's own cross-site form check out of the way,
// so that the baseline POST reaches the endpoint.
const SAME_ORIGIN = { origin: 'https://example.com', 'content-type': 'application/json' };
const post = (headers = {}) => ({
	method: 'POST',
	body: '{"hello":"world"}',
	headers: { ...SAME_ORIGIN, ...headers },
});

show(
	'1. POST with a body straight to the normal function at /api/x (no rewrite involved)',
	await call(render, 'https://example.com/api/x', post()),
);
show(
	'2. POST with a body to the ISR function, x-vercel-isr: 1, /_isr?x_astro_path=/api/x',
	await call(isr, 'https://example.com/_isr?x_astro_path=/api/x', post({ 'x-vercel-isr': '1' })),
);
show(
	'3. Same request without the x-vercel-isr header',
	await call(isr, 'https://example.com/_isr?x_astro_path=/api/x', post()),
);
show(
	'4. POST without a body to the ISR function, x-vercel-isr: 1',
	await call(isr, 'https://example.com/_isr?x_astro_path=/api/x', {
		method: 'POST',
		headers: { ...SAME_ORIGIN, 'x-vercel-isr': '1' },
	}),
);
show(
	'5. GET to the ISR function, x-vercel-isr: 1',
	await call(isr, 'https://example.com/_isr?x_astro_path=/api/x', { headers: { 'x-vercel-isr': '1' } }),
);

// The other way into the same rewrite: the edge-middleware branch, which exists at both commits.
const secret = await readMiddlewareSecret(render.entry);
show(
	`6. POST with a body to the normal function carrying the valid middleware secret and x-astro-path: /api/x (secret found in build: ${secret !== null})`,
	await call(
		render,
		'https://example.com/_render',
		post({ 'x-astro-middleware-secret': secret ?? 'missing', 'x-astro-path': '/api/x' }),
	),
);
