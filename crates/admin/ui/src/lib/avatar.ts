// Prepares the profile photo in the browser: centred square crop, scaled to
// AVATAR_SIZE and compactly encoded. The server only accepts PNG, JPEG or
// WebP up to MAX_BYTES (see crates/admin/src/profile.rs).

export const AVATAR_SIZE = 256
export const MAX_BYTES = 256 * 1024

/** Largest centred square that fits the image ("cover" crop). */
export function centerSquare(width: number, height: number): { sx: number; sy: number; size: number } {
  const size = Math.min(width, height)
  return { sx: Math.floor((width - size) / 2), sy: Math.floor((height - size) / 2), size }
}

/** Bytes a base64 text represents (without decoding it). */
export function base64Bytes(base64: string): number {
  const padding = base64.endsWith('==') ? 2 : base64.endsWith('=') ? 1 : 0
  return (base64.length * 3) / 4 - padding
}

/** Splits `data:<type>;base64,<data>`; `null` if it is not base64. */
export function parseDataUrl(url: string): { type: string; base64: string } | null {
  const match = /^data:([^;,]+);base64,(.*)$/.exec(url)
  return match ? { type: match[1], base64: match[2] } : null
}

/**
 * Reads the file, crops and scales it. Tries WebP (smaller); if the browser
 * cannot encode WebP, falls back to JPEG. Returns the data URL (used as the
 * preview) and the base64 that goes to the API.
 */
export async function prepareAvatar(file: Blob): Promise<{ dataUrl: string; base64: string }> {
  const bitmap = await createImageBitmap(file)
  try {
    const canvas = document.createElement('canvas')
    canvas.width = canvas.height = AVATAR_SIZE
    const ctx = canvas.getContext('2d')
    if (!ctx) throw new Error('canvas unavailable')
    const { sx, sy, size } = centerSquare(bitmap.width, bitmap.height)
    ctx.imageSmoothingQuality = 'high'
    ctx.drawImage(bitmap, sx, sy, size, size, 0, 0, AVATAR_SIZE, AVATAR_SIZE)

    for (const [type, quality] of [
      ['image/webp', 0.9],
      ['image/jpeg', 0.9],
      ['image/jpeg', 0.75],
    ] as const) {
      const dataUrl = canvas.toDataURL(type, quality)
      const parsed = parseDataUrl(dataUrl)
      // toDataURL returns PNG when it cannot encode the requested type.
      if (parsed?.type === type && base64Bytes(parsed.base64) <= MAX_BYTES) {
        return { dataUrl, base64: parsed.base64 }
      }
    }
    throw new Error('could not shrink the image')
  } finally {
    bitmap.close()
  }
}
