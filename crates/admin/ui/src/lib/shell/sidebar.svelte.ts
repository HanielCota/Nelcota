import { readText, write } from '$lib/local-storage'

const KEY = 'nelcota.sidebar'

// Sidebar pinned open (wide screens) or collapsed into an icon rail that
// expands over the content on hover.
export const sidebar = $state({ pinned: readText(KEY, 'pinned') === 'pinned' })

export function togglePinned() {
  sidebar.pinned = !sidebar.pinned
  write(KEY, sidebar.pinned ? 'pinned' : 'rail')
}
