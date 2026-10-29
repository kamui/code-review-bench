import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { pathToFileURL } from 'node:url';

const clone = '/home/jack/.t3/bench-runs-lanes/thermo-213-codex/2026-10-28-thermo-nuclear-review-codex-astra/att-039/clone';
async function load(fixture, name) {
  const folder = `${clone}/packages/integrations/vercel/test/fixtures/${fixture}/.vercel/output/functions/${name}.func/`;
  const config = JSON.parse(await readFile(`${folder}.vc-config.json`, 'utf8'));
  return (await import(pathToFileURL(folder + config.handler))).default;
}

const ssr = await load('serverless-with-dynamic-routes', '_render');
const url = 'https://example.com/api/public?x_astro_path=/api/private';
for (const [name, headers, expected] of [
  ['no ISR header', {}, 'public'],
  ['forged ISR header', { 'x-vercel-isr': '1' }, 'private'],
  ['forged ISR header and invalid middleware secret', { 'x-vercel-isr': '1', 'x-astro-middleware-secret': 'invalid' }, 'private'],
  ['other ISR header value', { 'x-vercel-isr': '0' }, 'public'],
]) {
  const response = await ssr.fetch(new Request(url, { headers }));
  const body = await response.json();
  assert.equal(response.status, 200);
  assert.equal(body.id, expected);
  console.log(JSON.stringify({ case: name, status: response.status, body }));
}

const localsResponse = await ssr.fetch(new Request(url, {
  headers: { 'x-vercel-isr': '1', 'x-astro-locals': '{}' },
}));
assert.equal(localsResponse.status, 403);
console.log(JSON.stringify({ case: 'locals guard retained', status: localsResponse.status }));

const isr = await load('isr', '_isr');
const properISR = await isr.fetch(new Request('https://example.com/_isr?x_astro_path=/one', {
  headers: { 'x-vercel-isr': '1' },
}));
assert.equal(properISR.status, 200);
assert.match(await properISR.text(), /<h1>One<\/h1>/);
console.log(JSON.stringify({ case: 'intended ISR rewrite', status: properISR.status }));

const excludedSSR = await load('isr', '_render');
const excludedResponse = await excludedSSR.fetch(new Request('https://example.com/two?x_astro_path=/api/private', {
  headers: { 'x-vercel-isr': '1' },
}));
assert.equal(excludedResponse.status, 200);
assert.equal(await excludedResponse.text(), 'OK private');
console.log(JSON.stringify({ case: 'ISR-excluded SSR route overridden', status: excludedResponse.status, body: 'OK private' }));
