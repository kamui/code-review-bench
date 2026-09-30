# 02 — Test coverage for the ISR/override contract

Scope: `packages/integrations/vercel/test/isr.test.js`, `packages/integrations/vercel/test/path-override-security.test.js`, and the ISR fixture at `test/fixtures/isr/`.

## Finding 2.1 — the ISR function's runtime contract is untested, which is how both #15959's 404 and this PR's trust leak shipped

`isr.test.js` builds the ISR fixture and asserts only two static artefacts: the contents of `_isr.prerender-config.json` and the route table in `config.json`. It never invokes the built `_isr` handler. That is why #15959 could change the entrypoint so that every ISR route 404s while all tests stayed green, and why this PR, which states "No additional test cases added", can fix it with no regression guard.

Symmetrically, `path-override-security.test.js` only exercises `_render` with no headers besides the override itself. It does not cover the new branch, so the bypass in 01 §1.1 passes the suite.

**Verification: CONFIRMED.** `node --test test/path-override-security.test.js test/isr.test.js` passes 4/4 at the head, while the in-process probe (`clone-work/scratch/probe.mjs`) shows `_render` returning the private route when `x-vercel-isr: 1` is sent. The existing harness already has everything needed: `path-override-security.test.js` contains a `loadFunctionModule(fixture, functionName)` helper that imports a built function and calls `default.fetch` in-process.

Remedy — add two focused cases using that helper (lift it into `test-utils.js` so both files share it rather than copy it):

```js
// isr.test.js
it('_isr renders the route named by x_astro_path', async () => {
	const isr = await loadFunctionModule(fixture, '_isr');
	const res = await isr.default.fetch(new Request('https://example.com/_isr?x_astro_path=/one'));
	assert.equal(res.status, 200);
	assert.match(await res.text(), /<h1>One<\/h1>/);
});

// path-override-security.test.js
it('ignores x_astro_path on _render even with ISR-looking headers', async () => {
	const render = await loadFunctionModule(fixture, '_render');
	const res = await render.default.fetch(
		new Request('https://example.com/api/public?x_astro_path=/api/private', {
			headers: { 'x-vercel-isr': '1' },
		}),
	);
	assert.equal((await res.json()).id, 'public');
});
```

Note that the first test, as written, fails at the head unless the request also carries `x-vercel-isr: 1` — which is itself a useful signal: the test encodes "the ISR function honours its query param", and under the proposal in 01 §1.2 it passes with no header at all. The second test fails at the head (probe line 2) and passes under 01 §1.2.

Verification of the proposed tests: PROPOSAL; the behaviours they assert were each observed in the probe, but the test code itself was not added to the clone.
