import { readText, write } from './storage'

const KEY = 'nelcota.sidebar'

// Barra lateral fixada aberta (telas largas) ou recolhida em trilho de ícones,
// que se expande por cima do conteúdo ao passar o mouse.
export const sidebar = $state({ pinned: readText(KEY, 'pinned') === 'pinned' })

export function togglePinned() {
  sidebar.pinned = !sidebar.pinned
  write(KEY, sidebar.pinned ? 'pinned' : 'rail')
}
