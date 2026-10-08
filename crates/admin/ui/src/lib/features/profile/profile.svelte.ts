// Admin profile: photo version (ms of the last change) or `null` without a photo.
// The version goes into the URL, so the browser caches the image until it changes.
import { api } from '$lib/api'

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
