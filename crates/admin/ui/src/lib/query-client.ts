import { QueryClient } from '@tanstack/svelte-query'
import { session } from './features/auth/session.svelte'

// JSON reads share a short-lived cache. SQL execution and all writes always
// reach the server; a session boundary cancels and discards private data.
export const queryClient = new QueryClient({
  defaultOptions: { queries: { retry: false, staleTime: 2000, gcTime: 60_000, structuralSharing: false } },
})
let revision = 0
const readers = new Map<string, number>()

export const querySession = () => session.epoch

export function clearQuerySession() {
  session.epoch++
  revision = 0
  queryClient.clear()
}

export async function queryRead<T>(path: string, account: string | null | undefined, fetcher: (signal: AbortSignal) => Promise<T>, signal?: AbortSignal, freshForMs = 2000): Promise<T> {
  signal?.throwIfAborted()
  const started = querySession()
  const queryKey = ['admin', started, revision, account, path]
  const key = JSON.stringify(queryKey)
  readers.set(key, (readers.get(key) ?? 0) + 1)
  try {
    const pending = queryClient.fetchQuery({ queryKey, queryFn: ({ signal }) => fetcher(signal), staleTime: freshForMs })
    // Cancelling one reader must not cancel another reader of the same query.
    const result = signal ? await new Promise<T>((resolve, reject) => {
      const abort = () => reject(signal.reason ?? new DOMException('Aborted', 'AbortError'))
      signal.addEventListener('abort', abort, { once: true })
      pending.then(resolve, reject).finally(() => signal.removeEventListener('abort', abort))
      // A synchronous adapter can abort while fetchQuery starts its request.
      if (signal.aborted) abort()
    }) : await pending
    if (started !== querySession()) throw new DOMException('Session changed', 'AbortError')
    // Table cell editing mutates local rows; it must not mutate cached JSON.
    return structuredClone(result)
  } finally {
    const remaining = (readers.get(key) ?? 1) - 1
    if (remaining > 0) {
      readers.set(key, remaining)
    }
    if (remaining === 0) {
      readers.delete(key)
      if (signal?.aborted) void queryClient.cancelQueries({ queryKey, exact: true })
    }
  }
}

export function invalidateReads() {
  // A read started before a write may still resolve afterwards. New readers
  // must neither join that request nor cache its pre-mutation result.
  revision++
  void queryClient.invalidateQueries({ queryKey: ['admin'], refetchType: 'none' })
}
