// Grouping runs for M1 (rule Before 4): every failing case of M1 and of the four known problems of this
// pull request, run against one state of the adapter (the head, or the head with one or two small patches).
// Each case prints what was observed and one verdict: FAULT SHOWN or fault not shown.
// The built functions are called in-process. No network, no Vercel platform.
import {
	buildFixture,
	call,
	describe,
	loadEdgeMiddleware,
	loadFunction,
	readMiddlewareSecret,
} from './probe-m1-lib.mjs';

const verdict = (shown) => (shown ? 'FAULT SHOWN' : 'fault not shown');
function report(id, what, observed, shown) {
	console.log(`${id}  ${verdict(shown)}\n    case: ${what}\n    observed: ${JSON.stringify(observed)}`);
}

// ---------- M1: edge middleware, address with a query string ----------
console.log('== M1: a request with a query string through the edge middleware (site with middlewareMode: edge, no ISR)');
{
	const fixture = await buildFixture('./fixtures/probe-m1-edge/');
	const render = await loadFunction(fixture, '_render');
	const edge = await loadEdgeMiddleware(fixture);
	const realFetch = globalThis.fetch;
	const viaEdge = async (address) => {
		globalThis.fetch = async (input, init) => render.module.default.fetch(new Request(input, init));
		const result = await describe(
			Promise.resolve().then(() => edge.module.default(new Request(`https://example.com${address}`), {})),
		);
		globalThis.fetch = realFetch;
		return result;
	};
	let r = await viaEdge('/search');
	report('M1-0 (control)', 'GET /search through the edge middleware renders the search page', r, !(r.status === 200 && r.route === 'search'));
	r = await viaEdge('/search?q=a');
	report('M1-a', 'GET /search?q=a renders the search page with q=a', r, !(r.status === 200 && r.route === 'search' && r.q === 'a'));
	r = await viaEdge('/blog/hello?ref=x');
	report('M1-b', 'GET /blog/hello?ref=x renders blog/[slug] with slug "hello" and ref=x', r, !(r.status === 200 && r.slug === 'hello' && r.ref === 'x'));
	r = await viaEdge('/api/public?x=1');
	report('M1-c', 'GET /api/public?x=1 reaches api/[id] with id "public" and x=1', r, !(r.status === 200 && r.id === 'public' && r.x === '1'));
	r = await viaEdge('/?q=a');
	report('M1-d', 'GET /?q=a renders the index page with q=a', r, !(r.status === 200 && r.route === 'index' && r.q === 'a'));
}

// ---------- the known problems: a site with ISR ----------
const fixture = await buildFixture('./fixtures/probe-m1-isr/');
const isr = await loadFunction(fixture, '_isr');
const render = await loadFunction(fixture, '_render');
const secret = await readMiddlewareSecret(render.entry);
const ISR = { headers: { 'x-vercel-isr': '1' } };
// A genuine ISR rewrite. If this state of the adapter wants proof of origin (the GT-o1 fix), the probe
// supplies the token the patched route table would carry, and says so.
async function genuine(fn, value) {
	let result = await call(fn, `https://example.com/_isr?x_astro_path=${value}`, ISR);
	let withToken = false;
	if (result.status === 404 && secret) {
		const second = await call(
			fn,
			`https://example.com/_isr?x_astro_path=${value}&x_astro_path_token=${secret}`,
			ISR,
		);
		if (second.status !== 404 || second.threw) {
			result = second;
			withToken = true;
		}
	}
	return { ...result, sentWithToken: withToken };
}

console.log('\n== GT-o1: a caller without credentials chooses the rendered route');
{
	let r = await genuine(isr, '/one');
	report('o1-0 (control)', 'a genuine ISR rewrite for /one still renders page one', r, !(r.status === 200 && r.route === 'one'));
	r = await call(render, 'https://example.com/api/public?x_astro_path=/api/private', ISR);
	report('o1-a', '_render, GET /api/public?x_astro_path=/api/private with x-vercel-isr: 1 and no secret: the caller must not get id "private"', r, r.id === 'private');
	r = await call(isr, 'https://example.com/_isr?x_astro_path=/two', ISR);
	report('o1-b', '_isr, GET /_isr?x_astro_path=/two with x-vercel-isr: 1 and no secret: the caller must not get page two, which the site excludes from ISR', r, r.route === 'two');
}

console.log('\n== GT-o2: the internal x_astro_path parameter stays on the address that site code sees');
{
	let r = await genuine(isr, '/one');
	report('o2-a', 'page one through a genuine ISR rewrite: Astro.url must not carry x_astro_path', r, typeof r.search !== 'string' || r.search.includes('x_astro_path='));
	r = await genuine(isr, '/api/a');
	report('o2-b', 'endpoint /api/a through a genuine ISR rewrite: request.url must not carry x_astro_path', r, typeof r.requestUrl !== 'string' || r.requestUrl.includes('x_astro_path='));
}

console.log('\n== GT-o3: one level of percent-encoding is lost (value as Vercel sends it: the path decoded once, then percent-encoded)');
for (const [id, visitorPath] of [
	['o3-a', '/blog/100%25'],
	['o3-b', '/blog/a%2Fb'],
	['o3-c', '/blog/r%26d'],
]) {
	const direct = await call(render, `https://example.com${visitorPath}`);
	const r = await genuine(isr, encodeURIComponent(decodeURIComponent(visitorPath)));
	const same = !r.threw && r.status === direct.status && r.slug === direct.slug;
	report(id, `visitor path ${visitorPath}: the ISR function must match what _render gives for the same path, which is ${JSON.stringify({ status: direct.status, slug: direct.slug })}`, r, !same);
}

console.log('\n== GT-o4: an unfilled $0 placeholder is accepted as a page address');
{
	let r = await call(isr, 'https://example.com/_isr?x_astro_path=$0', ISR);
	if (r.status === 404 && secret) {
		const second = await call(isr, `https://example.com/_isr?x_astro_path=$0&x_astro_path_token=${secret}`, ISR);
		if (second.status !== 404 || second.nonProbeBody !== r.nonProbeBody) r = { ...second, sentWithToken: true };
	}
	console.log(`o4-0 (observation, default trailingSlash)\n    case: _isr, GET /_isr?x_astro_path=$0 with x-vercel-isr: 1\n    observed: ${JSON.stringify(r)}`);
	const slash = await buildFixture('./fixtures/probe-m1-isr-slash/');
	const isrSlash = await loadFunction(slash, '_isr');
	const secretSlash = await readMiddlewareSecret(isrSlash.entry);
	r = await call(isrSlash, 'https://example.com/_isr?x_astro_path=$0', ISR);
	if (r.status === 404 && secretSlash) {
		const second = await call(isrSlash, `https://example.com/_isr?x_astro_path=$0&x_astro_path_token=${secretSlash}`, ISR);
		if (second.status !== 404) r = { ...second, sentWithToken: true };
	}
	report('o4-a', 'site with trailingSlash: always, _isr, GET /_isr?x_astro_path=$0 with x-vercel-isr: 1: must not redirect to the placeholder as a page address', r, r.status === 301 && String(r.location).includes('$0'));
	r = await call(isrSlash, 'https://example.com/_isr?x_astro_path=/one/', ISR);
	if (r.status === 404 && secretSlash) {
		r = { ...(await call(isrSlash, `https://example.com/_isr?x_astro_path=/one/&x_astro_path_token=${secretSlash}`, ISR)), sentWithToken: true };
	}
	report('o4-1 (control)', 'the same site, a genuine rewrite for /one/ renders page one', r, !(r.status === 200 && r.route === 'one'));
}
