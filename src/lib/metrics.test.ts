import { describe, expect, test } from 'bun:test'
import { readFile } from 'node:fs/promises'
import { z } from 'zod'
import { datasetSchema, detailSchema } from './data'
import type { Attempt, Configuration, Dataset, Outcome, Task } from './data'
import { commonTasks, compareTasks, duration, feedbackSummary, leaderboardComparison, money, scoreRange, summarize, takeaways } from './metrics'

const all = { concern: '', severity: 'all' } satisfies Parameters<typeof summarize>[3]
const imported = datasetSchema.parse(JSON.parse(await readFile('public/data/benchmark.json', 'utf8')))
const configuration: Configuration = { id: 'setup', label: 'Setup', short: 'Setup', version: '1', method: 'builtin', builtin: true, experimental: false, note: '', billing: 'api-dollars', models: ['model'], reasoningEffort: 'high', reasoningSource: 'explicit', reviewEdition: 'baseline', reviewChange: null, skillProvenanceUrl: null, skillReleases: [] }

function task(id: string, count: number): Task {
  return { id, repo: id, pr: 1, head: 'head', base: 'base', shape: 'fixture', language: 'TypeScript', registerVersion: 1,
    profile: { changeKinds: [], areas: [], technologies: [], concerns: [] },
    defects: Array.from({ length: count }, (_, index) => ({ id: `${id}-${index}`, title: 'Problem', trigger: '', consequence: '', requiredOutcome: '', severity: null, concerns: ['Functional'] })),
    registerUrl: '', packetUrl: '', sourceUrl: '' }
}

function attempt(id: string, taskId: string, recovered: string[], overrides: Partial<Attempt> = {}): Attempt {
  return { id, label: id, runId: 'run', taskId, replicate: 1, disposition: 'valid completed', complete: true, admitted: true,
    recovered, falseFindings: 0, rawFalseFindings: 0, noise: 0, unresolved: 0, duplicates: 0, cost: 1, outputTokens: 100,
    durationSeconds: 60, billing: 'api-dollars', predecessor: null, retryReason: null, detailUrl: '', ...overrides }
}

function outcome(taskId: string, trials: string[][]): Outcome {
  return { configurationId: configuration.id, taskId, status: 'ran', reason: '', mappingUrl: null, scorecardUrl: null,
    attemptIds: trials.flat(), trials: trials.map((attemptIds, index) => ({ replicate: index + 1, status: 'valid completed', attemptIds })) }
}

function dataset(tasks: Task[], attempts: Attempt[], outcomes: Outcome[]): Dataset {
  return { schemaVersion: 2, release: 'fixture', revision: 'fixture', profileStatus: 'proposed', grading: { rubricVersion: 2, qualification: '', auditUrl: '', neutralWorkspaceReviews: 0, legacyWorkspaceReviews: 0 }, configurations: [configuration],
    tasks, attempts, outcomes, import: { files: 0, transcripts: 0, mismatches: 0 } }
}

