// Olhar do mascote: geometria dos olhos e direção das pupilas.
// Adaptado de features/mascot/domain/eye-tracking.ts do NelcotaScreenShare.

export type Point = { x: number; y: number }
type Bounds = Pick<DOMRect, 'left' | 'top' | 'width' | 'height'>

/** Lado dos quadros recortados do atlas (ver src/assets/mascot). */
export const FRAME_SIZE = 480
/** Os olhos do desenho são elipses inclinadas. */
export const EYE_ANGLE = 18

/**
 * Centro de cada olho no quadro. As poses com olhos abertos foram recortadas
 * ancoradas nos pés e no braço direito, então o rosto cai no mesmo lugar em
 * todas (diferença abaixo de 1,5px).
 */
export const EYES = [
  { x: 165.4, y: 222.1, rx: 39, ry: 50, pupilRx: 24.5, pupilRy: 35.5 },
  { x: 304.8, y: 248.7, rx: 42, ry: 51.8, pupilRx: 27.5, pupilRy: 37.3 },
] as const

export type Eye = (typeof EYES)[number]

/**
 * Direção (vetor de comprimento < 1) de cada olho até o ponto. A profundidade
 * faz o olhar saturar devagar: perto do rosto ele se move muito, longe quase nada.
 */
export function gazeAt(bounds: Bounds, point: Point): Point[] {
  const depth = Math.max(24, bounds.width * 0.42)
  return EYES.map((eye) => {
    const dx = point.x - (bounds.left + (bounds.width * eye.x) / FRAME_SIZE)
    const dy = point.y - (bounds.top + (bounds.height * eye.y) / FRAME_SIZE)
    const distance = Math.hypot(dx, dy, depth)
    return { x: dx / distance, y: dy / distance }
  })
}

/**
 * Deslocamento da pupila no sistema do olho (já girado), sem encostar na borda.
 * O resultado é desenhado dentro de `rotate(EYE_ANGLE)`.
 */
export function pupilOffset(gaze: Point, eye: Eye): Point {
  const angle = (EYE_ANGLE * Math.PI) / 180
  const reach = Math.max(1, Math.hypot(gaze.x, gaze.y))
  const x = gaze.x / reach
  const y = gaze.y / reach
  const marginX = Math.max(0, eye.rx - eye.pupilRx - 1.5)
  const marginY = Math.max(0, eye.ry - eye.pupilRy - 1.5)
  return {
    x: (x * Math.cos(angle) + y * Math.sin(angle)) * marginX,
    y: (-x * Math.sin(angle) + y * Math.cos(angle)) * marginY,
  }
}

/** Aproxima `from` de `to` numa fração por quadro (movimento suave). */
export function approach(from: Point, to: Point, factor: number): Point {
  return { x: from.x + (to.x - from.x) * factor, y: from.y + (to.y - from.y) * factor }
}
