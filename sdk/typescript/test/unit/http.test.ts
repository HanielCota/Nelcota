import { afterEach, describe, expect, it, vi } from 'vitest';
import { createClient, NelcotaUsageError } from '../../src/index.js';
import { baseUrl, parseRetryAfter } from '../../src/core/http.js';
import { assertTokenAllowed } from '../../src/core/jwt.js';
import { json, jwt, mockFetch } from './helpers.js';

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

function client(mock: ReturnType<typeof mockFetch>, options: Parameters<typeof createClient>[1] = {}) {
  return createClient('https://api.example.com', { fetch: mock.fetch, auth: { autoRefresh: false }, ...options });
}

describe('retries', () => {
  it('retries reads on 429 honouring Retry-After', async () => {
    vi.useFakeTimers();
    const mock = mockFetch(json({ code: 'rate_limited', message: 'slow down' }, 429, { 'retry-after': '2' }), json([]));
    const pending = client(mock).from('todos').select().execute();
    await vi.advanceTimersByTimeAsync(1999);
    expect(mock.calls).toHaveLength(1);
    await vi.advanceTimersByTimeAsync(1);
    const { data, error } = await pending;
    expect(error).toBeNull();
    expect(data).toEqual([]);
    expect(mock.calls).toHaveLength(2);
  });

  it('gives up after the configured retries and reports retryAfter', async () => {
    const mock = mockFetch(json({ code: 'rate_limited', message: 'slow down' }, 429, { 'retry-after': '0' }));
    const { error } = await client(mock, { retries: 1 }).from('todos').select();
    expect(mock.calls).toHaveLength(2);
    expect(error).toMatchObject({ status: 429, code: 'rate_limited', retryAfter: 0 });
  });

  it('never retries writes', async () => {
    const mock = mockFetch(json({ code: 'unavailable', message: 'down' }, 503, { 'retry-after': '0' }));
    const { error } = await client(mock).from('todos').insert({ title: 'x' });
    expect(mock.calls).toHaveLength(1);
    expect(error?.code).toBe('unavailable');
  });

  it('retries reads after network errors, not after 4xx', async () => {
    let attempts = 0;
    const mock = mockFetch(() => {
      attempts++;
      if (attempts === 1) throw new TypeError('fetch failed');
      return json({ code: 'invalid_query', message: 'bad' }, 400);
    });
    const { error } = await client(mock).from('todos').select();
    expect(attempts).toBe(2);
    expect(error?.code).toBe('invalid_query');
  });

  it('parses both Retry-After forms', () => {
    expect(parseRetryAfter('7')).toBe(7);
    expect(parseRetryAfter(new Date(Date.now() + 3000).toUTCString())).toBeGreaterThanOrEqual(2);
    expect(parseRetryAfter('soon')).toBeUndefined();
    expect(parseRetryAfter(null)).toBeUndefined();
  });
});

describe('failures without a response', () => {
  it('reports network errors, timeouts and aborts with client codes', async () => {
    const down = mockFetch(() => {
      throw new TypeError('fetch failed');
    });
    expect((await client(down, { retries: 0 }).from('t').select()).error).toMatchObject({ status: 0, code: 'network_error' });

    const hanging = mockFetch(
      (request) =>
        new Promise<Response>((_, reject) => {
          void request;
          setTimeout(() => reject(new DOMException('timed out', 'TimeoutError')), 20);
        }),
    );
    expect((await client(hanging, { retries: 0, timeout: 10 }).from('t').select()).error?.code).toBe('timeout');

    const controller = new AbortController();
    controller.abort();
    const aborting = mockFetch(json([]));
    expect((await client(aborting).from('t').select().abortSignal(controller.signal)).error?.code).toBe('aborted');
  });

  it('keeps non-JSON error pages generic', async () => {
    const mock = mockFetch(new Response('<html>Bad Gateway</html>', { status: 502, statusText: 'Bad Gateway' }));
    expect((await client(mock).from('t').select()).error).toMatchObject({ status: 502, code: 'http_502', message: 'Bad Gateway' });
  });
});

describe('headers and tokens', () => {
  it('sends only the bearer token and the content type it needs', async () => {
    const mock = mockFetch(json([]));
    await client(mock, { accessToken: () => 'tok' }).from('t').insert({ a: 1 });
    const headers = [...mock.calls[0]!.headers.keys()].sort();
    expect(headers).toEqual(['authorization', 'content-type', 'prefer']);
    expect(mock.calls[0]!.headers.get('authorization')).toBe('Bearer tok');
  });

  it('sends no Authorization for visitors', async () => {
    const mock = mockFetch(json([]));
    await client(mock).from('t').select();
    expect(mock.calls[0]!.headers.has('authorization')).toBe(false);
  });

  it('refuses a service_role token in a browser, allows it on a server', () => {
    const token = jwt({ role: 'service_role' });
    expect(() => assertTokenAllowed(token, true)).toThrow(NelcotaUsageError);
    expect(() => assertTokenAllowed(token, false)).not.toThrow();
    expect(() => assertTokenAllowed(jwt({ role: 'authenticated' }), true)).not.toThrow();
  });

  it('refuses a service_role request when running in a browser', async () => {
    vi.stubGlobal('window', {});
    vi.stubGlobal('document', {});
    const mock = mockFetch(json([]));
    const nelcota = client(mock, { accessToken: () => jwt({ role: 'service_role', n: 1 }) });
    await expect(nelcota.from('t').select().execute()).rejects.toThrow(NelcotaUsageError);
    expect(mock.calls).toHaveLength(0);
  });

  it('validates the base URL', () => {
    expect(baseUrl('https://api.example.com/base/').href).toBe('https://api.example.com/base/');
    for (const url of ['ftp://x', 'not a url', 'https://u:p@x.com', 'https://x.com/?a=1', 'https://x.com/#f']) {
      expect(() => baseUrl(url), url).toThrow(NelcotaUsageError);
    }
  });

  it('keeps a base path', async () => {
    const mock = mockFetch(json([]));
    await createClient('https://example.com/nelcota/', { fetch: mock.fetch }).from('t').select();
    expect(mock.calls[0]!.url.pathname).toBe('/nelcota/rest/v1/t');
  });

  it('never shows tokens when errors are logged', async () => {
    const mock = mockFetch(json({ code: 'invalid_token', message: 'expired' }, 401));
    const { error } = await client(mock, { accessToken: () => 'secret-token' }).from('t').select();
    expect(JSON.stringify(error)).not.toContain('secret-token');
    expect(String(error)).not.toContain('secret-token');
  });
});
