import { defineConfig } from 'astro/config';
import vercel from '@astrojs/vercel';

export default defineConfig({
  output: 'server',
  adapter: vercel({
    isr: { expiration: 120, exclude: ['/account'] },
  }),
});
