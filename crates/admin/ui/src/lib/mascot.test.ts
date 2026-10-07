import { describe, expect, it } from 'vitest'
import { EYES, FRAME_SIZE, approach, gazeAt, pupilOffset } from './mascot'

const bounds = { left: 0, top: 0, width: FRAME_SIZE, height: FRAME_SIZE }
const center = (i: number) => ({ x: EYES[i].x, y: EYES[i].y })

describe('gazeAt', () => {
  it('olha para frente quando o ponto está no próprio olho', () => {
    const [left] = gazeAt(bounds, center(0))
    expect(left.x).toBeCloseTo(0)
    expect(left.y).toBeCloseTo(0)
  })

  it('aponta para o lado do ponto', () => {
    const [left, right] = gazeAt(bounds, { x: 2000, y: 230 })
    expect(left.x).toBeGreaterThan(0.9)
    expect(right.x).toBeGreaterThan(0.9)
    const [up] = gazeAt(bounds, { x: EYES[0].x, y: -2000 })
    expect(up.y).toBeLessThan(-0.9)
  })

  it('escala com o tamanho em que o mascote é desenhado', () => {
    const small = { left: 100, top: 50, width: 144, height: 144 }
    const eye = { x: 100 + (144 * EYES[0].x) / FRAME_SIZE, y: 50 + (144 * EYES[0].y) / FRAME_SIZE }
    const [left] = gazeAt(small, eye)
    expect(left.x).toBeCloseTo(0)
  })
})

describe('pupilOffset', () => {
  it('fica no centro sem direção', () => {
    const p = pupilOffset({ x: 0, y: 0 }, EYES[0])
    expect(p.x).toBeCloseTo(0)
    expect(p.y).toBeCloseTo(0)
  })

  it('nunca sai do olho', () => {
    for (const eye of EYES) {
      for (let a = 0; a < 2 * Math.PI; a += Math.PI / 8) {
        const p = pupilOffset({ x: Math.cos(a) * 3, y: Math.sin(a) * 3 }, eye)
        // Borda da pupila deslocada continua dentro da elipse do olho.
        expect(Math.abs(p.x) + eye.pupilRx).toBeLessThanOrEqual(eye.rx)
        expect(Math.abs(p.y) + eye.pupilRy).toBeLessThanOrEqual(eye.ry)
      }
    }
  })
})

describe('approach', () => {
  it('anda a fração pedida', () => {
    expect(approach({ x: 0, y: 0 }, { x: 10, y: -10 }, 0.25)).toEqual({ x: 2.5, y: -2.5 })
  })
})
