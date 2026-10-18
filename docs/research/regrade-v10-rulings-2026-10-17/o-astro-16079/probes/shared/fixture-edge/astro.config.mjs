import vercel from '@astrojs/vercel';
import { defineConfig } from 'astro/config';

// The same site with its middleware run at the edge.
export default defineConfig({
	output: 'server',
	adapter: vercel({
		middlewareMode: 'edge',
		isr: {
			bypassToken: '1c9e601d-9943-4e7c-9575-005556d774a8',
			expiration: 120,
			exclude: ['/account', '/two'],
		},
	}),
});