describe('trial scoring', () => {
  test('does not display a measured sub-cent cost as free', () => {
    expect(money(0.0017)).toBe('$0.0017')
    expect(money(0)).toBe('$0.00')
    expect(money(null)).toBe('—')
  })
  test('weights PRs equally, deduplicates recovery, and excludes clean tasks from detection', () => {
    const data = dataset([task('many', 3), task('one', 1), task('clean', 0)],
      [attempt('a', 'many', ['many-0', 'many-0']), attempt('b', 'one', ['one-0']), attempt('c', 'clean', [], { falseFindings: 6 })],
      [outcome('many', [['a']]), outcome('one', [['b']]), outcome('clean', [['c']])])
    const result = summarize(data, configuration, data.tasks, all)
    expect(result.score).toBeCloseTo(100 * (1 / 3 + 1) / 2)
    expect(result.falseFindings).toBe(2)
    expect(result.tasks).toBe(3)
    expect(result.cost).toBe(1)
  })

  test('averages trials within a PR before averaging PRs', () => {
    const data = dataset([task('a', 1), task('b', 1)],
      [attempt('a1', 'a', ['a-0']), attempt('a2', 'a', []), attempt('b1', 'b', ['b-0'])],
      [outcome('a', [['a1'], ['a2']]), outcome('b', [['b1']])])
    expect(summarize(data, configuration, data.tasks, all).score).toBe(75)
  })

  test('counts replacement usage once per trial and rejects invalid original findings', () => {
    const data = dataset([task('a', 1)], [attempt('failed', 'a', ['a-0'], { admitted: false, complete: false, cost: 2, outputTokens: 200 }),
      attempt('replacement', 'a', [], { cost: 3, outputTokens: 300, predecessor: 'failed' })], [outcome('a', [['failed', 'replacement']])])
    const result = summarize(data, configuration, data.tasks, all)
    expect(result.score).toBe(0)
    expect(result.cost).toBe(5)
    expect(result.tokens).toBe(500)
    expect(result.attempts).toBe(2)
    expect(result.trials).toBe(1)
    expect(result.completed).toBe(1)
  })

  test('admits incomplete usable findings and keeps noise separate from detection', () => {
    const data = dataset([task('a', 1)], [attempt('partial', 'a', ['a-0'], { complete: false, falseFindings: 5, unresolved: 2, duplicates: 3, noise: 4 })], [outcome('a', [['partial']])])
    const result = summarize(data, configuration, data.tasks, all)
    expect(result.score).toBe(100)
    expect(result.completed).toBe(0)
    expect(result.falseFindings).toBe(5)
    expect(result.unresolved).toBe(2)
    expect(result.duplicates).toBe(3)
    expect(result.noise).toBe(4)
  })

  test('missing usage and pending trials are unavailable, never zero', () => {
    const data = dataset([task('a', 1)], [attempt('a1', 'a', [], { cost: null, outputTokens: null })], [outcome('a', [['a1']])])
    expect(summarize(data, configuration, data.tasks, all)).toMatchObject({ score: 0, cost: null, tokens: null })
    data.outcomes = [outcome('a', [['a1'], []])]
    expect(summarize(data, configuration, data.tasks, all)).toMatchObject({ score: null, cost: null, tokens: null, falseFindings: null })
  })

  test('a missing replacement cannot fall back to its predecessor', () => {
    const data = dataset([task('a', 1)], [attempt('original', 'a', ['a-0'])],
      [outcome('a', [['original', 'missing-replacement']])])
    expect(summarize(data, configuration, data.tasks, all).score).toBeNull()
    expect(feedbackSummary(data, configuration.id, data.tasks).pendingTrials).toBe(1)
  })

  test('filters reference concerns and does not invent severity for unclassified findings', () => {
    const data = dataset([task('a', 2)], [attempt('a1', 'a', ['a-0'])], [outcome('a', [['a1']])])
    expect(summarize(data, configuration, data.tasks, { concern: 'Security', severity: 'all' }).score).toBeNull()
    expect(summarize(data, configuration, data.tasks, { concern: '', severity: 'high' }).score).toBeNull()
    expect(summarize(data, configuration, data.tasks, all).score).toBe(50)
  })

  test('reports the score range when any one buggy PR is left out', () => {
    const data = dataset([task('a', 1), task('b', 1), task('c', 2), task('clean', 0)],
      [attempt('a1', 'a', ['a-0']), attempt('b1', 'b', []), attempt('c1', 'c', ['c-0']), attempt('k1', 'clean', [])],
      [outcome('a', [['a1']]), outcome('b', [['b1']]), outcome('c', [['c1']]), outcome('clean', [['k1']])])
    expect(scoreRange(data, configuration, data.tasks, all)).toEqual({ low: 25, high: 75 })
    expect(scoreRange(data, configuration, [data.tasks[0]!], all)).toBeNull()
  })
})

describe('takeaways', () => {
  const row = (id: string, score: number | null, cost: number | null, falseFindings: number | null) =>
    ({ ...summarize(dataset([], [], []), { ...configuration, id }, [], all), score, cost, falseFindings })
  test('breaks score ties by cost and only recommends setups at or above the strong-score line', () => {
    const result = takeaways([row('pricey', 90, 4, 0), row('cheap', 90, 0.5, 0.4), row('weak', 50, 0.01, 0), row('quiet', 85, 1, 0)])
    expect(result.top?.configuration.id).toBe('cheap')
    expect(result.cheapest?.configuration.id).toBe('cheap')
    expect(result.quietest?.configuration.id).toBe('pricey')
  })
})

