// Navegação entre os painéis dos projetos do host.
import { api } from './api'
import type { ProjectLink } from './types'

/**
 * Abre o painel de outro projeto. Com login único, leva junto um token de
 * handoff (no fragmento da URL, que não vai para logs); sem ele, o outro
 * painel pede o próprio login.
 */
export async function openProject(project: ProjectLink, sso: boolean) {
  if (project.current || !project.url) return
  if (sso) {
    const { url } = await api.post<{ url: string }>('/sso/handoff', { project: project.name })
    location.href = url
  } else {
    location.href = `${project.url.replace(/\/$/, '')}/admin/`
  }
}

/** Token de handoff no fragmento (`#sso=...`), removido da URL na hora. */
export function takeHandoffToken(): string | null {
  const match = location.hash.match(/^#sso=([\w.-]+)$/)
  if (!match) return null
  history.replaceState(null, '', location.pathname + location.search)
  return match[1]
}
