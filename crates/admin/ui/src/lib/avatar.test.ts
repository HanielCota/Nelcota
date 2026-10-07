import { describe, expect, it } from 'vitest'
import { base64Bytes, centerSquare, parseDataUrl } from './avatar'

describe('centerSquare', () => {
  it('corta as laterais de uma foto deitada', () => {
    expect(centerSquare(400, 300)).toEqual({ sx: 50, sy: 0, size: 300 })
  })

  it('corta em cima e embaixo de uma foto em pé', () => {
    expect(centerSquare(300, 401)).toEqual({ sx: 0, sy: 50, size: 300 })
  })

  it('não mexe numa imagem quadrada', () => {
    expect(centerSquare(256, 256)).toEqual({ sx: 0, sy: 0, size: 256 })
  })
})

describe('base64Bytes', () => {
  it('desconta o preenchimento', () => {
    expect(base64Bytes(btoa('abc'))).toBe(3)
    expect(base64Bytes(btoa('ab'))).toBe(2)
    expect(base64Bytes(btoa('a'))).toBe(1)
  })
})

describe('parseDataUrl', () => {
  it('separa tipo e dados', () => {
    expect(parseDataUrl('data:image/webp;base64,UklGRg==')).toEqual({ type: 'image/webp', base64: 'UklGRg==' })
  })

  it('recusa o que não é base64', () => {
    expect(parseDataUrl('data:image/svg+xml,<svg/>')).toBeNull()
    expect(parseDataUrl('https://exemplo.com/foto.png')).toBeNull()
  })
})
