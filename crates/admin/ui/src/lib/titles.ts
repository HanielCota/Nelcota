// Page names: the topbar breadcrumb and the browser tab title.
import { t, type MessageKey } from './i18n/index.svelte'
import { match } from './route-match'

const titles: Record<string, MessageKey> = {
  '/': 'shell.pages.overview',
  '/tables': 'shell.pages.tables',
  '/sql': 'shell.pages.sql',
  '/migrations': 'shell.pages.migrations',
  '/users': 'shell.pages.users',
  '/policies': 'shell.pages.policies',
  '/projects': 'shell.pages.projects',
  '/connect': 'shell.pages.connect',
}

export type Crumb = { label: string; path?: string }

/** Breadcrumb of the current page (topbar); joined, it is the tab title. */
export function crumbsFor(path: string): Crumb[] {
  const table = match('/tables/:name', path)
  if (table) return [{ label: t('shell.pages.tables'), path: '/tables' }, { label: table.name }]
  const structure = match('/tables/:name/structure', path)
  if (structure) {
    return [
      { label: t('shell.pages.tables'), path: '/tables' },
      { label: structure.name, path: `/tables/${encodeURIComponent(structure.name)}` },
      { label: t('shell.pages.structure') },
    ]
  }
  return [{ label: t(titles[path.replace(/\/$/, '') || '/'] ?? 'shell.pages.notFound') }]
}

/** Browser tab title: most specific first, product name last. */
export const documentTitle = (crumbs: Crumb[]) =>
  [...crumbs.map((c) => c.label).reverse(), 'Nelcota'].join(' · ')
