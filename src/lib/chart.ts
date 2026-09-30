export type Scale = { kind: 'log' | 'linear'; min: number; max: number }

const steps = [1, 2, 5, 10]
const clean = (value: number) => Number(value.toPrecision(12))

function gridFloor(value: number) {
  const decade = 10 ** Math.floor(Math.log10(value))
  return clean(decade * ([...steps].reverse().find(step => step * decade <= value * (1 + 1e-9)) ?? 1))
}

function gridCeil(value: number) {
  const decade = 10 ** Math.floor(Math.log10(value))
  return clean(decade * (steps.find(step => step * decade >= value * (1 - 1e-9)) ?? 10))
}

export function logScale(values: number[]): Scale {
  const positive = values.filter(value => value > 0)
  if (!positive.length) return { kind: 'log', min: 0.1, max: 1 }
  const min = gridFloor(Math.min(...positive)), max = gridCeil(Math.max(...positive))
  if (Math.log10(max / min) <= 2.2) return { kind: 'log', min, max: max === min ? gridCeil(max * 1.01) : max }
  return { kind: 'log', min: 10 ** Math.floor(Math.log10(min)), max: 10 ** Math.ceil(Math.log10(max)) }
}

export function linearScale(values: number[]): Scale {
  const max = Math.max(0, ...values)
  if (max === 0) return { kind: 'linear', min: 0, max: 1 }
  const step = gridCeil(max / 4)
  return { kind: 'linear', min: 0, max: clean(Math.ceil(max / step - 1e-9) * step) }
}

export function ticks(scale: Scale): number[] {
  if (scale.kind === 'linear') {
    const step = gridCeil(scale.max / 4)
    return Array.from({ length: Math.round(scale.max / step) + 1 }, (_, index) => clean(index * step))
  }
  const decades = Math.log10(scale.max / scale.min)
  const multiples = decades <= 2.2 ? [1, 2, 5] : [1]
  const values: number[] = []
  for (let exponent = Math.floor(Math.log10(scale.min)); exponent <= Math.ceil(Math.log10(scale.max)); exponent++) {
    for (const multiple of multiples) {
      const value = clean(multiple * 10 ** exponent)
      if (value >= scale.min * (1 - 1e-9) && value <= scale.max * (1 + 1e-9)) values.push(value)
    }
  }
  return values
}

export function position(scale: Scale, value: number, from: number, to: number) {
  const clamped = Math.min(scale.max, Math.max(scale.min, value))
  const fraction = scale.kind === 'log'
    ? Math.log(clamped / scale.min) / Math.log(scale.max / scale.min)
    : (clamped - scale.min) / (scale.max - scale.min)
  return from + fraction * (to - from)
}

/** Points no other point beats on both lower x and higher y, ordered by x. */
export function paretoFrontier<T extends { x: number; y: number }>(points: T[]): T[] {
  const frontier: T[] = []
  for (const point of [...points].sort((left, right) => left.x - right.x || right.y - left.y)) {
    const best = frontier.at(-1)
    if (!best || point.y > best.y) frontier.push(point)
  }
  return frontier
}

export type Box = { left: number; top: number; right: number; bottom: number }
export type LabelRequest = { id: string; x: number; y: number; width: number; height: number }
export type LabelPlacement = { id: string; x: number; y: number; anchor: 'start' | 'middle' | 'end'; box: Box; leader: boolean }

const overlaps = (a: Box, b: Box) => a.left < b.right && b.left < a.right && a.top < b.bottom && b.top < a.bottom

/** Greedy placement in request order; a label that fits nowhere is left to the tooltip rather than drawn over data. */
export function placeLabels(requests: LabelRequest[], obstacles: Box[], bounds: Box): LabelPlacement[] {
  const taken = [...obstacles]
  const placed: LabelPlacement[] = []
  for (const request of requests) {
    const found = [10, 24, 40].flatMap(distance => {
      const diagonal = distance * 0.72
      const candidates: [number, number, LabelPlacement['anchor']][] = [
        [distance, 0, 'start'], [-distance, 0, 'end'], [diagonal, -diagonal, 'start'], [diagonal, diagonal, 'start'],
        [-diagonal, -diagonal, 'end'], [-diagonal, diagonal, 'end'], [0, -distance - 2, 'middle'], [0, distance + 2, 'middle'],
      ]
      return candidates.map(([dx, dy, anchor]) => {
        const x = request.x + dx, y = request.y + dy
        const left = anchor === 'start' ? x : anchor === 'end' ? x - request.width : x - request.width / 2
        const box = { left: left - 2, top: y - request.height / 2 - 1, right: left + request.width + 2, bottom: y + request.height / 2 + 1 }
        return { id: request.id, x, y, anchor, box, leader: distance > 10 }
      })
    }).find(candidate => candidate.box.left >= bounds.left && candidate.box.right <= bounds.right &&
      candidate.box.top >= bounds.top && candidate.box.bottom <= bounds.bottom && !taken.some(box => overlaps(box, candidate.box)))
    if (found) { placed.push(found); taken.push(found.box) }
  }
  return placed
}

export function groupPaths<T extends { x: number; y: number }>(points: T[], key: (point: T) => string): { key: string; points: T[] }[] {
  const groups = new Map<string, T[]>()
  for (const point of points) groups.set(key(point), [...groups.get(key(point)) ?? [], point])
  return Array.from(groups, ([groupKey, members]) => ({ key: groupKey, points: [...members].sort((left, right) => left.x - right.x || left.y - right.y) }))
}

export function segmentObstacles(points: { x: number; y: number }[], spacing = 7, radius = 2.5): Box[] {
  return points.slice(1).flatMap((point, index) => {
    const previous = points[index]!
    const count = Math.max(1, Math.ceil(Math.hypot(point.x - previous.x, point.y - previous.y) / spacing))
    return Array.from({ length: count + 1 }, (_, step) => {
      const x = previous.x + (point.x - previous.x) * step / count, y = previous.y + (point.y - previous.y) * step / count
      return { left: x - radius, top: y - radius, right: x + radius, bottom: y + radius }
    })
  })
}
