import { describe, expect, test } from 'bun:test'
import { readFile } from 'node:fs/promises'
import { z } from 'zod'
import { datasetSchema, detailSchema } from './data'
import { duration, money, summarize, takeaways } from './metrics'
import { leaderboard, pairwise } from './scoring'

const imported = datasetSchema.parse(JSON.parse(await readFile('public/data/benchmark.json', 'utf8')))
const standard = imported.configurations.filter(configuration => !configuration.experimental)
const board = leaderboard(imported, { selected: standard.map(configuration => configuration.id),
  candidateTaskIds: imported.tasks.map(task => task.id), concern: null })
const summaries = standard.flatMap(configuration => {
  const card = board.cards.find(item => item.configurationId === configuration.id)
  return card ? [summarize(configuration, card)] : []
})

describe('formatting', () => {
  test('does not display a measured sub-cent cost as free', () => {
    expect(money(0.0017)).toBe('$0.0017')
    expect(money(0)).toBe('$0.00')
    expect(money(null)).toBe('—')
  })

  test('formats review times with units and preserves a recorded zero', () => {
    expect(duration(0)).toBe('0 s')
    expect(duration(45)).toBe('45 s')
    expect(duration(90)).toBe('1.5 min')
  })
})

describe('takeaways', () => {
  const first = summaries[0]
  if (!first) throw new Error('Missing preview configuration')
  const row = (id: string, score: number | null, cost: number | null, refuted: number | null) =>
    ({ ...first, configuration: { ...first.configuration, id }, score, cost, refuted })
  test('breaks score ties by cost and only recommends setups at or above the strong-score line', () => {
    const result = takeaways([row('pricey', 90, 4, 0), row('cheap', 90, 0.5, 0.4), row('weak', 50, 0.01, 0), row('quiet', 85, 1, 0)])
    expect(result.top?.configuration.id).toBe('cheap')
    expect(result.cheapest?.configuration.id).toBe('cheap')
    expect(result.quietest?.configuration.id).toBe('pricey')
  })
})

describe('current ungraded preview', () => {
  test('preserves saved delivery and usage without inventing judgment values', () => {
    expect(imported.evidence.coverage).toMatchObject({ complete: false, assessedReviews: 0, requiredReviews: 733 })
    expect(imported.tasks).toHaveLength(17)
    expect(imported.configurations).toHaveLength(17)
    expect(imported.attempts).toHaveLength(793)
    expect(imported.tasks.flatMap(task => task.families)).toHaveLength(30)
    expect(imported.attempts.every(attempt => attempt.assessment === null)).toBe(true)
    const summary = summaries[0]
    if (!summary) throw new Error('Missing preview configuration')
    expect(summary.score).toBeNull()
    expect(summary.refuted).toBeNull()
    expect(summary.completed).toBeGreaterThan(0)
    expect(summary.cost).not.toBeNull()
    expect(summary.card.detection.all.equalPr).toEqual({ kind: 'unavailable', reason: 'No references in this band.' })
    expect(summary.card.limits.pendingCandidates.length).toBeGreaterThan(0)
    expect(pairwise(summary.card, summary.card, 'all').rows).toHaveLength(0)
  })

  test('every exported attempt has matching source evidence and verified downloads', async () => {
    for (const attempt of imported.attempts) {
      const detail = detailSchema.parse(JSON.parse(await readFile(`public${attempt.detailUrl}`, 'utf8')))
      expect(detail.id).toBe(attempt.id)
      const source = z.object({ usage: z.object({ metering_status: z.string().optional() }) }).parse(await Bun.file(`public${detail.recordUrl}`).json())
      if (source.usage.metering_status === 'complete') expect(attempt.outputTokens).not.toBeNull()
      else expect(attempt.outputTokens).toBeNull()
      for (const url of [detail.recordUrl, detail.normalizedUrl, detail.archiveUrl]) {
        if (url) expect((await Bun.file(`public${url}`).stat()).size).toBeGreaterThan(0)
      }
      if (detail.archiveUrl) expect(detail.archiveStatus).toBe('verified')
    }
  })
})
