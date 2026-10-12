// Pure geometry for the overview's charts: scales, clean ticks and a
// monotone curve (it never overshoots the data, so a count never dips below
// zero between two points).

export interface Point {
  x: number
  y: number
}

/** Ticks from 0 to a clean maximum (1, 2 or 5 times a power of ten). */
export function niceTicks(max: number, count = 4): number[] {
  if (!Number.isFinite(max) || max <= 0) return [0, 1]
  if (!Number.isSafeInteger(count) || count < 1 || count > 100) count = 4
  const raw = max / count
  const power = 10 ** Math.floor(Math.log10(raw))
  const step = [1, 2, 5, 10].map((m) => m * power).find((s) => s >= raw) ?? 10 * power
  if (!Number.isFinite(step) || step <= 0) return [0, max]
  const intervals = Math.ceil(max / step)
  if (!Number.isSafeInteger(intervals) || intervals > 100 || !Number.isFinite(intervals * step)) return [0, max]
  // An index bounds the loop and avoids accumulating a step too small to advance.
  return Array.from({ length: intervals + 1 }, (_, index) => Number((index * step).toPrecision(12)))
}

/** Fritsch–Carlson monotone cubic through the points (sorted by x), as SVG path commands. */
export function monotonePath(points: readonly Point[] | null | undefined): string {
  if (!points?.length) return ''
  if (points.some(point => !point || !Number.isFinite(point.x) || !Number.isFinite(point.y))) return ''
  const n = points.length
  const first = points[0]!
  if (n === 1) return `M${first.x},${first.y}`
  const dx: number[] = []
  const slope: number[] = []
  for (let i = 0; i < n - 1; i++) {
    const a = points[i]!
    const b = points[i + 1]!
    const delta = b.x - a.x
    if (!Number.isFinite(delta) || delta < 0) return ''
    const gradient = delta === 0 ? 0 : (b.y - a.y) / delta
    if (!Number.isFinite(gradient)) return ''
    dx.push(delta)
    slope.push(gradient)
  }
  const tangent: number[] = [slope[0]!]
  for (let i = 1; i < n - 1; i++) {
    const s0 = slope[i - 1]!
    const s1 = slope[i]!
    if (s0 * s1 <= 0) { tangent.push(0); continue }
    const w0 = 2 * dx[i]! + dx[i - 1]!
    const w1 = dx[i]! + 2 * dx[i - 1]!
    tangent.push((w0 + w1) / (w0 / s0 + w1 / s1))
  }
  tangent.push(slope[n - 2]!)
  let path = `M${round(first.x)},${round(first.y)}`
  for (let i = 0; i < n - 1; i++) {
    const a = points[i]!
    const b = points[i + 1]!
    const h = dx[i]! / 3
    const controls = [a.x + h, a.y + tangent[i]! * h, b.x - h, b.y - tangent[i + 1]! * h]
    if (controls.some(value => !Number.isFinite(value))) return ''
    path += `C${controls.map(round).join(',')},${round(b.x)},${round(b.y)}`
  }
  return path
}

/** The same curve closed down to the baseline, for an area fill. */
export function areaPath(points: readonly Point[] | null | undefined, baseline: number): string {
  if (!points?.length || !Number.isFinite(baseline)) return ''
  const path = monotonePath(points)
  if (!path) return ''
  const first = points[0]!
  const last = points[points.length - 1]!
  return `${path}L${round(last.x)},${round(baseline)}L${round(first.x)},${round(baseline)}Z`
}

/** Index of the point nearest to `x` among evenly spaced points. */
export function nearestIndex(x: number, left: number, width: number, count: number): number {
  if (!Number.isSafeInteger(count) || count <= 1 || !Number.isFinite(width) || width <= 0) return 0
  if (!Number.isFinite(x) || !Number.isFinite(left)) return 0
  const ratio = (x - left) / width
  return Math.min(count - 1, Math.max(0, Math.round(ratio * (count - 1))))
}

/** Sums of consecutive groups of `size` values (the last group may be shorter). */
export function groupSums(values: readonly number[] | null | undefined, size: number): number[] {
  if (!values?.length || !Number.isSafeInteger(size) || size <= 0) return []
  const out: number[] = []
  for (let i = 0; i < values.length; i += size) {
    const sum = values.slice(i, i + size).reduce((a, b) => a + (Number.isFinite(b) ? b : 0), 0)
    out.push(Number.isFinite(sum) ? sum : 0)
  }
  return out
}

/** A share in whole percent, 0 when there is nothing to share. */
export function percent(part: number, whole: number): number {
  if (!Number.isFinite(part) || !Number.isFinite(whole) || part < 0 || whole <= 0) return 0
  const share = Math.round((part / whole) * 100)
  if (!Number.isFinite(share)) return 0
  return share
}

const round = (v: number) => Math.abs(v) >= 1e21 ? v : Math.round(v * 100) / 100
