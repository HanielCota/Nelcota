/**
 * Sign-up, sign-in (password, OAuth with PKCE, email links) and the session's
 * life: stored, refreshed before it expires, shared across tabs.
 */

import { NelcotaError, NelcotaUsageError, clientError, fail, ok, type Result } from '../core/errors.js';
import type { HttpClient } from '../core/http.js';
import { isBrowser } from '../core/jwt.js';
import { Broadcast, withLock } from './coordination.js';
import { challengeFor, createVerifier } from './pkce.js';
import { isSession, isUser, memoryStorage, parseSession, secondsLeft, type Session, type SessionStorage, type User } from './session.js';

export type AuthEvent = 'signed_in' | 'signed_out' | 'refreshed' | 'user_updated';
export type AuthListener = (event: AuthEvent, session: Session | null) => void;

export type OAuthProvider = 'google' | 'github';
export type EmailLinkType = 'recovery' | 'signup' | 'magiclink';

export interface AuthOptions {
  /** Where the session is kept. Default: `localStorage` in browsers, memory elsewhere. */
  storage?: SessionStorage;
  /** Storage key. Default: `nelcota.<host>.session`. */
  storageKey?: string;
  /** Refresh in the background before expiry. Default: on in browsers. */
  autoRefresh?: boolean;
  /** Refresh when fewer than this many seconds are left. Default 60. */
  refreshMargin?: number;
}

export interface Credentials {
  email: string;
  password: string;
}

export interface SignUpInput extends Credentials {
  /** Becomes `user_metadata`. Never trust it for authorization. */
  data?: Record<string, unknown>;
}

export interface OAuthInput {
  provider: OAuthProvider;
  /** App page to come back to; must be allowed in NELCOTA_OAUTH_REDIRECT_URLS. */
  redirectTo: string;
  /** Navigate there right away (browsers only). Default true in browsers. */
  redirect?: boolean;
}

export interface EmailLink {
  type: EmailLinkType;
  token: string;
}

const LINK_TYPES = new Set<string>(['recovery', 'signup', 'magiclink']);
const RETRY_REFRESH_MS = 10_000;

function browserStorage(): SessionStorage | undefined {
  try {
    return isBrowser() ? globalThis.localStorage : undefined;
  } catch {
    // Storage can be blocked (privacy settings, sandboxed frames).
    return undefined;
  }
}

function email(value: unknown): string {
  if (typeof value !== 'string' || value.trim() === '') throw new NelcotaUsageError('An email is required');
  return value.trim();
}

export class AuthClient {
  readonly #http: HttpClient;
  readonly #storage: SessionStorage;
  readonly #key: string;
  readonly #margin: number;
  readonly #autoRefresh: boolean;
  readonly #listeners = new Set<AuthListener>();
  readonly #broadcast: Broadcast | undefined;
  /** `undefined` = not loaded from storage yet. */
  #session: Session | null | undefined;
  #refreshing: Promise<Result<Session>> | null = null;
  #timer: ReturnType<typeof setTimeout> | undefined;

  constructor(http: HttpClient, options: AuthOptions = {}) {
    this.#http = http;
    this.#storage = options.storage ?? browserStorage() ?? memoryStorage();
    this.#key = options.storageKey ?? `nelcota.${http.url.host}.session`;
    this.#margin = options.refreshMargin ?? 60;
    this.#autoRefresh = options.autoRefresh ?? isBrowser();
    if (isBrowser()) {
      this.#broadcast = new Broadcast(`${this.#key}.channel`, () => void this.#reload());
    }
    if (this.#autoRefresh) void this.#current().then((session) => this.#schedule(session));
  }

  // --------------------------------------------------------------- events

  /** Calls `listener` on sign-in, sign-out, refresh and user changes. Returns the unsubscribe. */
  onChange(listener: AuthListener): () => void {
    this.#listeners.add(listener);
    return () => this.#listeners.delete(listener);
  }

  #emit(event: AuthEvent, session: Session | null): void {
    for (const listener of this.#listeners) {
      try {
        listener(event, session);
      } catch (error) {
        queueMicrotask(() => {
          throw error;
        });
      }
    }
  }

  // -------------------------------------------------------------- storage

