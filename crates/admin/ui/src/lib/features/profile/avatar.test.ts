import { describe, expect, it } from 'vitest'
import { base64Bytes, centerSquare, parseDataUrl } from './avatar'

describe('centerSquare', () => {
  it('crops the sides of a landscape photo', () => {
    expect(centerSquare(400, 300)).toEqual({ sx: 50, sy: 0, size: 300 })
  })

  it('crops the top and bottom of a portrait photo', () => {
    expect(centerSquare(300, 401)).toEqual({ sx: 0, sy: 50, size: 300 })
  })

  it('leaves a square image alone', () => {
    expect(centerSquare(256, 256)).toEqual({ sx: 0, sy: 0, size: 256 })
  })
})

describe('base64Bytes', () => {
  it('discounts the padding', () => {
    expect(base64Bytes(btoa('abc'))).toBe(3)
    expect(base64Bytes(btoa('ab'))).toBe(2)
    expect(base64Bytes(btoa('a'))).toBe(1)
  })
})

describe('parseDataUrl', () => {
  it('splits type and data', () => {
    expect(parseDataUrl('data:image/webp;base64,UklGRg==')).toEqual({ type: 'image/webp', base64: 'UklGRg==' })
  })

  it('rejects anything that is not base64', () => {
    expect(parseDataUrl('data:image/svg+xml,<svg/>')).toBeNull()
    expect(parseDataUrl('https://example.com/photo.png')).toBeNull()
  })
})
