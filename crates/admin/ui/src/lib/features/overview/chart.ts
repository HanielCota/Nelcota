// Pure geometry for the overview's charts: scales, clean ticks and a
// monotone curve (it never overshoots the data, so a count never dips below
// zero between two points).

export interface Point {
  x: number
  y: number
}

/** Ticks from 0 to a clean maximum (1, 2 or 5 times a power of ten). */
export function niceTicks(max: number, count = 4): number[] {
  if (!(max > 0)) return [0, 1]
  const raw = max / count
  const power = 10 ** Math.floor(Math.log10(raw))
  const step = [1, 2, 5, 10].map((m) => m * power).find((s) => s >= raw) ?? 10 * power
  const top = Math.ceil(max / step) * step
  const ticks: number[] = []
  for (let v = 0; v <= top + step / 2; v += step) ticks.push(Math.round(v * 1e9) / 1e9)
  return ticks
}

/** Fritsch–Carlson monotone cubic through the points (sorted by x), as SVG path commands. */
export function monotonePath(points: readonly Point[]): string {
  const n = points.length
  if (n === 0) return ''
  const first = points[0]!
  if (n === 1) return `M${first.x},${first.y}`
  const dx: number[] = []
  const slope: number[] = []
  for (let i = 0; i < n - 1; i++) {
    const a = points[i]!
    const b = points[i + 1]!
    dx.push(b.x - a.x)
    slope.push(dx[i]! === 0 ? 0 : (b.y - a.y) / dx[i]!)
  }
  const tangent: number[] = [slope[0]!]
  for (let i = 1; i < n - 1; i++) {
    const s0 = slope[i - 1]!
    const s1 = slope[i]!
    if (s0 * s1 <= 0) tangent.push(0)
    else {
      const w0 = 2 * dx[i]! + dx[i - 1]!
      const w1 = dx[i]! + 2 * dx[i - 1]!
      tangent.push((w0 + w1) / (w0 / s0 + w1 / s1))
    }
  }
  tangent.push(slope[n - 2]!)
  let path = `M${round(first.x)},${round(first.y)}`
  for (let i = 0; i < n - 1; i++) {
    const a = points[i]!
    const b = points[i + 1]!
    const h = dx[i]! / 3
    path += `C${round(a.x + h)},${round(a.y + tangent[i]! * h)},${round(b.x - h)},${round(b.y - tangent[i + 1]! * h)},${round(b.x)},${round(b.y)}`
  }
  return path
}

/** The same curve closed down to the baseline, for an area fill. */
export function areaPath(points: readonly Point[], baseline: number): string {
  if (points.length === 0) return ''
  const first = points[0]!
  const last = points[points.length - 1]!
  return `${monotonePath(points)}L${round(last.x)},${round(baseline)}L${round(first.x)},${round(baseline)}Z`
}

/** Index of the point nearest to `x` among evenly spaced points. */
export function nearestIndex(x: number, left: number, width: number, count: number): number {
  if (count <= 1 || width <= 0) return 0
  const ratio = (x - left) / width
  return Math.min(count - 1, Math.max(0, Math.round(ratio * (count - 1))))
}

/** Sums of consecutive groups of `size` values (the last group may be shorter). */
export function groupSums(values: readonly number[], size: number): number[] {
  const out: number[] = []
  for (let i = 0; i < values.length; i += size) out.push(values.slice(i, i + size).reduce((a, b) => a + b, 0))
  return out
}

/** A share in whole percent, 0 when there is nothing to share. */
export const percent = (part: number, whole: number) => (whole > 0 ? Math.round((part / whole) * 100) : 0)

const round = (v: number) => Math.round(v * 100) / 100
