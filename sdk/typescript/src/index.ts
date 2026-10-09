/**
 * `@nelcota/client`: a thin, typed client for a Nelcota project. Every call
 * is one HTTP request; Postgres (GRANTs and RLS) decides what it may do.
 *
 * ```ts
 * import { createClient } from '@nelcota/client';
 * import type { Database } from './database'; // nelcota types -o database.ts
 *
 * const nelcota = createClient<Database>('https://api.example.com');
 * const { data, error } = await nelcota.from('notes').select('id,body').order('id', { ascending: false });
 * ```
 */

import { AuthClient, type AuthOptions } from './auth/index.js';
import { HttpClient, baseUrl, type FetchLike, type TokenSource } from './core/http.js';
import { RestClient } from './rest/index.js';
import type { DefaultSchemaName, SchemaOf } from './rest/types.js';
import { StorageClient } from './storage/index.js';

export interface ClientOptions<SchemaName> {
  /** Schema the types come from (the server exposes one, `public` by default). */
  schema?: SchemaName;
  auth?: AuthOptions;
  /**
   * Overrides where the access token comes from, e.g. a server that forwards
   * the caller's token. With it, no session is stored or refreshed.
   */
  accessToken?: TokenSource;
  /** Custom fetch (tests, instrumentation). Default: the global `fetch`. */
  fetch?: FetchLike;
  /** Extra headers on every request. Browsers only send headers the server's CORS allows. */
  headers?: Record<string, string>;
  /** Per-attempt timeout in ms for API calls. Default 30000; `0` disables it. */
  timeout?: number;
  /** Retries for reads on 429/503 and network errors. Default 2. */
  retries?: number;
}

export class NelcotaClient<Database = any, SchemaName extends string = DefaultSchemaName<Database>> extends RestClient<
  SchemaOf<Database, SchemaName>
> {
  readonly auth: AuthClient;
  readonly storage: StorageClient;
  /** The project's base URL. */
  readonly url: URL;

  constructor(url: string | URL, options: ClientOptions<SchemaName> = {}) {
    const base = baseUrl(url);
    const fetchImpl = options.fetch ?? globalThis.fetch?.bind(globalThis);
    if (!fetchImpl) throw new TypeError('No fetch available: pass options.fetch');
    let auth: AuthClient | undefined;
    const http = new HttpClient({
      url: base,
      fetch: fetchImpl,
      headers: { ...options.headers },
      timeout: options.timeout ?? 30_000,
      retries: options.retries ?? 2,
      token: options.accessToken ?? (() => auth?.accessToken() ?? null),
    });
    super(http);
    auth = new AuthClient(http, options.accessToken ? { ...options.auth, autoRefresh: false } : options.auth);
    this.auth = auth;
    this.storage = new StorageClient(http);
    this.url = base;
  }
}

/** Creates a client. Pass the `Database` type from `nelcota types` for typed rows. */
export function createClient<Database = any, SchemaName extends string = DefaultSchemaName<Database>>(
  url: string | URL,
  options: ClientOptions<SchemaName> = {},
): NelcotaClient<Database, SchemaName> {
  return new NelcotaClient<Database, SchemaName>(url, options);
}

export { NelcotaError, NelcotaUsageError } from './core/errors.js';
export type { ClientErrorCode, Result } from './core/errors.js';
export { escapeLike } from './core/encoding.js';
export type { FetchLike, TokenSource } from './core/http.js';
export * from './rest/index.js';
export * from './auth/index.js';
export * from './storage/index.js';
