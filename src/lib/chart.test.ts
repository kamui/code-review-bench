import { describe, expect, test } from 'bun:test'
import { groupPaths, linearScale, logScale, paretoFrontier, placeLabels, position, segmentObstacles, ticks } from './chart'

describe('scales', () => {
  test('spans wide ranges in whole decades and narrow ranges on a 1-2-5 grid', () => {
    const cost = logScale([0.0038, 0.12, 5.54])
    expect(cost).toEqual({ kind: 'log', min: 0.001, max: 10 })
    expect(ticks(cost)).toEqual([0.001, 0.01, 0.1, 1, 10])
    const minutes = logScale([0.6, 3.2, 11])
    expect(minutes).toEqual({ kind: 'log', min: 0.5, max: 20 })
    expect(ticks(minutes)).toEqual([0.5, 1, 2, 5, 10, 20])
  })
  test('starts linear scales at zero with round ticks', () => {
    expect(ticks(linearScale([0, 0.07, 0.6]))).toEqual([0, 0.2, 0.4, 0.6])
    expect(linearScale([0, 0])).toEqual({ kind: 'linear', min: 0, max: 1 })
  })
  test('places log values proportionally and clamps outside the domain', () => {
    const scale = logScale([0.01, 1])
    expect(position(scale, 0.1, 0, 100)).toBeCloseTo(50)
    expect(position(scale, 0, 0, 100)).toBe(0)
  })
})

describe('pareto frontier', () => {
  test('keeps only points not beaten on both lower x and higher y', () => {
    const points = [{ x: 1, y: 50 }, { x: 2, y: 40 }, { x: 3, y: 90 }, { x: 3, y: 70 }, { x: 5, y: 90 }, { x: 0.5, y: 20 }]
    expect(paretoFrontier(points)).toEqual([{ x: 0.5, y: 20 }, { x: 1, y: 50 }, { x: 3, y: 90 }])
  })
})

describe('label placement', () => {
  test('never overlaps marks or other labels and drops labels with no free position', () => {
    const bounds = { left: 0, top: 0, right: 200, bottom: 100 }
    const marks = [{ left: 44, top: 44, right: 56, bottom: 56 }, { left: 52, top: 44, right: 64, bottom: 56 }]
    const placed = placeLabels([{ id: 'a', x: 50, y: 50, width: 40, height: 12 }, { id: 'b', x: 58, y: 50, width: 40, height: 12 }], marks, bounds)
    expect(placed).toHaveLength(2)
    const [first, second] = placed
    expect(first!.box.right <= second!.box.left || second!.box.right <= first!.box.left ||
      first!.box.bottom <= second!.box.top || second!.box.bottom <= first!.box.top).toBe(true)
    expect(placeLabels([{ id: 'wide', x: 10, y: 10, width: 500, height: 12 }], [], bounds)).toEqual([])
  })
})

describe('method paths', () => {
  test('joins each group left to right and keeps groups apart', () => {
    const points = [{ x: 3, y: 60, g: 'a' }, { x: 1, y: 70, g: 'a' }, { x: 2, y: 80, g: 'b' }, { x: 2, y: 50, g: 'a' }]
    expect(groupPaths(points, point => point.g).map(group => [group.key, group.points.map(point => point.x)]))
      .toEqual([['a', [1, 2, 3]], ['b', [2]]])
  })
  test('covers a line with obstacles from end to end', () => {
    const boxes = segmentObstacles([{ x: 0, y: 0 }, { x: 70, y: 0 }], 7, 2)
    expect(boxes.at(0)).toEqual({ left: -2, top: -2, right: 2, bottom: 2 })
    expect(boxes.at(-1)).toEqual({ left: 68, top: -2, right: 72, bottom: 2 })
    expect(boxes.every((box, index) => index === 0 || box.left - boxes[index - 1]!.left <= 7)).toBe(true)
  })
})
