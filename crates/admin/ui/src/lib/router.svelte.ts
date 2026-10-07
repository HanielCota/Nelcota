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
    anchor.getAttribute('href')?.startsWith('#') ||
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

// Casamento de rotas é puro (testável fora do navegador): mora em route-match.
export { match } from './route-match'

export const href = (path: string) => BASE + path
