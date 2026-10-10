/**
 * The one place requests are sent: base URL, auth header, timeouts, retries
 * and error decoding. Every other module builds a request and hands it here.
 */

import { NelcotaError, NelcotaUsageError, clientError } from './errors.js';
import { assertTokenAllowed } from './jwt.js';

export type FetchLike = (input: string | URL | Request, init?: RequestInit) => Promise<Response>;

/** Where the access token for a request comes from (`null` = visitor). */
export type TokenSource = () => Promise<string | null> | string | null;

export interface HttpOptions {
  url: URL;
  fetch: FetchLike;
  headers: Readonly<Record<string, string>>;
  /** Default timeout per attempt, in ms. `0` disables it. */
  timeout: number;
  /** Retries for idempotent requests on 429/503 and network errors. */
  retries: number;
  token: TokenSource;
}

export interface RequestSpec {
  method: 'GET' | 'HEAD' | 'POST' | 'PUT' | 'PATCH' | 'DELETE';
  path: string;
  query?: URLSearchParams | undefined;
  /** A plain value is sent as JSON; a `BodyInit` as is. */
  json?: unknown;
  body?: BodyInit | null | undefined;
  headers?: Record<string, string> | undefined;
  signal?: AbortSignal | undefined;
  /** Per-request timeout in ms (`0` disables it). */
  timeout?: number | undefined;
  /** `false` sends no Authorization header; a string uses that token. */
  auth?: boolean | string | undefined;
}

const MAX_RETRY_WAIT_MS = 30_000;

function nonNegativeInteger(value: number, name: string, maximum = Number.MAX_SAFE_INTEGER): void {
  if (!Number.isSafeInteger(value) || value < 0 || value > maximum) {
    throw new NelcotaUsageError(`${name} must be an integer between 0 and ${maximum}`);
  }
}

/** Seconds from a `Retry-After` header (delay-seconds or HTTP date). */
export function parseRetryAfter(value: string | null): number | undefined {
  if (value === null) return undefined;
  const trimmed = value.trim();
  if (/^\d+$/.test(trimmed)) {
    const seconds = Number(trimmed);
    return Number.isSafeInteger(seconds) ? seconds : undefined;
  }
  const date = Date.parse(trimmed);
  if (Number.isNaN(date)) return undefined;
  return Math.max(0, Math.ceil((date - Date.now()) / 1000));
}

/** The server's `{ code, message }`, or a generic error from the status. */
export async function errorFromResponse(response: Response): Promise<NelcotaError> {
  const retryAfter = parseRetryAfter(response.headers.get('retry-after'));
  let code = `http_${response.status}`;
  let message = response.statusText || `HTTP ${response.status}`;
  const text = await response.text();
  try {
    const body: unknown = JSON.parse(text);
    if (typeof body === 'object' && body !== null) {
      const record = body as Record<string, unknown>;
      if (typeof record['code'] === 'string') code = record['code'];
      if (typeof record['message'] === 'string') message = record['message'];
    }
  } catch {
    // Not JSON (a proxy page, an empty body): keep the generic error.
  }
  return new NelcotaError({ status: response.status, code, message, retryAfter });
}

function sleep(ms: number, signal: AbortSignal | undefined): Promise<void> {
  return new Promise((resolve, reject) => {
    if (signal?.aborted) return reject(signal.reason);
    const timer = setTimeout(() => {
      signal?.removeEventListener('abort', onAbort);
      resolve();
    }, ms);
    const onAbort = (): void => {
      clearTimeout(timer);
      reject(signal?.reason);
    };
    signal?.addEventListener('abort', onAbort, { once: true });
  });
}

function backoff(attempt: number, retryAfter: number | undefined): number {
  if (retryAfter !== undefined) return retryAfter * 1000;
  // Full jitter: spreads clients that failed together.
  return Math.random() * Math.min(200 * 2 ** attempt, MAX_RETRY_WAIT_MS);
}

export class HttpClient {
  readonly url: URL;
  readonly #options: HttpOptions;
  /** The base URL without trailing slashes. */
  readonly #base: string;

  constructor(options: HttpOptions) {
    nonNegativeInteger(options.timeout, 'timeout', 2 ** 31 - 1);
    nonNegativeInteger(options.retries, 'retries');
    this.url = options.url;
    this.#options = options;
    let end = options.url.href.length;
    while (end > 0 && options.url.href[end - 1] === '/') end--;
    this.#base = options.url.href.slice(0, end);
  }

  /** Absolute URL for a path (and query) under the base URL. */
  href(path: string, query?: URLSearchParams): string {
    const search = query && query.size > 0 ? `?${query.toString()}` : '';
    return `${this.#base}${path}${search}`;
  }

