// Ações de sessão usadas em mais de um lugar (topbar, paleta de comandos).
import { api } from './api'
import { profile } from './profile.svelte'
import { session } from './session.svelte'

export async function logout() {
  // Mesmo se a requisição falhar, o painel volta para a tela de login.
  await api.post('/logout').catch(() => {})
  session.email = null
  profile.avatar = null
  profile.loaded = false
}
