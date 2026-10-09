import { afterEach, describe, expect, it, vi } from 'vitest';
import { createClient, memoryStorage, type AuthEvent, type SessionStorage } from '../../src/index.js';
import { challengeFor } from '../../src/auth/pkce.js';
import { empty, json, mockFetch, session } from './helpers.js';

afterEach(() => {
  vi.unstubAllGlobals();
});

const KEY = 'nelcota.api.example.com.session';

function setup(storage: SessionStorage = memoryStorage(), ...replies: Parameters<typeof mockFetch>) {
  const mock = mockFetch(...replies);
  const nelcota = createClient('https://api.example.com', {
    fetch: mock.fetch,
    auth: { storage, autoRefresh: false },
  });
  const events: AuthEvent[] = [];
  nelcota.auth.onChange((event) => events.push(event));
  return { nelcota, calls: mock.calls, storage, events };
}

function stored(value: unknown): SessionStorage {
  const storage = memoryStorage();
  void storage.setItem(KEY, JSON.stringify(value));
  return storage;
}

describe('password sign-in', () => {
  it('stores the session, emits signed_in and uses the token', async () => {
    const { nelcota, calls, storage, events } = setup(memoryStorage(), json(session()), json([]));
    const { data, error } = await nelcota.auth.signInWithPassword({ email: ' ana@example.com ', password: 'pw' });
    expect(error).toBeNull();
    expect(data?.access_token).toBe('access-1');
    expect(calls[0]!.url.search).toBe('?grant_type=password');
    expect(JSON.parse(calls[0]!.body!)).toEqual({ email: 'ana@example.com', password: 'pw' });
    expect(calls[0]!.headers.has('authorization')).toBe(false);
    expect(JSON.parse((await storage.getItem(KEY))!).refresh_token).toBe('refresh-1');
    expect(events).toEqual(['signed_in']);

    await nelcota.from('notes').select();
    expect(calls[1]!.headers.get('authorization')).toBe('Bearer access-1');
  });

  it('returns the server error and stores nothing', async () => {
    const { nelcota, storage } = setup(memoryStorage(), json({ code: 'invalid_credentials', message: 'no' }, 400));
    const { error } = await nelcota.auth.signInWithPassword({ email: 'a@x.com', password: 'bad' });
    expect(error).toMatchObject({ status: 400, code: 'invalid_credentials' });
    expect(await storage.getItem(KEY)).toBeNull();
  });

  it('sign-up without a session while the email awaits confirmation', async () => {
    const { nelcota, storage } = setup(memoryStorage(), json({ user: session().user }, 201));
    const { data } = await nelcota.auth.signUp({ email: 'ana@example.com', password: 'pw', data: { name: 'Ana' } });
    expect(data?.session).toBeNull();
    expect(data?.user.email).toBe('ana@example.com');
    expect(await storage.getItem(KEY)).toBeNull();
  });
});

