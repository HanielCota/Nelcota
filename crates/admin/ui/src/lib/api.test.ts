import { afterEach, describe, expect, it, vi } from 'vitest'
import { api, ApiError } from './api'
import { errorMessage, i18n } from './i18n/index.svelte'

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

describe('error responses', () => {
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

  it('other non-JSON or empty bodies read as an unexpected response', async () => {
    respond('Payload Too Large', 413)
    expect((await failure()).code).toBe('unexpected_response')
    respond('', 500)
    expect((await failure()).code).toBe('unexpected_response')
  })
})
