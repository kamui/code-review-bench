import { describe, expect, test } from 'bun:test'
import { readFile } from 'node:fs/promises'
import { z } from 'zod'
import { datasetSchema, detailSchema } from './data'
import { build, family, review, scorecardFixture, setup, task } from './fixture'
import { conditionDifferences, datasetStatus, detectionLabel, duration, heroCounts, money, perReview, reasonCounts, share, summarize } from './metrics'
import { leaderboard, matched, pairwise, scorecard } from './scoring'

const imported = datasetSchema.parse(JSON.parse(await readFile('public/data/benchmark.json', 'utf8')))
const standard = imported.configurations.filter(configuration => !configuration.experimental)
const board = leaderboard(imported, { selected: standard.map(configuration => configuration.id),
  candidateTaskIds: imported.tasks.map(task => task.id), concern: null })
const summaries = standard.flatMap(configuration => {
  const card = board.cards.find(item => item.configurationId === configuration.id)
  const refuted = matched(imported, standard.map(item => item.id), board.selection, 'refuted').rows.find(row => row.configurationId === configuration.id)
  return card && refuted ? [summarize(configuration, card, { band: 'all', estimator: 'equalPr' }, refuted.equalPr)] : []
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

describe('scorecard display', () => {
  const fixture = scorecardFixture()
  const everything = { taskIds: fixture.tasks.map(item => item.id), concern: null }
  const card = (id: string, selection = everything) => scorecard(fixture, id, selection)
  const own = (id: string, selection = everything) => card(id, selection).reliability.outcomes.refuted.perAdmittedReview
  const configuration = (id: string) => {
    const found = fixture.configurations.find(item => item.id === id)
    if (!found) throw new Error(`Missing fixture configuration ${id}`)
    return found
  }

  test('plots only the selected band and average and never substitutes another', () => {
    const serious = summarize(configuration('steady'), card('steady'), { band: 'serious', estimator: 'equalProblem' }, own('steady'))
    expect(serious).toMatchObject({ detection: 100, reasons: { detection: null } })
    expect(detectionLabel({ band: 'serious', estimator: 'equalProblem' })).toBe('Serious detection, problems weighted equally')
    const unlabelled = { taskIds: ['docs'], concern: null }
    const empty = summarize(configuration('steady'), card('steady', unlabelled), { band: 'serious', estimator: 'equalProblem' }, own('steady', unlabelled))
    expect(empty).toMatchObject({ detection: null, range: null, reasons: { detection: 'No references in this band.' } })
    expect(summarize(configuration('steady'), card('steady', unlabelled), { band: 'unknown', estimator: 'equalProblem' }, own('steady', unlabelled)).detection).toBe(50)
    expect(summarize(configuration('selective'), card('selective'), { band: 'serious', estimator: 'equalProblem' }, own('selective')).detection).toBeCloseTo(100 / 9)
    expect(summarize(configuration('selective'), card('selective'), { band: 'serious', estimator: 'equalPr' }, own('selective')).detection).toBeCloseTo(100 / 12)
  })

  test('the plotted refuted rate is matched on commonly admitted PRs, not the rate over all admitted reviews of a setup', () => {
    const pair = matched(fixture, ['steady', 'selective'], everything, 'refuted')
    expect(pair.included).toEqual(fixture.tasks.map(item => item.id))
    const [, selective] = pair.rows
    if (!selective) throw new Error('Missing matched row')
    const plotted = summarize(configuration('selective'), card('selective'), { band: 'serious', estimator: 'equalProblem' }, selective.equalPr)
    expect(plotted.refuted).toBeCloseTo(2 / 6)
    expect(plotted.admitted).toBe(16)
    expect(perReview(own('selective')).text).toBe('0.13')
    const withSilent = matched(fixture, ['steady', 'silent'], everything, 'refuted')
    expect(withSilent.included).toEqual([])
    expect(summarize(configuration('steady'), card('steady'), { band: 'serious', estimator: 'equalProblem' }, withSilent.rows[0]?.equalPr ?? own('steady')))
      .toMatchObject({ refuted: null, reasons: { refuted: 'No PR is commonly admitted and sufficiently assessed.' } })
  })

  test('unavailable values carry their reason and are counted, never shown as zero', () => {
    expect(perReview(card('silent').reliability.outcomes.refuted.perAdmittedReview)).toEqual({ text: 'Unavailable', reason: 'No admitted reviews.' })
    expect(share(card('silent').controls.cleanFraction)).toEqual({ text: 'Unavailable', reason: 'No admitted reviews on audited controls.' })
    expect(share(card('waiting').detection.serious.equalProblem)).toEqual({ text: 'Unavailable', reason: '1 scheduled trial awaits execution.' })
    expect(share(card('ungraded').detection.serious.equalProblem)).toEqual({ text: 'Unavailable', reason: '1 admitted review awaits assessment.' })
    expect(share(card('sparse').detection.serious.equalProblem)).toEqual({ text: 'Unavailable', reason: 'Ran 2 of 6 selected PRs.' })
    expect(share(card('steady').detection.serious.equalProblem)).toEqual({ text: '100.0%', reason: null })
    expect(reasonCounts(['a', null, 'b', 'a'])).toEqual([{ reason: 'a', count: 2 }, { reason: 'b', count: 1 }])
  })

  test('hero counts cover the whole export and ignore effort, client and repetition variations', () => {
    expect(heroCounts(fixture)).toEqual({ tasks: 6, problems: 10, methods: 2, models: 4 })
    const subject = task('pr', [family('f')])
    const cells = { a: [[subject, [review(subject, []), review(subject, [])]]], b: [[subject, [review(subject, [])]]], c: [[subject, [review(subject, [])]]] } satisfies Parameters<typeof build>[1]
    const varied = build([subject], cells, { experimental: ['c'], configurations: { a: { method: 'codex', models: ['m1'], reasoningEffort: 'high', version: 'codex-cli 1' },
      b: { method: 'codex', models: ['m1'], reasoningEffort: 'medium', version: 'codex-cli 2' }, c: { method: 'review-code', builtin: false, models: ['m2'] } } })
    expect(heroCounts(varied)).toEqual({ tasks: 1, problems: 1, methods: 2, models: 2 })
  })

  test('a preview with missing assessments or no audit is never labelled the completed dataset', () => {
    expect(datasetStatus(fixture)).toEqual({ label: 'v1 preview', complete: false, limits: [
      'Fixture data for interface checks, not benchmark evidence. 92 of 93 admitted reviews are assessed.',
      'The evaluator audit is unassessed. The fixture declares no evaluator audit.'] })
    const subject = task('pr', [family('f')])
    const cells = { a: [[subject, [review(subject, ['f'])]]] } satisfies Parameters<typeof build>[1]
    expect(datasetStatus(build([subject], cells, { audit: 'unassessed' })).label).toBe('v1 preview')
    expect(datasetStatus(build([subject], cells))).toEqual({ label: 'Current v1', complete: true, limits: [] })
  })

  test('names the recorded conditions that differ between compared setups', () => {
    expect(conditionDifferences([configuration('steady'), configuration('selective')]).map(row => [row.name, row.values.map(item => item.value)])).toEqual([
      ['Client', ['claude-code 2.1.284', 'codex-cli 0.159.0']], ['Network access', ['off', 'on']], ['Sandbox', ['n/a', 'workspace-write']]])
    expect(conditionDifferences([configuration('steady'), configuration('silent')])).toEqual([])
    expect(conditionDifferences([setup('a', false, { conditions: [{ name: 'Client', values: ['x'] }] }), setup('b')]).map(row => row.values.map(item => item.value)))
      .toEqual([['x', 'unrecorded']])
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
    expect(summary.detection).toBeNull()
    expect(summary.reasons.detection).toMatch(/^\d+ admitted reviews await assessment\.$/)
    expect(summary.refuted).toBeNull()
    expect(summary.completed).toBeGreaterThan(0)
    expect(summary.cost).not.toBeNull()
    for (const band of ['all', 'unknown'] as const) {
      expect(summary.card.detection[band].equalPr).toEqual({ kind: 'unavailable', reason: expect.stringMatching(/^\d+ admitted reviews await assessment\.$/) })
    }
    for (const band of ['serious', 'other-material'] as const) {
      expect(summary.card.detection[band].equalPr).toEqual({ kind: 'unavailable', reason: 'No references in this band.' })
    }
    expect(summary.card.controls.cleanFraction).toEqual({ kind: 'unavailable', reason: expect.stringMatching(/^\d+ admitted control reviews await assessment\.$/) })
    expect(summary.card.limits.pendingCandidates.length).toBeGreaterThan(0)
    expect(summary.card.limits.auditComplete).toBe(false)
    const comparison = pairwise(summary.card, summary.card, 'all')
    expect(comparison.rows.length).toBeGreaterThan(0)
    expect(comparison.rows.every(row => row.delta.kind === 'unavailable')).toBe(true)
    expect(comparison).toMatchObject({ wins: 0, ties: 0, losses: 0, pending: comparison.rows.length })
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
