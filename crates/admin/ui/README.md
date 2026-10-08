# Nelcota panel

SPA in Svelte 5 + shadcn-svelte + Tailwind v4 + CodeMirror 6, built with Vite.
`dist/` is committed and embedded in the binary by the `nelcota-admin` crate.

```sh
npm install
npm run dev     # http://localhost:5173/admin/ (API proxied to https://localhost)
npm run check
npm run build   # updates dist/: commit changed output in a separate commit
```

Each `src/lib/features/<feature>/` contains the feature's pages, state, API
adapter, helpers, components and adjacent unit tests. Use relative imports
inside the feature and `$lib/...` for dependencies outside it.

The application frame, navigation and command palette live in `src/lib/shell`.
Reusable panel components live in `src/lib/components/shared`; shadcn-svelte
primitives stay in `src/lib/components/ui`. Shared table/policy metadata and
DDL previews live in `src/lib/shared/schema`. General transport, resource
loading and persistence helpers stay at the library root (`local-storage.ts`
handles browser persistence; `features/storage` owns server files).

Wire types and validators in `src/lib/generated` come from Rust DTOs. Import
the generated types through `$lib/types` rather than declaring another copy.
Panel texts live in `src/lib/i18n/messages` (pt-BR and English).
