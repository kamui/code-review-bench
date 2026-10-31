import { z } from 'zod'
import type { Dataset } from './data'
import { bands, estimators } from './scoring'

export const chartViews = ['overview', 'skills', 'setups', 'models', 'tradeoff'] as const
export const axes = ['cost', 'tokens', 'refuted', 'time'] as const
export const scorecardTabs = ['detection', 'delivery', 'reliability', 'remedies', 'controls', 'pre-existing', 'advice', 'usage', 'candidates', 'comparison'] as const
export const drawerTabs = ['reviews', 'references', 'coverage'] as const
export type ScorecardTab = (typeof scorecardTabs)[number]
export type DrawerTab = (typeof drawerTabs)[number]
export type Section = 'scorecard' | 'review-setups' | 'dimensions' | 'serious-misses'
export type Inspection = { kind: 'task' | 'configuration'; id: string; attemptId?: string; tab?: DrawerTab }

// A hand-edited or outdated link should fall back to the default view, not fail the page.
const lenient = <T extends z.ZodType>(schema: T) => schema.optional().catch(undefined)

export const explorerSearch = z.object({
  chart: lenient(z.enum(chartViews)), axis: lenient(z.enum(axes)), band: lenient(z.enum(bands)), average: lenient(z.enum(estimators)),
  experiments: lenient(z.boolean()), labels: lenient(z.boolean()), setups: lenient(z.string()), models: lenient(z.string()),
  dimension: lenient(z.enum(scorecardTabs)), inspect: lenient(z.string()), attempt: lenient(z.string()), drawer: lenient(z.enum(drawerTabs)),
})
export type ExplorerSearch = z.infer<typeof explorerSearch>

export const unlessDefault = <T>(value: T, fallback: T) => value === fallback ? undefined : value

// A selection equal to its default stays out of the link, so the link follows setups added later.
export function listParam(ids: string[], known: string[], defaults: string[]) {
  const chosen = known.filter(id => ids.includes(id))
  return chosen.length === defaults.length && defaults.every(id => chosen.includes(id)) ? undefined : chosen.join(',')
}

export const listValue = (param: string | undefined, known: string[], defaults: string[]) =>
  param === undefined ? defaults : [...new Set(param.split(','))].filter(id => known.includes(id))

export const inspectionParams = (inspection: Inspection | null) => ({
  inspect: inspection ? `${inspection.kind === 'configuration' ? 'setup' : 'task'}:${inspection.id}` : undefined,
  attempt: inspection?.attemptId, drawer: unlessDefault(inspection?.tab, 'reviews'),
})

export function inspectionOf(search: ExplorerSearch, dataset: Dataset): Inspection | null {
  const [, prefix, id] = /^(setup|task):(.+)$/.exec(search.inspect ?? '') ?? []
  const kind = prefix === 'setup' ? 'configuration' : prefix === 'task' ? 'task' : null
  if (!kind || !id || !(kind === 'task' ? dataset.tasks : dataset.configurations).some(item => item.id === id)) return null
  return { kind, id, attemptId: search.attempt, tab: search.drawer }
}
