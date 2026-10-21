import { readFileSync } from 'node:fs'
import { svelte } from '@sveltejs/vite-plugin-svelte'
import { defineConfig } from 'vite'

const { version } = JSON.parse(readFileSync(new URL('./package.json', import.meta.url), 'utf8'))

// Static site: `npm run build` writes dist/, deployed as is (see .github/workflows/web.yml).
// The engine (ui/src/engine) is built first by scripts/build-wasm.sh.
//
// The dev vault (automatic test vault in the browser's storage: no
// passphrase, no auto-lock, fictitious data only) is on in `vite dev` and in
// `vite build --mode demo`; a normal build uses the real vault file.
export default defineConfig(({ command, mode }) => ({
  plugins: [svelte()],
  clearScreen: false,
  server: { port: 1420, strictPort: true, host: '127.0.0.1' },
  envPrefix: [], // nothing from the environment reaches the frontend bundle
  define: {
    __APP_VERSION__: JSON.stringify(version),
    __DEV_VAULT__: JSON.stringify(command === 'serve' || mode === 'demo'),
  },
  worker: { format: 'es' },
  build: { target: ['chrome110', 'edge110'], sourcemap: false },
}))