describe('review time', () => {
  test('uses all shared completed trials, including clean tasks, for the median, mean, and middle 50%', () => {
    const data = dataset([task('a', 1), task('clean', 0), task('excluded', 1)],
      [attempt('a1', 'a', ['a-0'], { durationSeconds: 60 }), attempt('a2', 'a', [], { durationSeconds: 120 }),
        attempt('a3', 'a', [], { durationSeconds: 180 }), attempt('c1', 'clean', [], { durationSeconds: 240 }),
        attempt('c2', 'clean', [], { durationSeconds: 300 }), attempt('c3', 'clean', [], { durationSeconds: 900 }),
        attempt('excluded', 'excluded', [], { durationSeconds: 10000 })],
      [outcome('a', [['a1'], ['a2'], ['a3']]), outcome('clean', [['c1'], ['c2'], ['c3']]), outcome('excluded', [['excluded']])])
      expect(summarize(data, configuration, data.tasks.slice(0, 2), all).time).toEqual({
        median: 210, mean: 300, q1: 135, q3: 285, reviews: 6, tasks: 2,
      })
  })

  test('includes every replacement attempt in the completed trial time', () => {
    const data = dataset([task('a', 1)], [
      attempt('failed', 'a', [], { admitted: false, complete: false, durationSeconds: 20 }),
      attempt('replacement', 'a', [], { durationSeconds: 40, predecessor: 'failed' }),
    ], [outcome('a', [['failed', 'replacement']])])
    expect(summarize(data, configuration, data.tasks, all).time).toEqual({
      median: 60, mean: 60, q1: 60, q3: 60, reviews: 1, tasks: 1,
    })
    data.attempts[0] = attempt('failed', 'a', [], { admitted: false, complete: false, durationSeconds: null })
    expect(summarize(data, configuration, data.tasks, all).time).toBeNull()
  })

  test('keeps missing timing unavailable and excludes failed or incomplete terminal reviews', () => {
    const data = dataset([task('a', 1), task('b', 1)], [attempt('a1', 'a', [], { durationSeconds: 120 }),
      attempt('b1', 'b', [], { admitted: false, complete: false, durationSeconds: 1 }),
      attempt('b2', 'b', [], { complete: false, durationSeconds: 10 })],
    [outcome('a', [['a1']]), outcome('b', [['b1'], ['b2']])])
    expect(summarize(data, configuration, data.tasks, all)).toMatchObject({
      time: { median: 120, mean: 120, q1: 120, q3: 120, reviews: 1, tasks: 1 }, completed: 1, trials: 3,
    })
    expect(summarize(data, configuration, data.tasks.slice(1), all).time).toBeNull()
    data.attempts[0] = attempt('a1', 'a', [], { durationSeconds: null })
    expect(summarize(data, configuration, data.tasks, all).time).toBeNull()
    data.outcomes = [outcome('a', [['a1'], []])]
    expect(summarize(data, configuration, data.tasks, all).time).toBeNull()
  })

  test('formats review times with units and preserves a recorded zero', () => {
    expect(duration(0)).toBe('0 s')
    expect(duration(45)).toBe('45 s')
    expect(duration(90)).toBe('1.5 min')
    const data = dataset([task('a', 1)], [attempt('a1', 'a', [], { durationSeconds: 0 })], [outcome('a', [['a1']])])
    expect(summarize(data, configuration, data.tasks, all).time?.median).toBe(0)
  })
})

