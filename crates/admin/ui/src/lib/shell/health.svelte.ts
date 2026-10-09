// Whether the server answers, for the dot on the project chip. Checked once a
// minute and whenever the window regains focus or the network comes back, so
// a stopped server shows before the next page load fails.
const INTERVAL_MS = 60_000

export const health = $state<{ status: 'checking' | 'online' | 'offline' }>({ status: 'checking' })

let started = false

async function check() {
  try {
    const res = await fetch('/health', { cache: 'no-store' })
    health.status = res.ok ? 'online' : 'offline'
  } catch {
    health.status = 'offline'
  }
}

/** Starts the checks (once per page load); returns nothing to stop, they live with the panel. */
export function watchHealth() {
  if (started) return
  started = true
  void check()
  setInterval(check, INTERVAL_MS)
  window.addEventListener('focus', check)
  window.addEventListener('online', check)
  window.addEventListener('offline', () => (health.status = 'offline'))
}

/** "local" for this machine, else the host the panel is served from. */
export function environment(hostname = location.hostname): { local: true } | { local: false; host: string } {
  const local = hostname === 'localhost' || hostname === '::1' || hostname === '[::1]' || hostname.startsWith('127.') || hostname.endsWith('.localhost')
  return local ? { local: true } : { local: false, host: hostname }
}
