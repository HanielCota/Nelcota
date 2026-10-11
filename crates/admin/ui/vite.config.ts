import { svelte } from '@sveltejs/vite-plugin-svelte'
import tailwindcss from '@tailwindcss/vite'
import { fileURLToPath, URL } from 'node:url'
import { defineConfig } from 'vite'

// .env.example exports NELCOTA_API_URL empty, which is not a proxy target.
const apiTarget = process.env.NELCOTA_API_URL || 'http://127.0.0.1:8000'

// The build goes to dist/ and is embedded in the binary (rust-embed) under /admin/.
export default defineConfig({
  base: '/admin/',
  define: { __VUE_OPTIONS_API__: false, __VUE_PROD_DEVTOOLS__: false, __VUE_PROD_HYDRATION_MISMATCH_DETAILS__: false },
  plugins: [tailwindcss(), svelte()],
  resolve: {
    alias: { $lib: fileURLToPath(new URL('./src/lib', import.meta.url)) },
  },
  build: {
    outDir: 'dist',
    assetsDir: 'assets',
    emptyOutDir: true,
    rolldownOptions: {
      input: {
        index: fileURLToPath(new URL('./index.html', import.meta.url)),
        reference: fileURLToPath(new URL('./assets/reference.html', import.meta.url)),
      },
    },
    // Fonts and icons as files (CSP: no data: in scripts).
    assetsInlineLimit: 0,
    chunkSizeWarningLimit: 900,
  },
  server: {
    // `npm run dev`: both the panel API and health checks come from Nelcota.
    // Keep Host aligned with the browser's Origin for the panel's CSRF check.
    proxy: {
      '/admin/api': { target: apiTarget, changeOrigin: false },
      '/health': { target: apiTarget, changeOrigin: false },
      '/rest/v1': { target: apiTarget, changeOrigin: false },
    },
    // Pages are lazy modules; transforming them up front keeps the first visit
    // to each page fast (the e2e tests open many pages in parallel).
    warmup: { clientFiles: ['./src/main.ts', './src/lib/shell/**/*.svelte', './src/lib/features/**/*.svelte'] },
  },
})
