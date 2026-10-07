// Cliente da API do painel (/admin/api). Sessão via cookie HttpOnly.
import { session } from './session.svelte'

export class ApiError extends Error {
  constructor(
    message: string,
    public status: number,
  ) {
    super(message)
  }
}

async function request<T>(method: string, path: string, body?: unknown, signal?: AbortSignal): Promise<T> {
  const res = await fetch(`/admin/api${path}`, {
    method,
    signal,
    credentials: 'same-origin',
    headers: body === undefined ? {} : { 'content-type': 'application/json' },
    body: body === undefined ? undefined : JSON.stringify(body),
  })
  const text = await res.text()
  let data: unknown = null
  try {
    data = text ? JSON.parse(text) : null
  } catch {
    data = { error: text }
  }
  if (!res.ok) {
    if (res.status === 401 && path !== '/login') session.email = null
    const message = (data as { error?: string } | null)?.error ?? `HTTP ${res.status}`
    throw new ApiError(message, res.status)
  }
  return data as T
}

export const api = {
  get: <T>(path: string, options: { signal?: AbortSignal } = {}) =>
    request<T>('GET', path, undefined, options.signal),
  post: <T>(path: string, body: unknown = {}) => request<T>('POST', path, body),
  patch: <T>(path: string, body: unknown) => request<T>('PATCH', path, body),
  delete: <T>(path: string, body?: unknown) => request<T>('DELETE', path, body),
}

export const enc = encodeURIComponent

/** Requisição cancelada por uma mais nova (não é erro para o usuário). */
export const isAbort = (e: unknown) => e instanceof DOMException && e.name === 'AbortError'
