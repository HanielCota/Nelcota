import { svelte } from '@sveltejs/vite-plugin-svelte'
import tailwindcss from '@tailwindcss/vite'
import { fileURLToPath, URL } from 'node:url'
import { defineConfig } from 'vite'

// The build goes to dist/ and is embedded in the binary (rust-embed) under /admin/.
export default defineConfig({
  base: '/admin/',
  plugins: [tailwindcss(), svelte()],
  resolve: {
    alias: { $lib: fileURLToPath(new URL('./src/lib', import.meta.url)) },
  },
  build: {
    outDir: 'dist',
    assetsDir: 'assets',
    emptyOutDir: true,
    // Fonts and icons as files (CSP: no data: in scripts).
    assetsInlineLimit: 0,
    chunkSizeWarningLimit: 900,
  },
  server: {
    // `npm run dev`: the panel API comes from nelcota running locally.
    proxy: { '/admin/api': { target: process.env.NELCOTA_API_URL ?? 'http://127.0.0.1:8000', changeOrigin: true } },
  },
})