describe('task sensitivity and feedback workload', () => {
  test('omits entire PRs, weights uneven repetitions equally and excludes clean tasks', () => {
    const data = dataset([task('a', 1), task('b', 1), task('clean', 0)],
      [attempt('a1', 'a', ['a-0']), attempt('a2', 'a', []), attempt('b1', 'b', ['b-0']),
        attempt('other-a', 'a', []), attempt('other-b', 'b', []), attempt('clean', 'clean', [])],
      [outcome('a', [['a1'], ['a2']]), outcome('b', [['b1']]), outcome('clean', [['clean']]),
        { ...outcome('a', [['other-a']]), configurationId: 'other' },
        { ...outcome('b', [['other-b']]), configurationId: 'other' },
        { ...outcome('clean', [['clean']]), configurationId: 'other' }])
    const result = compareTasks(data, configuration.id, 'other', data.tasks, all)
    expect(result.rows).toHaveLength(2)
    expect(result.full.delta).toBe(75)
    expect(result.omissions.map(row => row.delta)).toEqual([100, 50])
    expect(result.rows[0]?.configurations[0]).toMatchObject({ min: 0, max: 100 })
    expect(result).toMatchObject({ wins: 2, ties: 0, losses: 0, pending: 0 })
    expect(compareTasks(data, configuration.id, 'other', data.tasks.filter(task => task.id === 'a'), all).omissions[0]?.delta).toBeNull()
    expect(compareTasks(data, configuration.id, 'other', data.tasks, { concern: 'Security', severity: 'all' }).rows).toHaveLength(0)
  })

  test('keeps missing claim grading unavailable and unadmitted allegations separate', () => {
    const data = dataset([task('a', 1)], [attempt('a1', 'a', [], { feedback: { kind: 'unavailable', observedItems: 5 }, falseFindings: 2 }),
      attempt('stopped', 'a', [], { admitted: false, complete: false, rawFalseFindings: 3 })],
      [outcome('a', [['a1'], ['stopped']])])
    expect(feedbackSummary(data, configuration.id, data.tasks)).toMatchObject({ admittedReviews: 1,
      items: null, itemsPerReview: null, falsePerAdmittedReview: 2, falsePerTrial: 1, advisory: null,
      refuted: null, unsupported: null, claimGradedReviews: 0, unadmittedFalseOccurrences: 3 })
  })

  test('counts claim outcomes and clean exposure with explicit coverage', () => {
    const feedback = { kind: 'claims', items: 1, occurrences: 2, distinct: 2, duplicates: 0, mixedItems: 1, unresolvedItems: 0,
      outcomes: { eligible: { distinct: 1, occurrences: 1 }, advisory: { distinct: 0, occurrences: 0 },
        inconsequential: { distinct: 0, occurrences: 0 }, 'scope-excluded': { distinct: 0, occurrences: 0 },
        refuted: { distinct: 1, occurrences: 1 }, unsupported: { distinct: 0, occurrences: 0 }, unresolved: { distinct: 0, occurrences: 0 } } } satisfies NonNullable<Attempt['feedback']>
    const data = dataset([task('a', 1), task('clean', 0)], [attempt('a1', 'a', ['a-0'], { feedback }),
      attempt('c1', 'clean', [], { feedback: { ...feedback, outcomes: { ...feedback.outcomes, eligible: { distinct: 0, occurrences: 0 } } } })],
      [outcome('a', [['a1']]), outcome('clean', [['c1']])])
    expect(feedbackSummary(data, configuration.id, data.tasks)).toMatchObject({ items: 2, claimGradedReviews: 2,
      refuted: 2, mixedItems: 2, advisory: 0, cleanRefutedFraction: 1, cleanUnsupportedFraction: 0 })
    data.attempts[1] = attempt('c1', 'clean', [], { feedback: { kind: 'unavailable', observedItems: 1 } })
    expect(feedbackSummary(data, configuration.id, data.tasks)).toMatchObject({ items: null, refuted: null, cleanRefutedFraction: null })
  })
})

