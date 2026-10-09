import { describe, expect, it } from 'vitest'
import { areaPath, groupSums, monotonePath, nearestIndex, niceTicks, percent } from './chart'

describe('chart geometry', () => {
  it('ticks end on a clean number above the maximum', () => {
    expect(niceTicks(87)).toEqual([0, 50, 100])
    expect(niceTicks(70)).toEqual([0, 20, 40, 60, 80])
    expect(niceTicks(4)).toEqual([0, 1, 2, 3, 4])
    expect(niceTicks(1234)).toEqual([0, 500, 1000, 1500])
    expect(niceTicks(0)).toEqual([0, 1])
  })

  it('the monotone curve never overshoots between points', () => {
    const points = [
      { x: 0, y: 100 },
      { x: 10, y: 0 },
      { x: 20, y: 0 },
      { x: 30, y: 50 },
    ]
    const path = monotonePath(points)
    expect(path.startsWith('M0,100C')).toBe(true)
    // Control points stay within the data's y range [0, 100].
    const ys = [...path.matchAll(/,(-?[\d.]+)/g)].map((m) => Number(m[1]))
    expect(Math.min(...ys)).toBeGreaterThanOrEqual(0)
    expect(Math.max(...ys)).toBeLessThanOrEqual(100)
  })

  it('an area closes down to the baseline', () => {
    expect(areaPath([{ x: 0, y: 5 }, { x: 10, y: 1 }], 20)).toMatch(/L10,20L0,20Z$/)
    expect(monotonePath([])).toBe('')
    expect(monotonePath([{ x: 3, y: 4 }])).toBe('M3,4')
  })

  it('finds the nearest of evenly spaced points', () => {
    expect(nearestIndex(0, 0, 100, 5)).toBe(0)
    expect(nearestIndex(60, 0, 100, 5)).toBe(2)
    expect(nearestIndex(500, 0, 100, 5)).toBe(4)
    expect(nearestIndex(-5, 0, 100, 5)).toBe(0)
  })

  it('groups and shares', () => {
    expect(groupSums([1, 2, 3, 4, 5], 2)).toEqual([3, 7, 5])
    expect(percent(1, 3)).toBe(33)
    expect(percent(1, 0)).toBe(0)
  })
})
