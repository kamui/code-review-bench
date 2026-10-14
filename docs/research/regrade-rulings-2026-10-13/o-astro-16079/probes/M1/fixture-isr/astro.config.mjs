import vercel from '@astrojs/vercel';
import { defineConfig } from 'astro/config';

// A site with cached pages (ISR) switched on, as in the adapter's own test/fixtures/isr.
// /two is excluded from the cache, so it is served by the normal function.
export default defineConfig({
	output: 'server',
	adapter: vercel({
		isr: {
			bypassToken: '1c9e601d-9943-4e7c-9575-005556d774a8',
			expiration: 120,
			exclude: ['/two'],
		},
	}),
});
