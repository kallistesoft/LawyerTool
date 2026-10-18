import { svelte } from '@sveltejs/vite-plugin-svelte'
import { defineConfig } from 'vite'

// Tauri expects a fixed dev port and serves the built files itself in release.
export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: { port: 1420, strictPort: true, host: '127.0.0.1' },
  envPrefix: [], // nothing from the environment reaches the frontend bundle
  build: { target: ['safari15', 'chrome110'], sourcemap: false },
})
