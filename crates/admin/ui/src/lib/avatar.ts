// Preparo da foto de perfil no navegador: recorte quadrado no centro,
// redução para AVATAR_SIZE e codificação compacta. O servidor só aceita
// PNG, JPEG ou WebP de até MAX_BYTES (ver crates/admin/src/profile.rs).

export const AVATAR_SIZE = 256
export const MAX_BYTES = 256 * 1024

/** Maior quadrado centralizado que cabe na imagem (recorte tipo "cover"). */
export function centerSquare(width: number, height: number): { sx: number; sy: number; size: number } {
  const size = Math.min(width, height)
  return { sx: Math.floor((width - size) / 2), sy: Math.floor((height - size) / 2), size }
}

/** Bytes que um texto base64 representa (sem decodificar). */
export function base64Bytes(base64: string): number {
  const padding = base64.endsWith('==') ? 2 : base64.endsWith('=') ? 1 : 0
  return (base64.length * 3) / 4 - padding
}

/** Separa `data:<tipo>;base64,<dados>`; `null` se não for base64. */
export function parseDataUrl(url: string): { type: string; base64: string } | null {
  const match = /^data:([^;,]+);base64,(.*)$/.exec(url)
  return match ? { type: match[1], base64: match[2] } : null
}

/**
 * Lê o arquivo, recorta e reduz. Tenta WebP (menor); se o navegador não
 * codifica WebP, cai para JPEG. Devolve a data URL (serve de prévia) e o
 * base64 que vai para a API.
 */
export async function prepareAvatar(file: Blob): Promise<{ dataUrl: string; base64: string }> {
  const bitmap = await createImageBitmap(file)
  try {
    const canvas = document.createElement('canvas')
    canvas.width = canvas.height = AVATAR_SIZE
    const ctx = canvas.getContext('2d')
    if (!ctx) throw new Error('canvas indisponível')
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
      // toDataURL devolve PNG quando não sabe codificar o tipo pedido.
      if (parsed?.type === type && base64Bytes(parsed.base64) <= MAX_BYTES) {
        return { dataUrl, base64: parsed.base64 }
      }
    }
    throw new Error('não foi possível reduzir a imagem')
  } finally {
    bitmap.close()
  }
}