  /**
   * Sends the request and returns the response when it is 2xx/3xx, or a
   * `NelcotaError` otherwise. Never throws for HTTP or network failures.
   */
  async #request<T>(spec: RequestSpec, read: (response: Response) => Promise<T>): Promise<
    { data: T; error: null; response: Response } | { data: null; error: NelcotaError; response: Response | null }
  > {
    const timeout = spec.timeout ?? this.#options.timeout;
    nonNegativeInteger(timeout, 'timeout', 2 ** 31 - 1);
    if (spec.signal?.aborted) {
      return { data: null, response: null, error: clientError('aborted', 'The request was aborted') };
    }
    const headers = new Headers(this.#options.headers);
    for (const [name, value] of Object.entries(spec.headers ?? {})) headers.set(name, value);
    if (spec.auth === false) headers.delete('authorization');

    let token: string | null = null;
    if (typeof spec.auth === 'string') token = spec.auth;
    else if (spec.auth !== false) token = await this.#options.token();
    if (token) {
      assertTokenAllowed(token);
      headers.set('authorization', `Bearer ${token}`);
    }
    const bearer = headers.get('authorization');
    if (bearer?.toLowerCase().startsWith('bearer ')) assertTokenAllowed(bearer.slice(7));

    let body: BodyInit | null | undefined = spec.body;
    if (spec.json !== undefined) {
      if (body !== undefined) throw new NelcotaUsageError('A request has either json or body, not both');
      body = JSON.stringify(spec.json);
      headers.set('content-type', 'application/json');
    }

    const idempotent = spec.method === 'GET' || spec.method === 'HEAD';
    const retries = idempotent ? this.#options.retries : 0;
    const href = this.href(spec.path, spec.query);
    const streaming = typeof ReadableStream !== 'undefined' && body instanceof ReadableStream;

    for (let attempt = 0; ; attempt++) {
      const signals: AbortSignal[] = [];
      if (spec.signal) signals.push(spec.signal);
      if (timeout > 0) signals.push(AbortSignal.timeout(timeout));
      const signal = signals.length > 1 ? AbortSignal.any(signals) : signals[0];
      const init: RequestInit & { duplex?: 'half' } = { method: spec.method, headers, body: body ?? null, redirect: 'follow' };
      if (signal) init.signal = signal;
      if (streaming) init.duplex = 'half';

      let failure: NelcotaError;
      try {
        const response = await this.#options.fetch(href, init);
        if (response.ok || (response.status >= 300 && response.status < 400)) {
          // Buffered reads happen in this attempt: body failures, cancellation
          // and timeouts have the same result contract as failures in fetch.
          return { data: await read(response), response, error: null };
        }
        failure = await errorFromResponse(response);
        const retryable = response.status === 429 || response.status === 503;
        // Return long Retry-After delays to the caller instead of retrying early.
        if (!retryable || attempt >= retries || (failure.retryAfter ?? 0) * 1000 > MAX_RETRY_WAIT_MS) {
          return { data: null, response: null, error: failure };
        }
      } catch (cause) {
        if (cause instanceof NelcotaError) return { data: null, response: null, error: cause };
        if (spec.signal?.aborted) {
          return { data: null, response: null, error: clientError('aborted', 'The request was aborted', cause) };
        }
        if ((signal?.aborted && signal.reason?.name === 'TimeoutError') || (cause instanceof DOMException && cause.name === 'TimeoutError')) {
          failure = clientError('timeout', `No response within ${timeout} ms`, cause);
        } else {
          failure = clientError('network_error', 'The server could not be reached', cause);
        }
        if (attempt >= retries) return { data: null, response: null, error: failure };
      }
      try {
        await sleep(backoff(attempt, failure.retryAfter), spec.signal);
      } catch (cause) {
        return { data: null, response: null, error: clientError('aborted', 'The request was aborted', cause) };
      }
    }
  }

  /** Raw streaming response. Later body reads belong to the caller. */
  async send(spec: RequestSpec): Promise<{ response: Response; error: null } | { response: null; error: NelcotaError }> {
    const result = await this.#request(spec, async (response) => response);
    return result.error ? { response: null, error: result.error } : { response: result.data, error: null };
  }

  /** `send` plus JSON decoding; 204 and empty bodies give `null`. */
  async json<T>(spec: RequestSpec): Promise<{ data: T; error: null; response: Response } | { data: null; error: NelcotaError; response: Response | null }> {
    return this.#request(spec, async (response) => {
      const text = spec.method === 'HEAD' ? '' : await response.text();
      if (text === '') return null as T;
      try {
        return JSON.parse(text) as T;
      } catch (cause) {
        throw new NelcotaError({
          status: response.status,
          code: 'invalid_response',
          message: 'The server answered with something that is not JSON',
          cause,
        });
      }
    });
  }

  /** Buffered download with the same failure and retry contract as JSON. */
  async blob(spec: RequestSpec): Promise<{ data: Blob; error: null } | { data: null; error: NelcotaError }> {
    const result = await this.#request(spec, (response) => response.blob());
    return result.error ? { data: null, error: result.error } : { data: result.data, error: null };
  }
}

/** Validates the base URL: http(s), no credentials, query or fragment. */
export function baseUrl(input: string | URL): URL {
  let url: URL;
  try {
    url = new URL(input);
  } catch {
    throw new NelcotaUsageError(`Invalid Nelcota URL: ${String(input)}`);
  }
  if (url.protocol !== 'https:' && url.protocol !== 'http:') {
    throw new NelcotaUsageError('The Nelcota URL must be http or https');
  }
  if (url.username || url.password || url.search || url.hash) {
    throw new NelcotaUsageError('The Nelcota URL cannot carry credentials, a query or a fragment');
  }
  return url;
}
