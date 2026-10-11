// Panel API client (/admin/api). The session lives in an HttpOnly cookie.
import { session } from './features/auth/session.svelte'
import { responseContract } from './contracts'
import { t } from './i18n/index.svelte'
import { clearQuerySession, invalidateReads, queryRead } from './query-client'

/**
 * Failed request. `message` is the server's English text; `code` and `params`
 * let the panel show it in the chosen language (see `errorMessage` in i18n).
 */
export class ApiError extends Error {
  constructor(
    message: string,
    public status: number,
    public code?: string,
    public params?: Record<string, string | number>,
  ) {
    super(message)
  }
}

async function request<T>(method: string, path: string, body?: unknown, signal?: AbortSignal): Promise<T> {
  if (['/login', '/logout', '/sso'].includes(path)) clearQuerySession()
  const res = await fetch(`/admin/api${path}`, {
    method,
    signal,
    credentials: 'same-origin',
    headers: body === undefined ? {} : { 'content-type': 'application/json' },
    body: body === undefined ? undefined : JSON.stringify(body),
  })
  const result = await parseResponse<T>(res, path, method)
  return result
}

/** Sends a file as the raw request body (storage uploads). */
export async function uploadFile<T>(path: string, file: File, onprogress?: (loaded: number, total: number) => void): Promise<T> {
  const { upload } = await import('./upload')
  return upload(path, file, response => parseResponse<T>(response, path, 'POST'), onprogress)
}

export async function parseResponse<T>(res: Response, path: string, method: string): Promise<T> {
  const text = await res.text()
  let data: unknown = null
  let json = true
  try {
    data = text ? JSON.parse(text) : null
  } catch {
    json = false
    data = { error: text }
  }
  if (!res.ok) {
    if (res.status === 401 && path !== '/login') {
      clearQuerySession()
      if (session.email) session.expired = true
      session.email = null
    }
    // Not our JSON error (e.g. a reverse proxy's HTML 502 page): never show
    // the raw body, say what happened in the user's language instead.
    if (!json) throw httpError(res.status)
    const body = data as { error?: string; code?: string; params?: Record<string, string | number> } | null
    if (!body?.error && !body?.code) throw httpError(res.status)
    throw new ApiError(body?.error ?? `HTTP ${res.status}`, res.status, body?.code, body?.params)
  }
  const validate = responseContract(method, path)
  if (validate && !validate(data)) throw new ApiError(t('common.invalidResponse'), 502, 'invalid_response')
  if (method !== 'GET') invalidateReads()
  return data as T
}

/**
 * Failure without a panel error body. Gateway statuses mean the server
 * behind a proxy is down or restarting; anything else is unexpected.
 */
export function httpError(status: number): ApiError {
  const code = [502, 503, 504].includes(status) ? 'server_unreachable' : 'unexpected_response'
  return new ApiError(`HTTP ${status}`, status, code, { status })
}

export const api = {
  get: <T>(path: string, options: { signal?: AbortSignal; freshForMs?: number } = {}) =>
    path === '/session' ? request<T>('GET', path, undefined, options.signal)
      : queryRead(path, session.email, signal => request<T>('GET', path, undefined, signal), options.signal, options.freshForMs ?? 0),
  post: <T>(path: string, body: unknown = {}, options: { signal?: AbortSignal } = {}) =>
    request<T>('POST', path, body, options.signal),
  patch: <T>(path: string, body: unknown, options: { signal?: AbortSignal } = {}) =>
    request<T>('PATCH', path, body, options.signal),
  put: <T>(path: string, body: unknown, options: { signal?: AbortSignal } = {}) =>
    request<T>('PUT', path, body, options.signal),
  delete: <T>(path: string, body?: unknown) => request<T>('DELETE', path, body),
}

export const enc = encodeURIComponent

/** Request cancelled by a newer one (not an error for the user). */
export const isAbort = (e: unknown) => e instanceof DOMException && e.name === 'AbortError'
