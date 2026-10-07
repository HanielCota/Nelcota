// Minimal SPA router based at /admin (history API).
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

// Internal links (<a href="/admin/...">) navigate without reloading the page.
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

// Route matching is pure (testable outside the browser): it lives in route-match.
export { match } from './route-match'

export const href = (path: string) => BASE + path
