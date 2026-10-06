# Painel do nelcota

SPA em Svelte 5 + shadcn-svelte + Tailwind v4 + CodeMirror 6, compilada com Vite.
O `dist/` é versionado e embutido no binário pelo crate `nelcota-admin`.

```sh
npm install
npm run dev     # http://localhost:5173/admin/ (API encaminhada a https://localhost)
npm run check
npm run build   # atualiza dist/: faça commit junto com o código
```

Componentes de UI em `src/lib/components/ui` (gerados pelo CLI do shadcn-svelte);
componentes do painel em `src/lib/components/app`; páginas em `src/lib/pages`.
