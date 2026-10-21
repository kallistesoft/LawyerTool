import { readFileSync } from 'node:fs'
import { svelte } from '@sveltejs/vite-plugin-svelte'
import { defineConfig } from 'vite'

const { version } = JSON.parse(readFileSync(new URL('./package.json', import.meta.url), 'utf8'))

// Static site: `npm run build` writes dist/, deployed as is (see .github/workflows/deploy.yml).
// The engine (ui/src/engine) is built first by scripts/build-wasm.sh.
export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: { port: 1420, strictPort: true, host: '127.0.0.1' },
  envPrefix: [], // nothing from the environment reaches the frontend bundle
  define: { __APP_VERSION__: JSON.stringify(version) },
  worker: { format: 'es' },
  build: { target: ['chrome110', 'edge110'], sourcemap: false },
})
