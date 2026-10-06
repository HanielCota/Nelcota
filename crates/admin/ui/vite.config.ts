import { svelte } from '@sveltejs/vite-plugin-svelte'
import tailwindcss from '@tailwindcss/vite'
import { fileURLToPath, URL } from 'node:url'
import { defineConfig } from 'vite'

// O build vai para dist/ e é embutido no binário (rust-embed) em /admin/.
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
    // Fontes e ícones como arquivos (CSP: nada de data: em scripts).
    assetsInlineLimit: 0,
    chunkSizeWarningLimit: 900,
  },
  server: {
    // `npm run dev`: a API do painel vem do nelcota rodando localmente.
    proxy: { '/admin/api': { target: 'https://localhost', secure: false, changeOrigin: true } },
  },
})
