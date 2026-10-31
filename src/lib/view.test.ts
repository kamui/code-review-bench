import { describe, expect, test } from 'bun:test'
import { scorecardFixture } from './fixture'
import { explorerSearch, inspectionOf, inspectionParams, listParam, listValue } from './view'

describe('view links', () => {
  const known = ['a', 'b', 'c']

  test('a selection equal to its default stays out of the link, in any order', () => {
    expect(listParam(['b', 'a'], known, ['a', 'b'])).toBeUndefined()
    expect(listValue(undefined, known, ['a', 'b'])).toEqual(['a', 'b'])
  })

  test('any other selection is written in dataset order and read back without unknown or repeated ids', () => {
    expect(listParam(['c', 'a', 'a', 'gone'], known, ['a', 'b'])).toBe('a,c')
    expect(listValue('c,gone,a,c', known, ['a', 'b'])).toEqual(['c', 'a'])
  })

  test('an empty selection survives the link', () => {
    expect(listParam([], known, ['a'])).toBe('')
    expect(listValue('', known, ['a'])).toEqual([])
  })

  test('an invalid or unknown value falls back to the default and keeps the rest', () => {
    expect(explorerSearch.parse({ chart: 'pie', band: 'serious', experiments: 'yes', dimension: 'delivery', setups: 5, extra: 'x' }))
      .toEqual({ chart: undefined, band: 'serious', experiments: undefined, dimension: 'delivery', setups: undefined })
  })

  test('an open setup or PR, its review and its tab round-trip; an unknown one opens nothing', () => {
    const dataset = scorecardFixture()
    const [configuration] = dataset.configurations, [task] = dataset.tasks
    if (!configuration || !task) throw new Error('fixture has no setup or task')
    for (const inspection of [{ kind: 'configuration' as const, id: configuration.id, tab: 'coverage' as const },
      { kind: 'task' as const, id: task.id, attemptId: 'run/att-001', tab: 'references' as const }])
      expect(inspectionOf(inspectionParams(inspection), dataset)).toEqual({ attemptId: undefined, ...inspection })
    expect(inspectionParams({ kind: 'task', id: task.id, tab: 'reviews' })).toEqual({ inspect: `task:${task.id}`, attempt: undefined, drawer: undefined })
    expect(inspectionOf({ inspect: 'task:missing' }, dataset)).toBeNull()
    expect(inspectionOf({ inspect: `pr:${task.id}` }, dataset)).toBeNull()
  })
})
