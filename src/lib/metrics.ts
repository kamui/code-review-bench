import type { Configuration, Dataset } from './data'
import type { Band, Estimator, Measure, Scorecard } from './scoring'

export type Axis = 'cost' | 'tokens' | 'refuted' | 'time'
export type DetectionView = { band: Band; estimator: Estimator }
export type Reading = { text: string; reason: string | null }

export const bandLabels: Record<Band, string> = { serious: 'Serious', 'other-material': 'Other material', unknown: 'Unknown impact', all: 'All references' }
export const estimatorLabels: Record<Estimator, string> = { equalProblem: 'problems weighted equally', equalPr: 'PRs weighted equally' }
export const detectionLabel = (view: DetectionView) => `${bandLabels[view.band]} detection, ${estimatorLabels[view.estimator]}`
export const lowerFirst = (label: string) => label.charAt(0).toLowerCase() + label.slice(1)

export const measured = <T>(measure: Measure<T>): T | null => measure.kind === 'available' ? measure.value : null
export const scaled = (fraction: number | null): number | null => fraction === null ? null : fraction * 100
export const points = (measure: Measure): number | null => scaled(measured(measure))
const reasonOf = (measure: Measure<unknown>) => measure.kind === 'unavailable' ? measure.reason : null

export const reading = <T>(measure: Measure<T>, format: (value: T) => string): Reading =>
  measure.kind === 'available' ? { text: format(measure.value), reason: null } : { text: 'Unavailable', reason: measure.reason }

export function reasonCounts(reasons: (string | null)[]) {
  const counts = new Map<string, number>()
  for (const reason of reasons) if (reason !== null) counts.set(reason, (counts.get(reason) ?? 0) + 1)
  return Array.from(counts, ([reason, count]) => ({ reason, count }))
}

export type Summary = {
  configuration: Configuration
  card: Scorecard
  detection: number | null
  range: { low: number; high: number } | null
  cost: number | null
  tokens: number | null
  refuted: number | null
  time: { median: number; mean: number; q1: number; q3: number; reviews: number; tasks: number } | null
  reasons: Record<'detection' | Axis, string | null>
  admitted: number
  completed: number
  trials: number
  tasks: number
}

/** `matchedRefuted` is the setup's rate on the PRs every compared setup admitted, so the plotted reliability axis compares like with like. */
export function summarize(configuration: Configuration, card: Scorecard, view: DetectionView, matchedRefuted: Measure): Summary {
  const recall = card.detection[view.band], range = measured(recall.omissions[view.estimator]), time = measured(card.time.summary)
  const refuted = matchedRefuted
  return {
    configuration, card, detection: points(recall[view.estimator]),
    range: range && { low: range.low * 100, high: range.high * 100 },
    cost: measured(card.cost.perTrial), tokens: measured(card.tokens.perTrial), refuted: measured(refuted),
    time: time && { ...time, reviews: card.time.completed, tasks: card.time.tasks },
    reasons: { detection: reasonOf(recall[view.estimator]), cost: reasonOf(card.cost.perTrial), tokens: reasonOf(card.tokens.perTrial),
      refuted: reasonOf(refuted), time: reasonOf(card.time.summary) },
    admitted: card.delivery.admitted, completed: card.delivery.complete, trials: card.delivery.scheduled, tasks: card.coverage.ran,
  }
}

/** Effort, client version and repetition are properties of a setup, so they add no method or model. */
export function heroCounts(dataset: Dataset) {
  const families = dataset.tasks.flatMap(task => task.families)
  return { tasks: dataset.tasks.length, problems: families.length, awaitingEligibility: families.filter(family => family.eligibility !== 'approved').length,
    methods: new Set(dataset.configurations.map(configuration => configuration.method)).size,
    models: new Set(dataset.configurations.flatMap(configuration => configuration.models)).size }
}

export function datasetStatus(dataset: Dataset) {
  const { coverage, audit } = dataset.evidence
  const limits = [
    ...coverage.complete ? [] : [`${coverage.reason} ${coverage.assessedReviews} of ${coverage.requiredReviews} admitted reviews are assessed.`],
    ...audit.state === 'assessed' ? [] : [`The evaluator audit is ${audit.state}.${audit.reason ? ` ${audit.reason}` : ''}`]]
  return { label: limits.length ? 'v1 preview' : 'Current v1', complete: !limits.length, limits }
}

export function conditionDifferences(configurations: Configuration[]) {
  const names = Array.from(new Set(configurations.flatMap(configuration => configuration.conditions.map(condition => condition.name))))
  return names.flatMap(name => {
    const values = configurations.map(configuration => ({ configuration,
      value: configuration.conditions.find(condition => condition.name === name)?.values.join(', ') ?? 'unrecorded' }))
    return new Set(values.map(row => row.value)).size > 1 ? [{ name, values }] : []
  })
}

export const percent = (value: number | null): string => value === null ? '—' : `${value.toFixed(1)}%`
export const money = (value: number | null): string => value === null ? '—' : `$${value.toFixed(value > 0 && value < 0.01 ? 4 : 2)}`
export const compact = (value: number | null): string => value === null ? '—'
  : new Intl.NumberFormat('en-US', { notation: 'compact', maximumFractionDigits: 1 }).format(value)

export const duration = (seconds: number): string => seconds < 60 ? `${Math.round(seconds)} s` : `${(seconds / 60).toFixed(1)} min`
export const share = (measure: Measure): Reading => reading(measure, value => percent(value * 100))
export const perReview = (measure: Measure): Reading => reading(measure, value => value.toFixed(2))
