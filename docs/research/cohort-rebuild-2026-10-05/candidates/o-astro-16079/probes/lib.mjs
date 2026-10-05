// Shared helper for the o-astro-16079 probes. Copied next to the adapter's own
// tests in the scratch clone (packages/integrations/vercel/test/) so that it
// can use the project's fixture loader, exactly as path-override-security.test.js does.
import { readFile } from 'node:fs/promises';
import { loadFixture } from './test-utils.js';

async function loadFunctionModule(fixture, functionName) {
	const functionConfig = JSON.parse(
		await fixture.readFile(`../.vercel/output/functions/${functionName}.func/.vc-config.json`),
	);
	const functionEntry = new URL(
		`../.vercel/output/functions/${functionName}.func/${functionConfig.handler}`,
		fixture.config.outDir,
	);
	return { module: await import(functionEntry), entry: functionEntry };
}

export async function buildProbeFixture(root = './fixtures/probe-o16079/') {
	const fixture = await loadFixture({ root });
	await fixture.build();
	const isr = await loadFunctionModule(fixture, '_isr');
	const render = await loadFunctionModule(fixture, '_render');
	const config = JSON.parse(await fixture.readFile('../.vercel/output/config.json'));
	const prerenderConfig = JSON.parse(
		await fixture.readFile('../.vercel/output/functions/_isr.prerender-config.json'),
	);
	return { fixture, isr, render, config, prerenderConfig };
}

// Finds the per-build middleware secret that the adapter inlines into the built function.
export async function readMiddlewareSecret(functionEntry) {
	const { readdir } = await import('node:fs/promises');
	const root = new URL('./', functionEntry);
	const stack = [root];
	while (stack.length) {
		const dir = stack.pop();
		for (const entry of await readdir(dir, { withFileTypes: true })) {
			if (entry.name === 'node_modules') continue;
			const child = new URL(entry.name + (entry.isDirectory() ? '/' : ''), dir);
			if (entry.isDirectory()) stack.push(child);
			else if (/\.(mjs|js)$/.test(entry.name)) {
				const text = await readFile(child, 'utf8');
				const match = text.match(/const middlewareSecret = "([0-9a-f-]{36})"/);
				if (match) return match[1];
			}
		}
	}
	return null;
}

function summarise(text, contentType) {
	const probe = text.match(/<script type="application\/json" id="probe">(.*?)<\/script>/s);
	if (probe) return JSON.parse(probe[1]);
	if (contentType?.includes('application/json')) return JSON.parse(text);
	const title = text.match(/<title>(.*?)<\/title>/s);
	return { nonProbeBody: title ? `title=${title[1]}` : text.slice(0, 80) };
}

export async function call(fn, url, init) {
	try {
		const response = await fn.module.default.fetch(new Request(url, init));
		const text = await response.text();
		const out = { status: response.status };
		const location = response.headers.get('location');
		if (location) out.location = location;
		Object.assign(out, summarise(text, response.headers.get('content-type')));
		const canonical = text.match(/<link rel="canonical" href="(.*?)"/);
		if (canonical) out.canonicalLinkInHtml = canonical[1].replaceAll('&amp;', '&');
		return out;
	} catch (error) {
		return { threw: `${error.name}: ${error.message}` };
	}
}

export function show(label, value) {
	console.log(`${label}\n    ${JSON.stringify(value)}`);
}
