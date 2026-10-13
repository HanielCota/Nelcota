/** One owner for session persistence, lifecycle transitions and refresh timers. */
import { NelcotaUsageError, clientError, fail, ok, type Result } from '../core/errors.js';
import { abortable } from '../core/cancellation.js';
import { isBrowser } from '../core/jwt.js';
import { encodeJson } from '../core/json.js';
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
  /** Refresh when fewer than this many seconds remain, capped at half the token lifetime. Default 60. */
  refreshMargin?: number;
  /** Unexpected storage/adapter failures in background auth. Automatic refresh pauses until a session is written successfully. */
  onBackgroundError?: (error: unknown) => void | Promise<void>;
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
  readonly #onBackgroundError: AuthOptions['onBackgroundError'];
  #session: Session | null | undefined;
  #dirty = false;
  #refreshing: Promise<Result<Session>> | null = null;
  #timer: ReturnType<typeof setTimeout> | undefined;
  #disposed = false;

  constructor(url: URL, options: AuthOptions, private rotate: (token: string, options: AuthRequestOptions) => Promise<Result<Session>>) {
    const margin = options.refreshMargin ?? 60;
    if (!Number.isFinite(margin) || margin < 0) throw new NelcotaUsageError('refreshMargin must be a finite non-negative number');
    this.storage = options.storage ?? browserStorage() ?? memoryStorage();
    const pathname = url.pathname;
    let end = pathname.length;
    while (end > 0 && pathname[end - 1] === '/') end--;
    const path = pathname.slice(0, end);
    this.key = options.storageKey ?? `nelcota.${url.host}${path}.session`;
    this.#margin = margin;
    this.#autoRefresh = options.autoRefresh ?? isBrowser();
    this.#onBackgroundError = options.onBackgroundError;
    if (isBrowser()) this.#broadcast = new Broadcast(`${this.key}.channel`, () => this.#background(this.#reload()));
    if (this.#autoRefresh) this.#background(this.current().then(session => this.#schedule(session)));
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
    // Storage may still contain a spent token or a session already signed out.
    // Keep the newer local state until a later persistence attempt succeeds.
    if (this.#dirty) return this.#session ?? null;
    if (!force && this.#session !== undefined) return this.#session;
    // Web Locks and native storage use separate delivery queues. Leave the
    // grant task before reading the previous owner's storage write.
    if (this.storage === browserStorage()) await new Promise<void>(resolve => setTimeout(resolve, 0));
    const stored = await this.storage.getItem(this.key);
    this.#session = parseSession(stored);
    if (stored !== null && this.#session === null) await this.storage.removeItem(this.key);
    return this.#session;
  }
  current(): Promise<Session | null> { return this.#locked(() => this.#read()); }
  #marginFor(session: Session): number {
    // A short-lived token or a large configured margin must not make every
    // request (or every background tick) rotate a freshly issued token.
    return Math.min(this.#margin, session.expires_in / 2);
  }
  async #reload(): Promise<void> {
    await this.#locked(async () => {
      if (this.#dirty) return;
      const before = this.#session;
      const after = await this.#read(true);
      this.#schedule(after);
      if (after === null) {
        if (before) this.#emit('signed_out', null);
        return;
      }
      this.#emit(before ? 'refreshed' : 'signed_in', after);
    });
  }
  async #write(session: Session | null, event: AuthEvent): Promise<void> {
    const encoded = encodeJson(session);
    this.#session = session;
    this.#dirty = true;
    this.#schedule(null);
    try {
      await this.#persist(encoded);
      this.#schedule(session);
    }
    finally {
      this.#emit(event, session);
    }
  }
  async #persist(encoded?: string): Promise<void> {
    const session = this.#session;
    await (session ? this.storage.setItem(this.key, encoded ?? encodeJson(session)) : this.storage.removeItem(this.key));
    // Finish this owner's native write task before handing the lock over.
    if (this.storage === browserStorage()) await new Promise<void>(resolve => setTimeout(resolve, 0));
    this.#dirty = false;
    this.#broadcast?.notify();
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
    if (options.signal?.aborted) return fail(clientError('aborted', 'The request was aborted'));
    const current = await abortable(this.current(), options.signal);
    if (current.error) return fail(current.error);
    const session = current.data;
    if (!session || secondsLeft(session) > this.#marginFor(session)) return ok(session);
    const refreshed = await this.refresh(options, session);
    if (refreshed.error?.code === 'invalid_grant' || refreshed.error?.code === 'session_missing') return ok(null);
    return refreshed;
  }
  async accessToken(): Promise<string | null> {
    const result = await this.get();
    return result.data?.access_token ?? this.#session?.access_token ?? null;
  }
  refresh(options: AuthRequestOptions = {}, snapshot?: Session): Promise<Result<Session>> {
    if (options.signal?.aborted) return Promise.resolve(fail(clientError('aborted', 'The request was aborted')));
    this.#refreshing ??= (async () => {
      // A broadcast can reload a replacement after get() observed expiry.
      // Compare with that original session instead of rotating its replacement.
      const known = snapshot ?? await this.current();
      if (!known) return fail(clientError('session_missing', 'Not signed in'));
      return this.#locked(async () => {
        const current = await this.#read(true);
        if (!current) return fail(clientError('session_missing', 'Not signed in'));
        if (current.refresh_token !== known.refresh_token && secondsLeft(current) > this.#marginFor(current)) return ok(current);
        // Once started, rotation owns its lifetime. A cancelled waiter must
        // not discard the replacement for a token already spent by the server.
        const result = await this.rotate(current.refresh_token, options.timeout === undefined ? {} : { timeout: options.timeout });
        if (result.error) {
          if (result.error.code === 'invalid_grant') await this.#write(null, 'signed_out');
          return result;
        }
        await this.#write(result.data, 'refreshed');
        return result;
      });
    })().finally(() => { this.#refreshing = null; });
    return abortable(this.#refreshing, options.signal).then(result => result.error ? fail(result.error) : result.data);
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
      if (!current) {
        this.#session = null;
        if (this.#dirty) await this.#persist();
        this.#schedule(null);
        return ok(null);
      }
      const result = await revoke(current);
      await this.#write(null, 'signed_out');
      return result;
    });
  }
  #schedule(session: Session | null, delayMs?: number): void {
    clearTimeout(this.#timer);
    this.#timer = undefined;
    if (this.#disposed || !this.#autoRefresh || !session) return;
    const remaining = secondsLeft(session);
    const wait = Math.max(0, (remaining - this.#marginFor(session)) * 1000);
    const jitter = Math.random() * Math.min(5000, wait / 2);
    const due = delayMs ?? Math.max(remaining <= 0 ? RETRY_REFRESH_MS : 1000, wait - jitter);
    this.#timer = setTimeout(() => {
      this.#background(this.refresh().then(async ({ error }) => {
        if (error && error.code !== 'invalid_grant' && error.code !== 'session_missing') {
          this.#schedule(await this.current(), RETRY_REFRESH_MS);
        }
      }));
    }, Math.min(due, 2 ** 31 - 1));
    (this.#timer as { unref?: () => void }).unref?.();
  }
  #background(operation: Promise<unknown>): void {
    void operation.catch(error => {
      if (this.#disposed) return;
      clearTimeout(this.#timer);
      this.#timer = undefined;
      // A caller can observe and repair its adapter. Neither the operation nor
      // a throwing error handler may create an unhandled background rejection.
      try { void Promise.resolve(this.#onBackgroundError?.(error)).catch(() => {}); }
      catch { /* Error handlers are isolated. */ }
    });
  }
  dispose(): void {
    this.#disposed = true;
    clearTimeout(this.#timer);
    this.#broadcast?.close();
    this.#listeners.clear();
  }
}
