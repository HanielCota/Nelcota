import { afterEach, describe, expect, it, vi } from 'vitest'
import { api, ApiError, parseResponse } from './api'
import { errorMessage, i18n } from './i18n/index.svelte'
import { session } from './features/auth/session.svelte'

function respond(body: string, status: number) {
  vi.stubGlobal('fetch', vi.fn(async () => new Response(body, { status })))
}

async function failure(): Promise<ApiError> {
  try {
    await api.get('/anything')
  } catch (e) {
    return e as ApiError
  }
  throw new Error('the request did not fail')
}

afterEach(() => vi.unstubAllGlobals())

describe('JSON request values', () => {
  it.each([NaN, Infinity, -Infinity, new Number(NaN)])('refuses non-finite input %s instead of clearing a nullable setting', async value => {
    const fetch = vi.fn()
    vi.stubGlobal('fetch', fetch)
    await expect(api.put('/storage/buckets/documents', { file_size_limit: value })).rejects.toThrow()
    await expect(api.post('/storage/buckets', { nested: [value] })).rejects.toThrow()
    expect(fetch).not.toHaveBeenCalled()
  })

  it('still sends an explicit null to clear a setting', async () => {
    const fetch = vi.fn(async (_input: RequestInfo | URL, _init?: RequestInit) => new Response('{}', { status: 200 }))
    vi.stubGlobal('fetch', fetch)
    await api.put('/anything', { file_size_limit: null })
    expect(fetch.mock.calls[0]?.[1]).toMatchObject({ body: '{"file_size_limit":null}' })
  })
})

describe('error responses', () => {
  it.each([undefined, '1', '1000000'])('cancels oversized proxy errors with length %s', async length => {
    const cancel = vi.fn()
    let chunks = 0
    const pull = vi.fn((controller: ReadableStreamDefaultController<Uint8Array>) => {
      if (chunks++ === 10) { controller.close(); return }
      controller.enqueue(new Uint8Array(8192))
    })
    const body = new ReadableStream({ pull, cancel }, { highWaterMark: 0 })
    const headers = length ? { 'content-length': length } : undefined
    vi.stubGlobal('fetch', vi.fn(async () => new Response(body, { status: 502, headers })))
    const error = await failure()
    expect(error).toBeInstanceOf(ApiError)
    expect(error).toMatchObject({ status: 502, code: 'server_unreachable' })
    expect(cancel).toHaveBeenCalledOnce()
    expect(pull.mock.calls.length).toBe(length === '1000000' ? 0 : 9)
    expect(body.locked).toBe(false)
  })

  it('preserves a valid error at the exact byte boundary', async () => {
    const error = JSON.stringify({ error: 'ação', code: 'db_error' })
    respond(error + ' '.repeat(65536 - new TextEncoder().encode(error).length), 409)
    expect(await failure()).toMatchObject({ status: 409, code: 'db_error', message: 'ação' })
  })

  it('an oversized 401 still expires the local session', async () => {
    session.email = 'admin@example.com'
    session.expired = false
    // The query cache cancels callers on sign-out; inspect the parser's
    // HTTP failure directly while still exercising the session boundary.
    await expect(parseResponse(new Response(' '.repeat(65537), { status: 401 }), '/anything', 'GET'))
      .rejects.toMatchObject({ status: 401 })
    expect(session.email).toBeNull()
    expect(session.expired).toBe(true)
  })

  it('keeps the code and text of a panel error', async () => {
    respond(JSON.stringify({ error: 'bucket not found', code: 'bucket_not_found' }), 404)
    const error = await failure()
    expect(error).toBeInstanceOf(ApiError)
    expect(error.code).toBe('bucket_not_found')
    expect(error.message).toBe('bucket not found')
  })

  it('never shows a proxy HTML page; a gateway status reads as unreachable', async () => {
    respond('<html><body><h1>502 Bad Gateway</h1></body></html>', 502)
    const error = await failure()
    expect(error.status).toBe(502)
    expect(error.code).toBe('server_unreachable')
    i18n.locale = 'en'
    expect(errorMessage(error)).toContain('HTTP 502')
    expect(errorMessage(error)).not.toContain('<html>')
  })

  it('a 401 signs out and flags an expired session only when signed in', async () => {
    const unauthorized = JSON.stringify({ error: 'session expired', code: 'session_expired' })
    session.email = undefined
    session.expired = false
    respond(unauthorized, 401)
    await failure()
    expect(session.email).toBeNull()
    expect(session.expired).toBe(false)

    session.email = 'admin@example.com'
    respond(unauthorized, 401)
    await failure()
    expect(session.email).toBeNull()
    expect(session.expired).toBe(true)
  })

  it('other non-JSON or empty bodies read as an unexpected response', async () => {
    respond('Payload Too Large', 413)
    expect((await failure()).code).toBe('unexpected_response')
    respond('', 500)
    expect((await failure()).code).toBe('unexpected_response')
  })
})