describe('refresh', () => {
  it('refreshes a session about to expire before using it', async () => {
    const { nelcota, calls, events } = setup(stored(session(30)), json(session(900, 'refresh-2', 'access-2')), json([]));
    await nelcota.from('notes').select();
    expect(calls[0]!.url.search).toBe('?grant_type=refresh_token');
    expect(JSON.parse(calls[0]!.body!)).toEqual({ refresh_token: 'refresh-1' });
    expect(calls[1]!.headers.get('authorization')).toBe('Bearer access-2');
    expect(events).toEqual(['refreshed']);
  });

  it('sends one refresh for many concurrent requests', async () => {
    const { nelcota, calls } = setup(stored(session(30)), (request) =>
      request.url.pathname === '/auth/v1/token' ? json(session(900, 'refresh-2', 'access-2')) : json([]),
    );
    await Promise.all([1, 2, 3, 4, 5].map(() => nelcota.from('notes').select()));
    expect(calls.filter((c) => c.url.pathname === '/auth/v1/token')).toHaveLength(1);
    expect(calls.filter((c) => c.headers.get('authorization') === 'Bearer access-2')).toHaveLength(5);
  });

  it('serializes refreshes across tabs: the second tab takes the first one\'s result', async () => {
    // A Web Lock shared by two clients with the same storage (two tabs).
    let queue = Promise.resolve();
    vi.stubGlobal('navigator', {
      locks: {
        request: <T>(_name: string, task: () => Promise<T>) => {
          const run = queue.then(task);
          queue = run.then(
            () => undefined,
            () => undefined,
          );
          return run;
        },
      },
    });
    const storage = stored(session(30));
    let refreshes = 0;
    const reply = async (request: { url: URL }) => {
      if (request.url.pathname !== '/auth/v1/token') return json([]);
      refreshes++;
      await new Promise((r) => setTimeout(r, 20));
      return json(session(900, 'refresh-2', 'access-2'));
    };
    const tabA = setup(storage, reply);
    const tabB = setup(storage, reply);
    const [a, b] = await Promise.all([tabA.nelcota.auth.getSession(), tabB.nelcota.auth.getSession()]);
    expect(refreshes).toBe(1);
    expect(a.data?.access_token).toBe('access-2');
    expect(b.data?.access_token).toBe('access-2');
  });

  it('a rejected refresh token signs out', async () => {
    const { nelcota, storage, events } = setup(
      stored(session(30)),
      json({ code: 'invalid_grant', message: 'refresh token reused; session ended' }, 400),
    );
    expect(await nelcota.auth.getSession()).toEqual({ data: null, error: null });
    expect(await storage.getItem(KEY)).toBeNull();
    expect(events).toEqual(['signed_out']);
  });

  it('a transient failure keeps the session and still sends its token (never a silent visitor)', async () => {
    const { nelcota, calls, storage } = setup(stored(session(-5)), (request) => {
      if (request.url.pathname === '/auth/v1/token') throw new TypeError('fetch failed');
      return json({ code: 'invalid_token', message: 'expired' }, 401);
    });
    const { error } = await nelcota.from('notes').select();
    expect(error?.status).toBe(401);
    expect(calls.at(-1)!.headers.get('authorization')).toBe('Bearer access-1');
    expect(await storage.getItem(KEY)).not.toBeNull();
  });

  it('drops a corrupted stored session', async () => {
    const storage = memoryStorage();
    await storage.setItem(KEY, '{"access_token": 1}');
    const { nelcota } = setup(storage, json([]));
    expect((await nelcota.auth.getSession()).data).toBeNull();
    expect(await storage.getItem(KEY)).toBeNull();
  });

  it('refreshes in the background before expiry', async () => {
    vi.useFakeTimers();
    try {
      const mock = mockFetch(json(session(900, 'refresh-2', 'access-2')));
      const nelcota = createClient('https://api.example.com', {
        fetch: mock.fetch,
        auth: { storage: stored(session(120)), autoRefresh: true, refreshMargin: 60 },
      });
      await vi.advanceTimersByTimeAsync(50_000);
      expect(mock.calls).toHaveLength(0);
      await vi.advanceTimersByTimeAsync(11_000);
      expect(mock.calls).toHaveLength(1);
      expect((await nelcota.auth.getSession()).data?.access_token).toBe('access-2');
      nelcota.auth.dispose();
    } finally {
      vi.useRealTimers();
    }
  });
});

describe('sign-out and user', () => {
  it('revokes on the server and forgets locally even when the server fails', async () => {
    const ok = setup(stored(session()), empty(204));
    expect((await ok.nelcota.auth.signOut()).error).toBeNull();
    expect(ok.calls[0]!.url.pathname).toBe('/auth/v1/logout');
    expect(ok.calls[0]!.headers.get('authorization')).toBe('Bearer access-1');
    expect(await ok.storage.getItem(KEY)).toBeNull();
    expect(ok.events).toEqual(['signed_out']);

    const down = setup(stored(session()), json({ code: 'unavailable', message: 'down' }, 503));
    expect((await down.nelcota.auth.signOut()).error?.code).toBe('unavailable');
    expect(await down.storage.getItem(KEY)).toBeNull();
  });

  it('getUser updates the stored user', async () => {
    const changed = { ...session().user, email_confirmed_at: '2026-02-01T00:00:00Z' };
    const { nelcota, events, storage } = setup(stored(session()), json(changed));
    expect((await nelcota.auth.getUser()).data).toEqual(changed);
    expect(events).toEqual(['user_updated']);
    expect(JSON.parse((await storage.getItem(KEY))!).user.email_confirmed_at).toBe('2026-02-01T00:00:00Z');
    expect((await setup().nelcota.auth.getUser()).error?.code).toBe('session_missing');
  });
});

