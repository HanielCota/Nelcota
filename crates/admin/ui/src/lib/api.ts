// Panel API client (/admin/api). The session lives in an HttpOnly cookie.
import { session } from './session.svelte'
import { responseContract } from './contracts'
import { t } from './i18n/index.svelte'

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
  const res = await fetch(`/admin/api${path}`, {
    method,
    signal,
    credentials: 'same-origin',
    headers: body === undefined ? {} : { 'content-type': 'application/json' },
    body: body === undefined ? undefined : JSON.stringify(body),
  })
  return parse<T>(res, path, method)
}

/** Sends a file as the raw request body (storage uploads). */
export async function uploadFile<T>(path: string, file: File, onprogress?: (loaded: number, total: number) => void): Promise<T> {
  if (onprogress) {
    return new Promise<T>((resolve, reject) => {
      const request = new XMLHttpRequest()
      request.open('POST', `/admin/api${path}`)
      request.withCredentials = true
      request.setRequestHeader('content-type', file.type || 'application/octet-stream')
      request.upload.onprogress = (event) => onprogress(event.loaded, event.lengthComputable ? event.total : file.size)
      request.onerror = () => reject(new Error(t('common.requestFailed')))
      request.onabort = () => reject(new DOMException('Aborted', 'AbortError'))
      request.onload = () => {
        Promise.resolve().then(() => parse<T>(new Response([204, 205, 304].includes(request.status) ? null : request.responseText, { status: request.status }), path, 'POST')).then(resolve, reject)
      }
      request.send(file)
    })
  }
  const res = await fetch(`/admin/api${path}`, {
    method: 'POST',
    credentials: 'same-origin',
    headers: { 'content-type': file.type || 'application/octet-stream' },
    body: file,
  })
  return parse<T>(res, path, 'POST')
}

async function parse<T>(res: Response, path: string, method: string): Promise<T> {
  const text = await res.text()
  let data: unknown = null
  try {
    data = text ? JSON.parse(text) : null
  } catch {
    data = { error: text }
  }
  if (!res.ok) {
    if (res.status === 401 && path !== '/login') session.email = null
    const body = data as { error?: string; code?: string; params?: Record<string, string | number> } | null
    throw new ApiError(body?.error ?? `HTTP ${res.status}`, res.status, body?.code, body?.params)
  }
  const validate = responseContract(method, path)
  if (validate && !validate(data)) throw new ApiError(t('common.invalidResponse'), 502, 'invalid_response')
  return data as T
}

export const api = {
  get: <T>(path: string, options: { signal?: AbortSignal } = {}) =>
    request<T>('GET', path, undefined, options.signal),
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
