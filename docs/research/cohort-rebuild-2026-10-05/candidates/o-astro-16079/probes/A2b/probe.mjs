// A2b: the ISR function uses whatever is in x_astro_path as the path, without checking it is one.
// The case that matters upstream (withastro/astro#18028): Vercel does not fill in "$0" and the
// function receives the two literal characters "$0". Whether and when Vercel does that is NOT run here.
import { buildProbeFixture, call, show } from './probe-o16079-lib.mjs';

const ISR = { headers: { 'x-vercel-isr': '1' } };
const brief = (r) =>
	r.threw
		? { threw: r.threw }
		: { status: r.status, ...(r.location ? { location: r.location } : {}), rendered: r.route ?? r.nonProbeBody, ...(r.pathname ? { pathname: r.pathname } : {}) };

const plain = await buildProbeFixture();
console.log('Site with the default trailing-slash setting:');
show('1. normal value /one', brief(await call(plain.isr, 'https://example.com/_isr?x_astro_path=/one', ISR)));
show('2. literal $0 (not filled in)', brief(await call(plain.isr, 'https://example.com/_isr?x_astro_path=$0', ISR)));
show('3. no leading slash: one', brief(await call(plain.isr, 'https://example.com/_isr?x_astro_path=one', ISR)));
show('4. empty value', brief(await call(plain.isr, 'https://example.com/_isr?x_astro_path=', ISR)));
show('5. two leading slashes: //other.example/one', brief(await call(plain.isr, 'https://example.com/_isr?x_astro_path=//other.example/one', ISR)));

const slash = await buildProbeFixture('./fixtures/probe-o16079-slash/');
console.log("Site with trailingSlash: 'always' (the setting in the upstream report). Generated ISR routes:");
for (const route of slash.config.routes.filter((r) => String(r.dest).startsWith('/_isr'))) {
	console.log(`    ${JSON.stringify(route)}`);
}
show('6. normal value /one/', brief(await call(slash.isr, 'https://example.com/_isr?x_astro_path=/one/', ISR)));
show('7. literal $0 (not filled in)', brief(await call(slash.isr, 'https://example.com/_isr?x_astro_path=$0', ISR)));
show('8. literal $0, percent-encoded in the query (%240)', brief(await call(slash.isr, 'https://example.com/_isr?x_astro_path=%240', ISR)));
