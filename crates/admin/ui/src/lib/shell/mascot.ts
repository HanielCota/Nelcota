// Mascot gaze: eye geometry and pupil direction.
// Adapted from features/mascot/domain/eye-tracking.ts in NelcotaScreenShare.

export type Point = { x: number; y: number }
type Bounds = Pick<DOMRect, 'left' | 'top' | 'width' | 'height'>

/** Side of the frames cropped from the atlas (see src/assets/mascot). */
export const FRAME_SIZE = 480
/** The drawn eyes are tilted ellipses. */
export const EYE_ANGLE = 18

/**
 * Centre of each eye in the frame. The open-eyed poses were cropped anchored
 * at the feet and the right arm, so the face lands in the same place in all of
 * them (under 1.5px apart).
 */
export const EYES = [
  { x: 165.4, y: 222.1, rx: 39, ry: 50, pupilRx: 24.5, pupilRy: 35.5 },
  { x: 304.8, y: 248.7, rx: 42, ry: 51.8, pupilRx: 27.5, pupilRy: 37.3 },
] as const

export type Eye = (typeof EYES)[number]

/**
 * Direction (a vector shorter than 1) from each eye to the point. The depth
 * makes the gaze saturate slowly: near the face it moves a lot, far away barely.
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
 * Pupil offset in the eye's (already rotated) frame, never touching the edge.
 * The result is drawn inside `rotate(EYE_ANGLE)`.
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

/** Moves `from` towards `to` by a fraction per frame (smooth motion). */
export function approach(from: Point, to: Point, factor: number): Point {
  return { x: from.x + (to.x - from.x) * factor, y: from.y + (to.y - from.y) * factor }
}