describe('selected configuration comparison', () => {
  const selected = ['astra', 'luna', 'sol']
  const historicalTasks = Array.from({ length: 12 }, (_, index) => task(`historical-${index}`, 1))
  const selectedTasks = Array.from({ length: 5 }, (_, index) => task(`selected-${index}`, 1))
  const tasks = [...historicalTasks, ...selectedTasks]
  const setups = [...selected, 'other', 'experiment'].map(id => ({ ...configuration, id, experimental: id === 'experiment' }))
  const outcomes = setups.flatMap(setup => {
    const covered = setup.experimental ? historicalTasks.slice(0, 4) : setup.id === 'other' ? historicalTasks.slice(0, 9) : tasks
    return covered.map(task => ({ ...outcome(task.id, [[`${setup.id}/${task.id}`]]), configurationId: setup.id }))
  })
  const records = outcomes.flatMap(row => row.attemptIds.map(id => attempt(id, row.taskId, [`${row.taskId}-0`])))
  const data: Dataset = { ...dataset(tasks, records, outcomes), configurations: setups }

  test('selects all matching tasks for the active baseline configurations', () => {
    expect(leaderboardComparison(data, selected, tasks, all).tasks).toEqual(tasks)
    expect(leaderboardComparison(data, selected, selectedTasks, all).tasks).toEqual(selectedTasks)
    expect(leaderboardComparison(data, setups.filter(setup => !setup.experimental).map(setup => setup.id), tasks, all).tasks).toEqual(historicalTasks.slice(0, 9))
    const selectedComparison = leaderboardComparison(data, selected, selectedTasks, all)
    expect(selectedComparison.summaries.filter(row => selected.includes(row.configuration.id)).every(row => row.tasks === 5 && row.score === 100)).toBe(true)
    expect(selectedComparison.summaries.find(row => row.configuration.id === 'other')?.score).toBeNull()
  })

  test('keeps sparse experiments outside the selected baseline intersection', () => {
    const mixed = leaderboardComparison(data, [...selected, 'experiment'], tasks, all)
    expect(mixed.tasks).toEqual(tasks)
    expect(mixed.summaries.find(row => row.configuration.id === 'experiment')).toMatchObject({ tasks: 4, score: null, cost: null })
    expect(leaderboardComparison(data, ['experiment'], tasks, all).tasks).toEqual(historicalTasks.slice(0, 9))
    expect(leaderboardComparison(data, ['experiment'], historicalTasks.slice(0, 4), all).summaries.find(row => row.configuration.id === 'experiment')?.score).toBe(100)
    expect(leaderboardComparison(data, [], tasks, all).tasks).toEqual([])
  })
})

