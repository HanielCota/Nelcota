import { QueryClient } from '@tanstack/svelte-query'

// JSON reads share a short-lived cache. SQL execution and all writes always
// reach the server; a session boundary cancels and discards private data.
export const queryClient = new QueryClient({
  defaultOptions: { queries: { retry: false, staleTime: 2000, gcTime: 60_000, structuralSharing: false } },
})
let epoch = 0

export function clearQuerySession() {
  epoch++
  queryClient.clear()
}

export async function queryRead<T>(path: string, account: string | null | undefined, fetcher: (signal: AbortSignal) => Promise<T>, signal?: AbortSignal, freshForMs = 2000): Promise<T> {
  signal?.throwIfAborted()
  const started = epoch
  const pending = queryClient.fetchQuery({ queryKey: ['admin', epoch, account, path], queryFn: ({ signal }) => fetcher(signal), staleTime: freshForMs })
  // Cancelling one reader must not cancel another reader of the same query.
  const result = signal ? await new Promise<T>((resolve, reject) => {
    const abort = () => reject(signal.reason ?? new DOMException('Aborted', 'AbortError'))
    signal.addEventListener('abort', abort, { once: true })
    pending.then(resolve, reject).finally(() => signal.removeEventListener('abort', abort))
  }) : await pending
  if (started !== epoch) throw new DOMException('Session changed', 'AbortError')
  // Table cell editing mutates local rows; it must not mutate cached JSON.
  return structuredClone(result)
}

export function invalidateReads() {
  void queryClient.invalidateQueries({ queryKey: ['admin'], refetchType: 'none' })
}
