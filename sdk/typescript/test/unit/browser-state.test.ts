import { afterEach, expect, it, vi } from 'vitest';
import { IDBFactory } from 'fake-indexeddb';
import { createClient } from '../../src/index.js';
import { BrowserSessionState } from '../../src/auth/browser-state.js';
import { empty, json, mockFetch, session } from './helpers.js';

const KEY = 'nelcota.api.example.com.session';
afterEach(() => vi.unstubAllGlobals());

function browser() {
  const original = JSON.stringify(session(5));
  // A different WebKit process can retain its native storage snapshot even
  // after Web Locks has handed ownership to the next tab. No timer fixes it.
  const storage = { getItem: () => original, setItem: vi.fn(), removeItem: vi.fn() };
  const factory = new IDBFactory();
  vi.stubGlobal('window', {});
  vi.stubGlobal('document', {});
  vi.stubGlobal('BroadcastChannel', undefined);
  vi.stubGlobal('localStorage', storage);
  vi.stubGlobal('indexedDB', factory);
  let queue: Promise<unknown> = Promise.resolve();
  vi.stubGlobal('navigator', { locks: {
    request: <T>(_name: string, callback: () => Promise<T>) => {
      const result = queue.then(callback);
      queue = result.then(() => undefined, () => undefined);
      return result;
    },
  } });
  return { storage, factory };
}

it('hands the committed replacement to a queued tab with a stale native snapshot', async () => {
  browser();
  const mock = mockFetch(json(session(900, 'replacement-1', 'access-1')), json(session(900, 'replacement-2', 'access-2')));
  const clients = [0, 1].map(() => createClient('https://api.example.com', {
    fetch: mock.fetch, auth: { autoRefresh: false },
  }));
  try {
    const values = await Promise.all(clients.map(client => client.auth.getSession()));
    expect(mock.calls).toHaveLength(1);
    expect(values.map(value => value.data?.refresh_token)).toEqual(['replacement-1', 'replacement-1']);
  } finally { clients.forEach(client => client.dispose()); }
});

it('does not resurrect a signed-out session from a stale native mirror or a reopened client', async () => {
  browser();
  const mock = mockFetch(empty());
  const first = createClient('https://api.example.com', { fetch: mock.fetch, auth: { autoRefresh: false } });
  const other = createClient('https://api.example.com', { fetch: mock.fetch, auth: { autoRefresh: false } });
  try {
    await first.auth.setSession(session());
    expect((await other.auth.getSession()).data).not.toBeNull();
    await first.auth.signOut();
    first.dispose();
    expect((await other.auth.getSession()).data).toBeNull();
    expect((await other.auth.refreshSession()).error?.code).toBe('session_missing');
    expect(mock.calls).toHaveLength(1);
    const reopened = createClient('https://api.example.com', { fetch: mock.fetch, auth: { autoRefresh: false } });
    try { expect((await reopened.auth.getSession()).data).toBeNull(); }
    finally { reopened.dispose(); }
  } finally { first.dispose(); other.dispose(); }
});

it('retains a received replacement and blocks another owner after its commit fails', async () => {
  const { factory } = browser();
  const failed = new Error('IndexedDB write failed');
  const replacement = session(900, 'replacement', 'access-new');
  const mock = mockFetch(() => {
    vi.spyOn(factory, 'open').mockImplementationOnce(() => { throw failed; });
    return json(replacement);
  });
  const first = createClient('https://api.example.com', { fetch: mock.fetch, auth: { autoRefresh: false } });
  const other = createClient('https://api.example.com', { fetch: mock.fetch, auth: { autoRefresh: false } });
  try {
    await expect(first.auth.getSession()).rejects.toBe(failed);
    expect((await first.auth.getSession()).data?.refresh_token).toBe('replacement');
    await expect(other.auth.getSession()).rejects.toThrow('interrupted');
    expect(mock.calls).toHaveLength(1);
    await first.auth.setSession(replacement);
    expect((await other.auth.getSession()).data?.refresh_token).toBe('replacement');
    expect(mock.calls).toHaveLength(1);
  } finally { first.dispose(); other.dispose(); }
});

it('lets another tab sign out an interrupted refresh and supersedes the dirty owner', async () => {
  const { factory } = browser();
  const mock = mockFetch(() => {
    vi.spyOn(factory, 'open').mockImplementationOnce(() => { throw new Error('commit failed'); });
    return json(session(900, 'replacement', 'new-access'));
  }, empty());
  const clients = [0, 1].map(() => createClient('https://api.example.com', { fetch: mock.fetch, auth: { autoRefresh: false } }));
  try {
    await expect(clients[0]!.auth.getSession()).rejects.toThrow('commit failed');
    await clients[1]!.auth.signOut();
    expect((await clients[0]!.auth.getSession()).data).toBeNull();
    expect((await clients[1]!.auth.getSession()).data).toBeNull();
    expect(mock.calls.map(call => call.url.pathname)).toEqual(['/auth/v1/token', '/auth/v1/logout']);
  } finally { clients.forEach(client => client.dispose()); }
});

it('does not send a refresh when the reservation cannot commit', async () => {
  const { factory } = browser();
  const mock = mockFetch(json(session()));
  const client = createClient('https://api.example.com', { fetch: mock.fetch, auth: { autoRefresh: false } });
  try {
    // Seed a fresh session, then fail the transaction that reserves rotation.
    await client.auth.setSession(session());
    const open = factory.open.bind(factory);
    let calls = 0;
    vi.spyOn(factory, 'open').mockImplementation((...args) => {
      if (++calls === 3) throw new Error('reservation failed');
      return open(...args);
    });
    await expect(client.auth.refreshSession()).rejects.toThrow('reservation failed');
    expect(mock.calls).toHaveLength(0);
  } finally { client.dispose(); }
});

