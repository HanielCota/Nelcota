/** Reads only a preview's byte budget, even when the listing or length is stale. */
export async function readPreview(response: Response, limit: number, signal: AbortSignal): Promise<Blob | null> {
  signal.throwIfAborted()
  const body = response.body
  if (!body) return new Blob()
  const declared = response.headers.get('content-length')?.trim()
  if (declared && /^\d+$/.test(declared) && Number(declared) > limit) {
    await body.cancel()
    return null
  }

  const reader = body.getReader()
  const abort = () => { void reader.cancel().catch(() => {}) }
  signal.addEventListener('abort', abort, { once: true })
  const chunks: BlobPart[] = []
  let bytes = 0
  try {
    signal.throwIfAborted()
    while (true) {
      const { done, value } = await reader.read()
      signal.throwIfAborted()
      if (done) break
      bytes += value.byteLength
      if (bytes > limit) {
        await reader.cancel()
        return null
      }
      chunks.push(value)
    }
    return new Blob(chunks, { type: response.headers.get('content-type') ?? '' })
  } finally {
    signal.removeEventListener('abort', abort)
    reader.releaseLock()
  }
}
