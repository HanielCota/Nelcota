// Nome de cada página: trilha da topbar e título da aba do navegador.
import { match } from './route-match'

const titles: Record<string, string> = {
  '/': 'Visão geral',
  '/tables': 'Tabelas',
  '/sql': 'Editor SQL',
  '/migrations': 'Migrações',
  '/users': 'Usuários',
  '/policies': 'Policies',
  '/projects': 'Projetos',
  '/connect': 'API',
}

export type Crumb = { label: string; path?: string }

/** Trilha da página atual (topbar) e, juntando os rótulos, o título da aba. */
export function crumbsFor(path: string): Crumb[] {
  const table = match('/tables/:name', path)
  if (table) return [{ label: 'Tabelas', path: '/tables' }, { label: table.name }]
  const structure = match('/tables/:name/structure', path)
  if (structure) {
    return [
      { label: 'Tabelas', path: '/tables' },
      { label: structure.name, path: `/tables/${encodeURIComponent(structure.name)}` },
      { label: 'Estrutura' },
    ]
  }
  return [{ label: titles[path.replace(/\/$/, '') || '/'] ?? 'Página não encontrada' }]
}

/** Título da aba do navegador: do mais específico ao nome do produto. */
export const documentTitle = (crumbs: Crumb[]) =>
  [...crumbs.map((c) => c.label).reverse(), 'Nelcota'].join(' · ')
