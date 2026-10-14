// M1: with the adapter's edge middleware, what does the serverless function do with a request whose
// address has a query string?
//
// The built functions are called in-process, the method of the project's own
// test/path-override-security.test.js and test/edge-middleware.test.js. No network, no Vercel platform.
// NOT run: Vercel's edge runtime and Vercel's routing in front of the two functions.
import {
	buildFixture,
	call,
	describe,
	loadEdgeMiddleware,
	loadFunction,
	readMiddlewareSecret,
	show,
} from './probe-m1-lib.mjs';

const fixture = await buildFixture('./fixtures/probe-m1-edge/');
const render = await loadFunction(fixture, '_render');
const edge = await loadEdgeMiddleware(fixture);
const secret = await readMiddlewareSecret(render.entry);
console.log(`middleware secret found in the built _render function: ${secret !== null}`);
const config = JSON.parse(await fixture.readFile('../.vercel/output/config.json'));
console.log(
	`routes in .vercel/output/config.json that mention the middleware: ${JSON.stringify(config.routes.filter((route) => JSON.stringify(route).includes('_middleware')))}`,
);

const ADDRESSES = ['/search', '/blog/hello', '/api/public', '/'];
const WITH_QUERY = ['/search?q=a', '/blog/hello?ref=x', '/api/public?x=1', '/?q=a'];

console.log('\n== A. The expression the edge middleware puts in x-astro-path (src/serverless/middleware.ts line 133), evaluated in Node');
for (const address of [...ADDRESSES, ...WITH_QUERY]) {
	const requestUrl = `https://example.com${address}`;
	const { origin } = new URL(requestUrl);
	console.log(`    request.url ${requestUrl}  ->  request.url.replace(origin, '') = ${JSON.stringify(requestUrl.replace(origin, ''))}`);
}
console.log(
	`    new URL('/_render', 'https://example.com/search?q=a').href = ${JSON.stringify(new URL('/_render', 'https://example.com/search?q=a').href)}   (the address the middleware fetches, line 129)`,
);
{
	const url = new URL('https://example.com/_render');
	url.pathname = '/search?q=a';
	console.log(`    url.pathname = '/search?q=a' on https://example.com/_render gives href ${JSON.stringify(url.href)}, pathname ${JSON.stringify(url.pathname)}, search ${JSON.stringify(url.search)}`);
}

console.log('\n== B. Control: the _render function called directly, no middleware headers (what the same site gets without edge middleware)');
for (const address of WITH_QUERY) {
	show(`GET ${address}`, await call(render, `https://example.com${address}`));
}

const trusted = (path) => ({
	headers: { 'x-astro-middleware-secret': secret ?? 'missing', 'x-astro-path': path },
});

console.log('\n== C. Control: _render called at /_render with the valid middleware secret and x-astro-path WITHOUT a query string');
for (const address of ADDRESSES) {
	show(`x-astro-path: ${address}`, await call(render, 'https://example.com/_render', trusted(address)));
}

console.log('\n== D. _render called at /_render with the valid middleware secret and x-astro-path WITH a query string');
for (const address of WITH_QUERY) {
	show(`x-astro-path: ${address}`, await call(render, 'https://example.com/_render', trusted(address)));
}

console.log('\n== E. Control: the same header with a wrong secret (the override is not applied; the request is for /_render)');
show(
	'x-astro-path: /search?q=a, x-astro-middleware-secret: wrong',
	await call(render, 'https://example.com/_render', {
		headers: { 'x-astro-middleware-secret': 'wrong', 'x-astro-path': '/search?q=a' },
	}),
);

console.log('\n== F. End to end in-process: the built edge middleware function is called with the visitor\'s request;');
console.log('==    its fetch() to /_render is handed to the built _render function instead of the network.');
const realFetch = globalThis.fetch;
for (const address of [...ADDRESSES, ...WITH_QUERY]) {
	const sent = [];
	globalThis.fetch = async (input, init) => {
		const request = new Request(input, init);
		sent.push({
			fetched: request.url,
			method: request.method,
			'x-astro-path': request.headers.get('x-astro-path'),
			carriesValidSecret: request.headers.get('x-astro-middleware-secret') === secret,
		});
		return render.module.default.fetch(request);
	};
	const result = await describe(
		Promise.resolve().then(() => edge.module.default(new Request(`https://example.com${address}`), {})),
	);
	globalThis.fetch = realFetch;
	console.log(`visitor requests GET ${address}`);
	console.log(`    middleware sent: ${JSON.stringify(sent)}`);
	console.log(`    visitor receives: ${JSON.stringify(result)}`);
}

console.log('\n== G. The same site with a catch-all page ([...rest].astro), end to end in-process');
{
	const fixtureCatchall = await buildFixture('./fixtures/probe-m1-edge-catchall/');
	const renderCatchall = await loadFunction(fixtureCatchall, '_render');
	const edgeCatchall = await loadEdgeMiddleware(fixtureCatchall);
	for (const address of ['/search', '/search?q=a']) {
		globalThis.fetch = async (input, init) => renderCatchall.module.default.fetch(new Request(input, init));
		const result = await describe(
			Promise.resolve().then(() =>
				edgeCatchall.module.default(new Request(`https://example.com${address}`), {}),
			),
		);
		globalThis.fetch = realFetch;
		show(`visitor requests GET ${address}`, result);
	}
}
