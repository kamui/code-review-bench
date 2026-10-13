// Shared helper for the o-astro-16079 probes of this round. run.sh copies it next to the adapter's
// own tests in the scratch clone (packages/integrations/vercel/test/), so that it can use the
// project's fixture loader, as the project's test/path-override-security.test.js does.
// The method is the one of the earlier round's probes (cohort-rebuild-2026-10-05/candidates/o-astro-16079/probes/lib.mjs).
import { readFile, readdir } from 'node:fs/promises';
import { loadFixture } from './test-utils.js';

export async function buildFixture(root) {
	const fixture = await loadFixture({ root });
	await fixture.build();
	return fixture;
}

// Imports a built Vercel serverless function (.vercel/output/functions/<name>.func).
export async function loadFunction(fixture, functionName) {
	const functionConfig = JSON.parse(
		await fixture.readFile(`../.vercel/output/functions/${functionName}.func/.vc-config.json`),
	);
	const entry = new URL(
		`../.vercel/output/functions/${functionName}.func/${functionConfig.handler}`,
		fixture.config.outDir,
	);
	return { module: await import(entry), entry };
}

// Imports the built edge middleware, as the project's test/edge-middleware.test.js does.
export async function loadEdgeMiddleware(fixture) {
	const entry = new URL(
		'../.vercel/output/functions/_middleware.func/middleware.mjs',
		fixture.config.outDir,
	);
	return { module: await import(entry), entry };
}

// Finds the per-build middleware secret that the adapter inlines into the built function.
export async function readMiddlewareSecret(functionEntry) {
	const stack = [new URL('./', functionEntry)];
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

// Turns a Response into one line of facts: status, redirect target, and what the page or endpoint printed.
export async function describe(responsePromise) {
	try {
		const response = await responsePromise;
		const text = await response.text();
		const out = { status: response.status };
		const location = response.headers.get('location');
		if (location) out.location = location;
		const saw = response.headers.get('x-probe-middleware-saw');
		if (saw) out.middlewareSaw = saw;
		Object.assign(out, summarise(text, response.headers.get('content-type')));
		return out;
	} catch (error) {
		return { threw: `${error.name}: ${error.message}` };
	}
}

export function call(fn, url, init) {
	return describe(Promise.resolve().then(() => fn.module.default.fetch(new Request(url, init))));
}

export function show(label, value) {
	console.log(`${label}\n    ${JSON.stringify(value)}`);
}