it('keeps different API namespaces isolated in the shared database', async () => {
  browser();
  const clients = ['/one', '/two'].map(path => createClient(`https://api.example.com${path}`, { auth: { autoRefresh: false } }));
  try {
    await clients[0]!.auth.setSession(session(900, 'one', 'one-access'));
    await clients[1]!.auth.setSession(session(900, 'two', 'two-access'));
    expect((await clients[0]!.auth.getSession()).data?.refresh_token).toBe('one');
    expect((await clients[1]!.auth.getSession()).data?.refresh_token).toBe('two');
  } finally { clients.forEach(client => client.dispose()); }
});

it('rejects a late refresh completion after another owner signed out', async () => {
  const { factory } = browser();
  const first = new BrowserSessionState(KEY, factory);
  const second = new BrowserSessionState(KEY, factory);
  const original = session(5);
  await first.read(() => JSON.stringify(original));
  await first.reserve(original);
  await second.write(null);
  await expect(first.write(JSON.stringify(session()), true)).rejects.toThrow('changed during refresh');
  expect((await first.read(() => JSON.stringify(original))).value).toBeNull();
});

it('does not emit a replacement session when a late response conflicts with logout', async () => {
  const { factory } = browser();
  let started!: () => void;
  let complete!: () => void;
  const pending = new Promise<void>(resolve => { started = resolve; });
  const response = new Promise<void>(resolve => { complete = resolve; });
  const mock = mockFetch(async () => {
    started();
    await response;
    return json(session(900, 'late-replacement', 'new-access'));
  });
  const client = createClient('https://api.example.com', { fetch: mock.fetch, auth: { autoRefresh: false } });
  const events: unknown[] = [];
  client.auth.onChange((event, value) => { events.push({ event, value }); });
  try {
    const refresh = client.auth.getSession();
    const rejected = expect(refresh).rejects.toThrow('changed during refresh');
    await pending;
    // An independent owner without a shared Web Lock can commit logout.
    await new BrowserSessionState(KEY, factory).write(null);
    complete();
    await rejected;
    expect(events).toEqual([{ event: 'signed_out', value: null }]);
    expect((await client.auth.getSession()).data).toBeNull();
  } finally { complete(); client.dispose(); }
});

it('can reuse the original session on the same owner after an uncertain network outcome', async () => {
  browser();
  const mock = mockFetch(json({ code: 'gateway_error', message: 'unknown outcome' }, 503), json(session(900, 'replacement', 'new-access')));
  const client = createClient('https://api.example.com', { fetch: mock.fetch, auth: { autoRefresh: false } });
  try {
    expect((await client.auth.getSession()).error).not.toBeNull();
    expect((await client.auth.refreshSession()).data?.refresh_token).toBe('replacement');
    expect(mock.calls).toHaveLength(2);
  } finally { client.dispose(); }
});

it('rejects malformed receipts and closes storage connections for deletion', async () => {
  const { factory } = browser();
  const state = new BrowserSessionState(KEY, factory);
  await state.read(() => JSON.stringify(session()));
  await new Promise<void>((resolve, reject) => {
    const request = factory.open('nelcota.auth', 1);
    request.onsuccess = () => {
      const database = request.result;
      const transaction = database.transaction('sessions', 'readwrite');
      transaction.objectStore('sessions').put({ kind: 'ready', owner: 123, value: 'bad' }, KEY);
      transaction.oncomplete = () => { database.close(); resolve(); };
      transaction.onabort = () => { database.close(); reject(transaction.error); };
    };
    request.onerror = () => reject(request.error);
  });
  await expect(state.read(() => JSON.stringify(session()))).rejects.toThrow('Invalid browser session receipt');
  await new Promise<void>((resolve, reject) => {
    const request = factory.deleteDatabase('nelcota.auth');
    request.onsuccess = () => resolve();
    request.onerror = () => reject(request.error);
    request.onblocked = () => reject(new Error('Leaked browser session connection'));
  });
});

it('reports an unavailable or blocked database without sending auth requests', async () => {
  const { factory } = browser();
  await expect(new BrowserSessionState(KEY, undefined).read(() => JSON.stringify(session()))).rejects.toThrow('IndexedDB is required');
  const blocked = {} as IDBOpenDBRequest;
  vi.spyOn(factory, 'open').mockImplementationOnce(() => {
    setTimeout(() => blocked.onblocked?.(new Event('blocked') as IDBVersionChangeEvent), 0);
    return blocked;
  });
  await expect(new BrowserSessionState(KEY, factory).read(() => JSON.stringify(session()))).rejects.toThrow('storage is blocked');
});

it('closes the database when starting a storage request throws synchronously', async () => {
  const { factory } = browser();
  // IndexedDB rejects NaN keys synchronously rather than firing request.onerror.
  const state = new BrowserSessionState(Number.NaN as unknown as string, factory);
  await expect(state.read(() => JSON.stringify(session()))).rejects.toThrow();
  await new Promise<void>((resolve, reject) => {
    const request = factory.deleteDatabase('nelcota.auth');
    request.onsuccess = () => resolve();
    request.onerror = () => reject(request.error);
    request.onblocked = () => reject(new Error('Leaked browser session connection'));
  });
});
