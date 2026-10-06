// Roteador mínimo da SPA, com base em /admin (history API).
const BASE = '/admin'

function current(): string {
  const path = location.pathname.startsWith(BASE) ? location.pathname.slice(BASE.length) : '/'
  return path === '' ? '/' : path
}

export const route = $state({ path: current(), query: new URLSearchParams(location.search) })

export function navigate(to: string, replace = false) {
  const url = BASE + (to.startsWith('/') ? to : `/${to}`)
  if (replace) history.replaceState(null, '', url)
  else history.pushState(null, '', url)
  sync()
}

function sync() {
  route.path = current()
  route.query = new URLSearchParams(location.search)
}

addEventListener('popstate', sync)

// Links internos (<a href="/admin/...">) navegam sem recarregar a página.
addEventListener('click', (event) => {
  const anchor = (event.target as HTMLElement).closest('a')
  if (
    !anchor ||
    anchor.target ||
    event.defaultPrevented ||
    event.button !== 0 ||
    event.metaKey ||
    event.ctrlKey ||
    event.shiftKey
  )
    return
  const url = new URL(anchor.href, location.href)
  if (url.origin !== location.origin || !url.pathname.startsWith(BASE + '/')) return
  if (url.pathname.startsWith(BASE + '/api')) return
  event.preventDefault()
  navigate(url.pathname.slice(BASE.length) + url.search)
})

/** `/tables/notas` contra o padrão `/tables/:name` → `{ name: 'notas' }`. */
export function match(pattern: string, path: string): Record<string, string> | null {
  const p = pattern.split('/')
  const s = path.split('/')
  if (p.length !== s.length) return null
  const params: Record<string, string> = {}
  for (let i = 0; i < p.length; i++) {
    if (p[i].startsWith(':')) params[p[i].slice(1)] = decodeURIComponent(s[i])
    else if (p[i] !== s[i]) return null
  }
  return params
}

export const href = (path: string) => BASE + path