describe('preserved benchmark', () => {
  const builtins = imported.configurations.filter(row => row.builtin)
  const allConfigurations = imported.configurations.map(row => row.id)
  const selectedRun = '2026-09-30-selected-prs-review-only'
  const selectedTaskIds = new Set(imported.attempts.filter(attempt => attempt.runId === selectedRun).map(attempt => attempt.taskId))
  const historical: Dataset = { ...imported,
    tasks: imported.tasks.filter(task => !selectedTaskIds.has(task.id)),
    attempts: imported.attempts.filter(attempt => !selectedTaskIds.has(attempt.taskId)),
    outcomes: imported.outcomes.filter(row => !selectedTaskIds.has(row.taskId)),
  }
  const shared = commonTasks(imported, builtins.map(row => row.id), imported.tasks)
  const sparseExperiment: Dataset = { ...imported, outcomes: imported.outcomes.filter(row =>
    row.configurationId !== 'review-code-sonnet-5-5' || ['k-graphql-js-1582', 'l-bokeh-9232', 'm-grpc-go-7390', 'p-hono-5067'].includes(row.taskId),
  ) }

  test('keeps sparse skill experiments from shrinking the leaderboard to four tasks', () => {
    const comparison = leaderboardComparison(sparseExperiment, allConfigurations, imported.tasks, all)
    expect(comparison.tasks).toHaveLength(9)
    expect(comparison.summaries.find(row => row.configuration.id === 'claude-builtin-sonnet-5-5')?.score).toBeCloseTo(83.3333333333)
    expect(comparison.summaries.find(row => row.configuration.id === 'claude-builtin-sonnet-5')?.score).toBeCloseTo(75)
    expect(comparison.summaries.find(row => row.configuration.id === 'claude-builtin-opus-5-5')?.score).toBeCloseTo(76.1904761905)
    expect(comparison.summaries.find(row => row.configuration.id === 'review-code-sonnet-5-5')).toMatchObject({
      tasks: 4, score: null, cost: null, tokens: null, falseFindings: null, time: null,
    })
  })

  test('lets sparse experiments be compared when task filters select covered tasks', () => {
    const candidates = imported.tasks.filter(task => task.id === 'p-hono-5067')
    const comparison = leaderboardComparison(sparseExperiment, allConfigurations, candidates, all)
    expect(comparison.tasks).toEqual(candidates)
    expect(comparison.summaries.every(row => row.tasks === 1 && row.score !== null)).toBe(true)
    expect(leaderboardComparison(sparseExperiment, allConfigurations, [], all).summaries.every(row => row.score === null)).toBe(true)
  })

  test('keeps changed reference versions and unrun tasks out of the common comparison', () => {
    expect(imported.tasks).toHaveLength(17)
    expect(shared).toHaveLength(9)
    expect(shared.map(row => row.id)).not.toContain('i-requests-6667')
    expect(shared.map(row => row.id)).not.toContain('s-seaweedfs-10735')
    expect(commonTasks(imported, [], imported.tasks)).toEqual([])
  })

  test('publishes only rubric-v2 claim grading and current reference problems', () => {
    expect(imported.schemaVersion).toBe(2)
    expect(imported.grading.rubricVersion).toBe(2)
    expect(imported.tasks.reduce((sum, task) => sum + task.defects.length, 0)).toBe(30)
    expect(imported.attempts).toHaveLength(778)
    expect(historical.tasks).toHaveLength(12)
    expect(historical.tasks.reduce((sum, task) => sum + task.defects.length, 0)).toBe(17)
    expect(historical.attempts).toHaveLength(604)
    expect(imported.attempts.every(attempt => attempt.feedback?.kind === 'claims' || attempt.feedback?.kind === 'unavailable')).toBe(true)
    expect(imported.grading.neutralWorkspaceReviews + imported.grading.legacyWorkspaceReviews).toBe(imported.attempts.length)
    expect(imported.outcomes.every(outcome => !('historical' in outcome))).toBe(true)
    expect(datasetSchema.safeParse({ ...imported, schemaVersion: 1 }).success).toBe(false)
    expect(datasetSchema.safeParse({ ...imported, grading: { ...imported.grading, rubricVersion: 1 } }).success).toBe(false)
    expect(datasetSchema.safeParse({ ...imported, attempts: [{ ...imported.attempts[0], feedback: { kind: 'legacy', items: 1 } }] }).success).toBe(false)
  })

  test('every exported attempt has readable, matching evidence and verified downloads', async () => {
    for (const attempt of imported.attempts) {
      const detail = detailSchema.parse(JSON.parse(await readFile(`public${attempt.detailUrl}`, 'utf8')))
      expect(detail.id).toBe(attempt.id)
      const source = z.object({ usage: z.object({ metering_status: z.string().optional() }) }).parse(await Bun.file(`public${detail.recordUrl}`).json())
      if (source.usage.metering_status === 'complete') expect(attempt.outputTokens).not.toBeNull()
      else expect(attempt.outputTokens).toBeNull()
      if (attempt.admitted) expect(attempt.durationSeconds).not.toBeNull()
      for (const url of [detail.recordUrl, detail.normalizedUrl, detail.archiveUrl]) {
        if (url) expect((await Bun.file(`public${url}`).stat()).size).toBeGreaterThan(0)
      }
      if (detail.archiveUrl) expect(detail.archiveStatus).toBe('verified')
    }
  })

  test('preserves unknown aggregate usage for interrupted Astra attempts', () => {
    const setup = imported.configurations.find(row => row.id === 'codex-builtin-astra-high')
    if (!setup) throw new Error('Missing Astra High configuration')
    const result = summarize(historical, setup, historical.tasks, all)
    expect(result.score).toBeCloseTo(67.901234568, 5)
    expect(result).toMatchObject({ cost: null, tokens: null, completed: 36, trials: 36, attempts: 42, unresolved: 6 })
  })

  test('reports Sol/Astra sensitivity using the regraded claims', () => {
    const comparison = compareTasks(imported, 'codex-builtin-sol-high', 'codex-builtin-astra-high', imported.tasks, all)
    expect(comparison.rows).toHaveLength(9)
    expect(comparison.full.delta).toBeCloseTo(1.85185185185)
    expect(comparison.omissions.find(row => row.task.id === 'n-ripgrep-2957')?.delta).toBeCloseTo(-10.4166666667)
    expect(comparison.omissions.find(row => row.task.id === 's-seaweedfs-10735')?.delta).toBeCloseTo(8.3333333333)
  })
})
