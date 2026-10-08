// Session actions used in more than one place (sidebar, command palette).
import { api } from '$lib/api'
import { profile } from '$lib/features/profile/profile.svelte'
import { session } from './session.svelte'

export async function logout() {
  // Even if the request fails, the panel returns to the login screen.
  await api.post('/logout').catch(() => {})
  session.email = null
  profile.avatar = null
  profile.loaded = false
}
