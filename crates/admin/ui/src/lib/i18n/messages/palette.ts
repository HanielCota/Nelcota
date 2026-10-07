// Command palette (Ctrl/⌘ K). The `keywords` prefix each item's search text,
// so typing "table" or "tabela" finds tables in the matching language.
import type { Messages } from '../index.svelte'

const ptBR = {
  title: 'Paleta de comandos',
  description: 'Busque páginas, tabelas, consultas e ações',
  placeholder: 'Buscar páginas, tabelas, consultas…',
  empty: 'Nada encontrado.',
  groups: {
    pages: 'Páginas',
    tables: 'Tabelas',
    saved: 'Consultas salvas',
    templates: 'Modelos SQL',
    actions: 'Ações',
  },
  keywords: {
    page: 'página',
    table: 'tabela',
    query: 'consulta',
    template: 'modelo',
  },
  actions: {
    theme: 'Alternar tema',
    themeKeywords: 'ação alternar tema claro escuro',
    docs: 'Abrir documentação da API',
    docsKeywords: 'ação documentação api openapi',
    signOut: 'Sair',
    signOutKeywords: 'ação sair logout',
  },
}

const en: Messages<typeof ptBR> = {
  title: 'Command palette',
  description: 'Search pages, tables, queries and actions',
  placeholder: 'Search pages, tables, queries…',
  empty: 'Nothing found.',
  groups: {
    pages: 'Pages',
    tables: 'Tables',
    saved: 'Saved queries',
    templates: 'SQL templates',
    actions: 'Actions',
  },
  keywords: {
    page: 'page',
    table: 'table',
    query: 'query',
    template: 'template',
  },
  actions: {
    theme: 'Toggle theme',
    themeKeywords: 'action toggle theme light dark',
    docs: 'Open the API documentation',
    docsKeywords: 'action documentation api openapi',
    signOut: 'Sign out',
    signOutKeywords: 'action sign out logout',
  },
}

export default { 'pt-BR': ptBR, en }
