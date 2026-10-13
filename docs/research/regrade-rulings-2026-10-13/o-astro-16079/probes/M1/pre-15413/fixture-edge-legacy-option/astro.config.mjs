import vercel from '@astrojs/vercel';
import { defineConfig } from 'astro/config';

// The same site as ../fixture-edge. At the commit before #15413 the option is still called edgeMiddleware,
// as in the adapter's own test/fixtures/middleware-with-edge-file at that commit.
export default defineConfig({
	output: 'server',
	adapter: vercel({
		edgeMiddleware: true,
	}),
});
