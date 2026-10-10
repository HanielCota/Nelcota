import { clientError, fail, ok, type Result } from './errors.js';

/** Cancel a caller's wait without interrupting a shared operation. */
export function abortable<T>(operation: Promise<T>, signal?: AbortSignal): Promise<Result<T>> {
  if (!signal) return operation.then(value => ok(value));
  return new Promise<Result<T>>((resolve, reject) => {
    const abort = (): void => {
      signal.removeEventListener('abort', abort);
      resolve(fail(clientError('aborted', 'The request was aborted')));
    };
    // Attach both handlers even when already aborted: a detached operation
    // may still reject, and its rejection must remain observed.
    operation.then(value => {
      signal.removeEventListener('abort', abort);
      resolve(ok(value));
    }, error => {
      signal.removeEventListener('abort', abort);
      reject(error);
    });
    if (signal.aborted) abort();
    else signal.addEventListener('abort', abort, { once: true });
  });
}
