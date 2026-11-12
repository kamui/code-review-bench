import { defineConfig } from 'astro/config';
import vercel from '@astrojs/vercel';

export default defineConfig({
  output: 'server',
  adapter: vercel({
    isr: { expiration: 60, exclude: ['/account'], bypassToken: '<redacted 40-hex random token>' },
  }),
});
