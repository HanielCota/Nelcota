/**
 * Errors the client returns as values. `code` is the server's code, untouched
 * (`user_already_exists`, `invalid_grant`, `db_error`...), or one of the
 * client codes below when no response arrived.
 */

/** Codes produced by the client itself, never by the server. */
export type ClientErrorCode =
  | 'network_error'
  | 'timeout'
  | 'aborted'
  | 'invalid_response'
  | 'not_single'
  | 'session_missing'
  | 'pkce_missing';

export interface NelcotaErrorInit {
  status: number;
  code: string;
  message: string;
  retryAfter?: number | undefined;
  cause?: unknown;
}

export class NelcotaError extends Error {
  override readonly name = 'NelcotaError';
  /** HTTP status, or 0 when no response arrived. */
  readonly status: number;
  readonly code: string;
  /** Seconds to wait before retrying, from `Retry-After` (429/503). */
  readonly retryAfter: number | undefined;

  constructor(init: NelcotaErrorInit) {
    super(init.message, init.cause === undefined ? undefined : { cause: init.cause });
    this.status = init.status;
    this.code = init.code;
    this.retryAfter = init.retryAfter;
  }

  toJSON(): { name: string; status: number; code: string; message: string } {
    return { name: this.name, status: this.status, code: this.code, message: this.message };
  }
}

/**
 * A programming mistake (an invalid column name, a `service_role` token in a
 * browser...). It is thrown, not returned: no request was sent.
 */
export class NelcotaUsageError extends TypeError {
  override readonly name = 'NelcotaUsageError';
}

export type Result<T> = { data: T; error: null } | { data: null; error: NelcotaError };

export function ok<T>(data: T): { data: T; error: null } {
  return { data, error: null };
}

export function fail(error: NelcotaError): { data: null; error: NelcotaError } {
  return { data: null, error };
}

export function clientError(code: ClientErrorCode, message: string, cause?: unknown): NelcotaError {
  return new NelcotaError({ status: 0, code, message, cause });
}