  async #current(): Promise<Session | null> {
    if (this.#session === undefined) {
      const stored = await this.#storage.getItem(this.#key);
      this.#session = parseSession(stored);
      if (stored !== null && this.#session === null) await this.#storage.removeItem(this.#key);
    }
    return this.#session;
  }

  /** Another tab changed the session: drop the cached copy and tell listeners. */
  async #reload(): Promise<void> {
    const before = this.#session;
    this.#session = undefined;
    const after = await this.#current();
    this.#schedule(after);
    if (after === null && before) this.#emit('signed_out', null);
    else if (after && !before) this.#emit('signed_in', after);
    else if (after) this.#emit('refreshed', after);
  }

  async #save(session: Session, event: AuthEvent): Promise<void> {
    this.#session = session;
    await this.#storage.setItem(this.#key, JSON.stringify(session));
    this.#broadcast?.notify();
    this.#schedule(session);
    this.#emit(event, session);
  }

  async #clear(): Promise<void> {
    const had = (await this.#current()) !== null;
    this.#session = null;
    await this.#storage.removeItem(this.#key);
    this.#schedule(null);
    if (had) {
      this.#broadcast?.notify();
      this.#emit('signed_out', null);
    }
  }

  #schedule(session: Session | null, delayMs?: number): void {
    clearTimeout(this.#timer);
    this.#timer = undefined;
    if (!this.#autoRefresh || !session) return;
    // Jitter so tabs opened together do not all wake at the same instant.
    const due = delayMs ?? Math.max(0, (secondsLeft(session) - this.#margin) * 1000 - Math.random() * 5000);
    this.#timer = setTimeout(() => {
      void this.refreshSession().then(({ error }) => {
        if (error && error.code !== 'invalid_grant' && error.code !== 'session_missing') {
          void this.#current().then((s) => this.#schedule(s, RETRY_REFRESH_MS));
        }
      });
    }, Math.min(due, 2 ** 31 - 1));
    (this.#timer as { unref?: () => void }).unref?.();
  }

  /** Stops the background refresh and the cross-tab channel. */
  dispose(): void {
    clearTimeout(this.#timer);
    this.#broadcast?.close();
    this.#listeners.clear();
  }

  // -------------------------------------------------------------- session

  /** The session, refreshed first when it is about to expire; `null` when signed out. */
  async getSession(): Promise<Result<Session | null>> {
    const session = await this.#current();
    if (!session) return ok(null);
    if (secondsLeft(session) > this.#margin) return ok(session);
    const refreshed = await this.refreshSession();
    if (refreshed.error?.code === 'invalid_grant' || refreshed.error?.code === 'session_missing') return ok(null);
    return refreshed;
  }

  /**
   * The access token for API requests. When a refresh fails for a transient
   * reason the old token is still sent, so the request fails with 401 instead
   * of silently running as a visitor.
   */
  async accessToken(): Promise<string | null> {
    const { data } = await this.getSession();
    if (data) return data.access_token;
    return this.#session?.access_token ?? null;
  }

  /**
   * Trades the refresh token for a new pair. Single-flight in this process
   * and, in browsers, across tabs (a reused refresh token ends the session).
   */
  refreshSession(): Promise<Result<Session>> {
    this.#refreshing ??= (async () => {
      const known = await this.#current();
      if (!known) return fail(clientError('session_missing', 'Not signed in'));
      return withLock(`${this.#key}.refresh`, async () => {
        // Another tab may have refreshed while this one waited for the lock.
        this.#session = undefined;
        const current = await this.#current();
        if (!current) return fail(clientError('session_missing', 'Not signed in'));
        if (current.refresh_token !== known.refresh_token && secondsLeft(current) > this.#margin) {
          return ok(current);
        }
        const result = await this.#token('refresh_token', { refresh_token: current.refresh_token });
        if (result.error) {
          if (result.error.code === 'invalid_grant') await this.#clear();
          return result;
        }
        await this.#save(result.data, 'refreshed');
        return result;
      });
    })().finally(() => {
      this.#refreshing = null;
    });
    return this.#refreshing;
  }

  /** Stores a session obtained elsewhere (server-side rendering, tests). */
  async setSession(session: Session): Promise<void> {
    if (!isSession(session)) throw new NelcotaUsageError('Not a Nelcota session');
    await this.#save(session, 'signed_in');
  }

  // --------------------------------------------------------------- sign in

  async #token(grant: 'password' | 'refresh_token' | 'pkce', body: Record<string, string>): Promise<Result<Session>> {
    const result = await this.#http.json<unknown>({
      method: 'POST',
      path: '/auth/v1/token',
      query: new URLSearchParams({ grant_type: grant }),
      json: body,
      auth: false,
    });
    if (result.error) return fail(result.error);
    return isSession(result.data) ? ok(result.data) : fail(invalidResponse());
  }

  /** Creates an account. `session` is `null` while the email awaits confirmation. */
  async signUp(input: SignUpInput): Promise<Result<{ user: User; session: Session | null }>> {
    const body: Record<string, unknown> = { email: email(input.email), password: input.password };
    if (input.data !== undefined) body['data'] = input.data;
    const result = await this.#http.json<unknown>({ method: 'POST', path: '/auth/v1/signup', json: body, auth: false });
    if (result.error) return fail(result.error);
    if (isSession(result.data)) {
      await this.#save(result.data, 'signed_in');
      return ok({ user: result.data.user, session: result.data });
    }
    const user = (result.data as { user?: unknown } | null)?.user;
    return isUser(user) ? ok({ user, session: null }) : fail(invalidResponse());
  }

  async signInWithPassword(credentials: Credentials): Promise<Result<Session>> {
    const result = await this.#token('password', { email: email(credentials.email), password: credentials.password });
    if (result.data) await this.#save(result.data, 'signed_in');
    return result;
  }

  /**
   * Starts a Google/GitHub sign-in. The PKCE verifier stays in this
   * browser's storage; the app finishes on `redirectTo` with `handleRedirect()`.
   */
  async signInWithOAuth(input: OAuthInput): Promise<Result<{ url: string }>> {
    const redirectTo = absoluteUrl(input.redirectTo, 'redirectTo').href;
    const verifier = createVerifier();
    await this.#storage.setItem(`${this.#key}.pkce`, verifier);
    const query = new URLSearchParams({
      provider: input.provider,
      redirect_to: redirectTo,
      code_challenge: await challengeFor(verifier),
      code_challenge_method: 'S256',
    });
    const url = this.#http.href('/auth/v1/authorize', query);
    if ((input.redirect ?? true) && isBrowser()) globalThis.location.assign(url);
    return ok({ url });
  }

  /** Trades the `code` from the OAuth redirect (and the stored verifier) for a session. */
  async exchangeCode(code: string): Promise<Result<Session>> {
    const key = `${this.#key}.pkce`;
    const verifier = await this.#storage.getItem(key);
    // The code is spent on the first try, right or wrong: the verifier goes too.
    await this.#storage.removeItem(key);
    if (!verifier) {
      return fail(clientError('pkce_missing', 'No sign-in was started in this browser (PKCE verifier missing)'));
    }
    const result = await this.#token('pkce', { auth_code: code, code_verifier: verifier });
    if (result.data) await this.#save(result.data, 'signed_in');
    return result;
  }

  /**
   * Finishes an OAuth sign-in on the `redirectTo` page: reads `?code=` or
   * `?error=`, removes them from the address bar and history, and signs in.
   * Returns `null` data when the URL carries neither.
   */
  async handleRedirect(href: string = globalThis.location?.href ?? ''): Promise<Result<Session | null>> {
    if (href === '') return ok(null);
    const url = absoluteUrl(href, 'URL');
    const code = url.searchParams.get('code');
    const error = url.searchParams.get('error');
    if (code === null && error === null) return ok(null);
    url.searchParams.delete('code');
    url.searchParams.delete('error');
    replaceUrl(href, url);
    if (error !== null) {
      return fail(new NelcotaError({ status: 0, code: error, message: `Sign-in failed: ${error}` }));
    }
    return this.exchangeCode(code ?? '');
  }

  /**
   * Reads an email link's fragment (`#type=...&token=...`) and removes it
   * from the address bar and history, so the token cannot leak later.
   * Then call `verifyEmailLink` or, for `recovery`, `resetPassword`.
   */
  readEmailLink(href: string = globalThis.location?.href ?? ''): EmailLink | null {
    if (href === '') return null;
    const url = absoluteUrl(href, 'URL');
    const params = new URLSearchParams(url.hash.slice(1));
    const type = params.get('type');
    const token = params.get('token');
    if (type === null || token === null || !LINK_TYPES.has(type) || token.length === 0 || token.length > 128) {
      return null;
    }
    url.hash = '';
    replaceUrl(href, url);
    return { type: type as EmailLinkType, token };
  }

  /** Confirms a sign-up or signs in with a magic link. */
  async verifyEmailLink(link: { type: 'signup' | 'magiclink'; token: string }): Promise<Result<Session>> {
    return this.#verify({ type: link.type, token: link.token });
  }

  /** Sets a new password from a recovery link; other sessions end. */
  async resetPassword(input: { token: string; password: string }): Promise<Result<Session>> {
    return this.#verify({ type: 'recovery', token: input.token, password: input.password });
  }

  async #verify(body: Record<string, string>): Promise<Result<Session>> {
    const result = await this.#http.json<unknown>({ method: 'POST', path: '/auth/v1/verify', json: body, auth: false });
    if (result.error) return fail(result.error);
    if (!isSession(result.data)) return fail(invalidResponse());
    await this.#save(result.data, 'signed_in');
    return ok(result.data);
  }

  async #emailOnly(path: string, body: Record<string, string>): Promise<Result<null>> {
    const result = await this.#http.json<unknown>({ method: 'POST', path, json: body, auth: false });
    return result.error ? fail(result.error) : ok(null);
  }

  /** Emails a recovery link. Answers the same whether or not the account exists. */
  requestPasswordReset(address: string): Promise<Result<null>> {
    return this.#emailOnly('/auth/v1/recover', { email: email(address) });
  }

  /** Emails a passwordless sign-in link. */
  sendMagicLink(address: string): Promise<Result<null>> {
    return this.#emailOnly('/auth/v1/magiclink', { email: email(address) });
  }

  /** Sends the sign-up confirmation email again. */
  resendConfirmation(address: string): Promise<Result<null>> {
    return this.#emailOnly('/auth/v1/resend', { type: 'signup', email: email(address) });
  }

  // ---------------------------------------------------------------- user

  /** The signed-in user as the server sees it now. */
  async getUser(): Promise<Result<User>> {
    const { data: session, error } = await this.getSession();
    if (error) return fail(error);
    if (!session) return fail(clientError('session_missing', 'Not signed in'));
    const result = await this.#http.json<unknown>({ method: 'GET', path: '/auth/v1/user', auth: session.access_token });
    if (result.error) return fail(result.error);
    if (!isUser(result.data)) return fail(invalidResponse());
    if (JSON.stringify(result.data) !== JSON.stringify(session.user)) {
      await this.#save({ ...session, user: result.data }, 'user_updated');
    }
    return ok(result.data);
  }

  /**
   * Ends the session on the server (its refresh tokens stop working) and
   * forgets it here. The current access token stays valid until it expires.
   */
  async signOut(): Promise<Result<null>> {
    const { data: session } = await this.getSession();
    let error: NelcotaError | null = null;
    if (session) {
      const result = await this.#http.send({ method: 'POST', path: '/auth/v1/logout', auth: session.access_token });
      error = result.error;
    }
    await this.#clear();
    return error ? fail(error) : ok(null);
  }
}

function absoluteUrl(value: string, what: string): URL {
  try {
    return new URL(value);
  } catch {
    throw new NelcotaUsageError(`Invalid ${what}: ${JSON.stringify(value)} is not an absolute URL`);
  }
}

function invalidResponse(): NelcotaError {
  return clientError('invalid_response', 'The server answered with an unexpected shape');
}

/** Rewrites the address bar, only when `original` is the page being shown. */
function replaceUrl(original: string, url: URL): void {
  if (!isBrowser() || globalThis.location?.href !== original) return;
  try {
    globalThis.history.replaceState(globalThis.history.state, '', url.href);
  } catch {
    // A different origin or a sandboxed frame: nothing to clean.
  }
}
