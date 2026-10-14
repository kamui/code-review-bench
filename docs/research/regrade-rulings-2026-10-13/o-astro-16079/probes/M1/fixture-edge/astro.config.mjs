import vercel from '@astrojs/vercel';
import { defineConfig } from 'astro/config';

// The adapter's own edge-middleware fixture (test/fixtures/middleware-with-edge-file) uses the same two options.
export default defineConfig({
	output: 'server',
	adapter: vercel({
		middlewareMode: 'edge',
	}),
});
