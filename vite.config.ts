import { defineConfig } from 'vite'
import { tanstackStart } from '@tanstack/react-start/plugin/vite'
import react from '@vitejs/plugin-react'
import { fileURLToPath } from 'node:url'

const evidenceDirectories = ['.local', 'artifacts', 'bench'].map(directory =>
  fileURLToPath(new URL(`./${directory}/`, import.meta.url)),
)

export default defineConfig({
  base: process.env.BASE_PATH || '/',
  plugins: [tanstackStart({ prerender: { enabled: true, crawlLinks: false } }), react()],
  server: {
    port: 3000,
    host: '0.0.0.0',
    watch: { ignored: path => evidenceDirectories.some(directory => `${path}/`.startsWith(directory)) },
  },
})
