import { defineConfig } from 'vite'
import { svelte } from '@sveltejs/vite-plugin-svelte'

// Tauri expects a fixed dev port and no clearing of its own output.
export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: { ignored: ['**/src-tauri/**'] }
  },
  build: {
    target: 'chrome110',
    minify: 'esbuild',
    sourcemap: false
  }
})
