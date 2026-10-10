/** One owner for session persistence, lifecycle transitions and refresh timers. */
import { clientError, fail, ok, type Result } from '../core/errors.js';
import { isBrowser } from '../core/jwt.js';
import { Broadcast, withLock } from './coordination.js';
import { isSession, memoryStorage, parseSession, secondsLeft, type Session, type SessionStorage, type User } from './session.js';

export type AuthEvent = 'signed_in' | 'signed_out' | 'refreshed' | 'user_updated';
export type AuthListener = (event: AuthEvent, session: Session | null) => void;
export interface AuthOptions {
  /** Default: localStorage in browsers, memory elsewhere. */
  storage?: SessionStorage;
  /** Default: nelcota.<host><base-path>.session. */
  storageKey?: string;
  /** Refresh in the background. Default: on in browsers. */
  autoRefresh?: boolean;
  /** Refresh when fewer than this many seconds remain. Default 60. */
  refreshMargin?: number;
}
export interface AuthRequestOptions {
  signal?: AbortSignal;
  timeout?: number;
}
const RETRY_REFRESH_MS = 10_000;

function browserStorage(): SessionStorage | undefined {
  try { return isBrowser() ? globalThis.localStorage : undefined; }
  catch { return undefined; }
}

export class SessionManager {
  // Shared with the OAuth verifier store, using a distinct key suffix.
  readonly storage: SessionStorage;
  readonly key: string;
  readonly #margin: number;
  readonly #autoRefresh: boolean;
  readonly #listeners = new Set<AuthListener>();
  readonly #broadcast: Broadcast | undefined;
  #session: Session | null | undefined;
  #refreshing: Promise<Result<Session>> | null = null;
  #timer: ReturnType<typeof setTimeout> | undefined;
  #disposed = false;

  constructor(url: URL, options: AuthOptions, private rotate: (token: string, options: AuthRequestOptions) => Promise<Result<Session>>) {
    this.storage = options.storage ?? browserStorage() ?? memoryStorage();
    const path = url.pathname.replace(/\/+$/, '');
    this.key = options.storageKey ?? `nelcota.${url.host}${path}.session`;
    this.#margin = options.refreshMargin ?? 60;
    this.#autoRefresh = options.autoRefresh ?? isBrowser();
    if (isBrowser()) this.#broadcast = new Broadcast(`${this.key}.channel`, () => void this.#reload());
    if (this.#autoRefresh) void this.current().then(session => this.#schedule(session));
  }

  onChange(listener: AuthListener): () => void {
    this.#listeners.add(listener);
    return () => this.#listeners.delete(listener);
  }
  #emit(event: AuthEvent, session: Session | null): void {
    for (const listener of this.#listeners) {
      try { listener(event, session); }
      catch (error) { queueMicrotask(() => { throw error; }); }
    }
  }
  #locked<T>(task: () => Promise<T>): Promise<T> {
    // Keep the existing name to coordinate with other clients/tabs as well.
    return withLock(`${this.key}.refresh`, task);
  }
  async #read(force = false): Promise<Session | null> {
    if (!force && this.#session !== undefined) return this.#session;
    const stored = await this.storage.getItem(this.key);
    this.#session = parseSession(stored);
    if (stored !== null && this.#session === null) await this.storage.removeItem(this.key);
    return this.#session;
  }
  current(): Promise<Session | null> { return this.#locked(() => this.#read()); }
  async #reload(): Promise<void> {
    await this.#locked(async () => {
      const before = this.#session;
      const after = await this.#read(true);
      this.#schedule(after);
      if (after === null && before) this.#emit('signed_out', null);
      else if (after && !before) this.#emit('signed_in', after);
      else if (after) this.#emit('refreshed', after);
    });
  }
  async #write(session: Session | null, event: AuthEvent): Promise<void> {
    this.#session = session;
    if (session) await this.storage.setItem(this.key, JSON.stringify(session));
    else await this.storage.removeItem(this.key);
    this.#broadcast?.notify();
    this.#schedule(session);
    this.#emit(event, session);
  }
  set(session: Session): Promise<void> {
    if (!isSession(session)) throw new TypeError('Not a Nelcota session');
    return this.#locked(() => this.#write(session, 'signed_in'));
  }
  authenticate<T>(request: () => Promise<Result<T>>, sessionOf: (data: T) => Session | null): Promise<Result<T>> {
    return this.#locked(async () => {
      const result = await request();
      if (!result.error) {
        const session = sessionOf(result.data);
        if (session) await this.#write(session, 'signed_in');
      }
      return result;
    });
  }
  async get(options: AuthRequestOptions = {}): Promise<Result<Session | null>> {
    const session = await this.current();
    if (!session || secondsLeft(session) > this.#margin) return ok(session);
    const refreshed = await this.refresh(options);
    if (refreshed.error?.code === 'invalid_grant' || refreshed.error?.code === 'session_missing') return ok(null);
    return refreshed;
  }
  async accessToken(): Promise<string | null> {
    const result = await this.get();
    return result.data?.access_token ?? this.#session?.access_token ?? null;
  }
  refresh(options: AuthRequestOptions = {}): Promise<Result<Session>> {
    this.#refreshing ??= (async () => {
      const known = await this.current();
      if (!known) return fail(clientError('session_missing', 'Not signed in'));
      return this.#locked(async () => {
        const current = await this.#read(true);
        if (!current) return fail(clientError('session_missing', 'Not signed in'));
        if (current.refresh_token !== known.refresh_token && secondsLeft(current) > this.#margin) return ok(current);
        const result = await this.rotate(current.refresh_token, options);
        if (result.error) {
          if (result.error.code === 'invalid_grant') await this.#write(null, 'signed_out');
          return result;
        }
        await this.#write(result.data, 'refreshed');
        return result;
      });
    })().finally(() => { this.#refreshing = null; });
    return this.#refreshing;
  }
  updateUser(snapshot: Session, user: User): Promise<void> {
    return this.#locked(async () => {
      const current = await this.#read(true);
      if (current?.access_token === snapshot.access_token && current.refresh_token === snapshot.refresh_token
          && JSON.stringify(current.user) !== JSON.stringify(user)) {
        await this.#write({ ...current, user }, 'user_updated');
      }
    });
  }
  signOut(revoke: (session: Session) => Promise<Result<null>>): Promise<Result<null>> {
    return this.#locked(async () => {
      const current = await this.#read(true);
      const result = current ? await revoke(current) : ok(null);
      if (current) await this.#write(null, 'signed_out');
      else { this.#session = null; this.#schedule(null); }
      return result;
    });
  }
  #schedule(session: Session | null, delayMs?: number): void {
    clearTimeout(this.#timer);
    this.#timer = undefined;
    if (this.#disposed || !this.#autoRefresh || !session) return;
    const due = delayMs ?? Math.max(0, (secondsLeft(session) - this.#margin) * 1000 - Math.random() * 5000);
    this.#timer = setTimeout(() => {
      void this.refresh().then(({ error }) => {
        if (error && error.code !== 'invalid_grant' && error.code !== 'session_missing') {
          void this.current().then(current => this.#schedule(current, RETRY_REFRESH_MS));
        }
      });
    }, Math.min(due, 2 ** 31 - 1));
    (this.#timer as { unref?: () => void }).unref?.();
  }
  dispose(): void {
    this.#disposed = true;
    clearTimeout(this.#timer);
    this.#broadcast?.close();
    this.#listeners.clear();
  }
}
