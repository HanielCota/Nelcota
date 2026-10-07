// Perfil do admin: versão da foto (ms da última troca) ou `null` sem foto.
// A versão entra na URL, então o navegador guarda a imagem em cache até ela mudar.
import { api } from './api'

export const profile = $state<{ avatar: number | null; loaded: boolean }>({ avatar: null, loaded: false })

export async function loadProfile() {
  try {
    profile.avatar = (await api.get<{ avatar: number | null }>('/profile')).avatar
  } catch {
    profile.avatar = null
  } finally {
    profile.loaded = true
  }
}

export const avatarUrl = (version: number) => `/admin/api/profile/avatar?v=${version}`