describe('OAuth with PKCE', () => {
  it('builds the authorize URL with an S256 challenge of a stored verifier', async () => {
    const { nelcota, storage } = setup();
    const { data } = await nelcota.auth.signInWithOAuth({ provider: 'github', redirectTo: 'https://app.example.com/back' });
    const url = new URL(data!.url);
    expect(url.origin + url.pathname).toBe('https://api.example.com/auth/v1/authorize');
    expect(url.searchParams.get('provider')).toBe('github');
    expect(url.searchParams.get('redirect_to')).toBe('https://app.example.com/back');
    expect(url.searchParams.get('code_challenge_method')).toBe('S256');
    const verifier = (await storage.getItem(`${KEY}.pkce`))!;
    expect(verifier).toMatch(/^[A-Za-z0-9_-]{64}$/);
    expect(url.searchParams.get('code_challenge')).toBe(await challengeFor(verifier));
  });

  it('exchanges the code once with the verifier, then forgets the verifier', async () => {
    const { nelcota, calls, storage } = setup(memoryStorage(), json(session()));
    await storage.setItem(`${KEY}.pkce`, 'v'.repeat(64));
    const { data } = await nelcota.auth.handleRedirect('https://app.example.com/back?code=abc&x=1');
    expect(data?.access_token).toBe('access-1');
    expect(calls[0]!.url.search).toBe('?grant_type=pkce');
    expect(JSON.parse(calls[0]!.body!)).toEqual({ auth_code: 'abc', code_verifier: 'v'.repeat(64) });
    expect(await storage.getItem(`${KEY}.pkce`)).toBeNull();
    expect((await nelcota.auth.exchangeCode('abc')).error?.code).toBe('pkce_missing');
  });

  it('reports provider errors and ignores unrelated URLs', async () => {
    const { nelcota, calls } = setup();
    expect((await nelcota.auth.handleRedirect('https://app.example.com/back?error=access_denied')).error?.code).toBe('access_denied');
    expect(await nelcota.auth.handleRedirect('https://app.example.com/other')).toEqual({ data: null, error: null });
    expect(calls).toHaveLength(0);
  });

  it('the S256 challenge is base64url(SHA-256(verifier)) without padding', async () => {
    const { createHash } = await import('node:crypto');
    for (const verifier of ['a'.repeat(43), 'dBjftJeZ4CVP-mJ92K-1yLtDy1Y-ry9Lm1uTJ3mMkI8', 'x_~.-'.repeat(20)]) {
      expect(await challengeFor(verifier)).toBe(createHash('sha256').update(verifier).digest('base64url'));
    }
  });
});

describe('email links', () => {
  it('reads the fragment and cleans the address bar of the current page', () => {
    const href = 'https://app.example.com/new-password#type=recovery&token=tok_123';
    const replaceState = vi.fn();
    vi.stubGlobal('window', {});
    vi.stubGlobal('document', {});
    vi.stubGlobal('location', { href });
    vi.stubGlobal('history', { state: null, replaceState });
    const { nelcota } = setup();
    expect(nelcota.auth.readEmailLink()).toEqual({ type: 'recovery', token: 'tok_123' });
    expect(replaceState).toHaveBeenCalledWith(null, '', 'https://app.example.com/new-password');
  });

  it('ignores fragments that are not email links', () => {
    const { nelcota } = setup();
    for (const href of ['https://a.com/#type=admin&token=x', 'https://a.com/#token=x', `https://a.com/#type=signup&token=${'x'.repeat(129)}`]) {
      expect(nelcota.auth.readEmailLink(href), href).toBeNull();
    }
  });

  it('verifies links and resets passwords', async () => {
    const { nelcota, calls } = setup(memoryStorage(), json(session()));
    await nelcota.auth.verifyEmailLink({ type: 'magiclink', token: 't1' });
    await nelcota.auth.resetPassword({ token: 't2', password: 'new-password' });
    expect(calls.map((c) => JSON.parse(c.body!))).toEqual([
      { type: 'magiclink', token: 't1' },
      { type: 'recovery', token: 't2', password: 'new-password' },
    ]);
  });

  it('asks for emails without revealing whether the account exists', async () => {
    const { nelcota, calls } = setup(memoryStorage(), json({}));
    expect(await nelcota.auth.requestPasswordReset('a@x.com')).toEqual({ data: null, error: null });
    await nelcota.auth.sendMagicLink('a@x.com');
    await nelcota.auth.resendConfirmation('a@x.com');
    expect(calls.map((c) => [c.url.pathname, JSON.parse(c.body!)])).toEqual([
      ['/auth/v1/recover', { email: 'a@x.com' }],
      ['/auth/v1/magiclink', { email: 'a@x.com' }],
      ['/auth/v1/resend', { type: 'signup', email: 'a@x.com' }],
    ]);
  });
});
